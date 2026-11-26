#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static int ignore_error(Display *d, XErrorEvent *e) { (void)d; (void)e; return 0; }
static unsigned long pid_of(Display *d, Window w) {
  Atom actual; int format; unsigned long count, remaining; unsigned char *data = NULL;
  int status = XGetWindowProperty(d, w, XInternAtom(d, "_NET_WM_PID", False), 0, 1,
      False, XA_CARDINAL, &actual, &format, &count, &remaining, &data);
  unsigned long pid = status == Success && actual == XA_CARDINAL && format == 32 && count == 1
      ? *(unsigned long *)data : 0;
  if (data) XFree(data);
  return pid;
}
static unsigned char channel(unsigned long pixel, unsigned long mask) {
  if (!mask) return 0;
  while (!(mask & 1)) { mask >>= 1; pixel >>= 1; }
  return (unsigned char)(((pixel & mask) * 255) / mask);
}
static void find_owned(Display *d, Window w, unsigned long expected, unsigned depth,
    Window *selected, unsigned *matches) {
  if (depth > 16) return;
  XWindowAttributes attributes;
  if (pid_of(d, w) == expected && XGetWindowAttributes(d, w, &attributes)) {
    fprintf(stderr, "owned window=%lu depth=%u mapped=%d size=%dx%d\n",
        w, depth, attributes.map_state, attributes.width, attributes.height);
    if (attributes.map_state == IsViewable && attributes.width > 1 && attributes.height > 1) {
      *selected = w; ++*matches;
    }
  }
  Window root, parent, *children = NULL; unsigned int count = 0;
  if (XQueryTree(d, w, &root, &parent, &children, &count)) {
    for (unsigned int i = 0; i < count; ++i) find_owned(d, children[i], expected, depth + 1, selected, matches);
  }
  if (children) XFree(children);
}
int main(int argc, char **argv) {
  if (argc < 3) return 64;
  char *end; unsigned long expected = strtoul(argv[1], &end, 10);
  if (*end || expected < 2) return 64;
  Display *d = XOpenDisplay(NULL); if (!d) return 65;
  XSetErrorHandler(ignore_error);
  Window selected = 0; unsigned matches = 0;
  find_owned(d, DefaultRootWindow(d), expected, 0, &selected, &matches);
  if (matches > 1) { XCloseDisplay(d); return 67; }
  XWindowAttributes attributes;
  if (!selected || pid_of(d, selected) != expected || !XGetWindowAttributes(d, selected, &attributes)) {
    XCloseDisplay(d); return 68;
  }
  printf("{\"pid\":%lu,\"window\":%lu,\"width\":%d,\"height\":%d}\n", expected, selected, attributes.width, attributes.height);
  if (!strcmp(argv[2], "capture")) {
    if (argc != 4 || attributes.width < 1 || attributes.height < 1) return 64;
    XImage *image = XGetImage(d, selected, 0, 0, attributes.width, attributes.height, AllPlanes, ZPixmap);
    if (!image || pid_of(d, selected) != expected) return 69;
    FILE *f = fopen(argv[3], "wb"); if (!f) { XDestroyImage(image); return 70; }
    fprintf(f, "P6\n%d %d\n255\n", image->width, image->height);
    for (int y = 0; y < image->height; ++y) for (int x = 0; x < image->width; ++x) {
      unsigned long pixel = XGetPixel(image, x, y);
      unsigned char rgb[3] = { channel(pixel, image->red_mask), channel(pixel, image->green_mask), channel(pixel, image->blue_mask) };
      if (fwrite(rgb, 1, 3, f) != 3) return 71;
    }
    if (fclose(f)) return 71;
    XDestroyImage(image);
  } else if (!strcmp(argv[2], "close")) {
    XEvent e; memset(&e, 0, sizeof(e)); e.xclient.type = ClientMessage;
    e.xclient.window = selected; e.xclient.message_type = XInternAtom(d, "WM_PROTOCOLS", False);
    e.xclient.format = 32; e.xclient.data.l[0] = XInternAtom(d, "WM_DELETE_WINDOW", False);
    e.xclient.data.l[1] = CurrentTime;
    if (pid_of(d, selected) != expected || !XSendEvent(d, selected, False, NoEventMask, &e)) return 72;
    XFlush(d);
  } else if (strcmp(argv[2], "info")) return 64;
  XCloseDisplay(d); return 0;
}
