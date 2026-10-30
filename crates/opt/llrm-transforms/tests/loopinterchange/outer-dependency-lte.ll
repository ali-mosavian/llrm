; ModuleID = '<stdin>'
source_filename = "<stdin>"

@a = global [16 x [16 x [16 x i32]]] zeroinitializer, align 4

define void @f() {
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.cond.cleanup3, %entry
  %i.039 = phi i32 [ 0, %entry ], [ %inc26, %for.cond.cleanup3 ]
  %sub = add nuw nsw i32 %i.039, 3
  %idxprom = zext nneg i32 %sub to i64
  %mul = shl nuw nsw i32 %i.039, 1
  %idxprom13 = zext nneg i32 %mul to i64
  br label %for.cond5.preheader

for.cond.cleanup:                                 ; preds = %for.cond.cleanup3
  ret void

for.cond5.preheader:                              ; preds = %for.cond.cleanup7, %for.cond1.preheader
  %j.038 = phi i32 [ 1, %for.cond1.preheader ], [ %inc23, %for.cond.cleanup7 ]
  %idxprom11 = zext nneg i32 %j.038 to i64
  %sub18 = add nsw i32 %j.038, -1
  %idxprom19 = sext i32 %sub18 to i64
  br label %for.body8

for.cond.cleanup3:                                ; preds = %for.cond.cleanup7
  %inc26 = add nuw nsw i32 %i.039, 1
  %cmp = icmp samesign ult i32 %i.039, 3
  br i1 %cmp, label %for.cond1.preheader, label %for.cond.cleanup

for.cond.cleanup7:                                ; preds = %for.body8
  %inc23 = add nuw nsw i32 %j.038, 1
  %cmp2 = icmp samesign ult i32 %j.038, 7
  br i1 %cmp2, label %for.cond5.preheader, label %for.cond.cleanup3

for.body8:                                        ; preds = %for.body8, %for.cond5.preheader
  %k.037 = phi i32 [ 1, %for.cond5.preheader ], [ %add15, %for.body8 ]
  %idxprom9 = zext nneg i32 %k.037 to i64
  %arrayidx12 = getelementptr inbounds nuw [16 x [16 x [16 x i32]]], ptr @a, i64 0, i64 %idxprom, i64 %idxprom9, i64 %idxprom11
  %0 = load i32, ptr %arrayidx12, align 4
  %add15 = add nuw nsw i32 %k.037, 1
  %idxprom16 = zext nneg i32 %add15 to i64
  %arrayidx20 = getelementptr inbounds [16 x [16 x [16 x i32]]], ptr @a, i64 0, i64 %idxprom13, i64 %idxprom16, i64 %idxprom19
  %1 = load i32, ptr %arrayidx20, align 4
  %sub21 = sub nsw i32 %1, %0
  store i32 %sub21, ptr %arrayidx20, align 4
  %cmp6 = icmp samesign ult i32 %k.037, 7
  br i1 %cmp6, label %for.body8, label %for.cond.cleanup7
}
