; ModuleID = '<stdin>'
source_filename = "<stdin>"

define i8 @extra_reduction_use_in_inner0(ptr noalias %A, ptr noalias %B) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %sum.outer = phi i8 [ 0, %entry ], [ %sum.lcssa, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %sum.inner = phi i8 [ %sum.outer, %outer.header ], [ %sum.inner.next, %inner ]
  %gep.A = getelementptr [2 x i8], ptr %A, i64 %j, i64 %i
  %a = load i8, ptr %gep.A, align 1
  %sum.inner.next = add i8 %sum.inner, %a
  %gep.B = getelementptr [2 x i8], ptr %B, i64 %j, i64 %i
  store i8 %sum.outer, ptr %gep.B, align 1
  %j.next = add i64 %j, 1
  %ec.j = icmp eq i64 %j.next, 2
  br i1 %ec.j, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %sum.lcssa = phi i8 [ %sum.inner.next, %inner ]
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 2
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %res = phi i8 [ %sum.lcssa, %outer.latch ]
  ret i8 %res
}

define i8 @extra_reduction_use_in_inner1(ptr noalias %A, ptr noalias %B) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %sum.outer = phi i8 [ 0, %entry ], [ %sum.lcssa, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %sum.inner = phi i8 [ %sum.outer, %outer.header ], [ %sum.inner.next, %inner ]
  %gep.A = getelementptr [2 x i8], ptr %A, i64 %j, i64 %i
  %a = load i8, ptr %gep.A, align 1
  %sum.inner.next = add i8 %sum.inner, %a
  %gep.B = getelementptr [2 x i8], ptr %B, i64 %j, i64 %i
  store i8 %sum.inner.next, ptr %gep.B, align 1
  %j.next = add i64 %j, 1
  %ec.j = icmp eq i64 %j.next, 2
  br i1 %ec.j, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %sum.lcssa = phi i8 [ %sum.inner.next, %inner ]
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 2
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %res = phi i8 [ %sum.lcssa, %outer.latch ]
  ret i8 %res
}

define i8 @extra_reduction_use_in_inner2(ptr noalias %A, ptr noalias %B) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %sum.outer = phi i8 [ 0, %entry ], [ %sum.outer.lcssa, %outer.latch ]
  br label %middle.header

middle.header:                                    ; preds = %middle.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %middle.latch ]
  %sum.middle = phi i8 [ %sum.outer, %outer.header ], [ %sum.middle.lcssa, %middle.latch ]
  br label %inner

inner:                                            ; preds = %inner, %middle.header
  %k = phi i64 [ 0, %middle.header ], [ %k.next, %inner ]
  %sum.inner = phi i8 [ %sum.middle, %middle.header ], [ %sum.inner.next, %inner ]
  %gep.A = getelementptr [2 x [2 x i8]], ptr %A, i64 %k, i64 %j, i64 %i
  %a = load i8, ptr %gep.A, align 1
  %sum.inner.next = add i8 %sum.inner, %a
  %gep.B = getelementptr [2 x [2 x i8]], ptr %B, i64 %k, i64 %j, i64 %i
  store i8 %sum.outer, ptr %gep.B, align 1
  %k.next = add i64 %k, 1
  %ec.k = icmp eq i64 %k.next, 2
  br i1 %ec.k, label %middle.latch, label %inner

middle.latch:                                     ; preds = %inner
  %sum.middle.lcssa = phi i8 [ %sum.inner.next, %inner ]
  %j.next = add i64 %j, 1
  %ec.j = icmp eq i64 %j.next, 2
  br i1 %ec.j, label %outer.latch, label %middle.header

outer.latch:                                      ; preds = %middle.latch
  %sum.outer.lcssa = phi i8 [ %sum.middle.lcssa, %middle.latch ]
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 2
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %res = phi i8 [ %sum.outer.lcssa, %outer.latch ]
  ret i8 %res
}
