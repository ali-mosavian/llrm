declare i32 @get()
declare void @put(i32)
declare i64 @wide(i64, i32)
define void @f(i32 %a) {
  call void @put(i32 %a)
  %w = call i64 @wide(i64 1, i32 %a)
  %v = call i32 @get()
  call void @put(i32 %v)
  ret void
}
