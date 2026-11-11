target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@bmp = global [64 x i8] zeroinitializer, align 1
define void @put32(i32 noundef range(i32 2, 43) %0, i32 noundef %1) unnamed_addr {
  %3 = getelementptr inbounds nuw i8, ptr @bmp, i32 %0
  br label %4

4:                                                ; preds = %2, %4
  %5 = phi i32 [ 0, %2 ], [ %10, %4 ]
  %6 = shl nuw nsw i32 %5, 3
  %7 = lshr i32 %1, %6
  %8 = trunc i32 %7 to i8
  %9 = getelementptr inbounds nuw i8, ptr %3, i32 %5
  store i8 %8, ptr %9, align 1
  %10 = add nuw nsw i32 %5, 1
  %11 = icmp samesign ult i32 %5, 3
  br i1 %11, label %4, label %12

12:                                               ; preds = %4
  ret void
}
