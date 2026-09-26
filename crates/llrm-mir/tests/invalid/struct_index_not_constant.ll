; invalid: indexes a struct with other than a constant i32
define ptr @f(ptr %p, i32 %i) {
  %q = getelementptr { i16, i16 }, ptr %p, i32 0, i32 %i
  ret ptr %q
}
