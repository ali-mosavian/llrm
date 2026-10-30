source_filename = "bits.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

%struct.flags = type { i16, [2 x i8] }

@.str = private unnamed_addr constant [10 x i8] c"%u %d %u\0A\00", align 1
@.str.1 = private unnamed_addr constant [14 x i8] c"%lx %d %d %x\0A\00", align 1
@.str.2 = private unnamed_addr constant [10 x i8] c"%u %u %u\0A\00", align 1
@.str.3 = private unnamed_addr constant [10 x i8] c"%d %d %d\0A\00", align 1
@.str.4 = private unnamed_addr constant [13 x i8] c"%ld %ld %ld\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i64, align 8
  store i32 0, ptr %1, align 4
  %5 = call i32 @pack(i32 noundef 5, i32 noundef 17, i32 noundef 1)
  %6 = call i32 @popcount(i64 noundef -1085102592571150096)
  %7 = call i32 @rotl(i32 noundef 305419896, i32 noundef 8)
  %8 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %5, i32 noundef %6, i32 noundef %7)
  %9 = call i64 @rev(i64 noundef 1311768467463790320)
  %10 = call i32 @clz(i32 noundef 16711680)
  %11 = call i32 @ctz(i64 noundef 16384)
  %12 = call i32 @swapb(i32 noundef 287454020)
  %13 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i64 noundef %9, i32 noundef %10, i32 noundef %11, i32 noundef %12)
  store i32 -559038737, ptr %2, align 4, !tbaa !9
  %14 = load i32, ptr %2, align 4, !tbaa !9
  %15 = udiv i32 %14, 10
  %16 = load i32, ptr %2, align 4, !tbaa !9
  %17 = urem i32 %16, 1000
  %18 = load i32, ptr %2, align 4, !tbaa !9
  %19 = udiv i32 %18, 7
  %20 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %15, i32 noundef %17, i32 noundef %19)
  store i32 -123456, ptr %3, align 4, !tbaa !9
  %21 = load i32, ptr %3, align 4, !tbaa !9
  %22 = sdiv i32 %21, 10
  %23 = load i32, ptr %3, align 4, !tbaa !9
  %24 = srem i32 %23, 1000
  %25 = load i32, ptr %3, align 4, !tbaa !9
  %26 = sdiv i32 %25, 7
  %27 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, i32 noundef %22, i32 noundef %24, i32 noundef %26)
  store i64 -9876543210123, ptr %4, align 8, !tbaa !10
  %28 = load i64, ptr %4, align 8, !tbaa !10
  %29 = sdiv i64 %28, 1000
  %30 = load i64, ptr %4, align 8, !tbaa !10
  %31 = srem i64 %30, 1000
  %32 = load i64, ptr %4, align 8, !tbaa !10
  %33 = sdiv i64 %32, 13
  %34 = call i32 (ptr, ...) @printf(ptr noundef @.str.4, i64 noundef %29, i64 noundef %31, i64 noundef %33)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @pack(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca %struct.flags, align 4
  store i32 %0, ptr %4, align 4, !tbaa !9
  store i32 %1, ptr %5, align 4, !tbaa !9
  store i32 %2, ptr %6, align 4, !tbaa !9
  store i16 0, ptr %7, align 4
  %8 = load i32, ptr %4, align 4, !tbaa !9
  %9 = trunc i32 %8 to i16
  %10 = load i16, ptr %7, align 4
  %11 = and i16 %9, 7
  %12 = and i16 %10, -8
  %13 = or i16 %12, %11
  store i16 %13, ptr %7, align 4
  %14 = load i32, ptr %5, align 4, !tbaa !9
  %15 = trunc i32 %14 to i16
  %16 = load i16, ptr %7, align 4
  %17 = and i16 %15, 31
  %18 = shl i16 %17, 3
  %19 = and i16 %16, -249
  %20 = or i16 %19, %18
  store i16 %20, ptr %7, align 4
  %21 = load i32, ptr %6, align 4, !tbaa !9
  %22 = trunc i32 %21 to i16
  %23 = load i16, ptr %7, align 4
  %24 = and i16 %22, 1
  %25 = shl i16 %24, 8
  %26 = and i16 %23, -257
  %27 = or i16 %26, %25
  store i16 %27, ptr %7, align 4
  %28 = load i16, ptr %7, align 4
  %29 = and i16 %28, 511
  %30 = or i16 %29, -1536
  store i16 %30, ptr %7, align 4
  %31 = getelementptr i8, ptr %7, i64 2
  call void @llvm.memset.p0.i64(ptr align 2 %31, i8 0, i64 2, i1 false)
  %32 = load i16, ptr %7, align 4
  %33 = and i16 %32, 7
  %34 = zext i16 %33 to i32
  %35 = load i16, ptr %7, align 4
  %36 = lshr i16 %35, 3
  %37 = and i16 %36, 31
  %38 = zext i16 %37 to i32
  %39 = shl i32 %38, 3
  %40 = or i32 %34, %39
  %41 = load i16, ptr %7, align 4
  %42 = lshr i16 %41, 8
  %43 = and i16 %42, 1
  %44 = zext i16 %43 to i32
  %45 = shl i32 %44, 8
  %46 = or i32 %40, %45
  %47 = load i16, ptr %7, align 4
  %48 = ashr i16 %47, 9
  %49 = sext i16 %48 to i32
  %50 = shl i32 %49, 16
  %51 = or i32 %46, %50
  ret i32 %51
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @popcount(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca i32, align 4
  store i64 %0, ptr %2, align 8, !tbaa !10
  store i32 0, ptr %3, align 4, !tbaa !9
  br label %4

4:                                                ; preds = %7, %1
  %5 = load i64, ptr %2, align 8, !tbaa !10
  %6 = icmp ne i64 %5, 0
  br i1 %6, label %7, label %14

7:                                                ; preds = %4
  %8 = load i64, ptr %2, align 8, !tbaa !10
  %9 = sub i64 %8, 1
  %10 = load i64, ptr %2, align 8, !tbaa !10
  %11 = and i64 %10, %9
  store i64 %11, ptr %2, align 8, !tbaa !10
  %12 = load i32, ptr %3, align 4, !tbaa !9
  %13 = add nsw i32 %12, 1
  store i32 %13, ptr %3, align 4, !tbaa !9
  br label %4, !llvm.loop !12

14:                                               ; preds = %4
  %15 = load i32, ptr %3, align 4, !tbaa !9
  ret i32 %15
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @rotl(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !9
  store i32 %1, ptr %4, align 4, !tbaa !9
  %5 = load i32, ptr %3, align 4, !tbaa !9
  %6 = load i32, ptr %4, align 4, !tbaa !9
  %7 = shl i32 %5, %6
  %8 = load i32, ptr %3, align 4, !tbaa !9
  %9 = load i32, ptr %4, align 4, !tbaa !9
  %10 = sub nsw i32 32, %9
  %11 = lshr i32 %8, %10
  %12 = or i32 %7, %11
  ret i32 %12
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @rev(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  %4 = alloca i32, align 4
  store i64 %0, ptr %2, align 8, !tbaa !10
  store i64 0, ptr %3, align 8, !tbaa !10
  store i32 0, ptr %4, align 4, !tbaa !9
  br label %5

5:                                                ; preds = %16, %1
  %6 = load i32, ptr %4, align 4, !tbaa !9
  %7 = icmp slt i32 %6, 64
  br i1 %7, label %8, label %19

8:                                                ; preds = %5
  %9 = load i64, ptr %3, align 8, !tbaa !10
  %10 = shl i64 %9, 1
  %11 = load i64, ptr %2, align 8, !tbaa !10
  %12 = and i64 %11, 1
  %13 = or i64 %10, %12
  store i64 %13, ptr %3, align 8, !tbaa !10
  %14 = load i64, ptr %2, align 8, !tbaa !10
  %15 = lshr i64 %14, 1
  store i64 %15, ptr %2, align 8, !tbaa !10
  br label %16

16:                                               ; preds = %8
  %17 = load i32, ptr %4, align 4, !tbaa !9
  %18 = add nsw i32 %17, 1
  store i32 %18, ptr %4, align 4, !tbaa !9
  br label %5, !llvm.loop !14

19:                                               ; preds = %5
  %20 = load i64, ptr %3, align 8, !tbaa !10
  ret i64 %20
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @clz(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  %3 = load i32, ptr %2, align 4, !tbaa !9
  %4 = or i32 %3, 1
  %5 = call i32 @llvm.ctlz.i32(i32 %4, i1 false)
  ret i32 %5
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @ctz(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !10
  %3 = load i64, ptr %2, align 8, !tbaa !10
  %4 = or i64 %3, -9223372036854775808
  %5 = call i64 @llvm.cttz.i64(i64 %4, i1 false)
  %6 = trunc i64 %5 to i32
  ret i32 %6
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @swapb(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  %3 = load i32, ptr %2, align 4, !tbaa !9
  %4 = call i32 @llvm.bswap.i32(i32 %3)
  ret i32 %4
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i64(ptr writeonly captures(none), i8, i64, i1 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.ctlz.i32(i32, i1 immarg) #3

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.cttz.i64(i64, i1 immarg) #3

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.bswap.i32(i32) #4

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #3 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #4 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }

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
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !7, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
