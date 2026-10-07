#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
/* Tarn internal runtime ABI v0: Linux x86_64, System V C ABI. */
#define _POSIX_C_SOURCE 200809L
#include <sys/timerfd.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <inttypes.h>
#include <math.h>

/* Opt-in allocation observations for compiler tests; no ownership decisions. */
static void exec_trace(const char *kind, const char *event, uintptr_t storage) {
    if (storage && getenv("TARN_TRACE_EXEC")) {
        flockfile(stderr);
        fprintf(stderr, "exec:%s:%s:%" PRIxPTR "\n", kind, event, storage);
        funlockfile(stderr);
    }
}

typedef struct { uint64_t len; unsigned char bytes[]; } TarnString;
/* Owned string pointers are unique. Literals allocate; moves transfer ownership. */
TarnString *tarn_rt_string(const unsigned char *data, uint64_t len) {
    if (len > SIZE_MAX - sizeof(TarnString)) abort();
    TarnString *s = malloc(sizeof(TarnString) + len);
    if (!s) abort();
    s->len = len;
    if (len) memcpy(s->bytes, data, len);
    return s;
}
/* Strict UTF-8: reject overlong encodings, surrogates and values > U+10FFFF.
 * The private null result represents invalid bytes, never a Tarn string. */
TarnString *tarn_rt_string_utf8(const unsigned char *data, uint64_t len) {
    uint64_t i = 0;
    while (i < len) {
        unsigned char c = data[i++];
        if (c < 0x80) continue;
        uint64_t n;
        uint32_t cp, min;
        if (c >= 0xc2 && c <= 0xdf) { n = 1; cp = c & 0x1f; min = 0x80; }
        else if (c >= 0xe0 && c <= 0xef) { n = 2; cp = c & 0x0f; min = 0x800; }
        else if (c >= 0xf0 && c <= 0xf4) { n = 3; cp = c & 0x07; min = 0x10000; }
        else return NULL;
        if (n > len - i) return NULL;
        for (uint64_t j = 0; j < n; j++) {
            unsigned char next = data[i++];
            if ((next & 0xc0) != 0x80) return NULL;
            cp = (cp << 6) | (next & 0x3f);
        }
        if (cp < min || (cp >= 0xd800 && cp <= 0xdfff) || cp > 0x10ffff) return NULL;
    }
    return tarn_rt_string(data, len);
}
TarnString *tarn_rt_string_add(const TarnString *a, const TarnString *b) {
    if (b->len > SIZE_MAX - sizeof(TarnString) - a->len) abort();
    uint64_t len = a->len + b->len;
    TarnString *s = malloc(sizeof(TarnString) + len);
    if (!s) abort();
    s->len = len;
    if (a->len) memcpy(s->bytes, a->bytes, a->len);
    if (b->len) memcpy(s->bytes + a->len, b->bytes, b->len);
    return s;
}
int32_t tarn_rt_string_compare(const TarnString *a, const TarnString *b) {
    uint64_t len = a->len < b->len ? a->len : b->len;
    int order = len ? memcmp(a->bytes, b->bytes, len) : 0;
    if (order) return order < 0 ? -1 : 1;
    return (a->len > b->len) - (a->len < b->len);
}
void tarn_rt_drop_string(TarnString *s) {
    /* Opt-in compiler test observation, never a user destructor hook. */
    if (getenv("TARN_TRACE_DROPS")) { flockfile(stderr); fputs("drop:", stderr); fwrite(s->bytes, 1, s->len, stderr); fputc('\n', stderr); funlockfile(stderr); }
    free(s);
}
void tarn_rt_print_i64(int64_t v) { printf("%" PRId64 "\n", v); }
void tarn_rt_print_u64(uint64_t v) { printf("%" PRIu64 "\n", v); }
void tarn_rt_print_f64(double v) { printf("%.17g\n", v); }
void tarn_rt_print_bool(uint8_t v) { puts(v ? "true" : "false"); }
void tarn_rt_print_string(const TarnString *s) { fwrite(s->bytes, 1, s->len, stdout); fputc('\n', stdout); }
_Noreturn void tarn_rt_panic(const TarnString *s) {
    fputs("panic: ", stderr); fwrite(s->bytes, 1, s->len, stderr); fputc('\n', stderr); fflush(NULL); abort();
}
_Noreturn void tarn_rt_fault(void) { fputs("panic: checked arithmetic or bounds failure\n", stderr); fflush(NULL); abort(); }

float tarn_rt_rem_f32(float a, float b) { return fmodf(a, b); }
double tarn_rt_rem_f64(double a, double b) { return fmod(a, b); }

/* A closure environment header holds an internal, compiler-generated thunk.
 * Borrowed stack environments use a null thunk. Destruction and consumption
 * bodies are post-drop functions; their thunk releases owned storage. */
void *tarn_rt_env_alloc(uint64_t size) {
    void *p = malloc((size_t)size);
    if (!p) abort();
    exec_trace("storage", "alloc", (uintptr_t)p);
    return p;
}
void tarn_rt_env_free(void *p) { exec_trace("storage", "free", (uintptr_t)p); free(p); }
/* Vec storage: reallocate to `count` elements of `size` bytes. Overflow and
 * exhaustion abort. Element ownership and destruction stay in compiled code. */
void *tarn_rt_vec_grow(void *data, uint64_t count, uint64_t size) {
    if (size != 0 && count > UINT64_MAX / size) abort();
    uint64_t bytes = count * size;
    exec_trace("storage", "free", (uintptr_t)data);
    void *p = realloc(data, bytes ? (size_t)bytes : 1);
    if (!p) abort();
    exec_trace("storage", "alloc", (uintptr_t)p);
    return p;
}
void tarn_rt_env_drop(void *p) {
    if (!p) return;
    void (*drop)(void *);
    memcpy(&drop, p, sizeof(drop));
    if (drop) drop(p);
}

/* Private 11A task ABI. Worker/result adapters are generated by the compiler;
 * this record has no knowledge of Tarn types or resource layouts. */
#include <pthread.h>
typedef void (*TarnTaskWorker)(void *, void *, void *);
typedef void (*TarnTaskResultDrop)(void *);
typedef struct {
    pthread_t thread;
    TarnTaskWorker worker;
    TarnTaskResultDrop drop_result;
    void *code;
    void *environment;
    void *result;
    unsigned initialized;
    unsigned joined;
} TarnTask;

static _Noreturn void tarn_task_fault(void) {
    fputs("panic: native task runtime fault\n", stderr);
    fflush(NULL);
    abort();
}

static void *tarn_task_entry(void *argument) {
    TarnTask *task = argument;
    task->worker(task->result, task->code, task->environment);
    task->initialized = 1;
    return NULL;
}

