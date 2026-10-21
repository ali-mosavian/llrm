; ModuleID = '<stdin>'
source_filename = "<stdin>"

define i64 @reduction_seed_in_guarded_preheader(ptr noalias readonly %Arr, ptr noalias readonly %guard) {
entry:
  br label %for.body

for.cond.cleanup:                                 ; preds = %for.inc15
  %total.1.lcssa = phi i64 [ %total.1, %for.inc15 ]
  ret i64 %total.1.lcssa

for.body:                                         ; preds = %for.inc15, %entry
  %k.032 = phi i64 [ 0, %entry ], [ %inc16, %for.inc15 ]
  %total.031 = phi i64 [ 0, %entry ], [ %total.1, %for.inc15 ]
  %arrayidx = getelementptr inbounds nuw [8 x i8], ptr %guard, i64 %k.032
  %0 = load i64, ptr %arrayidx, align 8
  %tobool.not = icmp eq i64 %0, 0
  br i1 %tobool.not, label %for.inc15, label %if.then

if.then:                                          ; preds = %for.body
  %mul = mul nuw nsw i64 %k.032, 7
  br label %for.cond5.preheader

for.cond5.preheader:                              ; preds = %for.cond.cleanup7, %if.then
  %j.030 = phi i64 [ 0, %if.then ], [ %inc12, %for.cond.cleanup7 ]
  %acc.029 = phi i64 [ %mul, %if.then ], [ %add.lcssa, %for.cond.cleanup7 ]
  %invariant.gep = getelementptr inbounds nuw [8 x i8], ptr %Arr, i64 %j.030
  br label %for.body8

for.cond.cleanup3:                                ; preds = %for.cond.cleanup7
  %add.lcssa.lcssa = phi i64 [ %add.lcssa, %for.cond.cleanup7 ]
  %add14 = add nsw i64 %add.lcssa.lcssa, %total.031
  br label %for.inc15

for.cond.cleanup7:                                ; preds = %for.body8
  %add.lcssa = phi i64 [ %add, %for.body8 ]
  %inc12 = add nuw nsw i64 %j.030, 1
  %exitcond33.not = icmp eq i64 %inc12, 100
  br i1 %exitcond33.not, label %for.cond.cleanup3, label %for.cond5.preheader

for.body8:                                        ; preds = %for.body8, %for.cond5.preheader
  %i.028 = phi i64 [ 0, %for.cond5.preheader ], [ %inc, %for.body8 ]
  %acc.127 = phi i64 [ %acc.029, %for.cond5.preheader ], [ %add, %for.body8 ]
  %gep = getelementptr inbounds nuw [800 x i8], ptr %invariant.gep, i64 %i.028
  %1 = load i64, ptr %gep, align 8
  %add = add nsw i64 %1, %acc.127
  %inc = add nuw nsw i64 %i.028, 1
  %exitcond.not = icmp eq i64 %inc, 100
  br i1 %exitcond.not, label %for.cond.cleanup7, label %for.body8

for.inc15:                                        ; preds = %for.cond.cleanup3, %for.body
  %total.1 = phi i64 [ %add14, %for.cond.cleanup3 ], [ %total.031, %for.body ]
  %inc16 = add nuw nsw i64 %k.032, 1
  %exitcond34.not = icmp eq i64 %inc16, 100
  br i1 %exitcond34.not, label %for.cond.cleanup, label %for.body
}
