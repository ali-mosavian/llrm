declare void @v0()
declare i32 @i0()
declare i32 @i2(i32, i32)
declare i64 @l1(i64)
declare void @vp(ptr, i32)
define void @callv() { call void @v0()
 ret void }
define i32 @calli() { %r = call i32 @i0()
 ret i32 %r }
define i32 @call2(i32 %a, i32 %b) { %r = call i32 @i2(i32 %b, i32 %a)
 ret i32 %r }
define i64 @call64(i64 %a) { %r = call i64 @l1(i64 %a)
 ret i64 %r }
define i32 @twice(i32 %a) { %x = call i32 @i2(i32 %a, i32 1)
 %y = call i32 @i2(i32 %x, i32 %a)
 ret i32 %y }
define void @callp(ptr %p) { call void @vp(ptr %p, i32 42)
 ret void }
define i32 @tail(i32 %a) { %x = call i32 @i2(i32 %a, i32 %a)
 %y = add i32 %x, 1
 ret i32 %y }
