//! Lazy GTK 3 C-ABI desktop. Only the dedicated owner thread touches GTK.
//! Successful loader handles remain process-resident: GTK registers process-global
//! GTypes whose function pointers must not be invalidated by dlclose.
use super::{Command, View, diagnostic_document};
use anyhow::{Context, Result, bail, ensure};
use std::{
    cell::Cell,
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    rc::Rc,
    sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{self, Receiver, SyncSender}},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type Widget = *mut c_void;
type Callback = unsafe extern "C" fn();
// Public GtkTextIter ABI from GTK 3 gtktextiter.h (not an opaque guessed byte array).
#[repr(C)]
#[derive(Default)]
struct TextIter {
    dummy1: *mut c_void, dummy2: *mut c_void,
    dummy3: c_int, dummy4: c_int, dummy5: c_int, dummy6: c_int,
    dummy7: c_int, dummy8: c_int, dummy9: *mut c_void, dummy10: *mut c_void,
    dummy11: c_int, dummy12: c_int, dummy13: c_int, dummy14: *mut c_void,
}

macro_rules! gtk_api {
    ($($name:ident: $ty:ty),+ $(,)?) => {
        struct Api { $($name: $ty,)+ }
        impl Api {
            fn load() -> Result<Self> {
                // RTLD_LOCAL prevents exporting GTK ABI into unrelated plugin code.
                let handle = unsafe { libc::dlopen(c"libgtk-3.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
                ensure!(!handle.is_null(), "Native window unavailable: install the GTK 3 runtime (Ubuntu/Debian: libgtk-3-0; Fedora: gtk3) and run mcw tray open in your graphical desktop session. CLI and tray remain available.");
                let loaded = (|| -> Result<Self> {
                    Ok(Self { $($name: {
                        let symbol = unsafe { libc::dlsym(handle, concat!(stringify!($name), "\0").as_ptr().cast()) };
                        ensure!(!symbol.is_null(), "GTK 3 runtime lacks required ABI symbol {}; install a complete GTK 3 runtime", stringify!($name));
                        // Every signature below is the stable GTK3/GLib/GObject C ABI.
                        unsafe { std::mem::transmute::<*mut c_void, $ty>(symbol) }
                    },)+ })
                })();
                if loaded.is_err() { unsafe { libc::dlclose(handle); } }
                loaded
            }
        }
    };
}
gtk_api! {
    gtk_init_check: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int,
    gtk_window_new: unsafe extern "C" fn(c_int) -> Widget,
    gtk_window_set_title: unsafe extern "C" fn(Widget, *const c_char),
    gtk_window_set_default_size: unsafe extern "C" fn(Widget, c_int, c_int),
    gtk_window_set_icon_name: unsafe extern "C" fn(Widget, *const c_char),
    gtk_window_present: unsafe extern "C" fn(Widget),
    gtk_widget_set_size_request: unsafe extern "C" fn(Widget, c_int, c_int),
    gtk_widget_show_all: unsafe extern "C" fn(Widget),
    gtk_widget_hide: unsafe extern "C" fn(Widget),
    gtk_widget_destroy: unsafe extern "C" fn(Widget),
    gtk_widget_get_visible: unsafe extern "C" fn(Widget) -> c_int,
    gtk_widget_set_sensitive: unsafe extern "C" fn(Widget, c_int),
    gtk_widget_set_tooltip_text: unsafe extern "C" fn(Widget, *const c_char),
    gtk_widget_get_accessible: unsafe extern "C" fn(Widget) -> Widget,
    atk_object_set_name: unsafe extern "C" fn(Widget, *const c_char),
    gtk_container_add: unsafe extern "C" fn(Widget, Widget),
    gtk_container_set_border_width: unsafe extern "C" fn(Widget, c_uint),
    gtk_box_new: unsafe extern "C" fn(c_int, c_int) -> Widget,
    gtk_box_pack_start: unsafe extern "C" fn(Widget, Widget, c_int, c_int, c_uint),
    gtk_label_new: unsafe extern "C" fn(*const c_char) -> Widget,
    gtk_label_set_text: unsafe extern "C" fn(Widget, *const c_char),
    gtk_label_get_text: unsafe extern "C" fn(Widget) -> *const c_char,
    gtk_label_set_xalign: unsafe extern "C" fn(Widget, f32),
    gtk_label_set_line_wrap: unsafe extern "C" fn(Widget, c_int),
    gtk_label_set_line_wrap_mode: unsafe extern "C" fn(Widget, c_int),
    gtk_label_set_selectable: unsafe extern "C" fn(Widget, c_int),
    gtk_label_set_max_width_chars: unsafe extern "C" fn(Widget, c_int),
    gtk_frame_new: unsafe extern "C" fn(*const c_char) -> Widget,
    gtk_notebook_new: unsafe extern "C" fn() -> Widget,
    gtk_notebook_append_page: unsafe extern "C" fn(Widget, Widget, Widget) -> c_int,
    gtk_scrolled_window_new: unsafe extern "C" fn(Widget, Widget) -> Widget,
    gtk_scrolled_window_set_policy: unsafe extern "C" fn(Widget, c_int, c_int),
    gtk_scrolled_window_get_vadjustment: unsafe extern "C" fn(Widget) -> Widget,
    gtk_scrolled_window_get_hadjustment: unsafe extern "C" fn(Widget) -> Widget,
    gtk_adjustment_get_value: unsafe extern "C" fn(Widget) -> f64,
    gtk_adjustment_set_value: unsafe extern "C" fn(Widget, f64),
    gtk_button_new_with_label: unsafe extern "C" fn(*const c_char) -> Widget,
    gtk_button_set_label: unsafe extern "C" fn(Widget, *const c_char),
    gtk_check_button_new_with_label: unsafe extern "C" fn(*const c_char) -> Widget,
    gtk_toggle_button_get_active: unsafe extern "C" fn(Widget) -> c_int,
    gtk_toggle_button_set_active: unsafe extern "C" fn(Widget, c_int),
    gtk_combo_box_text_new: unsafe extern "C" fn() -> Widget,
    gtk_combo_box_text_append_text: unsafe extern "C" fn(Widget, *const c_char),
    gtk_combo_box_get_active: unsafe extern "C" fn(Widget) -> c_int,
    gtk_combo_box_set_active: unsafe extern "C" fn(Widget, c_int),
    gtk_text_view_new: unsafe extern "C" fn() -> Widget,
    gtk_text_view_set_editable: unsafe extern "C" fn(Widget, c_int),
    gtk_text_view_set_cursor_visible: unsafe extern "C" fn(Widget, c_int),
    gtk_text_view_set_wrap_mode: unsafe extern "C" fn(Widget, c_int),
    gtk_text_view_set_monospace: unsafe extern "C" fn(Widget, c_int),
    gtk_text_view_get_buffer: unsafe extern "C" fn(Widget) -> Widget,
    gtk_text_buffer_set_text: unsafe extern "C" fn(Widget, *const c_char, c_int),
    gtk_text_buffer_get_insert: unsafe extern "C" fn(Widget) -> Widget,
    gtk_text_buffer_get_selection_bound: unsafe extern "C" fn(Widget) -> Widget,
    gtk_text_buffer_get_iter_at_mark: unsafe extern "C" fn(Widget, *mut TextIter, Widget),
    gtk_text_iter_get_offset: unsafe extern "C" fn(*const TextIter) -> c_int,
    gtk_text_buffer_get_iter_at_offset: unsafe extern "C" fn(Widget, *mut TextIter, c_int),
    gtk_text_buffer_select_range: unsafe extern "C" fn(Widget, *const TextIter, *const TextIter),
    g_signal_connect_data: unsafe extern "C" fn(Widget, *const c_char, Option<Callback>, *mut c_void, Option<unsafe extern "C" fn(*mut c_void, Widget)>, c_uint) -> libc::c_ulong,
    g_main_context_iteration: unsafe extern "C" fn(Widget, c_int) -> c_int,
    gdk_event_get_keyval: unsafe extern "C" fn(Widget, *mut c_uint) -> c_int,
    gdk_event_get_state: unsafe extern "C" fn(Widget, *mut c_uint) -> c_int,
}

struct Shared {
    view: Mutex<View>, revision: AtomicU64, open: AtomicBool, stop: AtomicBool,
}
pub(super) struct Desktop {
    shared: Arc<Shared>, errors: Receiver<String>, owner: Option<JoinHandle<()>>,
}
impl Desktop {
    pub(super) fn start(view: View, actions: SyncSender<Command>) -> Result<Self> {
        let shared = Arc::new(Shared { view: Mutex::new(view), revision: AtomicU64::new(1), open: AtomicBool::new(true), stop: AtomicBool::new(false) });
        let (errors_tx, errors) = mpsc::sync_channel(1);
        let owner_shared = shared.clone();
        let owner = thread::Builder::new().name("mcw-gtk-desktop".into()).spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| run(owner_shared, actions)));
            let message = match outcome {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(format!("{error:#}")),
                Err(_) => Some("Native GTK window failed unexpectedly; close and retry Open. Tray observation continues.".to_owned()),
            };
            if let Some(message) = message { let _ = errors_tx.try_send(message); }
        }).context("start native GTK owner thread")?;
        Ok(Self { shared, errors, owner: Some(owner) })
    }
    pub(super) fn update(&self, view: View) {
        // This single bounded latest-value slot never builds a snapshot/history queue.
        *self.shared.view.lock().unwrap_or_else(|e| e.into_inner()) = view;
        self.shared.revision.fetch_add(1, Ordering::Release);
    }
    pub(super) fn open(&self) { self.shared.open.store(true, Ordering::Release); }
    pub(super) fn finished(&self) -> bool { self.owner.as_ref().is_none_or(JoinHandle::is_finished) }
    pub(super) fn take_error(&self) -> Option<String> { self.errors.try_recv().ok() }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(owner) = self.owner.take() { let _ = owner.join(); }
    }
}

