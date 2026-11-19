target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"SUM&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"P\00", ptr getelementptr (i8, ptr @$string3, i16 4), [80 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|" }>
@$string4 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string4, i16 4), [8 x i8] c"DICKENS\00" }>

define internal void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" optsize norecurse {
b1:
  %0 = tail call addrspace(1) i32 @BENCHGREP(ptr @"SUM&")
  %1 = tail call addrspace(1) ptr @qb_STI4(i32 %0)
  %2 = tail call addrspace(1) ptr @qb_LTRM(ptr %1)
  tail call addrspace(1) void @qb_PESD(ptr %2)
  %3 = load i32, ptr @"SUM&", !tbaa !2
  %4 = tail call addrspace(1) ptr @qb_STI4(i32 %3)
  %5 = tail call addrspace(1) ptr @qb_LTRM(ptr %4)
  tail call addrspace(1) void @qb_PESD(ptr %5)
  ret void
}

define i32 @BENCHGREP(ptr dereferenceable(4) nonnull noundef nocapture nofree writeonly %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" optsize norecurse {
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [18 x i8]
  %5 = alloca [18 x i8]
  %6 = alloca [18 x i8]
  %7 = alloca [18 x i8]
  %8 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %4, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %5, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %6, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %7, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %8, i8 0, i16 18, i1 false)
  call addrspace(1) void @qb_DDIM(i16 0, i16 2559, i16 2, i16 257, ptr nonnull %8)
  call addrspace(1) void @qb_DDIM(i16 0, i16 255, i16 2, i16 257, ptr nonnull %7)
  call addrspace(1) void @qb_DDIM(i16 0, i16 79, i16 2, i16 257, ptr nonnull %6)
  call addrspace(1) void @qb_DDIM(i16 0, i16 79, i16 2, i16 257, ptr nonnull %5)
  call addrspace(1) void @qb_DDIM(i16 0, i16 79, i16 2, i16 257, ptr nonnull %4)
  call addrspace(1) void @qb_SASS(ptr @$string3, ptr nonnull %3)
  %length2 = load i16, ptr %3
  %9 = icmp slt i16 %length2, 1
  br i1 %9, label %b26, label %10

10:
  %11 = getelementptr inbounds nuw i8, ptr %7, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  %13 = inttoptr i16 %12 to ptr addrspace(2), !llrm.descriptor !19
  %14 = getelementptr inbounds nuw i8, ptr %8, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2), !llrm.descriptor !19
  %17 = getelementptr inbounds nuw i8, ptr %5, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = inttoptr i16 %18 to ptr addrspace(2), !llrm.descriptor !19
  br label %20

20:
  %21 = phi i16 [ %55, %b9 ], [ 0, %10 ]
  %22 = phi i16 [ 1, %10 ], [ %54, %b9 ]
  %23 = phi i16 [ 1, %10 ], [ %53, %b9 ]
  %24 = phi i16 [ 1, %10 ], [ %52, %b9 ]
  %25 = phi i16 [ 0, %10 ], [ %51, %b9 ]
  %26 = getelementptr i8, ptr %3, i16 2
  %data3 = load ptr, ptr %26
  %27 = getelementptr i8, ptr %data3, i16 %21
  %byte1 = load i8, ptr %27
  %28 = zext i8 %byte1 to i16
  %29 = icmp eq i16 %28, 124
  br i1 %29, label %b7, label %b8

b6:
  %30 = add i16 %54, -1
  %31 = icmp slt i16 %30, 1
  br i1 %31, label %b26, label %32

32:
  %33 = getelementptr inbounds nuw i8, ptr %8, i16 2
  %34 = load i16, ptr %33, !tbaa !2
  %35 = inttoptr i16 %34 to ptr addrspace(2), !llrm.descriptor !19
  %36 = addrspacecast ptr addrspace(2) %35 to ptr addrspace(1)
  %37 = getelementptr inbounds nuw i8, ptr %4, i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = inttoptr i16 %38 to ptr addrspace(2), !llrm.descriptor !19
  %40 = add i16 %54, -1
  %41 = getelementptr i8, ptr addrspace(1) %36, i16 2
  br label %b19

