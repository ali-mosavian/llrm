target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%struct.pair = type { i32, i32 }

define void @__udivmod_w(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, ptr noundef nocapture nofree writeonly "llrm.reads"="" "llrm.writes"="0:16" %4) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" nofree nosync norecurse memory(argmem: write) willreturn {
  %6 = or i32 %3, %2
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %185, label %8

8:
  %9 = icmp eq i32 %1, %3
  %10 = icmp ule i32 %1, %3
  %11 = icmp ult i32 %0, %2
  %12 = select i1 %9, i1 %11, i1 %10
  br i1 %12, label %185, label %13

13:
  %14 = icmp eq i32 %3, 0
  br i1 %14, label %44, label %15

15:
  %16 = icmp ult i32 %3, 65536
  %17 = shl nuw i32 %3, 16
  %18 = select i1 %16, i32 %17, i32 %3
  %19 = select i1 %16, i32 16, i32 0
  %20 = icmp ult i32 %18, 16777216
  br i1 %20, label %21, label %24

21:
  %22 = or disjoint i32 %19, 8
  %23 = shl nuw i32 %18, 8
  br label %24

24:
  %25 = phi i32 [ %18, %15 ], [ %23, %21 ]
  %26 = phi i32 [ %19, %15 ], [ %22, %21 ]
  %27 = icmp ult i32 %25, 268435456
  br i1 %27, label %28, label %31

28:
  %29 = add nuw nsw i32 %26, 4
  %30 = shl nuw i32 %25, 4
  br label %31

31:
  %32 = phi i32 [ %25, %24 ], [ %30, %28 ]
  %33 = phi i32 [ %26, %24 ], [ %29, %28 ]
  %34 = icmp ult i32 %32, 1073741824
  br i1 %34, label %35, label %38

35:
  %36 = add nuw nsw i32 %33, 2
  %37 = shl nuw i32 %32, 2
  br label %38

38:
  %39 = phi i32 [ %32, %31 ], [ %37, %35 ]
  %40 = phi i32 [ %33, %31 ], [ %36, %35 ]
  %41 = icmp sgt i32 %39, -1
  %42 = zext i1 %41 to i32
  %43 = add nuw nsw i32 %40, %42
  br label %74

44:
  %45 = icmp ult i32 %2, 65536
  %46 = shl nuw i32 %2, 16
  %47 = select i1 %45, i32 %46, i32 %2
  %48 = select i1 %45, i32 16, i32 0
  %49 = icmp ult i32 %47, 16777216
  br i1 %49, label %50, label %53

50:
  %51 = or disjoint i32 %48, 8
  %52 = shl nuw i32 %47, 8
  br label %53

53:
  %54 = phi i32 [ %47, %44 ], [ %52, %50 ]
  %55 = phi i32 [ %48, %44 ], [ %51, %50 ]
  %56 = icmp ult i32 %54, 268435456
  br i1 %56, label %57, label %60

57:
  %58 = add nuw nsw i32 %55, 4
  %59 = shl nuw i32 %54, 4
  br label %60

60:
  %61 = phi i32 [ %54, %53 ], [ %59, %57 ]
  %62 = phi i32 [ %55, %53 ], [ %58, %57 ]
  %63 = icmp ult i32 %61, 1073741824
  br i1 %63, label %64, label %67

64:
  %65 = add nuw nsw i32 %62, 2
  %66 = shl nuw i32 %61, 2
  br label %67

67:
  %68 = phi i32 [ %61, %60 ], [ %66, %64 ]
  %69 = phi i32 [ %62, %60 ], [ %65, %64 ]
  %70 = icmp sgt i32 %68, -1
  %71 = zext i1 %70 to i32
  %72 = add nuw nsw i32 %69, 32
  %73 = add nuw nsw i32 %72, %71
  br label %74

74:
  %75 = phi i32 [ %43, %38 ], [ %73, %67 ]
  %76 = icmp eq i32 %1, 0
  br i1 %76, label %106, label %77

77:
  %78 = icmp ult i32 %1, 65536
  %79 = shl nuw i32 %1, 16
  %80 = select i1 %78, i32 %79, i32 %1
  %81 = select i1 %78, i32 16, i32 0
  %82 = icmp ult i32 %80, 16777216
  br i1 %82, label %83, label %86

