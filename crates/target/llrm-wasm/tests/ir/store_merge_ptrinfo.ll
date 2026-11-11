target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@a = dso_local global [64 x i16] zeroinitializer, align 16
@b = dso_local global [64 x i32] zeroinitializer, align 16
; pattern_fill's zeroing loop, unrolled by 4: i16 stores of @a through an index,
; i32 stores of @b through a loop-carried pointer, interleaved.
define void @f() #0 {
  br label %1
1:
  %p = phi ptr [ @b, %0 ], [ %p4, %1 ]
  %i = phi i32 [ -128, %0 ], [ %i4, %1 ]
  %a = getelementptr i8, ptr @a, i32 %i
  %a0 = getelementptr i8, ptr %a, i32 128
  store i16 0, ptr %a0, align 8
  store i32 0, ptr %p, align 16
  %a1 = getelementptr i8, ptr %a, i32 130
  store i16 0, ptr %a1, align 2
  %p1 = getelementptr i8, ptr %p, i32 4
  store i32 0, ptr %p1, align 4
  %a2 = getelementptr i8, ptr %a, i32 132
  store i16 0, ptr %a2, align 4
  %p2 = getelementptr i8, ptr %p, i32 8
  store i32 0, ptr %p2, align 8
  %a3 = getelementptr i8, ptr %a, i32 134
  store i16 0, ptr %a3, align 2
  %p3 = getelementptr i8, ptr %p, i32 12
  store i32 0, ptr %p3, align 4
  %i4 = add nsw i32 %i, 8
  %p4 = getelementptr i8, ptr %p, i32 16
  %c = icmp eq i32 %i4, 0
  br i1 %c, label %2, label %1
2:
  ret void
}
attributes #0 = { "no-builtins" }
