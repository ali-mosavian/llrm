; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

define i64 @test1(ptr %Arr) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc, %entry
  %indvars.iv23 = phi i64 [ 0, %entry ], [ %indvars.iv.next24, %for1.inc ]
  %sum.outer = phi i64 [ 0, %entry ], [ %sum.inc.lcssa, %for1.inc ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ 0, %for1.header ], [ %indvars.iv.next.3, %for2 ]
  %sum.inner = phi i64 [ %sum.outer, %for1.header ], [ %sum.inc, %for2 ]
  %arrayidx = getelementptr inbounds [100 x [100 x i64]], ptr %Arr, i64 0, i64 %indvars.iv, i64 %indvars.iv23
  %lv = load i64, ptr %arrayidx, align 4
  %sum.inc = add i64 %sum.inner, %lv
  %indvars.iv.next.3 = add nuw nsw i64 %indvars.iv, 1
  %exit1 = icmp eq i64 %indvars.iv.next.3, 100
  br i1 %exit1, label %for1.inc, label %for2

for1.inc:                                         ; preds = %for2
  %sum.inc.lcssa = phi i64 [ %sum.inc, %for2 ]
  %indvars.iv.next24 = add nuw nsw i64 %indvars.iv23, 1
  %exit2 = icmp eq i64 %indvars.iv.next24, 100
  br i1 %exit2, label %for1.loopexit, label %for1.header

for1.loopexit:                                    ; preds = %for1.inc
  %sum.inc.lcssa2 = phi i64 [ %sum.inc.lcssa, %for1.inc ]
  ret i64 %sum.inc.lcssa2
}

define i64 @test2(ptr %Arr) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc, %entry
  %indvars.iv23 = phi i64 [ 0, %entry ], [ %indvars.iv.next24, %for1.inc ]
  %sum.outer = phi i64 [ 0, %entry ], [ %sum.inc.lcssa, %for1.inc ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ 0, %for1.header ], [ %indvars.iv.next.3, %for2 ]
  %inner = phi i64 [ %indvars.iv23, %for1.header ], [ %sum.inc, %for2 ]
  %arrayidx = getelementptr inbounds [100 x [100 x i64]], ptr %Arr, i64 0, i64 %indvars.iv, i64 %indvars.iv23
  %lv = load i64, ptr %arrayidx, align 4
  %sum.inc = add i64 %inner, %lv
  %indvars.iv.next.3 = add nuw nsw i64 %indvars.iv, 1
  %exit1 = icmp eq i64 %indvars.iv.next.3, 100
  br i1 %exit1, label %for1.inc, label %for2

for1.inc:                                         ; preds = %for2
  %sum.inc.lcssa = phi i64 [ %sum.inc, %for2 ]
  %indvars.iv.next24 = add nuw nsw i64 %indvars.iv23, 1
  %exit2 = icmp eq i64 %indvars.iv.next24, 100
  br i1 %exit2, label %for1.loopexit, label %for1.header

for1.loopexit:                                    ; preds = %for1.inc
  %sum.inc.lcssa2 = phi i64 [ %sum.inc.lcssa, %for1.inc ]
  ret i64 %sum.inc.lcssa2
}

define i64 @test3(ptr %Arr) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc, %entry
  %indvars.iv23 = phi i64 [ 0, %entry ], [ %indvars.iv.next24, %for1.inc ]
  %sum.outer = phi i64 [ 0, %entry ], [ %sum.inc.lcssa, %for1.inc ]
  %so = add i64 %sum.outer, 10
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ 0, %for1.header ], [ %indvars.iv.next.3, %for2 ]
  %sum.inner = phi i64 [ %so, %for1.header ], [ %sum.inc, %for2 ]
  %arrayidx = getelementptr inbounds [100 x [100 x i64]], ptr %Arr, i64 0, i64 %indvars.iv, i64 %indvars.iv23
  %lv = load i64, ptr %arrayidx, align 4
  %sum.inc = add i64 %sum.inner, %lv
  %indvars.iv.next.3 = add nuw nsw i64 %indvars.iv, 1
  %exit1 = icmp eq i64 %indvars.iv.next.3, 100
  br i1 %exit1, label %for1.inc, label %for2

