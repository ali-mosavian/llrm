target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%union.anon = type { double }
%union.anon.0 = type { double }

define double @__adddf3(double noundef %0, double noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = bitcast double %0 to i64
  %4 = bitcast double %1 to i64
  %5 = tail call fastcc i64 @add_bits_64(i64 noundef %3, i64 noundef %4) nobuiltin "no-builtins"
  %6 = bitcast i64 %5 to double
  ret double %6
}

define internal fastcc i64 @add_bits_64(i64 noundef %0, i64 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = and i64 %0, 9223372036854775807
  %4 = and i64 %1, 9223372036854775807
  %5 = icmp samesign ult i64 %3, %4
  %6 = select i1 %5, i64 %1, i64 %0
  %7 = select i1 %5, i64 %0, i64 %1
  %8 = shl i64 %6, 1
  %9 = icmp ugt i64 %8, -9007199254740993
  br i1 %9, label %10, label %21

10:
  %11 = trunc i64 %6 to i63
  %12 = icmp ult i63 %11, -4503599627370495
  br i1 %12, label %15, label %13

13:
  %14 = or i64 %6, 2251799813685248
  br label %155

15:
  %16 = xor i64 %1, %0
  %17 = icmp slt i64 %16, 0
  %18 = and i64 %7, 9223372036854775807
  %19 = icmp eq i64 %18, 9218868437227405312
  %or.cond = and i1 %17, %19
  %20 = select i1 %or.cond, i64 9221120237041090560, i64 %6
  br label %155

21:
  %22 = and i64 %7, 9223372036854775807
  %23 = icmp eq i64 %22, 0
  br i1 %23, label %24, label %28

24:
  %25 = icmp eq i64 %8, 0
  %26 = and i64 %1, %0
  %27 = select i1 %25, i64 %26, i64 %6
  br label %155

28:
  %29 = lshr i64 %6, 52
  %30 = trunc nuw nsw i64 %29 to i32
  %31 = and i32 %30, 2047
  %32 = icmp eq i32 %31, 0
  %33 = select i1 %32, i32 1, i32 %31
  %34 = shl i64 %6, 3
  %35 = and i64 %34, 36028797018963960
  %36 = or disjoint i64 %35, 36028797018963968
  %37 = select i1 %32, i64 %35, i64 %36
  %38 = lshr i64 %7, 52
  %39 = trunc nuw nsw i64 %38 to i32
  %40 = and i32 %39, 2047
  %41 = icmp eq i32 %40, 0
  %42 = select i1 %41, i32 1, i32 %40
  %43 = sub nsw i32 %33, %42
  %44 = icmp sgt i32 %43, 54
  br i1 %44, label %155, label %45

45:
  %46 = icmp eq i32 %40, 0
  %47 = shl i64 %7, 3
  %48 = and i64 %47, 36028797018963960
  %49 = or disjoint i64 %48, 36028797018963968
  %50 = select i1 %46, i64 %48, i64 %49
  %51 = icmp slt i32 %43, 1
  br i1 %51, label %61, label %52

52:
  %53 = zext nneg i32 %43 to i64
  %54 = lshr i64 %50, %53
  %55 = sub nuw nsw i32 64, %43
  %56 = zext nneg i32 %55 to i64
  %57 = shl i64 %50, %56
  %58 = icmp ne i64 %57, 0
  %59 = zext i1 %58 to i64
  %60 = or i64 %54, %59
  br label %61

61:
  %62 = phi i64 [ %60, %52 ], [ %50, %45 ]
  %63 = and i64 %6, -9223372036854775808
  %64 = xor i64 %1, %0
  %65 = icmp sgt i64 %64, -1
  br i1 %65, label %142, label %66

66:
  %67 = sub nsw i64 %37, %62
  %68 = icmp eq i64 %37, %62
  br i1 %68, label %155, label %69

69:
  %70 = lshr i64 %67, 32
  %71 = trunc nuw i64 %70 to i32
  %72 = icmp eq i32 %71, 0
  br i1 %72, label %102, label %73

73:
  %74 = icmp ult i32 %71, 65536
  %75 = shl nuw i32 %71, 16
  %76 = select i1 %74, i32 %75, i32 %71
  %77 = select i1 %74, i32 16, i32 0
  %78 = icmp ult i32 %76, 16777216
  br i1 %78, label %79, label %82

79:
  %80 = or disjoint i32 %77, 8
  %81 = shl nuw i32 %76, 8
  br label %82

82:
  %83 = phi i32 [ %76, %73 ], [ %81, %79 ]
  %84 = phi i32 [ %77, %73 ], [ %80, %79 ]
  %85 = icmp ult i32 %83, 268435456
  br i1 %85, label %86, label %89

86:
  %87 = add nuw nsw i32 %84, 4
  %88 = shl nuw i32 %83, 4
  br label %89

89:
  %90 = phi i32 [ %83, %82 ], [ %88, %86 ]
  %91 = phi i32 [ %84, %82 ], [ %87, %86 ]
  %92 = icmp ult i32 %90, 1073741824
  br i1 %92, label %93, label %96

93:
  %94 = add nuw nsw i32 %91, 2
  %95 = shl nuw i32 %90, 2
  br label %96

96:
  %97 = phi i32 [ %90, %89 ], [ %95, %93 ]
  %98 = phi i32 [ %91, %89 ], [ %94, %93 ]
  %99 = icmp sgt i32 %97, -1
  %100 = zext i1 %99 to i32
  %101 = add nuw nsw i32 %98, %100
  br label %133

102:
  %103 = trunc i64 %67 to i32
  %104 = icmp ult i32 %103, 65536
  %105 = shl nuw i32 %103, 16
  %106 = select i1 %104, i32 %105, i32 %103
  %107 = select i1 %104, i32 16, i32 0
  %108 = icmp ult i32 %106, 16777216
  br i1 %108, label %109, label %112

109:
  %110 = or disjoint i32 %107, 8
  %111 = shl nuw i32 %106, 8
  br label %112

112:
  %113 = phi i32 [ %106, %102 ], [ %111, %109 ]
  %114 = phi i32 [ %107, %102 ], [ %110, %109 ]
  %115 = icmp ult i32 %113, 268435456
  br i1 %115, label %116, label %119

116:
  %117 = add nuw nsw i32 %114, 4
  %118 = shl nuw i32 %113, 4
  br label %119

119:
  %120 = phi i32 [ %113, %112 ], [ %118, %116 ]
  %121 = phi i32 [ %114, %112 ], [ %117, %116 ]
  %122 = icmp ult i32 %120, 1073741824
  br i1 %122, label %123, label %126

123:
  %124 = add nuw nsw i32 %121, 2
  %125 = shl nuw i32 %120, 2
  br label %126

126:
  %127 = phi i32 [ %120, %119 ], [ %125, %123 ]
  %128 = phi i32 [ %121, %119 ], [ %124, %123 ]
  %129 = icmp sgt i32 %127, -1
  %130 = zext i1 %129 to i32
  %131 = add nuw nsw i32 %128, 32
  %132 = add nuw nsw i32 %131, %130
  br label %133

133:
  %134 = phi i32 [ %101, %96 ], [ %132, %126 ]
  %135 = add nsw i32 %134, -8
  %136 = icmp slt i32 %135, %33
  %137 = add nsw i32 %33, -1
  %138 = select i1 %136, i32 %135, i32 %137
  %139 = zext nneg i32 %138 to i64
  %140 = shl i64 %67, %139
  %141 = sub nsw i32 %33, %138
  br label %151

142:
  %143 = add nuw nsw i64 %62, %37
  %144 = and i64 %143, 72057594037927936
  %145 = icmp eq i64 %144, 0
  br i1 %145, label %151, label %146

146:
  %147 = lshr i64 %143, 1
  %148 = and i64 %62, 1
  %149 = or i64 %147, %148
  %150 = add nuw nsw i32 %33, 1
  br label %151

151:
  %152 = phi i32 [ %33, %142 ], [ %150, %146 ], [ %141, %133 ]
  %153 = phi i64 [ %140, %133 ], [ %149, %146 ], [ %143, %142 ]
  %154 = tail call i64 @sf64_pack(i64 noundef %63, i32 noundef %152, i64 noundef %153) nobuiltin "no-builtins"
  ret i64 %154

155:
  %156 = phi i64 [ %14, %13 ], [ %20, %15 ], [ %27, %24 ], [ %6, %28 ], [ 0, %66 ]
  ret i64 %156
}

define double @__subdf3(double noundef %0, double noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = bitcast double %0 to i64
  %4 = bitcast double %1 to i64
  %5 = xor i64 %4, -9223372036854775808
  %6 = tail call fastcc i64 @add_bits_64(i64 noundef %3, i64 noundef %5) nobuiltin "no-builtins"
  %7 = bitcast i64 %6 to double
  ret double %7
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
