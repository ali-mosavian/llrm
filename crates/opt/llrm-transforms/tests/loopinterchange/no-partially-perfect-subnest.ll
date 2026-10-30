; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @no_partially_perfect_subnest(ptr noalias %A, ptr noalias %B) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop.i.latch ]
  br label %loop.j.header

loop.j.header:                                    ; preds = %loop.j.latch, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.next, %loop.j.latch ]
  %a.row.base = mul nuw nsw i64 %i, 16
  %a.index = add nuw nsw i64 %a.row.base, %j
  %a.element.ptr = getelementptr i8, ptr %A, i64 %a.index
  store i8 1, ptr %a.element.ptr, align 1
  br label %loop.j.latch

loop.j.latch:                                     ; preds = %loop.j.header
  %j.next = add nuw nsw i64 %j, 1
  %j.done = icmp eq i64 %j.next, 16
  br i1 %j.done, label %loop.k.header.preheader, label %loop.j.header

loop.k.header.preheader:                          ; preds = %loop.j.latch
  br label %loop.k.header

loop.k.header:                                    ; preds = %loop.k.header.preheader, %loop.k.latch
  %k = phi i64 [ %k.next, %loop.k.latch ], [ 0, %loop.k.header.preheader ]
  %b.row.base = mul nuw nsw i64 %i, 16
  %b.index = add nuw nsw i64 %b.row.base, %k
  %b.element.ptr = getelementptr i8, ptr %B, i64 %b.index
  store i8 2, ptr %b.element.ptr, align 1
  br label %loop.k.latch

loop.k.latch:                                     ; preds = %loop.k.header
  %k.next = add nuw nsw i64 %k, 1
  %k.done = icmp eq i64 %k.next, 16
  br i1 %k.done, label %loop.i.latch, label %loop.k.header

loop.i.latch:                                     ; preds = %loop.k.latch
  %i.next = add nuw nsw i64 %i, 1
  %i.done = icmp eq i64 %i.next, 16
  br i1 %i.done, label %exit, label %loop.i.header

exit:                                             ; preds = %loop.i.latch
  ret void
}
