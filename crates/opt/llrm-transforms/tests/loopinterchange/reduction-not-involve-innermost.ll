; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f() {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  %red.i = phi i8 [ 0, %entry ], [ %red.j.next.lcssa, %loop.i.latch ]
  br label %loop.j.header

loop.j.header:                                    ; preds = %loop.j.latch, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.inc, %loop.j.latch ]
  %red.j = phi i8 [ %red.i, %loop.i.header ], [ %red.j.next, %loop.j.latch ]
  br label %loop.k

loop.k:                                           ; preds = %loop.k, %loop.j.header
  %k = phi i64 [ 0, %loop.j.header ], [ %k.inc, %loop.k ]
  %k.inc = add i64 %k, 1
  %ec.k = icmp eq i64 %k.inc, 10
  br i1 %ec.k, label %loop.j.latch, label %loop.k

loop.j.latch:                                     ; preds = %loop.k
  %red.j.next = or i8 %red.j, 42
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 20
  br i1 %ec.j, label %loop.i.latch, label %loop.j.header

loop.i.latch:                                     ; preds = %loop.j.latch
  %red.j.next.lcssa = phi i8 [ %red.j.next, %loop.j.latch ]
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 30
  br i1 %ec.i, label %exit, label %loop.i.header

exit:                                             ; preds = %loop.i.latch
  ret void
}
