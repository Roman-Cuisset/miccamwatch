//! Lazy GTK 3 C-ABI desktop. Only the dedicated owner thread touches GTK.
//! Successful loader handles remain process-resident: GTK registers process-global
//! GTypes whose function pointers must not be invalidated by dlclose.
use super::{Command, View, diagnostic_document};
use anyhow::{Context, Result, ensure};
use std::{
    cell::Cell,
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

type Widget = *mut c_void;
type Callback = unsafe extern "C" fn();
// Public GtkTextIter ABI from GTK 3 gtktextiter.h (not an opaque guessed byte array).
#[repr(C)]
#[derive(Default)]
struct TextIter {
    dummy1: *mut c_void,
    dummy2: *mut c_void,
    dummy3: c_int,
    dummy4: c_int,
    dummy5: c_int,
    dummy6: c_int,
    dummy7: c_int,
    dummy8: c_int,
    dummy9: *mut c_void,
    dummy10: *mut c_void,
    dummy11: c_int,
    dummy12: c_int,
    dummy13: c_int,
    dummy14: *mut c_void,
}

macro_rules! gtk_api {
    ($($(#[$meta:meta])* $name:ident: $ty:ty),+ $(,)?) => {
        #[derive(Clone, Copy)]
        struct Api { $($(#[$meta])* $name: $ty,)+ }
        impl Api {
            fn load() -> Result<Self> {
                // RTLD_LOCAL prevents exporting GTK ABI into unrelated plugin code.
                let handle = unsafe { libc::dlopen(c"libgtk-3.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
                ensure!(!handle.is_null(), "Native window unavailable: install the GTK 3 runtime (Ubuntu/Debian: libgtk-3-0; Fedora: gtk3) and run mcw tray open in your graphical desktop session. CLI and tray remain available.");
                let loaded = (|| -> Result<Self> {
                    Ok(Self { $($(#[$meta])* $name: {
                        let symbol = unsafe { libc::dlsym(handle, concat!(stringify!($name), "\0").as_ptr().cast()) };
                        ensure!(!symbol.is_null(), "GTK 3 runtime lacks required ABI symbol {}; install a complete GTK 3 runtime", stringify!($name));
                        // Every signature below is the stable GTK3/GLib/GObject C ABI.
                        unsafe { std::mem::transmute::<*mut c_void, $ty>(symbol) }
                    },)+ })
                })();
                // Even a partial load may run constructors registering GTypes.
                // Never dlclose GTK or its transitive libraries in this process.
                loaded
            }
        }
    };
}
gtk_api! {
    g_set_prgname: unsafe extern "C" fn(*const c_char),
    g_set_application_name: unsafe extern "C" fn(*const c_char),
    gdk_set_program_class: unsafe extern "C" fn(*const c_char),
    gtk_init_check: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int,
    gtk_window_new: unsafe extern "C" fn(c_int) -> Widget,
    gtk_window_set_title: unsafe extern "C" fn(Widget, *const c_char),
    gtk_window_set_default_size: unsafe extern "C" fn(Widget, c_int, c_int),
    gtk_window_set_icon: unsafe extern "C" fn(Widget, Widget),
    #[cfg(test)]
    gtk_window_get_focus: unsafe extern "C" fn(Widget) -> Widget,
    #[cfg(test)]
    gtk_widget_grab_focus: unsafe extern "C" fn(Widget),
    gtk_widget_get_screen: unsafe extern "C" fn(Widget) -> Widget,
    gtk_widget_get_style_context: unsafe extern "C" fn(Widget) -> Widget,
    gtk_style_context_add_class: unsafe extern "C" fn(Widget, *const c_char),
    gtk_style_context_add_provider_for_screen: unsafe extern "C" fn(Widget, Widget, c_uint),
    gtk_style_context_remove_provider_for_screen: unsafe extern "C" fn(Widget, Widget),
    gtk_css_provider_new: unsafe extern "C" fn() -> Widget,
    gtk_css_provider_load_from_data: unsafe extern "C" fn(Widget, *const c_char, isize, *mut Widget) -> c_int,
    g_object_unref: unsafe extern "C" fn(Widget),
    gdk_pixbuf_loader_new: unsafe extern "C" fn() -> Widget,
    gdk_pixbuf_loader_write: unsafe extern "C" fn(Widget, *const u8, usize, *mut Widget) -> c_int,
    gdk_pixbuf_loader_close: unsafe extern "C" fn(Widget, *mut Widget) -> c_int,
    gdk_pixbuf_loader_get_pixbuf: unsafe extern "C" fn(Widget) -> Widget,
    gdk_pixbuf_scale_simple: unsafe extern "C" fn(Widget, c_int, c_int, c_int) -> Widget,
    gtk_image_new_from_pixbuf: unsafe extern "C" fn(Widget) -> Widget,
    gtk_window_present: unsafe extern "C" fn(Widget),
    gtk_widget_set_size_request: unsafe extern "C" fn(Widget, c_int, c_int),
    gtk_widget_show_all: unsafe extern "C" fn(Widget),
    gtk_widget_hide: unsafe extern "C" fn(Widget),
    gtk_widget_destroy: unsafe extern "C" fn(Widget),
    gtk_widget_get_visible: unsafe extern "C" fn(Widget) -> c_int,
    #[cfg(test)]
    gtk_widget_get_tooltip_text: unsafe extern "C" fn(Widget) -> *mut c_char,
    gtk_widget_set_sensitive: unsafe extern "C" fn(Widget, c_int),
    gtk_widget_set_tooltip_text: unsafe extern "C" fn(Widget, *const c_char),
    gtk_widget_is_sensitive: unsafe extern "C" fn(Widget) -> c_int,
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
    gtk_label_set_lines: unsafe extern "C" fn(Widget, c_int),
    gtk_label_set_ellipsize: unsafe extern "C" fn(Widget, c_int),
    gtk_frame_new: unsafe extern "C" fn(*const c_char) -> Widget,
    gtk_notebook_new: unsafe extern "C" fn() -> Widget,
    gtk_notebook_append_page: unsafe extern "C" fn(Widget, Widget, Widget) -> c_int,
    gtk_notebook_set_scrollable: unsafe extern "C" fn(Widget, c_int),
    gtk_notebook_get_current_page: unsafe extern "C" fn(Widget) -> c_int,
    gtk_notebook_set_current_page: unsafe extern "C" fn(Widget, c_int),
    gtk_scrolled_window_new: unsafe extern "C" fn(Widget, Widget) -> Widget,
    gtk_scrolled_window_set_policy: unsafe extern "C" fn(Widget, c_int, c_int),
    gtk_scrolled_window_get_vadjustment: unsafe extern "C" fn(Widget) -> Widget,
    gtk_scrolled_window_get_hadjustment: unsafe extern "C" fn(Widget) -> Widget,
    gtk_adjustment_get_value: unsafe extern "C" fn(Widget) -> f64,
    gtk_adjustment_set_value: unsafe extern "C" fn(Widget, f64),
    gtk_button_new_with_label: unsafe extern "C" fn(*const c_char) -> Widget,
    gtk_button_set_label: unsafe extern "C" fn(Widget, *const c_char),
    gtk_bin_get_child: unsafe extern "C" fn(Widget) -> Widget,
    #[cfg(test)]
    gtk_button_clicked: unsafe extern "C" fn(Widget),
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
    #[cfg(test)]
    gtk_window_close: unsafe extern "C" fn(Widget),
    #[cfg(test)]
    gtk_text_buffer_get_bounds: unsafe extern "C" fn(Widget, *mut TextIter, *mut TextIter),
    #[cfg(test)]
    gtk_text_buffer_get_text: unsafe extern "C" fn(Widget, *const TextIter, *const TextIter, c_int) -> *mut c_char,
    #[cfg(test)]
    g_free: unsafe extern "C" fn(Widget),
}

struct Shared {
    view: Mutex<View>,
    revision: AtomicU64,
    open: AtomicBool,
    stop: AtomicBool,
    finished: AtomicBool,
    completed: Condvar,
    #[cfg(test)]
    probe: Mutex<Option<tests::ProbeRequest>>,
}
struct Session {
    shared: Arc<Shared>,
    actions: SyncSender<Command>,
    errors: SyncSender<String>,
}
// GTK may initialize process-global state even when display initialization fails.
// Keep one owning OS thread for every open/retry/teardown for the process lifetime.
static OWNER: Mutex<Option<SyncSender<Session>>> = Mutex::new(None);
pub(super) struct Desktop {
    shared: Arc<Shared>,
    errors: Receiver<String>,
}
impl Desktop {
    pub(super) fn start(view: View, actions: SyncSender<Command>) -> Result<Self> {
        let shared = Arc::new(Shared {
            view: Mutex::new(view),
            revision: AtomicU64::new(1),
            open: AtomicBool::new(true),
            stop: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            completed: Condvar::new(),
            #[cfg(test)]
            probe: Mutex::new(None),
        });
        let (errors_tx, errors) = mpsc::sync_channel(1);
        let mut owner = OWNER.lock().unwrap_or_else(|e| e.into_inner());
        if owner.is_none() {
            let (sender, sessions) = mpsc::sync_channel::<Session>(1);
            thread::Builder::new().name("mcw-gtk-desktop".into()).spawn(move || {
                let mut api = None;
                while let Ok(session) = sessions.recv() {
                    let outcome = catch_unwind(AssertUnwindSafe(|| {
                        if session.shared.stop.load(Ordering::Acquire) { return Ok(()); }
                        // Resolve once, lazily, on the GTK owner. A failed loader
                        // has not initialized GTK and may be retried after install.
                        if api.is_none() { api = Some(Api::load()?); }
                        run(api.expect("loaded GTK ABI"), session.shared.clone(), session.actions)
                    }));
                    let message = match outcome {
                        Ok(Ok(())) => None,
                        Ok(Err(error)) => Some(format!("{error:#}")),
                        Err(_) => Some("Native GTK window failed unexpectedly; retry Open. Tray observation continues.".to_owned()),
                    };
                    if let Some(message) = message { let _ = session.errors.try_send(message); }
                    let _view = session.shared.view.lock().unwrap_or_else(|e| e.into_inner());
                    session.shared.finished.store(true, Ordering::Release);
                    session.shared.completed.notify_all();
                }
            }).context("start native GTK owner thread")?;
            *owner = Some(sender);
        }
        owner
            .as_ref()
            .expect("GTK owner")
            .try_send(Session {
                shared: shared.clone(),
                actions,
                errors: errors_tx,
            })
            .context("native GTK owner is unavailable or finishing a previous window")?;
        Ok(Self { shared, errors })
    }
    pub(super) fn update(&self, view: View) {
        // This single bounded latest-value slot never builds a snapshot/history queue.
        *self.shared.view.lock().unwrap_or_else(|e| e.into_inner()) = view;
        self.shared.revision.fetch_add(1, Ordering::Release);
    }
    pub(super) fn open(&self) {
        self.shared.open.store(true, Ordering::Release);
    }
    pub(super) fn finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }
    pub(super) fn take_error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        // Wait until widget destruction and signal-data finalization finish on
        // the persistent GTK owner before the tray or a replacement can proceed.
        let mut view = self.shared.view.lock().unwrap_or_else(|e| e.into_inner());
        while !self.finished() {
            view = self
                .shared
                .completed
                .wait(view)
                .unwrap_or_else(|e| e.into_inner());
        }
    }
}

// Signal data never aliases mutable Ui. GTK owns each Box until widget destruction.
// The render guard stops programmatic widget synchronization from emitting actions.
enum ActionKind {
    Command(Command),
    Preference(Command, Command),
    Profile,
}
struct Action {
    kind: ActionKind,
    actions: SyncSender<Command>,
    rendering: Rc<Cell<bool>>,
    get_toggle: unsafe extern "C" fn(Widget) -> c_int,
    get_combo: unsafe extern "C" fn(Widget) -> c_int,
    get_sensitive: unsafe extern "C" fn(Widget) -> c_int,
    congested: Rc<Cell<bool>>,
}
unsafe extern "C" fn action_callback(widget: Widget, data: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let action = unsafe { &*data.cast::<Action>() };
        if action.rendering.get() || unsafe { (action.get_sensitive)(widget) } == 0 {
            return;
        }
        let command = match action.kind {
            ActionKind::Command(command) => command,
            ActionKind::Preference(on, off) => {
                if unsafe { (action.get_toggle)(widget) } != 0 {
                    on
                } else {
                    off
                }
            }
            ActionKind::Profile => match unsafe { (action.get_combo)(widget) } {
                0 => Command::ProfileBalanced,
                1 => Command::ProfilePrivate,
                2 => Command::ProfileMeeting,
                3 => Command::ProfileDevelopment,
                _ => return,
            },
        };
        if action.actions.try_send(command).is_err() {
            action.congested.set(true);
        }
    }));
}
unsafe extern "C" fn free_action(data: *mut c_void, _: Widget) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(data.cast::<Action>()));
    }));
}
struct WindowAction {
    hide: unsafe extern "C" fn(Widget),
    keyval: unsafe extern "C" fn(Widget, *mut c_uint) -> c_int,
    state: unsafe extern "C" fn(Widget, *mut c_uint) -> c_int,
}
unsafe extern "C" fn close_callback(widget: Widget, _: Widget, data: *mut c_void) -> c_int {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        ((&*data.cast::<WindowAction>()).hide)(widget);
    }));
    1 // Hide this retained window; never terminate the tray.
}
unsafe extern "C" fn key_callback(widget: Widget, event: Widget, data: *mut c_void) -> c_int {
    catch_unwind(AssertUnwindSafe(|| {
        let action = unsafe { &*data.cast::<WindowAction>() };
        let (mut key, mut state) = (0, 0);
        unsafe {
            (action.keyval)(event, &mut key);
            (action.state)(event, &mut state);
        }
        if key == 65307 || (state & 4 != 0 && matches!(key, 87 | 119)) {
            unsafe {
                (action.hide)(widget);
            }
            1
        } else {
            0
        }
    }))
    .unwrap_or(0)
}
unsafe extern "C" fn free_window_action(data: *mut c_void, _: Widget) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(data.cast::<WindowAction>()));
    }));
}
fn cstring(text: &str) -> CString {
    // Native labels accept arbitrary observed text, never markup or embedded NUL.
    CString::new(text.replace('\0', "�")).expect("NUL replaced")
}
struct TextPane {
    widget: Widget,
    scroll: Widget,
    text: String,
    pending_scroll: Option<(f64, f64)>,
}
struct Card {
    activity: Widget,
    protection: Widget,
}
// Own the top-level from the first allocation, including partial construction.
// A caught Rust panic must not leave a visible orphan or live signal userdata.
struct NativeWindow {
    api: Api,
    widget: Widget,
    style_provider: Widget,
}
impl NativeWindow {
    fn destroy(&mut self) {
        if self.widget.is_null() {
            return;
        }
        unsafe {
            if !self.style_provider.is_null() {
                (self.api.gtk_style_context_remove_provider_for_screen)(
                    (self.api.gtk_widget_get_screen)(self.widget),
                    self.style_provider,
                );
                (self.api.g_object_unref)(self.style_provider);
            }
            (self.api.gtk_widget_destroy)(self.widget);
        }
        self.widget = ptr::null_mut();
    }
}
impl Drop for NativeWindow {
    fn drop(&mut self) {
        self.destroy();
    }
}
struct Ui {
    api: Api,
    window: Widget,
    notebook: Widget,
    status: Widget,
    feedback: Widget,
    cards: [Card; 2],
    observed: Widget,
    access_count: Widget,
    accesses: TextPane,
    diagnostics: TextPane,
    profile: Widget,
    profile_note: Widget,
    preferences: [Widget; 3],
    buttons: Vec<(&'static str, Widget)>,
    rendering: Rc<Cell<bool>>,
    congested: Rc<Cell<bool>>,
    owner: NativeWindow,
}
impl Ui {
    fn new(api: Api, view: &View, actions: &SyncSender<Command>) -> Self {
        let rendering = Rc::new(Cell::new(false));
        let congested = Rc::new(Cell::new(false));
        unsafe {
            let window = (api.gtk_window_new)(0);
            let mut owner = NativeWindow {
                api,
                widget: window,
                style_provider: ptr::null_mut(),
            };
            (api.gtk_window_set_title)(window, c"MicCamWatch".as_ptr());
            owner.style_provider = install_style(&api, window);
            let brand_image = brand_image(&api, window);
            (api.gtk_window_set_default_size)(window, 720, 640);
            (api.gtk_widget_set_size_request)(window, 460, 400);
            for (signal, callback) in [
                (
                    c"delete-event",
                    std::mem::transmute::<
                        unsafe extern "C" fn(Widget, Widget, *mut c_void) -> c_int,
                        Callback,
                    >(close_callback),
                ),
                (
                    c"key-press-event",
                    std::mem::transmute::<
                        unsafe extern "C" fn(Widget, Widget, *mut c_void) -> c_int,
                        Callback,
                    >(key_callback),
                ),
            ] {
                let data = Box::new(WindowAction {
                    hide: api.gtk_widget_hide,
                    keyval: api.gdk_event_get_keyval,
                    state: api.gdk_event_get_state,
                });
                (api.g_signal_connect_data)(
                    window,
                    signal.as_ptr(),
                    Some(callback),
                    Box::into_raw(data).cast(),
                    Some(free_window_action),
                    0,
                );
            }
            let root = (api.gtk_box_new)(1, 12);
            (api.gtk_container_set_border_width)(root, 24);
            (api.gtk_container_add)(window, root);
            let header = (api.gtk_box_new)(0, 12);
            (api.gtk_box_pack_start)(header, brand_image, 0, 0, 0);
            let identity = (api.gtk_box_new)(1, 4);
            let brand = label(&api, "MicCamWatch");
            style(&api, brand, c"mcw-title");
            (api.gtk_box_pack_start)(identity, brand, 0, 0, 0);
            let version = label(&api, &format!("MCW  ·  {}", view.version));
            style(&api, version, c"dim-label");
            (api.gtk_box_pack_start)(identity, version, 0, 0, 0);
            (api.gtk_box_pack_start)(header, identity, 1, 1, 0);
            (api.gtk_box_pack_start)(root, header, 0, 0, 0);
            let status = label(&api, "");
            style(&api, status, c"mcw-heading");
            (api.gtk_box_pack_start)(root, status, 0, 0, 0);
            let feedback = label(&api, "");
            (api.gtk_label_set_lines)(feedback, 3);
            (api.gtk_label_set_ellipsize)(feedback, 3);
            (api.gtk_box_pack_start)(root, feedback, 0, 0, 0);
            // Display-only chrome must not become the initial text-selection
            // target ahead of the native tab navigation.
            for display in [brand, version, status, feedback] {
                (api.gtk_label_set_selectable)(display, 0);
            }
            let notebook = (api.gtk_notebook_new)();
            (api.gtk_notebook_set_scrollable)(notebook, 1);
            (api.gtk_box_pack_start)(root, notebook, 1, 1, 0);
            let overview = page(&api, notebook, text(view.language, "overview"));
            let mut cards = std::array::from_fn::<_, 2, _>(|_| Card {
                activity: ptr::null_mut(),
                protection: ptr::null_mut(),
            });
            for (index, resource) in ["microphone", "camera"].iter().enumerate() {
                let frame = (api.gtk_frame_new)(cstring(text(view.language, resource)).as_ptr());
                let activity = label(&api, "");
                style(&api, activity, c"mcw-heading");
                let protection = label(&api, "");
                let padding = (api.gtk_box_new)(1, 8);
                (api.gtk_container_set_border_width)(padding, 16);
                (api.gtk_box_pack_start)(padding, activity, 0, 0, 0);
                (api.gtk_box_pack_start)(padding, protection, 0, 0, 0);
                (api.gtk_container_add)(frame, padding);
                (api.gtk_box_pack_start)(overview, frame, 0, 0, 0);
                cards[index] = Card {
                    activity,
                    protection,
                };
            }
            let observed = label(&api, "");
            (api.gtk_box_pack_start)(overview, observed, 0, 0, 0);
            let access_count = label(&api, "");
            (api.gtk_box_pack_start)(overview, access_count, 0, 0, 0);
            let accesses = text_pane(&api, false);
            accessible(&api, accesses.widget, text(view.language, "accesses"));
            (api.gtk_widget_set_size_request)(accesses.scroll, -1, 150);
            (api.gtk_box_pack_start)(overview, accesses.scroll, 1, 1, 0);
            let controls_note = label(&api, text(view.language, "explicit"));
            (api.gtk_box_pack_start)(overview, controls_note, 0, 0, 0);
            let mut buttons = Vec::new();
            for name in ["mic", "camera"] {
                let button = (api.gtk_button_new_with_label)(c"".as_ptr());
                connect(
                    &api,
                    button,
                    c"clicked",
                    ActionKind::Command(Command::from_name(name).expect("native action")),
                    actions,
                    &rendering,
                    &congested,
                );
                wrap_button(&api, button);
                (api.gtk_box_pack_start)(overview, button, 0, 0, 0);
                buttons.push((name, button));
            }
            let settings = page(&api, notebook, text(view.language, "settings"));
            let profile_label = label(&api, super::words(language(view.language)).profile);
            (api.gtk_box_pack_start)(settings, profile_label, 0, 0, 0);
            let profile = (api.gtk_combo_box_text_new)();
            for choice in super::words(language(view.language)).profiles {
                (api.gtk_combo_box_text_append_text)(profile, cstring(choice).as_ptr());
            }
            accessible(&api, profile, super::words(language(view.language)).profile);
            connect(
                &api,
                profile,
                c"changed",
                ActionKind::Profile,
                actions,
                &rendering,
                &congested,
            );
            (api.gtk_box_pack_start)(settings, profile, 0, 0, 0);
            let profile_note = label(&api, "");
            (api.gtk_box_pack_start)(settings, profile_note, 0, 0, 0);
            let mut preferences = [ptr::null_mut(); 3];
            for (index, (key, on, off)) in [
                (
                    "notifications",
                    Command::NotificationsEnable,
                    Command::NotificationsDisable,
                ),
                ("sound", Command::SoundEnable, Command::SoundDisable),
                ("history", Command::HistoryEnable, Command::HistoryDisable),
            ]
            .into_iter()
            .enumerate()
            {
                let button = (api.gtk_check_button_new_with_label)(
                    cstring(text(view.language, key)).as_ptr(),
                );
                wrap_button(&api, button);
                connect(
                    &api,
                    button,
                    c"toggled",
                    ActionKind::Preference(on, off),
                    actions,
                    &rendering,
                    &congested,
                );
                (api.gtk_box_pack_start)(settings, button, 0, 0, 0);
                preferences[index] = button;
            }
            let notification_note = label(&api, text(view.language, "delivery"));
            (api.gtk_box_pack_start)(settings, notification_note, 0, 0, 0);
            for name in ["pause", "autostart"] {
                let button = (api.gtk_button_new_with_label)(c"".as_ptr());
                connect(
                    &api,
                    button,
                    c"clicked",
                    ActionKind::Command(Command::from_name(name).expect("native action")),
                    actions,
                    &rendering,
                    &congested,
                );
                wrap_button(&api, button);
                (api.gtk_box_pack_start)(settings, button, 0, 0, 0);
                buttons.push((name, button));
            }
            let diagnostics_page = page(&api, notebook, text(view.language, "diagnostics"));
            let diagnostics_note = label(&api, text(view.language, "copy"));
            (api.gtk_box_pack_start)(diagnostics_page, diagnostics_note, 0, 0, 0);
            let diagnostics = text_pane(&api, true);
            accessible(&api, diagnostics.widget, text(view.language, "diagnostics"));
            (api.gtk_widget_set_size_request)(diagnostics.scroll, -1, 260);
            (api.gtk_box_pack_start)(diagnostics_page, diagnostics.scroll, 1, 1, 0);
            let about = page(&api, notebook, text(view.language, "about"));
            for line in [
                format!("MicCamWatch {}", view.version),
                text(view.language, "description").to_owned(),
                text(view.language, "privacy").to_owned(),
                env!("CARGO_PKG_REPOSITORY").to_owned(),
            ] {
                (api.gtk_box_pack_start)(about, label(&api, &line), 0, 0, 0);
            }
            Self {
                api,
                window,
                notebook,
                status,
                feedback,
                cards,
                observed,
                access_count,
                accesses,
                diagnostics,
                profile,
                profile_note,
                preferences,
                buttons,
                rendering,
                congested,
                owner,
            }
        }
    }
    fn render(&mut self, view: &View) {
        self.rendering.set(true);
        let api = &self.api;
        let lang = view.language;
        let selected_page = unsafe { (api.gtk_notebook_get_current_page)(self.notebook) };
        unsafe {
            let headline = if view.overview.microphone == "active"
                || view.overview.camera == "active"
            {
                "activity_detected"
            } else if view.overview.microphone == "unknown" || view.overview.camera == "unknown" {
                "observation_incomplete"
            } else {
                "observation_current"
            };
            set_label(api, self.status, text(lang, headline));
            let mut feedback = view.feedback.clone().unwrap_or_default();
            if view.visual == "error" || view.visual == "partial" {
                append_line(&mut feedback, text(lang, "degraded"));
            }
            if view.limited || view.overview.text_limited {
                append_line(&mut feedback, text(lang, "limited"));
            }
            if self.congested.replace(false) {
                append_line(&mut feedback, text(lang, "busy"));
            }
            set_label(api, self.feedback, &feedback);
            (api.gtk_widget_set_tooltip_text)(self.feedback, cstring(&feedback).as_ptr());
            for (index, (observed, effective)) in [
                (view.overview.microphone, view.protection.microphone),
                (view.overview.camera, view.protection.camera),
            ]
            .into_iter()
            .enumerate()
            {
                let requested = match view.protection.requested[index] {
                    Some(true) => text(lang, "requested_on"),
                    Some(false) => text(lang, "requested_off"),
                    None => text(lang, "not_requested"),
                };
                set_label(
                    api,
                    self.cards[index].activity,
                    &format!("{}: {}", text(lang, "observed"), text(lang, observed)),
                );
                set_label(
                    api,
                    self.cards[index].protection,
                    &format!(
                        "{}: {}\n{}: {}",
                        text(lang, "effective"),
                        text(lang, effective),
                        text(lang, "requested"),
                        requested
                    ),
                );
            }
            set_label(
                api,
                self.observed,
                &format!(
                    "{}: {}",
                    text(lang, "observation_time"),
                    view.overview
                        .observed_at
                        .as_deref()
                        .unwrap_or(text(lang, "unknown"))
                ),
            );
            set_label(
                api,
                self.access_count,
                &format!(
                    "{}: {}  ·  {}: {}",
                    text(lang, "accesses"),
                    view.overview.total_accesses,
                    text(lang, "omitted"),
                    view.overview.omitted_accesses
                ),
            );
            let mut rows = String::new();
            for access in &view.overview.accesses {
                if !rows.is_empty() {
                    rows.push('\n');
                }
                let resource = match access.resource {
                    crate::model::Resource::Microphone => "microphone",
                    crate::model::Resource::Camera => "camera",
                };
                let activity = match access.activity {
                    crate::model::Activity::Active => "active",
                    crate::model::Activity::Ready => "ready",
                };
                rows.push_str(&format!(
                    "{}  ·  {}  ·  {}",
                    access.application,
                    text(lang, resource),
                    text(lang, activity)
                ));
                if let Some(pid) = access.pid {
                    rows.push_str(&format!("  ·  PID {pid}"));
                }
            }
            if rows.is_empty() {
                rows.push_str(text(lang, "empty"));
            }
            set_text(api, &mut self.accesses, rows);
            set_text(api, &mut self.diagnostics, diagnostic_document(view));
            if let Some(profile) = view.items.iter().find(|entry| entry.action == "profile") {
                let selected = profile
                    .children
                    .as_deref()
                    .and_then(|choices| {
                        choices
                            .iter()
                            .position(|choice| choice.checked == Some(true))
                    })
                    .map_or(-1, |index| index as c_int);
                if (api.gtk_combo_box_get_active)(self.profile) != selected {
                    (api.gtk_combo_box_set_active)(self.profile, selected);
                }
                (api.gtk_widget_set_sensitive)(self.profile, profile.enabled.into());
                set_label(
                    api,
                    self.profile_note,
                    if profile.enabled {
                        text(lang, "profile_note")
                    } else {
                        &profile.label
                    },
                );
            } else {
                (api.gtk_widget_set_sensitive)(self.profile, 0);
            }
            for (widget, active) in self.preferences.iter().zip([
                view.preferences.notifications_enabled,
                view.preferences.sound_enabled,
                view.preferences.history_enabled,
            ]) {
                if ((api.gtk_toggle_button_get_active)(*widget) != 0) != active {
                    (api.gtk_toggle_button_set_active)(*widget, active.into());
                }
            }
            for (name, button) in &self.buttons {
                if let Some(entry) = view.items.iter().find(|entry| entry.action == *name) {
                    (api.gtk_button_set_label)(*button, cstring(&entry.label).as_ptr());
                    wrap_button(api, *button);
                    (api.gtk_widget_set_sensitive)(*button, entry.enabled.into());
                    (api.gtk_widget_set_tooltip_text)(
                        *button,
                        entry
                            .detail
                            .as_deref()
                            .map(cstring)
                            .as_ref()
                            .map_or(ptr::null(), |text| text.as_ptr()),
                    );
                } else {
                    (api.gtk_widget_set_sensitive)(*button, 0);
                }
            }
            if (api.gtk_notebook_get_current_page)(self.notebook) != selected_page {
                (api.gtk_notebook_set_current_page)(self.notebook, selected_page);
            }
        }
        self.rendering.set(false);
    }
}
impl Drop for Ui {
    fn drop(&mut self) {
        // Destroy synchronously on the GTK owner, releasing signal userdata before
        // Rc guards and function table disappear. Do not run callbacks afterward.
        self.owner.destroy();
    }
}
fn append_line(value: &mut String, line: &str) {
    if !value.is_empty() {
        value.push('\n');
    }
    value.push_str(line);
}
unsafe fn accessible(api: &Api, widget: Widget, name: &str) {
    unsafe {
        (api.atk_object_set_name)(
            (api.gtk_widget_get_accessible)(widget),
            cstring(name).as_ptr(),
        );
    }
}
unsafe fn style(api: &Api, widget: Widget, class: &CStr) {
    unsafe {
        (api.gtk_style_context_add_class)(
            (api.gtk_widget_get_style_context)(widget),
            class.as_ptr(),
        );
    }
}
unsafe fn install_style(api: &Api, window: Widget) -> Widget {
    // Typography only. Backgrounds, borders, focus, selection and contrast
    // remain entirely controlled by the installed GTK theme.
    unsafe {
        let provider = (api.gtk_css_provider_new)();
        (api.gtk_css_provider_load_from_data)(provider, c".mcw-title { font-size: 1.5em; font-weight: 700; } .mcw-heading { font-size: 1.1em; font-weight: 600; }".as_ptr(), -1, ptr::null_mut());
        (api.gtk_style_context_add_provider_for_screen)(
            (api.gtk_widget_get_screen)(window),
            provider,
            600,
        );
        provider
    }
}
unsafe fn brand_image(api: &Api, window: Widget) -> Widget {
    const PNG: &[u8] = include_bytes!("../../../site/assets/favicon.png");
    unsafe {
        let loader = (api.gdk_pixbuf_loader_new)();
        let decoded =
            (api.gdk_pixbuf_loader_write)(loader, PNG.as_ptr(), PNG.len(), ptr::null_mut()) != 0;
        let closed = (api.gdk_pixbuf_loader_close)(loader, ptr::null_mut()) != 0;
        let pixbuf = (api.gdk_pixbuf_loader_get_pixbuf)(loader);
        if !decoded || !closed || pixbuf.is_null() {
            (api.g_object_unref)(loader);
            panic!("GTK cannot decode the bundled MCW logo");
        }
        (api.gtk_window_set_icon)(window, pixbuf);
        let scaled = (api.gdk_pixbuf_scale_simple)(pixbuf, 48, 48, 2);
        if scaled.is_null() {
            (api.g_object_unref)(loader);
            panic!("GTK cannot scale the bundled MCW logo");
        }
        let image = (api.gtk_image_new_from_pixbuf)(scaled);
        accessible(api, image, "MicCamWatch");
        (api.g_object_unref)(scaled);
        (api.g_object_unref)(loader);
        image
    }
}
unsafe fn wrap_button(api: &Api, widget: Widget) {
    unsafe {
        let child = (api.gtk_bin_get_child)(widget);
        if !child.is_null() {
            (api.gtk_label_set_line_wrap)(child, 1);
            (api.gtk_label_set_line_wrap_mode)(child, 2);
            (api.gtk_label_set_max_width_chars)(child, 60);
        }
    }
}
unsafe fn restore_scroll(api: &Api, pane: &mut TextPane) {
    if let Some((vertical, horizontal)) = pane.pending_scroll.take() {
        unsafe {
            (api.gtk_adjustment_set_value)(
                (api.gtk_scrolled_window_get_vadjustment)(pane.scroll),
                vertical,
            );
            (api.gtk_adjustment_set_value)(
                (api.gtk_scrolled_window_get_hadjustment)(pane.scroll),
                horizontal,
            );
        }
    }
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
        if CStr::from_ptr((api.gtk_label_get_text)(widget)) != value.as_c_str() {
            (api.gtk_label_set_text)(widget, value.as_ptr());
        }
    }
}
unsafe fn page(api: &Api, notebook: Widget, title: &str) -> Widget {
    unsafe {
        let scroll = (api.gtk_scrolled_window_new)(ptr::null_mut(), ptr::null_mut());
        (api.gtk_scrolled_window_set_policy)(scroll, 2, 1); // no horizontal, automatic vertical
        let content = (api.gtk_box_new)(1, 12);
        (api.gtk_container_set_border_width)(content, 20);
        (api.gtk_container_add)(scroll, content);
        let tab = (api.gtk_label_new)(cstring(title).as_ptr());
        (api.gtk_notebook_append_page)(notebook, scroll, tab);
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
        TextPane {
            widget,
            scroll,
            text: String::new(),
            pending_scroll: None,
        }
    }
}
unsafe fn set_text(api: &Api, pane: &mut TextPane, value: String) {
    if pane.text == value {
        return;
    }
    unsafe {
        let buffer = (api.gtk_text_view_get_buffer)(pane.widget);
        let (mut insert, mut bound) = (TextIter::default(), TextIter::default());
        (api.gtk_text_buffer_get_iter_at_mark)(
            buffer,
            &mut insert,
            (api.gtk_text_buffer_get_insert)(buffer),
        );
        (api.gtk_text_buffer_get_iter_at_mark)(
            buffer,
            &mut bound,
            (api.gtk_text_buffer_get_selection_bound)(buffer),
        );
        let offsets = (
            (api.gtk_text_iter_get_offset)(&insert),
            (api.gtk_text_iter_get_offset)(&bound),
        );
        let vertical = (api.gtk_scrolled_window_get_vadjustment)(pane.scroll);
        let horizontal = (api.gtk_scrolled_window_get_hadjustment)(pane.scroll);
        // A newer publication must not replace the pre-refresh viewport with
        // an adjustment temporarily clamped by unfinished GTK text layout.
        let scroll = pane.pending_scroll.unwrap_or_else(|| {
            (
                (api.gtk_adjustment_get_value)(vertical),
                (api.gtk_adjustment_get_value)(horizontal),
            )
        });
        let native = cstring(&value);
        (api.gtk_text_buffer_set_text)(buffer, native.as_ptr(), -1);
        let count = value.chars().count().min(c_int::MAX as usize) as c_int;
        (api.gtk_text_buffer_get_iter_at_offset)(buffer, &mut insert, offsets.0.min(count));
        (api.gtk_text_buffer_get_iter_at_offset)(buffer, &mut bound, offsets.1.min(count));
        (api.gtk_text_buffer_select_range)(buffer, &insert, &bound);
        (api.gtk_adjustment_set_value)(vertical, scroll.0);
        (api.gtk_adjustment_set_value)(horizontal, scroll.1);
        pane.pending_scroll = Some(scroll);
        pane.text = value;
    }
}
unsafe fn connect(
    api: &Api,
    widget: Widget,
    signal: &CStr,
    kind: ActionKind,
    actions: &SyncSender<Command>,
    rendering: &Rc<Cell<bool>>,
    congested: &Rc<Cell<bool>>,
) {
    let data = Box::new(Action {
        kind,
        actions: actions.clone(),
        rendering: rendering.clone(),
        congested: congested.clone(),
        get_toggle: api.gtk_toggle_button_get_active,
        get_combo: api.gtk_combo_box_get_active,
        get_sensitive: api.gtk_widget_is_sensitive,
    });
    unsafe {
        let callback = std::mem::transmute::<unsafe extern "C" fn(Widget, *mut c_void), Callback>(
            action_callback,
        );
        (api.g_signal_connect_data)(
            widget,
            signal.as_ptr(),
            Some(callback),
            Box::into_raw(data).cast(),
            Some(free_action),
            0,
        );
    }
}
fn run(api: Api, shared: Arc<Shared>, actions: SyncSender<Command>) -> Result<()> {
    unsafe {
        (api.g_set_prgname)(c"com.roman-cuisset.miccamwatch".as_ptr());
        (api.g_set_application_name)(c"MicCamWatch".as_ptr());
        (api.gdk_set_program_class)(c"MicCamWatch".as_ptr());
    }
    ensure!(
        unsafe { (api.gtk_init_check)(ptr::null_mut(), ptr::null_mut()) } != 0,
        "Native window unavailable: GTK cannot connect to a display. Run mcw tray open as your desktop user inside the graphical Wayland/X11 session (DISPLAY or WAYLAND_DISPLAY required), not through a headless/root helper. Tray and CLI remain available."
    );
    let initial = shared
        .view
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let mut ui = Ui::new(api, &initial, &actions);
    let mut revision = 0;
    while !shared.stop.load(Ordering::Acquire) {
        let opening = shared.open.swap(false, Ordering::AcqRel);
        let visible = opening || unsafe { (ui.api.gtk_widget_get_visible)(ui.window) } != 0;
        let current = shared.revision.load(Ordering::Acquire);
        if visible && (revision != current || opening || ui.congested.get()) {
            let view = shared
                .view
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            ui.render(&view);
            revision = current;
        }
        if opening {
            unsafe {
                (ui.api.gtk_widget_show_all)(ui.window);
                (ui.api.gtk_window_present)(ui.window);
            }
        }
        // Both iteration count and elapsed time are bounded. Never wait for an
        // event: shutdown/open/latest-state checks cannot be starved by GLib.
        let started = Instant::now();
        let mut settled = false;
        for _ in 0..32 {
            if unsafe { (ui.api.g_main_context_iteration)(ptr::null_mut(), 0) } == 0 {
                settled = true;
                break;
            }
            if started.elapsed() >= Duration::from_millis(4) {
                break;
            }
        }
        // GTK validates text lines incrementally in idle handlers. A bounded
        // event slice can end before those handlers finish; consuming the saved
        // viewport then lets subsequent validation overwrite it. Keep it until
        // the context drains, without blocking shutdown/open/latest-state work.
        if settled {
            unsafe {
                restore_scroll(&ui.api, &mut ui.accesses);
                restore_scroll(&ui.api, &mut ui.diagnostics);
            }
        }
        #[cfg(test)]
        tests::handle_probe(&mut ui, &shared, revision);
        thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}
fn language(code: &str) -> crate::i18n::Language {
    use crate::i18n::Language;
    match code {
        "fr" => Language::Fr,
        "de" => Language::De,
        "es" => Language::Es,
        "ja" => Language::Ja,
        "zh" => Language::Zh,
        "ru" => Language::Ru,
        _ => Language::En,
    }
}
fn text<'a>(code: &str, key: &'a str) -> &'a str {
    let index = match code {
        "fr" => 1,
        "de" => 2,
        "es" => 3,
        "ja" => 4,
        "zh" => 5,
        "ru" => 6,
        _ => 0,
    };
    let values = match key {
        "activity_detected" => [
            "Capture activity observed",
            "Activité de capture observée",
            "Aufnahmeaktivität beobachtet",
            "Actividad de captura observada",
            "キャプチャ動作を観測",
            "观测到采集活动",
            "Обнаружена активность захвата",
        ],
        "observation_incomplete" => [
            "Observation is incomplete",
            "Observation incomplète",
            "Beobachtung unvollständig",
            "Observación incompleta",
            "観測は不完全です",
            "观测不完整",
            "Наблюдение неполно",
        ],
        "observation_current" => [
            "Latest device observation",
            "Dernière observation des appareils",
            "Aktuelle Gerätebeobachtung",
            "Última observación de dispositivos",
            "最新のデバイス観測",
            "最新设备观测",
            "Последнее наблюдение устройств",
        ],
        "overview" => [
            "Overview",
            "Vue d’ensemble",
            "Übersicht",
            "Resumen",
            "概要",
            "概览",
            "Обзор",
        ],
        "settings" => [
            "Settings",
            "Réglages",
            "Einstellungen",
            "Ajustes",
            "設定",
            "设置",
            "Настройки",
        ],
        "diagnostics" => [
            "Diagnostics",
            "Diagnostic",
            "Diagnose",
            "Diagnóstico",
            "診断",
            "诊断",
            "Диагностика",
        ],
        "about" => [
            "About",
            "À propos",
            "Über",
            "Acerca de",
            "このアプリについて",
            "关于",
            "О программе",
        ],
        "microphone" => [
            "Microphone",
            "Microphone",
            "Mikrofon",
            "Micrófono",
            "マイク",
            "麦克风",
            "Микрофон",
        ],
        "camera" => [
            "Camera",
            "Caméra",
            "Kamera",
            "Cámara",
            "カメラ",
            "摄像头",
            "Камера",
        ],
        "observed" => [
            "Observed activity",
            "Activité observée",
            "Beobachtete Aktivität",
            "Actividad observada",
            "観測された動作",
            "观测活动",
            "Наблюдаемая активность",
        ],
        "effective" => [
            "Effective control",
            "Contrôle effectif",
            "Wirksame Steuerung",
            "Control efectivo",
            "実際の制御状態",
            "实际控制",
            "Фактическое управление",
        ],
        "requested" => [
            "Last manual request",
            "Dernière demande manuelle",
            "Letzte manuelle Anforderung",
            "Última solicitud manual",
            "最後の手動要求",
            "上次手动请求",
            "Последний ручной запрос",
        ],
        "requested_on" => [
            "Mute / restrict",
            "Couper / restreindre",
            "Stummschalten / einschränken",
            "Silenciar / restringir",
            "ミュート・制限",
            "静音／限制",
            "Отключить / ограничить",
        ],
        "requested_off" => [
            "Restore",
            "Restaurer",
            "Wiederherstellen",
            "Restaurar",
            "復元",
            "恢复",
            "Восстановить",
        ],
        "not_requested" => [
            "No request this session",
            "Aucune demande dans cette session",
            "Keine Anforderung in dieser Sitzung",
            "Sin solicitud en esta sesión",
            "このセッションでは要求なし",
            "本会话无请求",
            "В этом сеансе запросов не было",
        ],
        "observation_time" => [
            "Latest successful observation (UTC)",
            "Dernière observation réussie (UTC)",
            "Letzte erfolgreiche Beobachtung (UTC)",
            "Última observación correcta (UTC)",
            "最終観測成功時刻 (UTC)",
            "最近成功观测 (UTC)",
            "Последнее успешное наблюдение (UTC)",
        ],
        "active" => [
            "Active",
            "Actif",
            "Aktiv",
            "Activo",
            "使用中",
            "正在使用",
            "Активно",
        ],
        "ready" => [
            "Ready / open",
            "Prêt / ouvert",
            "Bereit / geöffnet",
            "Preparado / abierto",
            "待機・オープン",
            "就绪／已打开",
            "Готово / открыто",
        ],
        "idle" => [
            "No current access observed",
            "Aucun accès actuel observé",
            "Kein aktueller Zugriff beobachtet",
            "Sin acceso actual observado",
            "現在のアクセスは観測されていません",
            "未观测到当前访问",
            "Текущих обращений не обнаружено",
        ],
        "unknown" => [
            "Unknown / unavailable",
            "Inconnu / indisponible",
            "Unbekannt / nicht verfügbar",
            "Desconocido / no disponible",
            "不明・利用不可",
            "未知／不可用",
            "Неизвестно / недоступно",
        ],
        "muted" => [
            "Muted",
            "Coupé",
            "Stummgeschaltet",
            "Silenciado",
            "ミュート",
            "已静音",
            "Звук отключён",
        ],
        "unmuted" => [
            "Not muted",
            "Non coupé",
            "Nicht stummgeschaltet",
            "No silenciado",
            "ミュートなし",
            "未静音",
            "Звук не отключён",
        ],
        "mixed" => [
            "Mixed input states",
            "États des entrées mixtes",
            "Gemischte Eingangszustände",
            "Estados de entrada mixtos",
            "入力状態が混在",
            "输入状态混合",
            "Смешанные состояния входов",
        ],
        "owned" => [
            "MCW-owned camera restriction present",
            "Restriction caméra appartenant à MCW présente",
            "MCW-eigene Kamerasperre vorhanden",
            "Restricción de cámara de MCW presente",
            "MCWによるカメラ制限あり",
            "存在 MCW 摄像头限制",
            "Есть ограничение камеры MCW",
        ],
        "absent" => [
            "No MCW-owned camera block; not a permission guarantee",
            "Aucun blocage caméra MCW ; aucune garantie d’autorisation",
            "Keine MCW-Kamerasperre; keine Berechtigungsgarantie",
            "Sin bloqueo MCW; no garantiza permisos",
            "MCWのブロックなし（権限の保証ではありません）",
            "无 MCW 摄像头阻止；不保证权限",
            "Нет блокировки MCW; это не гарантия разрешений",
        ],
        "system_managed" => [
            "System-managed restriction",
            "Restriction gérée par le système",
            "Systemverwaltete Einschränkung",
            "Restricción del sistema",
            "システム管理の制限",
            "系统管理限制",
            "Системное ограничение",
        ],
        "accesses" => [
            "Current known accesses",
            "Accès actuels connus",
            "Aktuelle bekannte Zugriffe",
            "Accesos actuales conocidos",
            "現在判明しているアクセス",
            "当前已知访问",
            "Известные текущие обращения",
        ],
        "omitted" => [
            "Rows omitted",
            "Lignes omises",
            "Ausgelassene Zeilen",
            "Filas omitidas",
            "省略された行",
            "省略行数",
            "Пропущено строк",
        ],
        "empty" => [
            "No current application rows. Unknown or incomplete observation is not proof of no access.",
            "Aucune application actuelle. Une observation inconnue ou incomplète ne prouve pas l’absence d’accès.",
            "Keine aktuellen Anwendungen. Unbekannte oder unvollständige Beobachtung beweist keine Zugriffsfreiheit.",
            "Sin aplicaciones actuales. Una observación desconocida o incompleta no demuestra ausencia de acceso.",
            "現在のアプリ行はありません。不明・不完全な観測はアクセスがない証拠ではありません。",
            "无当前应用行。未知或不完整的观测不能证明没有访问。",
            "Нет строк приложений. Неизвестное или неполное наблюдение не доказывает отсутствие доступа.",
        ],
        "explicit" => [
            "Explicit controls only: microphone applies to this PipeWire session; supported USB camera blocks require administrator approval. Opening or refreshing this window never changes devices.",
            "Contrôles explicites : microphone limité à cette session PipeWire ; blocage des caméras USB compatibles avec autorisation administrateur. Ouvrir ou actualiser ne modifie aucun appareil.",
            "Explizite Steuerung: Mikrofon nur in dieser PipeWire-Sitzung; unterstützte USB-Kamerasperren benötigen Administratorfreigabe. Öffnen oder Aktualisieren ändert keine Geräte.",
            "Controles explícitos: micrófono solo en esta sesión PipeWire; bloqueo de cámaras USB compatibles con aprobación administrativa. Abrir o actualizar no cambia dispositivos.",
            "明示的な操作のみ：マイクはこのPipeWireセッション、対応USBカメラのブロックは管理者承認が必要です。表示・更新ではデバイスを変更しません。",
            "仅显式控制：麦克风限于当前 PipeWire 会话；支持的 USB 摄像头阻止需要管理员批准。打开或刷新不会更改设备。",
            "Только явное управление: микрофон в этом сеансе PipeWire; блокировка поддерживаемых USB-камер требует администратора. Открытие и обновление не меняют устройства.",
        ],
        "notifications" => [
            "Notifications",
            "Notifications",
            "Benachrichtigungen",
            "Notificaciones",
            "通知",
            "通知",
            "Уведомления",
        ],
        "sound" => [
            "Notification sound",
            "Son des notifications",
            "Benachrichtigungston",
            "Sonido de notificación",
            "通知音",
            "通知声音",
            "Звук уведомлений",
        ],
        "history" => [
            "Local event history",
            "Historique local",
            "Lokaler Ereignisverlauf",
            "Historial local",
            "ローカルイベント履歴",
            "本地事件历史",
            "Локальная история событий",
        ],
        "delivery" => [
            "Notification delivery depends on your desktop notification service. Pause suppresses alerts; history is stored locally only when enabled.",
            "La livraison dépend du service de notifications du bureau. La pause suspend les alertes ; l’historique reste local et optionnel.",
            "Zustellung benötigt den Benachrichtigungsdienst des Desktops. Pause unterdrückt Hinweise; Verlauf wird nur aktiviert lokal gespeichert.",
            "La entrega depende del servicio de notificaciones del escritorio. La pausa suprime alertas; el historial solo se guarda localmente si está habilitado.",
            "通知はデスクトップ通知サービスに依存します。一時停止は通知を抑制し、履歴は有効時のみローカル保存します。",
            "通知依赖桌面通知服务。暂停会抑制提醒；仅启用时才在本地存储历史。",
            "Доставка зависит от службы уведомлений рабочего стола. Пауза отключает оповещения; история хранится локально только при включении.",
        ],
        "profile_note" => [
            "A profile selects policy, not a mute or camera block. Balanced and Meeting currently share a policy.",
            "Un profil choisit la politique, sans couper ni bloquer. Équilibré et Réunion partagent actuellement la même politique.",
            "Profile wählen Richtlinien, keine Stumm- oder Kamerasperre. Ausgewogen und Besprechung nutzen derzeit dieselbe Richtlinie.",
            "El perfil selecciona política, no silencia ni bloquea. Equilibrado y Reunión comparten actualmente política.",
            "プロファイルはポリシーを選択します。ミュートやカメラブロックではありません。バランスと会議は同じポリシーです。",
            "配置选择策略，不会静音或阻止摄像头。平衡与会议目前使用相同策略。",
            "Профиль выбирает политику, а не отключает устройства. Сбалансированный и Встреча пока используют одну политику.",
        ],
        "copy" => [
            "Read-only diagnostics. Select text; Ctrl+A / Ctrl+C copy. Ctrl+W or Escape hides the window without stopping the tray.",
            "Diagnostic en lecture seule. Sélectionner ; Ctrl+A / Ctrl+C pour copier. Ctrl+W ou Échap masque sans arrêter.",
            "Schreibgeschützte Diagnose. Text auswählen; Strg+A / Strg+C kopiert. Strg+W oder Escape blendet das Fenster aus.",
            "Diagnóstico de solo lectura. Seleccione; Ctrl+A / Ctrl+C copia. Ctrl+W o Escape oculta sin detener la bandeja.",
            "読み取り専用の診断。Ctrl+A / Ctrl+Cでコピー。Ctrl+WまたはEscapeでトレイを止めず非表示にします。",
            "只读诊断。选择文本；Ctrl+A / Ctrl+C 复制。Ctrl+W 或 Escape 隐藏窗口但不停止托盘。",
            "Диагностика только для чтения. Ctrl+A / Ctrl+C — копировать. Ctrl+W или Escape скрывает окно, не останавливая трей.",
        ],
        "description" => [
            "Native microphone and camera observation with explicit, platform-limited privacy controls.",
            "Observation native du microphone et de la caméra avec contrôles explicites limités par la plateforme.",
            "Native Mikrofon- und Kamerabeobachtung mit expliziten, plattformbegrenzten Datenschutzfunktionen.",
            "Observación nativa del micrófono y cámara con controles explícitos limitados por la plataforma.",
            "マイク・カメラをネイティブに観測し、プラットフォームの範囲内で明示的に制御します。",
            "原生麦克风与摄像头观测，提供受平台限制的显式隐私控制。",
            "Нативное наблюдение за микрофоном и камерой с явным управлением в пределах возможностей платформы.",
        ],
        "privacy" => [
            "No automatic uploads or telemetry. This window shows the latest snapshot, not an additional history buffer.",
            "Aucun envoi automatique ni télémétrie. Dernier instantané uniquement, sans historique supplémentaire.",
            "Keine automatischen Uploads oder Telemetrie. Nur der neueste Zustand, kein zusätzlicher Verlaufspuffer.",
            "Sin cargas automáticas ni telemetría. Solo la última instantánea, sin búfer de historial adicional.",
            "自動アップロード・テレメトリなし。最新のスナップショットのみで、追加履歴は保持しません。",
            "无自动上传或遥测。本窗口仅显示最新快照，不添加历史缓冲区。",
            "Без автоматической отправки и телеметрии. Только последний снимок, без дополнительного буфера истории.",
        ],
        "degraded" => [
            "Observation or an action is incomplete. See Diagnostics; unknown is not safe or idle.",
            "Observation ou action incomplète. Voir Diagnostic ; inconnu ne signifie pas inactif ni protégé.",
            "Beobachtung oder Aktion unvollständig. Siehe Diagnose; unbekannt bedeutet nicht sicher oder inaktiv.",
            "Observación o acción incompleta. Consulte Diagnóstico; desconocido no significa seguro ni inactivo.",
            "観測または操作が不完全です。診断を確認してください。不明は安全・未使用を意味しません。",
            "观测或操作不完整。请查看诊断；未知不代表安全或空闲。",
            "Наблюдение или действие неполно. См. диагностику; неизвестно не значит безопасно или неактивно.",
        ],
        "limited" => [
            "Some text or current access rows are omitted; this view is incomplete.",
            "Du texte ou des accès actuels sont omis ; vue incomplète.",
            "Text oder aktuelle Zugriffe ausgelassen; Ansicht unvollständig.",
            "Se omiten textos o accesos actuales; vista incompleta.",
            "一部のテキストまたは現在のアクセス行を省略しています。",
            "部分文本或当前访问行被省略；此视图不完整。",
            "Часть текста или текущих обращений пропущена; вид неполон.",
        ],
        "busy" => [
            "The engine is busy; the action was not queued. Wait for current work, then retry.",
            "Moteur occupé ; action non envoyée. Attendre puis réessayer.",
            "Engine beschäftigt; Aktion nicht eingereiht. Warten und erneut versuchen.",
            "Motor ocupado; acción no enviada. Espere y reintente.",
            "エンジンは処理中です。操作は送信されませんでした。完了後に再試行してください。",
            "引擎忙；操作未排队。请等待当前工作完成后重试。",
            "Движок занят; действие не поставлено в очередь. Дождитесь завершения и повторите.",
        ],
        _ => return key,
    };
    values[index]
}

