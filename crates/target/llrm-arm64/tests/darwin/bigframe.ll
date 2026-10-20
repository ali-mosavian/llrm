source_filename = "bigframe.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [9 x i8] c"%ld %ld\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i64 @big(i32 noundef 3)
  %3 = call i64 @big(i32 noundef -5)
  %4 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %2, i64 noundef %3)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @big(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca [5000 x i8], align 1
  %4 = alloca i32, align 4
  %5 = alloca i64, align 8
  %6 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  store i32 0, ptr %4, align 4, !tbaa !9
  br label %7

7:                                                ; preds = %18, %1
  %8 = load i32, ptr %4, align 4, !tbaa !9
  %9 = icmp slt i32 %8, 5000
  br i1 %9, label %10, label %21

10:                                               ; preds = %7
  %11 = load i32, ptr %4, align 4, !tbaa !9
  %12 = load i32, ptr %2, align 4, !tbaa !9
  %13 = mul nsw i32 %11, %12
  %14 = trunc i32 %13 to i8
  %15 = load i32, ptr %4, align 4, !tbaa !9
  %16 = sext i32 %15 to i64
  %17 = getelementptr inbounds [5000 x i8], ptr %3, i64 0, i64 %16
  store i8 %14, ptr %17, align 1, !tbaa !10
  br label %18

18:                                               ; preds = %10
  %19 = load i32, ptr %4, align 4, !tbaa !9
  %20 = add nsw i32 %19, 1
  store i32 %20, ptr %4, align 4, !tbaa !9
  br label %7, !llvm.loop !11

21:                                               ; preds = %7
  store i64 0, ptr %5, align 8, !tbaa !13
  store i32 0, ptr %6, align 4, !tbaa !9
  br label %22

22:                                               ; preds = %33, %21
  %23 = load i32, ptr %6, align 4, !tbaa !9
  %24 = icmp slt i32 %23, 5000
  br i1 %24, label %25, label %36

25:                                               ; preds = %22
  %26 = load i32, ptr %6, align 4, !tbaa !9
  %27 = sext i32 %26 to i64
  %28 = getelementptr inbounds [5000 x i8], ptr %3, i64 0, i64 %27
  %29 = load i8, ptr %28, align 1, !tbaa !10
  %30 = sext i8 %29 to i64
  %31 = load i64, ptr %5, align 8, !tbaa !13
  %32 = add nsw i64 %31, %30
  store i64 %32, ptr %5, align 8, !tbaa !13
  br label %33

33:                                               ; preds = %25
  %34 = load i32, ptr %6, align 4, !tbaa !9
  %35 = add nsw i32 %34, 7
  store i32 %35, ptr %6, align 4, !tbaa !9
  br label %22, !llvm.loop !15

36:                                               ; preds = %22
  %37 = load i64, ptr %5, align 8, !tbaa !13
  ret i64 %37
}

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
!9 = !{!6, !6, i64 0}
!10 = !{!7, !7, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !7, i64 0}
!15 = distinct !{!15, !12}
