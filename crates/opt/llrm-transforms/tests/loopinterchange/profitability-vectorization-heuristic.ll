; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = global [256 x [256 x float]] zeroinitializer
@B = global [256 x [256 x float]] zeroinitializer
@C = global [256 x [256 x float]] zeroinitializer
@D = global [256 x [256 x [256 x float]]] zeroinitializer
@E = global [256 x [256 x [256 x float]]] zeroinitializer

define void @interchange_necessary_for_vectorization() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.inc, %entry
  %i = phi i64 [ 1, %entry ], [ %i.next, %for.i.inc ]
  br label %for.j.body

for.j.body:                                       ; preds = %for.j.body, %for.i.header
  %j = phi i64 [ 1, %for.i.header ], [ %j.next, %for.j.body ]
  %j.dec = add nsw i64 %j, -1
  %a.load.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @A, i64 0, i64 %i, i64 %j.dec
  %b.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @B, i64 0, i64 %i, i64 %j
  %c.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @C, i64 0, i64 %i, i64 %j
  %a = load float, ptr %a.load.index, align 4
  %b = load float, ptr %b.index, align 4
  %c = load float, ptr %c.index, align 4
  %add.0 = fadd float %a, %b
  %a.store.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @A, i64 0, i64 %i, i64 %j
  store float %add.0, ptr %a.store.index, align 4
  %add.1 = fadd float %c, 1.000000e+00
  store float %add.1, ptr %c.index, align 4
  %j.next = add nuw nsw i64 %j, 1
  %cmp.j = icmp eq i64 %j.next, 256
  br i1 %cmp.j, label %for.i.inc, label %for.j.body

for.i.inc:                                        ; preds = %for.j.body
  %i.next = add nuw nsw i64 %i, 1
  %cmp.i = icmp eq i64 %i.next, 256
  br i1 %cmp.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.inc
  ret void
}

define void @interchange_unnecesasry_for_vectorization() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.inc, %entry
  %i = phi i64 [ 1, %entry ], [ %i.next, %for.i.inc ]
  br label %for.j.body

for.j.body:                                       ; preds = %for.j.body, %for.i.header
  %j = phi i64 [ 1, %for.i.header ], [ %j.next, %for.j.body ]
  %j.dec = add nsw i64 %j, -1
  %a.load.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @A, i64 0, i64 %i, i64 %j
  %b.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @B, i64 0, i64 %i, i64 %j
  %a = load float, ptr %a.load.index, align 4
  %b = load float, ptr %b.index, align 4
  %add = fadd float %a, %b
  %a.store.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @A, i64 0, i64 %i, i64 %j.dec
  store float %add, ptr %a.store.index, align 4
  %j.next = add nuw nsw i64 %j, 1
  %cmp.j = icmp eq i64 %j.next, 256
  br i1 %cmp.j, label %for.i.inc, label %for.j.body

for.i.inc:                                        ; preds = %for.j.body
  %i.next = add nuw nsw i64 %i, 1
  %cmp.i = icmp eq i64 %i.next, 256
  br i1 %cmp.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.inc
  ret void
}

define void @interchange_necessary_for_vectorization2() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.inc, %entry
  %i = phi i64 [ 1, %entry ], [ %i.next, %for.i.inc ]
  %i.inc = add nuw nsw i64 %i, 1
  br label %for.j.body

for.j.body:                                       ; preds = %for.j.body, %for.i.header
  %j = phi i64 [ 1, %for.i.header ], [ %j.next, %for.j.body ]
  %j.dec = add nsw i64 %j, -1
  %a.load.index = getelementptr inbounds [256 x [256 x float]], ptr @A, i64 0, i64 %i, i64 %j.dec
  %b.index = getelementptr inbounds [256 x [256 x float]], ptr @B, i64 0, i64 %i, i64 %j
  %c.load.index = getelementptr inbounds [256 x [256 x float]], ptr @C, i64 0, i64 %i.inc, i64 %j
  %c.store.index = getelementptr inbounds [256 x [256 x float]], ptr @C, i64 0, i64 %i, i64 %j
  %a = load float, ptr %a.load.index, align 4
  %b = load float, ptr %b.index, align 4
  %c0 = load float, ptr %c.load.index, align 4
  %c1 = load float, ptr %c.store.index, align 4
  %add.0 = fadd float %a, %b
  %a.store.index = getelementptr inbounds [256 x [256 x float]], ptr @A, i64 0, i64 %i, i64 %j
  store float %add.0, ptr %a.store.index, align 4
  %add.1 = fadd float %c0, %c1
  store float %add.1, ptr %c.store.index, align 4
  %j.next = add nuw nsw i64 %j, 1
  %cmp.j = icmp eq i64 %j.next, 256
  br i1 %cmp.j, label %for.i.inc, label %for.j.body

for.i.inc:                                        ; preds = %for.j.body
  %i.next = add nuw nsw i64 %i, 1
  %cmp.i = icmp eq i64 %i.next, 255
  br i1 %cmp.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.inc
  ret void
}

define void @multiple_BBs_in_loop() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.inc, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %for.i.inc ]
  %i.inc = add nuw nsw i64 %i, 1
  br label %for.j.header

for.j.header:                                     ; preds = %for.j.inc, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.inc, %for.j.inc ]
  %j.inc = add nuw nsw i64 %j, 1
  br label %for.k.body

for.k.body:                                       ; preds = %for.k.inc, %for.j.header
  %k = phi i64 [ 0, %for.j.header ], [ %k.inc, %for.k.inc ]
  %k.inc = add nuw nsw i64 %k, 1
  %k.2 = mul nuw nsw i64 %k, 2
  %d.index = getelementptr inbounds [256 x [256 x [256 x float]]], ptr @D, i64 0, i64 %i.inc, i64 %j.inc, i64 %k.2
  %e.index = getelementptr inbounds [256 x [256 x [256 x float]]], ptr @E, i64 0, i64 %i, i64 %j, i64 %k
  %d.load = load float, ptr %d.index, align 4
  store float %d.load, ptr %e.index, align 4
  %cond = freeze i1 undef
  br i1 %cond, label %if.then, label %for.k.inc

if.then:                                          ; preds = %for.k.body
  %d.index2 = getelementptr inbounds [256 x [256 x [256 x float]]], ptr @D, i64 0, i64 %i, i64 %j, i64 %k.inc
  store float 1.000000e+00, ptr %d.index2, align 4
  br label %for.k.inc

for.k.inc:                                        ; preds = %if.then, %for.k.body
  %cmp.k = icmp eq i64 %k.inc, 128
  br i1 %cmp.k, label %for.j.inc, label %for.k.body

for.j.inc:                                        ; preds = %for.k.inc
  %cmp.j = icmp eq i64 %j.inc, 255
  br i1 %cmp.j, label %for.i.inc, label %for.j.header

for.i.inc:                                        ; preds = %for.j.inc
  %cmp.i = icmp eq i64 %i.inc, 255
  br i1 %cmp.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.inc
  ret void
}
