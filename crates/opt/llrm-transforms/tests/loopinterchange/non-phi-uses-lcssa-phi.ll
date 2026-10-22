; ModuleID = '<stdin>'
source_filename = "<stdin>"

define i64 @non_phi_lcssa_phi_user(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %ptr = getelementptr inbounds [100 x i32], ptr %A, i64 %j, i64 %i
  store i32 0, ptr %ptr, align 4
  %j.next = add i64 %j, 1
  %exitcond.inner = icmp eq i64 %j, 99
  br i1 %exitcond.inner, label %inner.exit, label %inner

inner.exit:                                       ; preds = %inner
  %j.lcssa = phi i64 [ %j, %inner ]
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %j.latch = phi i64 [ %j.lcssa, %inner.exit ]
  %use.j = add i64 %j.latch, 1
  %i.next = add i64 %i, 1
  %exitcond.outer = icmp eq i64 %i, 99
  br i1 %exitcond.outer, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %result = phi i64 [ %use.j, %outer.latch ]
  ret i64 %result
}

define void @non_phi_lcssa_phi_user2(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.inc, %inner ]
  %p = getelementptr [4 x i8], ptr %A, i64 %j, i64 %i
  %v = load i8, ptr %p, align 1
  %add = add i8 %v, 1
  store i8 %add, ptr %p, align 1
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 4
  br i1 %ec.j, label %outer.body, label %inner

outer.body:                                       ; preds = %inner
  %lcssa.add.0 = phi i8 [ %add, %inner ]
  br label %outer.latch

outer.latch:                                      ; preds = %outer.body
  %lcssa.add.1 = phi i8 [ %lcssa.add.0, %outer.body ]
  %use = add i8 %lcssa.add.1, 1
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 4
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
