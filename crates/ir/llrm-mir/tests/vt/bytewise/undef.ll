define void @f(ptr %p) {
  store i8 undef, ptr %p
  store i16 undef, ptr %p
  store i32 undef, ptr %p
  store i64 undef, ptr %p
  store ptr undef, ptr %p
  ret void
}
