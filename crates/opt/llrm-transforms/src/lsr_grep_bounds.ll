target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00", align 1
@_hit = internal unnamed_addr global [80 x i8] zeroinitializer, align 1
@_classof = internal unnamed_addr global [256 x i8] zeroinitializer, align 1
@_nxt = internal unnamed_addr global [2560 x i8] zeroinitializer, align 1
@_queue = internal unnamed_addr global [80 x i8] zeroinitializer, align 1
@_fail = internal unnamed_addr global [80 x i8] zeroinitializer, align 1
@_bench_grep_via = global ptr addrspace(1) @"_bench_grep@3", align 2

define cc 1005 i32 @"_bench_grep@3"(ptr noundef nocapture nofree writeonly "llrm.reads"="" "llrm.writes"="0:4" %0) addrspace(1) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" {
  %2 = alloca [1024 x i8], align 1
  call addrspace(1) void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call addrspace(1) void @llvm.memset.p0.i16(ptr nonnull align 1 %2, i8 0, i16 1024, i1 false)
  br label %3

3:
  %4 = phi ptr [ @_patterns, %1 ], [ %59, %54 ]
  %5 = phi i16 [ 1, %1 ], [ %55, %54 ]
  %6 = phi i8 [ 1, %1 ], [ %56, %54 ]
  %7 = phi i8 [ 1, %1 ], [ %57, %54 ]
  %8 = phi i16 [ 0, %1 ], [ %58, %54 ]
  %9 = load i8, ptr %4, align 1, !tbaa !16
  switch i8 %9, label %20 [
    i8 0, label %10
    i8 124, label %15
  ]

10:
  %11 = icmp sgt i16 %5, 1
  br i1 %11, label %12, label %82

12:
  %13 = load i8, ptr @_nxt, align 1, !tbaa !16
  %14 = icmp eq i8 %13, 0
  br label %62

15:
  %16 = icmp samesign ult i16 %8, 80
  %17 = select i1 %16, i16 %8, i16 0
  %18 = getelementptr inbounds nuw i8, ptr @_hit, i16 %17
  store i8 %7, ptr %18, align 1, !tbaa !16
  %19 = add i8 %7, 1
  br label %54

20:
  %21 = zext i8 %9 to i16
  %22 = getelementptr inbounds nuw i8, ptr @_classof, i16 %21
  %23 = load i8, ptr %22, align 1, !tbaa !16
  %24 = icmp eq i8 %23, 0
  br i1 %24, label %25, label %28

25:
  %26 = add nsw i16 %5, 1
  %27 = trunc i16 %5 to i8
  store i8 %27, ptr %22, align 1, !tbaa !16
  br label %28

28:
  %29 = phi i8 [ %23, %20 ], [ %27, %25 ]
  %30 = phi i16 [ %5, %20 ], [ %26, %25 ]
  %31 = shl nuw nsw i16 %8, 5
  %32 = zext i8 %29 to i16
  %33 = add nuw nsw i16 %31, %32
  %34 = icmp samesign ult i16 %33, 2560
  br i1 %34, label %35, label %39

35:
  %36 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %33
  %37 = load i8, ptr %36, align 1, !tbaa !16
  %38 = icmp eq i8 %37, 0
  br i1 %38, label %42, label %46

39:
  %40 = load i8, ptr @_nxt, align 1, !tbaa !16
  %41 = icmp eq i8 %40, 0
  br i1 %41, label %42, label %50

42:
  %43 = phi i16 [ %33, %35 ], [ 0, %39 ]
  %44 = add i8 %6, 1
  %45 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %43
  store i8 %6, ptr %45, align 1, !tbaa !16
  br label %46

46:
  %47 = phi i8 [ %6, %35 ], [ %44, %42 ]
  %48 = select i1 %34, i16 %33, i16 0
  %.phi.trans.insert = getelementptr inbounds nuw i8, ptr @_nxt, i16 %48
  %49 = load i8, ptr %.phi.trans.insert, align 1, !tbaa !16
  br label %50