b7:
  %42 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %43 = shl i16 %25, 1
  %44 = getelementptr i8, ptr addrspace(1) %42, i16 %43
  store i16 %24, ptr addrspace(1) %44, !tbaa !10
  %45 = add i16 %24, 1
  br label %b9

b8:
  %46 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %47 = shl nuw nsw i16 %28, 1
  %48 = getelementptr i8, ptr addrspace(1) %46, i16 %47
  %49 = load i16, ptr addrspace(1) %48, !tbaa !12
  %50 = icmp eq i16 %49, 0
  br i1 %50, label %b10, label %b12

b9:
  %51 = phi i16 [ 0, %b7 ], [ %81, %b15 ]
  %52 = phi i16 [ %45, %b7 ], [ %24, %b15 ]
  %53 = phi i16 [ %23, %b7 ], [ %78, %b15 ]
  %54 = phi i16 [ %22, %b7 ], [ %61, %b15 ]
  %55 = add nuw nsw i16 %21, 1
  %56 = icmp eq i16 %length2, %55
  br i1 %56, label %b6, label %20

b10:
  %57 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %58 = getelementptr i8, ptr addrspace(1) %57, i16 %47
  store i16 %22, ptr addrspace(1) %58, !tbaa !12
  %59 = add i16 %22, 1
  br label %b12

b12:
  %60 = phi i16 [ %22, %b10 ], [ %49, %b8 ]
  %61 = phi i16 [ %59, %b10 ], [ %22, %b8 ]
  %62 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %63 = shl i16 %25, 5
  %64 = add i16 %60, %63
  %65 = shl i16 %64, 1
  %66 = getelementptr i8, ptr addrspace(1) %62, i16 %65
  %67 = load i16, ptr addrspace(1) %66, !tbaa !14
  %68 = icmp eq i16 %67, 0
  br i1 %68, label %b13, label %b15

b13:
  %69 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %70 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %69, i16 %65
  store i16 %23, ptr addrspace(1) %71, !tbaa !14
  %72 = add i16 %23, 1
  %73 = getelementptr i8, ptr addrspace(1) %70, i16 %47
  %74 = load i16, ptr addrspace(1) %73, !tbaa !12
  %75 = add i16 %74, %63
  %76 = shl i16 %75, 1
  br label %b15

b15:
  %77 = phi i16 [ %65, %b12 ], [ %76, %b13 ]
  %78 = phi i16 [ %72, %b13 ], [ %23, %b12 ]
  %79 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %80 = getelementptr i8, ptr addrspace(1) %79, i16 %77
  %81 = load i16, ptr addrspace(1) %80, !tbaa !14
  br label %b9

82:
  %83 = icmp sgt i16 %106, 0
  br i1 %83, label %84, label %b26

84:
  %85 = getelementptr inbounds nuw i8, ptr %4, i16 2
  %86 = load i16, ptr %85, !tbaa !2
  %87 = inttoptr i16 %86 to ptr addrspace(2), !llrm.descriptor !19
  %88 = getelementptr inbounds nuw i8, ptr %8, i16 2
  %89 = load i16, ptr %88, !tbaa !2
  %90 = inttoptr i16 %89 to ptr addrspace(2), !llrm.descriptor !19
  %91 = getelementptr inbounds nuw i8, ptr %6, i16 2
  %92 = load i16, ptr %91, !tbaa !2
  %93 = inttoptr i16 %92 to ptr addrspace(2), !llrm.descriptor !19
  %94 = getelementptr inbounds nuw i8, ptr %5, i16 2
  %95 = load i16, ptr %94, !tbaa !2
  %96 = inttoptr i16 %95 to ptr addrspace(2), !llrm.descriptor !19
  br label %b25

b19:
  %97 = phi ptr addrspace(1) [ %108, %b23 ], [ %41, %32 ]
  %98 = phi i16 [ %107, %b23 ], [ %40, %32 ]
  %99 = phi i16 [ 0, %32 ], [ %106, %b23 ]
  %100 = load i16, ptr addrspace(1) %97, !tbaa !14
  %101 = icmp eq i16 %100, 0
  br i1 %101, label %b23, label %b21

