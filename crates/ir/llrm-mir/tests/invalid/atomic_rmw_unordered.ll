; invalid: atomicrmw instructions cannot be unordered.
target datalayout = "e-p:16:16-i32:16-i64:16"

define i32 @f(ptr %p) {
entry:
  %a = atomicrmw add ptr %p, i32 1 unordered
  ret i32 %a
}