50:
  %51 = phi i8 [ %49, %46 ], [ %40, %39 ]
  %52 = phi i8 [ %47, %46 ], [ %6, %39 ]
  %53 = zext i8 %51 to i16
  br label %54

54:
  %55 = phi i16 [ %5, %15 ], [ %30, %50 ]
  %56 = phi i8 [ %6, %15 ], [ %52, %50 ]
  %57 = phi i8 [ %19, %15 ], [ %7, %50 ]
  %58 = phi i16 [ 0, %15 ], [ %53, %50 ]
  %59 = getelementptr inbounds nuw i8, ptr %4, i16 1
  br label %3

60:
  %61 = icmp sgt i16 %78, 0
  br i1 %61, label %85, label %82

62:
  %63 = phi i16 [ 1, %12 ], [ %79, %77 ]
  %64 = phi i16 [ 0, %12 ], [ %78, %77 ]
  %65 = icmp samesign ult i16 %63, 2560
  br i1 %65, label %66, label %70

66:
  %67 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %63
  %68 = load i8, ptr %67, align 1, !tbaa !16
  %69 = icmp eq i8 %68, 0
  br i1 %69, label %77, label %71

70:
  br i1 %14, label %77, label %71

71:
  %72 = phi i8 [ %68, %66 ], [ %13, %70 ]
  %73 = icmp ult i16 %64, 80
  %74 = select i1 %73, i16 %64, i16 0
  %75 = getelementptr inbounds nuw i8, ptr @_queue, i16 %74
  store i8 %72, ptr %75, align 1, !tbaa !16
  %76 = add nsw i16 %64, 1
  br label %77

77:
  %78 = phi i16 [ %76, %71 ], [ %64, %66 ], [ %64, %70 ]
  %79 = add nuw nsw i16 %63, 1
  %exitcond = icmp eq i16 %79, %5
  br i1 %exitcond, label %60, label %62

80:
  %81 = icmp slt i16 %93, %150
  br i1 %81, label %85, label %82

82:
  %83 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %2, i16 noundef 1024)
  %84 = icmp sgt i16 %83, 0
  br i1 %84, label %155, label %214

85:
  %86 = phi i16 [ %150, %80 ], [ %78, %60 ]
  %87 = phi i16 [ %93, %80 ], [ 0, %60 ]
  %88 = icmp samesign ult i16 %87, 80
  %89 = select i1 %88, i16 %87, i16 0
  %90 = getelementptr inbounds nuw i8, ptr @_queue, i16 %89
  %91 = load i8, ptr %90, align 1, !tbaa !16
  %92 = zext i8 %91 to i16
  %93 = add nuw nsw i16 %87, 1
  %94 = shl nuw nsw i16 %92, 5
  %95 = icmp ult i8 %91, 80
  %96 = getelementptr inbounds nuw i8, ptr @_fail, i16 %92
  br label %97

97:
  %98 = phi i16 [ 0, %85 ], [ %151, %149 ]
  %99 = phi i16 [ %86, %85 ], [ %150, %149 ]
  %100 = add nuw nsw i16 %98, %94
  %101 = icmp samesign ult i16 %100, 2560
  %102 = select i1 %101, i16 %100, i16 0
  %103 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %102
  %104 = load i8, ptr %103, align 1, !tbaa !16
  %105 = zext i8 %104 to i16
  br i1 %95, label %106, label %112

106:
  %107 = load i8, ptr %96, align 1, !tbaa !16
  %108 = zext i8 %107 to i16
  %109 = shl nuw nsw i16 %108, 5
  %110 = add nuw nsw i16 %109, %98
  %111 = icmp samesign ult i16 %110, 2560
  br i1 %111, label %118, label %120

