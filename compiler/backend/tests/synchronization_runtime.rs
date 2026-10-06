//! Private runtime contention gate. Barriers coordinate the test only and are
//! not Tarn APIs; no timing assumption establishes the expected final values.
use std::process::Command;

#[test]
fn pthread_mutex_and_seq_cst_atomics_survive_simultaneous_workers() {
    let dir = std::env::temp_dir().join(format!("tarn-sync-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("stress.c");
    let test = r#"#include <assert.h>
static pthread_barrier_t barrier;
static TarnMutex *mutex;
static int64_t payload;
static TarnAtomici64 *atomic_counter;
static void *worker(void *unused) {
    (void)unused;
    int rc = pthread_barrier_wait(&barrier);
    assert(rc == 0 || rc == PTHREAD_BARRIER_SERIAL_THREAD);
    for (unsigned i = 0; i < 20000; i++) {
        tarn_rt_mutex_lock(mutex);
        payload++;
        tarn_rt_mutex_unlock(mutex);
        tarn_rt_atomic_i64_fetch_add(atomic_counter, 1);
    }
    return NULL;
}
int main(void) {
    pthread_t threads[8];
    mutex = tarn_rt_mutex_create();
    atomic_counter = tarn_rt_atomic_i64_new(0);
    assert(!pthread_barrier_init(&barrier, NULL, 8));
    for (unsigned i = 0; i < 8; i++) assert(!pthread_create(&threads[i], NULL, worker, NULL));
    for (unsigned i = 0; i < 8; i++) assert(!pthread_join(threads[i], NULL));
    assert(payload == 160000);
    assert(tarn_rt_atomic_i64_load(atomic_counter) == 160000);
    assert(!pthread_barrier_destroy(&barrier));
    tarn_rt_mutex_destroy(mutex);
    tarn_rt_atomic_destroy(atomic_counter);
    /* Checked signed arithmetic must accept the endpoints without ever negating
     * INT64_MIN or depending on signed C wraparound. */
    TarnAtomici64 *a = tarn_rt_atomic_i64_new(0);
    assert(tarn_rt_atomic_i64_fetch_add(a, INT64_MIN) == 0);
    assert(tarn_rt_atomic_i64_fetch_sub(a, INT64_MIN) == INT64_MIN);
    assert(tarn_rt_atomic_i64_load(a) == 0);
    assert(tarn_rt_atomic_i64_fetch_sub(a, -1) == 0);
    assert(tarn_rt_atomic_i64_fetch_add(a, -1) == 1);
    tarn_rt_atomic_destroy(a);
    return 0;
}
"#;
    std::fs::write(&source, format!("{}\n{test}", include_str!("../../../runtime/native.c"))).unwrap();
    let exe = dir.join("stress");
    let compilation = Command::new("cc").args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread", "-fsanitize=undefined", "-fno-sanitize-recover=all"])
        .arg(&source).arg("-lm").arg("-o").arg(&exe).output().unwrap();
    assert!(compilation.status.success(), "{}", String::from_utf8_lossy(&compilation.stderr));
    let out = Command::new("timeout").arg("30s").arg(&exe).env_remove("TARN_TRACE_DROPS").env_remove("TARN_TRACE_SYNC").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
