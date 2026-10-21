; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @fixed_size_5x5(ptr noalias %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i.count = phi i64 [ 5, %entry ], [ %i.count.next, %outer.latch ]
  %i = phi i32 [ 1, %entry ], [ %i.next, %outer.latch ]
  %i.ext = zext nneg i32 %i to i64
  %row.gep = getelementptr [4 x i8], ptr %A, i64 %i.ext
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 1, %outer.header ], [ %j.next, %inner ]
  %j.count = phi i64 [ 5, %outer.header ], [ %j.count.next, %inner ]
  %col.off = mul nuw nsw i64 %j, 20
  %elt.gep = getelementptr i8, ptr %row.gep, i64 %col.off
  %addr = getelementptr i8, ptr %elt.gep, i64 -24
  %v = load float, ptr %addr, align 4
  %inc = fadd contract float %v, 1.000000e+00
  store float %inc, ptr %addr, align 4
  %j.next = add nuw nsw i64 %j, 1
  %j.count.next = add nsw i64 %j.count, -1
  %j.done = icmp eq i64 %j.count.next, 0
  br i1 %j.done, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %i.next = add nuw nsw i32 %i, 1
  %i.count.next = add nsw i64 %i.count, -1
  %i.cmp = icmp sgt i64 %i.count, 1
  br i1 %i.cmp, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  ret void
}
