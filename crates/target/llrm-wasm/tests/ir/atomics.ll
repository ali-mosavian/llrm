@shown = global ptr null
define ptr @ld() {
  %1 = load atomic ptr, ptr @shown seq_cst, align 4
  ret ptr %1
}
define void @st(ptr %p) {
  store atomic ptr %p, ptr @shown release, align 4
  fence seq_cst
  ret void
}
define i32 @ldv(ptr %p) {
  %1 = load atomic volatile i32, ptr %p acquire, align 4
  ret i32 %1
}
