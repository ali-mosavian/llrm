target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@s = hidden local_unnamed_addr global [48 x i16] zeroinitializer, align 16
@d = hidden local_unnamed_addr global [48 x i16] zeroinitializer, align 16

; Function Attrs: mustprogress nofree norecurse nosync nounwind optsize willreturn memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none)
define hidden void @copy() local_unnamed_addr #0 {
  %1 = load i16, ptr @s, align 16, !tbaa !6
  store i16 %1, ptr @d, align 16, !tbaa !6
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 2), align 2, !tbaa !6
  store i16 %2, ptr getelementptr inbounds nuw (i8, ptr @d, i32 2), align 2, !tbaa !6
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 4), align 4, !tbaa !6
  store i16 %3, ptr getelementptr inbounds nuw (i8, ptr @d, i32 4), align 4, !tbaa !6
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 6), align 2, !tbaa !6
  store i16 %4, ptr getelementptr inbounds nuw (i8, ptr @d, i32 6), align 2, !tbaa !6
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 8), align 8, !tbaa !6
  store i16 %5, ptr getelementptr inbounds nuw (i8, ptr @d, i32 8), align 8, !tbaa !6
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 10), align 2, !tbaa !6
  store i16 %6, ptr getelementptr inbounds nuw (i8, ptr @d, i32 10), align 2, !tbaa !6
  %7 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 12), align 4, !tbaa !6
  store i16 %7, ptr getelementptr inbounds nuw (i8, ptr @d, i32 12), align 4, !tbaa !6
  %8 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 14), align 2, !tbaa !6
  store i16 %8, ptr getelementptr inbounds nuw (i8, ptr @d, i32 14), align 2, !tbaa !6
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 16), align 16, !tbaa !6
  store i16 %9, ptr getelementptr inbounds nuw (i8, ptr @d, i32 16), align 16, !tbaa !6
  %10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 18), align 2, !tbaa !6
  store i16 %10, ptr getelementptr inbounds nuw (i8, ptr @d, i32 18), align 2, !tbaa !6
  %11 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 20), align 4, !tbaa !6
  store i16 %11, ptr getelementptr inbounds nuw (i8, ptr @d, i32 20), align 4, !tbaa !6
  %12 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 22), align 2, !tbaa !6
  store i16 %12, ptr getelementptr inbounds nuw (i8, ptr @d, i32 22), align 2, !tbaa !6
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 24), align 8, !tbaa !6
  store i16 %13, ptr getelementptr inbounds nuw (i8, ptr @d, i32 24), align 8, !tbaa !6
  %14 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 26), align 2, !tbaa !6
  store i16 %14, ptr getelementptr inbounds nuw (i8, ptr @d, i32 26), align 2, !tbaa !6
  %15 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 28), align 4, !tbaa !6
  store i16 %15, ptr getelementptr inbounds nuw (i8, ptr @d, i32 28), align 4, !tbaa !6
  %16 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 30), align 2, !tbaa !6
  store i16 %16, ptr getelementptr inbounds nuw (i8, ptr @d, i32 30), align 2, !tbaa !6
  %17 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 32), align 16, !tbaa !6
  store i16 %17, ptr getelementptr inbounds nuw (i8, ptr @d, i32 32), align 16, !tbaa !6
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 34), align 2, !tbaa !6
  store i16 %18, ptr getelementptr inbounds nuw (i8, ptr @d, i32 34), align 2, !tbaa !6
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 36), align 4, !tbaa !6
  store i16 %19, ptr getelementptr inbounds nuw (i8, ptr @d, i32 36), align 4, !tbaa !6
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 38), align 2, !tbaa !6
  store i16 %20, ptr getelementptr inbounds nuw (i8, ptr @d, i32 38), align 2, !tbaa !6
  %21 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 40), align 8, !tbaa !6
  store i16 %21, ptr getelementptr inbounds nuw (i8, ptr @d, i32 40), align 8, !tbaa !6
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 42), align 2, !tbaa !6
  store i16 %22, ptr getelementptr inbounds nuw (i8, ptr @d, i32 42), align 2, !tbaa !6
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 44), align 4, !tbaa !6
  store i16 %23, ptr getelementptr inbounds nuw (i8, ptr @d, i32 44), align 4, !tbaa !6
  %24 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 46), align 2, !tbaa !6
  store i16 %24, ptr getelementptr inbounds nuw (i8, ptr @d, i32 46), align 2, !tbaa !6
  %25 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 48), align 16, !tbaa !6
  store i16 %25, ptr getelementptr inbounds nuw (i8, ptr @d, i32 48), align 16, !tbaa !6
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 50), align 2, !tbaa !6
  store i16 %26, ptr getelementptr inbounds nuw (i8, ptr @d, i32 50), align 2, !tbaa !6
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 52), align 4, !tbaa !6
  store i16 %27, ptr getelementptr inbounds nuw (i8, ptr @d, i32 52), align 4, !tbaa !6
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 54), align 2, !tbaa !6
  store i16 %28, ptr getelementptr inbounds nuw (i8, ptr @d, i32 54), align 2, !tbaa !6
  %29 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 56), align 8, !tbaa !6
  store i16 %29, ptr getelementptr inbounds nuw (i8, ptr @d, i32 56), align 8, !tbaa !6
  %30 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 58), align 2, !tbaa !6
  store i16 %30, ptr getelementptr inbounds nuw (i8, ptr @d, i32 58), align 2, !tbaa !6
  %31 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 60), align 4, !tbaa !6
  store i16 %31, ptr getelementptr inbounds nuw (i8, ptr @d, i32 60), align 4, !tbaa !6
  %32 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 62), align 2, !tbaa !6
  store i16 %32, ptr getelementptr inbounds nuw (i8, ptr @d, i32 62), align 2, !tbaa !6
  %33 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 64), align 16, !tbaa !6
  store i16 %33, ptr getelementptr inbounds nuw (i8, ptr @d, i32 64), align 16, !tbaa !6
  %34 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 66), align 2, !tbaa !6
  store i16 %34, ptr getelementptr inbounds nuw (i8, ptr @d, i32 66), align 2, !tbaa !6
  %35 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 68), align 4, !tbaa !6
  store i16 %35, ptr getelementptr inbounds nuw (i8, ptr @d, i32 68), align 4, !tbaa !6
  %36 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 70), align 2, !tbaa !6
  store i16 %36, ptr getelementptr inbounds nuw (i8, ptr @d, i32 70), align 2, !tbaa !6
  %37 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 72), align 8, !tbaa !6
  store i16 %37, ptr getelementptr inbounds nuw (i8, ptr @d, i32 72), align 8, !tbaa !6
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 74), align 2, !tbaa !6
  store i16 %38, ptr getelementptr inbounds nuw (i8, ptr @d, i32 74), align 2, !tbaa !6
  %39 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 76), align 4, !tbaa !6
  store i16 %39, ptr getelementptr inbounds nuw (i8, ptr @d, i32 76), align 4, !tbaa !6
  %40 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 78), align 2, !tbaa !6
  store i16 %40, ptr getelementptr inbounds nuw (i8, ptr @d, i32 78), align 2, !tbaa !6
  %41 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 80), align 16, !tbaa !6
  store i16 %41, ptr getelementptr inbounds nuw (i8, ptr @d, i32 80), align 16, !tbaa !6
  %42 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 82), align 2, !tbaa !6
  store i16 %42, ptr getelementptr inbounds nuw (i8, ptr @d, i32 82), align 2, !tbaa !6
  %43 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 84), align 4, !tbaa !6
  store i16 %43, ptr getelementptr inbounds nuw (i8, ptr @d, i32 84), align 4, !tbaa !6
  %44 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 86), align 2, !tbaa !6
  store i16 %44, ptr getelementptr inbounds nuw (i8, ptr @d, i32 86), align 2, !tbaa !6
  ret void
}

attributes #0 = { mustprogress nofree norecurse nosync nounwind optsize willreturn memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"short", !4, i64 0}
