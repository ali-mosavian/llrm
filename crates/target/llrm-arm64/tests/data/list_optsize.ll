; ModuleID = 'tests/llc/list.c'
source_filename = "tests/llc/list.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

%struct.N = type { i32, ptr }

@pool = internal global [16 x %struct.N] zeroinitializer, align 8
@.str = private unnamed_addr constant [5 x i8] c"%lu\0A\00", align 1

; Function Attrs: nounwind optsize ssp
define i32 @main() #0 {
  %1 = alloca ptr, align 8
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca ptr, align 8
  store ptr null, ptr %1, align 8, !tbaa !9
  store i32 0, ptr %2, align 4, !tbaa !12
  br label %5

5:                                                ; preds = %24, %0
  %6 = load i32, ptr %2, align 4, !tbaa !12
  %7 = icmp slt i32 %6, 16
  br i1 %7, label %8, label %27

8:                                                ; preds = %5
  %9 = load i32, ptr %2, align 4, !tbaa !12
  %10 = mul nsw i32 %9, 3
  %11 = add nsw i32 %10, 1
  %12 = load i32, ptr %2, align 4, !tbaa !12
  %13 = sext i32 %12 to i64
  %14 = getelementptr inbounds [16 x %struct.N], ptr @pool, i64 0, i64 %13
  %15 = getelementptr inbounds nuw %struct.N, ptr %14, i32 0, i32 0
  store i32 %11, ptr %15, align 8, !tbaa !13
  %16 = load ptr, ptr %1, align 8, !tbaa !9
  %17 = load i32, ptr %2, align 4, !tbaa !12
  %18 = sext i32 %17 to i64
  %19 = getelementptr inbounds [16 x %struct.N], ptr @pool, i64 0, i64 %18
  %20 = getelementptr inbounds nuw %struct.N, ptr %19, i32 0, i32 1
  store ptr %16, ptr %20, align 8, !tbaa !15
  %21 = load i32, ptr %2, align 4, !tbaa !12
  %22 = sext i32 %21 to i64
  %23 = getelementptr inbounds [16 x %struct.N], ptr @pool, i64 0, i64 %22
  store ptr %23, ptr %1, align 8, !tbaa !9
  br label %24

24:                                               ; preds = %8
  %25 = load i32, ptr %2, align 4, !tbaa !12
  %26 = add nsw i32 %25, 1
  store i32 %26, ptr %2, align 4, !tbaa !12
  br label %5, !llvm.loop !16

27:                                               ; preds = %5
  store i32 0, ptr %3, align 4, !tbaa !12
  %28 = load ptr, ptr %1, align 8, !tbaa !9
  store ptr %28, ptr %4, align 8, !tbaa !9
  br label %29

29:                                               ; preds = %39, %27
  %30 = load ptr, ptr %4, align 8, !tbaa !9
  %31 = icmp ne ptr %30, null
  br i1 %31, label %32, label %43

32:                                               ; preds = %29
  %33 = load i32, ptr %3, align 4, !tbaa !12
  %34 = mul i32 %33, 3
  %35 = load ptr, ptr %4, align 8, !tbaa !9
  %36 = getelementptr inbounds nuw %struct.N, ptr %35, i32 0, i32 0
  %37 = load i32, ptr %36, align 8, !tbaa !13
  %38 = add i32 %34, %37
  store i32 %38, ptr %3, align 4, !tbaa !12
  br label %39

39:                                               ; preds = %32
  %40 = load ptr, ptr %4, align 8, !tbaa !9
  %41 = getelementptr inbounds nuw %struct.N, ptr %40, i32 0, i32 1
  %42 = load ptr, ptr %41, align 8, !tbaa !15
  store ptr %42, ptr %4, align 8, !tbaa !9
  br label %29, !llvm.loop !18

43:                                               ; preds = %29
  %44 = load i32, ptr %3, align 4, !tbaa !12
  %45 = zext i32 %44 to i64
  %46 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %45) #2
  ret i32 0
}

; Function Attrs: optsize
declare i32 @printf(ptr noundef, ...) #1

attributes #0 = { nounwind optsize ssp "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nobuiltin optsize "no-builtins" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 2, !"SDK Version", [2 x i32] [i32 27, i32 0]}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 _ZTS1N", !11, i64 0}
!11 = !{!"any pointer", !7, i64 0}
!12 = !{!6, !6, i64 0}
!13 = !{!14, !6, i64 0}
!14 = !{!"N", !6, i64 0, !10, i64 8}
!15 = !{!14, !10, i64 8}
!16 = distinct !{!16, !17}
!17 = !{!"llvm.loop.mustprogress"}
!18 = distinct !{!18, !17}