TarnTask *tarn_rt_task_spawn(TarnTaskWorker worker, TarnTaskResultDrop drop_result,
                           uint64_t result_size, void *code, void *environment) {
    if (!worker || !drop_result || result_size > 65536) tarn_task_fault();
    TarnTask *task = calloc(1, sizeof(*task));
    if (!task) tarn_task_fault();
    task->result = calloc(1, result_size ? (size_t)result_size : 1);
    if (!task->result) tarn_task_fault();
    task->worker = worker;
    task->drop_result = drop_result;
    task->code = code;
    task->environment = environment;
    if (pthread_create(&task->thread, NULL, tarn_task_entry, task)) tarn_task_fault();
    return task;
}

/* Borrowed result storage, valid until release; ownership transfers only when
 * generated join code moves its initialized value into caller storage. */
void *tarn_rt_task_wait(TarnTask *task) {
    if (!task || task->joined || pthread_equal(task->thread, pthread_self())) tarn_task_fault();
    if (pthread_join(task->thread, NULL)) tarn_task_fault();
    task->joined = 1;
    if (!task->initialized) tarn_task_fault();
    return task->result;
}

/* Called only after generated code has moved the result out. */
void tarn_rt_task_release(TarnTask *task) {
    if (!task || !task->joined || !task->initialized) tarn_task_fault();
    task->initialized = 0;
    free(task->result);
    free(task);
}

void tarn_rt_task_drop(TarnTask *task) {
    void *result = tarn_rt_task_wait(task);
    task->drop_result(result);
    tarn_rt_task_release(task);
}


/* Private 11C ABI: native mechanics only. Payloads and destruction belong to
 * verified Tarn values. No recursive locking, poisoning or ownership bit. */
#include <stdatomic.h>
#include <stdbool.h>
#include <limits.h>
typedef struct { pthread_mutex_t mutex; } TarnMutex;
static void tarn_sync_trace(const char *event) {
    if (getenv("TARN_TRACE_SYNC")) {
        flockfile(stderr); fprintf(stderr, "sync:%s\n", event); funlockfile(stderr);
    }
}
TarnMutex *tarn_rt_mutex_create(void) {
    TarnMutex *m = malloc(sizeof(*m));
    if (!m || pthread_mutex_init(&m->mutex, NULL)) abort();
    tarn_sync_trace("create");
    return m;
}
void tarn_rt_mutex_lock(TarnMutex *m) {
    if (pthread_mutex_lock(&m->mutex)) abort();
    tarn_sync_trace("lock");
}
void tarn_rt_mutex_unlock(TarnMutex *m) {
    tarn_sync_trace("unlock");
    if (pthread_mutex_unlock(&m->mutex)) abort();
}
void tarn_rt_mutex_destroy(TarnMutex *m) {
    if (pthread_mutex_destroy(&m->mutex)) abort();
    tarn_sync_trace("destroy");
    free(m);
}

/* Each scalar is real C11 atomic storage. All operations, including failure
 * of compare/exchange, are sequentially consistent. Never expose its address. */
#define TARN_ATOMIC(NAME, TYPE) \
typedef struct { _Atomic(TYPE) value; } TarnAtomic##NAME; \
TarnAtomic##NAME *tarn_rt_atomic_##NAME##_new(TYPE value) { \
    TarnAtomic##NAME *a = malloc(sizeof(*a)); if (!a) abort(); \
    atomic_init(&a->value, value); return a; \
} \
TYPE tarn_rt_atomic_##NAME##_load(TarnAtomic##NAME *a) { \
    return atomic_load_explicit(&a->value, memory_order_seq_cst); \
} \
void tarn_rt_atomic_##NAME##_store(TarnAtomic##NAME *a, TYPE value) { \
    atomic_store_explicit(&a->value, value, memory_order_seq_cst); \
} \
TYPE tarn_rt_atomic_##NAME##_swap(TarnAtomic##NAME *a, TYPE value) { \
    return atomic_exchange_explicit(&a->value, value, memory_order_seq_cst); \
} \
uint8_t tarn_rt_atomic_##NAME##_compare_exchange(TarnAtomic##NAME *a, TYPE expected, TYPE value) { \
    return atomic_compare_exchange_strong_explicit(&a->value, &expected, value, memory_order_seq_cst, memory_order_seq_cst); \
}
TARN_ATOMIC(bool, uint8_t)
TARN_ATOMIC(i32, int32_t)
TARN_ATOMIC(i64, int64_t)
TARN_ATOMIC(u32, uint32_t)
TARN_ATOMIC(u64, uint64_t)
TARN_ATOMIC(usize, uintptr_t)

/* Checked CAS loops: failed exchanges retry from the new observed value.
 * Overflow aborts without writing; no signed C overflow is ever evaluated. */
#define TARN_ATOMIC_SIGNED(NAME, TYPE, MINIMUM, MAXIMUM) \
TYPE tarn_rt_atomic_##NAME##_fetch_add(TarnAtomic##NAME *a, TYPE value) { \
    TYPE old = atomic_load_explicit(&a->value, memory_order_seq_cst); \
    for (;;) { \
        if ((value > 0 && old > MAXIMUM - value) || (value < 0 && old < MINIMUM - value)) tarn_rt_fault(); \
        TYPE next = old + value; \
        if (atomic_compare_exchange_strong_explicit(&a->value, &old, next, memory_order_seq_cst, memory_order_seq_cst)) return old; \
    } \
} \
TYPE tarn_rt_atomic_##NAME##_fetch_sub(TarnAtomic##NAME *a, TYPE value) { \
    TYPE old = atomic_load_explicit(&a->value, memory_order_seq_cst); \
    for (;;) { \
        if ((value > 0 && old < MINIMUM + value) || (value < 0 && old > MAXIMUM + value)) tarn_rt_fault(); \
        TYPE next = old - value; \
        if (atomic_compare_exchange_strong_explicit(&a->value, &old, next, memory_order_seq_cst, memory_order_seq_cst)) return old; \
    } \
}
#define TARN_ATOMIC_UNSIGNED(NAME, TYPE, MAXIMUM) \
TYPE tarn_rt_atomic_##NAME##_fetch_add(TarnAtomic##NAME *a, TYPE value) { \
    TYPE old = atomic_load_explicit(&a->value, memory_order_seq_cst); \
    for (;;) { \
        if (old > MAXIMUM - value) tarn_rt_fault(); \
        TYPE next = old + value; \
        if (atomic_compare_exchange_strong_explicit(&a->value, &old, next, memory_order_seq_cst, memory_order_seq_cst)) return old; \
    } \
} \
TYPE tarn_rt_atomic_##NAME##_fetch_sub(TarnAtomic##NAME *a, TYPE value) { \
    TYPE old = atomic_load_explicit(&a->value, memory_order_seq_cst); \
    for (;;) { \
        if (old < value) tarn_rt_fault(); \
        TYPE next = old - value; \
        if (atomic_compare_exchange_strong_explicit(&a->value, &old, next, memory_order_seq_cst, memory_order_seq_cst)) return old; \
    } \
}
TARN_ATOMIC_SIGNED(i32, int32_t, INT32_MIN, INT32_MAX)
TARN_ATOMIC_SIGNED(i64, int64_t, INT64_MIN, INT64_MAX)
TARN_ATOMIC_UNSIGNED(u32, uint32_t, UINT32_MAX)
TARN_ATOMIC_UNSIGNED(u64, uint64_t, UINT64_MAX)
TARN_ATOMIC_UNSIGNED(usize, uintptr_t, UINTPTR_MAX)
void tarn_rt_atomic_destroy(void *a) { free(a); }

