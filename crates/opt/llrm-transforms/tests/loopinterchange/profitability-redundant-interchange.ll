; ModuleID = '<stdin>'
source_filename = "<stdin>"

@a = local_unnamed_addr global [16 x [16 x [16 x i32]]] zeroinitializer, align 4

define void @f() {
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.cond.cleanup3, %entry
  %indvars.iv46 = phi i64 [ 0, %entry ], [ %indvars.iv.next47, %for.cond.cleanup3 ]
  %0 = add nuw nsw i64 %indvars.iv46, 3
  br label %for.cond5.preheader

for.cond5.preheader:                              ; preds = %for.cond.cleanup7, %for.cond1.preheader
  %indvars.iv41 = phi i64 [ 1, %for.cond1.preheader ], [ %indvars.iv.next42, %for.cond.cleanup7 ]
  %1 = add nsw i64 %indvars.iv41, -1
  br label %for.body8

for.cond.cleanup3:                                ; preds = %for.cond.cleanup7
  %indvars.iv.next47 = add nuw nsw i64 %indvars.iv46, 1
  %exitcond50 = icmp ne i64 %indvars.iv.next47, 4
  br i1 %exitcond50, label %for.cond1.preheader, label %for.cond.cleanup

for.cond.cleanup7:                                ; preds = %for.body8
  %indvars.iv.next42 = add nuw nsw i64 %indvars.iv41, 1
  %exitcond45 = icmp ne i64 %indvars.iv.next42, 8
  br i1 %exitcond45, label %for.cond5.preheader, label %for.cond.cleanup3

for.body8:                                        ; preds = %for.body8, %for.cond5.preheader
  %indvars.iv = phi i64 [ 1, %for.cond5.preheader ], [ %indvars.iv.next, %for.body8 ]
  %arrayidx12 = getelementptr inbounds nuw [16 x [16 x [16 x i32]]], ptr @a, i64 0, i64 %0, i64 %indvars.iv, i64 %indvars.iv41
  %2 = load i32, ptr %arrayidx12, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %arrayidx20 = getelementptr inbounds [16 x [16 x [16 x i32]]], ptr @a, i64 0, i64 %indvars.iv46, i64 %indvars.iv.next, i64 %1
  %3 = load i32, ptr %arrayidx20, align 4
  %sub21 = sub nsw i32 %3, %2
  store i32 %sub21, ptr %arrayidx20, align 4
  %exitcond = icmp ne i64 %indvars.iv.next, 8
  br i1 %exitcond, label %for.body8, label %for.cond.cleanup7

for.cond.cleanup:                                 ; preds = %for.cond.cleanup3
  ret void
}
