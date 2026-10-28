; invalid: Store cannot have Acquire ordering
target datalayout = "e-p:16:16-i32:16-i64:16"

define void @f(ptr %p) {
entry:
  store atomic i32 0, ptr %p acquire, align 4
  ret void
}
