; ModuleID = '<stdin>'
source_filename = "<stdin>"

@a = global [256 x [256 x float]] zeroinitializer, align 4
@b = global [20 x [20 x [20 x i32]]] zeroinitializer, align 4

define void @all_eq_lt() {
entry:
  br label %for.n.header

for.n.header:                                     ; preds = %for.n.latch, %entry
  %n = phi i32 [ 0, %entry ], [ %n.inc, %for.n.latch ]
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %for.n.header
  %i = phi i32 [ 0, %for.n.header ], [ %i.inc, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 1, %for.i.header ], [ %j.inc, %for.j ]
  %j.dec = sub nsw i32 %j, 1
  %idx.store = getelementptr inbounds [256 x [256 x float]], ptr @a, i32 0, i32 %j.dec, i32 %i
  %idx.load = getelementptr inbounds [256 x [256 x float]], ptr @a, i32 0, i32 %j, i32 %i
  %0 = load float, ptr %idx.load, align 4
  %1 = load float, ptr %idx.store, align 4
  %add = fadd fast float %0, %1
  store float %add, ptr %idx.store, align 4
  %j.inc = add nuw nsw i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 256
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %i.inc = add nuw nsw i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 256
  br i1 %cmp.i, label %for.i.header, label %for.n.latch

for.n.latch:                                      ; preds = %for.i.latch
  %n.inc = add nuw nsw i32 %n, 1
  %cmp.n = icmp slt i32 %n.inc, 100
  br i1 %cmp.n, label %for.n.header, label %exit

exit:                                             ; preds = %for.n.latch
  ret void
}

define void @all_lt() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %i.rev = sub nsw i32 255, %i
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 1, %for.i.header ], [ %j.inc, %for.j ]
  %j.dec = sub nsw i32 %j, 1
  %idx.store = getelementptr inbounds [256 x [256 x float]], ptr @a, i32 0, i32 %j.dec, i32 %i
  %idx.load = getelementptr inbounds [256 x [256 x float]], ptr @a, i32 0, i32 %j, i32 %i.rev
  %0 = load float, ptr %idx.load, align 4
  store float %0, ptr %idx.store, align 4
  %j.inc = add nuw nsw i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 256
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %i.inc = add nuw nsw i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 256
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @lt_gt() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %i.inc = add nuw nsw i32 %i, 1
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 1, %for.i.header ], [ %j.inc, %for.j ]
  %j.dec = sub nsw i32 %j, 1
  %idx.store = getelementptr inbounds [256 x [256 x float]], ptr @a, i32 0, i32 %j, i32 %i
  %idx.load = getelementptr inbounds [256 x [256 x float]], ptr @a, i32 0, i32 %j.dec, i32 %i.inc
  %0 = load float, ptr %idx.load, align 4
  store float %0, ptr %idx.store, align 4
  %j.inc = add nuw nsw i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 256
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %cmp.i = icmp slt i32 %i.inc, 255
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @eq_all_lt() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  br label %for.j.header

for.j.header:                                     ; preds = %for.j.latch, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j.latch ]
  br label %for.k

for.k:                                            ; preds = %for.k, %for.j.header
  %k = phi i32 [ 0, %for.j.header ], [ %k.inc, %for.k ]
  %k.inc = add nuw nsw i32 %k, 1
  %idx.store = getelementptr inbounds [20 x [20 x [20 x i32]]], ptr @b, i32 0, i32 %i, i32 %j, i32 %k
  %idx.load = getelementptr inbounds [20 x [20 x [20 x i32]]], ptr @b, i32 0, i32 %i, i32 5, i32 %k.inc
  %0 = load i32, ptr %idx.load, align 4
  store i32 %0, ptr %idx.store, align 4
  %cmp.k = icmp slt i32 %k.inc, 19
  br i1 %cmp.k, label %for.k, label %for.j.latch

for.j.latch:                                      ; preds = %for.k
  %j.inc = add nuw nsw i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 20
  br i1 %cmp.j, label %for.j.header, label %for.i.latch

for.i.latch:                                      ; preds = %for.j.latch
  %i.inc = add nuw nsw i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 20
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}
