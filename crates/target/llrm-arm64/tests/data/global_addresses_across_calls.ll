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

declare void @use()

define void @f() {
  call void @use()
  store volatile i32 1, ptr @g0, align 4
  call void @use()
  store volatile i32 2, ptr @g1, align 4
  call void @use()
  store volatile i32 3, ptr @g2, align 4
  call void @use()
  store volatile i32 4, ptr @g3, align 4
  call void @use()
  store volatile i32 5, ptr @g4, align 4
  call void @use()
  store volatile i32 6, ptr @g5, align 4
  call void @use()
  store volatile i32 7, ptr @g6, align 4
  call void @use()
  store volatile i32 8, ptr @g7, align 4
  call void @use()
  store volatile i32 9, ptr @g8, align 4
  call void @use()
  store volatile i32 10, ptr @g9, align 4
  call void @use()
  store volatile i32 11, ptr @g10, align 4
  call void @use()
  store volatile i32 12, ptr @g11, align 4
  call void @use()
  store volatile i32 13, ptr @g12, align 4
  call void @use()
  store volatile i32 14, ptr @g13, align 4
  call void @use()
  store volatile i32 15, ptr @g0, align 4
  call void @use()
  store volatile i32 16, ptr @g1, align 4
  call void @use()
  store volatile i32 17, ptr @g2, align 4
  call void @use()
  store volatile i32 18, ptr @g3, align 4
  call void @use()
  store volatile i32 19, ptr @g4, align 4
  call void @use()
  store volatile i32 20, ptr @g5, align 4
  call void @use()
  store volatile i32 21, ptr @g6, align 4
  call void @use()
  store volatile i32 22, ptr @g7, align 4
  call void @use()
  store volatile i32 23, ptr @g8, align 4
  call void @use()
  store volatile i32 24, ptr @g9, align 4
  call void @use()
  store volatile i32 25, ptr @g10, align 4
  call void @use()
  store volatile i32 26, ptr @g11, align 4
  call void @use()
  store volatile i32 27, ptr @g12, align 4
  call void @use()
  store volatile i32 28, ptr @g13, align 4
  ret void
}
