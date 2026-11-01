define i32 @shifts(i32 %a, i32 %b) { %x = shl i32 %a, %b
 %y = lshr i32 %x, %b
 %z = ashr i32 %y, 3
 ret i32 %z }
define i32 @shl(i32 %a, i32 %b) { %x = shl i32 %a, %b
 ret i32 %x }
define i32 @shr_s(i32 %a, i32 %b) { %x = ashr i32 %a, %b
 ret i32 %x }
define i32 @shr_u(i32 %a, i32 %b) { %x = lshr i32 %a, %b
 ret i32 %x }
define i64 @shl64(i64 %a, i64 %b) { %x = shl i64 %a, %b
 ret i64 %x }
define i64 @shr_s64(i64 %a) { %x = ashr i64 %a, 7
 ret i64 %x }
