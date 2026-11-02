%struct.P = type { i32, i32 }
%struct.Q = type { i8, i16, i32, i64 }
@g1 = global i32 0, align 4
@garr = global [8 x i32] zeroinitializer, align 16
@gp = global %struct.P zeroinitializer, align 4
declare void @take(ptr noundef)
declare i32 @get()
declare void @put(i32 noundef)
declare void @takep(ptr noundef, i32 noundef)

define i32 @f1(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %4 = load i32, ptr %2, align 4
  store i32 %4, ptr %3, align 4
  %5 = load i32, ptr %3, align 4
  %6 = add nsw i32 %5, 1
  ret i32 %6
}

define i32 @f2(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca [4 x i32], align 16
  store i32 %0, ptr %2, align 4
  %4 = load i32, ptr %2, align 4
  %5 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 0
  store i32 %4, ptr %5, align 16
  %6 = call i32 @get()
  %7 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 1
  store i32 %6, ptr %7, align 4
  %8 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 0
  %9 = load i32, ptr %8, align 16
  %10 = getelementptr inbounds [4 x i32], ptr %3, i32 0, i32 1
  %11 = load i32, ptr %10, align 4
  %12 = add nsw i32 %9, %11
  ret i32 %12
}

define i32 @addr_taken(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %4 = load i32, ptr %2, align 4
  store i32 %4, ptr %3, align 4
  call void @take(ptr noundef %3)
  %5 = load i32, ptr %3, align 4
  ret i32 %5
}

define i32 @arr_local(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca [10 x i32], align 16
  %4 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 0, ptr %4, align 4
  br label %5

5:                                                ; preds = %14, %1
  %6 = load i32, ptr %4, align 4
  %7 = icmp slt i32 %6, 10
  br i1 %7, label %8, label %17

8:                                                ; preds = %5
  %9 = load i32, ptr %4, align 4
  %10 = load i32, ptr %2, align 4
  %11 = mul nsw i32 %9, %10
  %12 = load i32, ptr %4, align 4
  %13 = getelementptr inbounds [10 x i32], ptr %3, i32 0, i32 %12
  store i32 %11, ptr %13, align 4
  br label %14

14:                                               ; preds = %8
  %15 = load i32, ptr %4, align 4
  %16 = add nsw i32 %15, 1
  store i32 %16, ptr %4, align 4
  br label %5

17:                                               ; preds = %5
  %18 = getelementptr inbounds [10 x i32], ptr %3, i32 0, i32 3
  %19 = load i32, ptr %18, align 4
  %20 = getelementptr inbounds [10 x i32], ptr %3, i32 0, i32 9
  %21 = load i32, ptr %20, align 4
  %22 = add nsw i32 %19, %21
  ret i32 %22
}

define i32 @char_local(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i8, align 1
  %4 = alloca i16, align 2
  store i32 %0, ptr %2, align 4
  %5 = load i32, ptr %2, align 4
  %6 = trunc i32 %5 to i8
  store i8 %6, ptr %3, align 1
  %7 = load i32, ptr %2, align 4
  %8 = add nsw i32 %7, 1
  %9 = trunc i32 %8 to i16
  store i16 %9, ptr %4, align 2
  %10 = load i8, ptr %3, align 1
  %11 = sext i8 %10 to i32
  %12 = load i16, ptr %4, align 2
  %13 = sext i16 %12 to i32
  %14 = add nsw i32 %11, %13
  ret i32 %14
}

define i64 @ll_local(i64 noundef %0) {
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  store i64 %0, ptr %2, align 8
  %4 = load i64, ptr %2, align 8
  %5 = mul nsw i64 %4, 3
  store i64 %5, ptr %3, align 8
  %6 = load i64, ptr %3, align 8
  %7 = add nsw i64 %6, 1
  ret i64 %7
}

