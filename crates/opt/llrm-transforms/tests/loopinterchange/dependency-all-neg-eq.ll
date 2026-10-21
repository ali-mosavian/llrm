; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @all_neg_eq(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  br label %for.j.header

for.j.header:                                     ; preds = %for.j.latch, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.inc, %for.j.latch ]
  %j.inc = add i64 %j, 1
  br label %for.k

for.k:                                            ; preds = %for.k, %for.j.header
  %k = phi i64 [ 0, %for.j.header ], [ %k.inc, %for.k ]
  %ptr.load = getelementptr [256 x i8], ptr %A, i64 %j, i64 %k
  %ld = load i8, ptr %ptr.load, align 1
  %add = add i8 %ld, 1
  %ptr.store = getelementptr [256 x i8], ptr %A, i64 %j.inc, i64 %k
  store i8 %add, ptr %ptr.store, align 1
  %k.inc = add i64 %k, 1
  %ec.k = icmp eq i64 %k.inc, 256
  br i1 %ec.k, label %for.j.latch, label %for.k

for.j.latch:                                      ; preds = %for.k
  %ec.j = icmp eq i64 %j.inc, 255
  br i1 %ec.j, label %for.i.latch, label %for.j.header

for.i.latch:                                      ; preds = %for.j.latch
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 100
  br i1 %ec.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}
