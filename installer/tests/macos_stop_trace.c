/* Stop-client-only Darwin diagnostic. Never inspects buffers, addresses or paths.
 * Compile: xcrun clang -dynamiclib -O2 macos_stop_trace.c -o stop-trace.dylib
 * MCW_STOP_TRACE_FD must be an inherited, private, already-open descriptor.
 */
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/uio.h>
#include <unistd.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern ssize_t real_write_nocancel(int, const void *, size_t) __asm("_write$NOCANCEL");
static int trace_fd = -1;
static void trace(const char *operation, int fd, long long result, int error) {
    int saved = errno;
    if (trace_fd >= 0) {
        char line[192];
        int n = snprintf(line, sizeof(line),
                         "{\"operation\":\"%s\",\"fd\":%d,\"result\":%lld,\"errno\":%d}\n",
                         operation, fd, result, error);
        if (n > 0 && (size_t)n < sizeof(line))
            (void)real_write_nocancel(trace_fd, line, (size_t)n);
    }
    errno = saved;
}
__attribute__((constructor)) static void loaded(void) {
    int saved = errno;
    const char *value = getenv("MCW_STOP_TRACE_FD");
    if (value) {
        char *end = NULL;
        long number = strtol(value, &end, 10);
        if (end != value && *end == '\0' && number >= 3 && number <= 1048576)
            trace_fd = (int)number;
    }
    trace("interposer_loaded", trace_fd, 0, 0);
    errno = saved;
}
#define INTERPOSE(replacement, original) \
    __attribute__((used)) static const struct { const void *replace; const void *original; } \
    interpose_##replacement __attribute__((section("__DATA,__interpose"))) = \
        { (const void *)(uintptr_t)&replacement, (const void *)(uintptr_t)&original }
#define RETURN_TRACE(operation, fd, expression, type) \
    type result = (expression); int error = result < 0 ? errno : 0; \
    trace(operation, fd, (long long)result, error); return result

static int traced_socket(int domain, int type, int protocol) {
    RETURN_TRACE("socket", -1, socket(domain, type, protocol), int);
}
INTERPOSE(traced_socket, socket);
static int traced_connect(int fd, const struct sockaddr *address, socklen_t length) {
    RETURN_TRACE("connect", fd, connect(fd, address, length), int);
}
INTERPOSE(traced_connect, connect);
static int traced_setsockopt(int fd, int level, int option, const void *value, socklen_t length) {
    const char *operation = level == SOL_SOCKET && option == SO_RCVTIMEO ? "setsockopt_read_timeout" :
        level == SOL_SOCKET && option == SO_SNDTIMEO ? "setsockopt_write_timeout" : "setsockopt";
    RETURN_TRACE(operation, fd, setsockopt(fd, level, option, value, length), int);
}
INTERPOSE(traced_setsockopt, setsockopt);
static int traced_getsockopt(int fd, int level, int option, void *value, socklen_t *length) {
    RETURN_TRACE("getsockopt", fd, getsockopt(fd, level, option, value, length), int);
}
INTERPOSE(traced_getsockopt, getsockopt);
static int traced_getpeereid(int fd, uid_t *user, gid_t *group) {
    RETURN_TRACE("getpeereid", fd, getpeereid(fd, user, group), int);
}
INTERPOSE(traced_getpeereid, getpeereid);
static int traced_lstat(const char *path, struct stat *info) {
    RETURN_TRACE("lstat", -1, lstat(path, info), int);
}
INTERPOSE(traced_lstat, lstat);
static int traced_stat(const char *path, struct stat *info) {
    RETURN_TRACE("stat", -1, stat(path, info), int);
}
INTERPOSE(traced_stat, stat);
static int traced_fstatat(int fd, const char *path, struct stat *info, int flags) {
    RETURN_TRACE("fstatat", fd, fstatat(fd, path, info, flags), int);
}
INTERPOSE(traced_fstatat, fstatat);
static int traced_mkdir(const char *path, mode_t mode) {
    RETURN_TRACE("mkdir", -1, mkdir(path, mode), int);
}
INTERPOSE(traced_mkdir, mkdir);
static ssize_t traced_write(int fd, const void *buffer, size_t length) {
    RETURN_TRACE("write", fd, write(fd, buffer, length), ssize_t);
}
INTERPOSE(traced_write, write);
static ssize_t traced_writev(int fd, const struct iovec *vectors, int count) {
    RETURN_TRACE("writev", fd, writev(fd, vectors, count), ssize_t);
}
INTERPOSE(traced_writev, writev);
static ssize_t traced_read(int fd, void *buffer, size_t length) {
    RETURN_TRACE("read", fd, read(fd, buffer, length), ssize_t);
}
INTERPOSE(traced_read, read);
static ssize_t traced_send(int fd, const void *buffer, size_t length, int flags) {
    RETURN_TRACE("send", fd, send(fd, buffer, length, flags), ssize_t);
}
INTERPOSE(traced_send, send);
static ssize_t traced_sendto(int fd, const void *buffer, size_t length, int flags,
                           const struct sockaddr *address, socklen_t address_length) {
    RETURN_TRACE("sendto", fd, sendto(fd, buffer, length, flags, address, address_length), ssize_t);
}
INTERPOSE(traced_sendto, sendto);
static ssize_t traced_sendmsg(int fd, const struct msghdr *message, int flags) {
    RETURN_TRACE("sendmsg", fd, sendmsg(fd, message, flags), ssize_t);
}
INTERPOSE(traced_sendmsg, sendmsg);
static ssize_t traced_recv(int fd, void *buffer, size_t length, int flags) {
    RETURN_TRACE("recv", fd, recv(fd, buffer, length, flags), ssize_t);
}
INTERPOSE(traced_recv, recv);
static ssize_t traced_recvfrom(int fd, void *buffer, size_t length, int flags,
                             struct sockaddr *address, socklen_t *address_length) {
    RETURN_TRACE("recvfrom", fd, recvfrom(fd, buffer, length, flags, address, address_length), ssize_t);
}
INTERPOSE(traced_recvfrom, recvfrom);
static ssize_t traced_recvmsg(int fd, struct msghdr *message, int flags) {
    RETURN_TRACE("recvmsg", fd, recvmsg(fd, message, flags), ssize_t);
}
INTERPOSE(traced_recvmsg, recvmsg);
/* Rust/libSystem can bind cancellation entry points directly. Cover both, without
 * changing their cancellation semantics or replacing syscall return values. */
extern int real_connect_nocancel(int, const struct sockaddr *, socklen_t) __asm("_connect$NOCANCEL");
static int traced_connect_nocancel(int fd, const struct sockaddr *address, socklen_t length) {
    RETURN_TRACE("connect_nocancel", fd, real_connect_nocancel(fd, address, length), int);
}
INTERPOSE(traced_connect_nocancel, real_connect_nocancel);
extern ssize_t real_read_nocancel(int, void *, size_t) __asm("_read$NOCANCEL");
static ssize_t traced_read_nocancel(int fd, void *buffer, size_t length) {
    RETURN_TRACE("read_nocancel", fd, real_read_nocancel(fd, buffer, length), ssize_t);
}
INTERPOSE(traced_read_nocancel, real_read_nocancel);
static ssize_t traced_write_nocancel(int fd, const void *buffer, size_t length) {
    RETURN_TRACE("write_nocancel", fd, real_write_nocancel(fd, buffer, length), ssize_t);
}
INTERPOSE(traced_write_nocancel, real_write_nocancel);
