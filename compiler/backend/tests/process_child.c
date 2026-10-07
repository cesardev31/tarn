#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static void all(int fd, const void *bytes, size_t count) {
    const char *p = bytes;
    while (count) { ssize_t n = write(fd, p, count); if (n < 0 && errno == EINTR) continue; if (n <= 0) exit(90); p += n; count -= (size_t)n; }
}
int main(int argc, char **argv) {
    if (argc < 2) return 91;
    if (!strcmp(argv[1], "args")) { for (int i = 2; i < argc; ++i) puts(argv[i]); return 0; }
    if (!strcmp(argv[1], "cwd")) { char text[4096]; if (!getcwd(text, sizeof(text))) return 92; puts(text); return 0; }
    if (!strcmp(argv[1], "env")) { const char *value = getenv("TARN_PROCESS_TEST_ENV"); if (!value) return 93; puts(value); return 0; }
    if (!strcmp(argv[1], "stdin")) { char byte; if (read(0, &byte, 1) != 0) return 94; puts("eof"); return 0; }
    if (!strcmp(argv[1], "large")) { char out[4096], err[4096]; memset(out, 'o', sizeof(out)); memset(err, 'e', sizeof(err)); for (int i = 0; i < 32; ++i) { all(1, out, sizeof(out)); all(2, err, sizeof(err)); } all(1, out, 1); all(2, err, 1); return 7; }
    if (!strcmp(argv[1], "binary")) { const unsigned char bytes[] = {255, 0, 128}; all(1, bytes, sizeof(bytes)); all(2, bytes + 1, 1); return 0; }
    if (!strcmp(argv[1], "pause")) { for (;;) pause(); }
    if (!strcmp(argv[1], "fd")) { errno = 0; return fcntl(atoi(argv[2]), F_GETFD) == -1 && errno == EBADF ? 0 : 1; }
    return 95;
}
