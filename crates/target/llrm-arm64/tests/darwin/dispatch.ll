source_filename = "dispatch.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@__const.main.ops = private unnamed_addr constant [12 x i32] [i32 1, i32 2, i32 3, i32 4, i32 10, i32 100, i32 1000, i32 70000, i32 -5, i32 7, i32 0, i32 -6], align 4
@.str = private unnamed_addr constant [4 x i8] c"%d \00", align 1
@.str.1 = private unnamed_addr constant [2 x i8] c"\0A\00", align 1
@.str.2 = private unnamed_addr constant [5 x i8] c"ten\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [12 x i32], align 4
  %3 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const.main.ops, i64 48, i1 false)
  store i32 0, ptr %3, align 4, !tbaa !9
  br label %4

4:                                                ; preds = %16, %0
  %5 = load i32, ptr %3, align 4, !tbaa !9
  %6 = icmp slt i32 %5, 12
  br i1 %6, label %7, label %19

7:                                                ; preds = %4
  %8 = load i32, ptr %3, align 4, !tbaa !9
  %9 = sext i32 %8 to i64
  %10 = getelementptr inbounds [12 x i32], ptr %2, i64 0, i64 %9
  %11 = load i32, ptr %10, align 4, !tbaa !9
  %12 = load i32, ptr %3, align 4, !tbaa !9
  %13 = add nsw i32 20, %12
  %14 = call i32 @run(i32 noundef %11, i32 noundef %13)
  %15 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %14)
  br label %16

16:                                               ; preds = %7
  %17 = load i32, ptr %3, align 4, !tbaa !9
  %18 = add nsw i32 %17, 1
  store i32 %18, ptr %3, align 4, !tbaa !9
  br label %4, !llvm.loop !10

19:                                               ; preds = %4
  %20 = call i32 (ptr, ...) @printf(ptr noundef @.str.1)
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #1

declare i32 @printf(ptr noundef, ...) #2

; Function Attrs: noinline nounwind ssp uwtable(sync)
define internal i32 @run(i32 noundef %0, i32 noundef %1) #3 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !9
  store i32 %1, ptr %5, align 4, !tbaa !9
  %6 = load i32, ptr %4, align 4, !tbaa !9
  switch i32 %6, label %36 [
    i32 1, label %7
    i32 2, label %10
    i32 3, label %13
    i32 4, label %16
    i32 10, label %19
    i32 100, label %22
    i32 1000, label %26
    i32 70000, label %30
    i32 -5, label %33
  ]

7:                                                ; preds = %2
  %8 = load i32, ptr %5, align 4, !tbaa !9
  %9 = call i32 @a(i32 noundef %8)
  store i32 %9, ptr %3, align 4
  br label %37

10:                                               ; preds = %2
  %11 = load i32, ptr %5, align 4, !tbaa !9
  %12 = call i32 @b(i32 noundef %11)
  store i32 %12, ptr %3, align 4
  br label %37

13:                                               ; preds = %2
  %14 = load i32, ptr %5, align 4, !tbaa !9
  %15 = add nsw i32 %14, 5
  store i32 %15, ptr %5, align 4, !tbaa !9
  br label %16

16:                                               ; preds = %2, %13
  %17 = load i32, ptr %5, align 4, !tbaa !9
  %18 = call i32 @c(i32 noundef %17)
  store i32 %18, ptr %3, align 4
  br label %37

19:                                               ; preds = %2
  %20 = call i32 (ptr, ...) @printf(ptr noundef @.str.2)
  %21 = load i32, ptr %5, align 4, !tbaa !9
  store i32 %21, ptr %3, align 4
  br label %37

22:                                               ; preds = %2
  %23 = load i32, ptr %5, align 4, !tbaa !9
  %24 = call i32 @b(i32 noundef %23)
  %25 = call i32 @a(i32 noundef %24)
  store i32 %25, ptr %3, align 4
  br label %37

26:                                               ; preds = %2
  %27 = load i32, ptr %5, align 4, !tbaa !9
  %28 = call i32 @c(i32 noundef %27)
  %29 = call i32 @c(i32 noundef %28)
  store i32 %29, ptr %3, align 4
  br label %37

30:                                               ; preds = %2
  %31 = load i32, ptr %5, align 4, !tbaa !9
  %32 = xor i32 %31, 255
  store i32 %32, ptr %3, align 4
  br label %37

33:                                               ; preds = %2
  %34 = load i32, ptr %5, align 4, !tbaa !9
  %35 = sub nsw i32 0, %34
  store i32 %35, ptr %3, align 4
  br label %37

36:                                               ; preds = %2
  store i32 0, ptr %3, align 4
  br label %37

37:                                               ; preds = %36, %33, %30, %26, %22, %19, %16, %10, %7
  %38 = load i32, ptr %3, align 4
  ret i32 %38
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @a(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  %3 = load i32, ptr %2, align 4, !tbaa !9
  %4 = add nsw i32 %3, 1
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @b(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  %3 = load i32, ptr %2, align 4, !tbaa !9
  %4 = mul nsw i32 %3, 3
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @c(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  %3 = load i32, ptr %2, align 4, !tbaa !9
  %4 = sub nsw i32 %3, 7
  ret i32 %4
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { noinline nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

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
