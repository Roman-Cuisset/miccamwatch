#include "my_application.h"

#include <flutter_linux/flutter_linux.h>
#include <glib-unix.h>
#include <signal.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  guint activation_signal_source;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

// Called when first Flutter frame received.
static void first_frame_cb(MyApplication* self, FlView* view) {
  gtk_widget_show(gtk_widget_get_toplevel(GTK_WIDGET(view)));
}

// The native owner signals only its already-ready child; no public activation IPC.
static gboolean activate_owned_window(gpointer application) {
  GtkWindow* window =
      gtk_application_get_active_window(GTK_APPLICATION(application));
  if (window != nullptr) {
    gtk_window_present(window);
  }
  return G_SOURCE_CONTINUE;
}

static void set_bundled_window_icon(GtkWindow* window) {
  g_autoptr(GError) error = nullptr;
  g_autofree gchar* executable = g_file_read_link("/proc/self/exe", &error);
  if (executable == nullptr) {
    g_warning("Cannot locate MicCamWatch bundle icon: %s", error->message);
    return;
  }
  g_autofree gchar* bundle_directory = g_path_get_dirname(executable);
  // This is the existing pubspec asset in Flutter's installed relocatable bundle.
  // Never resolve against the caller's CWD or claim the native installer's identity.
  g_autofree gchar* icon = g_build_filename(
      bundle_directory, "data", "flutter_assets", "assets", "miccamwatch.png",
      nullptr);
  if (!gtk_window_set_icon_from_file(window, icon, &error)) {
    g_warning("Cannot load MicCamWatch bundle icon: %s", error->message);
  }
}

static void fit_window(GtkWindow* window) {
  GdkWindow* native = gtk_widget_get_window(GTK_WIDGET(window));
  if (native == nullptr) return;
  GdkMonitor* monitor = gdk_display_get_monitor_at_window(gtk_widget_get_display(GTK_WIDGET(window)), native);
  if (monitor == nullptr) return;
  GdkRectangle work;
  gdk_monitor_get_workarea(monitor, &work);
  GdkGeometry minimum = {};
  minimum.min_width = MIN(640, MAX(1, work.width - 24));
  minimum.min_height = MIN(440, MAX(1, work.height - 64));
  gtk_window_set_geometry_hints(window, nullptr, &minimum, GDK_HINT_MIN_SIZE);
  gint x, y, width, height;
  gtk_window_get_position(window, &x, &y);
  gtk_window_get_size(window, &width, &height);
  width = MIN(width, MAX(1, work.width - 24));
  height = MIN(height, MAX(1, work.height - 64));
  gtk_window_resize(window, width, height);
  gtk_window_move(window, CLAMP(x, work.x, work.x + MAX(0, work.width - width)),
                  CLAMP(y, work.y, work.y + MAX(0, work.height - height)));
}

static gboolean save_window(GtkWidget* widget, GdkEvent*, gpointer) {
  GtkWindow* window = GTK_WINDOW(widget);
  if (gtk_window_is_maximized(window)) return FALSE;
  gint x, y, width, height;
  gtk_window_get_position(window, &x, &y);
  gtk_window_get_size(window, &width, &height);
  g_autofree gchar* directory = g_build_filename(g_get_user_config_dir(), "miccamwatch-desktop", nullptr);
  if (g_mkdir_with_parents(directory, 0700) == 0) {
    g_autofree gchar* path = g_build_filename(directory, "window.ini", nullptr);
    g_autofree gchar* data = g_strdup_printf("[window]\nx=%d\ny=%d\nwidth=%d\nheight=%d\n", x, y, width, height);
    g_file_set_contents(path, data, -1, nullptr);
  }
  return FALSE;
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));
  gtk_window_set_title(window, "MicCamWatch");
  set_bundled_window_icon(window);

  // Use a header bar when running in GNOME as this is the common style used
  // by applications and is the setup most users will be using (e.g. Ubuntu
  // desktop).
  // If running on X and not using GNOME then just use a traditional title bar
  // in case the window manager does more exotic layout, e.g. tiling.
  // If running on Wayland assume the header bar will work (may need changing
  // if future cases occur).
  gboolean use_header_bar = TRUE;
