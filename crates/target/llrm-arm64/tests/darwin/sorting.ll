source_filename = "sorting.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

%struct.item = type { ptr, i32 }

@.str = private unnamed_addr constant [6 x i8] c"delta\00", align 1
@.str.1 = private unnamed_addr constant [6 x i8] c"alpha\00", align 1
@.str.2 = private unnamed_addr constant [8 x i8] c"charlie\00", align 1
@.str.3 = private unnamed_addr constant [6 x i8] c"bravo\00", align 1
@.str.4 = private unnamed_addr constant [5 x i8] c"echo\00", align 1
@__const.main.items = private unnamed_addr constant [5 x { ptr, i32, [4 x i8] }] [{ ptr, i32, [4 x i8] } { ptr @.str, i32 40, [4 x i8] zeroinitializer }, { ptr, i32, [4 x i8] } { ptr @.str.1, i32 90, [4 x i8] zeroinitializer }, { ptr, i32, [4 x i8] } { ptr @.str.2, i32 10, [4 x i8] zeroinitializer }, { ptr, i32, [4 x i8] } { ptr @.str.3, i32 75, [4 x i8] zeroinitializer }, { ptr, i32, [4 x i8] } { ptr @.str.4, i32 60, [4 x i8] zeroinitializer }], align 8
@.str.5 = private unnamed_addr constant [7 x i8] c"%s:%d \00", align 1
@.str.6 = private unnamed_addr constant [2 x i8] c"\0A\00", align 1
@.str.7 = private unnamed_addr constant [4 x i8] c"%s \00", align 1
@.str.8 = private unnamed_addr constant [4 x i8] c"-37\00", align 1
@.str.9 = private unnamed_addr constant [4 x i8] c"%d \00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [5 x %struct.item], align 8
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca [12 x i32], align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %2, ptr align 8 @__const.main.items, i64 80, i1 false)
  %8 = getelementptr inbounds [5 x %struct.item], ptr %2, i64 0, i64 0
  call void @qsort(ptr noundef %8, i64 noundef 5, i64 noundef 16, ptr noundef @by_score)
  store i32 0, ptr %3, align 4, !tbaa !9
  br label %9

9:                                                ; preds = %24, %0
  %10 = load i32, ptr %3, align 4, !tbaa !9
  %11 = icmp slt i32 %10, 5
  br i1 %11, label %12, label %27

12:                                               ; preds = %9
  %13 = load i32, ptr %3, align 4, !tbaa !9
  %14 = sext i32 %13 to i64
  %15 = getelementptr inbounds [5 x %struct.item], ptr %2, i64 0, i64 %14
  %16 = getelementptr inbounds nuw %struct.item, ptr %15, i32 0, i32 0
  %17 = load ptr, ptr %16, align 8, !tbaa !10
  %18 = load i32, ptr %3, align 4, !tbaa !9
  %19 = sext i32 %18 to i64
  %20 = getelementptr inbounds [5 x %struct.item], ptr %2, i64 0, i64 %19
  %21 = getelementptr inbounds nuw %struct.item, ptr %20, i32 0, i32 1
  %22 = load i32, ptr %21, align 8, !tbaa !14
  %23 = call i32 (ptr, ...) @printf(ptr noundef @.str.5, ptr noundef %17, i32 noundef %22)
  br label %24

24:                                               ; preds = %12
  %25 = load i32, ptr %3, align 4, !tbaa !9
  %26 = add nsw i32 %25, 1
  store i32 %26, ptr %3, align 4, !tbaa !9
  br label %9, !llvm.loop !15

27:                                               ; preds = %9
  %28 = call i32 (ptr, ...) @printf(ptr noundef @.str.6)
  %29 = getelementptr inbounds [5 x %struct.item], ptr %2, i64 0, i64 0
  call void @qsort(ptr noundef %29, i64 noundef 5, i64 noundef 16, ptr noundef @by_name)
  store i32 0, ptr %4, align 4, !tbaa !9
  br label %30

30:                                               ; preds = %40, %27
  %31 = load i32, ptr %4, align 4, !tbaa !9
  %32 = icmp slt i32 %31, 5
  br i1 %32, label %33, label %43

33:                                               ; preds = %30
  %34 = load i32, ptr %4, align 4, !tbaa !9
  %35 = sext i32 %34 to i64
  %36 = getelementptr inbounds [5 x %struct.item], ptr %2, i64 0, i64 %35
  %37 = getelementptr inbounds nuw %struct.item, ptr %36, i32 0, i32 0
  %38 = load ptr, ptr %37, align 8, !tbaa !10
  %39 = call i32 (ptr, ...) @printf(ptr noundef @.str.7, ptr noundef %38)
  br label %40

40:                                               ; preds = %33
  %41 = load i32, ptr %4, align 4, !tbaa !9
  %42 = add nsw i32 %41, 1
  store i32 %42, ptr %4, align 4, !tbaa !9
  br label %30, !llvm.loop !17

43:                                               ; preds = %30
  %44 = call i32 (ptr, ...) @printf(ptr noundef @.str.6)
  store i32 0, ptr %6, align 4, !tbaa !9
  br label %45

