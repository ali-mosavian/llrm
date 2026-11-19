target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@array = global [4 x i32] [i32 10, i32 -20, i32 300, i32 -4000]
@word = global i16 -2
@msg = private constant [6 x i8] c"hello\00"
@table = global { ptr, i32, ptr } { ptr @array, i32 7, ptr getelementptr inbounds ([4 x i32], ptr @array, i32 0, i32 2) }
@counter = global i32 0
@zeros = internal global [40 x i8] zeroinitializer

define i32 @sum() {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %j, %loop ]
  %s = phi i32 [ 0, %entry ], [ %t, %loop ]
  %p = getelementptr [4 x i32], ptr @array, i32 0, i32 %i
  %v = load i32, ptr %p
  %t = add i32 %s, %v
  %j = add i32 %i, 1
  %c = icmp ult i32 %j, 4
  br i1 %c, label %loop, label %out
out:
  ret i32 %t
}

define i32 @letter(i32 %i) {
  %p = getelementptr [6 x i8], ptr @msg, i32 0, i32 %i
  %v = load i8, ptr %p
  %z = zext i8 %v to i32
  ret i32 %z
}

define i32 @word_value() {
  %v = load i16, ptr @word
  %s = sext i16 %v to i32
  ret i32 %s
}

define i32 @through_table() {
  %pa = load ptr, ptr @table
  %a = load i32, ptr %pa
  %f = getelementptr { ptr, i32, ptr }, ptr @table, i32 0, i32 1
  %n = load i32, ptr %f
  %pc = getelementptr { ptr, i32, ptr }, ptr @table, i32 0, i32 2
  %pp = load ptr, ptr %pc
  %c = load i32, ptr %pp
  %x = add i32 %a, %n
  %y = add i32 %x, %c
  ret i32 %y
}

define i32 @bump() {
  %v = load i32, ptr @counter
  %w = add i32 %v, 5
  store i32 %w, ptr @counter
  %z = load i8, ptr @zeros
  %zz = zext i8 %z to i32
  %r = add i32 %w, %zz
  ret i32 %r
}
