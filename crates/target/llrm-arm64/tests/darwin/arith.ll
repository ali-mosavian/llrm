source_filename = "arith.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [10 x i8] c"%d %d %d \00", align 1
@.str.1 = private unnamed_addr constant [14 x i8] c"%lu %d %d %d\0A\00", align 1
@.str.2 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store i32 -3, ptr %2, align 4, !tbaa !9
  br label %4

4:                                                ; preds = %73, %0
  %5 = load i32, ptr %2, align 4, !tbaa !9
  %6 = icmp slt i32 %5, 4
  br i1 %6, label %7, label %76

7:                                                ; preds = %4
  store i32 -2, ptr %3, align 4, !tbaa !9
  br label %8

8:                                                ; preds = %69, %7
  %9 = load i32, ptr %3, align 4, !tbaa !9
  %10 = icmp slt i32 %9, 3
  br i1 %10, label %11, label %72

11:                                               ; preds = %8
  %12 = load i32, ptr %2, align 4, !tbaa !9
  %13 = mul nsw i32 %12, 1000
  %14 = load i32, ptr %3, align 4, !tbaa !9
  %15 = mul nsw i32 %14, 77
  %16 = call i32 @a32(i32 noundef %13, i32 noundef %15)
  %17 = load i32, ptr %2, align 4, !tbaa !9
  %18 = mul i32 %17, 999
  %19 = load i32, ptr %3, align 4, !tbaa !9
  %20 = call i32 @u32(i32 noundef %18, i32 noundef %19)
  %21 = load i32, ptr %2, align 4, !tbaa !9
  %22 = sext i32 %21 to i64
  %23 = mul nsw i64 %22, 100000000
  %24 = load i32, ptr %3, align 4, !tbaa !9
  %25 = mul nsw i32 %24, 3
  %26 = sext i32 %25 to i64
  %27 = call i64 @a64(i64 noundef %23, i64 noundef %26)
  %28 = trunc i64 %27 to i32
  %29 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %16, i32 noundef %20, i32 noundef %28)
  %30 = load i32, ptr %2, align 4, !tbaa !9
  %31 = sext i32 %30 to i64
  %32 = mul i64 %31, 123456789
  %33 = load i32, ptr %3, align 4, !tbaa !9
  %34 = add nsw i32 %33, 5
  %35 = sext i32 %34 to i64
  %36 = call i64 @u64(i64 noundef %32, i64 noundef %35)
  %37 = load i32, ptr %2, align 4, !tbaa !9
  %38 = trunc i32 %37 to i8
  %39 = load i32, ptr %3, align 4, !tbaa !9
  %40 = trunc i32 %39 to i8
  %41 = call signext i8 @c8(i8 noundef signext %38, i8 noundef signext %40)
  %42 = sext i8 %41 to i32
  %43 = load i32, ptr %2, align 4, !tbaa !9
  %44 = mul nsw i32 %43, 50
  %45 = trunc i32 %44 to i8
  %46 = load i32, ptr %3, align 4, !tbaa !9
  %47 = mul nsw i32 %46, 30
  %48 = trunc i32 %47 to i8
  %49 = call zeroext i8 @uc8(i8 noundef zeroext %45, i8 noundef zeroext %48)
  %50 = zext i8 %49 to i32
  %51 = load i32, ptr %2, align 4, !tbaa !9
  %52 = mul nsw i32 %51, 1000
  %53 = trunc i32 %52 to i16
  %54 = load i32, ptr %3, align 4, !tbaa !9
  %55 = mul nsw i32 %54, 200
  %56 = trunc i32 %55 to i16
  %57 = call signext i16 @s16(i16 noundef signext %53, i16 noundef signext %56)
  %58 = sext i16 %57 to i32
  %59 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i64 noundef %36, i32 noundef %42, i32 noundef %50, i32 noundef %58)
  %60 = load i32, ptr %2, align 4, !tbaa !9
  %61 = mul nsw i32 %60, 4000
  %62 = trunc i32 %61 to i16
  %63 = load i32, ptr %3, align 4, !tbaa !9
  %64 = mul nsw i32 %63, 5000
  %65 = trunc i32 %64 to i16
  %66 = call zeroext i16 @us16(i16 noundef zeroext %62, i16 noundef zeroext %65)
  %67 = zext i16 %66 to i32
  %68 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %67)
  br label %69