b21:
  %102 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %103 = shl i16 %99, 1
  %104 = getelementptr i8, ptr addrspace(1) %102, i16 %103
  store i16 %100, ptr addrspace(1) %104, !tbaa !16
  %105 = add i16 %99, 1
  br label %b23

b23:
  %106 = phi i16 [ %105, %b21 ], [ %99, %b19 ]
  %107 = add i16 %98, -1
  %108 = getelementptr i8, ptr addrspace(1) %97, i16 2
  %exitcond2 = icmp eq i16 %107, 0
  br i1 %exitcond2, label %82, label %b19

109:
  %110 = icmp slt i16 %118, %146
  br i1 %110, label %b25, label %b26

b25:
  %111 = phi i16 [ %118, %109 ], [ 0, %84 ]
  %112 = phi i16 [ %146, %109 ], [ %106, %84 ]
  %113 = addrspacecast ptr addrspace(2) %90 to ptr addrspace(1)
  %114 = addrspacecast ptr addrspace(2) %87 to ptr addrspace(1)
  %115 = shl nuw i16 %111, 1
  %116 = getelementptr i8, ptr addrspace(1) %114, i16 %115
  %117 = load i16, ptr addrspace(1) %116, !tbaa !16
  %118 = add nuw nsw i16 %111, 1
  %119 = shl i16 %117, 6
  %120 = shl i16 %117, 1
  br label %b30

b26:
  call addrspace(1) void @qb_OPEN(ptr @$string4, i16 1, i16 -1, i16 32)
  %121 = call addrspace(1) i32 @qb_FLOF(i16 1)
  %122 = call addrspace(1) ptr @qb_SPAC(i16 1024)
  call addrspace(1) void @qb_SASS(ptr %122, ptr nonnull %2)
  %123 = icmp sgt i32 %121, 0
  br i1 %123, label %124, label %b40

124:
  %125 = addrspacecast ptr %2 to ptr addrspace(1)
  br label %b39

b30:
  %126 = phi ptr addrspace(1) [ %148, %b34 ], [ %113, %b25 ]
  %127 = phi i16 [ %147, %b34 ], [ %54, %b25 ]
  %128 = phi i16 [ %112, %b25 ], [ %146, %b34 ]
  %129 = addrspacecast ptr addrspace(2) %93 to ptr addrspace(1)
  %130 = getelementptr i8, ptr addrspace(1) %126, i16 %119
  %131 = load i16, ptr addrspace(1) %130, !tbaa !14
  %132 = getelementptr i8, ptr addrspace(1) %129, i16 %120
  %133 = load i16, ptr addrspace(1) %132, !tbaa !18
  %134 = shl i16 %133, 6
  %135 = getelementptr i8, ptr addrspace(1) %126, i16 %134
  %136 = load i16, ptr addrspace(1) %135, !tbaa !14
  %137 = icmp eq i16 %131, 0
  br i1 %137, label %b33, label %b32

b32:
  %138 = addrspacecast ptr addrspace(2) %96 to ptr addrspace(1)
  %139 = addrspacecast ptr addrspace(2) %93 to ptr addrspace(1)
  %140 = shl i16 %131, 1
  %141 = getelementptr i8, ptr addrspace(1) %139, i16 %140
  store i16 %136, ptr addrspace(1) %141, !tbaa !18
  %142 = getelementptr i8, ptr addrspace(1) %138, i16 %140
  %143 = load i16, ptr addrspace(1) %142, !tbaa !10
  %144 = icmp eq i16 %143, 0
  br i1 %144, label %b35, label %b37

b33:
  %145 = getelementptr i8, ptr addrspace(1) %126, i16 %119
  store i16 %136, ptr addrspace(1) %145, !tbaa !14
  br label %b34

b34:
  %146 = phi i16 [ %158, %b37 ], [ %128, %b33 ]
  %147 = add i16 %127, -1
  %148 = getelementptr i8, ptr addrspace(1) %126, i16 2
  %exitcond1 = icmp eq i16 %147, 0
  br i1 %exitcond1, label %109, label %b30

