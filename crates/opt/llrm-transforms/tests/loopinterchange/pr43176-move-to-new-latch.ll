; ModuleID = '<stdin>'
source_filename = "<stdin>"

@b = external global [5 x i32], align 16

define void @test1() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i32 [ %i.next, %outer.latch ], [ 0, %entry ]
  %idxprom = sext i32 %i to i64
  %arrayidx = getelementptr inbounds [5 x i32], ptr @b, i64 0, i64 %idxprom
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i32 [ %j.next, %inner.latch ], [ 1, %outer.header ]
  br label %inner.latch

inner.latch:                                      ; preds = %inner.header
  %0 = load i32, ptr %arrayidx, align 4
  store i32 undef, ptr %arrayidx, align 4
  %cmp = icmp slt i32 %j, 4
  %j.next = add nuw nsw i32 %j, 1
  br i1 %cmp, label %inner.header, label %outer.body

outer.body:                                       ; preds = %inner.latch
  br label %outer.latch

outer.latch:                                      ; preds = %outer.body
  %i.next = add nsw i32 %i, 1
  %cmp2 = icmp slt i32 %i, 4
  br i1 %cmp2, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  ret void
}

define void @test2() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i32 [ %i.next, %outer.latch ], [ 0, %entry ]
  %idxprom = sext i32 %i to i64
  %arrayidx = getelementptr inbounds [5 x i32], ptr @b, i64 0, i64 %idxprom
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %lsr.iv = phi i32 [ %lsr.iv.next, %inner.latch ], [ 1, %outer.header ]
  br label %inner.latch

inner.latch:                                      ; preds = %inner.header
  %0 = load i32, ptr %arrayidx, align 4
  %cmp = icmp slt i32 %lsr.iv, 4
  %cmp.zext = zext i1 %cmp to i32
  store i32 %cmp.zext, ptr %arrayidx, align 4
  %lsr.iv.next = add nuw nsw i32 %lsr.iv, 1
  br i1 %cmp, label %inner.header, label %outer.body

outer.body:                                       ; preds = %inner.latch
  br label %outer.latch

outer.latch:                                      ; preds = %outer.body
  %i.next = add nsw i32 %i, 1
  %cmp2 = icmp slt i32 %i, 4
  br i1 %cmp2, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  ret void
}
