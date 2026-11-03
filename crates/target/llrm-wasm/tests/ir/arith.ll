define i32 @add(i32 %a, i32 %b) { %r = add i32 %a, %b
 ret i32 %r }
define i32 @sub(i32 %a, i32 %b) { %r = sub i32 %a, %b
 ret i32 %r }
define i32 @mul(i32 %a, i32 %b) { %r = mul i32 %a, %b
 ret i32 %r }
define i32 @sdiv(i32 %a, i32 %b) { %r = sdiv i32 %a, %b
 ret i32 %r }
define i32 @udiv(i32 %a, i32 %b) { %r = udiv i32 %a, %b
 ret i32 %r }
define i32 @srem(i32 %a, i32 %b) { %r = srem i32 %a, %b
 ret i32 %r }
define i32 @urem(i32 %a, i32 %b) { %r = urem i32 %a, %b
 ret i32 %r }
define i64 @add64(i64 %a, i64 %b) { %r = add i64 %a, %b
 ret i64 %r }
define i64 @mul64(i64 %a, i64 %b) { %r = mul i64 %a, %b
 ret i64 %r }
define i32 @consts(i32 %a) { %x = add i32 %a, 1000
 %y = sub i32 %x, -7
 ret i32 %y }
define i64 @big(i64 %a) { %x = add i64 %a, 81985529216486895
 ret i64 %x }
define i32 @trunc(i64 %a) { %x = trunc i64 %a to i32
 ret i32 %x }
define i64 @ext(i32 %a, i32 %b) { %x = zext i32 %a to i64
 %y = sext i32 %b to i64
 %z = add i64 %x, %y
 ret i64 %z }
