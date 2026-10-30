source_filename = "aggregates.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

%struct.pair = type { i64, i64 }
%struct.triple = type { i64, i64, i64 }
%struct.vec2 = type { double, double }
%struct.mixed = type { i32, float }
%struct.tiny = type { i8, i16 }

@.str = private unnamed_addr constant [9 x i8] c"%ld %ld\0A\00", align 1
@.str.1 = private unnamed_addr constant [17 x i8] c"%ld %ld %ld %ld\0A\00", align 1
@.str.2 = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1
@.str.3 = private unnamed_addr constant [5 x i8] c"%ld\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.pair, align 8
  %3 = alloca %struct.pair, align 8
  %4 = alloca %struct.triple, align 8
  %5 = alloca %struct.triple, align 8
  %6 = alloca %struct.triple, align 8
  %7 = alloca %struct.triple, align 8
  %8 = alloca %struct.vec2, align 8
  %9 = alloca %struct.vec2, align 8
  %10 = alloca %struct.vec2, align 8
  %11 = alloca %struct.mixed, align 4
  %12 = alloca %struct.mixed, align 4
  %13 = alloca %struct.tiny, align 2
  %14 = alloca %struct.tiny, align 2
  %15 = alloca i64, align 8
  %16 = alloca %struct.pair, align 8
  %17 = alloca %struct.pair, align 8
  %18 = alloca %struct.pair, align 8
  %19 = alloca %struct.pair, align 8
  %20 = alloca %struct.pair, align 8
  store i32 0, ptr %1, align 4
  %21 = getelementptr inbounds nuw %struct.pair, ptr %3, i32 0, i32 0
  store i64 3, ptr %21, align 8, !tbaa !9
  %22 = getelementptr inbounds nuw %struct.pair, ptr %3, i32 0, i32 1
  store i64 4, ptr %22, align 8, !tbaa !12
  %23 = load [2 x i64], ptr %3, align 8
  %24 = call [2 x i64] @swap([2 x i64] %23)
  store [2 x i64] %24, ptr %2, align 8
  %25 = getelementptr inbounds nuw %struct.pair, ptr %2, i32 0, i32 0
  %26 = load i64, ptr %25, align 8, !tbaa !9
  %27 = getelementptr inbounds nuw %struct.pair, ptr %2, i32 0, i32 1
  %28 = load i64, ptr %27, align 8, !tbaa !12
  %29 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %26, i64 noundef %28)
  %30 = getelementptr inbounds nuw %struct.triple, ptr %5, i32 0, i32 0
  store i64 1, ptr %30, align 8, !tbaa !13
  %31 = getelementptr inbounds nuw %struct.triple, ptr %5, i32 0, i32 1
  store i64 2, ptr %31, align 8, !tbaa !15
  %32 = getelementptr inbounds nuw %struct.triple, ptr %5, i32 0, i32 2
  store i64 3, ptr %32, align 8, !tbaa !16
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %6, ptr align 8 %5, i64 24, i1 false), !tbaa.struct !17
  call void @scale(ptr dead_on_unwind writable sret(%struct.triple) align 8 %4, ptr nofreeobj noundef align 8 dead_on_return dereferenceable(24) %6, i64 noundef 5)
  %33 = getelementptr inbounds nuw %struct.triple, ptr %4, i32 0, i32 0
  %34 = load i64, ptr %33, align 8, !tbaa !13
  %35 = getelementptr inbounds nuw %struct.triple, ptr %4, i32 0, i32 1
  %36 = load i64, ptr %35, align 8, !tbaa !15
  %37 = getelementptr inbounds nuw %struct.triple, ptr %4, i32 0, i32 2
  %38 = load i64, ptr %37, align 8, !tbaa !16
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %7, ptr align 8 %4, i64 24, i1 false), !tbaa.struct !17
  %39 = call i64 @sum3(ptr nofreeobj noundef align 8 dead_on_return dereferenceable(24) %7)
  %40 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i64 noundef %34, i64 noundef %36, i64 noundef %38, i64 noundef %39)
  %41 = getelementptr inbounds nuw %struct.vec2, ptr %9, i32 0, i32 0
  store double 1.500000e+00, ptr %41, align 8, !tbaa !19
  %42 = getelementptr inbounds nuw %struct.vec2, ptr %9, i32 0, i32 1
  store double 2.500000e+00, ptr %42, align 8, !tbaa !22
  %43 = getelementptr inbounds nuw %struct.vec2, ptr %10, i32 0, i32 0
  store double 3.000000e+00, ptr %43, align 8, !tbaa !19
  %44 = getelementptr inbounds nuw %struct.vec2, ptr %10, i32 0, i32 1
  store double 4.000000e+00, ptr %44, align 8, !tbaa !22
  %45 = load [2 x double], ptr %9, align 8
  %46 = load [2 x double], ptr %10, align 8
  %47 = call %struct.vec2 @add2([2 x double] %45, [2 x double] %46)
  %48 = getelementptr inbounds nuw %struct.vec2, ptr %8, i32 0, i32 0
  %49 = extractvalue %struct.vec2 %47, 0
  store double %49, ptr %48, align 8
  %50 = getelementptr inbounds nuw %struct.vec2, ptr %8, i32 0, i32 1
  %51 = extractvalue %struct.vec2 %47, 1
  store double %51, ptr %50, align 8
  %52 = getelementptr inbounds nuw %struct.vec2, ptr %8, i32 0, i32 0
  %53 = load double, ptr %52, align 8, !tbaa !19
  %54 = fmul double %53, 1.000000e+01
  %55 = fptosi double %54 to i32
  %56 = getelementptr inbounds nuw %struct.vec2, ptr %8, i32 0, i32 1
  %57 = load double, ptr %56, align 8, !tbaa !22
  %58 = fmul double %57, 1.000000e+01
  %59 = fptosi double %58 to i32
  %60 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %55, i32 noundef %59)
  %61 = getelementptr inbounds nuw %struct.mixed, ptr %12, i32 0, i32 0
  store i32 7, ptr %61, align 4, !tbaa !23
  %62 = getelementptr inbounds nuw %struct.mixed, ptr %12, i32 0, i32 1
  store float 1.250000e+00, ptr %62, align 4, !tbaa !26
  %63 = load i64, ptr %12, align 4
  %64 = call i64 @bump(i64 %63)
  store i64 %64, ptr %11, align 4
  %65 = getelementptr inbounds nuw %struct.mixed, ptr %11, i32 0, i32 0
  %66 = load i32, ptr %65, align 4, !tbaa !23
  %67 = getelementptr inbounds nuw %struct.mixed, ptr %11, i32 0, i32 1
  %68 = load float, ptr %67, align 4, !tbaa !26
  %69 = fmul float %68, 1.000000e+02
  %70 = fptosi float %69 to i32
  %71 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %66, i32 noundef %70)
  %72 = getelementptr inbounds nuw %struct.tiny, ptr %14, i32 0, i32 0
  store i8 97, ptr %72, align 2, !tbaa !27
  %73 = getelementptr i8, ptr %14, i64 1
  call void @llvm.memset.p0.i64(ptr align 1 %73, i8 0, i64 1, i1 false)
  %74 = getelementptr inbounds nuw %struct.tiny, ptr %14, i32 0, i32 1
  store i16 300, ptr %74, align 2, !tbaa !30
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %15, ptr align 2 %14, i64 4, i1 false)
  %75 = load i64, ptr %15, align 8
  %76 = call i32 @shrink(i64 %75)
  store i32 %76, ptr %13, align 2
  %77 = getelementptr inbounds nuw %struct.tiny, ptr %13, i32 0, i32 0
  %78 = load i8, ptr %77, align 2, !tbaa !27
  %79 = sext i8 %78 to i32
  %80 = getelementptr inbounds nuw %struct.tiny, ptr %13, i32 0, i32 1
  %81 = load i16, ptr %80, align 2, !tbaa !30
  %82 = sext i16 %81 to i32
  %83 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %79, i32 noundef %82)
  %84 = getelementptr inbounds nuw %struct.pair, ptr %16, i32 0, i32 0
  store i64 1, ptr %84, align 8, !tbaa !9
  %85 = getelementptr inbounds nuw %struct.pair, ptr %16, i32 0, i32 1
  store i64 2, ptr %85, align 8, !tbaa !12
  %86 = getelementptr inbounds nuw %struct.pair, ptr %17, i32 0, i32 0
  store i64 3, ptr %86, align 8, !tbaa !9
  %87 = getelementptr inbounds nuw %struct.pair, ptr %17, i32 0, i32 1
  store i64 4, ptr %87, align 8, !tbaa !12
  %88 = getelementptr inbounds nuw %struct.pair, ptr %18, i32 0, i32 0
  store i64 5, ptr %88, align 8, !tbaa !9
  %89 = getelementptr inbounds nuw %struct.pair, ptr %18, i32 0, i32 1
  store i64 6, ptr %89, align 8, !tbaa !12
  %90 = getelementptr inbounds nuw %struct.pair, ptr %19, i32 0, i32 0
  store i64 7, ptr %90, align 8, !tbaa !9
  %91 = getelementptr inbounds nuw %struct.pair, ptr %19, i32 0, i32 1
  store i64 8, ptr %91, align 8, !tbaa !12
  %92 = getelementptr inbounds nuw %struct.pair, ptr %20, i32 0, i32 0
  store i64 9, ptr %92, align 8, !tbaa !9
  %93 = getelementptr inbounds nuw %struct.pair, ptr %20, i32 0, i32 1
  store i64 10, ptr %93, align 8, !tbaa !12
  %94 = load [2 x i64], ptr %16, align 8
  %95 = load [2 x i64], ptr %17, align 8
  %96 = load [2 x i64], ptr %18, align 8
  %97 = load [2 x i64], ptr %19, align 8
  %98 = load [2 x i64], ptr %20, align 8
  %99 = call i64 @many([2 x i64] %94, [2 x i64] %95, [2 x i64] %96, [2 x i64] %97, [2 x i64] %98)
  %100 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, i64 noundef %99)
  ret i32 0
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal [2 x i64] @swap([2 x i64] %0) #0 {
  %2 = alloca %struct.pair, align 8
  %3 = alloca %struct.pair, align 8
  store [2 x i64] %0, ptr %3, align 8
  %4 = getelementptr inbounds nuw %struct.pair, ptr %2, i32 0, i32 0
  %5 = getelementptr inbounds nuw %struct.pair, ptr %3, i32 0, i32 1
  %6 = load i64, ptr %5, align 8, !tbaa !12
  store i64 %6, ptr %4, align 8, !tbaa !9
  %7 = getelementptr inbounds nuw %struct.pair, ptr %2, i32 0, i32 1
  %8 = getelementptr inbounds nuw %struct.pair, ptr %3, i32 0, i32 0
  %9 = load i64, ptr %8, align 8, !tbaa !9
  store i64 %9, ptr %7, align 8, !tbaa !12
  %10 = load [2 x i64], ptr %2, align 8
  ret [2 x i64] %10
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @scale(ptr dead_on_unwind noalias writable sret(%struct.triple) align 8 %0, ptr nofreeobj noundef align 8 dead_on_return dereferenceable(24) %1, i64 noundef %2) #0 {
  %4 = alloca ptr, align 8
  %5 = alloca i64, align 8
  store ptr %1, ptr %4, align 8, !tbaa !31
  store i64 %2, ptr %5, align 8, !tbaa !18
  %6 = getelementptr inbounds nuw %struct.triple, ptr %0, i32 0, i32 0
  %7 = getelementptr inbounds nuw %struct.triple, ptr %1, i32 0, i32 0
  %8 = load i64, ptr %7, align 8, !tbaa !13
  %9 = load i64, ptr %5, align 8, !tbaa !18
  %10 = mul nsw i64 %8, %9
  store i64 %10, ptr %6, align 8, !tbaa !13
  %11 = getelementptr inbounds nuw %struct.triple, ptr %0, i32 0, i32 1
  %12 = getelementptr inbounds nuw %struct.triple, ptr %1, i32 0, i32 1
  %13 = load i64, ptr %12, align 8, !tbaa !15
  %14 = load i64, ptr %5, align 8, !tbaa !18
  %15 = mul nsw i64 %13, %14
  store i64 %15, ptr %11, align 8, !tbaa !15
  %16 = getelementptr inbounds nuw %struct.triple, ptr %0, i32 0, i32 2
  %17 = getelementptr inbounds nuw %struct.triple, ptr %1, i32 0, i32 2
  %18 = load i64, ptr %17, align 8, !tbaa !16
  %19 = load i64, ptr %5, align 8, !tbaa !18
  %20 = mul nsw i64 %18, %19
  store i64 %20, ptr %16, align 8, !tbaa !16
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @sum3(ptr nofreeobj noundef align 8 dead_on_return dereferenceable(24) %0) #0 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8, !tbaa !31
  %3 = getelementptr inbounds nuw %struct.triple, ptr %0, i32 0, i32 0
  %4 = load i64, ptr %3, align 8, !tbaa !13
  %5 = getelementptr inbounds nuw %struct.triple, ptr %0, i32 0, i32 1
  %6 = load i64, ptr %5, align 8, !tbaa !15
  %7 = add nsw i64 %4, %6
  %8 = getelementptr inbounds nuw %struct.triple, ptr %0, i32 0, i32 2
  %9 = load i64, ptr %8, align 8, !tbaa !16
  %10 = add nsw i64 %7, %9
  ret i64 %10
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal %struct.vec2 @add2([2 x double] %0, [2 x double] %1) #0 {
  %3 = alloca %struct.vec2, align 8
  %4 = alloca %struct.vec2, align 8
  %5 = alloca %struct.vec2, align 8
  store [2 x double] %0, ptr %4, align 8
  store [2 x double] %1, ptr %5, align 8
  %6 = getelementptr inbounds nuw %struct.vec2, ptr %3, i32 0, i32 0
  %7 = getelementptr inbounds nuw %struct.vec2, ptr %4, i32 0, i32 0
  %8 = load double, ptr %7, align 8, !tbaa !19
  %9 = getelementptr inbounds nuw %struct.vec2, ptr %5, i32 0, i32 0
  %10 = load double, ptr %9, align 8, !tbaa !19
  %11 = fadd double %8, %10
  store double %11, ptr %6, align 8, !tbaa !19
  %12 = getelementptr inbounds nuw %struct.vec2, ptr %3, i32 0, i32 1
  %13 = getelementptr inbounds nuw %struct.vec2, ptr %4, i32 0, i32 1
  %14 = load double, ptr %13, align 8, !tbaa !22
  %15 = getelementptr inbounds nuw %struct.vec2, ptr %5, i32 0, i32 1
  %16 = load double, ptr %15, align 8, !tbaa !22
  %17 = fadd double %14, %16
  store double %17, ptr %12, align 8, !tbaa !22
  %18 = load %struct.vec2, ptr %3, align 8
  ret %struct.vec2 %18
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @bump(i64 %0) #0 {
  %2 = alloca %struct.mixed, align 4
  %3 = alloca %struct.mixed, align 4
  store i64 %0, ptr %3, align 4
  %4 = getelementptr inbounds nuw %struct.mixed, ptr %3, i32 0, i32 0
  %5 = load i32, ptr %4, align 4, !tbaa !23
  %6 = add nsw i32 %5, 1
  store i32 %6, ptr %4, align 4, !tbaa !23
  %7 = getelementptr inbounds nuw %struct.mixed, ptr %3, i32 0, i32 1
  %8 = load float, ptr %7, align 4, !tbaa !26
  %9 = fmul float %8, 2.000000e+00
  store float %9, ptr %7, align 4, !tbaa !26
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 %3, i64 8, i1 false), !tbaa.struct !34
  %10 = load i64, ptr %2, align 4
  ret i64 %10
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @shrink(i64 %0) #0 {
  %2 = alloca %struct.tiny, align 2
  %3 = alloca %struct.tiny, align 2
  %4 = trunc i64 %0 to i32
  store i32 %4, ptr %3, align 2
  %5 = getelementptr inbounds nuw %struct.tiny, ptr %3, i32 0, i32 0
  %6 = load i8, ptr %5, align 2, !tbaa !27
  %7 = add i8 %6, 1
  store i8 %7, ptr %5, align 2, !tbaa !27
  %8 = getelementptr inbounds nuw %struct.tiny, ptr %3, i32 0, i32 1
  %9 = load i16, ptr %8, align 2, !tbaa !30
  %10 = sext i16 %9 to i32
  %11 = add nsw i32 %10, 2
  %12 = trunc i32 %11 to i16
  store i16 %12, ptr %8, align 2, !tbaa !30
  call void @llvm.memcpy.p0.p0.i64(ptr align 2 %2, ptr align 2 %3, i64 4, i1 false), !tbaa.struct !37
  %13 = load i32, ptr %2, align 2
  ret i32 %13
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i64(ptr writeonly captures(none), i8, i64, i1 immarg) #3

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @many([2 x i64] %0, [2 x i64] %1, [2 x i64] %2, [2 x i64] %3, [2 x i64] %4) #0 {
  %6 = alloca %struct.pair, align 8
  %7 = alloca %struct.pair, align 8
  %8 = alloca %struct.pair, align 8
  %9 = alloca %struct.pair, align 8
  %10 = alloca %struct.pair, align 8
  store [2 x i64] %0, ptr %6, align 8
  store [2 x i64] %1, ptr %7, align 8
  store [2 x i64] %2, ptr %8, align 8
  store [2 x i64] %3, ptr %9, align 8
  store [2 x i64] %4, ptr %10, align 8
  %11 = getelementptr inbounds nuw %struct.pair, ptr %6, i32 0, i32 0
  %12 = load i64, ptr %11, align 8, !tbaa !9
  %13 = getelementptr inbounds nuw %struct.pair, ptr %7, i32 0, i32 1
  %14 = load i64, ptr %13, align 8, !tbaa !12
  %15 = add nsw i64 %12, %14
  %16 = getelementptr inbounds nuw %struct.pair, ptr %8, i32 0, i32 0
  %17 = load i64, ptr %16, align 8, !tbaa !9
  %18 = add nsw i64 %15, %17
  %19 = getelementptr inbounds nuw %struct.pair, ptr %9, i32 0, i32 1
  %20 = load i64, ptr %19, align 8, !tbaa !12
  %21 = add nsw i64 %18, %20
  %22 = getelementptr inbounds nuw %struct.pair, ptr %10, i32 0, i32 0
  %23 = load i64, ptr %22, align 8, !tbaa !9
  %24 = add nsw i64 %21, %23
  ret i64 %24
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }

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
!9 = !{!10, !11, i64 0}
!10 = !{!"pair", !11, i64 0, !11, i64 8}
!11 = !{!"long", !7, i64 0}
!12 = !{!10, !11, i64 8}
!13 = !{!14, !11, i64 0}
!14 = !{!"triple", !11, i64 0, !11, i64 8, !11, i64 16}
!15 = !{!14, !11, i64 8}
!16 = !{!14, !11, i64 16}
!17 = !{i64 0, i64 8, !18, i64 8, i64 8, !18, i64 16, i64 8, !18}
!18 = !{!11, !11, i64 0}
!19 = !{!20, !21, i64 0}
!20 = !{!"vec2", !21, i64 0, !21, i64 8}
!21 = !{!"double", !7, i64 0}
!22 = !{!20, !21, i64 8}
!23 = !{!24, !6, i64 0}
!24 = !{!"mixed", !6, i64 0, !25, i64 4}
!25 = !{!"float", !7, i64 0}
!26 = !{!24, !25, i64 4}
!27 = !{!28, !7, i64 0}
!28 = !{!"tiny", !7, i64 0, !29, i64 2}
!29 = !{!"short", !7, i64 0}
!30 = !{!28, !29, i64 2}
!31 = !{!32, !32, i64 0}
!32 = !{!"p1 _ZTS6triple", !33, i64 0}
!33 = !{!"any pointer", !7, i64 0}
!34 = !{i64 0, i64 4, !35, i64 4, i64 4, !36}
!35 = !{!6, !6, i64 0}
!36 = !{!25, !25, i64 0}
!37 = !{i64 0, i64 1, !38, i64 2, i64 2, !39}
!38 = !{!7, !7, i64 0}
!39 = !{!29, !29, i64 0}
