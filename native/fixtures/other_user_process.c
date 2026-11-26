#include <stdio.h>
#include <unistd.h>

// Disposable qualification process. Closing its owned stdin ends it normally.
int main(void) {
  printf("%d\n", getpid());
  fflush(stdout);
  (void)getchar();
  return 0;
}
