/* Disposable native proof only: pause the genuine tray immediately before its
 * prepared private socket is published, after its lease is already held. No
 * syscall results or device behavior are replaced. Private pipes are the barrier. */
#include <stdio.h>
#include <sys/stat.h>
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

static int paused_renamex_np(const char *source, const char *destination, unsigned int flags) {
    const char *expected = getenv("MCW_COLD_SOCKET");
    if (expected != NULL && destination != NULL && strcmp(destination, expected) == 0 &&
        flags == RENAME_EXCL) {
        struct stat info;
        if (lstat(source, &info) != 0 || !S_ISSOCK(info.st_mode) ||
            info.st_uid != geteuid() || (info.st_mode & 0777) != 0600) _exit(78);
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
    return renamex_np(source, destination, flags);
}

__attribute__((used)) static const struct { const void *replace; const void *original; }
    interpose_publish __attribute__((section("__DATA,__interpose"))) = {
        (const void *)(uintptr_t)&paused_renamex_np, (const void *)(uintptr_t)&renamex_np
    };
