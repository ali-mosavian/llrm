; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(ptr noalias %A) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  br label %loop.j

loop.j:                                           ; preds = %loop.j, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.inc, %loop.j ]
  %gep = getelementptr [10 x i8], ptr %A, i64 %j, i64 %i
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  store i8 0, ptr %gep, align 1
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 10
  br i1 %ec.j, label %loop.i.latch, label %loop.j

loop.i.latch:                                     ; preds = %loop.j
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 10
  br i1 %ec.i, label %exit, label %loop.i.header

exit:                                             ; preds = %loop.i.latch
  ret void
}