for1.inc:                                         ; preds = %for2
  %sum.inc.lcssa = phi i64 [ %sum.inc, %for2 ]
  %indvars.iv.next24 = add nuw nsw i64 %indvars.iv23, 1
  %exit2 = icmp eq i64 %indvars.iv.next24, 100
  br i1 %exit2, label %for1.loopexit, label %for1.header

for1.loopexit:                                    ; preds = %for1.inc
  %sum.inc.lcssa2 = phi i64 [ %sum.inc.lcssa, %for1.inc ]
  ret i64 %sum.inc.lcssa2
}

define i64 @test4(ptr %Arr, ptr %dst) {
entry:
  %gep.dst = getelementptr inbounds i64, ptr %dst, i64 42
  br label %for1.header

for1.header:                                      ; preds = %for1.inc, %entry
  %indvars.iv23 = phi i64 [ 0, %entry ], [ %indvars.iv.next24, %for1.inc ]
  %sum.outer = phi i64 [ 0, %entry ], [ %sum.inc.lcssa, %for1.inc ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ 0, %for1.header ], [ %indvars.iv.next.3, %for2 ]
  %sum.inner = phi i64 [ %sum.outer, %for1.header ], [ %sum.inc, %for2 ]
  %arrayidx = getelementptr inbounds [100 x [100 x i64]], ptr %Arr, i64 0, i64 %indvars.iv, i64 %indvars.iv23
  %lv = load i64, ptr %arrayidx, align 4
  %sum.inc = add i64 %sum.inner, %lv
  store i64 %sum.inc, ptr %gep.dst, align 4
  %indvars.iv.next.3 = add nuw nsw i64 %indvars.iv, 1
  %exit1 = icmp eq i64 %indvars.iv.next.3, 100
  br i1 %exit1, label %for1.inc, label %for2

for1.inc:                                         ; preds = %for2
  %sum.inc.lcssa = phi i64 [ %sum.inc, %for2 ]
  %indvars.iv.next24 = add nuw nsw i64 %indvars.iv23, 1
  %exit2 = icmp eq i64 %indvars.iv.next24, 100
  br i1 %exit2, label %for1.loopexit, label %for1.header

for1.loopexit:                                    ; preds = %for1.inc
  %sum.inc.lcssa2 = phi i64 [ %sum.inc.lcssa, %for1.inc ]
  ret i64 %sum.inc.lcssa2
}

define i64 @test_constant_inner_loop_res(ptr %Arr) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc, %entry
  %indvars.iv23 = phi i64 [ 0, %entry ], [ %indvars.iv.next24, %for1.inc ]
  %sum.outer = phi i64 [ 0, %entry ], [ %sum.inc.amend, %for1.inc ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ 0, %for1.header ], [ %indvars.iv.next.3, %for2 ]
  %sum.inner = phi i64 [ %sum.outer, %for1.header ], [ %sum.inc, %for2 ]
  %arrayidx = getelementptr inbounds [100 x [100 x i64]], ptr %Arr, i64 0, i64 %indvars.iv, i64 %indvars.iv23
  %lv = load i64, ptr %arrayidx, align 4
  %sum.inc = add i64 %sum.inner, %lv
  %indvars.iv.next.3 = add nuw nsw i64 %indvars.iv, 1
  %exit1 = icmp eq i64 %indvars.iv.next.3, 100
  br i1 %exit1, label %for1.inc, label %for2

for1.inc:                                         ; preds = %for2
  %sum.inc.lcssa = phi i64 [ %sum.inc, %for2 ]
  %const.lcssa = phi i64 [ 0, %for2 ]
  %sum.inc.amend = add i64 %const.lcssa, %sum.inc.lcssa
  %indvars.iv.next24 = add nuw nsw i64 %indvars.iv23, 1
  %exit2 = icmp eq i64 %indvars.iv.next24, 100
  br i1 %exit2, label %for1.loopexit, label %for1.header

