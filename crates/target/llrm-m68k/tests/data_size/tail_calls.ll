target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@sink = global [4 x i32] zeroinitializer, align 2
@chosen = global ptr @forty_two, align 2
@in = global [10 x i32] [i32 1, i32 2, i32 3, i32 4, i32 5, i32 6, i32 7, i32 41, i32 40, i32 20], align 2

define i32 @add3(i32 noundef %0, i32 noundef %1, i32 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %4 = shl nsw i32 %1, 1
  %5 = add nsw i32 %4, %0
  %6 = mul nsw i32 %2, 3
  %7 = add nsw i32 %5, %6
  ret i32 %7
}

define i32 @same(i32 noundef %0, i32 noundef %1, i32 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %4 = shl nsw i32 %1, 1
  %5 = add nsw i32 %4, %0
  %6 = mul nsw i32 %2, 3
  %7 = add nsw i32 %5, %6
  ret i32 %7
}

define range(i32 -2147483627, -2147483648) i32 @extra(i32 noundef %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %3 = shl nsw i32 %1, 1
  %4 = add i32 %0, 21
  %5 = add i32 %4, %3
  ret i32 %5
}

define i32 @swapped(i32 noundef %0, i32 noundef %1, i32 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %4 = shl nsw i32 %1, 1
  %5 = mul nsw i32 %0, 3
  %6 = add i32 %2, %5
  %7 = add i32 %6, %4
  ret i32 %7
}

define i32 @mixed(i32 noundef %0, i32 noundef %1, i32 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %4 = shl nsw i32 %2, 1
  %5 = mul nsw i32 %1, 3
  %6 = add i32 %5, %0
  %7 = add i32 %6, %4
  %8 = xor i32 %7, 85
  ret i32 %8
}

define i32 @framed(i32 noundef %0, i32 noundef %1, i32 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(readwrite, argmem: none) nofree nosync norecurse willreturn {
  %4 = mul nsw i32 %0, 3
  %5 = add nsw i32 %4, %1
  %6 = mul nsw i32 %0, 5
  %7 = add nsw i32 %6, %1
  %8 = add nsw i32 %7, %5
  store volatile i32 %8, ptr @sink, align 2, !tbaa !8
  %9 = shl nsw i32 %1, 1
  %10 = add nsw i32 %9, %0
  %11 = mul nsw i32 %2, 3
  %12 = add nsw i32 %10, %11
  ret i32 %12
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @odd(i32 noundef %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) nofree nosync norecurse {
  br label %tailrecurse

tailrecurse:
  %3 = phi i32 [ %0, %2 ], [ %11, %10 ]
  %4 = phi i32 [ %1, %2 ], [ %13, %10 ]
  %5 = icmp eq i32 %3, 0
  br i1 %5, label %14, label %6

6:
  %7 = icmp eq i32 %3, 1
  br i1 %7, label %8, label %10

8:
  %9 = add nsw i32 %4, 4
  br label %14

10:
  %11 = add nsw i32 %3, -2
  %12 = shl i32 %4, 1
  %13 = add i32 %12, 7
  br label %tailrecurse

14:
  %15 = phi i32 [ %9, %8 ], [ %4, %tailrecurse ]
  ret i32 %15
}

define i32 @even(i32 noundef %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) nofree nosync norecurse {
  %3 = icmp eq i32 %0, 0
  br i1 %3, label %4, label %6

4:
  %5 = add nsw i32 %1, 1
  br label %22

6:
  %7 = add nsw i32 %0, -1
  %8 = shl nsw i32 %1, 1
  %9 = or disjoint i32 %8, 1
  br label %10

10:
  %11 = phi i32 [ %7, %6 ], [ %19, %18 ]
  %12 = phi i32 [ %9, %6 ], [ %21, %18 ]
  %13 = icmp eq i32 %11, 0
  br i1 %13, label %22, label %14

14:
  %15 = icmp eq i32 %11, 1
  br i1 %15, label %16, label %18

16:
  %17 = add nsw i32 %12, 4
  br label %22

18:
  %19 = add nsw i32 %11, -2
  %20 = shl i32 %12, 1
  %21 = add i32 %20, 7
  br label %10

22:
  %23 = phi i32 [ %5, %4 ], [ %17, %16 ], [ %12, %10 ]
  ret i32 %23
}

define i32 @countdown(i32 noundef %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) nofree nosync willreturn norecurse {
  %3 = icmp eq i32 %0, 0
  br i1 %3, label %18, label %4

4:
  %5 = sub i32 -1, %0
  br label %6

6:
  %7 = phi i32 [ %5, %4 ], [ %13, %6 ]
  %8 = phi i32 [ %12, %6 ], [ %1, %4 ]
  %9 = phi i32 [ %10, %6 ], [ %0, %4 ]
  %10 = add nsw i32 %9, -1
  %11 = mul nsw i32 %8, 3
  %12 = add nsw i32 %11, %9
  %13 = add i32 %7, 1
  %14 = icmp eq i32 %13, -1
  br i1 %14, label %15, label %6

15:
  %16 = mul i32 %8, 3
  %17 = sub i32 %16, %13
  br label %18

18:
  %19 = phi i32 [ %17, %15 ], [ %1, %2 ]
  ret i32 %19
}

define i32 @forty_two() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  ret i32 42
}

define i32 @through_pointer() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = load volatile ptr, ptr @chosen, align 2, !tbaa !12
  %2 = tail call i32 %1() nobuiltin "no-builtins"
  ret i32 %2
}

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = load volatile i32, ptr @in, align 2, !tbaa !8
  %2 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 4), align 2, !tbaa !8
  %3 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 8), align 2, !tbaa !8
  %4 = shl nsw i32 %2, 1
  %5 = add nsw i32 %4, %1
  %6 = mul nsw i32 %3, 3
  %7 = add nsw i32 %5, %6
  tail call void @report(i32 noundef %7) nobuiltin "no-builtins"
  %8 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 12), align 2, !tbaa !8
  %9 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 16), align 2, !tbaa !8
  %10 = shl nsw i32 %9, 1
  %11 = add i32 %8, 21
  %12 = add i32 %11, %10
  tail call void @report(i32 noundef %12) nobuiltin "no-builtins"
  %13 = load volatile i32, ptr @in, align 2, !tbaa !8
  %14 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 4), align 2, !tbaa !8
  %15 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 8), align 2, !tbaa !8
  %16 = shl nsw i32 %14, 1
  %17 = mul nsw i32 %13, 3
  %18 = add i32 %15, %17
  %19 = add i32 %18, %16
  tail call void @report(i32 noundef %19) nobuiltin "no-builtins"
  %20 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 4), align 2, !tbaa !8
  %21 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 8), align 2, !tbaa !8
  %22 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 12), align 2, !tbaa !8
  %23 = shl nsw i32 %22, 1
  %24 = mul nsw i32 %21, 3
  %25 = add i32 %24, %20
  %26 = add i32 %25, %23
  %27 = xor i32 %26, 85
  tail call void @report(i32 noundef %27) nobuiltin "no-builtins"
  %28 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 16), align 2, !tbaa !8
  %29 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 20), align 2, !tbaa !8
  %30 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 24), align 2, !tbaa !8
  %31 = shl i32 %29, 1
  %32 = shl i32 %28, 3
  %33 = add i32 %31, %32
  store volatile i32 %33, ptr @sink, align 2, !tbaa !8
  %34 = shl nsw i32 %29, 1
  %35 = add nsw i32 %34, %28
  %36 = mul nsw i32 %30, 3
  %37 = add nsw i32 %35, %36
  tail call void @report(i32 noundef %37) nobuiltin "no-builtins"
  %38 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 28), align 2, !tbaa !8
  %39 = icmp eq i32 %38, 0
  br i1 %39, label %54, label %40