define i32 @struct_local(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca %struct.P, align 4
  store i32 %0, ptr %2, align 4
  %4 = load i32, ptr %2, align 4
  %5 = getelementptr inbounds nuw %struct.P, ptr %3, i32 0, i32 0
  store i32 %4, ptr %5, align 4
  %6 = call i32 @get()
  %7 = getelementptr inbounds nuw %struct.P, ptr %3, i32 0, i32 1
  store i32 %6, ptr %7, align 4
  %8 = getelementptr inbounds nuw %struct.P, ptr %3, i32 0, i32 0
  %9 = load i32, ptr %8, align 4
  %10 = getelementptr inbounds nuw %struct.P, ptr %3, i32 0, i32 1
  %11 = load i32, ptr %10, align 4
  %12 = add nsw i32 %9, %11
  ret i32 %12
}

define i32 @struct_q(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca %struct.Q, align 8
  store i32 %0, ptr %2, align 4
  %4 = load i32, ptr %2, align 4
  %5 = trunc i32 %4 to i8
  %6 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 0
  store i8 %5, ptr %6, align 8
  %7 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 1
  store i16 2, ptr %7, align 2
  %8 = load i32, ptr %2, align 4
  %9 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 2
  store i32 %8, ptr %9, align 4
  %10 = load i32, ptr %2, align 4
  %11 = sext i32 %10 to i64
  %12 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 3
  store i64 %11, ptr %12, align 8
  %13 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 0
  %14 = load i8, ptr %13, align 8
  %15 = sext i8 %14 to i32
  %16 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 1
  %17 = load i16, ptr %16, align 2
  %18 = sext i16 %17 to i32
  %19 = add nsw i32 %15, %18
  %20 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 2
  %21 = load i32, ptr %20, align 4
  %22 = add nsw i32 %19, %21
  %23 = getelementptr inbounds nuw %struct.Q, ptr %3, i32 0, i32 3
  %24 = load i64, ptr %23, align 8
  %25 = trunc i64 %24 to i32
  %26 = add nsw i32 %22, %25
  ret i32 %26
}

define i32 @nested_calls(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %5 = call i32 @get()
  store i32 %5, ptr %3, align 4
  %6 = call i32 @get()
  store i32 %6, ptr %4, align 4
  %7 = load i32, ptr %3, align 4
  %8 = load i32, ptr %4, align 4
  %9 = add nsw i32 %7, %8
  %10 = load i32, ptr %2, align 4
  %11 = add nsw i32 %9, %10
  call void @put(i32 noundef %11)
  %12 = load i32, ptr %3, align 4
  %13 = load i32, ptr %4, align 4
  %14 = sub nsw i32 %12, %13
  ret i32 %14
}

define i32 @globals(i32 noundef %0) {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %3 = load i32, ptr %2, align 4
  store i32 %3, ptr @g1, align 4
  %4 = load i32, ptr @g1, align 4
  %5 = add nsw i32 %4, 1
  store i32 %5, ptr getelementptr inbounds ([8 x i32], ptr @garr, i32 0, i32 3), align 4
  %6 = load i32, ptr getelementptr inbounds ([8 x i32], ptr @garr, i32 0, i32 3), align 4
  store i32 %6, ptr getelementptr inbounds nuw (%struct.P, ptr @gp, i32 0, i32 1), align 4
  %7 = load i32, ptr getelementptr inbounds nuw (%struct.P, ptr @gp, i32 0, i32 1), align 4
  %8 = load i32, ptr @g1, align 4
  %9 = add nsw i32 %7, %8
  ret i32 %9
}

