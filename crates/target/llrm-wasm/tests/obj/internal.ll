define internal i32 @helper(i32 %x) {
  %y = mul i32 %x, 3
  ret i32 %y
}
define i32 @entry(i32 %a) {
  %r = call i32 @helper(i32 %a)
  ret i32 %r
}
