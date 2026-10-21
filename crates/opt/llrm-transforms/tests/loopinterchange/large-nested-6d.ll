; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@GlobC = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobD = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobE = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobF = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobG = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobH = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobI = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobJ = local_unnamed_addr global [54 x [54 x [54 x double]]] zeroinitializer
@GlobK = local_unnamed_addr global [1000 x [1000 x double]] zeroinitializer
@GlobL = local_unnamed_addr global [1000 x [1000 x double]] zeroinitializer
@GlobM = local_unnamed_addr global [2500 x double] zeroinitializer

define void @test(ptr noalias readonly nocapture %0, ptr noalias readonly nocapture %1, ptr noalias nocapture %2, ptr noalias nocapture %3, ptr noalias readonly nocapture %4, ptr noalias readonly nocapture %5, ptr noalias readonly nocapture %6, ptr noalias readonly nocapture %7, ptr noalias readonly nocapture %8, ptr noalias readonly nocapture %9) {
  %11 = alloca [2500 x double], align 8
  %12 = load i32, ptr %4, align 4
  %13 = tail call i32 @llvm.smax.i32(i32 %12, i32 0)
  %14 = zext nneg i32 %13 to i64
  %15 = load i32, ptr %9, align 4
  %.not = icmp eq i32 %15, 1
  br i1 %.not, label %170, label %16

16:                                               ; preds = %10
  %17 = load i32, ptr %7, align 4
  %18 = sext i32 %17 to i64
  %19 = icmp sgt i32 %17, 0
  br i1 %19, label %.lr.ph286, label %._crit_edge287

.lr.ph286:                                        ; preds = %16
  %20 = load i32, ptr %8, align 4
  %21 = sext i32 %20 to i64
  %22 = icmp sgt i32 %20, 0
  br i1 %22, label %preheader.L, label %._crit_edge287

preheader.L:                                      ; preds = %.lr.ph286
  %23 = load i32, ptr %5, align 4
  %24 = tail call i32 @llvm.smax.i32(i32 %23, i32 0)
  %25 = zext nneg i32 %24 to i64
  %26 = load i32, ptr %6, align 4
  %27 = sext i32 %26 to i64
  %28 = getelementptr double, ptr %1, i64 %27
  %.not241270.us = icmp slt i32 %23, 1
  %29 = shl nuw nsw i64 %25, 3
  %30 = add nuw nsw i64 %25, 2
  %31 = icmp sgt i32 %23, 0
  %.neg = sext i1 %31 to i64
  %32 = add nsw i64 %30, %.neg
  br label %L.header

L.header:                                         ; preds = %L.latch, %preheader.L
  %L = phi i64 [ %L.next, %L.latch ], [ 1, %preheader.L ]
  %33 = mul nuw nsw i64 %L, 2916
  %34 = add nsw i64 %33, -2971
  %35 = add nsw i64 %L, -1
  %36 = mul nsw i64 %35, %21
  br label %M.header

exit.i:                                           ; preds = %i.latch
  br i1 %.not241270.us, label %._crit_edge275.us.thread, label %.preheader258.us.preheader

.lr.ph274.us:                                     ; preds = %.preheader260.us, %.lr.ph274.us
  %37 = phi i64 [ %48, %.lr.ph274.us ], [ %25, %.preheader260.us ]
  %38 = phi double [ %46, %.lr.ph274.us ], [ 0.000000e+00, %.preheader260.us ]
  %39 = phi i64 [ %47, %.lr.ph274.us ], [ 1, %.preheader260.us ]
  %40 = add nsw i64 %39, -1
  %41 = getelementptr double, ptr %28, i64 %40
  %42 = load double, ptr %41, align 8
  %43 = getelementptr double, ptr @GlobM, i64 %40
  %44 = load double, ptr %43, align 8
  %45 = fmul fast double %44, %42
  %46 = fadd fast double %45, %38
  %47 = add nuw nsw i64 %39, 1
  %48 = add nsw i64 %37, -1
  %.not242.us = icmp eq i64 %48, 0
  br i1 %.not242.us, label %.lr.ph278.us.preheader, label %.lr.ph274.us

.lr.ph278.us.preheader:                           ; preds = %.lr.ph274.us
  %.lcssa = phi double [ %46, %.lr.ph274.us ]
  %49 = add nsw i64 %M, %36
  %50 = getelementptr double, ptr %11, i64 %49
  %51 = getelementptr i8, ptr %50, i64 -8
  store double %.lcssa, ptr %51, align 8
  %52 = getelementptr double, ptr @GlobK, i64 %49
  %53 = getelementptr i8, ptr %52, i64 -8
  br label %.lr.ph278.us

latch.M.loopexit:                                 ; preds = %._crit_edge279.us
  br label %latch.M

latch.M:                                          ; preds = %._crit_edge275.us.thread, %latch.M.loopexit
  %M.next = add nuw nsw i64 %M, 1
  %exitcond335.not = icmp eq i64 %M, %21
  br i1 %exitcond335.not, label %L.latch, label %M.header

.lr.ph278.us:                                     ; preds = %._crit_edge279.us, %.lr.ph278.us.preheader
  %54 = phi i64 [ %132, %._crit_edge279.us ], [ 1, %.lr.ph278.us.preheader ]
  %55 = add nsw i64 %54, -1
  %.idx244.us = mul nuw nsw i64 %55, 8000
  %56 = getelementptr i8, ptr @GlobL, i64 %.idx244.us
  br label %57

57:                                               ; preds = %57, %.lr.ph278.us
  %58 = phi i64 [ %25, %.lr.ph278.us ], [ %69, %57 ]
  %59 = phi double [ 0.000000e+00, %.lr.ph278.us ], [ %67, %57 ]
  %60 = phi i64 [ 1, %.lr.ph278.us ], [ %68, %57 ]
  %61 = add nsw i64 %60, -1
  %62 = getelementptr double, ptr %56, i64 %61
  %63 = load double, ptr %62, align 8
  %64 = getelementptr double, ptr %28, i64 %61
  %65 = load double, ptr %64, align 8
  %66 = fmul fast double %65, %63
  %67 = fadd fast double %66, %59
  %68 = add nuw nsw i64 %60, 1
  %69 = add nsw i64 %58, -1
  %.not243.us = icmp eq i64 %69, 0
  br i1 %.not243.us, label %._crit_edge279.us, label %57

70:                                               ; preds = %.preheader258.us, %70
  %71 = phi i64 [ %25, %.preheader258.us ], [ %81, %70 ]
  %72 = phi i64 [ 1, %.preheader258.us ], [ %80, %70 ]
  %73 = add nsw i64 %72, -1
  %74 = getelementptr double, ptr @GlobM, i64 %73
  %75 = load double, ptr %74, align 8
  %76 = getelementptr double, ptr %84, i64 %73
  %77 = load double, ptr %76, align 8
  %78 = fmul fast double %86, %77
  %79 = fadd fast double %78, %75
  store double %79, ptr %74, align 8
  %80 = add nuw nsw i64 %72, 1
  %81 = add nsw i64 %71, -1
  %.not245.us = icmp eq i64 %81, 0
  br i1 %.not245.us, label %._crit_edge.us, label %70

.preheader258.us:                                 ; preds = %._crit_edge.us, %.preheader258.us.preheader
  %82 = phi i64 [ %127, %._crit_edge.us ], [ 1, %.preheader258.us.preheader ]
  %83 = add nsw i64 %82, -1
  %.idx246.us = mul nuw nsw i64 %83, 8000
  %84 = getelementptr i8, ptr @GlobL, i64 %.idx246.us
  %85 = getelementptr double, ptr %28, i64 %83
  %86 = load double, ptr %85, align 8
  br label %70

.preheader260.us:                                 ; preds = %._crit_edge.us
  br label %.lr.ph274.us

._crit_edge275.us.thread:                         ; preds = %exit.i
  %87 = getelementptr double, ptr %11, i64 %M
  %88 = getelementptr double, ptr %87, i64 %36
  %89 = getelementptr i8, ptr %88, i64 -8
  store double 0.000000e+00, ptr %89, align 8
  br label %latch.M

.preheader258.us.preheader:                       ; preds = %exit.i
  call void @llvm.memset.p0.i64(ptr nonnull align 16 @GlobM, i8 0, i64 %29, i1 false)
  br label %.preheader258.us

M.header:                                         ; preds = %latch.M, %L.header
  %M = phi i64 [ 1, %L.header ], [ %M.next, %latch.M ]
  %90 = mul nuw nsw i64 %M, 2916
  %91 = add nsw i64 %90, -2971
  br label %i.header

i.header:                                         ; preds = %i.latch, %M.header
  %i = phi i64 [ %i.next, %i.latch ], [ 1, %M.header ]
  %92 = add nsw i64 %34, %i
  %93 = add nsw i64 %i, -1
  %94 = mul nsw i64 %93, %21
  %invariant.gep = getelementptr double, ptr @GlobL, i64 %94
  br label %j.header

j.header:                                         ; preds = %j.latch, %i.header
  %j = phi i64 [ %j.next, %j.latch ], [ 1, %i.header ]
  %95 = add nsw i64 %91, %j
  %gep358 = getelementptr double, ptr %invariant.gep, i64 %j
  br label %IL.header

IL.header:                                        ; preds = %IL.latch, %j.header
  %IL = phi i64 [ %IL.next, %IL.latch ], [ 1, %j.header ]
  %96 = mul nuw nsw i64 %IL, 54
  %97 = add nsw i64 %92, %96
  %98 = getelementptr double, ptr @GlobC, i64 %97
  %99 = load double, ptr %98, align 8
  %100 = getelementptr double, ptr @GlobG, i64 %97
  %101 = load double, ptr %100, align 8
  %102 = getelementptr double, ptr @GlobE, i64 %97
  %103 = load double, ptr %102, align 8
  %104 = getelementptr double, ptr @GlobI, i64 %97
  %105 = load double, ptr %104, align 8
  %106 = add nsw i64 %IL, -1
  %107 = mul nsw i64 %106, %21
  br label %JL.body

JL.body:                                          ; preds = %JL.body, %IL.header
  %JL = phi i64 [ %JL.next, %JL.body ], [ 1, %IL.header ]
  %108 = mul nuw nsw i64 %JL, 54
  %109 = add nsw i64 %95, %108
  %110 = getelementptr double, ptr @GlobD, i64 %109
  %111 = load double, ptr %110, align 8
  %112 = fmul fast double %111, %99
  %113 = getelementptr double, ptr @GlobH, i64 %109
  %114 = load double, ptr %113, align 8
  %115 = fmul fast double %114, %101
  %116 = fadd fast double %115, %112
  %117 = getelementptr double, ptr @GlobF, i64 %109
  %118 = load double, ptr %117, align 8
  %119 = fmul fast double %118, %103
  %120 = fadd fast double %116, %119
  %121 = getelementptr double, ptr @GlobJ, i64 %109
  %122 = load double, ptr %121, align 8
  %123 = fmul fast double %122, %105
  %124 = fadd fast double %120, %123
  %125 = add nsw i64 %JL, %107
  %.idx247.us.us.us.us.us.us = mul nsw i64 %125, 8000
  %gep.us.us.us.us.us.us = getelementptr i8, ptr %gep358, i64 %.idx247.us.us.us.us.us.us
  %126 = getelementptr i8, ptr %gep.us.us.us.us.us.us, i64 -8008
  store double %124, ptr %126, align 8
  %JL.next = add nuw nsw i64 %JL, 1
  %exitcond.not = icmp eq i64 %JL, %21
  br i1 %exitcond.not, label %IL.latch, label %JL.body

IL.latch:                                         ; preds = %JL.body
  %IL.next = add nuw nsw i64 %IL, 1
  %exitcond320.not = icmp eq i64 %IL, %18
  br i1 %exitcond320.not, label %j.latch, label %IL.header

j.latch:                                          ; preds = %IL.latch
  %j.next = add nuw nsw i64 %j, 1
  %exitcond324.not = icmp eq i64 %j, %21
  br i1 %exitcond324.not, label %i.latch, label %j.header

i.latch:                                          ; preds = %j.latch
  %i.next = add nuw nsw i64 %i, 1
  %exitcond328.not = icmp eq i64 %i, %18
  br i1 %exitcond328.not, label %exit.i, label %i.header

._crit_edge.us:                                   ; preds = %70
  %127 = add nuw nsw i64 %82, 1
  %exitcond329.not = icmp eq i64 %127, %32
  br i1 %exitcond329.not, label %.preheader260.us, label %.preheader258.us

._crit_edge279.us:                                ; preds = %57
  %.lcssa360 = phi double [ %67, %57 ]
  %128 = getelementptr double, ptr @GlobM, i64 %55
  %129 = load double, ptr %128, align 8
  %130 = fadd fast double %129, %.lcssa360
  %131 = getelementptr i8, ptr %53, i64 %.idx244.us
  store double %130, ptr %131, align 8
  %132 = add nuw nsw i64 %54, 1
  %exitcond331.not = icmp eq i64 %132, %32
  br i1 %exitcond331.not, label %latch.M.loopexit, label %.lr.ph278.us

L.latch:                                          ; preds = %latch.M
  %L.next = add nuw nsw i64 %L, 1
  %exitcond339.not = icmp eq i64 %L, %18
  br i1 %exitcond339.not, label %exit.L, label %L.header

exit.L:                                           ; preds = %L.latch
  br label %._crit_edge287

._crit_edge287:                                   ; preds = %exit.L, %.lr.ph286, %16
  %133 = load i32, ptr %6, align 4
  %134 = load i32, ptr %5, align 4
  %135 = tail call i32 @llvm.smax.i32(i32 %134, i32 0)
  %136 = zext nneg i32 %135 to i64
  %137 = sext i32 %133 to i64
  %138 = getelementptr double, ptr %2, i64 %137
  %139 = shl nuw nsw i64 %136, 3
  %.not236 = icmp slt i32 %134, 1
  %140 = select i1 %.not236, i64 1, i64 %139
  %141 = tail call ptr @malloc(i64 %140)
  br i1 %.not236, label %._crit_edge294, label %.preheader254.preheader

.preheader254.preheader:                          ; preds = %._crit_edge287
  call void @llvm.memset.p0.i64(ptr align 8 %141, i8 0, i64 %139, i1 false)
  br label %.preheader254

.preheader254:                                    ; preds = %._crit_edge, %.preheader254.preheader
  %142 = phi i64 [ %159, %._crit_edge ], [ 1, %.preheader254.preheader ]
  %143 = add nsw i64 %142, -1
  %.idx240 = mul nuw nsw i64 %143, 8000
  %144 = getelementptr i8, ptr %0, i64 %.idx240
  %145 = getelementptr double, ptr %11, i64 %143
  %146 = load double, ptr %145, align 8
  br label %147

.preheader253:                                    ; preds = %._crit_edge
  br label %.lr.ph293

147:                                              ; preds = %147, %.preheader254
  %148 = phi i64 [ %136, %.preheader254 ], [ %158, %147 ]
  %149 = phi i64 [ 1, %.preheader254 ], [ %157, %147 ]
  %150 = add nsw i64 %149, -1
  %151 = getelementptr double, ptr %141, i64 %150
  %152 = load double, ptr %151, align 8
  %153 = getelementptr double, ptr %144, i64 %150
  %154 = load double, ptr %153, align 8
  %155 = fmul fast double %146, %154
  %156 = fadd fast double %155, %152
  store double %156, ptr %151, align 8
  %157 = add nuw nsw i64 %149, 1
  %158 = add nsw i64 %148, -1
  %.not239 = icmp eq i64 %158, 0
  br i1 %.not239, label %._crit_edge, label %147

._crit_edge:                                      ; preds = %147
  %159 = add nuw nsw i64 %142, 1
  %exitcond341.not = icmp eq i64 %142, %136
  br i1 %exitcond341.not, label %.preheader253, label %.preheader254

.lr.ph293:                                        ; preds = %.lr.ph293, %.preheader253
  %160 = phi i64 [ %169, %.lr.ph293 ], [ %136, %.preheader253 ]
  %161 = phi i64 [ %168, %.lr.ph293 ], [ 1, %.preheader253 ]
  %162 = add nsw i64 %161, -1
  %163 = getelementptr double, ptr %138, i64 %162
  %164 = getelementptr double, ptr %141, i64 %162
  %165 = load double, ptr %164, align 8
  %166 = load double, ptr %163, align 8
  %167 = fsub fast double %166, %165
  store double %167, ptr %163, align 8
  %168 = add nuw nsw i64 %161, 1
  %169 = add nsw i64 %160, -1
  %.not238 = icmp eq i64 %169, 0
  br i1 %.not238, label %._crit_edge294.loopexit359, label %.lr.ph293

170:                                              ; preds = %10
  %171 = load i32, ptr %6, align 4
  %172 = load i32, ptr %5, align 4
  %173 = tail call i32 @llvm.smax.i32(i32 %172, i32 0)
  %174 = zext nneg i32 %173 to i64
  %175 = shl nuw nsw i64 %174, 3
  %176 = mul i64 %175, %174
  %177 = tail call i64 @llvm.smax.i64(i64 %176, i64 1)
  %178 = tail call ptr @malloc(i64 %177)
  %.not311 = icmp slt i32 %172, 1
  br i1 %.not311, label %._crit_edge294, label %.preheader250.us.preheader

.preheader250.us.preheader:                       ; preds = %170
  %179 = mul nuw nsw i64 %174, %174
  %180 = shl i64 %179, 3
  call void @llvm.memset.p0.i64(ptr align 8 %178, i8 0, i64 %180, i1 false)
  br label %.preheader250.us

.preheader250.us:                                 ; preds = %._crit_edge301.split.us, %.preheader250.us.preheader
  %181 = phi i64 [ %202, %._crit_edge301.split.us ], [ 1, %.preheader250.us.preheader ]
  %182 = add nsw i64 %181, -1
  %.idx.us = mul nuw nsw i64 %182, 8000
  %183 = getelementptr i8, ptr %0, i64 %.idx.us
  %invariant.gep.us = getelementptr double, ptr @GlobK, i64 %182
  br label %.preheader249.us

184:                                              ; preds = %.preheader249.us, %184
  %185 = phi i64 [ %174, %.preheader249.us ], [ %195, %184 ]
  %186 = phi i64 [ 1, %.preheader249.us ], [ %194, %184 ]
  %187 = add nsw i64 %186, -1
  %188 = getelementptr double, ptr %199, i64 %187
  %189 = load double, ptr %188, align 8
  %190 = getelementptr double, ptr %183, i64 %187
  %191 = load double, ptr %190, align 8
  %192 = fmul fast double %200, %191
  %193 = fadd fast double %192, %189
  store double %193, ptr %188, align 8
  %194 = add nuw nsw i64 %186, 1
  %195 = add nsw i64 %185, -1
  %.not233.us = icmp eq i64 %195, 0
  br i1 %.not233.us, label %._crit_edge300.us, label %184

.preheader249.us:                                 ; preds = %._crit_edge300.us, %.preheader250.us
  %196 = phi i64 [ 1, %.preheader250.us ], [ %201, %._crit_edge300.us ]
  %197 = add nsw i64 %196, -1
  %198 = mul nuw nsw i64 %197, %174
  %199 = getelementptr double, ptr %178, i64 %198
  %.idx234.us = mul nuw nsw i64 %197, 8000
  %gep.us = getelementptr i8, ptr %invariant.gep.us, i64 %.idx234.us
  %200 = load double, ptr %gep.us, align 8
  br label %184

._crit_edge300.us:                                ; preds = %184
  %201 = add nuw nsw i64 %196, 1
  %exitcond344.not = icmp eq i64 %196, %174
  br i1 %exitcond344.not, label %._crit_edge301.split.us, label %.preheader249.us

._crit_edge301.split.us:                          ; preds = %._crit_edge300.us
  %202 = add nuw nsw i64 %181, 1
  %exitcond345.not = icmp eq i64 %181, %174
  br i1 %exitcond345.not, label %.preheader248, label %.preheader250.us

.preheader248:                                    ; preds = %._crit_edge301.split.us
  br label %.preheader.lr.ph

.preheader.lr.ph:                                 ; preds = %.preheader248
  %203 = sext i32 %171 to i64
  %invariant.gep306 = getelementptr double, ptr %3, i64 %203
  br label %.preheader

.preheader:                                       ; preds = %._crit_edge304, %.preheader.lr.ph
  %204 = phi i64 [ 1, %.preheader.lr.ph ], [ %220, %._crit_edge304 ]
  %205 = add nsw i64 %204, -1
  %206 = add nsw i64 %205, %203
  %207 = mul nsw i64 %206, %14
  %gep307 = getelementptr double, ptr %invariant.gep306, i64 %207
  %208 = mul nuw nsw i64 %205, %174
  %209 = getelementptr double, ptr %178, i64 %208
  br label %210

210:                                              ; preds = %210, %.preheader
  %211 = phi i64 [ %174, %.preheader ], [ %219, %210 ]
  %212 = phi i64 [ 1, %.preheader ], [ %218, %210 ]
  %213 = add nsw i64 %212, -1
  %gep = getelementptr double, ptr %gep307, i64 %213
  %214 = getelementptr double, ptr %209, i64 %213
  %215 = load double, ptr %214, align 8
  %216 = load double, ptr %gep, align 8
  %217 = fsub fast double %216, %215
  store double %217, ptr %gep, align 8
  %218 = add nuw nsw i64 %212, 1
  %219 = add nsw i64 %211, -1
  %.not232 = icmp eq i64 %219, 0
  br i1 %.not232, label %._crit_edge304, label %210

._crit_edge304:                                   ; preds = %210
  %220 = add nuw nsw i64 %204, 1
  %exitcond347.not = icmp eq i64 %204, %174
  br i1 %exitcond347.not, label %._crit_edge294.loopexit, label %.preheader

._crit_edge294.loopexit:                          ; preds = %._crit_edge304
  br label %._crit_edge294

._crit_edge294.loopexit359:                       ; preds = %.lr.ph293
  br label %._crit_edge294

._crit_edge294:                                   ; preds = %._crit_edge294.loopexit359, %._crit_edge294.loopexit, %170, %._crit_edge287
  %.sink = phi ptr [ %141, %._crit_edge287 ], [ %178, %170 ], [ %178, %._crit_edge294.loopexit ], [ %141, %._crit_edge294.loopexit359 ]
  tail call void @free(ptr %.sink)
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.smax.i64(i64, i64) #0

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smax.i32(i32, i32) #0

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i64(ptr writeonly nocapture, i8, i64, i1 immarg) #1

declare void @free(ptr noundef nocapture) local_unnamed_addr

declare noalias noundef ptr @malloc(i64 noundef) local_unnamed_addr

attributes #0 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
