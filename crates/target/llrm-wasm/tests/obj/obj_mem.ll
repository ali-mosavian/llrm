define i32 @l8s(ptr %p) { %v = load i8, ptr %p
 %r = sext i8 %v to i32
 ret i32 %r }
define i32 @l8u(ptr %p) { %v = load i8, ptr %p
 %r = zext i8 %v to i32
 ret i32 %r }
define i32 @l16s(ptr %p) { %v = load i16, ptr %p
 %r = sext i16 %v to i32
 ret i32 %r }
define i32 @l16u(ptr %p) { %v = load i16, ptr %p
 %r = zext i16 %v to i32
 ret i32 %r }
define i64 @l64(ptr %p) { %v = load i64, ptr %p
 ret i64 %v }
define i64 @l32s64(ptr %p) { %v = load i32, ptr %p
 %r = sext i32 %v to i64
 ret i64 %r }
define void @s8(ptr %p, i32 %x) { %t = trunc i32 %x to i8
 store i8 %t, ptr %p
 ret void }
define void @s16(ptr %p, i32 %x) { %t = trunc i32 %x to i16
 store i16 %t, ptr %p
 ret void }
define void @s64(ptr %p, i64 %x) { store i64 %x, ptr %p
 ret void }
define i32 @off(ptr %p) { %q = getelementptr inbounds i8, ptr %p, i32 12
 %v = load i32, ptr %q
 ret i32 %v }
define i32 @idx(ptr %p, i32 %i) { %q = getelementptr inbounds i32, ptr %p, i32 %i
 %v = load i32, ptr %q
 ret i32 %v }
define i32 @unaligned(ptr %p) { %v = load i32, ptr %p, align 1
 ret i32 %v }
define i32 @volat(ptr %p) { %v = load volatile i32, ptr %p
 ret i32 %v }
