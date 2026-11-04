target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@bitcnt = internal global i32 0, align 4
@bitbuf = internal global i32 0, align 4
@bad = internal global i32 0, align 4
@inpos = internal global i32 0, align 4
@inlen = internal global i32 0, align 4
@infile = internal global i32 0, align 4
@inbuf = internal global [512 x i8] zeroinitializer, align 16

define hidden i32 @getbit() {
  %1 = load i32, ptr @bitcnt, align 4
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %3, label %12

3:                                                ; preds = %0
  %4 = call i32 @next()
  store i32 %4, ptr @bitbuf, align 4
  %5 = load i32, ptr @bitbuf, align 4
  %6 = icmp eq i32 %5, 255
  br i1 %6, label %7, label %11

7:                                                ; preds = %3
  %8 = call i32 @next()
  %9 = icmp ne i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %7
  store i32 1, ptr @bad, align 4
  br label %11

11:                                               ; preds = %10, %7, %3
  store i32 8, ptr @bitcnt, align 4
  br label %12

12:                                               ; preds = %11, %0
  %13 = load i32, ptr @bitcnt, align 4
  %14 = add nsw i32 %13, -1
  store i32 %14, ptr @bitcnt, align 4
  %15 = load i32, ptr @bitbuf, align 4
  %16 = load i32, ptr @bitcnt, align 4
  %17 = ashr i32 %15, %16
  %18 = and i32 %17, 1
  ret i32 %18
}

define internal i32 @next() {
  %1 = alloca i32, align 4
  %2 = load i32, ptr @inpos, align 4
  %3 = load i32, ptr @inlen, align 4
  %4 = icmp eq i32 %2, %3
  br i1 %4, label %5, label %12

5:                                                ; preds = %0
  %6 = load i32, ptr @infile, align 4
  %7 = call i32 @bench_read(i32 noundef %6, ptr noundef @inbuf, i32 noundef 512)
  store i32 %7, ptr @inlen, align 4
  store i32 0, ptr @inpos, align 4
  %8 = load i32, ptr @inlen, align 4
  %9 = icmp sle i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %5
  store i32 0, ptr @inlen, align 4
  store i32 1, ptr @bad, align 4
  store i32 0, ptr %1, align 4
  br label %18

11:                                               ; preds = %5
  br label %12

12:                                               ; preds = %11, %0
  %13 = load i32, ptr @inpos, align 4
  %14 = add nsw i32 %13, 1
  store i32 %14, ptr @inpos, align 4
  %15 = getelementptr inbounds [512 x i8], ptr @inbuf, i32 0, i32 %13
  %16 = load i8, ptr %15, align 1
  %17 = zext i8 %16 to i32
  store i32 %17, ptr %1, align 4
  br label %18

18:                                               ; preds = %12, %10
  %19 = load i32, ptr %1, align 4
  ret i32 %19
}

declare i32 @bench_read(i32 noundef, ptr noundef, i32 noundef)