69:                                               ; preds = %11
  %70 = load i32, ptr %3, align 4, !tbaa !9
  %71 = add nsw i32 %70, 1
  store i32 %71, ptr %3, align 4, !tbaa !9
  br label %8, !llvm.loop !10

72:                                               ; preds = %8
  br label %73

73:                                               ; preds = %72
  %74 = load i32, ptr %2, align 4, !tbaa !9
  %75 = add nsw i32 %74, 1
  store i32 %75, ptr %2, align 4, !tbaa !9
  br label %4, !llvm.loop !12

76:                                               ; preds = %4
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @a32(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !9
  store i32 %1, ptr %4, align 4, !tbaa !9
  %5 = load i32, ptr %3, align 4, !tbaa !9
  %6 = load i32, ptr %4, align 4, !tbaa !9
  %7 = add nsw i32 %5, %6
  %8 = mul nsw i32 %7, 3
  %9 = load i32, ptr %3, align 4, !tbaa !9
  %10 = load i32, ptr %4, align 4, !tbaa !9
  %11 = xor i32 %9, %10
  %12 = sub nsw i32 %8, %11
  %13 = load i32, ptr %3, align 4, !tbaa !9
  %14 = and i32 %13, 255
  %15 = add nsw i32 %12, %14
  %16 = load i32, ptr %4, align 4, !tbaa !9
  %17 = or i32 %16, 7
  %18 = sub nsw i32 %15, %17
  %19 = load i32, ptr %3, align 4, !tbaa !9
  %20 = shl i32 %19, 3
  %21 = add nsw i32 %18, %20
  %22 = load i32, ptr %4, align 4, !tbaa !9
  %23 = ashr i32 %22, 2
  %24 = add nsw i32 %21, %23
  ret i32 %24
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @u32(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !9
  store i32 %1, ptr %4, align 4, !tbaa !9
  %5 = load i32, ptr %3, align 4, !tbaa !9
  %6 = mul i32 %5, 7
  %7 = load i32, ptr %4, align 4, !tbaa !9
  %8 = or i32 %7, 1
  %9 = udiv i32 %6, %8
  %10 = load i32, ptr %3, align 4, !tbaa !9
  %11 = urem i32 %10, 13
  %12 = add i32 %9, %11
  %13 = load i32, ptr %3, align 4, !tbaa !9
  %14 = lshr i32 %13, 5
  %15 = add i32 %12, %14
  %16 = load i32, ptr %4, align 4, !tbaa !9
  %17 = shl i32 %16, 2
  %18 = add i32 %15, %17
  ret i32 %18
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @a64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8, !tbaa !13
  store i64 %1, ptr %4, align 8, !tbaa !13
  %5 = load i64, ptr %3, align 8, !tbaa !13
  %6 = load i64, ptr %4, align 8, !tbaa !13
  %7 = mul nsw i64 %5, %6
  %8 = load i64, ptr %4, align 8, !tbaa !13
  %9 = or i64 %8, 1
  %10 = sdiv i64 %7, %9
  %11 = load i64, ptr %3, align 8, !tbaa !13
  %12 = srem i64 %11, 101
  %13 = add nsw i64 %10, %12
  %14 = load i64, ptr %3, align 8, !tbaa !13
  %15 = ashr i64 %14, 3
  %16 = sub nsw i64 %13, %15
  %17 = load i64, ptr %4, align 8, !tbaa !13
  %18 = shl i64 %17, 5
  %19 = add nsw i64 %16, %18
  %20 = load i64, ptr %3, align 8, !tbaa !13
  %21 = and i64 %20, 4294967295
  %22 = add nsw i64 %19, %21
  ret i64 %22
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @u64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8, !tbaa !13
  store i64 %1, ptr %4, align 8, !tbaa !13
  %5 = load i64, ptr %3, align 8, !tbaa !13
  %6 = load i64, ptr %4, align 8, !tbaa !13
  %7 = mul i64 %5, %6
  %8 = load i64, ptr %3, align 8, !tbaa !13
  %9 = load i64, ptr %4, align 8, !tbaa !13
  %10 = or i64 %9, 1
  %11 = udiv i64 %8, %10
  %12 = add i64 %7, %11
  %13 = load i64, ptr %3, align 8, !tbaa !13
  %14 = lshr i64 %13, 7
  %15 = add i64 %12, %14
  %16 = load i64, ptr %4, align 8, !tbaa !13
  %17 = shl i64 %16, 11
  %18 = add i64 %15, %17
  %19 = load i64, ptr %3, align 8, !tbaa !13
  %20 = load i64, ptr %4, align 8, !tbaa !13
  %21 = xor i64 %19, %20
  %22 = add i64 %18, %21
  ret i64 %22
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal signext i8 @c8(i8 noundef signext %0, i8 noundef signext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1, !tbaa !15
  store i8 %1, ptr %4, align 1, !tbaa !15
  %5 = load i8, ptr %3, align 1, !tbaa !15
  %6 = sext i8 %5 to i32
  %7 = load i8, ptr %4, align 1, !tbaa !15
  %8 = sext i8 %7 to i32
  %9 = add nsw i32 %6, %8
  %10 = trunc i32 %9 to i8
  %11 = sext i8 %10 to i32
  %12 = mul nsw i32 %11, 3
  %13 = trunc i32 %12 to i8
  ret i8 %13
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal zeroext i8 @uc8(i8 noundef zeroext %0, i8 noundef zeroext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1, !tbaa !15
  store i8 %1, ptr %4, align 1, !tbaa !15
  %5 = load i8, ptr %3, align 1, !tbaa !15
  %6 = zext i8 %5 to i32
  %7 = load i8, ptr %4, align 1, !tbaa !15
  %8 = zext i8 %7 to i32
  %9 = mul nsw i32 %6, %8
  %10 = add nsw i32 %9, 17
  %11 = trunc i32 %10 to i8
  ret i8 %11
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal signext i16 @s16(i16 noundef signext %0, i16 noundef signext %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !16
  store i16 %1, ptr %4, align 2, !tbaa !16
  %5 = load i16, ptr %3, align 2, !tbaa !16
  %6 = sext i16 %5 to i32
  %7 = load i16, ptr %4, align 2, !tbaa !16
  %8 = sext i16 %7 to i32
  %9 = mul nsw i32 %6, %8
  %10 = sub nsw i32 %9, 100
  %11 = trunc i32 %10 to i16
  %12 = sext i16 %11 to i32
  %13 = ashr i32 %12, 1
  %14 = trunc i32 %13 to i16
  ret i16 %14
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal zeroext i16 @us16(i16 noundef zeroext %0, i16 noundef zeroext %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !16
  store i16 %1, ptr %4, align 2, !tbaa !16
  %5 = load i16, ptr %3, align 2, !tbaa !16
  %6 = zext i16 %5 to i32
  %7 = load i16, ptr %4, align 2, !tbaa !16
  %8 = zext i16 %7 to i32
  %9 = mul nsw i32 %6, %8
  %10 = add nsw i32 %9, 3
  %11 = trunc i32 %10 to i16
  %12 = zext i16 %11 to i32
  %13 = ashr i32 %12, 2
  %14 = trunc i32 %13 to i16
  ret i16 %14
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
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !7, i64 0}
!15 = !{!7, !7, i64 0}
!16 = !{!17, !17, i64 0}
!17 = !{!"short", !7, i64 0}