#[cfg(test)]
mod tests {
    use super::super::{
        CameraInspectionUi, DesktopAccess, DesktopHost, DesktopOverview, MenuContext, make_view,
    };
    use super::*;
    use crate::{
        i18n::Language,
        model::{Activity, MicrophoneMuteState, Resource},
        platform::SessionLockState,
        settings::{PrivacyProfile, Settings},
    };
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    pub(super) enum Probe {
        Snapshot,
        Tab(c_int),
        Preference(usize),
        Profile(c_int),
        Close,
        SelectDiagnostics,
        Scroll(f64),
        Control(&'static str),
    }
    pub(super) struct ProbeRequest {
        operation: Probe,
        revision: u64,
        reply: SyncSender<NativeState>,
    }
    #[derive(serde::Serialize)]
    struct NativeState {
        window: usize,
        visible: bool,
        tab: c_int,
        focus: usize,
        selection: (c_int, c_int),
        scroll: f64,
        headline: String,
        activities: [String; 2],
        protection: [String; 2],
        diagnostics: String,
        feedback: String,
        preferences: [bool; 3],
        profile: c_int,
        profile_enabled: bool,
        camera_enabled: bool,
        camera_tooltip: Option<String>,
        owner_thread: String,
    }
    impl Desktop {
        fn probe(&self, operation: Probe) -> NativeState {
            let (reply, response) = mpsc::sync_channel(1);
            *self.shared.probe.lock().unwrap() = Some(ProbeRequest {
                operation,
                revision: self.shared.revision.load(Ordering::Acquire),
                reply,
            });
            response
                .recv_timeout(Duration::from_secs(15))
                .unwrap_or_else(|error| {
                    panic!(
                        "GTK owner did not answer: {error}; native error: {:?}",
                        self.take_error()
                    )
                })
        }
    }
    unsafe fn native_text(api: &Api, widget: Widget) -> String {
        unsafe {
            CStr::from_ptr((api.gtk_label_get_text)(widget))
                .to_string_lossy()
                .into_owned()
        }
    }
    pub(super) fn handle_probe(ui: &mut Ui, shared: &Shared, revision: u64) {
        let mut pending = shared.probe.lock().unwrap();
        let Some(request) = pending.as_ref() else {
            return;
        };
        // An update/open can arrive after this iteration's render but before
        // its probe. Observe the requested publication, not the previous frame.
        // Hidden windows deliberately keep their last rendered state.
        if ui.accesses.pending_scroll.is_some()
            || ui.diagnostics.pending_scroll.is_some()
            || shared.open.load(Ordering::Acquire)
            || (unsafe { (ui.api.gtk_widget_get_visible)(ui.window) } != 0
                && revision < request.revision)
        {
            return;
        }
        let request = pending.take().expect("pending native probe");
        drop(pending);
        let api = &ui.api;
        unsafe {
            match request.operation {
                Probe::Snapshot => {}
                Probe::Tab(index) => (api.gtk_notebook_set_current_page)(ui.notebook, index),
                Probe::Preference(index) => (api.gtk_button_clicked)(ui.preferences[index]),
                Probe::Profile(index) => (api.gtk_combo_box_set_active)(ui.profile, index),
                Probe::Close => (api.gtk_window_close)(ui.window),
                Probe::Control(name) => {
                    let button = ui
                        .buttons
                        .iter()
                        .find(|(action, _)| *action == name)
                        .unwrap()
                        .1;
                    (api.gtk_button_clicked)(button);
                }
                Probe::SelectDiagnostics => {
                    (api.gtk_widget_grab_focus)(ui.diagnostics.widget);
                    let buffer = (api.gtk_text_view_get_buffer)(ui.diagnostics.widget);
                    let (mut start, mut end) = (TextIter::default(), TextIter::default());
                    (api.gtk_text_buffer_get_iter_at_offset)(buffer, &mut start, 12);
                    (api.gtk_text_buffer_get_iter_at_offset)(buffer, &mut end, 3);
                    (api.gtk_text_buffer_select_range)(buffer, &start, &end);
                }
                Probe::Scroll(value) => (api.gtk_adjustment_set_value)(
                    (api.gtk_scrolled_window_get_vadjustment)(ui.diagnostics.scroll),
                    value,
                ),
            }
            for _ in 0..64 {
                if (api.g_main_context_iteration)(ptr::null_mut(), 0) == 0 {
                    break;
                }
            }
            let buffer = (api.gtk_text_view_get_buffer)(ui.diagnostics.widget);
            let (mut start, mut end) = (TextIter::default(), TextIter::default());
            (api.gtk_text_buffer_get_bounds)(buffer, &mut start, &mut end);
            let text = (api.gtk_text_buffer_get_text)(buffer, &start, &end, 0);
            let diagnostics = CStr::from_ptr(text).to_string_lossy().into_owned();
            (api.g_free)(text.cast());
            (api.gtk_text_buffer_get_iter_at_mark)(
                buffer,
                &mut start,
                (api.gtk_text_buffer_get_insert)(buffer),
            );
            (api.gtk_text_buffer_get_iter_at_mark)(
                buffer,
                &mut end,
                (api.gtk_text_buffer_get_selection_bound)(buffer),
            );
            let camera = ui
                .buttons
                .iter()
                .find(|(name, _)| *name == "camera")
                .unwrap()
                .1;
            let tooltip = (api.gtk_widget_get_tooltip_text)(camera);
            let camera_tooltip = if tooltip.is_null() {
                None
            } else {
                let value = CStr::from_ptr(tooltip).to_string_lossy().into_owned();
                (api.g_free)(tooltip.cast());
                Some(value)
            };
            let state = NativeState {
                window: ui.window as usize,
                visible: (api.gtk_widget_get_visible)(ui.window) != 0,
                tab: (api.gtk_notebook_get_current_page)(ui.notebook),
                focus: (api.gtk_window_get_focus)(ui.window) as usize,
                selection: (
                    (api.gtk_text_iter_get_offset)(&start),
                    (api.gtk_text_iter_get_offset)(&end),
                ),
                scroll: (api.gtk_adjustment_get_value)((api.gtk_scrolled_window_get_vadjustment)(
                    ui.diagnostics.scroll,
                )),
                headline: native_text(api, ui.status),
                activities: std::array::from_fn(|index| native_text(api, ui.cards[index].activity)),
                protection: std::array::from_fn(|index| {
                    native_text(api, ui.cards[index].protection)
                }),
                diagnostics,
                feedback: native_text(api, ui.feedback),
                preferences: std::array::from_fn(|index| {
                    (api.gtk_toggle_button_get_active)(ui.preferences[index]) != 0
                }),
                profile: (api.gtk_combo_box_get_active)(ui.profile),
                profile_enabled: (api.gtk_widget_is_sensitive)(ui.profile) != 0,
                camera_enabled: (api.gtk_widget_is_sensitive)(camera) != 0,
                camera_tooltip,
                owner_thread: format!("{:?}", thread::current().id()),
            };
            let _ = request.reply.try_send(state);
        }
    }
    fn fixture(lang: Language, settings: &Settings, explicit_policy: bool) -> View {
        let mut view = make_view(
            (
                "SIMULATED GTK proof — technical evidence belongs in Diagnostics\n".to_owned()
                    + &"Simulated collector evidence; no hardware validation.\n".repeat(160),
                "partial",
            ),
            MicrophoneMuteState::Unmuted,
            (false, None),
            settings,
            lang,
            &Some("SIMULATED UI FIXTURE — no device control or hardware validation.".to_owned()),
            MenuContext {
                lock: SessionLockState::Unknown,
                explicit_policy,
                inspection: &CameraInspectionUi::default(),
            },
        );
        view.overview = DesktopOverview {
            microphone: "active",
            camera: "unknown",
            observed_at: Some("2026-10-23T12:34:56Z".to_owned()),
            total_accesses: 12,
            omitted_accesses: 4,
            accesses: (0..8)
                .map(|index| DesktopAccess {
                    application: format!(
                        "SIMULATED application {index} — Français 日本語 中文 Русский"
                    ),
                    resource: if index % 2 == 0 {
                        Resource::Microphone
                    } else {
                        Resource::Camera
                    },
                    activity: Activity::Active,
                    pid: Some(4000 + index),
                })
                .collect(),
            text_limited: false,
        };
        view.protection.requested = [Some(true), None];
        super::super::bound_desktop_view(&mut view);
        view
    }
    fn publish(host: &mut DesktopHost, view: View) -> Result<()> {
        host.overview = view.overview.clone();
        host.requested = view.protection.requested;
        host.update(view)
    }
    fn state(host: &DesktopHost, probe: Probe) -> NativeState {
        host.window.as_ref().unwrap().probe(probe)
    }
    fn capture(proof: &Path, name: &str, state: &NativeState) -> Result<()> {
        thread::sleep(Duration::from_millis(180));
        let output = std::process::Command::new("import")
            .args(["-window", "root"])
            .arg(proof.join(format!("{name}.png")))
            .output()?;
        ensure!(
            output.status.success(),
            "native screenshot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(
            proof.join(format!("{name}.json")),
            serde_json::to_vec_pretty(state)?,
        )?;
        Ok(())
    }
    fn require_isolation() -> Result<()> {
        for (name, expected) in [
            ("GITHUB_ACTIONS", "true"),
            ("RUNNER_ENVIRONMENT", "github-hosted"),
            ("MCW_NATIVE_LINUX_ISOLATED", "1"),
        ] {
            ensure!(
                std::env::var(name).as_deref() == Ok(expected),
                "{name} must equal {expected}"
            );
        }
        let root = PathBuf::from(std::env::var("MCW_NATIVE_LINUX_ROOT")?);
        ensure!(
            root.is_absolute() && root.is_dir(),
            "private proof root is missing"
        );
        for name in [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_RUNTIME_DIR",
            "XDG_CACHE_HOME",
        ] {
            ensure!(
                PathBuf::from(std::env::var(name)?).starts_with(&root),
                "{name} escapes isolated root"
            );
        }
        ensure!(
            std::env::var("PIPEWIRE_REMOTE").as_deref() == Ok("mcw-native-no-hardware"),
            "fixture must not connect to host PipeWire"
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires explicitly isolated GitHub-hosted GTK/X11 and XFCE SNI desktop"]
    fn native_linux_desktop() -> Result<()> {
        require_isolation()?;
        let proof = PathBuf::from(std::env::var("MCW_LINUX_DESKTOP_PROOF")?);
        fs::create_dir_all(&proof)?;
        let lang = Language::from_code(&std::env::var("MCW_LINUX_DESKTOP_LANGUAGE")?)
            .context("unsupported language")?;
        let mut settings = Settings::update(|current| {
            current.notifications_enabled = true;
            current.sound_enabled = false;
            current.history_enabled = true;
        })?;
        let initial = fixture(lang, &settings, false);
        let (sender, receiver) = mpsc::sync_channel(32);
        let ready = Arc::new(AtomicBool::new(false));
        let mut host = DesktopHost::start(sender, ready, initial.clone())?;
        publish(&mut host, initial.clone())?;
        ensure!(
            OWNER.lock().unwrap().is_none(),
            "tray startup eagerly loaded GTK"
        );
        if std::env::var_os("MCW_LINUX_DESKTOP_HEADLESS").is_some() {
            for attempt in 0..2 {
                host.show_window()?;
                let deadline = Instant::now() + Duration::from_secs(10);
                let error = loop {
                    if let Some(error) = host.take_window_error() {
                        break error;
                    }
                    ensure!(
                        Instant::now() < deadline,
                        "headless GTK failure was not reported"
                    );
                    thread::sleep(Duration::from_millis(20));
                };
                ensure!(
                    error.contains("display") || error.contains("GTK 3"),
                    "GTK error is not actionable: {error}"
                );
                host.check()?;
                fs::write(proof.join(format!("headless-{attempt}.txt")), error)?;
                // Reap the failed session before exercising the same owner again.
                let failed = host.window.take().unwrap();
                drop(failed);
            }
            ensure!(
                receiver.try_recv().is_err(),
                "headless failure dispatched an action"
            );
            host.shutdown()?;
            fs::write(
                proof.join("passed"),
                b"GTK failure/retry kept the real SNI tray alive without actions.\n",
            )?;
            return Ok(());
        }
        host.show_window()?;
        let first = state(&host, Probe::Snapshot);
        ensure!(
            first.visible && first.window != 0,
            "real GTK window did not open"
        );
        ensure!(
            first.headline != initial.summary,
            "technical diagnostics leaked into the main headline"
        );
        ensure!(
            first.activities[0].contains(text(lang.code(), "active")),
            "positive observed activity lost"
        );
        ensure!(
            first.activities[1].contains(text(lang.code(), "unknown")),
            "missing coverage became idle"
        );
        ensure!(
            first.protection[0].contains(text(lang.code(), "unmuted"))
                && first.protection[0].contains(text(lang.code(), "requested_on")),
            "observed/requested/effective states conflated"
        );
        ensure!(
            receiver.try_recv().is_err(),
            "opening/rendering emitted a command"
        );
        capture(&proof, "overview", &first)?;
        // Resize the real top-level X11 window, not a synthetic layout surface.
        let windows = std::process::Command::new("xdotool")
            .args(["search", "--onlyvisible", "--name", "^MicCamWatch$"])
            .output()?;
        ensure!(windows.status.success(), "native GTK X11 window is missing");
        let windows = String::from_utf8(windows.stdout)?;
        let windows: Vec<_> = windows.split_whitespace().collect();
        ensure!(windows.len() == 1, "more than one native window exists");
        let window = windows[0];
        let resize = |width: &str, height: &str| -> Result<()> {
            ensure!(
                std::process::Command::new("xdotool")
                    .args(["windowsize", "--sync", window, width, height])
                    .status()?
                    .success(),
                "native resize failed"
            );
            thread::sleep(Duration::from_millis(120));
            Ok(())
        };
        resize("460", "400")?;
        capture(&proof, "overview-minimum", &state(&host, Probe::Snapshot))?;
        state(&host, Probe::Tab(1));
        capture(&proof, "settings-minimum", &state(&host, Probe::Snapshot))?;
        resize("720", "640")?;
        for (index, name) in [(1, "settings"), (2, "diagnostics"), (3, "about")] {
            let selected = state(&host, Probe::Tab(index));
            ensure!(
                selected.tab == index,
                "native notebook selection did not change"
            );
            capture(&proof, name, &selected)?;
        }
        state(&host, Probe::Tab(2));
        state(&host, Probe::SelectDiagnostics);
        let before = state(&host, Probe::Scroll(90.0));
        ensure!(
            before.focus != 0 && before.selection == (12, 3) && before.scroll > 0.0,
            "diagnostics focus/selection/scroll not established"
        );
        let mut refreshed = initial.clone();
        refreshed
            .diagnostics
            .push_str("\nSIMULATED refresh retained the selected text.");
        publish(&mut host, refreshed.clone())?;
        let after = state(&host, Probe::Snapshot);
        fs::write(
            proof.join("refresh-before.json"),
            serde_json::to_vec_pretty(&before)?,
        )?;
        fs::write(
            proof.join("refresh-after.json"),
            serde_json::to_vec_pretty(&after)?,
        )?;
        ensure!(
            after.window == first.window
                && after.tab == 2
                && after.focus == before.focus
                && after.selection == before.selection
                && (after.scroll - before.scroll).abs() < 1.0,
            "refresh reset native window/tab/focus/selection/scroll: \
             window {} -> {}, tab {} -> {}, focus {} -> {}, \
             selection {:?} -> {:?}, scroll {} -> {}",
            before.window,
            after.window,
            before.tab,
            after.tab,
            before.focus,
            after.focus,
            before.selection,
            after.selection,
            before.scroll,
            after.scroll
        );
        ensure!(
            after.diagnostics.contains("SIMULATED refresh"),
            "native text buffer did not receive refresh"
        );
        ensure!(
            receiver.try_recv().is_err(),
            "refresh/tab selection emitted an action"
        );
        capture(&proof, "refresh-retained-selection", &after)?;
        state(&host, Probe::Tab(1));
        for (index, off, on) in [
            (
                0,
                Command::NotificationsDisable,
                Command::NotificationsEnable,
            ),
            (1, Command::SoundDisable, Command::SoundEnable),
            (2, Command::HistoryDisable, Command::HistoryEnable),
        ] {
            for _ in 0..2 {
                let before = state(&host, Probe::Snapshot).preferences[index];
                state(&host, Probe::Preference(index));
                let command = receiver.recv_timeout(Duration::from_secs(2))?;
                let expected = if before { off } else { on };
                ensure!(
                    std::mem::discriminant(&command) == std::mem::discriminant(&expected),
                    "GTK preference emitted the wrong desired boolean"
                );
                // A concurrent writer changes another preference after render.
                let latest = Settings::update(|current| current.profile = PrivacyProfile::Private)?;
                let mut expected_settings = serde_json::to_value(latest)?;
                expected_settings
                    [["notifications_enabled", "sound_enabled", "history_enabled"][index]] =
                    serde_json::json!(!before);
                settings = super::super::select_desktop_preference(command)?;
                ensure!(
                    serde_json::to_value(&settings)? == expected_settings,
                    "preference discarded concurrent state"
                );
                let repeated = super::super::select_desktop_preference(command)?;
                ensure!(
                    serde_json::to_value(repeated)? == expected_settings,
                    "desired boolean is not idempotent"
                );
                publish(&mut host, fixture(lang, &settings, false))?;
                ensure!(
                    state(&host, Probe::Snapshot).preferences[index] != before,
                    "native checkbox failed to reflect persisted state"
                );
                ensure!(
                    receiver.try_recv().is_err(),
                    "rendering checkbox state emitted another command"
                );
            }
        }
        for (index, command) in [
            (0, Command::ProfileBalanced),
            (2, Command::ProfileMeeting),
            (3, Command::ProfileDevelopment),
        ] {
            state(&host, Probe::Profile(index));
            let delivered = receiver.recv_timeout(Duration::from_secs(2))?;
            ensure!(
                std::mem::discriminant(&command) == std::mem::discriminant(&delivered),
                "wrong native profile intent"
            );
            settings = super::super::select_profile(delivered, false, lang)?;
            publish(&mut host, fixture(lang, &settings, false))?;
            ensure!(
                state(&host, Probe::Snapshot).profile == index,
                "native profile failed to reflect persisted choice"
            );
        }
        // A full bounded command channel must reject, explain and resynchronize
        // a native checkbox action, never silently retain an unpersisted value.
        for _ in 0..32 {
            host.actions.try_send(Command::Status)?;
        }
        let persisted = state(&host, Probe::Snapshot).preferences[0];
        state(&host, Probe::Preference(0));
        thread::sleep(Duration::from_millis(50));
        let busy = state(&host, Probe::Snapshot);
        ensure!(
            busy.preferences[0] == persisted && busy.feedback.contains(text(lang.code(), "busy")),
            "congested action was not explained/resynchronized"
        );
        for _ in 0..32 {
            ensure!(
                matches!(receiver.try_recv()?, Command::Status),
                "congested checkbox queued a preference"
            );
        }
        ensure!(
            receiver.try_recv().is_err(),
            "unexpected command followed congested action"
        );
        capture(&proof, "busy-action", &busy)?;
        publish(&mut host, fixture(lang, &settings, true))?;
        let disabled = state(&host, Probe::Snapshot);
        ensure!(
            !disabled.profile_enabled && disabled.profile == -1,
            "explicit config left a selectable or falsely checked profile"
        );
        state(&host, Probe::Profile(0));
        ensure!(
            receiver.try_recv().is_err(),
            "disabled native profile dispatched an action"
        );
        // The fixture never invokes a hardware control, even if a CI image has a helper.
        let mut unavailable = fixture(lang, &settings, true);
        for entry in &mut unavailable.items {
            if matches!(entry.action, "mic" | "camera") {
                entry.enabled = false;
            }
        }
        publish(&mut host, unavailable)?;
        state(&host, Probe::Control("mic"));
        state(&host, Probe::Control("camera"));
        ensure!(
            receiver.try_recv().is_err(),
            "disabled native control dispatched an action"
        );
        if crate::privacy::camera_control_preflight().is_err() {
            ensure!(
                !disabled.camera_enabled && disabled.camera_tooltip.is_some(),
                "missing trusted camera helper was not explained/disabled"
            );
        }
        capture(&proof, "explicit-config-disabled", &disabled)?;
        let hidden = state(&host, Probe::Close);
        ensure!(
            !hidden.visible,
            "native delete-event destroyed or failed to hide the window"
        );
        let mut unknown = fixture(lang, &settings, false);
        unknown.overview = DesktopOverview::default();
        publish(&mut host, unknown)?;
        ensure!(
            !state(&host, Probe::Snapshot).visible,
            "hidden refresh reopened window"
        );
        host.show_window()?;
        let reopened = state(&host, Probe::Snapshot);
        ensure!(
            reopened.visible && reopened.window == first.window && reopened.tab == 1,
            "reopen lost singleton or selected tab"
        );
        ensure!(
            reopened
                .activities
                .iter()
                .all(|activity| activity.contains(text(lang.code(), "unknown"))),
            "empty unknown snapshot became idle"
        );
        ensure!(
            receiver.try_recv().is_err(),
            "close/hidden refresh/reopen changed devices or preferences"
        );
        capture(&proof, "reopened-unknown", &reopened)?;
        ensure!(
            std::process::Command::new("xdotool")
                .args([
                    "windowactivate",
                    "--sync",
                    window,
                    "key",
                    "--clearmodifiers",
                    "ctrl+w"
                ])
                .status()?
                .success(),
            "native keyboard hide failed"
        );
        thread::sleep(Duration::from_millis(100));
        ensure!(
            !state(&host, Probe::Snapshot).visible,
            "Ctrl+W did not hide the retained window"
        );
        host.show_window()?;
        ensure!(
            state(&host, Probe::Snapshot).window == first.window,
            "keyboard reopen replaced the window"
        );
        // Exercise ordered teardown/recreate on the same persistent GTK owner.
        let shared = host.window.as_ref().unwrap().shared.clone();
        host.window.take();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !shared.finished.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        ensure!(
            shared.finished.load(Ordering::Acquire),
            "GTK teardown did not finish"
        );
        host.show_window()?;
        let recreated = state(&host, Probe::Snapshot);
        ensure!(
            recreated.visible && recreated.owner_thread == first.owner_thread,
            "GTK recreate moved to another owning thread"
        );
        ensure!(
            receiver.try_recv().is_err(),
            "teardown/recreate emitted an action"
        );
        host.shutdown()?;
        fs::write(proof.join("passed"), b"Actual production GTK owner/widgets, isolated synthetic observations; no hardware calls.\n")?;
        Ok(())
    }
}