45:                                               ; preds = %60, %43
  %46 = load i32, ptr %6, align 4, !tbaa !9
  %47 = icmp slt i32 %46, 12
  br i1 %47, label %48, label %63

48:                                               ; preds = %45
  %49 = call i32 @atoi(ptr noundef @.str.8)
  %50 = load i32, ptr %6, align 4, !tbaa !9
  %51 = srem i32 %50, 5
  %52 = mul nsw i32 %49, %51
  %53 = load i32, ptr %6, align 4, !tbaa !9
  %54 = mul nsw i32 %53, 13
  %55 = srem i32 %54, 17
  %56 = add nsw i32 %52, %55
  %57 = load i32, ptr %6, align 4, !tbaa !9
  %58 = sext i32 %57 to i64
  %59 = getelementptr inbounds [12 x i32], ptr %5, i64 0, i64 %58
  store i32 %56, ptr %59, align 4, !tbaa !9
  br label %60

60:                                               ; preds = %48
  %61 = load i32, ptr %6, align 4, !tbaa !9
  %62 = add nsw i32 %61, 1
  store i32 %62, ptr %6, align 4, !tbaa !9
  br label %45, !llvm.loop !18

63:                                               ; preds = %45
  %64 = getelementptr inbounds [12 x i32], ptr %5, i64 0, i64 0
  call void @qsort(ptr noundef %64, i64 noundef 12, i64 noundef 4, ptr noundef @ints)
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %65

65:                                               ; preds = %74, %63
  %66 = load i32, ptr %7, align 4, !tbaa !9
  %67 = icmp slt i32 %66, 12
  br i1 %67, label %68, label %77

68:                                               ; preds = %65
  %69 = load i32, ptr %7, align 4, !tbaa !9
  %70 = sext i32 %69 to i64
  %71 = getelementptr inbounds [12 x i32], ptr %5, i64 0, i64 %70
  %72 = load i32, ptr %71, align 4, !tbaa !9
  %73 = call i32 (ptr, ...) @printf(ptr noundef @.str.9, i32 noundef %72)
  br label %74

74:                                               ; preds = %68
  %75 = load i32, ptr %7, align 4, !tbaa !9
  %76 = add nsw i32 %75, 1
  store i32 %76, ptr %7, align 4, !tbaa !9
  br label %65, !llvm.loop !19

77:                                               ; preds = %65
  %78 = call i32 (ptr, ...) @printf(ptr noundef @.str.6)
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #1

declare void @qsort(ptr noundef, i64 noundef, i64 noundef, ptr noundef) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @by_score(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !20
  store ptr %1, ptr %4, align 8, !tbaa !20
  %5 = load ptr, ptr %4, align 8, !tbaa !20
  %6 = getelementptr inbounds nuw %struct.item, ptr %5, i32 0, i32 1
  %7 = load i32, ptr %6, align 8, !tbaa !14
  %8 = load ptr, ptr %3, align 8, !tbaa !20
  %9 = getelementptr inbounds nuw %struct.item, ptr %8, i32 0, i32 1
  %10 = load i32, ptr %9, align 8, !tbaa !14
  %11 = sub nsw i32 %7, %10
  ret i32 %11
}

declare i32 @printf(ptr noundef, ...) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @by_name(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !20
  store ptr %1, ptr %4, align 8, !tbaa !20
  %5 = load ptr, ptr %3, align 8, !tbaa !20
  %6 = getelementptr inbounds nuw %struct.item, ptr %5, i32 0, i32 0
  %7 = load ptr, ptr %6, align 8, !tbaa !10
  %8 = load ptr, ptr %4, align 8, !tbaa !20
  %9 = getelementptr inbounds nuw %struct.item, ptr %8, i32 0, i32 0
  %10 = load ptr, ptr %9, align 8, !tbaa !10
  %11 = call i32 @strcmp(ptr noundef %7, ptr noundef %10) #4
  ret i32 %11
}

declare i32 @atoi(ptr noundef) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @ints(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !20
  store ptr %1, ptr %4, align 8, !tbaa !20
  %5 = load ptr, ptr %3, align 8, !tbaa !20
  %6 = load i32, ptr %5, align 4, !tbaa !9
  %7 = load ptr, ptr %4, align 8, !tbaa !20
  %8 = load i32, ptr %7, align 4, !tbaa !9
  %9 = sub nsw i32 %6, %8
  ret i32 %9
}

; Function Attrs: nounwind
declare i32 @strcmp(ptr noundef, ptr noundef) #3

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #4 = { nounwind }

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
!10 = !{!11, !12, i64 0}
!11 = !{!"item", !12, i64 0, !6, i64 8}
!12 = !{!"p1 omnipotent char", !13, i64 0}
!13 = !{!"any pointer", !7, i64 0}
!14 = !{!11, !6, i64 8}
!15 = distinct !{!15, !16}
!16 = !{!"llvm.loop.mustprogress"}
!17 = distinct !{!17, !16}
!18 = distinct !{!18, !16}
!19 = distinct !{!19, !16}
!20 = !{!13, !13, i64 0}
