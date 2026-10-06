#define _POSIX_C_SOURCE 200809L
#include <sys/socket.h>
#include <errno.h>
#include <stdlib.h>
ssize_t __real_send(int, const void *, size_t, int);
ssize_t __real_recv(int, void *, size_t, int);
static int sends, reads;
ssize_t __wrap_send(int fd, const void *bytes, size_t count, int flags) {
    if (getenv("TEST_BLOCK_WRITE")) { errno = EAGAIN; return -1; }
    if (getenv("TEST_WRITE_ALL")) {
        ++sends;
        if (sends % 3 == 0) { errno = EAGAIN; return -1; }
        return __real_send(fd, bytes, count ? 1 : 0, flags);
    }
    return __real_send(fd, bytes, count, flags);
}
ssize_t __wrap_recv(int fd, void *bytes, size_t count, int flags) {
    /* Data is already present between observed WouldBlock and registration. */
    if (getenv("TEST_ARM_RACE") && !reads++) { errno = EAGAIN; return -1; }
    return __real_recv(fd, bytes, count, flags);
}
