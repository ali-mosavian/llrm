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
@GlobL = local_unnamed_addr global [1000 x [1000 x double]] zeroinitializer

define void @test(ptr noalias readonly nocapture %0, ptr noalias readonly nocapture %1, ptr noalias nocapture %2, ptr noalias nocapture %3, ptr noalias readonly nocapture %4, ptr noalias readonly nocapture %5, ptr noalias readonly nocapture %6, ptr noalias readonly nocapture %7, ptr noalias readonly nocapture %8, ptr noalias readonly nocapture %9) {
entry:
  %10 = load i32, ptr %7, align 4
  %11 = sext i32 %10 to i64
  %12 = load i32, ptr %8, align 4
  %13 = sext i32 %12 to i64
  %cmp1 = icmp sgt i32 %10, 0
  %cmp2 = icmp sgt i32 %12, 0
  %cond = and i1 %cmp1, %cmp2
  br i1 %cond, label %preheader, label %exit

preheader:                                        ; preds = %entry
  br label %i.header

i.header:                                         ; preds = %i.latch, %preheader
  %i = phi i64 [ %i.next, %i.latch ], [ 1, %preheader ]
  %14 = add nsw i64 -55, %i
  %15 = add nsw i64 %i, -1
  %16 = mul nsw i64 %15, %13
  %invariant.gep = getelementptr double, ptr @GlobL, i64 %16
  br label %j.header

j.header:                                         ; preds = %j.latch, %i.header
  %j = phi i64 [ %j.next, %j.latch ], [ 1, %i.header ]
  %17 = add nsw i64 -55, %j
  %gep358 = getelementptr double, ptr %invariant.gep, i64 %j
  br label %IL.header

IL.header:                                        ; preds = %IL.latch, %j.header
  %IL = phi i64 [ %IL.next, %IL.latch ], [ 1, %j.header ]
  %18 = mul nuw nsw i64 %IL, 54
  %19 = add nsw i64 %14, %18
  %20 = getelementptr double, ptr @GlobC, i64 %19
  %21 = load double, ptr %20, align 8
  %22 = getelementptr double, ptr @GlobG, i64 %19
  %23 = load double, ptr %22, align 8
  %24 = getelementptr double, ptr @GlobE, i64 %19
  %25 = load double, ptr %24, align 8
  %26 = getelementptr double, ptr @GlobI, i64 %19
  %27 = load double, ptr %26, align 8
  %28 = add nsw i64 %IL, -1
  %29 = mul nsw i64 %28, %13
  br label %JL.body

JL.body:                                          ; preds = %JL.body, %IL.header
  %JL = phi i64 [ %JL.next, %JL.body ], [ 1, %IL.header ]
  %30 = mul nuw nsw i64 %JL, 54
  %31 = add nsw i64 %17, %30
  %32 = getelementptr double, ptr @GlobD, i64 %31
  %33 = load double, ptr %32, align 8
  %34 = fmul fast double %33, %21
  %35 = getelementptr double, ptr @GlobH, i64 %31
  %36 = load double, ptr %35, align 8
  %37 = fmul fast double %36, %23
  %38 = fadd fast double %37, %34
  %39 = getelementptr double, ptr @GlobF, i64 %31
  %40 = load double, ptr %39, align 8
  %41 = fmul fast double %40, %25
  %42 = fadd fast double %38, %41
  %43 = getelementptr double, ptr @GlobJ, i64 %31
  %44 = load double, ptr %43, align 8
  %45 = fmul fast double %44, %27
  %46 = fadd fast double %42, %45
  %47 = add nsw i64 %JL, %29
  %.idx247.us.us.us.us.us.us = mul nsw i64 %47, 8000
  %gep.us.us.us.us.us.us = getelementptr i8, ptr %gep358, i64 %.idx247.us.us.us.us.us.us
  %48 = getelementptr i8, ptr %gep.us.us.us.us.us.us, i64 -8008
  store double %46, ptr %48, align 8
  %JL.next = add nuw nsw i64 %JL, 1
  %exitcond.not = icmp eq i64 %JL, %13
  br i1 %exitcond.not, label %IL.latch, label %JL.body

IL.latch:                                         ; preds = %JL.body
  %IL.next = add nuw nsw i64 %IL, 1
  %exitcond320.not = icmp eq i64 %IL, %11
  br i1 %exitcond320.not, label %j.latch, label %IL.header

j.latch:                                          ; preds = %IL.latch
  %j.next = add nuw nsw i64 %j, 1
  %exitcond324.not = icmp eq i64 %j, %13
  br i1 %exitcond324.not, label %i.latch, label %j.header

i.latch:                                          ; preds = %j.latch
  %i.next = add nuw nsw i64 %i, 1
  %exitcond328.not = icmp eq i64 %i, %11
  br i1 %exitcond328.not, label %exit.loopexit, label %i.header

exit.loopexit:                                    ; preds = %i.latch
  br label %exit

exit:                                             ; preds = %exit.loopexit, %entry
  ret void
}
