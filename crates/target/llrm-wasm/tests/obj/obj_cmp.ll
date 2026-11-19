define i32 @eq(i32 %a, i32 %b) { %c = icmp eq i32 %a, %b
 %r = zext i1 %c to i32
 ret i32 %r }
define i32 @slt(i32 %a, i32 %b) { %c = icmp slt i32 %a, %b
 %r = zext i1 %c to i32
 ret i32 %r }
define i32 @ule(i32 %a, i32 %b) { %c = icmp ule i32 %a, %b
 %r = zext i1 %c to i32
 ret i32 %r }
define i32 @sel(i32 %a, i32 %b, i32 %x, i32 %y) { %c = icmp ugt i32 %a, %b
 %r = select i1 %c, i32 %x, i32 %y
 ret i32 %r }
define i32 @eq64(i64 %a, i64 %b) { %c = icmp ne i64 %a, %b
 %r = zext i1 %c to i32
 ret i32 %r }
define i32 @isz(i32 %a) { %c = icmp eq i32 %a, 0
 %r = zext i1 %c to i32
 ret i32 %r }
