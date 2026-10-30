; invalid: Load cannot have Release ordering
target datalayout = "e-p:16:16-i32:16-i64:16"

define i32 @f(ptr %p) {
entry:
  %a = load atomic i32, ptr %p release, align 4
  ret i32 %a
}
