target datalayout = "@m64-datalayout@"
target triple = "x86_64-unknown-linux-gnu"

@back = dso_local global [256 x i8] zeroinitializer, align 16

define dso_local void @f() nounwind uwtable {
  %1 = tail call i32 @op()
  %2 = tail call i32 @rd(i32 noundef %1, ptr noundef @back, i64 noundef 200)
  %3 = icmp sgt i32 %2, 0
  br i1 %3, label %4, label %15

4:
  %5 = phi i32 [ %13, %4 ], [ %2, %0 ]
  %6 = zext nneg i32 %5 to i64
  %7 = mul nuw nsw i64 %6, 1000
  %8 = getelementptr i8, ptr @back, i64 %6
  %9 = getelementptr i8, ptr %8, i64 -1
  %10 = load i8, ptr %9, align 1
  %11 = sext i8 %10 to i64
  %12 = add nsw i64 %7, %11
  tail call void @report(i64 noundef %12)
  %13 = tail call i32 @rd(i32 noundef %1, ptr noundef @back, i64 noundef 200)
  %14 = icmp sgt i32 %13, 0
  br i1 %14, label %4, label %15

15:
  %16 = phi i32 [ %2, %0 ], [ %13, %4 ]
  %17 = sext i32 %16 to i64
  tail call void @report(i64 noundef %17)
  ret void
}

declare i32 @op()
declare i32 @rd(i32 noundef, ptr noundef, i64 noundef)
declare void @report(i64 noundef)
