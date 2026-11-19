target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@z1 = global [3 x i32] zeroinitializer
@z2 = global [3 x i32] zeroinitializer
@zl = global i64 0
@ints = global [4 x i16] [i16 1, i16 -2, i16 300, i16 -4000]
@longs = global [2 x i64] [i64 -2, i64 72623859790382856]
@bytes = global [5 x i8] c"\01\02\03\04\FF"
@.str = private unnamed_addr constant [4 x i8] c"abc\00"
@.str.1 = private unnamed_addr constant [3 x i8] c"xy\00"
@ptrs = global [3 x ptr] [ptr @ints, ptr getelementptr inbounds ([4 x i16], ptr @ints, i32 0, i32 3), ptr @.str.1]
@fptrs = global [2 x ptr] [ptr @one, ptr @two]
@pad = global { i8, i32, i8, i16, i8 } { i8 1, i32 2, i8 3, i16 4, i8 5 }
@f.count = internal global i32 100
@f.names = internal constant [2 x i8] c"AB"

define i32 @one() {
  ret i32 1
}

define i32 @two() {
  ret i32 2
}

define i32 @touch_bss(i32 %one) {
  %a = getelementptr [3 x i32], ptr @z1, i32 0, i32 %one
  store i32 7, ptr %a
  store i32 9, ptr @z2
  ret i32 0
}

define i32 @abc_length() {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %j, %loop ]
  %p = getelementptr [4 x i8], ptr @.str, i32 0, i32 %i
  %v = load i8, ptr %p
  %j = add i32 %i, 1
  %c = icmp ne i8 %v, 0
  br i1 %c, label %loop, label %out
out:
  ret i32 %i
}

define i32 @padsum() {
  %a = load i8, ptr @pad
  %pb = getelementptr { i8, i32, i8, i16, i8 }, ptr @pad, i32 0, i32 1
  %b = load i32, ptr %pb
  %pc = getelementptr { i8, i32, i8, i16, i8 }, ptr @pad, i32 0, i32 2
  %c = load i8, ptr %pc
  %pd = getelementptr { i8, i32, i8, i16, i8 }, ptr @pad, i32 0, i32 3
  %d = load i16, ptr %pd
  %pe = getelementptr { i8, i32, i8, i16, i8 }, ptr @pad, i32 0, i32 4
  %e = load i8, ptr %pe
  %a32 = zext i8 %a to i32
  %c32 = zext i8 %c to i32
  %d32 = zext i16 %d to i32
  %e32 = zext i8 %e to i32
  %s1 = add i32 %a32, %b
  %s2 = add i32 %s1, %c32
  %s3 = add i32 %s2, %d32
  %s4 = add i32 %s3, %e32
  ret i32 %s4
}

define i32 @f() {
  %v = load i32, ptr @f.count
  %w = add i32 %v, 1
  store i32 %w, ptr @f.count
  ret i32 %w
}

define i32 @name(i32 %i) {
  %p = getelementptr [2 x i8], ptr @f.names, i32 0, i32 %i
  %v = load i8, ptr %p
  %z = zext i8 %v to i32
  ret i32 %z
}