define i32 @ptr_arith(ptr noundef %0, i32 noundef %1) {
  %3 = alloca ptr, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store ptr %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  store i32 0, ptr %5, align 4
  store i32 0, ptr %6, align 4
  br label %7

7:                                                ; preds = %18, %2
  %8 = load i32, ptr %6, align 4
  %9 = load i32, ptr %4, align 4
  %10 = icmp slt i32 %8, %9
  br i1 %10, label %11, label %21

11:                                               ; preds = %7
  %12 = load ptr, ptr %3, align 4
  %13 = load i32, ptr %6, align 4
  %14 = getelementptr inbounds i32, ptr %12, i32 %13
  %15 = load i32, ptr %14, align 4
  %16 = load i32, ptr %5, align 4
  %17 = add nsw i32 %16, %15
  store i32 %17, ptr %5, align 4
  br label %18

18:                                               ; preds = %11
  %19 = load i32, ptr %6, align 4
  %20 = add nsw i32 %19, 1
  store i32 %20, ptr %6, align 4
  br label %7

21:                                               ; preds = %7
  %22 = load i32, ptr %5, align 4
  ret i32 %22
}

define void @bytes(ptr noundef %0, i32 noundef %1) {
  %3 = alloca ptr, align 4
  %4 = alloca i32, align 4
  %5 = alloca [16 x i8], align 16
  %6 = alloca i32, align 4
  store ptr %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  store i32 0, ptr %6, align 4
  br label %7

7:                                                ; preds = %15, %2
  %8 = load i32, ptr %6, align 4
  %9 = icmp slt i32 %8, 16
  br i1 %9, label %10, label %18

10:                                               ; preds = %7
  %11 = load i32, ptr %6, align 4
  %12 = trunc i32 %11 to i8
  %13 = load i32, ptr %6, align 4
  %14 = getelementptr inbounds [16 x i8], ptr %5, i32 0, i32 %13
  store i8 %12, ptr %14, align 1
  br label %15

15:                                               ; preds = %10
  %16 = load i32, ptr %6, align 4
  %17 = add nsw i32 %16, 1
  store i32 %17, ptr %6, align 4
  br label %7

18:                                               ; preds = %7
  %19 = getelementptr inbounds [16 x i8], ptr %5, i32 0, i32 0
  %20 = load i32, ptr %4, align 4
  call void @takep(ptr noundef %19, i32 noundef %20)
  ret void
}

define i32 @cond_locals(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %6 = load i32, ptr %2, align 4
  %7 = icmp ne i32 %6, 0
  br i1 %7, label %8, label %11

8:                                                ; preds = %1
  %9 = call i32 @get()
  store i32 %9, ptr %4, align 4
  %10 = load i32, ptr %4, align 4
  store i32 %10, ptr %3, align 4
  br label %15

11:                                               ; preds = %1
  %12 = load i32, ptr %2, align 4
  %13 = add nsw i32 %12, 1
  store i32 %13, ptr %5, align 4
  %14 = load i32, ptr %5, align 4
  store i32 %14, ptr %3, align 4
  br label %15

15:                                               ; preds = %11, %8
  %16 = load i32, ptr %3, align 4
  ret i32 %16
}

define i32 @loop_locals(i32 noundef %0) {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 0, ptr %3, align 4
  store i32 0, ptr %4, align 4
  br label %6

6:                                                ; preds = %16, %1
  %7 = load i32, ptr %4, align 4
  %8 = load i32, ptr %2, align 4
  %9 = icmp slt i32 %7, %8
  br i1 %9, label %10, label %19

10:                                               ; preds = %6
  %11 = load i32, ptr %4, align 4
  %12 = mul nsw i32 %11, 2
  store i32 %12, ptr %5, align 4
  %13 = load i32, ptr %5, align 4
  %14 = load i32, ptr %3, align 4
  %15 = add nsw i32 %14, %13
  store i32 %15, ptr %3, align 4
  br label %16

16:                                               ; preds = %10
  %17 = load i32, ptr %4, align 4
  %18 = add nsw i32 %17, 1
  store i32 %18, ptr %4, align 4
  br label %6

19:                                               ; preds = %6
  %20 = load i32, ptr %3, align 4
  ret i32 %20
}
