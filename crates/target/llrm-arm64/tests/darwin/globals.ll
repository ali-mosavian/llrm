source_filename = "globals.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@table = global [8 x i32] [i32 3, i32 1, i32 4, i32 1, i32 5, i32 9, i32 2, i32 6], align 4
@.str = private unnamed_addr constant [5 x i8] c"zero\00", align 1
@.str.1 = private unnamed_addr constant [4 x i8] c"one\00", align 1
@.str.2 = private unnamed_addr constant [4 x i8] c"two\00", align 1
@names = global [3 x ptr] [ptr @.str, ptr @.str.1, ptr @.str.2], align 8
@.str.3 = private unnamed_addr constant [7 x i8] c"%s=%d\0A\00", align 1
@counter = internal global i64 0, align 8
@.str.4 = private unnamed_addr constant [11 x i8] c"total %ld\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @total() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 4
  store i64 0, ptr %1, align 8, !tbaa !9
  store i32 0, ptr %2, align 4, !tbaa !11
  br label %3

3:                                                ; preds = %14, %0
  %4 = load i32, ptr %2, align 4, !tbaa !11
  %5 = icmp slt i32 %4, 8
  br i1 %5, label %6, label %17

6:                                                ; preds = %3
  %7 = load i32, ptr %2, align 4, !tbaa !11
  %8 = sext i32 %7 to i64
  %9 = getelementptr inbounds [8 x i32], ptr @table, i64 0, i64 %8
  %10 = load i32, ptr %9, align 4, !tbaa !11
  %11 = sext i32 %10 to i64
  %12 = load i64, ptr %1, align 8, !tbaa !9
  %13 = add nsw i64 %12, %11
  store i64 %13, ptr %1, align 8, !tbaa !9
  br label %14

14:                                               ; preds = %6
  %15 = load i32, ptr %2, align 4, !tbaa !11
  %16 = add nsw i32 %15, 1
  store i32 %16, ptr %2, align 4, !tbaa !11
  br label %3, !llvm.loop !12

17:                                               ; preds = %3
  %18 = load i64, ptr %1, align 8, !tbaa !9
  ret i64 %18
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store i32 0, ptr %2, align 4, !tbaa !11
  br label %3

3:                                                ; preds = %13, %0
  %4 = load i32, ptr %2, align 4, !tbaa !11
  %5 = icmp slt i32 %4, 3
  br i1 %5, label %6, label %16

6:                                                ; preds = %3
  %7 = load i32, ptr %2, align 4, !tbaa !11
  %8 = sext i32 %7 to i64
  %9 = getelementptr inbounds [3 x ptr], ptr @names, i64 0, i64 %8
  %10 = load ptr, ptr %9, align 8, !tbaa !14
  %11 = load i32, ptr %2, align 4, !tbaa !11
  %12 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, ptr noundef %10, i32 noundef %11)
  br label %13

13:                                               ; preds = %6
  %14 = load i32, ptr %2, align 4, !tbaa !11
  %15 = add nsw i32 %14, 1
  store i32 %15, ptr %2, align 4, !tbaa !11
  br label %3, !llvm.loop !17

16:                                               ; preds = %3
  %17 = call i64 @total()
  %18 = load i64, ptr @counter, align 8, !tbaa !9
  %19 = add nsw i64 %18, %17
  store i64 %19, ptr @counter, align 8, !tbaa !9
  %20 = load i64, ptr @counter, align 8, !tbaa !9
  %21 = call i32 (ptr, ...) @printf(ptr noundef @.str.4, i64 noundef %20)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

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
!10 = !{!"long", !7, i64 0}
!11 = !{!6, !6, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 omnipotent char", !16, i64 0}
!16 = !{!"any pointer", !7, i64 0}
!17 = distinct !{!17, !13}
