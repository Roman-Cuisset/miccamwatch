// Finder's native entrypoint. No shell, runtime, environment rewriting or
// resident wrapper: the existing Rust tray owns its normal lifecycle/lock.
#include <mach-o/dyld.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(void) {
    char executable[PATH_MAX];
    uint32_t size = sizeof(executable);
    if (_NSGetExecutablePath(executable, &size) != 0) {
        fputs("MicCamWatch: launcher path is too long\n", stderr);
        return 1;
    }
    char resolved[PATH_MAX];
    if (realpath(executable, resolved) == NULL) {
        perror("MicCamWatch: cannot resolve launcher");
        return 1;
    }
    char *separator = strrchr(resolved, '/');
    if (separator == NULL || (size_t)(separator - resolved) + sizeof("/mcw") > sizeof(resolved)) {
        fputs("MicCamWatch: invalid launcher path\n", stderr);
        return 1;
    }
    strcpy(separator, "/mcw");
    // Ignore LaunchServices' arguments. The app has exactly one launch mode.
    execl(resolved, "mcw", "tray", "run", (char *)NULL);
    perror("MicCamWatch: cannot start bundled mcw");
    return 1;
}
