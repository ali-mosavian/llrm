; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = global [16 x [16 x i32]] zeroinitializer

define void @legal_to_interchange() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.next, %for.j ]
  %idx = getelementptr inbounds [16 x [16 x i32]], ptr @A, i32 0, i32 %j, i32 %i
  %val = load i32, ptr %idx, align 4
  %inc = add i32 %val, 1
  store i32 %inc, ptr %idx, align 4
  %j.next = add i32 %j, 1
  %j.exit = icmp eq i32 %j.next, 16
  br i1 %j.exit, label %for.i.latch, label %for.j

for.i.latch:                                      ; preds = %for.j
  %i.next = add i32 %i, 1
  %i.exit = icmp eq i32 %i.next, 16
  br i1 %i.exit, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @illegal_to_interchange() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.next, %for.j ]
  %idx.load = getelementptr inbounds [16 x [16 x i32]], ptr @A, i32 0, i32 %i, i32 %j
  %idx.store = getelementptr inbounds [16 x [16 x i32]], ptr @A, i32 0, i32 %j, i32 %i
  %val = load i32, ptr %idx.load, align 4
  store i32 %val, ptr %idx.store, align 4
  %j.next = add i32 %j, 1
  %j.exit = icmp eq i32 %j.next, 16
  br i1 %j.exit, label %for.i.latch, label %for.j

for.i.latch:                                      ; preds = %for.j
  %i.next = add i32 %i, 1
  %i.exit = icmp eq i32 %i.next, 16
  br i1 %i.exit, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}
