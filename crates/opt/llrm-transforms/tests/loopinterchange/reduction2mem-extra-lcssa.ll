; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = common global [100 x [100 x i32]] zeroinitializer
@sum = common global [100 x i32] zeroinitializer

define void @reduction_lcssa_with_non_phi_user() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %sum.ptr = getelementptr inbounds i32, ptr @sum, i64 %i
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %red = phi i32 [ 0, %outer.header ], [ %add, %inner ]
  %ptr = getelementptr inbounds [100 x i32], ptr @A, i64 %j, i64 %i
  %val = load i32, ptr %ptr, align 4
  %add = add i32 %red, %val
  %j.next = add nuw nsw i64 %j, 1
  %exitcond.inner = icmp eq i64 %j, 99
  br i1 %exitcond.inner, label %inner.exit, label %inner

inner.exit:                                       ; preds = %inner
  %red.lcssa = phi i32 [ %add, %inner ]
  %j.lcssa = phi i64 [ %j, %inner ]
  store i32 %red.lcssa, ptr %sum.ptr, align 4
  %use.j = add i64 %j.lcssa, 1
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %i.next = add nuw nsw i64 %i, 1
  %exitcond.outer = icmp eq i64 %i, 99
  br i1 %exitcond.outer, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
