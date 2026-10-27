; invalid: cmpxchg failure ordering cannot include release semantics
define void @f(ptr %p) {
  %r = cmpxchg ptr %p, i32 0, i32 1 seq_cst release, align 4
  ret void
}