83:
  %84 = or disjoint i32 %81, 8
  %85 = shl nuw i32 %80, 8
  br label %86

86:
  %87 = phi i32 [ %80, %77 ], [ %85, %83 ]
  %88 = phi i32 [ %81, %77 ], [ %84, %83 ]
  %89 = icmp ult i32 %87, 268435456
  br i1 %89, label %90, label %93

90:
  %91 = add nuw nsw i32 %88, 4
  %92 = shl nuw i32 %87, 4
  br label %93

93:
  %94 = phi i32 [ %87, %86 ], [ %92, %90 ]
  %95 = phi i32 [ %88, %86 ], [ %91, %90 ]
  %96 = icmp ult i32 %94, 1073741824
  br i1 %96, label %97, label %100

97:
  %98 = add nuw nsw i32 %95, 2
  %99 = shl nuw i32 %94, 2
  br label %100

100:
  %101 = phi i32 [ %94, %93 ], [ %99, %97 ]
  %102 = phi i32 [ %95, %93 ], [ %98, %97 ]
  %103 = icmp sgt i32 %101, -1
  %104 = zext i1 %103 to i32
  %105 = add nuw nsw i32 %102, %104
  br label %136

106:
  %107 = icmp ult i32 %0, 65536
  %108 = shl nuw i32 %0, 16
  %109 = select i1 %107, i32 %108, i32 %0
  %110 = select i1 %107, i32 16, i32 0
  %111 = icmp ult i32 %109, 16777216
  br i1 %111, label %112, label %115

112:
  %113 = or disjoint i32 %110, 8
  %114 = shl nuw i32 %109, 8
  br label %115

115:
  %116 = phi i32 [ %109, %106 ], [ %114, %112 ]
  %117 = phi i32 [ %110, %106 ], [ %113, %112 ]
  %118 = icmp ult i32 %116, 268435456
  br i1 %118, label %119, label %122

119:
  %120 = add nuw nsw i32 %117, 4
  %121 = shl nuw i32 %116, 4
  br label %122

122:
  %123 = phi i32 [ %116, %115 ], [ %121, %119 ]
  %124 = phi i32 [ %117, %115 ], [ %120, %119 ]
  %125 = icmp ult i32 %123, 1073741824
  br i1 %125, label %126, label %129

126:
  %127 = add nuw nsw i32 %124, 2
  %128 = shl nuw i32 %123, 2
  br label %129

129:
  %130 = phi i32 [ %123, %122 ], [ %128, %126 ]
  %131 = phi i32 [ %124, %122 ], [ %127, %126 ]
  %132 = icmp sgt i32 %130, -1
  %133 = zext i1 %132 to i32
  %134 = add nuw nsw i32 %131, 32
  %135 = add nuw nsw i32 %134, %133
  br label %136

136:
  %137 = phi i32 [ %105, %100 ], [ %135, %129 ]
  %138 = sub nsw i32 %75, %137
  tail call void @llvm.experimental.noalias.scope.decl(metadata !32)
  %139 = icmp sgt i32 %138, 31
  br i1 %139, label %140, label %143

140:
  %141 = add nsw i32 %138, -32
  %142 = shl i32 %2, %141
  br label %152

143:
  %144 = icmp eq i32 %75, %137
  br i1 %144, label %152, label %145

145:
  %146 = shl i32 %2, %138
  %147 = shl i32 %3, %138
  %148 = sub nsw i32 32, %138
  %149 = lshr i32 %2, %148
  %150 = or disjoint i32 %149, %147
  %151 = icmp slt i32 %138, 0
  br i1 %151, label %185, label %152

152:
  %153 = phi i32 [ %150, %145 ], [ %142, %140 ], [ %3, %143 ]
  %154 = phi i32 [ %146, %145 ], [ 0, %140 ], [ %2, %143 ]
  %155 = add i32 %75, 1
  %156 = sub i32 %155, %137
  br label %157