b35:
  %149 = addrspacecast ptr addrspace(2) %96 to ptr addrspace(1)
  %150 = addrspacecast ptr addrspace(2) %96 to ptr addrspace(1)
  %151 = shl i16 %136, 1
  %152 = getelementptr i8, ptr addrspace(1) %150, i16 %151
  %153 = load i16, ptr addrspace(1) %152, !tbaa !10
  %154 = getelementptr i8, ptr addrspace(1) %149, i16 %140
  store i16 %153, ptr addrspace(1) %154, !tbaa !10
  br label %b37

b37:
  %155 = addrspacecast ptr addrspace(2) %87 to ptr addrspace(1)
  %156 = shl i16 %128, 1
  %157 = getelementptr i8, ptr addrspace(1) %155, i16 %156
  store i16 %131, ptr addrspace(1) %157, !tbaa !16
  %158 = add i16 %128, 1
  br label %b34

159:
  %160 = icmp sgt i32 %179, 0
  br i1 %160, label %b39, label %168

b39:
  %161 = phi i16 [ 0, %124 ], [ %211, %159 ]
  %162 = phi i16 [ 0, %124 ], [ %227, %159 ]
  %163 = phi i32 [ 0, %124 ], [ %226, %159 ]
  %164 = phi i32 [ 0, %124 ], [ %221, %159 ]
  %165 = phi i32 [ 0, %124 ], [ %191, %159 ]
  %166 = phi i32 [ %121, %124 ], [ %179, %159 ]
  %167 = icmp samesign ult i32 %166, 1024
  br i1 %167, label %b41, label %176

168:
  %169 = zext nneg i16 %227 to i32
  br label %b40

b40:
  %170 = phi i32 [ 0, %b26 ], [ %169, %168 ]
  %171 = phi i32 [ 0, %b26 ], [ %226, %168 ]
  %172 = phi i32 [ 0, %b26 ], [ %221, %168 ]
  call addrspace(1) void @qb_CLOS(i16 1, i16 1)
  store i32 %172, ptr %0
  %173 = add i32 %171, %170
  call addrspace(1) void @qb_STDL(ptr nonnull %3)
  call addrspace(1) void @qb_STDL(ptr nonnull %2)
  call addrspace(1) void @qb_ERAS(ptr nonnull %7)
  call addrspace(1) void @qb_ERAS(ptr nonnull %6)
  call addrspace(1) void @qb_ERAS(ptr nonnull %5)
  call addrspace(1) void @qb_ERAS(ptr nonnull %8)
  call addrspace(1) void @qb_ERAS(ptr nonnull %4)
  ret i32 %173

b41:
  %174 = trunc nuw nsw i32 %166 to i16
  %175 = call addrspace(1) ptr @qb_SPAC(i16 %174)
  call addrspace(1) void @qb_SASS(ptr %175, ptr nonnull %2)
  br label %176

176:
  %177 = phi i16 [ %174, %b41 ], [ 1024, %b39 ]
  call addrspace(1) void @qb_GET3(i16 1, ptr addrspace(1) %125, i16 0)
  %178 = zext nneg i16 %177 to i32
  %179 = sub nsw i32 %166, %178
  %180 = getelementptr i8, ptr %2, i16 2
  %data1 = load ptr, ptr %180
  %181 = getelementptr i8, ptr %7, i16 2
  %182 = load i16, ptr %181, !tbaa !2
  %183 = inttoptr i16 %182 to ptr addrspace(2), !llrm.descriptor !19
  %184 = getelementptr i8, ptr %8, i16 2
  %185 = load i16, ptr %184, !tbaa !2
  %186 = inttoptr i16 %185 to ptr addrspace(2), !llrm.descriptor !19
  %187 = getelementptr i8, ptr %5, i16 2
  %188 = load i16, ptr %187, !tbaa !2
  %189 = inttoptr i16 %188 to ptr addrspace(2), !llrm.descriptor !19
  %190 = zext nneg i16 %177 to i32
  %191 = add i32 %165, %190
  br label %192

