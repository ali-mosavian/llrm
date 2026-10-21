; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = local_unnamed_addr global [1024 x [1024 x i32]] zeroinitializer, align 4

define void @f() local_unnamed_addr {
entry:
  br label %outer.header

outer.header:                                     ; preds = %inner.header, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %inner.header ]
  br label %inner.body

inner.header:                                     ; preds = %inner.body
  %i.next = add nuw nsw i64 %i, 1
  %exitcond20.not = icmp eq i64 %i.next, 1024
  br i1 %exitcond20.not, label %exit, label %outer.header

inner.body:                                       ; preds = %inner.body, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.body ]
  %arrayidx6 = getelementptr inbounds nuw [1024 x [1024 x i32]], ptr @A, i64 0, i64 %i, i64 %j
  store i32 42, ptr %arrayidx6, align 4
  %j.next = add nuw nsw i64 %j, 1
  %exitcond.not = icmp eq i64 %j.next, 1024
  br i1 %exitcond.not, label %inner.header, label %inner.body

exit:                                             ; preds = %inner.header
  ret void
}
