source_filename = "vla.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [11 x i8] c"%d %d %ld\0A\00", align 1
@.str.1 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @vla(i32 noundef 5)
  %3 = call i32 @vla(i32 noundef 50)
  %4 = call i64 @nested(i32 noundef 6)
  %5 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %2, i32 noundef %3, i64 noundef %4)
  %6 = call i32 @many(i32 noundef 3, i64 noundef 1, i64 noundef 2, i64 noundef 3, i64 noundef 4, i64 noundef 5, i64 noundef 6, i64 noundef 7, i64 noundef 8, i64 noundef 9, i64 noundef 10)
  %7 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i32 noundef %6)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @vla(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 8
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  %8 = load i32, ptr %2, align 4, !tbaa !9
  %9 = zext i32 %8 to i64
  %10 = call ptr @llvm.stacksave.p0()
  store ptr %10, ptr %3, align 8
  %11 = alloca i32, i64 %9, align 4
  store i64 %9, ptr %4, align 8
  store i32 0, ptr %5, align 4, !tbaa !9
  br label %12

12:                                               ; preds = %23, %1
  %13 = load i32, ptr %5, align 4, !tbaa !9
  %14 = load i32, ptr %2, align 4, !tbaa !9
  %15 = icmp slt i32 %13, %14
  br i1 %15, label %16, label %26

16:                                               ; preds = %12
  %17 = load i32, ptr %5, align 4, !tbaa !9
  %18 = load i32, ptr %5, align 4, !tbaa !9
  %19 = mul nsw i32 %17, %18
  %20 = load i32, ptr %5, align 4, !tbaa !9
  %21 = sext i32 %20 to i64
  %22 = getelementptr inbounds i32, ptr %11, i64 %21
  store i32 %19, ptr %22, align 4, !tbaa !9
  br label %23

23:                                               ; preds = %16
  %24 = load i32, ptr %5, align 4, !tbaa !9
  %25 = add nsw i32 %24, 1
  store i32 %25, ptr %5, align 4, !tbaa !9
  br label %12, !llvm.loop !10

26:                                               ; preds = %12
  store i32 0, ptr %6, align 4, !tbaa !9
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %27

27:                                               ; preds = %38, %26
  %28 = load i32, ptr %7, align 4, !tbaa !9
  %29 = load i32, ptr %2, align 4, !tbaa !9
  %30 = icmp slt i32 %28, %29
  br i1 %30, label %31, label %41

31:                                               ; preds = %27
  %32 = load i32, ptr %7, align 4, !tbaa !9
  %33 = sext i32 %32 to i64
  %34 = getelementptr inbounds i32, ptr %11, i64 %33
  %35 = load i32, ptr %34, align 4, !tbaa !9
  %36 = load i32, ptr %6, align 4, !tbaa !9
  %37 = add nsw i32 %36, %35
  store i32 %37, ptr %6, align 4, !tbaa !9
  br label %38

38:                                               ; preds = %31
  %39 = load i32, ptr %7, align 4, !tbaa !9
  %40 = add nsw i32 %39, 1
  store i32 %40, ptr %7, align 4, !tbaa !9
  br label %27, !llvm.loop !12

41:                                               ; preds = %27
  %42 = load i32, ptr %6, align 4, !tbaa !9
  %43 = load ptr, ptr %3, align 8
  call void @llvm.stackrestore.p0(ptr %43)
  ret i32 %42
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @nested(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i64, align 8
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 8
  %6 = alloca i64, align 8
  %7 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  store i64 0, ptr %3, align 8, !tbaa !13
  store i32 1, ptr %4, align 4, !tbaa !9
  br label %8

8:                                                ; preds = %49, %1
  %9 = load i32, ptr %4, align 4, !tbaa !9
  %10 = load i32, ptr %2, align 4, !tbaa !9
  %11 = icmp sle i32 %9, %10
  br i1 %11, label %12, label %52

12:                                               ; preds = %8
  %13 = load i32, ptr %4, align 4, !tbaa !9
  %14 = mul nsw i32 %13, 3
  %15 = zext i32 %14 to i64
  %16 = call ptr @llvm.stacksave.p0()
  store ptr %16, ptr %5, align 8
  %17 = alloca i8, i64 %15, align 1
  store i64 %15, ptr %6, align 8
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %18

18:                                               ; preds = %31, %12
  %19 = load i32, ptr %7, align 4, !tbaa !9
  %20 = load i32, ptr %4, align 4, !tbaa !9
  %21 = mul nsw i32 %20, 3
  %22 = icmp slt i32 %19, %21
  br i1 %22, label %23, label %34

23:                                               ; preds = %18
  %24 = load i32, ptr %7, align 4, !tbaa !9
  %25 = load i32, ptr %4, align 4, !tbaa !9
  %26 = add nsw i32 %24, %25
  %27 = trunc i32 %26 to i8
  %28 = load i32, ptr %7, align 4, !tbaa !9
  %29 = sext i32 %28 to i64
  %30 = getelementptr inbounds i8, ptr %17, i64 %29
  store i8 %27, ptr %30, align 1, !tbaa !15
  br label %31

31:                                               ; preds = %23
  %32 = load i32, ptr %7, align 4, !tbaa !9
  %33 = add nsw i32 %32, 1
  store i32 %33, ptr %7, align 4, !tbaa !9
  br label %18, !llvm.loop !16

34:                                               ; preds = %18
  %35 = load i32, ptr %4, align 4, !tbaa !9
  %36 = mul nsw i32 %35, 3
  %37 = sub nsw i32 %36, 1
  %38 = sext i32 %37 to i64
  %39 = getelementptr inbounds i8, ptr %17, i64 %38
  %40 = load i8, ptr %39, align 1, !tbaa !15
  %41 = sext i8 %40 to i32
  %42 = load i32, ptr %4, align 4, !tbaa !9
  %43 = call i32 @vla(i32 noundef %42)
  %44 = add nsw i32 %41, %43
  %45 = sext i32 %44 to i64
  %46 = load i64, ptr %3, align 8, !tbaa !13
  %47 = add nsw i64 %46, %45
  store i64 %47, ptr %3, align 8, !tbaa !13
  %48 = load ptr, ptr %5, align 8
  call void @llvm.stackrestore.p0(ptr %48)
  br label %49

49:                                               ; preds = %34
  %50 = load i32, ptr %4, align 4, !tbaa !9
  %51 = add nsw i32 %50, 1
  store i32 %51, ptr %4, align 4, !tbaa !9
  br label %8, !llvm.loop !17

52:                                               ; preds = %8
  %53 = load i64, ptr %3, align 8, !tbaa !13
  ret i64 %53
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @many(i32 noundef %0, i64 noundef %1, i64 noundef %2, i64 noundef %3, i64 noundef %4, i64 noundef %5, i64 noundef %6, i64 noundef %7, i64 noundef %8, i64 noundef %9, i64 noundef %10) #0 {
  %12 = alloca i32, align 4
  %13 = alloca i64, align 8
  %14 = alloca i64, align 8
  %15 = alloca i64, align 8
  %16 = alloca i64, align 8
  %17 = alloca i64, align 8
  %18 = alloca i64, align 8
  %19 = alloca i64, align 8
  %20 = alloca i64, align 8
  %21 = alloca i64, align 8
  %22 = alloca i64, align 8
  %23 = alloca ptr, align 8
  %24 = alloca i64, align 8
  store i32 %0, ptr %12, align 4, !tbaa !9
  store i64 %1, ptr %13, align 8, !tbaa !13
  store i64 %2, ptr %14, align 8, !tbaa !13
  store i64 %3, ptr %15, align 8, !tbaa !13
  store i64 %4, ptr %16, align 8, !tbaa !13
  store i64 %5, ptr %17, align 8, !tbaa !13
  store i64 %6, ptr %18, align 8, !tbaa !13
  store i64 %7, ptr %19, align 8, !tbaa !13
  store i64 %8, ptr %20, align 8, !tbaa !13
  store i64 %9, ptr %21, align 8, !tbaa !13
  store i64 %10, ptr %22, align 8, !tbaa !13
  %25 = load i32, ptr %12, align 4, !tbaa !9
  %26 = zext i32 %25 to i64
  %27 = call ptr @llvm.stacksave.p0()
  store ptr %27, ptr %23, align 8
  %28 = alloca i32, i64 %26, align 4
  store i64 %26, ptr %24, align 8
  %29 = load i64, ptr %13, align 8, !tbaa !13
  %30 = load i64, ptr %14, align 8, !tbaa !13
  %31 = add nsw i64 %29, %30
  %32 = load i64, ptr %15, align 8, !tbaa !13
  %33 = add nsw i64 %31, %32
  %34 = load i64, ptr %16, align 8, !tbaa !13
  %35 = add nsw i64 %33, %34
  %36 = load i64, ptr %17, align 8, !tbaa !13
  %37 = add nsw i64 %35, %36
  %38 = load i64, ptr %18, align 8, !tbaa !13
  %39 = add nsw i64 %37, %38
  %40 = load i64, ptr %19, align 8, !tbaa !13
  %41 = add nsw i64 %39, %40
  %42 = load i64, ptr %20, align 8, !tbaa !13
  %43 = add nsw i64 %41, %42
  %44 = load i64, ptr %21, align 8, !tbaa !13
  %45 = add nsw i64 %43, %44
  %46 = load i64, ptr %22, align 8, !tbaa !13
  %47 = add nsw i64 %45, %46
  %48 = trunc i64 %47 to i32
  %49 = getelementptr inbounds i32, ptr %28, i64 0
  store i32 %48, ptr %49, align 4, !tbaa !9
  %50 = getelementptr inbounds i32, ptr %28, i64 0
  %51 = load i32, ptr %50, align 4, !tbaa !9
  %52 = load i32, ptr %12, align 4, !tbaa !9
  %53 = add nsw i32 %51, %52
  %54 = load ptr, ptr %23, align 8
  call void @llvm.stackrestore.p0(ptr %54)
  ret i32 %53
}

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare ptr @llvm.stacksave.p0() #2

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.stackrestore.p0(ptr) #2

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nofree nosync nounwind willreturn }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"uwtable", i32 1}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!6, !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !7, i64 0}
!15 = !{!7, !7, i64 0}
!16 = distinct !{!16, !11}
!17 = distinct !{!17, !11}
