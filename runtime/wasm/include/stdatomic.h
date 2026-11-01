/* No threads: an atomic is its plain type. */
#ifndef LLRM_WASM_STDATOMIC_H
#define LLRM_WASM_STDATOMIC_H
typedef int atomic_int;
enum { memory_order_relaxed, memory_order_acquire, memory_order_release, memory_order_seq_cst };
#define atomic_load(p) (*(p))
#define atomic_store(p, v) (*(p) = (v))
#define atomic_load_explicit(p, order) (*(p))
#define atomic_store_explicit(p, v, order) (*(p) = (v))
#endif
