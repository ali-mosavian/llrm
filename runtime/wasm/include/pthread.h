/* One thread: a mutex is a flag nothing contends, and a thread cannot start. */
#ifndef LLRM_WASM_PTHREAD_H
#define LLRM_WASM_PTHREAD_H
typedef int pthread_mutex_t;
typedef int pthread_t;
#define PTHREAD_MUTEX_INITIALIZER 0

static inline int pthread_mutex_lock(pthread_mutex_t *m) { *m = 1; return 0; }
static inline int pthread_mutex_trylock(pthread_mutex_t *m) { if (*m) return 16; *m = 1; return 0; }
static inline int pthread_mutex_unlock(pthread_mutex_t *m) { *m = 0; return 0; }
static inline int pthread_create(pthread_t *t, const void *attr, void *(*run)(void *), void *arg)
{
    (void)t; (void)attr; (void)run; (void)arg;
    return 11;
}
static inline int pthread_join(pthread_t t, void **result) { (void)t; (void)result; return 0; }
#endif
