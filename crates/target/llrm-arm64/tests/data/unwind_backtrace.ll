declare i32 @_Unwind_Backtrace(ptr, ptr)
define i32 @cb(ptr %c, ptr %p) {
  %v = load i32, ptr %p
  %n = add i32 %v, 1
  store i32 %n, ptr %p
  ret i32 0
}
define i32 @leaf(ptr %n) uwtable noinline {
  %buf = alloca [40 x i8], align 8
  store volatile i8 1, ptr %buf
  %r = call i32 @_Unwind_Backtrace(ptr @cb, ptr %n)
  %v = load volatile i8, ptr %buf
  %z = zext i8 %v to i32
  %s = add i32 %r, %z
  ret i32 %s
}
define i32 @main() uwtable {
  %n = alloca i32, align 8
  %pad = alloca [24 x i8], align 8
  store volatile i8 2, ptr %pad
  store i32 0, ptr %n
  %r = call i32 @leaf(ptr %n)
  %c = load i32, ptr %n
  %ok = icmp sge i32 %c, 3
  %code = select i1 %ok, i32 0, i32 %r
  ret i32 %code
}