40:
  %41 = add nsw i32 %38, -1
  br label %42

42:
  %43 = phi i32 [ %41, %40 ], [ %51, %50 ]
  %44 = phi i32 [ 1, %40 ], [ %53, %50 ]
  %45 = icmp eq i32 %43, 0
  br i1 %45, label %54, label %46

46:
  %47 = icmp eq i32 %43, 1
  br i1 %47, label %48, label %50

48:
  %49 = add nsw i32 %44, 4
  br label %54

50:
  %51 = add nsw i32 %43, -2
  %52 = shl i32 %44, 1
  %53 = add i32 %52, 7
  br label %42

54:
  %55 = phi i32 [ %49, %48 ], [ 1, %0 ], [ %44, %42 ]
  tail call void @report(i32 noundef %55) nobuiltin "no-builtins"
  %56 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 32), align 2, !tbaa !8
  br label %57

57:
  %58 = phi i32 [ %56, %54 ], [ %66, %65 ]
  %59 = phi i32 [ 1, %54 ], [ %68, %65 ]
  %60 = icmp eq i32 %58, 0
  br i1 %60, label %69, label %61

61:
  %62 = icmp eq i32 %58, 1
  br i1 %62, label %63, label %65

63:
  %64 = add nsw i32 %59, 4
  br label %69

65:
  %66 = add nsw i32 %58, -2
  %67 = shl i32 %59, 1
  %68 = add i32 %67, 7
  br label %57

69:
  %70 = phi i32 [ %64, %63 ], [ %59, %57 ]
  tail call void @report(i32 noundef %70) nobuiltin "no-builtins"
  %71 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i32 36), align 2, !tbaa !8
  %72 = icmp eq i32 %71, 0
  br i1 %72, label %87, label %73

73:
  %74 = sub i32 -1, %71
  br label %75

75:
  %76 = phi i32 [ %74, %73 ], [ %82, %75 ]
  %77 = phi i32 [ %81, %75 ], [ 1, %73 ]
  %78 = phi i32 [ %79, %75 ], [ %71, %73 ]
  %79 = add nsw i32 %78, -1
  %80 = mul nsw i32 %77, 3
  %81 = add nsw i32 %80, %78
  %82 = add i32 %76, 1
  %83 = icmp eq i32 %82, -1
  br i1 %83, label %84, label %75

84:
  %85 = mul i32 %77, 3
  %86 = sub i32 %85, %82
  br label %87

87:
  %88 = phi i32 [ %86, %84 ], [ 1, %69 ]
  %89 = and i32 %88, 16777215
  tail call void @report(i32 noundef %89) nobuiltin "no-builtins"
  %90 = load volatile ptr, ptr @chosen, align 2, !tbaa !12
  %91 = tail call i32 %90() nobuiltin "no-builtins"
  tail call void @report(i32 noundef %91) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!13, !13, i64 0}
!13 = !{!"any pointer", !6, i64 0}
