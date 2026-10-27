; invalid: atomic load must have explicit non-zero alignment
define i32 @f(ptr %p) {
  %v = load atomic i32, ptr %p seq_cst
  ret i32 %v
}
