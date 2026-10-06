/* Tarn internal runtime ABI v0: Linux x86_64, System V C ABI. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <inttypes.h>
#include <math.h>

typedef struct { uint64_t len; unsigned char bytes[]; } TarnString;
/* Owned string pointers are unique. Literals allocate; moves transfer ownership. */
TarnString *tarn_rt_string(const unsigned char *data, uint64_t len) {
    if (len > SIZE_MAX - sizeof(TarnString)) abort();
    TarnString *s = malloc(sizeof(TarnString) + len);
    if (!s) abort();
    s->len = len;
    memcpy(s->bytes, data, len);
    return s;
}
void tarn_rt_drop_string(TarnString *s) {
    /* Opt-in compiler test observation, never a user destructor hook. */
    if (getenv("TARN_TRACE_DROPS")) { fputs("drop:", stderr); fwrite(s->bytes, 1, s->len, stderr); fputc('\n', stderr); }
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