157:
  %158 = phi i32 [ %184, %178 ], [ %156, %152 ]
  %159 = phi i32 [ %181, %178 ], [ %1, %152 ]
  %160 = phi i32 [ %180, %178 ], [ %0, %152 ]
  %161 = phi i32 [ %183, %178 ], [ %153, %152 ]
  %162 = phi i32 [ %182, %178 ], [ %154, %152 ]
  %163 = phi i32 [ %179, %178 ], [ 0, %152 ]
  %164 = phi i32 [ %166, %178 ], [ 0, %152 ]
  tail call void @llvm.experimental.noalias.scope.decl(metadata !28)
  %165 = shl i32 %163, 1
  %166 = tail call i32 @llvm.fshl.i32(i32 %164, i32 %163, i32 1)
  %167 = icmp eq i32 %159, %161
  %168 = icmp ule i32 %159, %161
  %169 = icmp ult i32 %160, %162
  %170 = select i1 %167, i1 %169, i1 %168
  br i1 %170, label %178, label %171

171:
  %172 = icmp ult i32 %160, %162
  tail call void @llvm.experimental.noalias.scope.decl(metadata !24)
  %173 = sub i32 %160, %162
  %174 = sext i1 %172 to i32
  %175 = sub i32 %159, %161
  %176 = add i32 %175, %174
  %177 = or disjoint i32 %165, 1
  br label %178

178:
  %179 = phi i32 [ %165, %157 ], [ %177, %171 ]
  %180 = phi i32 [ %160, %157 ], [ %173, %171 ]
  %181 = phi i32 [ %159, %157 ], [ %176, %171 ]
  tail call void @llvm.experimental.noalias.scope.decl(metadata !20)
  %182 = tail call i32 @llvm.fshl.i32(i32 %161, i32 %162, i32 31), !noalias !21
  %183 = lshr i32 %161, 1
  %184 = add i32 %158, -1
  %exitcond = icmp eq i32 %184, 0
  br i1 %exitcond, label %185, label %157, !llvm.loop !16

185:
  %186 = phi i32 [ 0, %8 ], [ -1, %5 ], [ 0, %145 ], [ %166, %178 ]
  %187 = phi i32 [ 0, %8 ], [ -1, %5 ], [ 0, %145 ], [ %179, %178 ]
  %188 = phi i32 [ %0, %8 ], [ %0, %5 ], [ %0, %145 ], [ %180, %178 ]
  %189 = phi i32 [ %1, %8 ], [ %1, %5 ], [ %1, %145 ], [ %181, %178 ]
  store i32 %187, ptr %4, align 2, !tbaa !8
  %190 = getelementptr inbounds nuw i8, ptr %4, i32 4
  store i32 %186, ptr %190, align 2, !tbaa !8
  %191 = getelementptr inbounds nuw i8, ptr %4, i32 8
  store i32 %188, ptr %191, align 2, !tbaa !8
  %192 = getelementptr inbounds nuw i8, ptr %4, i32 12
  store i32 %189, ptr %192, align 2, !tbaa !8
  ret void
}

declare void @llvm.lifetime.start.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p0.i32(ptr nocapture writeonly, i8, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @llvm.lifetime.end.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i32 @llvm.fshl.i32(i32, i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.experimental.noalias.scope.decl(metadata) nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: readwrite)

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
!8 = !{!5, !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 int", !11, i64 0}
!11 = !{!"any pointer", !6, i64 0}
!12 = !{!13, !5, i64 0}
!13 = !{!"", !5, i64 0, !5, i64 4}
!14 = !{!13, !5, i64 4}
!15 = !{i64 0, i64 4, !8, i64 4, i64 4, !8}
!16 = distinct !{!16, !17}
!17 = !{!"llvm.loop.mustprogress"}
!18 = distinct !{!18, !"inlined"}
!19 = distinct !{!19, !18, !"inlined: argument 0"}
!20 = !{!19}
!21 = !{!19}
!22 = distinct !{!22, !"inlined"}
!23 = distinct !{!23, !22, !"inlined: argument 0"}
!24 = !{!23}
!25 = !{!23}
!26 = distinct !{!26, !"inlined"}
!27 = distinct !{!27, !26, !"inlined: argument 0"}
!28 = !{!27}
!29 = !{!27}
!30 = distinct !{!30, !"inlined"}
!31 = distinct !{!31, !30, !"inlined: argument 0"}
!32 = !{!31}
!33 = !{!31}