112:
  %113 = load i8, ptr @_fail, align 1, !tbaa !16
  %114 = zext i8 %113 to i16
  %115 = shl nuw nsw i16 %114, 5
  %116 = add nuw nsw i16 %115, %98
  %117 = icmp samesign ult i16 %116, 2560
  br i1 %117, label %118, label %120

118:
  %119 = phi i16 [ %116, %112 ], [ %110, %106 ]
  br label %120

120:
  %121 = phi i16 [ %119, %118 ], [ 0, %112 ], [ 0, %106 ]
  %122 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %121
  %123 = load i8, ptr %122, align 1, !tbaa !16
  %124 = icmp eq i8 %104, 0
  br i1 %124, label %148, label %125

125:
  %126 = icmp ult i8 %104, 80
  br i1 %126, label %128, label %127

127:
  store i8 %123, ptr @_fail, align 1, !tbaa !16
  br label %130

128:
  %129 = getelementptr inbounds nuw i8, ptr @_fail, i16 %105
  store i8 %123, ptr %129, align 1, !tbaa !16
  br label %130

130:
  %131 = phi i16 [ 0, %127 ], [ %105, %128 ]
  %132 = getelementptr inbounds nuw i8, ptr @_hit, i16 %131
  %133 = load i8, ptr %132, align 1, !tbaa !16
  %134 = icmp eq i8 %133, 0
  br i1 %134, label %135, label %143

135:
  %136 = icmp ult i8 %123, 80
  %137 = select i1 %136, i8 %123, i8 0
  %138 = zext i8 %137 to i16
  %139 = getelementptr inbounds nuw i8, ptr @_hit, i16 %138
  %140 = load i8, ptr %139, align 1, !tbaa !16
  %141 = select i1 %126, i16 %105, i16 0
  %142 = getelementptr inbounds nuw i8, ptr @_hit, i16 %141
  store i8 %140, ptr %142, align 1, !tbaa !16
  br label %143

143:
  %144 = icmp ult i16 %99, 80
  %145 = select i1 %144, i16 %99, i16 0
  %146 = getelementptr inbounds nuw i8, ptr @_queue, i16 %145
  store i8 %104, ptr %146, align 1, !tbaa !16
  %147 = add nuw nsw i16 %99, 1
  br label %149

148:
  store i8 %123, ptr %103, align 1, !tbaa !16
  br label %149

149:
  %150 = phi i16 [ %147, %143 ], [ %99, %148 ]
  %151 = add nuw nsw i16 %98, 1
  %exitcond1 = icmp eq i16 %151, %5
  br i1 %exitcond1, label %80, label %97

152:
  %153 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %2, i16 noundef 1024)
  %154 = icmp sgt i16 %153, 0
  br i1 %154, label %155, label %211

155:
  %156 = phi i16 [ %153, %152 ], [ %83, %82 ]
  %157 = phi i32 [ %209, %152 ], [ 0, %82 ]
  %158 = phi i32 [ %201, %152 ], [ 0, %82 ]
  %159 = phi i32 [ %208, %152 ], [ 0, %82 ]
  %160 = phi i16 [ %207, %152 ], [ 0, %82 ]
  %161 = phi i16 [ %185, %152 ], [ 0, %82 ]
  %162 = load i8, ptr @_hit, align 1, !tbaa !16
  %163 = icmp eq i8 %162, 0
  br label %164

