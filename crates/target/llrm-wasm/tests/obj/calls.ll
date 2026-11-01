declare i32 @get()
declare void @put(i32)
define i32 @f(i32 %a) {
  %v = call i32 @get()
  %s = add i32 %v, %a
  call void @put(i32 %s)
  ret i32 %s
}
