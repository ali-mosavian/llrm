; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @profitable(ptr %A) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  br label %loop.j

loop.j:                                           ; preds = %loop.j, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.inc, %loop.j ]
  %mul = mul i64 %j, 100
  %offset = add i64 %i, %mul
  %gep = getelementptr inbounds i8, ptr %A, i64 %offset
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

define void @profitable_neg_step(ptr %A) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  br label %loop.j

loop.j:                                           ; preds = %loop.j, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.inc, %loop.j ]
  %j.rev = sub i64 9, %j
  %mul = mul i64 %j.rev, 100
  %offset = add i64 %i, %mul
  %gep = getelementptr inbounds i8, ptr %A, i64 %offset
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

define void @unprofitable0(ptr %A) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  %mul = mul i64 %i, 100
  br label %loop.j

loop.j:                                           ; preds = %loop.j, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.inc, %loop.j ]
  %gep = getelementptr inbounds [1 x i8], ptr %A, i64 %j, i64 %mul
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

define void @unprofitable1(ptr noalias %A, ptr noalias %B, ptr noalias %C) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %loop.i.latch ]
  br label %loop.j

loop.j:                                           ; preds = %loop.j, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.inc, %loop.j ]
  %gep.A = getelementptr inbounds [100 x i8], ptr %A, i64 %i, i64 %j
  %gep.B = getelementptr inbounds [100 x i8], ptr %B, i64 %i, i64 %j
  %gep.C0 = getelementptr inbounds [100 x i8], ptr %C, i64 %j, i64 %i
  %gep.C1 = getelementptr inbounds [99 x i8], ptr %C, i64 %j, i64 %i
  %gep.C2 = getelementptr inbounds [98 x i8], ptr %C, i64 %j, i64 %i
  %val.B = load i8, ptr %gep.B, align 1
  %val.C0 = load i8, ptr %gep.C0, align 1
  %val.C1 = load i8, ptr %gep.C1, align 1
  %val.C2 = load i8, ptr %gep.C2, align 1
  %sum.0 = add i8 %val.B, %val.C0
  %sum.1 = add i8 %sum.0, %val.C1
  %sum.2 = add i8 %sum.1, %val.C2
  store i8 %sum.2, ptr %gep.A, align 1
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 42
  br i1 %ec.j, label %loop.i.latch, label %loop.j

loop.i.latch:                                     ; preds = %loop.j
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 42
  br i1 %ec.i, label %exit, label %loop.i.header

exit:                                             ; preds = %loop.i.latch
  ret void
}
