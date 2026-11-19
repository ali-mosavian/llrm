target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%union.anon = type { double }
%union.anon.0 = type { double }

define double @__divdf3(double noundef %0, double noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = bitcast double %0 to i64
  %4 = bitcast double %1 to i64
  %5 = xor i64 %4, %3
  %6 = and i64 %5, -9223372036854775808
  %7 = shl i64 %3, 1
  %8 = icmp ugt i64 %7, -9007199254740993
  %9 = shl i64 %4, 1
  %10 = icmp ugt i64 %9, -9007199254740993
  %11 = or i1 %8, %10
  br i1 %11, label %12, label %28

12:
  %13 = trunc i64 %3 to i63
  %14 = icmp ult i63 %13, -4503599627370495
  br i1 %14, label %17, label %15

15:
  %16 = or i64 %3, 2251799813685248
  br label %96

17:
  %18 = trunc i64 %4 to i63
  %19 = icmp ult i63 %18, -4503599627370495
  br i1 %19, label %22, label %20

20:
  %21 = or i64 %4, 2251799813685248
  br label %96

22:
  %23 = icmp eq i64 %7, -9007199254740992
  br i1 %23, label %24, label %96

24:
  %25 = icmp eq i64 %9, -9007199254740992
  %26 = or disjoint i64 %6, 9218868437227405312
  %27 = select i1 %25, i64 9221120237041090560, i64 %26
  br label %96

28:
  %29 = icmp eq i64 %9, 0
  %30 = icmp eq i64 %7, 0
  br i1 %29, label %31, label %34

31:
  %32 = or disjoint i64 %6, 9218868437227405312
  %33 = select i1 %30, i64 9221120237041090560, i64 %32
  br label %96

34:
  br i1 %30, label %96, label %35

35:
  %36 = and i64 %3, 4503599627370495
  %37 = lshr i64 %3, 52
  %38 = trunc nuw nsw i64 %37 to i32
  %39 = and i32 %38, 2047
  %40 = icmp eq i32 %39, 0
  %41 = or disjoint i64 %36, 4503599627370496
  %42 = and i64 %4, 4503599627370495
  %43 = lshr i64 %4, 52
  %44 = trunc nuw nsw i64 %43 to i32
  %45 = and i32 %44, 2047
  %46 = icmp eq i32 %45, 0
  %47 = or disjoint i64 %42, 4503599627370496
  br i1 %40, label %48, label %54

48:
  %49 = tail call fastcc i32 @sf_clz64(i64 noundef %36) nobuiltin "no-builtins"
  %50 = add nsw i32 %49, -11
  %51 = zext nneg i32 %50 to i64
  %52 = shl i64 %36, %51
  %53 = sub nsw i32 12, %49
  br label %54

54:
  %55 = phi i32 [ %53, %48 ], [ %39, %35 ]
  %56 = phi i64 [ %41, %35 ], [ %52, %48 ]
  br i1 %46, label %57, label %63

57:
  %58 = tail call fastcc i32 @sf_clz64(i64 noundef %42) nobuiltin "no-builtins"
  %59 = add nsw i32 %58, -11
  %60 = zext nneg i32 %59 to i64
  %61 = shl i64 %42, %60
  %62 = sub nsw i32 12, %58
  br label %63

63:
  %64 = phi i32 [ %62, %57 ], [ %45, %54 ]
  %65 = phi i64 [ %47, %54 ], [ %61, %57 ]
  br label %70

66:
  %67 = sub nsw i32 %55, %64
  %68 = add nsw i32 %67, 1022
  %69 = icmp ult i64 %80, 72057594037927936
  br i1 %69, label %89, label %84

70:
  %71 = phi i32 [ 0, %63 ], [ %83, %79 ]
  %72 = phi i64 [ %56, %63 ], [ %82, %79 ]
  %73 = phi i64 [ 0, %63 ], [ %80, %79 ]
  %74 = shl i64 %73, 1
  %75 = icmp ult i64 %72, %65
  br i1 %75, label %79, label %76

76:
  %77 = sub nuw i64 %72, %65
  %78 = or disjoint i64 %74, 1
  br label %79

79:
  %80 = phi i64 [ %78, %76 ], [ %74, %70 ]
  %81 = phi i64 [ %77, %76 ], [ %72, %70 ]
  %82 = shl i64 %81, 1
  %83 = add nuw nsw i32 %71, 1
  %exitcond = icmp eq i32 %83, 57
  br i1 %exitcond, label %66, label %70, !llvm.loop !17

84:
  %85 = lshr i64 %80, 1
  %86 = and i64 %80, 1
  %87 = or i64 %85, %86
  %88 = add nsw i32 %67, 1023
  br label %89

89:
  %90 = phi i64 [ %87, %84 ], [ %80, %66 ]
  %91 = phi i32 [ %88, %84 ], [ %68, %66 ]
  %92 = icmp ne i64 %82, 0
  %93 = zext i1 %92 to i64
  %94 = or i64 %90, %93
  %95 = tail call i64 @sf64_pack(i64 noundef %6, i32 noundef %91, i64 noundef %94) nobuiltin "no-builtins"
  br label %96

96:
  %97 = phi i64 [ %16, %15 ], [ %21, %20 ], [ %95, %89 ], [ %6, %34 ], [ %27, %24 ], [ %6, %22 ], [ %33, %31 ]
  %98 = bitcast i64 %97 to double
  ret double %98
}

