declare cc1002 i32 @g(i32)

define i32 @f(i32 %x) {
entry:
  %r = call cc1002 i32 @g(i32 %x)
  ret i32 %r
}
