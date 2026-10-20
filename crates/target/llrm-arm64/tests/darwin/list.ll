source_filename = "list.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

%struct.node = type { i32, ptr }
%struct.tree = type { i32, ptr, ptr }

@.str = private unnamed_addr constant [10 x i8] c"%d %d %d\0A\00", align 1
@__const.main.keys = private unnamed_addr constant [10 x i32] [i32 50, i32 30, i32 70, i32 20, i32 40, i32 60, i32 80, i32 35, i32 45, i32 65], align 4
@.str.1 = private unnamed_addr constant [2 x i8] c"\0A\00", align 1
@.str.2 = private unnamed_addr constant [7 x i8] c"%d@%d \00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca ptr, align 8
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 8
  %6 = alloca ptr, align 8
  %7 = alloca [10 x i32], align 4
  %8 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store ptr null, ptr %2, align 8, !tbaa !9
  store i32 1, ptr %3, align 4, !tbaa !12
  br label %9

9:                                                ; preds = %18, %0
  %10 = load i32, ptr %3, align 4, !tbaa !12
  %11 = icmp sle i32 %10, 10
  br i1 %11, label %12, label %21

12:                                               ; preds = %9
  %13 = load ptr, ptr %2, align 8, !tbaa !9
  %14 = load i32, ptr %3, align 4, !tbaa !12
  %15 = load i32, ptr %3, align 4, !tbaa !12
  %16 = mul nsw i32 %14, %15
  %17 = call ptr @push(ptr noundef %13, i32 noundef %16)
  store ptr %17, ptr %2, align 8, !tbaa !9
  br label %18

18:                                               ; preds = %12
  %19 = load i32, ptr %3, align 4, !tbaa !12
  %20 = add nsw i32 %19, 1
  store i32 %20, ptr %3, align 4, !tbaa !12
  br label %9, !llvm.loop !13

21:                                               ; preds = %9
  %22 = load ptr, ptr %2, align 8, !tbaa !9
  %23 = call ptr @reverse(ptr noundef %22)
  store ptr %23, ptr %2, align 8, !tbaa !9
  store i32 0, ptr %4, align 4, !tbaa !12
  %24 = load ptr, ptr %2, align 8, !tbaa !9
  store ptr %24, ptr %5, align 8, !tbaa !9
  br label %25

25:                                               ; preds = %34, %21
  %26 = load ptr, ptr %5, align 8, !tbaa !9
  %27 = icmp ne ptr %26, null
  br i1 %27, label %28, label %38

28:                                               ; preds = %25
  %29 = load ptr, ptr %5, align 8, !tbaa !9
  %30 = getelementptr inbounds nuw %struct.node, ptr %29, i32 0, i32 0
  %31 = load i32, ptr %30, align 8, !tbaa !15
  %32 = load i32, ptr %4, align 4, !tbaa !12
  %33 = add nsw i32 %32, %31
  store i32 %33, ptr %4, align 4, !tbaa !12
  br label %34

34:                                               ; preds = %28
  %35 = load ptr, ptr %5, align 8, !tbaa !9
  %36 = getelementptr inbounds nuw %struct.node, ptr %35, i32 0, i32 1
  %37 = load ptr, ptr %36, align 8, !tbaa !17
  store ptr %37, ptr %5, align 8, !tbaa !9
  br label %25, !llvm.loop !18

38:                                               ; preds = %25
  %39 = load ptr, ptr %2, align 8, !tbaa !9
  %40 = call i32 @length(ptr noundef %39)
  %41 = load i32, ptr %4, align 4, !tbaa !12
  %42 = load ptr, ptr %2, align 8, !tbaa !9
  %43 = getelementptr inbounds nuw %struct.node, ptr %42, i32 0, i32 0
  %44 = load i32, ptr %43, align 8, !tbaa !15
  %45 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %40, i32 noundef %41, i32 noundef %44)
  store ptr null, ptr %6, align 8, !tbaa !19
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %7, ptr align 4 @__const.main.keys, i64 40, i1 false)
  store i32 0, ptr %8, align 4, !tbaa !12
  br label %46

46:                                               ; preds = %56, %38
  %47 = load i32, ptr %8, align 4, !tbaa !12
  %48 = icmp slt i32 %47, 10
  br i1 %48, label %49, label %59

49:                                               ; preds = %46
  %50 = load ptr, ptr %6, align 8, !tbaa !19
  %51 = load i32, ptr %8, align 4, !tbaa !12
  %52 = sext i32 %51 to i64
  %53 = getelementptr inbounds [10 x i32], ptr %7, i64 0, i64 %52
  %54 = load i32, ptr %53, align 4, !tbaa !12
  %55 = call ptr @insert(ptr noundef %50, i32 noundef %54)
  store ptr %55, ptr %6, align 8, !tbaa !19
  br label %56

56:                                               ; preds = %49
  %57 = load i32, ptr %8, align 4, !tbaa !12
  %58 = add nsw i32 %57, 1
  store i32 %58, ptr %8, align 4, !tbaa !12
  br label %46, !llvm.loop !21

