; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(ptr noalias %A, ptr noalias %B) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j.header

for.j.header:                                     ; preds = %for.j.latch, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.next, %for.j.latch ]
  br label %for.r.header

for.r.header:                                     ; preds = %for.r.header, %for.j.header
  %r = phi i64 [ 0, %for.j.header ], [ %r.next, %for.r.header ]
  %a.element = getelementptr [64 x i8], ptr %A, i64 %j, i64 %r
  store i8 0, ptr %a.element, align 1
  %r.next = add i64 %r, 1
  %r.done = icmp eq i64 %r.next, 64
  br i1 %r.done, label %for.j.latch, label %for.r.header

for.j.latch:                                      ; preds = %for.r.header
  %j.next = add i64 %j, 1
  %j.done = icmp eq i64 %j.next, 64
  br i1 %j.done, label %for.k.header.preheader, label %for.j.header

for.k.header.preheader:                           ; preds = %for.j.latch
  br label %for.k.header

for.k.header:                                     ; preds = %for.k.header.preheader, %for.k.latch
  %k = phi i64 [ %k.next, %for.k.latch ], [ 0, %for.k.header.preheader ]
  br label %for.l.header

for.l.header:                                     ; preds = %for.l.header, %for.k.header
  %l = phi i64 [ 0, %for.k.header ], [ %l.next, %for.l.header ]
  %b.element = getelementptr [64 x i8], ptr %B, i64 %l, i64 %k
  store i8 0, ptr %b.element, align 1
  %l.next = add i64 %l, 1
  %l.done = icmp eq i64 %l.next, 64
  br i1 %l.done, label %for.k.latch, label %for.l.header

for.k.latch:                                      ; preds = %for.l.header
  %k.next = add i64 %k, 1
  %k.done = icmp eq i64 %k.next, 64
  br i1 %k.done, label %for.i.latch, label %for.k.header

for.i.latch:                                      ; preds = %for.k.latch
  %i.next = add i64 %i, 1
  %i.done = icmp eq i64 %i.next, 64
  br i1 %i.done, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}
