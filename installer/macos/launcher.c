// Finder's short-lived native entrypoint. The outer LaunchServices application
// must retire: its Rust child has no AppleEvent loop. A later ordinary Finder
// reopen can then launch this entrypoint and authenticate `tray open` again.
// Rust and its one AppKit helper remain the only resident frontend owners.
#include <mach-o/dyld.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <spawn.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

extern char **environ;

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
    // Ignore LaunchServices' arguments; never rewrite the user's environment.
    char *arguments[] = {"mcw", "tray", "open", NULL};
    pid_t child;
    int error = posix_spawn(&child, resolved, NULL, NULL, arguments, environ);
    if (error != 0) {
        fprintf(stderr, "MicCamWatch: cannot start bundled mcw: %s\n", strerror(error));
        return 1;
    }
    // Report an observed early child refusal rather than treating spawn as
    // frontend readiness. A running tray is intentionally not waited for:
    // authenticated `tray status` remains the native readiness authority.
    for (int attempt = 0; attempt < 15; ++attempt) {
        int status = 0;
        pid_t result = waitpid(child, &status, WNOHANG);
        if (result == child) {
            if (WIFEXITED(status)) return WEXITSTATUS(status);
            fprintf(stderr, "MicCamWatch: bundled mcw terminated during launch\n");
            return 1;
        }
        if (result < 0 && errno != EINTR) {
            perror("MicCamWatch: cannot observe bundled mcw launch");
            return 1;
        }
        struct timespec delay = {.tv_sec = 0, .tv_nsec = 20000000};
        while (nanosleep(&delay, &delay) != 0 && errno == EINTR) {}
    }
    return 0;
}