for1.loopexit:                                    ; preds = %for1.inc
  %il.res.lcssa2 = phi i64 [ %sum.inc.amend, %for1.inc ]
  ret i64 %il.res.lcssa2
}

define float @test5(ptr %Arr, ptr %Arr2) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.inc, %entry
  %iv.outer = phi i64 [ 1, %entry ], [ %iv.outer.next, %outer.inc ]
  %float.outer = phi float [ 1.000000e+00, %entry ], [ %float.inner.lcssa, %outer.inc ]
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %outer.header
  %float.inner = phi float [ %float.outer, %outer.header ], [ %float.inner.inc.inc, %for.body3 ]
  %iv.inner = phi i64 [ %iv.inner.next, %for.body3 ], [ 1, %outer.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x float]], ptr %Arr, i64 0, i64 %iv.inner, i64 %iv.outer
  %vA = load float, ptr %arrayidx5, align 4
  %float.inner.inc = fadd fast float %float.inner, %vA
  %arrayidx6 = getelementptr inbounds [100 x [100 x float]], ptr %Arr2, i64 0, i64 %iv.inner, i64 %iv.outer
  %vB = load float, ptr %arrayidx6, align 4
  %float.inner.inc.inc = fadd fast float %float.inner.inc, %vB
  %iv.inner.next = add nuw nsw i64 %iv.inner, 1
  %exitcond = icmp slt i64 %iv.inner.next, 100
  br i1 %exitcond, label %for.body3, label %outer.inc

outer.inc:                                        ; preds = %for.body3
  %float.inner.lcssa = phi float [ %float.inner.inc.inc, %for.body3 ]
  %iv.outer.next = add nsw i64 %iv.outer, 1
  %cmp = icmp slt i64 %iv.outer.next, 100
  br i1 %cmp, label %outer.header, label %for.exit

for.exit:                                         ; preds = %outer.inc
  %float.outer.lcssa = phi float [ %float.inner.lcssa, %outer.inc ]
  ret float %float.outer.lcssa
}

define float @test6(ptr %Arr, ptr %Arr2) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.inc, %entry
  %iv.outer = phi i64 [ 1, %entry ], [ %iv.outer.next, %outer.inc ]
  %float.outer = phi float [ 1.000000e+00, %entry ], [ %float.inner.lcssa, %outer.inc ]
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %outer.header
  %float.inner = phi float [ %float.outer, %outer.header ], [ %float.inner.inc.inc, %for.body3 ]
  %iv.inner = phi i64 [ %iv.inner.next, %for.body3 ], [ 1, %outer.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x float]], ptr %Arr, i64 0, i64 %iv.inner, i64 %iv.outer
  %vA = load float, ptr %arrayidx5, align 4
  %float.inner.inc = fadd float %float.inner, %vA
  %arrayidx6 = getelementptr inbounds [100 x [100 x float]], ptr %Arr2, i64 0, i64 %iv.inner, i64 %iv.outer
  %vB = load float, ptr %arrayidx6, align 4
  %float.inner.inc.inc = fadd fast float %float.inner.inc, %vB
  %iv.inner.next = add nuw nsw i64 %iv.inner, 1
  %exitcond = icmp eq i64 %iv.inner.next, 100
  br i1 %exitcond, label %outer.inc, label %for.body3

outer.inc:                                        ; preds = %for.body3
  %float.inner.lcssa = phi float [ %float.inner.inc.inc, %for.body3 ]
  %iv.outer.next = add nsw i64 %iv.outer, 1
  %cmp = icmp eq i64 %iv.outer.next, 100
  br i1 %cmp, label %outer.header, label %for.exit

for.exit:                                         ; preds = %outer.inc
  %float.outer.lcssa = phi float [ %float.inner.lcssa, %outer.inc ]
  ret float %float.outer.lcssa
}
