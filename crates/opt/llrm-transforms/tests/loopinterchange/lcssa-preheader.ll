; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

@global = external local_unnamed_addr global [4 x [4 x [2 x i16]]]

define void @lcssa_08(i32 %n, i32 %m) {
entry:
  %temp = alloca [16 x [16 x i32]], align 4
  %res = alloca [16 x [16 x i32]], align 4
  %cmp24 = icmp sgt i32 %n, 0
  br i1 %cmp24, label %outer.preheader, label %for.cond.cleanup

outer.preheader:                                  ; preds = %entry
  %wide.trip.count29 = zext i32 %n to i64
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %outer.preheader
  %indvars.iv27 = phi i64 [ 0, %outer.preheader ], [ %indvars.iv.next28, %outer.latch ]
  %cmp222 = icmp sgt i32 %m, 0
  br i1 %cmp222, label %inner.preheader, label %outer.latch

inner.preheader:                                  ; preds = %outer.header
  %wide.trip.count = zext i32 %m to i64
  br label %inner.for.body

inner.for.body:                                   ; preds = %inner.for.body, %inner.preheader
  %indvars.iv = phi i64 [ 0, %inner.preheader ], [ %indvars.iv.next, %inner.for.body ]
  %arrayidx6 = getelementptr inbounds [16 x [16 x i32]], ptr %temp, i64 0, i64 %indvars.iv, i64 %indvars.iv27
  %0 = load i32, ptr %arrayidx6, align 4
  %arrayidx8 = getelementptr inbounds [16 x [16 x i32]], ptr %res, i64 0, i64 %indvars.iv, i64 %indvars.iv27
  store i32 %0, ptr %arrayidx8, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp ne i64 %indvars.iv.next, %wide.trip.count
  br i1 %exitcond, label %inner.for.body, label %inner.crit_edge

inner.crit_edge:                                  ; preds = %inner.for.body
  br label %outer.latch

outer.latch:                                      ; preds = %inner.crit_edge, %outer.header
  %indvars.iv.next28 = add nuw nsw i64 %indvars.iv27, 1
  %exitcond30 = icmp ne i64 %indvars.iv.next28, %wide.trip.count29
  br i1 %exitcond30, label %outer.header, label %outer.crit_edge

outer.crit_edge:                                  ; preds = %outer.latch
  br label %for.cond.cleanup

for.cond.cleanup:                                 ; preds = %outer.crit_edge, %entry
  ret void
}

define void @test2(i32 %N) {
bb:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %bb
  %outer.iv = phi i64 [ 0, %bb ], [ %outer.iv.next, %outer.latch ]
  %N.ext = sext i32 %N to i64
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %inner.iv = phi i64 [ 0, %outer.header ], [ %inner.iv.next, %inner ]
  %gep = getelementptr inbounds [2 x i16], ptr @global, i64 %inner.iv, i64 %outer.iv
  store i16 0, ptr %gep, align 2
  %inner.iv.next = add nsw i64 %inner.iv, 1
  %c.1 = icmp ne i64 %inner.iv.next, %N.ext
  br i1 %c.1, label %inner, label %outer.latch

outer.latch:                                      ; preds = %inner
  %outer.iv.next = add nsw i64 %outer.iv, 1
  %c.2 = icmp ne i64 %outer.iv, %N.ext
  br i1 %c.2, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  ret void
}

