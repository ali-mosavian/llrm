; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

define i16 @main(ptr %a) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 1, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i16 [ 0, %outer.header ], [ %j.next, %inner.latch ]
  %arrayidx.us.us = getelementptr i16, ptr %a, i16 %j
  %0 = load i16, ptr %arrayidx.us.us, align 1
  %cond = select i1 false, i16 0, i16 0
  br label %inner.latch

inner.latch:                                      ; preds = %inner.header
  %j.next = add i16 %j, 1
  %cmp = icmp slt i16 %j, 42
  br i1 %cmp, label %inner.header, label %outer.body

outer.body:                                       ; preds = %inner.latch
  %new.cond.lcssa = phi i16 [ %cond, %inner.latch ]
  br label %outer.latch

outer.latch:                                      ; preds = %outer.body
  %old.cond.lcssa = phi i16 [ %new.cond.lcssa, %outer.body ]
  %i.next = add i64 %i, 1
  %cmp286.us = icmp ugt i64 %i, 0
  br i1 %cmp286.us, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  %old.cond.lcssa.lcssa = phi i16 [ %old.cond.lcssa, %outer.latch ]
  ret i16 %old.cond.lcssa.lcssa
}
