; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(ptr noalias %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %pair.j

pair.j:                                           ; preds = %pair.j.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %pair.j.latch ]
  br label %pair.k

pair.k:                                           ; preds = %pair.k, %pair.j
  %k = phi i64 [ 0, %pair.j ], [ %k.next, %pair.k ]
  %idx = getelementptr [64 x i8], ptr %A, i64 %k, i64 %j
  store i8 0, ptr %idx, align 1
  %k.next = add i64 %k, 1
  %k.done = icmp eq i64 %k.next, 64
  br i1 %k.done, label %pair.j.latch, label %pair.k

pair.j.latch:                                     ; preds = %pair.k
  %j.next = add i64 %j, 1
  %j.done = icmp eq i64 %j.next, 64
  br i1 %j.done, label %sib1.header.preheader, label %pair.j

sib1.header.preheader:                            ; preds = %pair.j.latch
  br label %sib1.header

sib1.header:                                      ; preds = %sib1.header.preheader, %sib1.header
  %s1 = phi i64 [ %s1.next, %sib1.header ], [ 0, %sib1.header.preheader ]
  %s1.next = add i64 %s1, 1
  %s1.done = icmp eq i64 %s1.next, 4
  br i1 %s1.done, label %sib2.header.preheader, label %sib1.header

sib2.header.preheader:                            ; preds = %sib1.header
  br label %sib2.header

sib2.header:                                      ; preds = %sib2.header.preheader, %sib2.header
  %s2 = phi i64 [ %s2.next, %sib2.header ], [ 0, %sib2.header.preheader ]
  %s2.next = add i64 %s2, 1
  %s2.done = icmp eq i64 %s2.next, 4
  br i1 %s2.done, label %outer.latch, label %sib2.header

outer.latch:                                      ; preds = %sib2.header
  %i.next = add i64 %i, 1
  %i.done = icmp eq i64 %i.next, 64
  br i1 %i.done, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
