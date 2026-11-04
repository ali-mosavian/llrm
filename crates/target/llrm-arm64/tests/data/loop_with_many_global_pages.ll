; ModuleID = 'runtime/qb/cntext.c'
source_filename = "runtime/qb/cntext.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

%struct.Driver = type { ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr }

@text_driver = internal constant %struct.Driver { ptr @text_init, ptr @text_write, ptr @text_crlf, ptr @text_erase, ptr @text_sync, ptr @text_cursor, ptr @text_pos, ptr @text_line, ptr @text_width, ptr @text_color, ptr @text_locate_cursor, ptr @text_clear, ptr @text_view, ptr @text_size }, align 8
@cn_text_driver = external global ptr, align 8
@cursor_shown = internal global i32 1, align 4
@columns = internal global i8 0, align 1
@rows = internal global i8 0, align 1
@row = internal global i8 0, align 1
@column = internal global i8 0, align 1
@view_top = internal global i8 0, align 1
@view_bottom = internal global i8 0, align 1
@cursor_stale = internal global i32 0, align 4
@attribute = internal global i8 7, align 1
@cursor_wanted = internal global i32 0, align 4
@view_set = internal global i32 0, align 4
@llvm.global_ctors = appending global [1 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 65535, ptr @xi_run_cn_text_xinit, ptr null }]

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @xi_run_cn_text_xinit() #0 {
  call void @cn_text_xinit()
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define void @cn_text_xinit() #0 {
  store ptr @text_driver, ptr @cn_text_driver, align 8, !tbaa !10
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_init() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr %1) #3
  %3 = call i32 @dev_text_size()
  store i32 %3, ptr %1, align 4, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %2) #3
  %4 = call i32 @dev_text_cursor()
  store i32 %4, ptr %2, align 4, !tbaa !13
  call void @dev_text_cursor_show(i32 noundef 0)
  store i32 0, ptr @cursor_shown, align 4, !tbaa !13
  %5 = load i32, ptr %1, align 4, !tbaa !13
  %6 = trunc i32 %5 to i8
  store i8 %6, ptr @columns, align 1, !tbaa !14
  %7 = load i32, ptr %1, align 4, !tbaa !13
  %8 = lshr i32 %7, 8
  %9 = trunc i32 %8 to i8
  store i8 %9, ptr @rows, align 1, !tbaa !14
  %10 = load i32, ptr %2, align 4, !tbaa !13
  %11 = lshr i32 %10, 8
  %12 = trunc i32 %11 to i8
  store i8 %12, ptr @row, align 1, !tbaa !14
  %13 = load i32, ptr %2, align 4, !tbaa !13
  %14 = trunc i32 %13 to i8
  store i8 %14, ptr @column, align 1, !tbaa !14
  store i8 0, ptr @view_top, align 1, !tbaa !14
  %15 = load i8, ptr @rows, align 1, !tbaa !14
  %16 = zext i8 %15 to i32
  %17 = sub nsw i32 %16, 2
  %18 = trunc i32 %17 to i8
  store i8 %18, ptr @view_bottom, align 1, !tbaa !14
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_write(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !15
  store i32 %1, ptr %4, align 4, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %5) #3
  br label %6

6:                                                ; preds = %21, %2
  %7 = load i32, ptr %4, align 4, !tbaa !13
  %8 = icmp ne i32 %7, 0
  br i1 %8, label %9, label %29

9:                                                ; preds = %6
  %10 = load ptr, ptr %3, align 8, !tbaa !15
  %11 = load i8, ptr %10, align 1, !tbaa !14
  %12 = call i32 @plain_char(i8 noundef signext %11)
  %13 = icmp ne i32 %12, 0
  br i1 %13, label %14, label %18

14:                                               ; preds = %9
  %15 = load ptr, ptr %3, align 8, !tbaa !15
  %16 = load i32, ptr %4, align 4, !tbaa !13
  %17 = call i32 @put_run(ptr noundef %15, i32 noundef %16)
  store i32 %17, ptr %5, align 4, !tbaa !13
  br label %21

