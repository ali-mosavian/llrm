int w;
int fetch_add(int v) { return __atomic_fetch_add(&w, v, __ATOMIC_RELAXED); }
void add_only(int v) { __atomic_fetch_add(&w, v, __ATOMIC_SEQ_CST); }
int fetch_sub(int v) { return __atomic_fetch_sub(&w, v, __ATOMIC_SEQ_CST); }
void or_only(int v) { __atomic_fetch_or(&w, v, __ATOMIC_SEQ_CST); }
int cas(int expected, int desired) {
    return __atomic_compare_exchange_n(&w, &expected, desired, 0, __ATOMIC_SEQ_CST, __ATOMIC_RELAXED);
}
long long wq;
long long swap64(long long v) { return __atomic_exchange_n(&wq, v, __ATOMIC_SEQ_CST); }
unsigned long long presses[2];
unsigned long long take(unsigned char b) { return __atomic_exchange_n(&presses[b], 0, __ATOMIC_RELAXED); }
