; ModuleID = '/home/alim/scratch/wt-z80f32b/tests/run/c/compound_assignment_types.c'
source_filename = "/home/alim/scratch/wt-z80f32b/tests/run/c/compound_assignment_types.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

@uc = dso_local global i8 50, align 1
@sy = dso_local global i16 -5, align 2
@uy = dso_local global i8 7, align 1
@ly = dso_local global i32 -3, align 2
@us = dso_local global i16 -25536, align 2
@sc = dso_local global i8 -100, align 1
@shift = dso_local global i16 3, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  %3 = load volatile i16, ptr @sy, align 2, !tbaa !9
  %4 = load i8, ptr @uc, align 1, !tbaa !11
  %5 = zext i8 %4 to i16
  %6 = sdiv i16 %5, %3
  %7 = trunc i16 %6 to i8
  store i8 %7, ptr @uc, align 1, !tbaa !11
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = mul nsw i32 %8, 7
  %10 = load i8, ptr @uc, align 1, !tbaa !11
  %11 = zext i8 %10 to i32
  %12 = add nsw i32 %9, %11
  store i32 %12, ptr %2, align 2, !tbaa !7
  store i8 -56, ptr @uc, align 1, !tbaa !11
  %13 = load volatile i16, ptr @sy, align 2, !tbaa !9
  %14 = load i8, ptr @uc, align 1, !tbaa !11
  %15 = zext i8 %14 to i16
  %16 = srem i16 %15, %13
  %17 = trunc i16 %16 to i8
  store i8 %17, ptr @uc, align 1, !tbaa !11
  %18 = load i32, ptr %2, align 2, !tbaa !7
  %19 = mul nsw i32 %18, 7
  %20 = load i8, ptr @uc, align 1, !tbaa !11
  %21 = zext i8 %20 to i32
  %22 = add nsw i32 %19, %21
  store i32 %22, ptr %2, align 2, !tbaa !7
  %23 = load volatile i32, ptr @ly, align 2, !tbaa !7
  %24 = load i16, ptr @us, align 2, !tbaa !9
  %25 = zext i16 %24 to i32
  %26 = sdiv i32 %25, %23
  %27 = trunc i32 %26 to i16
  store i16 %27, ptr @us, align 2, !tbaa !9
  %28 = load i32, ptr %2, align 2, !tbaa !7
  %29 = mul nsw i32 %28, 7
  %30 = load i16, ptr @us, align 2, !tbaa !9
  %31 = zext i16 %30 to i32
  %32 = add nsw i32 %29, %31
  store i32 %32, ptr %2, align 2, !tbaa !7
  %33 = load volatile i8, ptr @uy, align 1, !tbaa !11
  %34 = zext i8 %33 to i16
  %35 = load i8, ptr @sc, align 1, !tbaa !11
  %36 = sext i8 %35 to i16
  %37 = sdiv i16 %36, %34
  %38 = trunc i16 %37 to i8
  store i8 %38, ptr @sc, align 1, !tbaa !11
  %39 = load i32, ptr %2, align 2, !tbaa !7
  %40 = mul nsw i32 %39, 7
  %41 = load i8, ptr @sc, align 1, !tbaa !11
  %42 = sext i8 %41 to i32
  %43 = add nsw i32 %40, %42
  store i32 %43, ptr %2, align 2, !tbaa !7
  store i8 -100, ptr @sc, align 1, !tbaa !11
  %44 = load volatile i16, ptr @shift, align 2, !tbaa !12
  %45 = load i8, ptr @sc, align 1, !tbaa !11
  %46 = sext i8 %45 to i16
  %47 = ashr i16 %46, %44
  %48 = trunc i16 %47 to i8
  store i8 %48, ptr @sc, align 1, !tbaa !11
  %49 = load i32, ptr %2, align 2, !tbaa !7
  %50 = mul nsw i32 %49, 7
  %51 = load i8, ptr @sc, align 1, !tbaa !11
  %52 = sext i8 %51 to i32
  %53 = add nsw i32 %50, %52
  store i32 %53, ptr %2, align 2, !tbaa !7
  store i8 -127, ptr @uc, align 1, !tbaa !11
  %54 = load volatile i16, ptr @shift, align 2, !tbaa !12
  %55 = load i8, ptr @uc, align 1, !tbaa !11
  %56 = zext i8 %55 to i16
  %57 = shl i16 %56, %54
  %58 = trunc i16 %57 to i8
  store i8 %58, ptr @uc, align 1, !tbaa !11
  %59 = load i32, ptr %2, align 2, !tbaa !7
  %60 = mul nsw i32 %59, 7
  %61 = load i8, ptr @uc, align 1, !tbaa !11
  %62 = zext i8 %61 to i32
  %63 = add nsw i32 %60, %62
  store i32 %63, ptr %2, align 2, !tbaa !7
  %64 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %64) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !5, i64 0}
!11 = !{!5, !5, i64 0}
!12 = !{!4, !4, i64 0}
