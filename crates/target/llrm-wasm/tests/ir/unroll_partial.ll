target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@tile = external global [4096 x i16], align 2
define i32 @bench_tile() {
  br label %1

1:                                                ; preds = %0, %18
  %2 = phi i16 [ 0, %0 ], [ %19, %18 ]
  %3 = phi i32 [ 0, %0 ], [ %15, %18 ]
  %4 = add nuw nsw i16 %2, 61
  %5 = and i16 %4, 63
  %6 = zext nneg i16 %5 to i32
  %7 = getelementptr inbounds nuw [128 x i8], ptr @tile, i32 %6
  br label %8

8:                                                ; preds = %1, %8
  %9 = phi i32 [ 0, %1 ], [ %16, %8 ]
  %10 = phi i32 [ %3, %1 ], [ %15, %8 ]
  %11 = getelementptr inbounds nuw [2 x i8], ptr %7, i32 %9
  %12 = getelementptr inbounds nuw i8, ptr %11, i32 14
  %13 = load i16, ptr %12, align 2
  %14 = sext i16 %13 to i32
  %15 = add nsw i32 %10, %14
  %16 = add nuw nsw i32 %9, 1
  %17 = icmp eq i32 %16, 40
  br i1 %17, label %18, label %8

18:                                               ; preds = %8
  %19 = add nuw nsw i16 %2, 1
  %20 = icmp samesign ult i16 %2, 24
  br i1 %20, label %1, label %21

21:                                               ; preds = %18
  ret i32 %15
}