#ifdef GDK_WINDOWING_X11
  GdkScreen* screen = gtk_window_get_screen(window);
  if (GDK_IS_X11_SCREEN(screen)) {
    const gchar* wm_name = gdk_x11_screen_get_window_manager_name(screen);
    if (g_strcmp0(wm_name, "GNOME Shell") != 0) {
      use_header_bar = FALSE;
    }
  }
#endif
  if (use_header_bar) {
    GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
    gtk_widget_show(GTK_WIDGET(header_bar));
    gtk_header_bar_set_title(header_bar, "MicCamWatch");
    gtk_header_bar_set_show_close_button(header_bar, TRUE);
    gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));
  }

  GdkRectangle work = {0, 0, 1040, 700};
  GdkDisplay* display = gtk_widget_get_display(GTK_WIDGET(window));
  GdkMonitor* monitor = gdk_display_get_primary_monitor(display);
  if (monitor == nullptr) monitor = gdk_display_get_monitor(display, 0);
  if (monitor != nullptr) gdk_monitor_get_workarea(monitor, &work);
  // Reserve room for the window manager's decorations.
  const int available_width = MAX(1, work.width - 24);
  const int available_height = MAX(1, work.height - 64);
  gtk_window_set_default_size(window, MIN(1040, available_width), MIN(700, available_height));
  GdkGeometry minimum = {};
  minimum.min_width = MIN(640, available_width);
  minimum.min_height = MIN(440, available_height);
  gtk_window_set_geometry_hints(window, nullptr, &minimum, GDK_HINT_MIN_SIZE);
  g_autofree gchar* frame_path = g_build_filename(
      g_get_user_config_dir(), "miccamwatch-desktop", "window.ini", nullptr);
  g_autoptr(GKeyFile) frame = g_key_file_new();
  if (g_key_file_load_from_file(frame, frame_path, G_KEY_FILE_NONE, nullptr)) {
    const int width = g_key_file_get_integer(frame, "window", "width", nullptr);
    const int height = g_key_file_get_integer(frame, "window", "height", nullptr);
    const int x = g_key_file_get_integer(frame, "window", "x", nullptr);
    const int y = g_key_file_get_integer(frame, "window", "y", nullptr);
    if (width >= 1 && width <= 20000 && height >= 1 && height <= 20000 &&
        x >= -100000 && x <= 100000 && y >= -100000 && y <= 100000) {
      gtk_window_set_default_size(window, MIN(width, available_width), MIN(height, available_height));
      gtk_window_move(window, x, y);
    }
  }
  g_signal_connect(window, "delete-event", G_CALLBACK(save_window), nullptr);
  g_signal_connect_object(gtk_window_get_screen(window), "monitors-changed",
      G_CALLBACK(fit_window), window, G_CONNECT_SWAPPED);
  g_signal_connect_object(window, "map", G_CALLBACK(fit_window), window, G_CONNECT_SWAPPED);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  // The Linux software compositor draws through Skia. Impeller text aborts
  // there, so honor the explicit software choice before creating the view.
  if (g_strcmp0(g_getenv("FLUTTER_LINUX_RENDERER"), "software") == 0) {
    fl_dart_project_set_enable_impeller(project, FALSE);
  }
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  // Background defaults to black, override it here if necessary, e.g. #00000000
  // for transparent.
  gdk_rgba_parse(&background_color, "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));

  // Show the window when Flutter renders.
  // Requires the view to be realized so we can start rendering.
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    g_warning("Failed to register: %s", error->message);
    *exit_status = 1;
    return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
  MyApplication* self = MY_APPLICATION(application);
  self->activation_signal_source =
      g_unix_signal_add(SIGUSR1, activate_owned_window, application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  if (self->activation_signal_source != 0) {
    g_source_remove(self->activation_signal_source);
    self->activation_signal_source = 0;
  }
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  // Preserve the Flutter development ID; this is not the installed native GTK
  // application's desktop/notification identity. The visible name is shared.
  g_set_prgname(APPLICATION_ID);
  g_set_application_name("MicCamWatch");

  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID, "flags",
                                     G_APPLICATION_NON_UNIQUE, nullptr));
}