/* Private blocking networking bridge. No ownership inference or public errno API. */
#include <errno.h>
#include <stddef.h>
#include <sys/socket.h>
#include <netdb.h>
#include <arpa/inet.h>
#include <unistd.h>
typedef struct { uint8_t bytes[24]; } TarnNetAddr;
typedef struct { int32_t domain, code; int64_t value; TarnNetAddr address; } TarnNetRaw;
_Static_assert(sizeof(TarnNetRaw) == 40 && offsetof(TarnNetRaw, address) == 16, "network ABI");
static void net_init(TarnNetRaw *out) { memset(out, 0, sizeof(*out)); }
static void net_status(TarnNetRaw *out, int64_t value) { out->value = value; if (value < 0) out->code = errno; }
static void net_trace(const char *event, int fd) {
    if (getenv("TARN_TRACE_NET")) { flockfile(stderr); fprintf(stderr, "net:%s:%d\n", event, fd); funlockfile(stderr); }
}
static int net_native(const TarnNetAddr *address, struct sockaddr_storage *storage, socklen_t *len) {
    memset(storage, 0, sizeof(*storage));
    if (address->bytes[18] == 4) {
        struct sockaddr_in *a = (struct sockaddr_in *)storage;
        a->sin_family = AF_INET; memcpy(&a->sin_addr, address->bytes, 4); memcpy(&a->sin_port, address->bytes + 16, 2);
        *len = sizeof(*a); return AF_INET;
    }
    if (address->bytes[18] == 6) {
        struct sockaddr_in6 *a = (struct sockaddr_in6 *)storage;
        a->sin6_family = AF_INET6; memcpy(&a->sin6_addr, address->bytes, 16); memcpy(&a->sin6_port, address->bytes + 16, 2);
        uint32_t scope; memcpy(&scope, address->bytes + 20, 4); a->sin6_scope_id = ntohl(scope);
        *len = sizeof(*a); return AF_INET6;
    }
    errno = EINVAL; return -1;
}
static void net_address(TarnNetAddr *address, const struct sockaddr *sa) {
    memset(address, 0, sizeof(*address));
    if (sa->sa_family == AF_INET) {
        const struct sockaddr_in *a = (const struct sockaddr_in *)sa;
        memcpy(address->bytes, &a->sin_addr, 4); memcpy(address->bytes + 16, &a->sin_port, 2); address->bytes[18] = 4;
    } else if (sa->sa_family == AF_INET6) {
        const struct sockaddr_in6 *a = (const struct sockaddr_in6 *)sa;
        memcpy(address->bytes, &a->sin6_addr, 16); memcpy(address->bytes + 16, &a->sin6_port, 2); address->bytes[18] = 6;
        uint32_t scope = htonl(a->sin6_scope_id); memcpy(address->bytes + 20, &scope, 4);
    } else abort(); /* bridge only creates IPv4/IPv6 sockets */
}
void tarn_rt_net_resolve(TarnNetRaw *out, const TarnString *endpoint) {
    net_init(out);
    if (!endpoint->len || endpoint->len > 4096 || memchr(endpoint->bytes, 0, endpoint->len)) { out->domain = 2; out->code = EINVAL; return; }
    char *text = malloc(endpoint->len + 1); if (!text) abort();
    memcpy(text, endpoint->bytes, endpoint->len); text[endpoint->len] = 0;
    char *host = text, *port = NULL;
    if (*text == '[') { char *end = strchr(text, ']'); if (end && end[1] == ':') { *end = 0; host++; port = end + 2; } }
    else { char *colon = strrchr(text, ':'); if (colon && !memchr(text, ':', (size_t)(colon - text))) { *colon = 0; port = colon + 1; } }
    unsigned number = 0; int valid = port && *port;
    if (valid) for (const char *c = port; *c; c++) { if (*c < '0' || *c > '9' || number > 65535 / 10) { valid = 0; break; } number = number * 10 + (unsigned)(*c - '0'); if (number > 65535) { valid = 0; break; } }
    if (!valid) { free(text); out->domain = 2; out->code = EINVAL; return; }
    struct addrinfo hints = {0}, *list = NULL;
    hints.ai_family = AF_UNSPEC; hints.ai_socktype = SOCK_STREAM; hints.ai_flags = AI_NUMERICSERV | (!*host ? AI_PASSIVE : 0);
    int error = getaddrinfo(*host ? host : NULL, port, &hints, &list);
    if (error) { out->domain = error == EAI_SYSTEM ? 0 : 1; out->code = error == EAI_SYSTEM ? errno : error; free(text); return; }
    struct addrinfo *choice = NULL;
    for (struct addrinfo *a = list; a; a = a->ai_next) { if (a->ai_family == AF_INET) { choice = a; break; } if (!choice && a->ai_family == AF_INET6) choice = a; }
    if (choice) net_address(&out->address, choice->ai_addr); else { out->domain = 1; out->code = EAI_FAMILY; }
    freeaddrinfo(list); free(text);
}
void tarn_rt_net_socket(TarnNetRaw *out, const TarnNetAddr *address, uint8_t udp) {
    net_init(out); struct sockaddr_storage sa; socklen_t len;
    int family = net_native(address, &sa, &len);
    if (family < 0) { out->code = errno; return; }
    net_status(out, socket(family, udp ? SOCK_DGRAM : SOCK_STREAM, 0));
    if (!out->code) net_trace("open", (int)out->value);
}
void tarn_rt_net_bind(TarnNetRaw *out, int32_t fd, const TarnNetAddr *address) {
    net_init(out); struct sockaddr_storage sa; socklen_t len;
    if (net_native(address, &sa, &len) < 0) { out->code = errno; return; }
    /* TCP listeners reuse addresses held in TIME_WAIT by connections the
       server closed; Linux still rejects a second active listener. */
    int type = 0, one = 1; socklen_t size = sizeof(type);
    if (getsockopt(fd, SOL_SOCKET, SO_TYPE, &type, &size) == 0 && type == SOCK_STREAM)
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    net_status(out, bind(fd, (struct sockaddr *)&sa, len));
}
void tarn_rt_net_listen(TarnNetRaw *out, int32_t fd) { net_init(out); net_status(out, listen(fd, SOMAXCONN)); }
void tarn_rt_net_accept(TarnNetRaw *out, int32_t fd) { net_init(out); net_status(out, accept(fd, NULL, NULL)); if (!out->code) net_trace("open", (int)out->value); }
void tarn_rt_net_connect(TarnNetRaw *out, int32_t fd, const TarnNetAddr *address) {
    net_init(out); struct sockaddr_storage sa; socklen_t len;
    if (net_native(address, &sa, &len) < 0) { out->code = errno; return; }
    net_status(out, connect(fd, (struct sockaddr *)&sa, len));
}
void tarn_rt_net_read(TarnNetRaw *out, int32_t fd, uint8_t *bytes, uint64_t len) {
    net_init(out); if (len) net_status(out, recv(fd, bytes, len, 0));
}
void tarn_rt_net_write(TarnNetRaw *out, int32_t fd, const uint8_t *bytes, uint64_t len) { net_init(out); net_status(out, send(fd, bytes, len, MSG_NOSIGNAL)); }
void tarn_rt_net_recv(TarnNetRaw *out, int32_t fd, uint8_t *bytes, uint64_t len) {
    net_init(out); struct sockaddr_storage sa; socklen_t size = sizeof(sa);
    net_status(out, recvfrom(fd, bytes, len, 0, (struct sockaddr *)&sa, &size));
    if (!out->code) net_address(&out->address, (struct sockaddr *)&sa);
}
void tarn_rt_net_send(TarnNetRaw *out, int32_t fd, const uint8_t *bytes, uint64_t len, const TarnNetAddr *address) {
    net_init(out); struct sockaddr_storage sa; socklen_t size;
    if (net_native(address, &sa, &size) < 0) { out->code = errno; return; }
    net_status(out, sendto(fd, bytes, len, MSG_NOSIGNAL, (struct sockaddr *)&sa, size));
}
void tarn_rt_net_addr(TarnNetRaw *out, int32_t fd, uint8_t peer) {
    net_init(out); struct sockaddr_storage sa; socklen_t size = sizeof(sa);
    net_status(out, peer ? getpeername(fd, (struct sockaddr *)&sa, &size) : getsockname(fd, (struct sockaddr *)&sa, &size));
    if (!out->code) net_address(&out->address, (struct sockaddr *)&sa);
}
void tarn_rt_net_shutdown(TarnNetRaw *out, int32_t fd, int32_t how) { net_init(out); net_status(out, shutdown(fd, how)); }
static void net_timer_track(int32_t fd);
static void net_timer_forget(int32_t fd);
void tarn_rt_net_close(TarnNetRaw *out, int32_t fd) {
    /* Observe the release attempt before close makes fd reusable by another
     * task. Logging after close can misorder reuse and falsely report two owners.
     * This is only test observation; the syscall remains the actual release. */
    net_trace("close", fd);
    net_timer_forget(fd); /* before close: the number may be reused at once */
    net_init(out); net_status(out, close(fd));
    if (out->code == EBADF) abort(); /* impossible with a verified live owner */
    /* Linux consumes fd even when close reports EINTR: never retry. */
}
void tarn_rt_net_drop(int32_t fd) { TarnNetRaw out; tarn_rt_net_close(&out, fd); }
/* Phase 14B one-shot timer: a nonblocking CLOCK_MONOTONIC timerfd, readable once
 * it expires, so it shares the 12B Poll. No thread, no sleep. Zero fires at once
 * (a zero it_value would disarm instead). Closed like sockets by Timer's owner. */
