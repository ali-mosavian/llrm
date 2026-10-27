; invalid: Load cannot have Release ordering
define i32 @f(ptr %p) {
  %v = load atomic i32, ptr %p release, align 4
  ret i32 %v
}
