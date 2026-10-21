; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = global [256 x [256 x float]] zeroinitializer
@B = global [256 x [256 x float]] zeroinitializer
@C = global [256 x [256 x float]] zeroinitializer
@D = global [256 x [256 x float]] zeroinitializer
@E = global [256 x [256 x float]] zeroinitializer
@F = global [256 x [256 x float]] zeroinitializer

define void @f() {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.inc, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.inc ]
  br label %for.j.body

for.j.body:                                       ; preds = %for.j.body, %for.i.header
  %j = phi i64 [ 1, %for.i.header ], [ %j.next, %for.j.body ]
  %j.dec = add nsw i64 %j, -1
  %a.0.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @A, i64 0, i64 %j.dec, i64 %i
  %b.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @B, i64 0, i64 %j, i64 %i
  %c.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @C, i64 0, i64 %i, i64 %j
  %d.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @D, i64 0, i64 %i, i64 %j
  %e.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @E, i64 0, i64 %i, i64 %j
  %f.index = getelementptr inbounds nuw [256 x [256 x float]], ptr @F, i64 0, i64 %i, i64 %j
  %a.0 = load float, ptr %a.0.index, align 4
  %b = load float, ptr %b.index, align 4
  %c = load float, ptr %c.index, align 4
  %d = load float, ptr %d.index, align 4
  %e = load float, ptr %e.index, align 4
  %f = load float, ptr %f.index, align 4
  %add.0 = fadd float %a.0, %b
  %add.1 = fadd float %add.0, %c
  %add.2 = fadd float %add.1, %d
  %add.3 = fadd float %add.2, %e
  %add.4 = fadd float %add.3, %f
  %a.1.index = getelementptr inbounds nuw [256 x float], ptr @A, i64 %j, i64 %i
  store float %add.4, ptr %a.1.index, align 4
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
