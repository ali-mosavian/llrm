; invalid: Store cannot have Acquire ordering
define void @f(ptr %p) {
  store atomic i32 1, ptr %p acquire, align 4
  ret void
}