59:                                               ; preds = %46
  %60 = load ptr, ptr %6, align 8, !tbaa !19
  call void @walk(ptr noundef %60, i32 noundef 0)
  %61 = call i32 (ptr, ...) @printf(ptr noundef @.str.1)
  ret i32 0
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal ptr @push(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !9
  store i32 %1, ptr %4, align 4, !tbaa !12
  %6 = call ptr @malloc(i64 noundef 16) #4
  store ptr %6, ptr %5, align 8, !tbaa !9
  %7 = load i32, ptr %4, align 4, !tbaa !12
  %8 = load ptr, ptr %5, align 8, !tbaa !9
  %9 = getelementptr inbounds nuw %struct.node, ptr %8, i32 0, i32 0
  store i32 %7, ptr %9, align 8, !tbaa !15
  %10 = load ptr, ptr %3, align 8, !tbaa !9
  %11 = load ptr, ptr %5, align 8, !tbaa !9
  %12 = getelementptr inbounds nuw %struct.node, ptr %11, i32 0, i32 1
  store ptr %10, ptr %12, align 8, !tbaa !17
  %13 = load ptr, ptr %5, align 8, !tbaa !9
  ret ptr %13
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal ptr @reverse(ptr noundef %0) #0 {
  %2 = alloca ptr, align 8
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8, !tbaa !9
  store ptr null, ptr %3, align 8, !tbaa !9
  br label %5

5:                                                ; preds = %8, %1
  %6 = load ptr, ptr %2, align 8, !tbaa !9
  %7 = icmp ne ptr %6, null
  br i1 %7, label %8, label %17

8:                                                ; preds = %5
  %9 = load ptr, ptr %2, align 8, !tbaa !9
  %10 = getelementptr inbounds nuw %struct.node, ptr %9, i32 0, i32 1
  %11 = load ptr, ptr %10, align 8, !tbaa !17
  store ptr %11, ptr %4, align 8, !tbaa !9
  %12 = load ptr, ptr %3, align 8, !tbaa !9
  %13 = load ptr, ptr %2, align 8, !tbaa !9
  %14 = getelementptr inbounds nuw %struct.node, ptr %13, i32 0, i32 1
  store ptr %12, ptr %14, align 8, !tbaa !17
  %15 = load ptr, ptr %2, align 8, !tbaa !9
  store ptr %15, ptr %3, align 8, !tbaa !9
  %16 = load ptr, ptr %4, align 8, !tbaa !9
  store ptr %16, ptr %2, align 8, !tbaa !9
  br label %5, !llvm.loop !22

17:                                               ; preds = %5
  %18 = load ptr, ptr %3, align 8, !tbaa !9
  ret ptr %18
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @length(ptr noundef %0) #0 {
  %2 = alloca ptr, align 8
  %3 = alloca i32, align 4
  store ptr %0, ptr %2, align 8, !tbaa !9
  store i32 0, ptr %3, align 4, !tbaa !12
  br label %4

4:                                                ; preds = %10, %1
  %5 = load ptr, ptr %2, align 8, !tbaa !9
  %6 = icmp ne ptr %5, null
  br i1 %6, label %7, label %14

7:                                                ; preds = %4
  %8 = load i32, ptr %3, align 4, !tbaa !12
  %9 = add nsw i32 %8, 1
  store i32 %9, ptr %3, align 4, !tbaa !12
  br label %10

10:                                               ; preds = %7
  %11 = load ptr, ptr %2, align 8, !tbaa !9
  %12 = getelementptr inbounds nuw %struct.node, ptr %11, i32 0, i32 1
  %13 = load ptr, ptr %12, align 8, !tbaa !17
  store ptr %13, ptr %2, align 8, !tbaa !9
  br label %4, !llvm.loop !23

14:                                               ; preds = %4
  %15 = load i32, ptr %3, align 4, !tbaa !12
  ret i32 %15
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal ptr @insert(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  %5 = alloca i32, align 4
  store ptr %0, ptr %4, align 8, !tbaa !19
  store i32 %1, ptr %5, align 4, !tbaa !12
  %6 = load ptr, ptr %4, align 8, !tbaa !19
  %7 = icmp ne ptr %6, null
  br i1 %7, label %18, label %8

8:                                                ; preds = %2
  %9 = call ptr @malloc(i64 noundef 24) #4
  store ptr %9, ptr %4, align 8, !tbaa !19
  %10 = load i32, ptr %5, align 4, !tbaa !12
  %11 = load ptr, ptr %4, align 8, !tbaa !19
  %12 = getelementptr inbounds nuw %struct.tree, ptr %11, i32 0, i32 0
  store i32 %10, ptr %12, align 8, !tbaa !24
  %13 = load ptr, ptr %4, align 8, !tbaa !19
  %14 = getelementptr inbounds nuw %struct.tree, ptr %13, i32 0, i32 2
  store ptr null, ptr %14, align 8, !tbaa !26
  %15 = load ptr, ptr %4, align 8, !tbaa !19
  %16 = getelementptr inbounds nuw %struct.tree, ptr %15, i32 0, i32 1
  store ptr null, ptr %16, align 8, !tbaa !27
  %17 = load ptr, ptr %4, align 8, !tbaa !19
  store ptr %17, ptr %3, align 8
  br label %42

18:                                               ; preds = %2
  %19 = load i32, ptr %5, align 4, !tbaa !12
  %20 = load ptr, ptr %4, align 8, !tbaa !19
  %21 = getelementptr inbounds nuw %struct.tree, ptr %20, i32 0, i32 0
  %22 = load i32, ptr %21, align 8, !tbaa !24
  %23 = icmp slt i32 %19, %22
  br i1 %23, label %24, label %32

24:                                               ; preds = %18
  %25 = load ptr, ptr %4, align 8, !tbaa !19
  %26 = getelementptr inbounds nuw %struct.tree, ptr %25, i32 0, i32 1
  %27 = load ptr, ptr %26, align 8, !tbaa !27
  %28 = load i32, ptr %5, align 4, !tbaa !12
  %29 = call ptr @insert(ptr noundef %27, i32 noundef %28)
  %30 = load ptr, ptr %4, align 8, !tbaa !19
  %31 = getelementptr inbounds nuw %struct.tree, ptr %30, i32 0, i32 1
  store ptr %29, ptr %31, align 8, !tbaa !27
  br label %40

32:                                               ; preds = %18
  %33 = load ptr, ptr %4, align 8, !tbaa !19
  %34 = getelementptr inbounds nuw %struct.tree, ptr %33, i32 0, i32 2
  %35 = load ptr, ptr %34, align 8, !tbaa !26
  %36 = load i32, ptr %5, align 4, !tbaa !12
  %37 = call ptr @insert(ptr noundef %35, i32 noundef %36)
  %38 = load ptr, ptr %4, align 8, !tbaa !19
  %39 = getelementptr inbounds nuw %struct.tree, ptr %38, i32 0, i32 2
  store ptr %37, ptr %39, align 8, !tbaa !26
  br label %40

40:                                               ; preds = %32, %24
  %41 = load ptr, ptr %4, align 8, !tbaa !19
  store ptr %41, ptr %3, align 8
  br label %42

42:                                               ; preds = %40, %8
  %43 = load ptr, ptr %3, align 8
  ret ptr %43
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @walk(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !19
  store i32 %1, ptr %4, align 4, !tbaa !12
  %5 = load ptr, ptr %3, align 8, !tbaa !19
  %6 = icmp ne ptr %5, null
  br i1 %6, label %8, label %7

7:                                                ; preds = %2
  br label %24

8:                                                ; preds = %2
  %9 = load ptr, ptr %3, align 8, !tbaa !19
  %10 = getelementptr inbounds nuw %struct.tree, ptr %9, i32 0, i32 1
  %11 = load ptr, ptr %10, align 8, !tbaa !27
  %12 = load i32, ptr %4, align 4, !tbaa !12
  %13 = add nsw i32 %12, 1
  call void @walk(ptr noundef %11, i32 noundef %13)
  %14 = load ptr, ptr %3, align 8, !tbaa !19
  %15 = getelementptr inbounds nuw %struct.tree, ptr %14, i32 0, i32 0
  %16 = load i32, ptr %15, align 8, !tbaa !24
  %17 = load i32, ptr %4, align 4, !tbaa !12
  %18 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %16, i32 noundef %17)
  %19 = load ptr, ptr %3, align 8, !tbaa !19
  %20 = getelementptr inbounds nuw %struct.tree, ptr %19, i32 0, i32 2
  %21 = load ptr, ptr %20, align 8, !tbaa !26
  %22 = load i32, ptr %4, align 4, !tbaa !12
  %23 = add nsw i32 %22, 1
  call void @walk(ptr noundef %21, i32 noundef %23)
  br label %24

24:                                               ; preds = %8, %7
  ret void
}

; Function Attrs: allocsize(0)
declare ptr @malloc(i64 noundef) #3

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { allocsize(0) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #4 = { allocsize(0) }

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
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 _ZTS4node", !11, i64 0}
!11 = !{!"any pointer", !7, i64 0}
!12 = !{!6, !6, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = !{!16, !6, i64 0}
!16 = !{!"node", !6, i64 0, !10, i64 8}
!17 = !{!16, !10, i64 8}
!18 = distinct !{!18, !14}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 _ZTS4tree", !11, i64 0}
!21 = distinct !{!21, !14}
!22 = distinct !{!22, !14}
!23 = distinct !{!23, !14}
!24 = !{!25, !6, i64 0}
!25 = !{!"tree", !6, i64 0, !20, i64 8, !20, i64 16}
!26 = !{!25, !20, i64 16}
!27 = !{!25, !20, i64 8}
