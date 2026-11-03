declare void @put(i32)
define void @first(i32 %a) {
  call void @second(i32 %a)
  call void @put(i32 %a)
  ret void
}
define void @second(i32 %a) {
  call void @first(i32 %a)
  ret void
}
