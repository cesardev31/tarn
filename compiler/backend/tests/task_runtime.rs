//! Runtime ABI tests, separate from the pending frontend Task<R> integration.
use std::process::Command;

#[test]
fn pthread_tasks_wait_transfer_and_destroy_without_sleeps() {
    let dir = std::env::temp_dir().join(format!("tarn-task-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("test.c");
    let test = r#"
#include <assert.h>
static pthread_t parent;
static unsigned discarded;
static void scalar_worker(void *result, void *code, void *env) {
    (void)code; (void)env;
    assert(!pthread_equal(parent, pthread_self()));
    *(int32_t *)result = 42;
}
static void scalar_drop(void *result) {
    assert(*(int32_t *)result == 42);
    discarded++;
}
static void string_worker(void *result, void *code, void *env) {
    (void)code;
    *(TarnString **)result = env; /* Transfer owned result into task storage. */
}
static void string_drop(void *result) { tarn_rt_drop_string(*(TarnString **)result); }
int main(void) {
    parent = pthread_self();
    for (unsigned i = 0; i < 256; i++) {
        TarnTask *a = tarn_rt_task_spawn(scalar_worker, scalar_drop, 4, NULL, NULL);
        TarnTask *b = tarn_rt_task_spawn(scalar_worker, scalar_drop, 4, NULL, NULL);
        assert(*(int32_t *)tarn_rt_task_wait(a) == 42);
        tarn_rt_task_release(a);
        tarn_rt_task_drop(b);
    }
    assert(discarded == 256);
    TarnString *owned = tarn_rt_string((const unsigned char *)"hello", 5);
    TarnTask *task = tarn_rt_task_spawn(string_worker, string_drop, 8, NULL, owned);
    TarnString *moved = *(TarnString **)tarn_rt_task_wait(task);
    tarn_rt_task_release(task);
    assert(moved->len == 5 && !memcmp(moved->bytes, "hello", 5));
    tarn_rt_drop_string(moved);
    tarn_rt_task_drop(tarn_rt_task_spawn(string_worker, string_drop, 8, NULL,
        tarn_rt_string((const unsigned char *)"unused", 6)));
    return 0;
}
"#;
    std::fs::write(&source, format!("{}\n{test}", include_str!("../../../runtime/native.c"))).unwrap();
    let exe = dir.join("test");
    let compilation = Command::new("cc").args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread"])
        .arg(&source).arg("-lm").arg("-o").arg(&exe).output().unwrap();
    assert!(compilation.status.success(), "{}", String::from_utf8_lossy(&compilation.stderr));
    let output = Command::new(&exe).env_remove("TARN_TRACE_DROPS").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_task_runtime_faults_abort_instead_of_detaching_or_hanging() {
    use std::os::unix::process::ExitStatusExt;
    let dir = std::env::temp_dir().join(format!("tarn-task-faults-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("faults.c");
    let test = r#"
#include <errno.h>
static int failure;
void *__real_calloc(size_t, size_t);
int __real_pthread_create(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
int __real_pthread_cond_init(pthread_cond_t *, const pthread_condattr_t *);
void *__wrap_calloc(size_t n, size_t size) {
    return failure == 1 ? NULL : __real_calloc(n, size);
}
int __wrap_pthread_create(pthread_t *t, const pthread_attr_t *a, void *(*fn)(void *), void *p) {
    return failure == 2 ? EAGAIN : __real_pthread_create(t, a, fn, p);
}
int __wrap_pthread_cond_init(pthread_cond_t *c, const pthread_condattr_t *a) {
    return failure == 3 ? ENOMEM : __real_pthread_cond_init(c, a);
}
static void worker(void *out, void *code, void *env) { (void)out; (void)code; (void)env; }
static void destroy(void *out) { (void)out; }
int main(int argc, char **argv) {
    if (argc != 2) return 1;
    failure = atoi(argv[1]);
    if (failure == 4) {
        /* Waiting twice for one task (a double join) is a runtime fault. */
        TarnTask *twice = tarn_rt_task_spawn(worker, destroy, 0, NULL, NULL);
        tarn_rt_task_wait(twice);
        tarn_rt_task_wait(twice);
    }
    TarnTask *task = tarn_rt_task_spawn(worker, destroy, 0, NULL, NULL);
    tarn_rt_task_drop(task);
    return 1; /* Every injected failure must terminate, never return success. */
}
"#;
    std::fs::write(&source, format!("{}\n{test}", include_str!("../../../runtime/native.c"))).unwrap();
    let exe = dir.join("faults");
    let compilation = Command::new("cc").args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread",
        "-Wl,--wrap=calloc", "-Wl,--wrap=pthread_create", "-Wl,--wrap=pthread_cond_init"])
        .arg(&source).arg("-lm").arg("-o").arg(&exe).output().unwrap();
    assert!(compilation.status.success(), "{}", String::from_utf8_lossy(&compilation.stderr));
    for failure in 1..=4 {
        let output = Command::new(&exe).arg(failure.to_string()).output().unwrap();
        assert_eq!(output.status.signal(), Some(6), "fault {failure}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("native task runtime fault"));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