164:
  %165 = phi i32 [ %157, %155 ], [ %209, %206 ]
  %166 = phi i32 [ %158, %155 ], [ %201, %206 ]
  %167 = phi i32 [ %159, %155 ], [ %208, %206 ]
  %168 = phi i16 [ %160, %155 ], [ %207, %206 ]
  %169 = phi i16 [ 0, %155 ], [ %210, %206 ]
  %170 = phi i16 [ %161, %155 ], [ %185, %206 ]
  %171 = icmp samesign ult i16 %169, 1024
  %172 = select i1 %171, i16 %169, i16 0
  %173 = getelementptr inbounds nuw i8, ptr %2, i16 %172
  %174 = load i8, ptr %173, align 1, !tbaa !16
  %175 = shl nuw nsw i16 %170, 5
  %176 = zext i8 %174 to i16
  %177 = getelementptr inbounds nuw i8, ptr @_classof, i16 %176
  %178 = load i8, ptr %177, align 1, !tbaa !16
  %179 = zext i8 %178 to i16
  %180 = add nuw nsw i16 %175, %179
  %181 = icmp samesign ult i16 %180, 2560
  %182 = select i1 %181, i16 %180, i16 0
  %183 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %182
  %184 = load i8, ptr %183, align 1, !tbaa !16
  %185 = zext i8 %184 to i16
  %186 = icmp ult i8 %184, 80
  br i1 %186, label %187, label %191

187:
  %188 = getelementptr inbounds nuw i8, ptr @_hit, i16 %185
  %189 = load i8, ptr %188, align 1, !tbaa !16
  %190 = icmp eq i8 %189, 0
  br i1 %190, label %199, label %192

191:
  br i1 %163, label %199, label %192

192:
  %193 = phi i8 [ %189, %187 ], [ %162, %191 ]
  %194 = shl nuw nsw i32 %166, 1
  %195 = add nsw i32 %194, %165
  %196 = zext i8 %193 to i32
  %197 = add nsw i32 %195, %196
  %198 = and i32 %197, 536870911
  br label %199

199:
  %200 = phi i16 [ 1, %192 ], [ %168, %187 ], [ %168, %191 ]
  %201 = phi i32 [ %198, %192 ], [ %166, %187 ], [ %166, %191 ]
  %202 = icmp eq i8 %174, 10
  br i1 %202, label %203, label %206

203:
  %204 = zext nneg i16 %200 to i32
  %205 = add nsw i32 %167, %204
  br label %206

206:
  %207 = phi i16 [ 0, %203 ], [ %200, %199 ]
  %208 = phi i32 [ %205, %203 ], [ %167, %199 ]
  %209 = add nsw i32 %165, 1
  %210 = add nuw nsw i16 %169, 1
  %exitcond2 = icmp eq i16 %210, %156
  br i1 %exitcond2, label %152, label %164

211:
  %212 = zext nneg i16 %207 to i32
  %213 = add nsw i32 %208, %212
  br label %214

214:
  %215 = phi i32 [ 0, %82 ], [ %201, %211 ]
  %216 = phi i32 [ %213, %211 ], [ 0, %82 ]
  store i32 %215, ptr %0, align 2, !tbaa !12
  call addrspace(1) void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret i32 %216
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) addrspace(1) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) addrspace(1) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1005 void @"_ebnd@3"() addrspace(1) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" memory(none) willreturn nofree nosync norecurse {
  ret void
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) addrspace(1) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 i16 @"_input_read@3"(ptr noundef, i16 noundef) addrspace(1) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486"

define cc 1002 i16 @main_() addrspace(1) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" {
  %1 = alloca i32, align 2
  call addrspace(1) void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  %2 = load volatile ptr addrspace(1), ptr @_bench_grep_via, align 2, !tbaa !17
  %3 = call cc 1005 addrspace(1) i32 %2(ptr noundef nonnull %1)
  call cc 1005 addrspace(1) void @"_report@3"(i32 noundef %3)
  %4 = load i32, ptr %1, align 2, !tbaa !12
  call cc 1005 addrspace(1) void @"_report@3"(i32 noundef %4)
  call addrspace(1) void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret i16 0
}

declare cc 1005 void @"_report@3"(i32 noundef) addrspace(1) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486"

declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 1, !"wchar_size", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git 7e3542bb5338616657772790c7499f26077216b8)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"p1 long", !10, i64 0}
!10 = !{!"any pointer", !6, i64 0}
!11 = !{!5, !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"long", !6, i64 0}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 omnipotent char", !10, i64 0}
!16 = !{!6, !6, i64 0}
!17 = !{!10, !10, i64 0}
