; ModuleID = '<stdin>'
source_filename = "<stdin>"

@global = external global [1000 x [1000 x i32]], align 16

define void @foo(i1 %cmp) {
entry:
  br i1 %cmp, label %outer.ph, label %outer.ph

outer.ph:                                         ; preds = %entry, %entry
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %outer.ph
  %outer.iv = phi i64 [ 0, %outer.ph ], [ %outer.iv.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %inner.iv = phi i64 [ %inner.iv.next, %inner.header ], [ 5, %outer.header ]
  %ptr = getelementptr inbounds [1000 x [1000 x i32]], ptr @global, i64 0, i64 %inner.iv, i64 %outer.iv
  %lv = load i32, ptr %ptr, align 4
  %v = mul i32 %lv, 100
  store i32 %v, ptr %ptr, align 4
  %inner.iv.next = add nsw i64 %inner.iv, 1
  %cond1 = icmp eq i64 %inner.iv.next, 1000
  br i1 %cond1, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %outer.iv.next = add nuw nsw i64 %outer.iv, 1
  %cond2 = icmp eq i64 %outer.iv.next, 1000
  br i1 %cond2, label %outer.exit, label %outer.header

outer.exit:                                       ; preds = %outer.latch
  br label %exit

exit:                                             ; preds = %outer.exit
  ret void
}

define void @foo1(i1 %cmp) {
entry:
  br i1 %cmp, label %outer.ph, label %outer.ph

outer.ph:                                         ; preds = %entry, %entry
  br i1 %cmp, label %outer.header.preheader, label %outer.header.preheader

outer.header.preheader:                           ; preds = %outer.ph, %outer.ph
  br label %outer.header

outer.header:                                     ; preds = %outer.header.preheader, %outer.latch
  %outer.iv = phi i64 [ %outer.iv.next, %outer.latch ], [ 0, %outer.header.preheader ]
  br i1 %cmp, label %inner.header.preheader, label %inner.header.preheader

inner.header.preheader:                           ; preds = %outer.header, %outer.header
  br label %inner.header

inner.header:                                     ; preds = %inner.header.preheader, %inner.header
  %inner.iv = phi i64 [ %inner.iv.next, %inner.header ], [ 5, %inner.header.preheader ]
  %ptr = getelementptr inbounds [1000 x [1000 x i32]], ptr @global, i64 0, i64 %inner.iv, i64 %outer.iv
  %lv = load i32, ptr %ptr, align 4
  %v = mul i32 %lv, 100
  store i32 %v, ptr %ptr, align 4
  %inner.iv.next = add nsw i64 %inner.iv, 1
  %cond1 = icmp eq i64 %inner.iv.next, 1000
  br i1 %cond1, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %outer.iv.next = add nuw nsw i64 %outer.iv, 1
  %cond2 = icmp eq i64 %outer.iv.next, 1000
  br i1 %cond2, label %outer.exit, label %outer.header

outer.exit:                                       ; preds = %outer.latch
  br label %exit

exit:                                             ; preds = %outer.exit
  ret void
}