// Signal data never aliases mutable Ui. GTK owns each Box until widget destruction.
// The render guard stops programmatic widget synchronization from emitting actions.
enum ActionKind { Command(Command), Preference(Command, Command), Profile }
struct Action {
    kind: ActionKind, actions: SyncSender<Command>, rendering: Rc<Cell<bool>>,
    get_toggle: unsafe extern "C" fn(Widget) -> c_int,
    get_combo: unsafe extern "C" fn(Widget) -> c_int,
    congested: Rc<Cell<bool>>,
}
unsafe extern "C" fn action_callback(widget: Widget, data: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let action = unsafe { &*data.cast::<Action>() };
        if action.rendering.get() { return; }
        let command = match action.kind {
            ActionKind::Command(command) => command,
            ActionKind::Preference(on, off) => if unsafe { (action.get_toggle)(widget) } != 0 { on } else { off },
            ActionKind::Profile => match unsafe { (action.get_combo)(widget) } {
                0 => Command::ProfileBalanced, 1 => Command::ProfilePrivate,
                2 => Command::ProfileMeeting, 3 => Command::ProfileDevelopment, _ => return,
            },
        };
        if action.actions.try_send(command).is_err() { action.congested.set(true); }
    }));
}
unsafe extern "C" fn free_action(data: *mut c_void, _: Widget) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe { drop(Box::from_raw(data.cast::<Action>())); }));
}
struct WindowAction {
    hide: unsafe extern "C" fn(Widget),
    keyval: unsafe extern "C" fn(Widget, *mut c_uint) -> c_int,
    state: unsafe extern "C" fn(Widget, *mut c_uint) -> c_int,
}
unsafe extern "C" fn close_callback(widget: Widget, _: Widget, data: *mut c_void) -> c_int {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe { ((&*data.cast::<WindowAction>()).hide)(widget); }));
    1 // Hide this retained window; never terminate the tray.
}
unsafe extern "C" fn key_callback(widget: Widget, event: Widget, data: *mut c_void) -> c_int {
    catch_unwind(AssertUnwindSafe(|| {
        let action = unsafe { &*data.cast::<WindowAction>() };
        let (mut key, mut state) = (0, 0);
        unsafe { (action.keyval)(event, &mut key); (action.state)(event, &mut state); }
        if key == 65307 || (state & 4 != 0 && matches!(key, 87 | 119)) {
            unsafe { (action.hide)(widget); }
            1
        } else { 0 }
    })).unwrap_or(0)
}
unsafe extern "C" fn free_window_action(data: *mut c_void, _: Widget) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe { drop(Box::from_raw(data.cast::<WindowAction>())); }));
}
fn cstring(text: &str) -> CString {
    // Native labels accept arbitrary observed text, never markup or embedded NUL.
    CString::new(text.replace('\0', "�")).expect("NUL replaced")
}
struct TextPane { widget: Widget, scroll: Widget, text: String }
struct Ui {
    api: Api, window: Widget, status: Widget, feedback: Widget,
    cards: [Widget; 2], observed: Widget, access_count: Widget, accesses: TextPane,
    diagnostics: TextPane, profile: Widget, profile_note: Widget,
    preferences: [Widget; 3], buttons: Vec<(&'static str, Widget)>,
    rendering: Rc<Cell<bool>>, congested: Rc<Cell<bool>>,
}
impl Ui {
    fn new(api: Api, view: &View, actions: &SyncSender<Command>) -> Self {
        let rendering = Rc::new(Cell::new(false));
        let congested = Rc::new(Cell::new(false));
        unsafe {
            let window = (api.gtk_window_new)(0);
            (api.gtk_window_set_title)(window, c"MicCamWatch".as_ptr());
            (api.gtk_window_set_icon_name)(window, c"audio-input-microphone".as_ptr());
            (api.gtk_window_set_default_size)(window, 720, 640);
            (api.gtk_widget_set_size_request)(window, 460, 400);
            for (signal, callback) in [
                (c"delete-event", std::mem::transmute::<unsafe extern "C" fn(Widget, Widget, *mut c_void) -> c_int, Callback>(close_callback)),
                (c"key-press-event", std::mem::transmute::<unsafe extern "C" fn(Widget, Widget, *mut c_void) -> c_int, Callback>(key_callback)),
            ] {
                let data = Box::new(WindowAction { hide: api.gtk_widget_hide, keyval: api.gdk_event_get_keyval, state: api.gdk_event_get_state });
                (api.g_signal_connect_data)(window, signal.as_ptr(), Some(callback), Box::into_raw(data).cast(), Some(free_window_action), 0);
            }
            let root = (api.gtk_box_new)(1, 12);
            (api.gtk_container_set_border_width)(root, 16);
            (api.gtk_container_add)(window, root);
            let brand = label(&api, &format!("MCW  ·  MicCamWatch {}", view.version));
            (api.gtk_box_pack_start)(root, brand, 0, 0, 0);
            let status = label(&api, "");
            (api.gtk_box_pack_start)(root, status, 0, 0, 0);
            let feedback = label(&api, "");
            (api.gtk_box_pack_start)(root, feedback, 0, 0, 0);
            let notebook = (api.gtk_notebook_new)();
            (api.gtk_box_pack_start)(root, notebook, 1, 1, 0);
            let overview = page(&api, notebook, text(view.language, "overview"));
            let mut cards = [ptr::null_mut(); 2];
            for (index, resource) in ["microphone", "camera"].iter().enumerate() {
                let frame = (api.gtk_frame_new)(cstring(text(view.language, resource)).as_ptr());
                let card = label(&api, "");
                let padding = (api.gtk_box_new)(1, 8);
                (api.gtk_container_set_border_width)(padding, 12);
                (api.gtk_box_pack_start)(padding, card, 0, 0, 0);
                (api.gtk_container_add)(frame, padding);
                (api.gtk_box_pack_start)(overview, frame, 0, 0, 0);
                cards[index] = card;
            }
            let observed = label(&api, "");
            (api.gtk_box_pack_start)(overview, observed, 0, 0, 0);
            let access_count = label(&api, "");
            (api.gtk_box_pack_start)(overview, access_count, 0, 0, 0);
            let accesses = text_pane(&api, false);
            (api.gtk_widget_set_size_request)(accesses.scroll, -1, 150);
            (api.gtk_box_pack_start)(overview, accesses.scroll, 1, 1, 0);
            let controls_note = label(&api, text(view.language, "explicit"));
            (api.gtk_box_pack_start)(overview, controls_note, 0, 0, 0);
            let mut buttons = Vec::new();
            for name in ["mic", "camera"] {
                let button = (api.gtk_button_new_with_label)(c"".as_ptr());
                connect(&api, button, c"clicked", ActionKind::Command(Command::from_name(name).expect("native action")), actions, &rendering, &congested);
                (api.gtk_box_pack_start)(overview, button, 0, 0, 0);
                buttons.push((name, button));
            }
            let settings = page(&api, notebook, text(view.language, "settings"));
            let profile_label = label(&api, super::words(language(view.language)).profile);
            (api.gtk_box_pack_start)(settings, profile_label, 0, 0, 0);
            let profile = (api.gtk_combo_box_text_new)();
            for choice in super::words(language(view.language)).profiles { (api.gtk_combo_box_text_append_text)(profile, cstring(choice).as_ptr()); }
            accessible(&api, profile, super::words(language(view.language)).profile);
            connect(&api, profile, c"changed", ActionKind::Profile, actions, &rendering, &congested);
            (api.gtk_box_pack_start)(settings, profile, 0, 0, 0);
            let profile_note = label(&api, "");
            (api.gtk_box_pack_start)(settings, profile_note, 0, 0, 0);
            let mut preferences = [ptr::null_mut(); 3];
            for (index, (key, on, off)) in [
                ("notifications", Command::NotificationsEnable, Command::NotificationsDisable),
                ("sound", Command::SoundEnable, Command::SoundDisable),
                ("history", Command::HistoryEnable, Command::HistoryDisable),
            ].into_iter().enumerate() {
                let button = (api.gtk_check_button_new_with_label)(cstring(text(view.language, key)).as_ptr());
                connect(&api, button, c"toggled", ActionKind::Preference(on, off), actions, &rendering, &congested);
                (api.gtk_box_pack_start)(settings, button, 0, 0, 0);
                preferences[index] = button;
            }
            let notification_note = label(&api, text(view.language, "delivery"));
            (api.gtk_box_pack_start)(settings, notification_note, 0, 0, 0);
            for name in ["pause", "autostart"] {
                let button = (api.gtk_button_new_with_label)(c"".as_ptr());
                connect(&api, button, c"clicked", ActionKind::Command(Command::from_name(name).expect("native action")), actions, &rendering, &congested);
                (api.gtk_box_pack_start)(settings, button, 0, 0, 0);
                buttons.push((name, button));
            }
            let diagnostics_page = page(&api, notebook, text(view.language, "diagnostics"));
            let diagnostics_note = label(&api, text(view.language, "copy"));
            (api.gtk_box_pack_start)(diagnostics_page, diagnostics_note, 0, 0, 0);
            let diagnostics = text_pane(&api, true);
            (api.gtk_widget_set_size_request)(diagnostics.scroll, -1, 260);
            (api.gtk_box_pack_start)(diagnostics_page, diagnostics.scroll, 1, 1, 0);
            let about = page(&api, notebook, text(view.language, "about"));
            for line in [format!("MicCamWatch {}", view.version), text(view.language, "description").to_owned(), text(view.language, "privacy").to_owned(), "https://github.com/stcui/miccamwatch".to_owned()] {
                (api.gtk_box_pack_start)(about, label(&api, &line), 0, 0, 0);
            }
            Self { api, window, status, feedback, cards, observed, access_count, accesses, diagnostics, profile, profile_note, preferences, buttons, rendering, congested }
        }
    }
    fn render(&mut self, view: &View) {
        self.rendering.set(true);
        let api = &self.api;
        let lang = view.language;
        unsafe {
            set_label(api, self.status, &view.summary);
            let mut feedback = view.feedback.clone().unwrap_or_default();
            if view.visual == "error" || view.visual == "partial" {
                append_line(&mut feedback, text(lang, "degraded"));
            }
            if view.limited || view.overview.text_limited { append_line(&mut feedback, text(lang, "limited")); }
            if self.congested.replace(false) { append_line(&mut feedback, text(lang, "busy")); }
            set_label(api, self.feedback, &feedback);
            for (index, (observed, effective)) in [
                (view.overview.microphone, view.protection.microphone),
                (view.overview.camera, view.protection.camera),
            ].into_iter().enumerate() {
                let requested = match view.protection.requested[index] {
                    Some(true) => text(lang, "requested_on"), Some(false) => text(lang, "requested_off"), None => text(lang, "not_requested"),
                };
                let card = format!("{}: {}\n{}: {}\n{}: {}", text(lang, "observed"), text(lang, observed), text(lang, "effective"), text(lang, effective), text(lang, "requested"), requested);
                set_label(api, self.cards[index], &card);
            }
            set_label(api, self.observed, &format!("{}: {}", text(lang, "observation_time"), view.overview.observed_at.as_deref().unwrap_or(text(lang, "unknown"))));
            set_label(api, self.access_count, &format!("{}: {}  ·  {}: {}", text(lang, "accesses"), view.overview.total_accesses, text(lang, "omitted"), view.overview.omitted_accesses));
            let mut rows = String::new();
            for access in &view.overview.accesses {
                if !rows.is_empty() { rows.push('\n'); }
                let resource = match access.resource { crate::model::Resource::Microphone => "microphone", crate::model::Resource::Camera => "camera" };
                let activity = match access.activity { crate::model::Activity::Active => "active", crate::model::Activity::Ready => "ready" };
                rows.push_str(&format!("{}  ·  {}  ·  {}", access.application, text(lang, resource), text(lang, activity)));
                if let Some(pid) = access.pid { rows.push_str(&format!("  ·  PID {pid}")); }
            }
            if rows.is_empty() { rows.push_str(text(lang, "empty")); }
            set_text(api, &mut self.accesses, rows);
            set_text(api, &mut self.diagnostics, diagnostic_document(view));
            if let Some(profile) = view.items.iter().find(|entry| entry.action == "profile") {
                let selected = profile.children.as_deref().and_then(|choices| choices.iter().position(|choice| choice.checked == Some(true))).map_or(-1, |index| index as c_int);
                if (api.gtk_combo_box_get_active)(self.profile) != selected { (api.gtk_combo_box_set_active)(self.profile, selected); }
                (api.gtk_widget_set_sensitive)(self.profile, profile.enabled.into());
                set_label(api, self.profile_note, if profile.enabled { text(lang, "profile_note") } else { &profile.label });
            }
            for (widget, active) in self.preferences.iter().zip([view.preferences.notifications_enabled, view.preferences.sound_enabled, view.preferences.history_enabled]) {
                if ((api.gtk_toggle_button_get_active)(*widget) != 0) != active { (api.gtk_toggle_button_set_active)(*widget, active.into()); }
            }
            for (name, button) in &self.buttons {
                if let Some(entry) = view.items.iter().find(|entry| entry.action == *name) {
                    (api.gtk_button_set_label)(*button, cstring(&entry.label).as_ptr());
                    (api.gtk_widget_set_sensitive)(*button, entry.enabled.into());
                    if let Some(detail) = &entry.detail { (api.gtk_widget_set_tooltip_text)(*button, cstring(detail).as_ptr()); }
                } else { (api.gtk_widget_set_sensitive)(*button, 0); }
            }
        }
        self.rendering.set(false);
    }
}
impl Drop for Ui {
    fn drop(&mut self) {
        // Destroy synchronously on the GTK owner, releasing signal userdata before
        // Rc guards and function table disappear. Do not run callbacks afterward.
        unsafe { (self.api.gtk_widget_destroy)(self.window); }
    }
}
fn append_line(value: &mut String, line: &str) { if !value.is_empty() { value.push('\n'); } value.push_str(line); }
unsafe fn accessible(api: &Api, widget: Widget, name: &str) {
    unsafe { (api.atk_object_set_name)((api.gtk_widget_get_accessible)(widget), cstring(name).as_ptr()); }
}
unsafe fn label(api: &Api, value: &str) -> Widget {
    unsafe {
        let widget = (api.gtk_label_new)(cstring(value).as_ptr());
        (api.gtk_label_set_xalign)(widget, 0.0);
        (api.gtk_label_set_line_wrap)(widget, 1);
        (api.gtk_label_set_line_wrap_mode)(widget, 2); // PANGO_WRAP_WORD_CHAR
        (api.gtk_label_set_max_width_chars)(widget, 90);
        (api.gtk_label_set_selectable)(widget, 1);
        widget
    }
}
unsafe fn set_label(api: &Api, widget: Widget, value: &str) {
    unsafe {
        let value = cstring(value);
        if CStr::from_ptr((api.gtk_label_get_text)(widget)) != value.as_c_str() { (api.gtk_label_set_text)(widget, value.as_ptr()); }
    }
}
unsafe fn page(api: &Api, notebook: Widget, title: &str) -> Widget {
    unsafe {
        let scroll = (api.gtk_scrolled_window_new)(ptr::null_mut(), ptr::null_mut());
        (api.gtk_scrolled_window_set_policy)(scroll, 2, 1); // no horizontal, automatic vertical
        let content = (api.gtk_box_new)(1, 12);
        (api.gtk_container_set_border_width)(content, 16);
        (api.gtk_container_add)(scroll, content);
        (api.gtk_notebook_append_page)(notebook, scroll, label(api, title));
        content
    }
}
unsafe fn text_pane(api: &Api, monospace: bool) -> TextPane {
    unsafe {
        let widget = (api.gtk_text_view_new)();
        (api.gtk_text_view_set_editable)(widget, 0);
        (api.gtk_text_view_set_cursor_visible)(widget, 1);
        (api.gtk_text_view_set_wrap_mode)(widget, 3); // GTK_WRAP_WORD_CHAR
        (api.gtk_text_view_set_monospace)(widget, monospace.into());
        let scroll = (api.gtk_scrolled_window_new)(ptr::null_mut(), ptr::null_mut());
        (api.gtk_scrolled_window_set_policy)(scroll, 1, 1);
        (api.gtk_container_add)(scroll, widget);
        TextPane { widget, scroll, text: String::new() }
    }
}
unsafe fn set_text(api: &Api, pane: &mut TextPane, value: String) {
    if pane.text == value { return; }
    unsafe {
        let buffer = (api.gtk_text_view_get_buffer)(pane.widget);
        let (mut insert, mut bound) = (TextIter::default(), TextIter::default());
        (api.gtk_text_buffer_get_iter_at_mark)(buffer, &mut insert, (api.gtk_text_buffer_get_insert)(buffer));
        (api.gtk_text_buffer_get_iter_at_mark)(buffer, &mut bound, (api.gtk_text_buffer_get_selection_bound)(buffer));
        let offsets = ((api.gtk_text_iter_get_offset)(&insert), (api.gtk_text_iter_get_offset)(&bound));
        let vertical = (api.gtk_scrolled_window_get_vadjustment)(pane.scroll);
        let horizontal = (api.gtk_scrolled_window_get_hadjustment)(pane.scroll);
        let scroll = ((api.gtk_adjustment_get_value)(vertical), (api.gtk_adjustment_get_value)(horizontal));
        let native = cstring(&value);
        (api.gtk_text_buffer_set_text)(buffer, native.as_ptr(), -1);
        let count = value.chars().count().min(c_int::MAX as usize) as c_int;
        (api.gtk_text_buffer_get_iter_at_offset)(buffer, &mut insert, offsets.0.min(count));
        (api.gtk_text_buffer_get_iter_at_offset)(buffer, &mut bound, offsets.1.min(count));
        (api.gtk_text_buffer_select_range)(buffer, &insert, &bound);
        (api.gtk_adjustment_set_value)(vertical, scroll.0);
        (api.gtk_adjustment_set_value)(horizontal, scroll.1);
        pane.text = value;
    }
}
unsafe fn connect(api: &Api, widget: Widget, signal: &CStr, kind: ActionKind, actions: &SyncSender<Command>, rendering: &Rc<Cell<bool>>, congested: &Rc<Cell<bool>>) {
    let data = Box::new(Action { kind, actions: actions.clone(), rendering: rendering.clone(), congested: congested.clone(), get_toggle: api.gtk_toggle_button_get_active, get_combo: api.gtk_combo_box_get_active });
    unsafe {
        let callback = std::mem::transmute::<unsafe extern "C" fn(Widget, *mut c_void), Callback>(action_callback);
        (api.g_signal_connect_data)(widget, signal.as_ptr(), Some(callback), Box::into_raw(data).cast(), Some(free_action), 0);
    }
}
fn run(shared: Arc<Shared>, actions: SyncSender<Command>) -> Result<()> {
    let api = Api::load()?; // Resolve the entire ABI before initializing or constructing widgets.
    ensure!(unsafe { (api.gtk_init_check)(ptr::null_mut(), ptr::null_mut()) } != 0,
        "Native window unavailable: GTK cannot connect to a display. Run mcw tray open as your desktop user inside the graphical Wayland/X11 session (DISPLAY or WAYLAND_DISPLAY required), not through a headless/root helper. Tray and CLI remain available.");
    let initial = shared.view.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut ui = Ui::new(api, &initial, &actions);
    let mut revision = 0;
    while !shared.stop.load(Ordering::Acquire) {
        let opening = shared.open.swap(false, Ordering::AcqRel);
        let visible = opening || unsafe { (ui.api.gtk_widget_get_visible)(ui.window) } != 0;
        let current = shared.revision.load(Ordering::Acquire);
        if visible && (revision != current || opening || ui.congested.get()) {
            let view = shared.view.lock().unwrap_or_else(|e| e.into_inner()).clone();
            ui.render(&view);
            revision = current;
        }
        if opening { unsafe { (ui.api.gtk_widget_show_all)(ui.window); (ui.api.gtk_window_present)(ui.window); } }
        // Both iteration count and elapsed time are bounded. Never wait for an
        // event: shutdown/open/latest-state checks cannot be starved by GLib.
        let started = Instant::now();
        for _ in 0..32 {
            if unsafe { (ui.api.g_main_context_iteration)(ptr::null_mut(), 0) } == 0 || started.elapsed() >= Duration::from_millis(4) { break; }
        }
        thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}
fn language(code: &str) -> crate::i18n::Language {
    use crate::i18n::Language;
    match code { "fr" => Language::Fr, "de" => Language::De, "es" => Language::Es, "ja" => Language::Ja, "zh" => Language::Zh, "ru" => Language::Ru, _ => Language::En }
}
fn text<'a>(code: &str, key: &'a str) -> &'a str {
    let index = match code { "fr" => 1, "de" => 2, "es" => 3, "ja" => 4, "zh" => 5, "ru" => 6, _ => 0 };
    let values = match key {
        "overview" => ["Overview", "Vue d’ensemble", "Übersicht", "Resumen", "概要", "概览", "Обзор"],
        "settings" => ["Settings", "Réglages", "Einstellungen", "Ajustes", "設定", "设置", "Настройки"],
        "diagnostics" => ["Diagnostics", "Diagnostic", "Diagnose", "Diagnóstico", "診断", "诊断", "Диагностика"],
        "about" => ["About", "À propos", "Über", "Acerca de", "このアプリについて", "关于", "О программе"],
        "microphone" => ["Microphone", "Microphone", "Mikrofon", "Micrófono", "マイク", "麦克风", "Микрофон"],
        "camera" => ["Camera", "Caméra", "Kamera", "Cámara", "カメラ", "摄像头", "Камера"],
        "observed" => ["Observed activity", "Activité observée", "Beobachtete Aktivität", "Actividad observada", "観測された動作", "观测活动", "Наблюдаемая активность"],
        "effective" => ["Effective control", "Contrôle effectif", "Wirksame Steuerung", "Control efectivo", "実際の制御状態", "实际控制", "Фактическое управление"],
        "requested" => ["Last manual request", "Dernière demande manuelle", "Letzte manuelle Anforderung", "Última solicitud manual", "最後の手動要求", "上次手动请求", "Последний ручной запрос"],
        "requested_on" => ["Mute / restrict", "Couper / restreindre", "Stummschalten / einschränken", "Silenciar / restringir", "ミュート・制限", "静音／限制", "Отключить / ограничить"],
        "requested_off" => ["Restore", "Restaurer", "Wiederherstellen", "Restaurar", "復元", "恢复", "Восстановить"],
        "not_requested" => ["No request this session", "Aucune demande dans cette session", "Keine Anforderung in dieser Sitzung", "Sin solicitud en esta sesión", "このセッションでは要求なし", "本会话无请求", "В этом сеансе запросов не было"],
        "observation_time" => ["Latest successful observation (UTC)", "Dernière observation réussie (UTC)", "Letzte erfolgreiche Beobachtung (UTC)", "Última observación correcta (UTC)", "最終観測成功時刻 (UTC)", "最近成功观测 (UTC)", "Последнее успешное наблюдение (UTC)"],
        "active" => ["Active", "Actif", "Aktiv", "Activo", "使用中", "正在使用", "Активно"],
        "ready" => ["Ready / open", "Prêt / ouvert", "Bereit / geöffnet", "Preparado / abierto", "待機・オープン", "就绪／已打开", "Готово / открыто"],
        "idle" => ["No current access observed", "Aucun accès actuel observé", "Kein aktueller Zugriff beobachtet", "Sin acceso actual observado", "現在のアクセスは観測されていません", "未观测到当前访问", "Текущих обращений не обнаружено"],
        "unknown" => ["Unknown / unavailable", "Inconnu / indisponible", "Unbekannt / nicht verfügbar", "Desconocido / no disponible", "不明・利用不可", "未知／不可用", "Неизвестно / недоступно"],
        "muted" => ["Muted", "Coupé", "Stummgeschaltet", "Silenciado", "ミュート", "已静音", "Звук отключён"],
        "unmuted" => ["Not muted", "Non coupé", "Nicht stummgeschaltet", "No silenciado", "ミュートなし", "未静音", "Звук не отключён"],
        "mixed" => ["Mixed input states", "États des entrées mixtes", "Gemischte Eingangszustände", "Estados de entrada mixtos", "入力状態が混在", "输入状态混合", "Смешанные состояния входов"],
        "owned" => ["MCW-owned camera restriction present", "Restriction caméra appartenant à MCW présente", "MCW-eigene Kamerasperre vorhanden", "Restricción de cámara de MCW presente", "MCWによるカメラ制限あり", "存在 MCW 摄像头限制", "Есть ограничение камеры MCW"],
        "absent" => ["No MCW-owned camera block; not a permission guarantee", "Aucun blocage caméra MCW ; aucune garantie d’autorisation", "Keine MCW-Kamerasperre; keine Berechtigungsgarantie", "Sin bloqueo MCW; no garantiza permisos", "MCWのブロックなし（権限の保証ではありません）", "无 MCW 摄像头阻止；不保证权限", "Нет блокировки MCW; это не гарантия разрешений"],
        "system_managed" => ["System-managed restriction", "Restriction gérée par le système", "Systemverwaltete Einschränkung", "Restricción del sistema", "システム管理の制限", "系统管理限制", "Системное ограничение"],
        "accesses" => ["Current known accesses", "Accès actuels connus", "Aktuelle bekannte Zugriffe", "Accesos actuales conocidos", "現在判明しているアクセス", "当前已知访问", "Известные текущие обращения"],
        "omitted" => ["Rows omitted", "Lignes omises", "Ausgelassene Zeilen", "Filas omitidas", "省略された行", "省略行数", "Пропущено строк"],
        "empty" => ["No current application rows. Unknown or incomplete observation is not proof of no access.", "Aucune application actuelle. Une observation inconnue ou incomplète ne prouve pas l’absence d’accès.", "Keine aktuellen Anwendungen. Unbekannte oder unvollständige Beobachtung beweist keine Zugriffsfreiheit.", "Sin aplicaciones actuales. Una observación desconocida o incompleta no demuestra ausencia de acceso.", "現在のアプリ行はありません。不明・不完全な観測はアクセスがない証拠ではありません。", "无当前应用行。未知或不完整的观测不能证明没有访问。", "Нет строк приложений. Неизвестное или неполное наблюдение не доказывает отсутствие доступа."],
        "explicit" => ["Explicit controls only: microphone applies to this PipeWire session; supported USB camera blocks require administrator approval. Opening or refreshing this window never changes devices.", "Contrôles explicites : microphone limité à cette session PipeWire ; blocage des caméras USB compatibles avec autorisation administrateur. Ouvrir ou actualiser ne modifie aucun appareil.", "Explizite Steuerung: Mikrofon nur in dieser PipeWire-Sitzung; unterstützte USB-Kamerasperren benötigen Administratorfreigabe. Öffnen oder Aktualisieren ändert keine Geräte.", "Controles explícitos: micrófono solo en esta sesión PipeWire; bloqueo de cámaras USB compatibles con aprobación administrativa. Abrir o actualizar no cambia dispositivos.", "明示的な操作のみ：マイクはこのPipeWireセッション、対応USBカメラのブロックは管理者承認が必要です。表示・更新ではデバイスを変更しません。", "仅显式控制：麦克风限于当前 PipeWire 会话；支持的 USB 摄像头阻止需要管理员批准。打开或刷新不会更改设备。", "Только явное управление: микрофон в этом сеансе PipeWire; блокировка поддерживаемых USB-камер требует администратора. Открытие и обновление не меняют устройства."],
        "notifications" => ["Notifications", "Notifications", "Benachrichtigungen", "Notificaciones", "通知", "通知", "Уведомления"],
        "sound" => ["Notification sound", "Son des notifications", "Benachrichtigungston", "Sonido de notificación", "通知音", "通知声音", "Звук уведомлений"],
        "history" => ["Local event history", "Historique local", "Lokaler Ereignisverlauf", "Historial local", "ローカルイベント履歴", "本地事件历史", "Локальная история событий"],
        "delivery" => ["Notification delivery depends on your desktop notification service. Pause suppresses alerts; history is stored locally only when enabled.", "La livraison dépend du service de notifications du bureau. La pause suspend les alertes ; l’historique reste local et optionnel.", "Zustellung benötigt den Benachrichtigungsdienst des Desktops. Pause unterdrückt Hinweise; Verlauf wird nur aktiviert lokal gespeichert.", "La entrega depende del servicio de notificaciones del escritorio. La pausa suprime alertas; el historial solo se guarda localmente si está habilitado.", "通知はデスクトップ通知サービスに依存します。一時停止は通知を抑制し、履歴は有効時のみローカル保存します。", "通知依赖桌面通知服务。暂停会抑制提醒；仅启用时才在本地存储历史。", "Доставка зависит от службы уведомлений рабочего стола. Пауза отключает оповещения; история хранится локально только при включении."],
        "profile_note" => ["A profile selects policy, not a mute or camera block. Balanced and Meeting currently share a policy.", "Un profil choisit la politique, sans couper ni bloquer. Équilibré et Réunion partagent actuellement la même politique.", "Profile wählen Richtlinien, keine Stumm- oder Kamerasperre. Ausgewogen und Besprechung nutzen derzeit dieselbe Richtlinie.", "El perfil selecciona política, no silencia ni bloquea. Equilibrado y Reunión comparten actualmente política.", "プロファイルはポリシーを選択します。ミュートやカメラブロックではありません。バランスと会議は同じポリシーです。", "配置选择策略，不会静音或阻止摄像头。平衡与会议目前使用相同策略。", "Профиль выбирает политику, а не отключает устройства. Сбалансированный и Встреча пока используют одну политику."],
        "copy" => ["Read-only diagnostics. Select text; Ctrl+A / Ctrl+C copy. Ctrl+W or Escape hides the window without stopping the tray.", "Diagnostic en lecture seule. Sélectionner ; Ctrl+A / Ctrl+C pour copier. Ctrl+W ou Échap masque sans arrêter.", "Schreibgeschützte Diagnose. Text auswählen; Strg+A / Strg+C kopiert. Strg+W oder Escape blendet das Fenster aus.", "Diagnóstico de solo lectura. Seleccione; Ctrl+A / Ctrl+C copia. Ctrl+W o Escape oculta sin detener la bandeja.", "読み取り専用の診断。Ctrl+A / Ctrl+Cでコピー。Ctrl+WまたはEscapeでトレイを止めず非表示にします。", "只读诊断。选择文本；Ctrl+A / Ctrl+C 复制。Ctrl+W 或 Escape 隐藏窗口但不停止托盘。", "Диагностика только для чтения. Ctrl+A / Ctrl+C — копировать. Ctrl+W или Escape скрывает окно, не останавливая трей."],
        "description" => ["Native microphone and camera observation with explicit, platform-limited privacy controls.", "Observation native du microphone et de la caméra avec contrôles explicites limités par la plateforme.", "Native Mikrofon- und Kamerabeobachtung mit expliziten, plattformbegrenzten Datenschutzfunktionen.", "Observación nativa del micrófono y cámara con controles explícitos limitados por la plataforma.", "マイク・カメラをネイティブに観測し、プラットフォームの範囲内で明示的に制御します。", "原生麦克风与摄像头观测，提供受平台限制的显式隐私控制。", "Нативное наблюдение за микрофоном и камерой с явным управлением в пределах возможностей платформы."],
        "privacy" => ["No automatic uploads or telemetry. This window shows the latest snapshot, not an additional history buffer.", "Aucun envoi automatique ni télémétrie. Dernier instantané uniquement, sans historique supplémentaire.", "Keine automatischen Uploads oder Telemetrie. Nur der neueste Zustand, kein zusätzlicher Verlaufspuffer.", "Sin cargas automáticas ni telemetría. Solo la última instantánea, sin búfer de historial adicional.", "自動アップロード・テレメトリなし。最新のスナップショットのみで、追加履歴は保持しません。", "无自动上传或遥测。本窗口仅显示最新快照，不添加历史缓冲区。", "Без автоматической отправки и телеметрии. Только последний снимок, без дополнительного буфера истории."],
        "degraded" => ["Observation or an action is incomplete. See Diagnostics; unknown is not safe or idle.", "Observation ou action incomplète. Voir Diagnostic ; inconnu ne signifie pas inactif ni protégé.", "Beobachtung oder Aktion unvollständig. Siehe Diagnose; unbekannt bedeutet nicht sicher oder inaktiv.", "Observación o acción incompleta. Consulte Diagnóstico; desconocido no significa seguro ni inactivo.", "観測または操作が不完全です。診断を確認してください。不明は安全・未使用を意味しません。", "观测或操作不完整。请查看诊断；未知不代表安全或空闲。", "Наблюдение или действие неполно. См. диагностику; неизвестно не значит безопасно или неактивно."],
        "limited" => ["Some text or current access rows are omitted; this view is incomplete.", "Du texte ou des accès actuels sont omis ; vue incomplète.", "Text oder aktuelle Zugriffe ausgelassen; Ansicht unvollständig.", "Se omiten textos o accesos actuales; vista incompleta.", "一部のテキストまたは現在のアクセス行を省略しています。", "部分文本或当前访问行被省略；此视图不完整。", "Часть текста или текущих обращений пропущена; вид неполон."],
        "busy" => ["The engine is busy; the action was not queued. Wait for current work, then retry.", "Moteur occupé ; action non envoyée. Attendre puis réessayer.", "Engine beschäftigt; Aktion nicht eingereiht. Warten und erneut versuchen.", "Motor ocupado; acción no enviada. Espere y reintente.", "エンジンは処理中です。操作は送信されませんでした。完了後に再試行してください。", "引擎忙；操作未排队。请等待当前工作完成后重试。", "Движок занят; действие не поставлено в очередь. Дождитесь завершения и повторите."],
        _ => return key,
    };
    values[index]
}