void tarn_rt_net_timer_new(TarnNetRaw *out, int64_t millis) {
    net_init(out);
    if (millis < 0) { out->code = EINVAL; return; }
    int fd = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK | TFD_CLOEXEC);
    if (fd < 0) { out->code = errno; return; }
    struct itimerspec spec; memset(&spec, 0, sizeof(spec));
    spec.it_value.tv_sec = (time_t)(millis / 1000);
    spec.it_value.tv_nsec = (long)(millis % 1000) * 1000000L;
    if (millis == 0) spec.it_value.tv_nsec = 1;
    if (timerfd_settime(fd, 0, &spec, NULL) < 0) { out->code = errno; close(fd); return; }
    net_timer_track(fd);
    out->value = fd; net_trace("open", fd);
}
void tarn_rt_net_timer_read(TarnNetRaw *out, int32_t fd) {
    uint64_t expirations; net_init(out); net_status(out, read(fd, &expirations, sizeof(expirations)));
}

void tarn_rt_net_main_error(uint32_t kind, int32_t code) {
    static const char *names[] = {"address in use", "connection refused", "connection reset", "broken pipe", "timed out", "would block", "invalid address", "DNS failure", "other OS error", "write made no progress", "unexpected EOF", "limit exceeded", "not found", "permission denied", "already exists", "invalid input", "invalid data", "not a directory", "is a directory", "directory not empty", "storage full"};
    if (kind >= sizeof(names) / sizeof(names[0])) abort();
    fprintf(stderr, "I/O error: %s (native code %" PRId32 ")\n", names[kind], code);
}

/* Phase 12B: no application pointers survive any of these calls. */
#include <sys/epoll.h>
#include <fcntl.h>
#include <time.h>
#ifndef SO_COOKIE
#define SO_COOKIE 57
#endif
/* Poll bookkeeping does not own sockets. Cookie checks prevent fd reuse from
   turning an old token into modification/deletion authority over a new socket. */
typedef struct TarnRegistration {
    int fd;
    uint64_t cookie, token;
    struct TarnRegistration *next;
} TarnRegistration;
typedef struct { int fd; TarnRegistration *head; } TarnPoll;
typedef struct { uint64_t token; uint8_t readable, writable, error, hangup; uint8_t padding[4]; } TarnEvent;
_Static_assert(sizeof(TarnEvent) == 16 && offsetof(TarnEvent, hangup) == 11, "event ABI");
static _Atomic uint64_t net_next_token = 1;
void tarn_rt_net_nonblocking(TarnNetRaw *out, int32_t fd, uint8_t enabled) {
    net_init(out);
    int mode = fcntl(fd, F_GETFL);
    if (mode < 0) { out->code = errno; return; }
    net_status(out, fcntl(fd, F_SETFL, enabled ? mode | O_NONBLOCK : mode & ~O_NONBLOCK));
}
void tarn_rt_net_mode(TarnNetRaw *out, int32_t fd) {
    net_init(out); int mode = fcntl(fd, F_GETFL);
    if (mode < 0) out->code = errno; else out->value = !!(mode & O_NONBLOCK);
}
void tarn_rt_net_connected(TarnNetRaw *out, int32_t fd) {
    net_init(out); int error = 0; socklen_t n = sizeof(error);
    if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &error, &n) < 0) { out->code = errno; return; }
    if (error) { out->code = error; return; }
    struct sockaddr_storage peer; n = sizeof(peer);
    if (getpeername(fd, (struct sockaddr *)&peer, &n) < 0) out->code = errno == ENOTCONN ? EAGAIN : errno;
}
void tarn_rt_net_now(TarnNetRaw *out) {
    net_init(out); struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) < 0) out->code = errno;
    else out->value = (int64_t)now.tv_sec * 1000 + now.tv_nsec / 1000000;
}
void tarn_rt_net_poll_new(TarnNetRaw *out) {
    net_init(out); int fd = epoll_create1(EPOLL_CLOEXEC);
    if (fd < 0) { out->code = errno; return; }
    TarnPoll *poll = malloc(sizeof(*poll));
    if (!poll) { int error = ENOMEM; close(fd); out->code = error; return; }
    poll->fd = fd; poll->head = NULL; out->value = (int64_t)(uintptr_t)poll;
    net_trace("open", fd);
}
/* Timers have no SO_COOKIE and share one anonymous inode, so each live timer
 * gets a process-unique identity (high bit set: never a socket cookie). */