18:                                               ; preds = %9
  %19 = load ptr, ptr %3, align 8, !tbaa !15
  %20 = load i8, ptr %19, align 1, !tbaa !14
  call void @control(i8 noundef signext %20)
  store i32 1, ptr %5, align 4, !tbaa !13
  br label %21

21:                                               ; preds = %18, %14
  %22 = load i32, ptr %5, align 4, !tbaa !13
  %23 = load ptr, ptr %3, align 8, !tbaa !15
  %24 = zext i32 %22 to i64
  %25 = getelementptr inbounds nuw i8, ptr %23, i64 %24
  store ptr %25, ptr %3, align 8, !tbaa !15
  %26 = load i32, ptr %5, align 4, !tbaa !13
  %27 = load i32, ptr %4, align 4, !tbaa !13
  %28 = sub i32 %27, %26
  store i32 %28, ptr %4, align 4, !tbaa !13
  br label %6, !llvm.loop !17

29:                                               ; preds = %6
  store i32 1, ptr @cursor_stale, align 4, !tbaa !13
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_crlf() #0 {
  call void @text_newline()
  store i32 1, ptr @cursor_stale, align 4, !tbaa !13
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_erase() #0 {
  %1 = load i8, ptr @column, align 1, !tbaa !14
  %2 = icmp ne i8 %1, 0
  br i1 %2, label %3, label %6

3:                                                ; preds = %0
  %4 = load i8, ptr @column, align 1, !tbaa !14
  %5 = add i8 %4, -1
  store i8 %5, ptr @column, align 1, !tbaa !14
  br label %6

6:                                                ; preds = %3, %0
  %7 = load i8, ptr @row, align 1, !tbaa !14
  %8 = zext i8 %7 to i32
  %9 = load i8, ptr @column, align 1, !tbaa !14
  %10 = zext i8 %9 to i32
  %11 = load i8, ptr @attribute, align 1, !tbaa !14
  %12 = zext i8 %11 to i32
  call void @dev_text_put(i32 noundef %8, i32 noundef %10, i32 noundef 32, i32 noundef %12)
  store i32 1, ptr @cursor_stale, align 4, !tbaa !13
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_sync() #0 {
  %1 = load i32, ptr @cursor_stale, align 4, !tbaa !13
  %2 = icmp ne i32 %1, 0
  br i1 %2, label %3, label %20

3:                                                ; preds = %0
  %4 = load i8, ptr @row, align 1, !tbaa !14
  %5 = zext i8 %4 to i32
  %6 = load i8, ptr @column, align 1, !tbaa !14
  %7 = zext i8 %6 to i32
  %8 = load i8, ptr @columns, align 1, !tbaa !14
  %9 = zext i8 %8 to i32
  %10 = icmp slt i32 %7, %9
  br i1 %10, label %11, label %14

11:                                               ; preds = %3
  %12 = load i8, ptr @column, align 1, !tbaa !14
  %13 = zext i8 %12 to i32
  br label %18

14:                                               ; preds = %3
  %15 = load i8, ptr @columns, align 1, !tbaa !14
  %16 = zext i8 %15 to i32
  %17 = sub nsw i32 %16, 1
  br label %18

18:                                               ; preds = %14, %11
  %19 = phi i32 [ %13, %11 ], [ %17, %14 ]
  call void @dev_text_move(i32 noundef %5, i32 noundef %19)
  store i32 0, ptr @cursor_stale, align 4, !tbaa !13
  br label %20

