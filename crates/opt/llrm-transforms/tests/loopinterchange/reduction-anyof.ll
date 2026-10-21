; ModuleID = '<stdin>'
source_filename = "<stdin>"

define i8 @reduce_anyof(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %red.outer = phi i8 [ 0, %entry ], [ %red.lcssa, %for.i.latch ]
  %add = add i64 %i, 1
  %add8 = trunc i64 %add to i8
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %red = phi i8 [ %red.outer, %for.i.header ], [ %sel, %for.j ]
  %arrayidx = getelementptr inbounds [2 x i8], ptr %A, i64 %j, i64 %i
  %lv = load i8, ptr %arrayidx, align 4
  %cmp = icmp eq i8 %lv, 0
  %sel = select i1 %cmp, i8 %add8, i8 %red
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 2
  br i1 %ec.j, label %for.i.latch, label %for.j

for.i.latch:                                      ; preds = %for.j
  %red.lcssa = phi i8 [ %sel, %for.j ]
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 2
  br i1 %ec.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  %res = phi i8 [ %red.lcssa, %for.i.latch ]
  ret i8 %res
}