typedef struct TarnTimerId { int32_t fd; uint64_t id; struct TarnTimerId *next; } TarnTimerId;
static TarnTimerId *net_timers;
static uint64_t net_timer_next = 1;
static pthread_mutex_t net_timer_lock = PTHREAD_MUTEX_INITIALIZER;
static void net_timer_track(int32_t fd) {
    TarnTimerId *entry = malloc(sizeof(*entry)); if (!entry) abort();
    pthread_mutex_lock(&net_timer_lock);
    entry->fd = fd; entry->id = (net_timer_next++) | (UINT64_C(1) << 63); entry->next = net_timers; net_timers = entry;
    pthread_mutex_unlock(&net_timer_lock);
}
static void net_timer_forget(int32_t fd) {
    pthread_mutex_lock(&net_timer_lock);
    for (TarnTimerId **link = &net_timers; *link; link = &(*link)->next)
        if ((*link)->fd == fd) { TarnTimerId *old = *link; *link = old->next; free(old); break; }
    pthread_mutex_unlock(&net_timer_lock);
}
static int net_identity_of(int32_t fd, uint64_t *cookie) {
    socklen_t n = sizeof(*cookie);
    if (getsockopt(fd, SOL_SOCKET, SO_COOKIE, cookie, &n) == 0) return 0;
    if (errno != ENOTSOCK) return -1;
    int found = 0;
    pthread_mutex_lock(&net_timer_lock);
    for (TarnTimerId *t = net_timers; t; t = t->next) if (t->fd == fd) { *cookie = t->id; found = 1; break; }
    pthread_mutex_unlock(&net_timer_lock);
    if (!found) errno = ENOTSOCK;
    return found ? 0 : -1;
}
void tarn_rt_net_poll_ctl(TarnNetRaw *out, TarnPoll *poll, int32_t fd, int32_t operation, int32_t interest, uint64_t token) {
    net_init(out); uint64_t cookie;
    if (operation < 0 || operation > 2 || interest < 1 || interest > 3) { out->code = EINVAL; return; }
    if (net_identity_of(fd, &cookie) < 0) { out->code = errno; return; }
    TarnRegistration **link = &poll->head;
    while (*link && (*link)->fd != fd) link = &(*link)->next;
    TarnRegistration *record = *link;
    if (operation && (!record || record->token != token || record->cookie != cookie)) { out->code = ENOENT; return; }
    if (!operation && record && record->cookie == cookie) { out->code = EEXIST; return; }
    TarnRegistration *fresh = NULL;
    if (!operation) {
        fresh = malloc(sizeof(*fresh));
        if (!fresh) { out->code = ENOMEM; return; }
        /* Never wrap or reuse. Signed raw value preserves checked u64 conversion. */
        token = atomic_load(&net_next_token);
        for (;;) {
            if (token >= INT64_MAX) { free(fresh); out->code = EOVERFLOW; return; }
            if (atomic_compare_exchange_weak(&net_next_token, &token, token + 1)) break;
        }
    }
    struct epoll_event event = {0};
    event.events = EPOLLRDHUP | ((interest & 1) ? EPOLLIN : 0) | ((interest & 2) ? EPOLLOUT : 0);
    event.data.u64 = token;
    int op = operation == 0 ? EPOLL_CTL_ADD : operation == 1 ? EPOLL_CTL_MOD : EPOLL_CTL_DEL;
    if (epoll_ctl(poll->fd, op, fd, &event) < 0) { out->code = errno; free(fresh); return; }
    if (!operation) {
        fresh->fd = fd; fresh->cookie = cookie; fresh->token = token;
        fresh->next = record ? record->next : NULL; *link = fresh; free(record);
    } else if (operation == 2) { *link = record->next; free(record); }
    out->value = (int64_t)token;
}
void tarn_rt_net_poll_wait(TarnNetRaw *out, TarnPoll *poll, TarnEvent *events, uint64_t length, int32_t timeout) {
    net_init(out);
    if (!length || timeout < -1) { out->code = EINVAL; return; }
    struct epoll_event native[64];
    int count = epoll_wait(poll->fd, native, length < 64 ? (int)length : 64, timeout);
    if (count < 0) { out->code = errno; return; }
    for (int i = 0; i < count; ++i) {
        memset(&events[i], 0, sizeof(events[i]));
        events[i].token = native[i].data.u64;
        events[i].readable = !!(native[i].events & EPOLLIN);
        events[i].writable = !!(native[i].events & EPOLLOUT);
        events[i].error = !!(native[i].events & EPOLLERR);
        events[i].hangup = !!(native[i].events & (EPOLLHUP | EPOLLRDHUP));
    }
    out->value = count;
}
void tarn_rt_net_close_poll(TarnNetRaw *out, TarnPoll *poll) {
    tarn_rt_net_close(out, poll->fd);
    while (poll->head) { TarnRegistration *record = poll->head; poll->head = record->next; free(record); }
    free(poll);
}
void tarn_rt_net_poll_drop(TarnPoll *poll) { TarnNetRaw out; tarn_rt_net_close_poll(&out, poll); }