20:                                               ; preds = %18, %0
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_cursor(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i32, ptr %2, align 4, !tbaa !13
  %5 = icmp ne i32 %4, 0
  br i1 %5, label %9, label %6

6:                                                ; preds = %1
  %7 = load i32, ptr @cursor_wanted, align 4, !tbaa !13
  %8 = icmp ne i32 %7, 0
  br label %9

9:                                                ; preds = %6, %1
  %10 = phi i1 [ true, %1 ], [ %8, %6 ]
  %11 = zext i1 %10 to i32
  store i32 %11, ptr %3, align 4, !tbaa !13
  %12 = load i32, ptr %3, align 4, !tbaa !13
  %13 = load i32, ptr @cursor_shown, align 4, !tbaa !13
  %14 = icmp ne i32 %12, %13
  br i1 %14, label %15, label %18

15:                                               ; preds = %9
  %16 = load i32, ptr %3, align 4, !tbaa !13
  call void @dev_text_cursor_show(i32 noundef %16)
  %17 = load i32, ptr %3, align 4, !tbaa !13
  store i32 %17, ptr @cursor_shown, align 4, !tbaa !13
  br label %18

18:                                               ; preds = %15, %9
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal zeroext i8 @text_pos() #0 {
  %1 = load i8, ptr @column, align 1, !tbaa !14
  ret i8 %1
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal zeroext i8 @text_line() #0 {
  %1 = load i8, ptr @row, align 1, !tbaa !14
  ret i8 %1
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal zeroext i8 @text_width() #0 {
  %1 = load i8, ptr @columns, align 1, !tbaa !14
  ret i8 %1
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_color(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !13
  store i32 %1, ptr %4, align 4, !tbaa !13
  %5 = load i32, ptr %3, align 4, !tbaa !13
  %6 = icmp sgt i32 %5, 31
  br i1 %6, label %10, label %7

7:                                                ; preds = %2
  %8 = load i32, ptr %4, align 4, !tbaa !13
  %9 = icmp sgt i32 %8, 15
  br i1 %9, label %10, label %11

10:                                               ; preds = %7, %2
  call void @qb_error(i32 noundef 5)
  br label %11

11:                                               ; preds = %10, %7
  %12 = load i32, ptr %3, align 4, !tbaa !13
  %13 = icmp sge i32 %12, 0
  br i1 %13, label %14, label %28

14:                                               ; preds = %11
  %15 = load i8, ptr @attribute, align 1, !tbaa !14
  %16 = zext i8 %15 to i32
  %17 = and i32 %16, 112
  %18 = load i32, ptr %3, align 4, !tbaa !13
  %19 = and i32 %18, 15
  %20 = or i32 %17, %19
  %21 = load i32, ptr %3, align 4, !tbaa !13
  %22 = and i32 %21, 16
  %23 = icmp ne i32 %22, 0
  %24 = zext i1 %23 to i64
  %25 = select i1 %23, i32 128, i32 0
  %26 = or i32 %20, %25
  %27 = trunc i32 %26 to i8
  store i8 %27, ptr @attribute, align 1, !tbaa !14
  br label %28

28:                                               ; preds = %14, %11
  %29 = load i32, ptr %4, align 4, !tbaa !13
  %30 = icmp sge i32 %29, 0
  br i1 %30, label %31, label %40

31:                                               ; preds = %28
  %32 = load i8, ptr @attribute, align 1, !tbaa !14
  %33 = zext i8 %32 to i32
  %34 = and i32 %33, 143
  %35 = load i32, ptr %4, align 4, !tbaa !13
  %36 = and i32 %35, 7
  %37 = shl i32 %36, 4
  %38 = or i32 %34, %37
  %39 = trunc i32 %38 to i8
  store i8 %39, ptr @attribute, align 1, !tbaa !14
  br label %40

40:                                               ; preds = %31, %28
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_locate_cursor(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !13
  store i32 %1, ptr %5, align 4, !tbaa !13
  store i32 %2, ptr %6, align 4, !tbaa !13
  %7 = load i32, ptr %4, align 4, !tbaa !13
  %8 = load i32, ptr %5, align 4, !tbaa !13
  call void @text_locate(i32 noundef %7, i32 noundef %8)
  %9 = load i32, ptr %6, align 4, !tbaa !13
  %10 = icmp sge i32 %9, 0
  br i1 %10, label %11, label %15

11:                                               ; preds = %3
  %12 = load i32, ptr %6, align 4, !tbaa !13
  %13 = icmp ne i32 %12, 0
  %14 = zext i1 %13 to i32
  store i32 %14, ptr @cursor_wanted, align 4, !tbaa !13
  call void @text_cursor(i32 noundef 0)
  br label %15

15:                                               ; preds = %11, %3
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_clear() #0 {
  %1 = load i8, ptr @view_top, align 1, !tbaa !14
  %2 = zext i8 %1 to i32
  %3 = load i32, ptr @view_set, align 4, !tbaa !13
  %4 = icmp ne i32 %3, 0
  br i1 %4, label %5, label %8

5:                                                ; preds = %0
  %6 = load i8, ptr @view_bottom, align 1, !tbaa !14
  %7 = zext i8 %6 to i32
  br label %12

8:                                                ; preds = %0
  %9 = load i8, ptr @rows, align 1, !tbaa !14
  %10 = zext i8 %9 to i32
  %11 = sub nsw i32 %10, 1
  br label %12

12:                                               ; preds = %8, %5
  %13 = phi i32 [ %7, %5 ], [ %11, %8 ]
  %14 = load i8, ptr @attribute, align 1, !tbaa !14
  %15 = zext i8 %14 to i32
  call void @dev_text_scroll(i32 noundef %2, i32 noundef %13, i32 noundef 0, i32 noundef %15)
  %16 = load i8, ptr @view_top, align 1, !tbaa !14
  store i8 %16, ptr @row, align 1, !tbaa !14
  store i8 0, ptr @column, align 1, !tbaa !14
  store i32 1, ptr @cursor_stale, align 4, !tbaa !13
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_view(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !13
  store i32 %1, ptr %4, align 4, !tbaa !13
  %5 = load i32, ptr %3, align 4, !tbaa !13
  %6 = icmp ne i32 %5, -1
  br i1 %6, label %10, label %7

7:                                                ; preds = %2
  %8 = load i32, ptr %4, align 4, !tbaa !13
  %9 = icmp ne i32 %8, -1
  br label %10

10:                                               ; preds = %7, %2
  %11 = phi i1 [ true, %2 ], [ %9, %7 ]
  %12 = zext i1 %11 to i32
  store i32 %12, ptr @view_set, align 4, !tbaa !13
  %13 = load i32, ptr @view_set, align 4, !tbaa !13
  %14 = icmp ne i32 %13, 0
  br i1 %14, label %20, label %15

15:                                               ; preds = %10
  store i8 0, ptr @view_top, align 1, !tbaa !14
  %16 = load i8, ptr @rows, align 1, !tbaa !14
  %17 = zext i8 %16 to i32
  %18 = sub nsw i32 %17, 1
  %19 = trunc i32 %18 to i8
  store i8 %19, ptr @view_bottom, align 1, !tbaa !14
  br label %40

20:                                               ; preds = %10
  %21 = load i32, ptr %3, align 4, !tbaa !13
  %22 = icmp slt i32 %21, 1
  br i1 %22, label %32, label %23

23:                                               ; preds = %20
  %24 = load i32, ptr %4, align 4, !tbaa !13
  %25 = load i32, ptr %3, align 4, !tbaa !13
  %26 = icmp slt i32 %24, %25
  br i1 %26, label %32, label %27

27:                                               ; preds = %23
  %28 = load i32, ptr %4, align 4, !tbaa !13
  %29 = load i8, ptr @rows, align 1, !tbaa !14
  %30 = zext i8 %29 to i32
  %31 = icmp sgt i32 %28, %30
  br i1 %31, label %32, label %33

32:                                               ; preds = %27, %23, %20
  call void @qb_error(i32 noundef 5)
  br label %33

33:                                               ; preds = %32, %27
  %34 = load i32, ptr %3, align 4, !tbaa !13
  %35 = sub nsw i32 %34, 1
  %36 = trunc i32 %35 to i8
  store i8 %36, ptr @view_top, align 1, !tbaa !14
  %37 = load i32, ptr %4, align 4, !tbaa !13
  %38 = sub nsw i32 %37, 1
  %39 = trunc i32 %38 to i8
  store i8 %39, ptr @view_bottom, align 1, !tbaa !14
  br label %40

40:                                               ; preds = %33, %15
  %41 = load i8, ptr @view_top, align 1, !tbaa !14
  store i8 %41, ptr @row, align 1, !tbaa !14
  store i8 0, ptr @column, align 1, !tbaa !14
  store i32 1, ptr @cursor_stale, align 4, !tbaa !13
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_size(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !13
  store i32 %1, ptr %4, align 4, !tbaa !13
  %5 = load i32, ptr %3, align 4, !tbaa !13
  %6 = icmp ne i32 %5, 0
  br i1 %6, label %7, label %12

7:                                                ; preds = %2
  %8 = load i32, ptr %3, align 4, !tbaa !13
  %9 = load i8, ptr @columns, align 1, !tbaa !14
  %10 = zext i8 %9 to i32
  %11 = icmp ne i32 %8, %10
  br i1 %11, label %20, label %12

12:                                               ; preds = %7, %2
  %13 = load i32, ptr %4, align 4, !tbaa !13
  %14 = icmp ne i32 %13, 0
  br i1 %14, label %15, label %21

15:                                               ; preds = %12
  %16 = load i32, ptr %4, align 4, !tbaa !13
  %17 = load i8, ptr @rows, align 1, !tbaa !14
  %18 = zext i8 %17 to i32
  %19 = icmp ne i32 %16, %18
  br i1 %19, label %20, label %21

20:                                               ; preds = %15, %7
  call void @qb_error(i32 noundef 5)
  br label %21

21:                                               ; preds = %20, %15, %12
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare i32 @dev_text_size() #2

declare i32 @dev_text_cursor() #2

declare void @dev_text_cursor_show(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @plain_char(i8 noundef signext %0) #0 {
  %2 = alloca i8, align 1
  %3 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i8, ptr %2, align 1, !tbaa !14
  store i8 %4, ptr %3, align 1, !tbaa !14
  %5 = load i8, ptr %3, align 1, !tbaa !14
  %6 = zext i8 %5 to i32
  %7 = icmp eq i32 %6, 7
  br i1 %7, label %26, label %8

8:                                                ; preds = %1
  %9 = load i8, ptr %3, align 1, !tbaa !14
  %10 = zext i8 %9 to i32
  %11 = icmp sge i32 %10, 9
  br i1 %11, label %12, label %16

12:                                               ; preds = %8
  %13 = load i8, ptr %3, align 1, !tbaa !14
  %14 = zext i8 %13 to i32
  %15 = icmp sle i32 %14, 13
  br i1 %15, label %26, label %16

16:                                               ; preds = %12, %8
  %17 = load i8, ptr %3, align 1, !tbaa !14
  %18 = zext i8 %17 to i32
  %19 = icmp sge i32 %18, 28
  br i1 %19, label %20, label %24

20:                                               ; preds = %16
  %21 = load i8, ptr %3, align 1, !tbaa !14
  %22 = zext i8 %21 to i32
  %23 = icmp sle i32 %22, 31
  br label %24

24:                                               ; preds = %20, %16
  %25 = phi i1 [ false, %16 ], [ %23, %20 ]
  br label %26

26:                                               ; preds = %24, %12, %1
  %27 = phi i1 [ true, %12 ], [ true, %1 ], [ %25, %24 ]
  %28 = xor i1 %27, true
  %29 = zext i1 %28 to i32
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %29
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @put_run(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !15
  store i32 %1, ptr %4, align 4, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 4, !tbaa !13
  %6 = load i8, ptr @column, align 1, !tbaa !14
  %7 = zext i8 %6 to i32
  %8 = load i8, ptr @columns, align 1, !tbaa !14
  %9 = zext i8 %8 to i32
  %10 = icmp eq i32 %7, %9
  br i1 %10, label %11, label %12

11:                                               ; preds = %2
  call void @text_newline()
  br label %12

12:                                               ; preds = %11, %2
  br label %13

13:                                               ; preds = %35, %12
  %14 = load i32, ptr %5, align 4, !tbaa !13
  %15 = load i32, ptr %4, align 4, !tbaa !13
  %16 = icmp ult i32 %14, %15
  br i1 %16, label %17, label %33

17:                                               ; preds = %13
  %18 = load ptr, ptr %3, align 8, !tbaa !15
  %19 = load i32, ptr %5, align 4, !tbaa !13
  %20 = zext i32 %19 to i64
  %21 = getelementptr inbounds nuw i8, ptr %18, i64 %20
  %22 = load i8, ptr %21, align 1, !tbaa !14
  %23 = call i32 @plain_char(i8 noundef signext %22)
  %24 = icmp ne i32 %23, 0
  br i1 %24, label %25, label %33

25:                                               ; preds = %17
  %26 = load i8, ptr @column, align 1, !tbaa !14
  %27 = zext i8 %26 to i32
  %28 = load i32, ptr %5, align 4, !tbaa !13
  %29 = add i32 %27, %28
  %30 = load i8, ptr @columns, align 1, !tbaa !14
  %31 = zext i8 %30 to i32
  %32 = icmp ult i32 %29, %31
  br label %33

33:                                               ; preds = %25, %17, %13
  %34 = phi i1 [ false, %17 ], [ false, %13 ], [ %32, %25 ]
  br i1 %34, label %35, label %38

35:                                               ; preds = %33
  %36 = load i32, ptr %5, align 4, !tbaa !13
  %37 = add i32 %36, 1
  store i32 %37, ptr %5, align 4, !tbaa !13
  br label %13, !llvm.loop !19

38:                                               ; preds = %33
  %39 = load i8, ptr @row, align 1, !tbaa !14
  %40 = zext i8 %39 to i32
  %41 = load i8, ptr @column, align 1, !tbaa !14
  %42 = zext i8 %41 to i32
  %43 = load ptr, ptr %3, align 8, !tbaa !15
  %44 = load i32, ptr %5, align 4, !tbaa !13
  %45 = load i8, ptr @attribute, align 1, !tbaa !14
  %46 = zext i8 %45 to i32
  call void @dev_text_write(i32 noundef %40, i32 noundef %42, ptr noundef %43, i32 noundef %44, i32 noundef %46)
  %47 = load i32, ptr %5, align 4, !tbaa !13
  %48 = load i8, ptr @column, align 1, !tbaa !14
  %49 = zext i8 %48 to i32
  %50 = add i32 %49, %47
  %51 = trunc i32 %50 to i8
  store i8 %51, ptr @column, align 1, !tbaa !14
  %52 = load i32, ptr %5, align 4, !tbaa !13
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %52
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @control(i8 noundef signext %0) #0 {
  %2 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !14
  %3 = load i8, ptr %2, align 1, !tbaa !14
  %4 = sext i8 %3 to i32
  switch i32 %4, label %76 [
    i32 13, label %5
    i32 10, label %5
    i32 12, label %6
    i32 9, label %7
    i32 11, label %29
    i32 28, label %31
    i32 29, label %42
    i32 30, label %49
    i32 31, label %65
    i32 7, label %75
  ]

5:                                                ; preds = %1, %1
  call void @text_newline()
  br label %77

6:                                                ; preds = %1
  call void @text_clear()
  br label %77

7:                                                ; preds = %1
  br label %8

8:                                                ; preds = %23, %7
  %9 = load i8, ptr @column, align 1, !tbaa !14
  %10 = zext i8 %9 to i32
  %11 = load i8, ptr @columns, align 1, !tbaa !14
  %12 = zext i8 %11 to i32
  %13 = icmp eq i32 %10, %12
  br i1 %13, label %14, label %15

14:                                               ; preds = %8
  call void @text_newline()
  br label %15

15:                                               ; preds = %14, %8
  %16 = load i8, ptr @row, align 1, !tbaa !14
  %17 = zext i8 %16 to i32
  %18 = load i8, ptr @column, align 1, !tbaa !14
  %19 = add i8 %18, 1
  store i8 %19, ptr @column, align 1, !tbaa !14
  %20 = zext i8 %18 to i32
  %21 = load i8, ptr @attribute, align 1, !tbaa !14
  %22 = zext i8 %21 to i32
  call void @dev_text_put(i32 noundef %17, i32 noundef %20, i32 noundef 32, i32 noundef %22)
  br label %23

23:                                               ; preds = %15
  %24 = load i8, ptr @column, align 1, !tbaa !14
  %25 = zext i8 %24 to i32
  %26 = srem i32 %25, 8
  %27 = icmp ne i32 %26, 0
  br i1 %27, label %8, label %28, !llvm.loop !20

28:                                               ; preds = %23
  br label %77

29:                                               ; preds = %1
  %30 = load i8, ptr @view_top, align 1, !tbaa !14
  store i8 %30, ptr @row, align 1, !tbaa !14
  store i8 0, ptr @column, align 1, !tbaa !14
  br label %77

31:                                               ; preds = %1
  %32 = load i8, ptr @column, align 1, !tbaa !14
  %33 = zext i8 %32 to i32
  %34 = load i8, ptr @columns, align 1, !tbaa !14
  %35 = zext i8 %34 to i32
  %36 = sub nsw i32 %35, 1
  %37 = icmp slt i32 %33, %36
  br i1 %37, label %38, label %41

38:                                               ; preds = %31
  %39 = load i8, ptr @column, align 1, !tbaa !14
  %40 = add i8 %39, 1
  store i8 %40, ptr @column, align 1, !tbaa !14
  br label %41

41:                                               ; preds = %38, %31
  br label %77

42:                                               ; preds = %1
  %43 = load i8, ptr @column, align 1, !tbaa !14
  %44 = icmp ne i8 %43, 0
  br i1 %44, label %45, label %48

45:                                               ; preds = %42
  %46 = load i8, ptr @column, align 1, !tbaa !14
  %47 = add i8 %46, -1
  store i8 %47, ptr @column, align 1, !tbaa !14
  br label %48

48:                                               ; preds = %45, %42
  br label %77

49:                                               ; preds = %1
  %50 = load i8, ptr @row, align 1, !tbaa !14
  %51 = zext i8 %50 to i32
  %52 = load i8, ptr @view_top, align 1, !tbaa !14
  %53 = zext i8 %52 to i32
  %54 = icmp sgt i32 %51, %53
  br i1 %54, label %55, label %64

55:                                               ; preds = %49
  %56 = load i8, ptr @row, align 1, !tbaa !14
  %57 = zext i8 %56 to i32
  %58 = load i8, ptr @view_bottom, align 1, !tbaa !14
  %59 = zext i8 %58 to i32
  %60 = icmp sle i32 %57, %59
  br i1 %60, label %61, label %64

61:                                               ; preds = %55
  %62 = load i8, ptr @row, align 1, !tbaa !14
  %63 = add i8 %62, -1
  store i8 %63, ptr @row, align 1, !tbaa !14
  br label %64

64:                                               ; preds = %61, %55, %49
  br label %77

65:                                               ; preds = %1
  %66 = load i8, ptr @row, align 1, !tbaa !14
  %67 = zext i8 %66 to i32
  %68 = load i8, ptr @view_bottom, align 1, !tbaa !14
  %69 = zext i8 %68 to i32
  %70 = icmp slt i32 %67, %69
  br i1 %70, label %71, label %74

71:                                               ; preds = %65
  %72 = load i8, ptr @row, align 1, !tbaa !14
  %73 = add i8 %72, 1
  store i8 %73, ptr @row, align 1, !tbaa !14
  br label %74

74:                                               ; preds = %71, %65
  br label %77

75:                                               ; preds = %1
  call void @cn_bell()
  br label %77

76:                                               ; preds = %1
  br label %77

77:                                               ; preds = %76, %75, %74, %64, %48, %41, %29, %28, %6, %5
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_newline() #0 {
  store i8 0, ptr @column, align 1, !tbaa !14
  %1 = load i8, ptr @row, align 1, !tbaa !14
  %2 = zext i8 %1 to i32
  %3 = load i8, ptr @view_bottom, align 1, !tbaa !14
  %4 = zext i8 %3 to i32
  %5 = icmp slt i32 %2, %4
  br i1 %5, label %6, label %9

6:                                                ; preds = %0
  %7 = load i8, ptr @row, align 1, !tbaa !14
  %8 = add i8 %7, 1
  store i8 %8, ptr @row, align 1, !tbaa !14
  br label %17

9:                                                ; preds = %0
  %10 = load i8, ptr @view_top, align 1, !tbaa !14
  %11 = zext i8 %10 to i32
  %12 = load i8, ptr @view_bottom, align 1, !tbaa !14
  %13 = zext i8 %12 to i32
  %14 = load i8, ptr @attribute, align 1, !tbaa !14
  %15 = zext i8 %14 to i32
  call void @dev_text_scroll(i32 noundef %11, i32 noundef %13, i32 noundef 1, i32 noundef %15)
  %16 = load i8, ptr @view_bottom, align 1, !tbaa !14
  store i8 %16, ptr @row, align 1, !tbaa !14
  br label %17

17:                                               ; preds = %9, %6
  ret void
}

declare void @dev_text_write(i32 noundef, i32 noundef, ptr noundef, i32 noundef, i32 noundef) #2

declare void @dev_text_scroll(i32 noundef, i32 noundef, i32 noundef, i32 noundef) #2

declare void @dev_text_put(i32 noundef, i32 noundef, i32 noundef, i32 noundef) #2

declare void @cn_bell() #2

declare void @dev_text_move(i32 noundef, i32 noundef) #2

declare void @qb_error(i32 noundef) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @text_locate(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !13
  store i32 %1, ptr %4, align 4, !tbaa !13
  %5 = load i32, ptr %3, align 4, !tbaa !13
  %6 = icmp eq i32 %5, 0
  br i1 %6, label %20, label %7

7:                                                ; preds = %2
  %8 = load i32, ptr %3, align 4, !tbaa !13
  %9 = load i8, ptr @rows, align 1, !tbaa !14
  %10 = zext i8 %9 to i32
  %11 = icmp sgt i32 %8, %10
  br i1 %11, label %20, label %12

12:                                               ; preds = %7
  %13 = load i32, ptr %4, align 4, !tbaa !13
  %14 = icmp eq i32 %13, 0
  br i1 %14, label %20, label %15

15:                                               ; preds = %12
  %16 = load i32, ptr %4, align 4, !tbaa !13
  %17 = load i8, ptr @columns, align 1, !tbaa !14
  %18 = zext i8 %17 to i32
  %19 = icmp sgt i32 %16, %18
  br i1 %19, label %20, label %21

20:                                               ; preds = %15, %12, %7, %2
  call void @qb_error(i32 noundef 5)
  br label %21

21:                                               ; preds = %20, %15
  %22 = load i32, ptr %3, align 4, !tbaa !13
  %23 = icmp sgt i32 %22, 0
  br i1 %23, label %24, label %28

24:                                               ; preds = %21
  %25 = load i32, ptr %3, align 4, !tbaa !13
  %26 = sub nsw i32 %25, 1
  %27 = trunc i32 %26 to i8
  store i8 %27, ptr @row, align 1, !tbaa !14
  br label %28

28:                                               ; preds = %24, %21
  %29 = load i32, ptr %4, align 4, !tbaa !13
  %30 = icmp sgt i32 %29, 0
  br i1 %30, label %31, label %35

31:                                               ; preds = %28
  %32 = load i32, ptr %4, align 4, !tbaa !13
  %33 = sub nsw i32 %32, 1
  %34 = trunc i32 %33 to i8
  store i8 %34, ptr @column, align 1, !tbaa !14
  br label %35

35:                                               ; preds = %31, %28
  store i32 1, ptr @cursor_stale, align 4, !tbaa !13
  ret void
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3}
!llvm.ident = !{!4}
!llvm.errno.tbaa = !{!5}

!0 = !{i32 2, !"SDK Version", [2 x i32] [i32 27, i32 0]}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"uwtable", i32 1}
!3 = !{i32 7, !"frame-pointer", i32 4}
!4 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!5 = !{!6, !7, i64 0}
!6 = !{!"__libc_errno", !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = !{!11, !11, i64 0}
!11 = !{!"p1 _ZTS6Driver", !12, i64 0}
!12 = !{!"any pointer", !8, i64 0}
!13 = !{!7, !7, i64 0}
!14 = !{!8, !8, i64 0}
!15 = !{!16, !16, i64 0}
!16 = !{!"p1 omnipotent char", !12, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
!19 = distinct !{!19, !18}
!20 = distinct !{!20, !18}
