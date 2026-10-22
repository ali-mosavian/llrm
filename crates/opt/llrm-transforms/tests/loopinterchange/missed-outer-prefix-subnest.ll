; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @missed_outer_prefix_subnest(ptr noalias %Left, ptr noalias %Right, ptr noalias %A) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop.i.latch ]
  br label %loop.j.header

loop.j.header:                                    ; preds = %loop.j.latch, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.next, %loop.j.latch ]
  %a.row = mul nuw nsw i64 %i, 8
  %a.idx = add nuw nsw i64 %a.row, %j
  %a.ptr = getelementptr i8, ptr %A, i64 %a.idx
  store i8 0, ptr %a.ptr, align 1
  br label %loop.k.header

loop.k.header:                                    ; preds = %loop.k.latch, %loop.j.header
  %k = phi i64 [ 0, %loop.j.header ], [ %k.next, %loop.k.latch ]
  br label %loop.l.header

loop.l.header:                                    ; preds = %loop.l.latch, %loop.k.header
  %l = phi i64 [ 0, %loop.k.header ], [ %l.next, %loop.l.latch ]
  br label %loop.m.header

loop.m.header:                                    ; preds = %loop.m.latch, %loop.l.header
  %m = phi i64 [ 0, %loop.l.header ], [ %m.next, %loop.m.latch ]
  %left.row.base = mul nuw nsw i64 %m, 8
  %left.index = add nuw nsw i64 %left.row.base, %l
  %left.element.ptr = getelementptr i8, ptr %Left, i64 %left.index
  store i8 0, ptr %left.element.ptr, align 1
  br label %loop.m.latch

loop.m.latch:                                     ; preds = %loop.m.header
  %m.next = add nuw nsw i64 %m, 1
  %m.done = icmp eq i64 %m.next, 8
  br i1 %m.done, label %loop.l.latch, label %loop.m.header

loop.l.latch:                                     ; preds = %loop.m.latch
  %l.next = add nuw nsw i64 %l, 1
  %l.done = icmp eq i64 %l.next, 8
  br i1 %l.done, label %loop.n.header.preheader, label %loop.l.header

loop.n.header.preheader:                          ; preds = %loop.l.latch
  br label %loop.n.header

loop.n.header:                                    ; preds = %loop.n.header.preheader, %loop.n.latch
  %n = phi i64 [ %n.next, %loop.n.latch ], [ 0, %loop.n.header.preheader ]
  br label %loop.o.header

loop.o.header:                                    ; preds = %loop.o.latch, %loop.n.header
  %o = phi i64 [ 0, %loop.n.header ], [ %o.next, %loop.o.latch ]
  %right.row.base = mul nuw nsw i64 %o, 8
  %right.index = add nuw nsw i64 %right.row.base, %n
  %right.element.ptr = getelementptr i8, ptr %Right, i64 %right.index
  store i8 0, ptr %right.element.ptr, align 1
  br label %loop.o.latch

loop.o.latch:                                     ; preds = %loop.o.header
  %o.next = add nuw nsw i64 %o, 1
  %o.done = icmp eq i64 %o.next, 8
  br i1 %o.done, label %loop.n.latch, label %loop.o.header

loop.n.latch:                                     ; preds = %loop.o.latch
  %n.next = add nuw nsw i64 %n, 1
  %n.done = icmp eq i64 %n.next, 8
  br i1 %n.done, label %loop.k.latch, label %loop.n.header

loop.k.latch:                                     ; preds = %loop.n.latch
  %k.next = add nuw nsw i64 %k, 1
  %k.done = icmp eq i64 %k.next, 8
  br i1 %k.done, label %loop.j.latch, label %loop.k.header

loop.j.latch:                                     ; preds = %loop.k.latch
  %j.next = add nuw nsw i64 %j, 1
  %j.done = icmp eq i64 %j.next, 8
  br i1 %j.done, label %loop.i.latch, label %loop.j.header

loop.i.latch:                                     ; preds = %loop.j.latch
  %i.next = add nuw nsw i64 %i, 1
  %i.done = icmp eq i64 %i.next, 8
  br i1 %i.done, label %exit, label %loop.i.header

exit:                                             ; preds = %loop.i.latch
  ret void
}