/* Mechanical wake bookkeeping, not an executor or application-state store. */
typedef struct TarnWake TarnWake;
typedef struct TarnSpawned TarnSpawned;
typedef struct { TarnPoll *poll; TarnWake *head; TarnSpawned *inbox, *inbox_tail; } TarnExecution;
struct TarnWake {
    TarnExecution *owner;
    TarnWake *next;
    uint64_t identity, token, parent;
    int fd;
    uint8_t queued;
};
static uint64_t net_identity(void) {
    uint64_t id = atomic_load(&net_next_token);
    for (;;) {
        if (id >= INT64_MAX) tarn_rt_fault();
        if (atomic_compare_exchange_weak(&net_next_token, &id, id + 1)) return id;
    }
}
void tarn_rt_net_exec_new(TarnNetRaw *out, TarnPoll *poll) {
    net_init(out); TarnExecution *owner = malloc(sizeof(*owner));
    if (!owner) { out->code = ENOMEM; return; }
    owner->poll = poll; owner->head = NULL; owner->inbox = owner->inbox_tail = NULL; out->value = (int64_t)(uintptr_t)owner;
}
void tarn_rt_net_waker_new(uintptr_t *out, TarnExecution *owner) {
    TarnWake *wake = calloc(1, sizeof(*wake)); if (!wake) tarn_rt_fault();
    exec_trace("waker", "alloc", (uintptr_t)wake);
    wake->identity = net_identity(); wake->owner = owner; wake->fd = -1;
    wake->queued = 1; wake->next = owner->head; owner->head = wake;
    *out = (uintptr_t)wake;
}
static TarnWake *net_wake_find(TarnExecution *owner, uint64_t id) {
    for (TarnWake *wake = owner->head; wake; wake = wake->next) if (wake->identity == id) return wake;
    return NULL;
}
static void net_wake_deliver(TarnWake *wake) {
    for (int depth = 0; wake; ++depth) {
        if (depth >= 64) tarn_rt_fault();
        wake->queued = 1;
        wake = net_wake_find(wake->owner, wake->parent);
    }
}
void tarn_rt_net_wake(TarnNetRaw *out, TarnWake *wake) { net_init(out); net_wake_deliver(wake); }
void tarn_rt_net_wake_link(TarnNetRaw *out, TarnWake *child, TarnWake *parent) {
    net_init(out);
    if (child->owner != parent->owner) { out->code = EINVAL; return; }
    TarnWake *next = parent;
    for (int depth = 0; next; ++depth) {
        if (next == child || depth >= 63) { out->code = EINVAL; return; }
        next = net_wake_find(next->owner, next->parent);
    }
    child->parent = parent->identity;
}
void tarn_rt_net_wake_owner(TarnNetRaw *out, TarnWake *wake, TarnExecution *owner) {
    net_init(out); if (wake->owner != owner) out->code = EINVAL;
}
void tarn_rt_net_wake_take(TarnNetRaw *out, TarnWake *wake) { net_init(out); out->value = wake->queued; wake->queued = 0; }
void tarn_rt_net_wake_arm(TarnNetRaw *out, TarnWake *wake, int32_t fd, int32_t interest) {
    if (wake->token && wake->fd != fd) { net_init(out); out->code = EINVAL; return; }
    tarn_rt_net_poll_ctl(out, wake->owner->poll, fd, wake->token ? 1 : 0, interest, wake->token);
    if (!out->code) { wake->fd = fd; wake->token = out->value; }
}
void tarn_rt_net_wake_clear(TarnNetRaw *out, TarnWake *wake) {
    net_init(out); wake->queued = 0;
    if (!wake->token) return;
    TarnPoll *poll = wake->owner->poll;
    /* Reuse the 12B token/cookie check before touching any live fd. */
    TarnNetRaw status; tarn_rt_net_poll_ctl(&status, poll, wake->fd, 2, 1, wake->token);
    /* A captured owner may already have closed. Retire only our token record;
       never erase replacement bookkeeping or modify an unrelated recycled fd. */
    TarnRegistration **link = &poll->head;
    while (*link && (*link)->token != wake->token) link = &(*link)->next;
    if (*link) { TarnRegistration *old = *link; *link = old->next; free(old); }
    wake->token = 0; wake->fd = -1;
}
void tarn_rt_net_exec_wait(TarnNetRaw *out, TarnExecution *owner, int32_t timeout) {
    TarnEvent events[64]; tarn_rt_net_poll_wait(out, owner->poll, events, 64, timeout);
    if (out->code) return;
    for (int i = 0; i < out->value; ++i) {
        for (TarnWake *wake = owner->head; wake; wake = wake->next)
            if (wake->token && wake->token == events[i].token) net_wake_deliver(wake);
    }
}
void tarn_rt_net_waker_drop(TarnWake *wake) {
    TarnNetRaw out; tarn_rt_net_wake_clear(&out, wake);
    TarnWake **link = &wake->owner->head;
    while (*link && *link != wake) link = &(*link)->next;
    if (!*link) tarn_rt_fault();
    *link = wake->next; exec_trace("waker", "free", (uintptr_t)wake); free(wake);
}
void tarn_rt_net_exec_drop(TarnExecution *owner) {
    if (owner->head) tarn_rt_fault(); /* Ordinary loans must keep Execution alive. */
    if (owner->inbox) tarn_rt_fault(); /* Spawned operations are drained by Tarn executors. */
    free(owner);
}

/* Phase 14A async task records. Storage only: compiled code copies and destroys
 * the typed result; Tarn decides scheduling, completion and abandonment. Two
 * references exist: the AsyncTask handle and the executor's runner entry. */
typedef struct {
    TarnExecution *owner;
    uint64_t joiner;     /* identity of a waiting Waker, 0 if none */
    uint32_t refs, complete, taken, abandoned;
    unsigned char result[];
} TarnAsyncTask;
TarnAsyncTask *tarn_rt_async_task_new(TarnExecution *owner, uint64_t size) {
    TarnAsyncTask *task = calloc(1, sizeof(TarnAsyncTask) + (size ? size : 1));
    if (!task) abort();
    task->owner = owner; task->refs = 2;
    exec_trace("task", "alloc", (uintptr_t)task);
    return task;
}
void *tarn_rt_async_task_result(TarnAsyncTask *task) { return task->result; }
/* After compiled code stored the result: mark complete and wake the joiner. */
void tarn_rt_async_task_complete(TarnAsyncTask *task) {
    if (task->complete || task->abandoned) tarn_rt_fault();
    task->complete = 1;
    if (task->joiner) { TarnWake *w = net_wake_find(task->owner, task->joiner); if (w) net_wake_deliver(w); }
}
/* 1 when an untaken result may be moved out now (marks it taken). */
int8_t tarn_rt_async_task_take(TarnAsyncTask *task) {
    if (!task->complete || task->taken) return 0;
    task->taken = 1; return 1;
}
/* Register the joining Waker; it must belong to the task's Execution. */
void tarn_rt_async_task_wait(TarnNetRaw *out, TarnAsyncTask *task, TarnWake *waker) {
    net_init(out);
    if (waker->owner != task->owner) { out->code = EINVAL; return; }
    task->joiner = waker->identity;
}
int8_t tarn_rt_async_task_abandoned(TarnAsyncTask *task) { return (int8_t)task->abandoned; }
void tarn_rt_async_task_release(TarnAsyncTask *task) {
    if (task->refs == 0) tarn_rt_fault();
    if (--task->refs == 0) { exec_trace("task", "free", (uintptr_t)task); free(task); }
}
/* Handle destruction: 1 when compiled code must destroy an untaken result.
 * Otherwise a pending task is marked abandoned for structured destruction. */
int8_t tarn_rt_async_task_handle_drop(TarnAsyncTask *task) {
    task->joiner = 0;
    if (task->complete && !task->taken) { task->taken = 1; return 1; }
    if (!task->complete) task->abandoned = 1;
    return 0;
}

