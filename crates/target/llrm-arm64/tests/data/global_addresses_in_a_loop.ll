target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@g0 = global i32 0, align 4
@g1 = global i32 0, align 4
@g2 = global i32 0, align 4
@g3 = global i32 0, align 4
@g4 = global i32 0, align 4
@g5 = global i32 0, align 4
@g6 = global i32 0, align 4
@g7 = global i32 0, align 4
@g8 = global i32 0, align 4
@g9 = global i32 0, align 4
@g10 = global i32 0, align 4
@g11 = global i32 0, align 4
@g12 = global i32 0, align 4
@g13 = global i32 0, align 4
@g14 = global i32 0, align 4
@g15 = global i32 0, align 4
@g16 = global i32 0, align 4
@g17 = global i32 0, align 4
@g18 = global i32 0, align 4
@g19 = global i32 0, align 4

declare void @use(i32)

define void @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  call void @use(i32 %i)
  store volatile i32 %i, ptr @g0, align 4
  store volatile i32 %i, ptr @g1, align 4
  store volatile i32 %i, ptr @g2, align 4
  store volatile i32 %i, ptr @g3, align 4
  store volatile i32 %i, ptr @g4, align 4
  store volatile i32 %i, ptr @g5, align 4
  store volatile i32 %i, ptr @g6, align 4
  store volatile i32 %i, ptr @g7, align 4
  store volatile i32 %i, ptr @g8, align 4
  store volatile i32 %i, ptr @g9, align 4
  store volatile i32 %i, ptr @g10, align 4
  store volatile i32 %i, ptr @g11, align 4
  store volatile i32 %i, ptr @g12, align 4
  store volatile i32 %i, ptr @g13, align 4
  store volatile i32 %i, ptr @g14, align 4
  store volatile i32 %i, ptr @g15, align 4
  store volatile i32 %i, ptr @g16, align 4
  store volatile i32 %i, ptr @g17, align 4
  store volatile i32 %i, ptr @g18, align 4
  store volatile i32 %i, ptr @g19, align 4
  %next = add i32 %i, 1
  %again = icmp ult i32 %next, %n
  br i1 %again, label %loop, label %done
done:
  ret void
}
