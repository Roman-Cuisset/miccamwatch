/* Disposable native proof only: pause the genuine tray immediately before its
 * control socket bind, after its lease is already held. No syscall results or
 * device behavior are replaced. Inherited private pipes provide the barrier. */
#include <sys/socket.h>
#include <sys/un.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>

static int descriptor(const char *name) {
    const char *value = getenv(name);
    if (value == NULL) return -1;
    char *end = NULL;
    long number = strtol(value, &end, 10);
    return end != value && *end == '\0' && number >= 3 && number <= 1048576
        ? (int)number : -1;
}

static int paused_bind(int fd, const struct sockaddr *address, socklen_t length) {
    const char *expected = getenv("MCW_COLD_SOCKET");
    if (expected != NULL && address != NULL && address->sa_family == AF_UNIX &&
        length > offsetof(struct sockaddr_un, sun_path)) {
        const struct sockaddr_un *local = (const struct sockaddr_un *)address;
        size_t capacity = length - offsetof(struct sockaddr_un, sun_path);
        if (capacity > sizeof(local->sun_path)) capacity = sizeof(local->sun_path);
        size_t size = strlen(expected);
        if (size <= capacity && size < sizeof(local->sun_path) &&
            memcmp(expected, local->sun_path, size) == 0 &&
            (size == capacity || local->sun_path[size] == '\0')) {
            int ready = descriptor("MCW_COLD_READY_FD");
            int release = descriptor("MCW_COLD_RELEASE_FD");
            if (ready < 0 || release < 0) _exit(78);
            char byte = 'L';
            ssize_t result;
            do { result = write(ready, &byte, 1); } while (result < 0 && errno == EINTR);
            if (result != 1) _exit(78);
            do { result = read(release, &byte, 1); } while (result < 0 && errno == EINTR);
            if (result != 1 || byte != 'R') _exit(78);
            close(ready);
            close(release);
            // Keep the actual AppKit child free of test interposition.
            unsetenv("DYLD_INSERT_LIBRARIES");
            unsetenv("MCW_COLD_SOCKET");
        }
    }
    return bind(fd, address, length);
}

__attribute__((used)) static const struct { const void *replace; const void *original; }
    interpose_bind __attribute__((section("__DATA,__interpose"))) = {
        (const void *)(uintptr_t)&paused_bind, (const void *)(uintptr_t)&bind
    };