/* Spawn mailbox: owned operation bytes moved in by Tarn, drained FIFO. */
struct TarnSpawned { TarnSpawned *next; uint64_t size; unsigned char bytes[]; };
void tarn_rt_async_inbox_push(TarnExecution *owner, const void *bytes, uint64_t size) {
    TarnSpawned *entry = malloc(sizeof(TarnSpawned) + (size ? size : 1));
    if (!entry) abort();
    entry->next = NULL; entry->size = size; memcpy(entry->bytes, bytes, size);
    if (owner->inbox_tail) owner->inbox_tail->next = entry; else owner->inbox = entry;
    owner->inbox_tail = entry;
}
/* Copies the oldest entry into `out` and frees it; 0 when empty. */
int8_t tarn_rt_async_inbox_pop(TarnExecution *owner, void *out, uint64_t size) {
    TarnSpawned *entry = owner->inbox;
    if (!entry) return 0;
    if (entry->size != size) tarn_rt_fault();
    owner->inbox = entry->next;
    if (!owner->inbox) owner->inbox_tail = NULL;
    memcpy(out, entry->bytes, size); free(entry);
    return 1;
}

/* Phase 15B syscall/ABI bridge. High-level retries and ownership are Tarn.
 * Domain 3 distinguishes filesystem errno from the networking categories. */
#include <dirent.h>
#include <sys/stat.h>
static void fs_error(TarnNetRaw *out, int code) { out->domain = 3; out->code = code; }
static void fs_status(TarnNetRaw *out, int64_t value) {
    net_init(out); out->value = value;
    if (value < 0) fs_error(out, errno);
}
static void fs_trace(const char *event, int fd) {
    if (getenv("TARN_TRACE_FS")) { flockfile(stderr); fprintf(stderr, "fs:%s:%d\n", event, fd); funlockfile(stderr); }
}
static char *fs_path(TarnNetRaw *out, const TarnString *path) {
    net_init(out);
    if (memchr(path->bytes, 0, path->len)) { fs_error(out, EINVAL); return NULL; }
    char *text = malloc(path->len + 1);
    if (!text) abort();
    if (path->len) memcpy(text, path->bytes, path->len);
    text[path->len] = 0;
    return text;
}
void tarn_rt_fs_open(TarnNetRaw *out, const TarnString *path, int32_t mode) {
    char *text = fs_path(out, path); if (!text) return;
    int flags;
    switch (mode) {
        case 0: flags = O_RDONLY; break;
        case 1: flags = O_RDWR | O_CREAT | O_TRUNC; break;
        case 2: flags = O_WRONLY | O_CREAT | O_APPEND; break;
        case 3: flags = O_RDWR; break;
        case 4: flags = O_RDWR | O_CREAT | O_EXCL; break;
        default: free(text); fs_error(out, EINVAL); return;
    }
    /* O_NONBLOCK prevents opening a FIFO from blocking before Tarn rejects
     * non-regular resources. It has no effect on regular-file operations. */
    fs_status(out, open(text, flags | O_CLOEXEC | O_NONBLOCK, 0666));
    free(text);
    if (!out->code) fs_trace("open", (int)out->value);
}
void tarn_rt_fs_read(TarnNetRaw *out, int32_t fd, uint8_t *bytes, uint64_t len) {
    net_init(out); if (!len) return;
    fs_status(out, read(fd, bytes, len > INT64_MAX ? INT64_MAX : len));
}
void tarn_rt_fs_write(TarnNetRaw *out, int32_t fd, const uint8_t *bytes, uint64_t len) {
    net_init(out); if (!len) return;
    fs_status(out, write(fd, bytes, len > INT64_MAX ? INT64_MAX : len));
}
void tarn_rt_fs_seek(TarnNetRaw *out, int32_t fd, int64_t offset, int32_t origin) { fs_status(out, lseek(fd, offset, origin)); }
static void fs_info(TarnNetRaw *out, const struct stat *info) {
    if (info->st_size < 0) { fs_error(out, EOVERFLOW); return; }
    out->value = info->st_size;
    out->address.bytes[0] = S_ISREG(info->st_mode) ? 1 : S_ISDIR(info->st_mode) ? 2 : 3;
}
void tarn_rt_fs_file_metadata(TarnNetRaw *out, int32_t fd) {
    struct stat info; fs_status(out, fstat(fd, &info)); if (!out->code) fs_info(out, &info);
}
void tarn_rt_fs_metadata(TarnNetRaw *out, const TarnString *path) {
    char *text = fs_path(out, path); if (!text) return;
    struct stat info; fs_status(out, stat(text, &info)); free(text); if (!out->code) fs_info(out, &info);
}
void tarn_rt_fs_sync(TarnNetRaw *out, int32_t fd) { fs_status(out, fsync(fd)); }
void tarn_rt_fs_close(TarnNetRaw *out, int32_t fd) {
    fs_trace("close", fd); fs_status(out, close(fd));
    if (out->code == EBADF) abort();
    /* Linux releases a valid descriptor even on EINTR; never retry close. */
}
void tarn_rt_fs_drop(int32_t fd) { TarnNetRaw out; tarn_rt_fs_close(&out, fd); }
void tarn_rt_fs_mkdir(TarnNetRaw *out, const TarnString *path) {
    char *text = fs_path(out, path); if (!text) return;
    fs_status(out, mkdir(text, 0777)); free(text);
}
void tarn_rt_fs_remove_file(TarnNetRaw *out, const TarnString *path) {
    char *text = fs_path(out, path); if (!text) return;
    fs_status(out, unlink(text)); free(text);
}
void tarn_rt_fs_remove_dir(TarnNetRaw *out, const TarnString *path) {
    char *text = fs_path(out, path); if (!text) return;
    fs_status(out, rmdir(text)); free(text);
}
void tarn_rt_fs_rename(TarnNetRaw *out, const TarnString *from, const TarnString *to) {
    char *a = fs_path(out, from); if (!a) return;
    char *b = fs_path(out, to); if (!b) { free(a); return; }
    fs_status(out, rename(a, b)); free(a); free(b);
}
void tarn_rt_fs_dir_open(TarnNetRaw *out, const TarnString *path) {
    char *text = fs_path(out, path); if (!text) return;
    DIR *directory = opendir(text);
    if (!directory) fs_error(out, errno);
    else { out->value = (int64_t)(uintptr_t)directory; fs_trace("dir_open", dirfd(directory)); }
    free(text);
}
void tarn_rt_fs_dir_next(TarnNetRaw *out, DIR *directory, uint8_t *bytes, uint64_t capacity) {
    net_init(out); errno = 0;
    struct dirent *entry = readdir(directory);
    if (!entry) { if (errno) fs_error(out, errno); else out->value = -1; return; }
    uint64_t len = strlen(entry->d_name);
    if (len > capacity) { fs_error(out, ENAMETOOLONG); return; }
    if (len) memcpy(bytes, entry->d_name, len);
    out->value = (int64_t)len;
}
void tarn_rt_fs_dir_drop(DIR *directory) {
    fs_trace("dir_close", dirfd(directory));
    if (closedir(directory) < 0 && errno == EBADF) abort();
}

/* Phase 15D: syscall/ABI bridge. Configuration, capture accumulation and
 * ownership are Tarn. No runtime owner bit or child registry. */
