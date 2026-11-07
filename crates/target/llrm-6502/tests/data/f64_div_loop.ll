target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"

define i64 @__divdf3(i64 noundef %0, i64 noundef %1) nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" {
  %3 = xor i64 %1, %0
  %4 = and i64 %3, -9223372036854775808
  %5 = shl i64 %0, 1
  %6 = icmp ugt i64 %5, -9007199254740993
  %7 = shl i64 %1, 1
  %8 = icmp ugt i64 %7, -9007199254740993
  %9 = or i1 %6, %8
  br i1 %9, label %10, label %26

10:
  %11 = trunc i64 %0 to i63
  %12 = icmp ult i63 %11, -4503599627370495
  br i1 %12, label %15, label %13

13:
  %14 = or i64 %0, 2251799813685248
  br label %126

15:
  %16 = trunc i64 %1 to i63
  %17 = icmp ult i63 %16, -4503599627370495
  br i1 %17, label %20, label %18

18:
  %19 = or i64 %1, 2251799813685248
  br label %126

20:
  %21 = icmp eq i64 %5, -9007199254740992
  br i1 %21, label %22, label %126

22:
  %23 = icmp eq i64 %7, -9007199254740992
  br i1 %23, label %126, label %24

24:
  %25 = or disjoint i64 %4, 9218868437227405312
  br label %126

26:
  %27 = icmp eq i64 %7, 0
  br i1 %27, label %28, label %32

28:
  %29 = icmp eq i64 %5, 0
  br i1 %29, label %126, label %30

30:
  %31 = or disjoint i64 %4, 9218868437227405312
  br label %126

32:
  %33 = icmp eq i64 %5, 0
  br i1 %33, label %126, label %34

34:
  %35 = and i64 %0, 4503599627370495
  %36 = lshr i64 %0, 52
  %37 = trunc nuw nsw i64 %36 to i16
  %38 = and i16 %37, 2047
  %39 = icmp eq i16 %38, 0
  br i1 %39, label %42, label %40

40:
  %41 = or disjoint i64 %35, 4503599627370496
  br label %42

42:
  %43 = phi i64 [ %41, %40 ], [ %35, %34 ]
  %44 = phi i16 [ %38, %40 ], [ 1, %34 ]
  %45 = and i64 %1, 4503599627370495
  %46 = lshr i64 %1, 52
  %47 = trunc nuw nsw i64 %46 to i16
  %48 = and i16 %47, 2047
  %49 = icmp eq i16 %48, 0
  br i1 %49, label %52, label %50

50:
  %51 = or disjoint i64 %45, 4503599627370496
  br label %52

52:
  %53 = phi i64 [ %51, %50 ], [ %45, %42 ]
  %54 = phi i16 [ %48, %50 ], [ 1, %42 ]
  %55 = icmp samesign ult i64 %43, 4503599627370496
  br i1 %55, label %56, label %72

56:
  %57 = lshr i64 %43, 32
  %58 = trunc nuw nsw i64 %57 to i32
  %59 = icmp eq i32 %58, 0
  br i1 %59, label %62, label %60

60:
  %61 = tail call i16 @__clzsi2(i32 noundef %58)
  br label %66

62:
  %63 = trunc i64 %43 to i32
  %64 = tail call i16 @__clzsi2(i32 noundef %63)
  %65 = add nsw i16 %64, 32
  br label %66

66:
  %67 = phi i16 [ %61, %60 ], [ %65, %62 ]
  %68 = add nsw i16 %67, -11
  %69 = zext nneg i16 %68 to i64
  %70 = shl i64 %43, %69
  %71 = sub nsw i16 %44, %68
  br label %72

72:
  %73 = phi i16 [ %71, %66 ], [ %44, %52 ]
  %74 = phi i64 [ %43, %52 ], [ %70, %66 ]
  %75 = icmp samesign ult i64 %53, 4503599627370496
  br i1 %75, label %76, label %92

76:
  %77 = lshr i64 %53, 32
  %78 = trunc nuw nsw i64 %77 to i32
  %79 = icmp eq i32 %78, 0
  br i1 %79, label %82, label %80

80:
  %81 = tail call i16 @__clzsi2(i32 noundef %78)
  br label %86

82:
  %83 = trunc i64 %53 to i32
  %84 = tail call i16 @__clzsi2(i32 noundef %83)
  %85 = add nsw i16 %84, 32
  br label %86

86:
  %87 = phi i16 [ %81, %80 ], [ %85, %82 ]
  %88 = add nsw i16 %87, -11
  %89 = zext nneg i16 %88 to i64
  %90 = shl i64 %53, %89
  %91 = sub nsw i16 %54, %88
  br label %92

92:
  %93 = phi i16 [ %91, %86 ], [ %54, %72 ]
  %94 = phi i64 [ %53, %72 ], [ %90, %86 ]
  br label %99

95:
  %96 = sub nsw i16 %73, %93
  %97 = add nsw i16 %96, 1022
  %98 = icmp ult i64 %109, 72057594037927936
  br i1 %98, label %119, label %114

99:
  %100 = phi i16 [ 0, %92 ], [ %112, %108 ]
  %101 = phi i64 [ %74, %92 ], [ %111, %108 ]
  %102 = phi i64 [ 0, %92 ], [ %109, %108 ]
  %103 = shl i64 %102, 1
  %104 = icmp ult i64 %101, %94
  br i1 %104, label %108, label %105

105:
  %106 = sub nuw i64 %101, %94
  %107 = or disjoint i64 %103, 1
  br label %108

108:
  %109 = phi i64 [ %107, %105 ], [ %103, %99 ]
  %110 = phi i64 [ %106, %105 ], [ %101, %99 ]
  %111 = shl i64 %110, 1
  %112 = add nuw nsw i16 %100, 1
  %113 = icmp samesign ult i16 %100, 56
  br i1 %113, label %99, label %95, !llvm.loop !15

114:
  %115 = lshr i64 %109, 1
  %116 = and i64 %109, 1
  %117 = or i64 %115, %116
  %118 = add nsw i16 %96, 1023
  br label %119

119:
  %120 = phi i64 [ %117, %114 ], [ %109, %95 ]
  %121 = phi i16 [ %118, %114 ], [ %97, %95 ]
  %122 = icmp ne i64 %111, 0
  %123 = zext i1 %122 to i64
  %124 = or i64 %120, %123
  %125 = tail call i64 @sf64_pack(i64 noundef %4, i16 noundef %121, i64 noundef %124)
  ret i64 %125

126:
  %127 = phi i64 [ %14, %13 ], [ %19, %18 ], [ %25, %24 ], [ 9221120237041090560, %22 ], [ %4, %20 ], [ %31, %30 ], [ 9221120237041090560, %28 ], [ %4, %32 ]
  ret i64 %127
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i16 @__clzsi2(i32 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8"

declare i64 @sf64_pack(i64 noundef, i16 noundef, i64 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8"

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
!8 = !{!"long long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"p1 int", !12, i64 0}
!12 = !{!"any pointer", !5, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !5, i64 0}
!15 = distinct !{!15, !16}
!16 = !{!"llvm.loop.mustprogress"}
