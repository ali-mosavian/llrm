target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"
define i32 @ld32(ptr %p) { %v = load i32, ptr %p
 ret i32 %v }
define i64 @ld64o(ptr %p) { %q = getelementptr i64, ptr %p, i64 3
 %v = load i64, ptr %q
 ret i64 %v }
define i64 @ldu(ptr %p) { %q = getelementptr i8, ptr %p, i64 -9
 %v = load i64, ptr %q
 ret i64 %v }
define i32 @ldb(ptr %p) { %v = load i8, ptr %p
 %z = zext i8 %v to i32
 ret i32 %z }
define i32 @ldsb(ptr %p) { %v = load i8, ptr %p
 %z = sext i8 %v to i32
 ret i32 %z }
define i64 @ldsw(ptr %p) { %v = load i32, ptr %p
 %z = sext i32 %v to i64
 ret i64 %z }
define void @st32(ptr %p, i32 %v) { store i32 %v, ptr %p
 ret void }
define void @sth(ptr %p, i32 %v) { %t = trunc i32 %v to i16
 store i16 %t, ptr %p
 ret void }
define i64 @ldx(ptr %p, i64 %i) { %q = getelementptr i64, ptr %p, i64 %i
 %v = load i64, ptr %q
 ret i64 %v }
define i32 @slot(i32 %a) { %s = alloca i32
 store i32 %a, ptr %s
 %v = load volatile i32, ptr %s
 ret i32 %v }
define i32 @sh(i32 %a, i32 %b) { %s = shl i32 %b, 2
 %r = add i32 %a, %s
 ret i32 %r }
