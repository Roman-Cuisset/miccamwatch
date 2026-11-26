#include <stdio.h>
#include <unistd.h>
#include <libproc.h>
#include <sys/proc_info.h>

// Disposable qualification process. Closing its owned stdin ends it normally.
int main(void) {
  struct proc_bsdinfo info = {0};
  if (proc_pidinfo(getpid(), PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != sizeof(info)) return 2;
  printf("%d %u %llu %llu\n", getpid(), getuid(),
         (unsigned long long)info.pbi_start_tvsec,
         (unsigned long long)info.pbi_start_tvusec);
  fflush(stdout);
  int command;
  while ((command = getchar()) != EOF) {
    if (command == 'p') { puts("alive"); fflush(stdout); }
  }
  return 0;
}
