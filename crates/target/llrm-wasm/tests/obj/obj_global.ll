@g = global i32 5
@h = global i64 7
@arr = global [4 x i32] zeroinitializer
define i32 @rg() { %v = load i32, ptr @g
 ret i32 %v }
define void @wg(i32 %x) { store i32 %x, ptr @g
 ret void }
define i64 @rh() { %v = load i64, ptr @h
 ret i64 %v }
define ptr @ag() { ret ptr @g }
define i32 @ra2() { %q = getelementptr inbounds [4 x i32], ptr @arr, i32 0, i32 2
 %v = load i32, ptr %q
 ret i32 %v }
define i32 @rai(i32 %i) { %q = getelementptr inbounds [4 x i32], ptr @arr, i32 0, i32 %i
 %v = load i32, ptr %q
 ret i32 %v }