#include <spawn.h>
#include <signal.h>
#include <sys/wait.h>
extern char **environ;
static void process_trace(const char *event, int id) {
    if (getenv("TARN_TRACE_PROCESS")) { flockfile(stderr); fprintf(stderr, "process:%s:%d\n", event, id); funlockfile(stderr); }
}
static void process_error(TarnNetRaw *out, int code) { net_init(out); out->domain = 4; out->code = code; }
static void process_close(int fd) {
    process_trace("fd_close", fd);
    if (close(fd) < 0 && errno == EBADF) abort(); /* Never retry Linux close. */
}
static char *process_text(TarnNetRaw *out, const TarnString *value) {
    if (memchr(value->bytes, 0, value->len)) { process_error(out, EINVAL); return NULL; }
    char *text = malloc(value->len + 1); if (!text) abort();
    memcpy(text, value->bytes, value->len); text[value->len] = 0; return text;
}
static int process_fd(int fd) {
    if (fd < 0) return fd;
    if (fd < 3) {
        int next = fcntl(fd, F_DUPFD_CLOEXEC, 3), code = errno;
        (void)close(fd); errno = code; fd = next;
        if (fd < 0) return -1;
    }
    process_trace("fd_open", fd); return fd;
}
static int process_pipe(int fds[2]) {
    int pair[2];
    if (pipe2(pair, O_CLOEXEC) < 0) return errno;
    int first = process_fd(pair[0]);
    if (first < 0) { int code = errno; (void)close(pair[1]); return code; }
    int second = process_fd(pair[1]);
    if (second < 0) { int code = errno; process_close(first); return code; }
    fds[0] = first; fds[1] = second; return 0;
}
void tarn_rt_process_spawn(TarnNetRaw *out, const TarnString *program,
        const TarnString *const *arguments, uint64_t count, const TarnString *directory,
        uint8_t use_directory, uint8_t capture) {
    net_init(out);
    if (!program->len || (use_directory && !directory->len)) { process_error(out, EINVAL); return; }
    if (count > SIZE_MAX / sizeof(char *) - 2) { process_error(out, E2BIG); return; }
    char **argv = calloc(count + 2, sizeof(char *)); if (!argv) abort();
    char *cwd = NULL; int code = 0, pipes[4] = {-1, -1, -1, -1}, input = -1;
    posix_spawn_file_actions_t actions; posix_spawnattr_t attributes;
    int actions_ready = 0, attributes_ready = 0; pid_t pid = -1;
    argv[0] = process_text(out, program); if (!argv[0]) { code = out->code; goto done; }
    for (uint64_t i = 0; i < count; ++i) {
        argv[i + 1] = process_text(out, arguments[i]);
        if (!argv[i + 1]) { code = out->code; goto done; }
    }
    if (use_directory) { cwd = process_text(out, directory); if (!cwd) { code = out->code; goto done; } }
    code = posix_spawn_file_actions_init(&actions); if (code) goto done; actions_ready = 1;
    code = posix_spawnattr_init(&attributes); if (code) goto done; attributes_ready = 1;
    sigset_t empty, defaults; sigemptyset(&empty); sigemptyset(&defaults); sigaddset(&defaults, SIGPIPE);
    code = posix_spawnattr_setsigmask(&attributes, &empty); if (code) goto done;
    code = posix_spawnattr_setsigdefault(&attributes, &defaults); if (code) goto done;
    code = posix_spawnattr_setflags(&attributes, POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF); if (code) goto done;
    if (capture) {
        code = process_pipe(pipes); if (code) goto done;
        code = process_pipe(pipes + 2); if (code) goto done;
        input = process_fd(open("/dev/null", O_RDONLY | O_CLOEXEC)); if (input < 0) { code = errno; goto done; }
        code = posix_spawn_file_actions_adddup2(&actions, input, 0); if (code) goto done;
        code = posix_spawn_file_actions_adddup2(&actions, pipes[1], 1); if (code) goto done;
        code = posix_spawn_file_actions_adddup2(&actions, pipes[3], 2); if (code) goto done;
    }
    if (use_directory) { code = posix_spawn_file_actions_addchdir_np(&actions, cwd); if (code) goto done; }
    /* Child never inherits other application/runtime descriptors. GNU libc
     * closefrom/chdir spawn actions avoid unsafe post-fork Tarn callbacks. */
    code = posix_spawn_file_actions_addclosefrom_np(&actions, 3); if (code) goto done;
    code = posix_spawnp(&pid, argv[0], &actions, &attributes, argv, environ);
    if (!code) {
        net_init(out); out->value = pid; process_trace("spawn", pid);
        if (capture) {
            uint8_t *bytes = (uint8_t *)&out->address;
            for (int i = 0; i < 4; ++i) { bytes[i] = (uint32_t)pipes[0] >> (i * 8); bytes[4 + i] = (uint32_t)pipes[2] >> (i * 8); }
            pipes[0] = -1; pipes[2] = -1; /* Acquisition passed to Tarn owners. */
        }
    }
done:
    if (actions_ready) posix_spawn_file_actions_destroy(&actions);
    if (attributes_ready) posix_spawnattr_destroy(&attributes);
    for (int i = 0; i < 4; ++i) if (pipes[i] >= 0) process_close(pipes[i]);
    if (input >= 0) process_close(input);
    for (uint64_t i = 0; i < count + 1; ++i) free(argv[i]);
    free(argv); free(cwd);
    if (code) process_error(out, code);
}
void tarn_rt_process_wait(TarnNetRaw *out, int32_t pid) {
    process_trace("wait", pid); /* One completion attempt, even on an OS error. */
    net_init(out); if (pid <= 0) { process_error(out, EINVAL); return; }
    int status; pid_t result;
    do { result = waitpid(pid, &status, 0); } while (result < 0 && errno == EINTR);
    if (result < 0) { process_error(out, errno); return; }
    process_trace("reap", pid);
    if (WIFEXITED(status)) { out->value = WEXITSTATUS(status); }
    else if (WIFSIGNALED(status)) { out->value = WTERMSIG(status); ((uint8_t *)&out->address)[0] = 1; }
    else abort();
}
void tarn_rt_process_drop(int32_t pid) { TarnNetRaw out; tarn_rt_process_wait(&out, pid); }
void tarn_rt_process_pipe_drop(int32_t fd) { process_close(fd); }
void tarn_rt_process_close(TarnNetRaw *out, int32_t fd) { net_init(out); process_close(fd); }
void tarn_rt_process_read(TarnNetRaw *out, int32_t fd, uint8_t *bytes, uint64_t len) {
    net_init(out); ssize_t result = read(fd, bytes, len); out->value = result;
    if (result < 0) process_error(out, errno);
}
void tarn_rt_process_kill(TarnNetRaw *out, int32_t pid) {
    net_init(out);
    if (pid <= 0) { process_error(out, EINVAL); return; }
    if (kill(pid, SIGKILL) < 0) process_error(out, errno);
}
