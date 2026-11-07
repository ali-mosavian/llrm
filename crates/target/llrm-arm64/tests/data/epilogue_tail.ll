target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: nofree noinline nosync nounwind memory(none)
define dso_local i64 @bench_rectwo(i32 noundef %0) local_unnamed_addr {
  %2 = tail call fastcc i64 @paths(i32 noundef %0, i32 noundef %0)
  ret i64 %2
}

; Function Attrs: nofree nosync nounwind memory(none)
define internal fastcc i64 @paths(i32 noundef %0, i32 noundef %1) unnamed_addr {
  %3 = icmp eq i32 %0, 0
  %4 = icmp eq i32 %1, 0
  %5 = or i1 %3, %4
  br i1 %5, label %17, label %6

6:                                                ; preds = %2
  %7 = add nsw i32 %0, -1
  br label %8

8:                                                ; preds = %6, %8
  %9 = phi i32 [ %1, %6 ], [ %12, %8 ]
  %10 = phi i64 [ 0, %6 ], [ %13, %8 ]
  %11 = tail call fastcc i64 @paths(i32 noundef %7, i32 noundef %9)
  %12 = add nsw i32 %9, -1
  %13 = add i64 %11, %10
  %14 = icmp eq i32 %12, 0
  br i1 %14, label %15, label %8

15:                                               ; preds = %8
  %16 = add i64 %13, 1
  br label %17

17:                                               ; preds = %15, %2
  %18 = phi i64 [ 1, %2 ], [ %16, %15 ]
  ret i64 %18
}

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr {
  %1 = load volatile i32, ptr @opq0, align 4
  %2 = add nsw i32 %1, 9
  %3 = tail call i64 @bench_rectwo(i32 noundef %2)
  tail call void @report(i64 noundef %3)
  ret i32 0
}

declare void @report(i64 noundef) local_unnamed_addr



