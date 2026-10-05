use super::{
    pipewire::{CaptureSource, Graph},
    procfs::{BootTime, ProcessIdentity},
};
use crate::model::MicrophoneMuteState;
use anyhow::{Context as _, Result, bail};
use pipewire as pw;
use pw::{
    permissions::PermissionFlags,
    proxy::ProxyT,
    spa::{
        param::{ParamInfoFlags, ParamType},
        pod::Pod,
    },
    types::ObjectType,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    env, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt},
            net::UnixStream,
        },
    },
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_STATE_BYTES: u64 = 1024 * 1024;

/// These controls change signal mute only in the connected PipeWire session.
/// They do not deny capture access, affect direct ALSA/V4L2, or control cameras.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlScope {
    PipewireSessionMute,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    PhysicalOrUnspecified,
    Virtual,
    Monitor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerIdentity {
    pub boot_id: String,
    pub endpoint: PathBuf,
    pub socket_device: u64,
    pub socket_inode: u64,
    pub peer_instance: String,
    pub core_cookie: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceIdentity {
    pub server: ServerIdentity,
    pub global_id: u32,
    pub object_serial: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceCapability {
    pub name: String,
    pub kind: SourceKind,
    pub identity: Option<SourceIdentity>,
    pub muted: Option<bool>,
    pub writable: bool,
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CapabilityReport {
    pub scope: ControlScope,
    pub sources: Vec<SourceCapability>,
    /// Original states retained for explicit restoration, including absent sources.
    pub retained_original_states: usize,
    pub pending_restorations: Vec<PendingRestore>,
    pub lock_active: bool,
    pub lock_suppressed_by_manual_intent: bool,
}

#[derive(Debug, Serialize)]
pub struct PendingRestore {
    pub name: String,
    pub identity: SourceIdentity,
    pub reason: String,
}

#[derive(Default)]
pub struct Control;

impl Control {
    pub fn new() -> Self {
        Self
    }

    pub fn capabilities(&self) -> Result<CapabilityReport> {
        let store = Store::open()?;
        let journal = store.load()?;
        let session = Session::open(Graph::read()?)?;
        let sources = session.capabilities();
        let pending_restorations = journal.entries.iter().filter_map(|entry| {
            let current = sources.iter().find(|source| source.identity.as_ref() == Some(&entry.identity));
            let reason = match current {
                None => "original PipeWire core/source identity is absent or changed; restoration retained".to_owned(),
                Some(source) if !source.writable => source.unsupported_reason.clone().unwrap_or_else(|| "original source is not writable; restoration retained".to_owned()),
                Some(_) if entry.restoring => "restoration intent is pending; retry explicit unmute or lock restoration".to_owned(),
                Some(_) => return None,
            };
            Some(PendingRestore { name: entry.name.clone(), identity: entry.identity.clone(), reason })
        }).collect();
        Ok(CapabilityReport {
            scope: ControlScope::PipewireSessionMute,
            sources,
            retained_original_states: journal.entries.len(),
            pending_restorations,
            lock_active: journal.lock_active,
            lock_suppressed_by_manual_intent: journal.lock_suppressed,
        })
    }

    pub fn microphone_mute_state(&self) -> Result<MicrophoneMuteState> {
        let session = Session::open(Graph::read()?)?;
        Ok(aggregate_state(&session.capabilities()))
    }

    pub fn set_microphone_mute(&self, muted: bool) -> Result<usize> {
        let store = Store::open()?;
        let mut journal = store.load()?;
        self.manual(&store, &mut journal, muted)
    }

    pub fn toggle_microphone_mute(&self) -> Result<bool> {
        let store = Store::open()?;
        let mut journal = store.load()?;
        let mut session = Session::open(Graph::read()?)?;
        let state = aggregate_state(&session.capabilities());
        if state == MicrophoneMuteState::Unavailable {
            bail!("no writable PipeWire capture-source mute controls are available");
        }
        let muted = state != MicrophoneMuteState::Muted;
        prepare_manual(&mut journal, muted);
        store.save(&journal)?;
        if muted {
            mute_sources(&mut journal, &mut session, Owner::Manual, &store)?;
        } else {
            restore_sources(&mut journal, &mut session, None, &store)?;
        }
        Ok(muted)
    }

    fn manual(&self, store: &Store, journal: &mut Journal, muted: bool) -> Result<usize> {
        // Persist the user's intent even if the server is currently unavailable.
        // A delayed unlock must never undo a newer manual request.
        prepare_manual(journal, muted);
        store.save(journal)?;
        if !muted && journal.entries.is_empty() {
            return Ok(0);
        }
        let mut session = Session::open(Graph::read()?)?;
        if muted {
            mute_sources(journal, &mut session, Owner::Manual, store)
        } else {
            restore_sources(journal, &mut session, None, store)
        }
    }

    /// Repeated calls also cover newly connected, currently writable sources.
    /// A manual action suppresses this lock epoch until restore_lock_mute.
    pub fn begin_lock_mute(&self) -> Result<()> {
        let store = Store::open()?;
        let mut journal = store.load()?;
        if journal.lock_active && journal.lock_suppressed {
            return Ok(());
        }
        journal.lock_active = true;
        store.save(&journal)?;
        let mut session = Session::open(Graph::read()?)?;
        mute_sources(&mut journal, &mut session, Owner::Lock, &store)?;
        Ok(())
    }

    pub fn restore_lock_mute(&self) -> Result<()> {
        let store = Store::open()?;
        let mut journal = store.load()?;
        journal.lock_active = false;
        journal.lock_suppressed = false;
        for entry in &mut journal.entries {
            if entry.owner == Owner::Lock {
                entry.restoring = true;
            }
        }
        store.save(&journal)?;
        if !journal
            .entries
            .iter()
            .any(|entry| entry.owner == Owner::Lock)
        {
            return Ok(());
        }
        let mut session = Session::open(Graph::read()?)?;
        restore_sources(&mut journal, &mut session, Some(Owner::Lock), &store)?;
        Ok(())
    }
}

fn aggregate_state(sources: &[SourceCapability]) -> MicrophoneMuteState {
    let mut muted = false;
    let mut unmuted = false;
    for source in sources.iter().filter(|source| source.writable) {
        match source.muted {
            Some(true) => muted = true,
            Some(false) => unmuted = true,
            None => return MicrophoneMuteState::Unavailable,
        }
    }
    match (muted, unmuted) {
        (true, true) => MicrophoneMuteState::Mixed,
        (true, false) => MicrophoneMuteState::Muted,
        (false, true) => MicrophoneMuteState::Unmuted,
        (false, false) => MicrophoneMuteState::Unavailable,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Owner {
    Manual,
    Lock,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    identity: SourceIdentity,
    name: String,
    original_muted: bool,
    owner: Owner,
    restoring: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    lock_active: bool,
    lock_suppressed: bool,
    entries: Vec<Entry>,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            version: 1,
            lock_active: false,
            lock_suppressed: false,
            entries: Vec::new(),
        }
    }
}

fn prepare_manual(journal: &mut Journal, muted: bool) {
    if journal.lock_active {
        journal.lock_suppressed = true;
    }
    for entry in &mut journal.entries {
        entry.owner = Owner::Manual;
        entry.restoring = !muted;
    }
}

fn plan_mute(
    journal: &mut Journal,
    sources: &[SourceCapability],
    owner: Owner,
) -> Vec<SourceIdentity> {
    let mut work = Vec::new();
    for source in sources.iter().filter(|source| source.writable) {
        let (Some(identity), Some(muted)) = (&source.identity, source.muted) else {
            continue;
        };
        if let Some(entry) = journal
            .entries
            .iter_mut()
            .find(|entry| &entry.identity == identity)
        {
            // A lock must not take ownership of manually muted sources.
            if owner == Owner::Manual || entry.owner == owner {
                entry.restoring = false;
            }
        } else if !muted {
            journal.entries.push(Entry {
                identity: identity.clone(),
                name: source.name.clone(),
                original_muted: muted,
                owner,
                restoring: false,
            });
        }
        if !muted {
            work.push(identity.clone());
        }
    }
    work
}

fn mute_sources(
    journal: &mut Journal,
    session: &mut Session,
    owner: Owner,
    store: &Store,
) -> Result<usize> {
    let sources = session.capabilities();
    let writable = sources.iter().filter(|source| source.writable).count();
    if writable == 0 {
        bail!(
            "no writable PipeWire capture-source mute controls are available: {}",
            capability_errors(&sources)
        );
    }
    let work = plan_mute(journal, &sources, owner);
    // Every original state and desired mute is durable before any native mutation.
    store.save(journal)?;
    let mut changed = 0;
    let mut errors = Vec::new();
    for identity in work {
        match session.set_mute(&identity, true) {
            Ok(()) => changed += 1,
            Err(error) => errors.push(format!(
                "source {} serial {}: {error:#}",
                identity.global_id, identity.object_serial
            )),
        }
    }
    // Unsupported sources are explicit partial coverage, not a global mute success.
    errors.extend(
        sources
            .iter()
            .filter(|source| !source.writable)
            .map(|source| {
                format!(
                    "{}: {}",
                    source.name,
                    source
                        .unsupported_reason
                        .as_deref()
                        .unwrap_or("unsupported mute control")
                )
            }),
    );
    finish(changed, errors, "mute")
}

fn restore_sources(
    journal: &mut Journal,
    session: &mut Session,
    owner: Option<Owner>,
    store: &Store,
) -> Result<usize> {
    for entry in &mut journal.entries {
        if owner.is_none_or(|owner| entry.owner == owner) {
            entry.restoring = true;
        }
    }
    store.save(journal)?;
    restore_entries(
        journal,
        owner,
        |identity, original| session.set_mute(identity, original),
        |journal| store.save(journal),
    )
}

fn restore_entries(
    journal: &mut Journal,
    owner: Option<Owner>,
    mut restore: impl FnMut(&SourceIdentity, bool) -> Result<()>,
    mut save: impl FnMut(&Journal) -> Result<()>,
) -> Result<usize> {
    let mut restored = 0;
    let mut errors = Vec::new();
    let mut index = 0;
    while index < journal.entries.len() {
        let entry = &journal.entries[index];
        if owner.is_some_and(|owner| entry.owner != owner) {
            index += 1;
            continue;
        }
        match restore(&entry.identity, entry.original_muted) {
            Ok(()) => {
                journal.entries.remove(index);
                // Crash after native restoration but before this save is safe:
                // the next readback sees the original state and removes the intent.
                save(journal)?;
                restored += 1;
            }
            Err(error) => {
                errors.push(format!(
                    "{} (source {}, serial {}): {error:#}; original state retained",
                    entry.name, entry.identity.global_id, entry.identity.object_serial
                ));
                index += 1;
            }
        }
    }
    finish(restored, errors, "restore")
}

fn finish(changed: usize, errors: Vec<String>, action: &str) -> Result<usize> {
    if errors.is_empty() {
        Ok(changed)
    } else {
        bail!(
            "PipeWire-session {action} incomplete ({changed} source changes verified): {}",
            errors.join("; ")
        )
    }
}

fn capability_errors(sources: &[SourceCapability]) -> String {
    if sources.is_empty() {
        return "no capture sources in this session".to_owned();
    }
    sources
        .iter()
        .map(|source| {
            format!(
                "{}: {}",
                source.name,
                source
                    .unsupported_reason
                    .as_deref()
                    .unwrap_or("mute readback unavailable")
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Default)]
struct NativeState {
    verified: bool,
    removed: bool,
    props_writable: bool,
    muted: Option<bool>,
    mute_readonly: bool,
}

struct BoundNode {
    // Remove listeners before destroying the bound proxy.
    _listener: pw::node::NodeListener,
    node: pw::node::Node,
    state: Rc<RefCell<NativeState>>,
    permission_writable: bool,
    serial: u64,
}

struct Session {
    _registry_listener: pw::registry::Listener,
    _core_listener: pw::core::Listener,
    nodes: Rc<RefCell<HashMap<u32, BoundNode>>>,
    _registry: Rc<pw::registry::Registry>,
    core: pw::core::Core,
    _context: pw::context::Context,
    main_loop: pw::main_loop::MainLoop,
    fatal: Rc<RefCell<Option<String>>>,
    errors: Rc<RefCell<HashMap<u32, String>>>,
    identity: ServerIdentity,
    inventory: Vec<CaptureSource>,
}

impl Session {
    fn open(graph: Graph) -> Result<Self> {
        let cookie = graph
            .core_cookie()
            .context("PipeWire graph lacks an authoritative core cookie; no control attempted")?;
        let inventory = graph.capture_sources();
        let (identity, socket) = server_identity(cookie)?;
        pw::init();
        let main_loop = pw::main_loop::MainLoop::new(None)?;
        let context = pw::context::Context::new(&main_loop)?;
        let core = context
            .connect_fd(socket.into(), None)
            .context("cannot connect native PipeWire control client")?;
        let fatal = Rc::new(RefCell::new(None));
        let errors = Rc::new(RefCell::new(HashMap::new()));
        let core_cookie = Rc::new(Cell::new(None));
        let cookie_capture = core_cookie.clone();
        let fatal_capture = fatal.clone();
        let errors_capture = errors.clone();
        let core_listener = core
            .add_listener_local()
            .info(move |info| cookie_capture.set(Some(info.cookie())))
            .error(move |id, _seq, result, message| {
                let message = format!("native PipeWire error {result}: {message}");
                if id == pw::core::PW_ID_CORE {
                    *fatal_capture.borrow_mut() = Some(message);
                } else {
                    errors_capture.borrow_mut().insert(id, message);
                }
            })
            .register();
        let registry = Rc::new(core.get_registry()?);
        let registry_weak = Rc::downgrade(&registry);
        let expected: HashMap<_, _> = inventory
            .iter()
            .filter_map(|source| Some((source.id?, source.serial?)))
            .collect();
        let nodes = Rc::new(RefCell::new(HashMap::<u32, BoundNode>::new()));
        let nodes_capture = nodes.clone();
        let nodes_removed = nodes.clone();
        let listener = registry
            .add_listener_local()
            .global(move |global| {
                if global.type_ != ObjectType::Node {
                    return;
                }
                let Some(&serial) = expected.get(&global.id) else {
                    return;
                };
                let observed_serial = global
                    .props
                    .and_then(|props| props.get("object.serial"))
                    .and_then(|value| value.parse::<u64>().ok());
                if observed_serial != Some(serial) {
                    return;
                }
                let Some(registry) = registry_weak.upgrade() else {
                    return;
                };
                let Ok(node) = registry.bind::<pw::node::Node, _>(global) else {
                    return;
                };
                let state = Rc::new(RefCell::new(NativeState::default()));
                let info_state = state.clone();
                let param_state = state.clone();
                let node_listener = node
                    .add_listener_local()
                    .info(move |info| {
                        let mut state = info_state.borrow_mut();
                        // Node info events are deltas. A parameter update may carry
                        // an empty Props dictionary; only PROPS changes replace
                        // the identity evidence established by the initial event.
                        if info.change_mask().contains(pw::node::NodeChangeMask::PROPS) {
                            state.verified = info.props().is_some_and(|props| {
                                props
                                    .get("object.serial")
                                    .and_then(|value| value.parse::<u64>().ok())
                                    == Some(serial)
                                    && matches!(
                                        props.get("media.class"),
                                        Some("Audio/Source" | "Audio/Source/Virtual")
                                    )
                            });
                        }
                        if info
                            .change_mask()
                            .contains(pw::node::NodeChangeMask::PARAMS)
                        {
                            state.props_writable = info
                                .params()
                                .iter()
                                .find(|param| param.id() == ParamType::Props)
                                .is_some_and(|props| {
                                    props.flags().contains(ParamInfoFlags::READWRITE)
                                });
                        }
                    })
                    .param(move |_seq, id, _index, _next, pod| {
                        if id != ParamType::Props {
                            return;
                        }
                        if let Some((muted, readonly)) =
                            pod.and_then(|pod| mute_property(pod.as_bytes()))
                        {
                            let mut state = param_state.borrow_mut();
                            state.muted = Some(muted);
                            state.mute_readonly = readonly;
                        }
                    })
                    .register();
                node.enum_params(1, Some(ParamType::Props), 0, u32::MAX);
                nodes_capture.borrow_mut().insert(
                    global.id,
                    BoundNode {
                        _listener: node_listener,
                        node,
                        state,
                        permission_writable: global
                            .permissions
                            .contains(PermissionFlags::R | PermissionFlags::W | PermissionFlags::X),
                        serial,
                    },
                );
            })
            .global_remove(move |id| {
                if let Some(node) = nodes_removed.borrow().get(&id) {
                    node.state.borrow_mut().removed = true;
                }
            })
            .register();
        let session = Self {
            _registry_listener: listener,
            _core_listener: core_listener,
            nodes,
            _registry: registry,
            core,
            _context: context,
            main_loop,
            fatal,
            errors,
            identity,
            inventory,
        };
        // Binding/parameter requests submitted during registry dispatch need a
        // second barrier; the first only completes the registry enumeration.
        session.roundtrip()?;
        session.roundtrip()?;
        if core_cookie.get() != Some(cookie) || server_identity(cookie)?.0 != session.identity {
            bail!(
                "PipeWire core/session changed between graph inventory and native binding; no control attempted"
            );
        }
        Ok(session)
    }

    fn roundtrip(&self) -> Result<()> {
        let pending = self.core.sync(0)?;
        let done = Rc::new(Cell::new(false));
        let done_capture = done.clone();
        let _listener = self
            .core
            .add_listener_local()
            .done(move |id, seq| {
                if id == pw::core::PW_ID_CORE && seq == pending {
                    done_capture.set(true);
                }
            })
            .register();
        let deadline = Instant::now() + TIMEOUT;
        while !done.get() {
            if let Some(error) = self.fatal.borrow().as_ref() {
                bail!("{error}");
            }
            let now = Instant::now();
            if now >= deadline {
                bail!("native PipeWire control timed out; original restoration state retained");
            }
            if self
                .main_loop
                .loop_()
                .iterate((deadline - now).min(Duration::from_millis(50)))
                < 0
            {
                bail!("native PipeWire loop iteration failed");
            }
        }
        if let Some(error) = self.fatal.borrow().as_ref() {
            bail!("{error}");
        }
        Ok(())
    }

    fn capabilities(&self) -> Vec<SourceCapability> {
        let nodes = self.nodes.borrow();
        self.inventory
            .iter()
            .map(|source| {
                let kind = if source.monitor {
                    SourceKind::Monitor
                } else if source.virtual_source {
                    SourceKind::Virtual
                } else {
                    SourceKind::PhysicalOrUnspecified
                };
                let node = source.id.and_then(|id| nodes.get(&id));
                let state = node.map(|node| node.state.borrow());
                let identity = match (source.id, source.serial) {
                    (Some(id), Some(serial)) if serial != 0 => Some(SourceIdentity {
                        server: self.identity.clone(),
                        global_id: id,
                        object_serial: serial,
                    }),
                    _ => None,
                };
                let error = node.and_then(|node| {
                    self.errors
                        .borrow()
                        .get(&node.node.upcast_ref().id())
                        .cloned()
                });
                let reason = if source.monitor {
                    Some("sink-monitor source is not a microphone control".to_owned())
                } else if identity.is_none() {
                    Some("source lacks an authoritative global ID/object.serial".to_owned())
                } else if node.is_none() {
                    Some(
                        "source disappeared or its serial changed before native binding".to_owned(),
                    )
                } else if state
                    .as_ref()
                    .is_some_and(|state| !state.verified || state.removed)
                {
                    Some("bound source identity is unverified or removed".to_owned())
                } else if let Some(error) = error {
                    Some(error)
                } else if node.is_some_and(|node| !node.permission_writable) {
                    Some("native PipeWire proxy lacks read/write/execute permission".to_owned())
                } else if state
                    .as_ref()
                    .is_some_and(|state| !state.props_writable || state.mute_readonly)
                {
                    Some("source does not expose writable Props/mute".to_owned())
                } else if state.as_ref().is_none_or(|state| state.muted.is_none()) {
                    Some("source has no authoritative boolean mute readback".to_owned())
                } else {
                    None
                };
                SourceCapability {
                    name: source.name.clone(),
                    kind,
                    identity,
                    muted: state.as_ref().and_then(|state| state.muted),
                    writable: reason.is_none(),
                    unsupported_reason: reason,
                }
            })
            .collect()
    }

    fn set_mute(&mut self, identity: &SourceIdentity, muted: bool) -> Result<()> {
        if identity.server != self.identity {
            bail!(
                "original server identity no longer matches; refusing to change any replacement source"
            );
        }
        let (proxy_id, state) = {
            let nodes = self.nodes.borrow();
            let node = nodes
                .get(&identity.global_id)
                .filter(|node| node.serial == identity.object_serial)
                .context("original source serial no longer exists; refusing a reused global ID")?;
            let state = node.state.borrow();
            if !state.verified
                || state.removed
                || !node.permission_writable
                || !state.props_writable
                || state.mute_readonly
            {
                bail!("original bound source is unavailable or not writable");
            }
            drop(state);
            (node.node.upcast_ref().id(), node.state.clone())
        };
        self.errors.borrow_mut().remove(&proxy_id);
        // Refresh before even an idempotent success: a stale snapshot must not
        // claim restoration without observing the bound source's current state.
        self.refresh_mute(identity, &state)?;
        {
            let current = state.borrow();
            if !current.verified
                || current.removed
                || !current.props_writable
                || current.mute_readonly
            {
                bail!("original bound source is unavailable or not writable");
            }
            if current.muted == Some(muted) {
                return Ok(());
            }
            if current.muted.is_none() {
                bail!("source mute state cannot be read; no mutation attempted");
            }
        }
        let pod = MutePod::new(muted);
        {
            let nodes = self.nodes.borrow();
            let node = nodes
                .get(&identity.global_id)
                .context("bound source disappeared")?;
            node.node.set_param(ParamType::Props, 0, pod.pod()?);
        }
        self.roundtrip()?;
        // Core sync orders protocol requests, not the adapter's asynchronous
        // data-loop mutation. Request fresh Props until native readback converges;
        // never resend the mutation or discard restoration intent on a mismatch.
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.refresh_mute(identity, &state)?;
            if let Some(error) = self.errors.borrow().get(&proxy_id) {
                bail!("{error}");
            }
            {
                let current = state.borrow();
                if !current.verified
                    || current.removed
                    || !current.props_writable
                    || current.mute_readonly
                {
                    bail!(
                        "original bound source is unavailable or not writable (identity_verified={}, removed={}, props_writable={}, mute_readonly={}); restoration intent retained",
                        current.verified,
                        current.removed,
                        current.props_writable,
                        current.mute_readonly
                    );
                }
                if current.muted == Some(muted) {
                    return Ok(());
                }
            }
            let now = Instant::now();
            if now >= deadline {
                bail!(
                    "mute readback did not confirm requested state {muted}; original restoration state retained"
                );
            }
            if self
                .main_loop
                .loop_()
                .iterate((deadline - now).min(Duration::from_millis(50)))
                < 0
            {
                bail!("native PipeWire loop iteration failed");
            }
        }
    }

    fn refresh_mute(
        &self,
        identity: &SourceIdentity,
        state: &Rc<RefCell<NativeState>>,
    ) -> Result<()> {
        state.borrow_mut().muted = None;
        {
            let nodes = self.nodes.borrow();
            let node = nodes
                .get(&identity.global_id)
                .filter(|node| node.serial == identity.object_serial)
                .context("original bound source disappeared")?;
            node.node
                .enum_params(2, Some(ParamType::Props), 0, u32::MAX);
        }
        self.roundtrip()
    }
}

// Props is a stable SPA ABI: object body, property header, boolean pod. An
// aligned fixed buffer avoids allocating a general Value tree for one boolean.
#[repr(C, align(8))]
struct MutePod([u32; 10]);

impl MutePod {
    fn new(muted: bool) -> Self {
        Self([
            32,
            pw::spa::sys::SPA_TYPE_Object,
            pw::spa::sys::SPA_TYPE_OBJECT_Props,
            ParamType::Props.as_raw(),
            pw::spa::sys::SPA_PROP_mute,
            0,
            4,
            pw::spa::sys::SPA_TYPE_Bool,
            u32::from(muted),
            0,
        ])
    }

    fn pod(&self) -> Result<&Pod> {
        // All bytes are initialized u32s; repr(C) plus align(8) satisfies SPA.
        let bytes = unsafe {
            std::slice::from_raw_parts(self.0.as_ptr().cast::<u8>(), std::mem::size_of_val(&self.0))
        };
        Pod::from_bytes(bytes).context("invalid native mute pod")
    }
}

fn mute_property(bytes: &[u8]) -> Option<(bool, bool)> {
    fn word(bytes: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_ne_bytes(
            bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
        ))
    }
    let end = usize::try_from(word(bytes, 0)?).ok()?.checked_add(8)?;
    if end > bytes.len()
        || end < 16
        || word(bytes, 4)? != pw::spa::sys::SPA_TYPE_Object
        || word(bytes, 8)? != pw::spa::sys::SPA_TYPE_OBJECT_Props
    {
        return None;
    }
    let mut offset = 16usize;
    while offset < end {
        let key = word(bytes, offset)?;
        let flags = word(bytes, offset + 4)?;
        let size = usize::try_from(word(bytes, offset + 8)?).ok()?;
        let pod_end = offset.checked_add(16)?.checked_add(size)?;
        if pod_end > end {
            return None;
        }
        if key == pw::spa::sys::SPA_PROP_mute {
            if size != 4 || word(bytes, offset + 12)? != pw::spa::sys::SPA_TYPE_Bool {
                return None;
            }
            return Some((
                word(bytes, offset + 16)? != 0,
                flags & pw::spa::sys::SPA_POD_PROP_FLAG_READONLY != 0,
            ));
        }
        offset = pod_end.checked_add(7)? & !7;
    }
    None
}

fn server_identity(cookie: u32) -> Result<(ServerIdentity, UnixStream)> {
    let remote = env::var_os("PIPEWIRE_REMOTE").unwrap_or_else(|| "pipewire-0".into());
    let remote = PathBuf::from(remote);
    let endpoint = if remote.is_absolute() {
        remote
    } else {
        if remote.components().count() != 1 {
            bail!("nonstandard PIPEWIRE_REMOTE path has no safely identifiable endpoint");
        }
        let runtime = env::var_os("PIPEWIRE_RUNTIME_DIR").or_else(|| env::var_os("XDG_RUNTIME_DIR"))
            .context("PIPEWIRE_RUNTIME_DIR/XDG_RUNTIME_DIR is missing; cannot identify the session socket")?;
        PathBuf::from(runtime).join(remote)
    };
    let before = fs::symlink_metadata(&endpoint)
        .with_context(|| format!("cannot identify PipeWire socket {}", endpoint.display()))?;
    if !before.file_type().is_socket() {
        bail!("PipeWire endpoint is not a native Unix socket");
    }
    let socket =
        UnixStream::connect(&endpoint).context("cannot verify PipeWire server peer identity")?;
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if result != 0 || len as usize != std::mem::size_of::<libc::ucred>() || credentials.pid <= 0 {
        bail!(
            "cannot obtain authoritative PipeWire socket peer credentials: {}",
            std::io::Error::last_os_error()
        );
    }
    let peer = ProcessIdentity::verify(credentials.pid as u32, &BootTime::read()?)?;
    if peer.uid != credentials.uid {
        bail!("PipeWire socket peer process changed during identity verification");
    }
    let after = fs::symlink_metadata(&endpoint)?;
    if before.dev() != after.dev() || before.ino() != after.ino() {
        bail!("PipeWire socket changed during identity verification");
    }
    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_owned();
    if boot_id.is_empty() {
        bail!("Linux boot identity is unavailable");
    }
    Ok((
        ServerIdentity {
            boot_id,
            endpoint,
            socket_device: before.dev(),
            socket_inode: before.ino(),
            peer_instance: peer.instance_id,
            core_cookie: cookie,
        },
        socket,
    ))
}

struct Store {
    directory: PathBuf,
    _lock: File,
}

impl Store {
    fn open() -> Result<Self> {
        let root = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .context("HOME/XDG_STATE_HOME is missing; cannot persist original mute states")?;
        let directory = root.join("miccamwatch");
        if !directory.exists() {
            fs::create_dir_all(&root)?;
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error.into()),
            }
        }
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            bail!(
                "mute state directory {} must be a private user-owned directory (0700), not a symlink",
                directory.display()
            );
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory.join("pipewire-mute.lock"))?;
        validate_private_file(&lock)?;
        let start = Instant::now();
        loop {
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(error.into());
            }
            if start.elapsed() >= TIMEOUT {
                bail!("another process is changing PipeWire mute state; lock timed out");
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(Self {
            directory,
            _lock: lock,
        })
    }

    fn load(&self) -> Result<Journal> {
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(self.directory.join("pipewire-mute.json"))
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Journal::default());
            }
            Err(error) => return Err(error.into()),
        };
        validate_private_file(&file)?;
        let mut bytes = Vec::new();
        file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            bail!("PipeWire original-state journal exceeds its size limit");
        }
        let journal: Journal = serde_json::from_slice(&bytes).context(
            "invalid PipeWire original-state journal; refusing to discard restoration data",
        )?;
        if journal.version != 1 {
            bail!(
                "unsupported PipeWire original-state journal version {}",
                journal.version
            );
        }
        for (index, entry) in journal.entries.iter().enumerate() {
            if entry.identity.object_serial == 0
                || entry.identity.server.boot_id.is_empty()
                || journal.entries[..index]
                    .iter()
                    .any(|other| other.identity == entry.identity)
            {
                bail!(
                    "invalid or duplicate original source identity in PipeWire restoration journal"
                );
            }
        }
        Ok(journal)
    }

    fn save(&self, journal: &Journal) -> Result<()> {
        static TEMP_ID: AtomicU64 = AtomicU64::new(0);
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let temp = self.directory.join(format!(
            ".pipewire-mute.{}.{}.{}.tmp",
            std::process::id(),
            now,
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&temp)?;
            serde_json::to_writer(&mut file, journal)?;
            file.flush()?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, self.directory.join("pipewire-mute.json"))?;
            File::open(&self.directory)?.sync_all()?;
            Ok::<_, anyhow::Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

fn validate_private_file(file: &File) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        bail!("PipeWire mute state must be a private, single-link, user-owned regular file");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    fn identity(cookie: u32, serial: u64) -> SourceIdentity {
        SourceIdentity {
            server: ServerIdentity {
                boot_id: "boot-a".to_owned(),
                endpoint: "/run/user/1000/pipewire-0".into(),
                socket_device: 1,
                socket_inode: 2,
                peer_instance: "linux:42:100:20".to_owned(),
                core_cookie: cookie,
            },
            global_id: 5,
            object_serial: serial,
        }
    }

    fn source(identity: SourceIdentity, muted: bool) -> SourceCapability {
        SourceCapability {
            name: "source".to_owned(),
            kind: SourceKind::Virtual,
            identity: Some(identity),
            muted: Some(muted),
            writable: true,
            unsupported_reason: None,
        }
    }

    #[test]
    fn premuted_sources_never_acquire_restore_intent_and_repeated_mute_preserves_original() {
        let mut journal = Journal::default();
        let first = identity(10, 1);
        let second = identity(10, 2);
        let work = plan_mute(
            &mut journal,
            &[source(first.clone(), false), source(second, true)],
            Owner::Manual,
        );
        assert_eq!(work, vec![first.clone()]);
        assert_eq!(journal.entries.len(), 1);
        assert!(!journal.entries[0].original_muted);
        assert!(plan_mute(&mut journal, &[source(first, true)], Owner::Manual).is_empty());
        assert!(!journal.entries[0].original_muted);
    }

    #[test]
    fn replacement_core_or_serial_never_inherits_the_original_restore_identity() {
        let old = identity(10, 1);
        let mut journal = Journal::default();
        plan_mute(&mut journal, &[source(old.clone(), false)], Owner::Manual);
        let replacements = [source(identity(11, 1), true), source(identity(10, 2), true)];
        assert!(
            replacements
                .iter()
                .all(|source| source.identity.as_ref() != Some(&old))
        );
        let result = restore_entries(
            &mut journal,
            None,
            |id, _| {
                if replacements
                    .iter()
                    .any(|source| source.identity.as_ref() == Some(id))
                {
                    Ok(())
                } else {
                    Err(anyhow!("original identity absent"))
                }
            },
            |_| Ok(()),
        );
        assert!(result.is_err());
        assert_eq!(journal.entries[0].identity, old);
    }

    #[test]
    fn partial_restore_retains_only_failed_sources_for_retry() {
        let first = identity(10, 1);
        let second = identity(10, 2);
        let mut journal = Journal::default();
        plan_mute(
            &mut journal,
            &[source(first.clone(), false), source(second.clone(), false)],
            Owner::Lock,
        );
        let result = restore_entries(
            &mut journal,
            Some(Owner::Lock),
            |id, original| {
                assert!(!original);
                if id == &second {
                    bail!("device disconnected")
                } else {
                    Ok(())
                }
            },
            |_| Ok(()),
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("1 source changes verified")
        );
        assert_eq!(journal.entries.len(), 1);
        assert_eq!(journal.entries[0].identity, second);
        assert_eq!(
            restore_entries(&mut journal, Some(Owner::Lock), |_, _| Ok(()), |_| Ok(())).unwrap(),
            1
        );
        assert!(journal.entries.is_empty());
    }

    #[test]
    fn manual_intent_prevents_stale_unlock_from_reverting_a_new_manual_mute() {
        let mut journal = Journal {
            lock_active: true,
            ..Journal::default()
        };
        plan_mute(&mut journal, &[source(identity(10, 1), false)], Owner::Lock);
        prepare_manual(&mut journal, true);
        assert!(journal.lock_suppressed);
        assert_eq!(journal.entries[0].owner, Owner::Manual);
        assert_eq!(
            restore_entries(
                &mut journal,
                Some(Owner::Lock),
                |_, _| panic!("unlock must not touch manual sources"),
                |_| Ok(())
            )
            .unwrap(),
            0
        );
        assert_eq!(journal.entries.len(), 1);
        prepare_manual(&mut journal, false);
        assert!(journal.entries[0].restoring);
    }

    #[test]
    fn spa_mute_decode_rejects_wrong_types_truncation_and_readonly_is_explicit() {
        let pod = MutePod::new(true);
        let bytes = pod.pod().unwrap().as_bytes();
        assert_eq!(mute_property(bytes), Some((true, false)));
        assert_eq!(mute_property(&bytes[..bytes.len() - 1]), None);
        let mut wrong = MutePod::new(false);
        wrong.0[7] = pw::spa::sys::SPA_TYPE_Int;
        assert_eq!(mute_property(wrong.pod().unwrap().as_bytes()), None);
        wrong.0[7] = pw::spa::sys::SPA_TYPE_Bool;
        wrong.0[5] = pw::spa::sys::SPA_POD_PROP_FLAG_READONLY;
        assert_eq!(
            mute_property(wrong.pod().unwrap().as_bytes()),
            Some((false, true))
        );
    }
}
