; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @confused_subnest_depth3(ptr %A, ptr %C) {
entry:
  br label %loop.h.header

loop.h.header:                                    ; preds = %loop.h.latch, %entry
  %h = phi i64 [ 0, %entry ], [ %h.next, %loop.h.latch ]
  br label %loop.i2.header

loop.i2.header:                                   ; preds = %loop.i2.latch, %loop.h.header
  %i = phi i64 [ 0, %loop.h.header ], [ %i.next, %loop.i2.latch ]
  br label %loop.j2.header

loop.j2.header:                                   ; preds = %loop.j2.latch, %loop.i2.header
  %j = phi i64 [ 0, %loop.i2.header ], [ %j.next, %loop.j2.latch ]
  br label %loop.k2.header

loop.k2.header:                                   ; preds = %loop.k2.latch, %loop.j2.header
  %k = phi i64 [ 0, %loop.j2.header ], [ %k.next, %loop.k2.latch ]
  %c.ptr = getelementptr double, ptr %C, i64 %k
  %c.val = load double, ptr %c.ptr, align 8
  %sum = fadd double %c.val, 1.000000e+00
  %jrow = mul nuw nsw i64 %j, 8
  %aidx = add nuw nsw i64 %jrow, %k
  %a.ptr = getelementptr double, ptr %A, i64 %aidx
  store double %sum, ptr %a.ptr, align 8
  br label %loop.k2.latch

loop.k2.latch:                                    ; preds = %loop.k2.header
  %k.next = add nuw nsw i64 %k, 1
  %k.done = icmp eq i64 %k.next, 8
  br i1 %k.done, label %loop.j2.latch, label %loop.k2.header

loop.j2.latch:                                    ; preds = %loop.k2.latch
  %j.next = add nuw nsw i64 %j, 1
  %j.done = icmp eq i64 %j.next, 8
  br i1 %j.done, label %loop.x2.header.preheader, label %loop.j2.header

loop.x2.header.preheader:                         ; preds = %loop.j2.latch
  br label %loop.x2.header

loop.x2.header:                                   ; preds = %loop.x2.header.preheader, %loop.x2.latch
  %x = phi i64 [ %x.next, %loop.x2.latch ], [ 0, %loop.x2.header.preheader ]
  br label %loop.y2.header

loop.y2.header:                                   ; preds = %loop.y2.latch, %loop.x2.header
  %y = phi i64 [ 0, %loop.x2.header ], [ %y.next, %loop.y2.latch ]
  %row = mul nuw nsw i64 %y, 8
  %idx = add nuw nsw i64 %row, %x
  %axy.ptr = getelementptr double, ptr %A, i64 %idx
  %old = load double, ptr %axy.ptr, align 8
  %new = fadd double %old, 2.000000e+00
  store double %new, ptr %axy.ptr, align 8
  br label %loop.y2.latch

loop.y2.latch:                                    ; preds = %loop.y2.header
  %y.next = add nuw nsw i64 %y, 1
  %y.done = icmp eq i64 %y.next, 8
  br i1 %y.done, label %loop.x2.latch, label %loop.y2.header

loop.x2.latch:                                    ; preds = %loop.y2.latch
  %x.next = add nuw nsw i64 %x, 1
  %x.done = icmp eq i64 %x.next, 8
  br i1 %x.done, label %loop.i2.latch, label %loop.x2.header

loop.i2.latch:                                    ; preds = %loop.x2.latch
  %i.next = add nuw nsw i64 %i, 1
  %i.done = icmp eq i64 %i.next, 8
  br i1 %i.done, label %loop.h.latch, label %loop.i2.header

loop.h.latch:                                     ; preds = %loop.i2.latch
  %h.next = add nuw nsw i64 %h, 1
  %h.done = icmp eq i64 %h.next, 8
  br i1 %h.done, label %exit, label %loop.h.header

exit:                                             ; preds = %loop.h.latch
  ret void
}
