/* Deterministic syscall failures for phase 15B; no sleeps. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int __real_open(const char *, int, ...);
ssize_t __real_read(int, void *, size_t);
ssize_t __real_write(int, const void *, size_t);
int __real_close(int);
static int target = -1, interrupted_open, interrupted_read, interrupted_write;
int __wrap_open(const char *path, int flags, ...) {
    va_list args; va_start(args, flags); int mode = va_arg(args, int); va_end(args);
    if (strstr(path, "/denied")) { errno = EACCES; return -1; }
    if (strstr(path, "/fixture") && !interrupted_open++) { errno = EINTR; return -1; }
    int fd = __real_open(path, flags, mode);
    if (strstr(path, "/fixture")) target = fd;
    return fd;
}
ssize_t __wrap_read(int fd, void *data, size_t len) {
    if (fd == target) {
        if (!interrupted_read++) { errno = EINTR; return -1; }
        if (len > 2) len = 2;
    }
    return __real_read(fd, data, len);
}
ssize_t __wrap_write(int fd, const void *data, size_t len) {
    if (fd == target) {
        if (getenv("TARN_FS_ZERO_WRITE")) return 0;
        if (!interrupted_write++) { errno = EINTR; return -1; }
        if (len > 3) len = 3;
    }
    return __real_write(fd, data, len);
}
int __wrap_close(int fd) {
    int status = __real_close(fd);
    if (fd == target && status == 0 && getenv("TARN_FS_CLOSE_EINTR")) { errno = EINTR; return -1; }
    return status;
}