192:
  %193 = phi ptr [ %230, %b51 ], [ %data1, %176 ]
  %194 = phi i16 [ %229, %b51 ], [ %177, %176 ]
  %195 = phi i16 [ %161, %176 ], [ %211, %b51 ]
  %196 = phi i16 [ %162, %176 ], [ %227, %b51 ]
  %197 = phi i32 [ %163, %176 ], [ %226, %b51 ]
  %198 = phi i32 [ %164, %176 ], [ %221, %b51 ]
  %199 = phi i32 [ %165, %176 ], [ %228, %b51 ]
  %200 = addrspacecast ptr addrspace(2) %189 to ptr addrspace(1)
  %201 = addrspacecast ptr addrspace(2) %186 to ptr addrspace(1)
  %202 = addrspacecast ptr addrspace(2) %183 to ptr addrspace(1)
  %byte = load i8, ptr %193
  %203 = zext i8 %byte to i16
  %204 = shl nuw nsw i16 %203, 1
  %205 = getelementptr i8, ptr addrspace(1) %202, i16 %204
  %206 = load i16, ptr addrspace(1) %205, !tbaa !12
  %207 = shl i16 %195, 6
  %208 = shl i16 %206, 1
  %209 = getelementptr i8, ptr addrspace(1) %201, i16 %207
  %210 = getelementptr i8, ptr addrspace(1) %209, i16 %208
  %211 = load i16, ptr addrspace(1) %210, !tbaa !14
  %212 = shl i16 %211, 1
  %213 = getelementptr i8, ptr addrspace(1) %200, i16 %212
  %214 = load i16, ptr addrspace(1) %213, !tbaa !10
  %215 = sext i16 %214 to i32
  %216 = icmp eq i32 %215, 0
  br i1 %216, label %b51, label %b49

b49:
  %217 = shl i32 %198, 1
  %218 = add i32 %217, %215
  %219 = add i32 %199, %218
  %220 = and i32 %219, 536870911
  br label %b51

b51:
  %221 = phi i32 [ %220, %b49 ], [ %198, %192 ]
  %222 = phi i16 [ 1, %b49 ], [ %196, %192 ]
  %223 = icmp eq i16 %203, 10
  %224 = zext nneg i16 %222 to i32
  %225 = select i1 %223, i32 %224, i32 0
  %226 = add i32 %225, %197
  %227 = select i1 %223, i16 0, i16 %222
  %228 = add i32 %199, 1
  %229 = add i16 %194, -1
  %230 = getelementptr i8, ptr %193, i16 1
  %231 = icmp eq i16 %229, 0
  br i1 %231, label %159, label %192
}

declare ptr @qb_STI4(i32) addrspace(1) nocallback

declare ptr @qb_LTRM(ptr) addrspace(1) nocallback

declare void @qb_PESD(ptr) addrspace(1) nocallback

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @qb_DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare void @qb_SASS(ptr, ptr) addrspace(1) nocallback

declare i16 @qb_FLEN(ptr) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback optsize willreturn

declare ptr @qb_FMID(ptr releases, i16, i16) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback optsize willreturn

declare i16 @qb_FASC(ptr releases) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback optsize willreturn

declare void @qb_OPEN(ptr, i16, i16, i16) addrspace(1) nocallback

declare i32 @qb_FLOF(i16) addrspace(1) nocallback

declare ptr @qb_SPAC(i16) addrspace(1) nocallback

declare void @qb_CLOS(i16, i16) addrspace(1) nocallback

declare void @qb_GET3(i16, ptr addrspace(1), i16) addrspace(1) nocallback

declare void @qb_STDL(ptr) addrspace(1) nocallback nounwind

declare void @qb_ERAS(ptr nocapture noretain) addrspace(1) nocallback

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !5, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !5, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"referent", !0, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"absolute", !0, i64 0}
!8 = !{!7, !7, i64 0}
!9 = !{!"allocation.0", !3, i64 0}
!10 = !{!9, !9, i64 0}
!11 = !{!"allocation.1", !3, i64 0}
!12 = !{!11, !11, i64 0}
!13 = !{!"allocation.2", !3, i64 0}
!14 = !{!13, !13, i64 0}
!15 = !{!"allocation.3", !3, i64 0}
!16 = !{!15, !15, i64 0}
!17 = !{!"allocation.4", !3, i64 0}
!18 = !{!17, !17, i64 0}
!19 = !{}