declare void @llvm.lifetime.start.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal fastcc range(i32 0, 64) i32 @sf_clz64(i64 noundef range(i64 0, 9007199254740992) %0) inlinehint nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %2 = lshr i64 %0, 32
  %3 = trunc nuw nsw i64 %2 to i32
  %4 = icmp eq i32 %3, 0
  br i1 %4, label %34, label %5

5:
  %6 = icmp samesign ult i32 %3, 65536
  %7 = shl nuw i32 %3, 16
  %8 = select i1 %6, i32 %7, i32 %3
  %9 = select i1 %6, i32 16, i32 0
  %10 = icmp ult i32 %8, 16777216
  br i1 %10, label %11, label %14

11:
  %12 = or disjoint i32 %9, 8
  %13 = shl nuw i32 %8, 8
  br label %14

14:
  %15 = phi i32 [ %8, %5 ], [ %13, %11 ]
  %16 = phi i32 [ %9, %5 ], [ %12, %11 ]
  %17 = icmp ult i32 %15, 268435456
  br i1 %17, label %18, label %21

18:
  %19 = add nuw nsw i32 %16, 4
  %20 = shl nuw i32 %15, 4
  br label %21

21:
  %22 = phi i32 [ %15, %14 ], [ %20, %18 ]
  %23 = phi i32 [ %16, %14 ], [ %19, %18 ]
  %24 = icmp ult i32 %22, 1073741824
  br i1 %24, label %25, label %28

25:
  %26 = add nuw nsw i32 %23, 2
  %27 = shl nuw i32 %22, 2
  br label %28

28:
  %29 = phi i32 [ %22, %21 ], [ %27, %25 ]
  %30 = phi i32 [ %23, %21 ], [ %26, %25 ]
  %31 = icmp sgt i32 %29, -1
  %32 = zext i1 %31 to i32
  %33 = add nuw nsw i32 %30, %32
  br label %65

34:
  %35 = trunc i64 %0 to i32
  %36 = icmp ult i32 %35, 65536
  %37 = shl nuw i32 %35, 16
  %38 = select i1 %36, i32 %37, i32 %35
  %39 = select i1 %36, i32 16, i32 0
  %40 = icmp ult i32 %38, 16777216
  br i1 %40, label %41, label %44

41:
  %42 = or disjoint i32 %39, 8
  %43 = shl nuw i32 %38, 8
  br label %44

44:
  %45 = phi i32 [ %38, %34 ], [ %43, %41 ]
  %46 = phi i32 [ %39, %34 ], [ %42, %41 ]
  %47 = icmp ult i32 %45, 268435456
  br i1 %47, label %48, label %51

48:
  %49 = add nuw nsw i32 %46, 4
  %50 = shl nuw i32 %45, 4
  br label %51

51:
  %52 = phi i32 [ %45, %44 ], [ %50, %48 ]
  %53 = phi i32 [ %46, %44 ], [ %49, %48 ]
  %54 = icmp ult i32 %52, 1073741824
  br i1 %54, label %55, label %58

55:
  %56 = add nuw nsw i32 %53, 2
  %57 = shl nuw i32 %52, 2
  br label %58

58:
  %59 = phi i32 [ %52, %51 ], [ %57, %55 ]
  %60 = phi i32 [ %53, %51 ], [ %56, %55 ]
  %61 = icmp sgt i32 %59, -1
  %62 = zext i1 %61 to i32
  %63 = add nuw nsw i32 %60, %62
  %64 = add nuw nsw i32 %63, 32
  br label %65

65:
  %66 = phi i32 [ %33, %28 ], [ %64, %58 ]
  ret i32 %66
}

declare i64 @sf64_pack(i64 noundef, i32 noundef, i64 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

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
!9 = !{!"double", !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long long", !6, i64 0}
!12 = !{!6, !6, i64 0}
!13 = !{!5, !5, i64 0}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 int", !16, i64 0}
!16 = !{!"any pointer", !6, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
