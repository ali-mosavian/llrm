; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = common global [500 x [500 x i32]] zeroinitializer

define i32 @reduction_01(i32 %N) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.inc, %entry
  %indvars.iv18 = phi i64 [ %indvars.iv.next19, %outer.inc ], [ 1, %entry ]
  %add15 = phi i32 [ 0, %entry ], [ %add, %outer.inc ]
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %outer.header
  %indvars.iv = phi i64 [ 1, %outer.header ], [ %indvars.iv.next, %for.body3 ]
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %lftr.wideiv = trunc i64 %indvars.iv.next to i32
  %exitcond = icmp eq i32 %lftr.wideiv, %N
  br i1 %exitcond, label %outer.inc, label %for.body3

outer.inc:                                        ; preds = %for.body3
  %indvars.iv.lcssa = phi i64 [ %indvars.iv, %for.body3 ]
  %arrayidx5 = getelementptr inbounds [500 x [500 x i32]], ptr @A, i64 0, i64 %indvars.iv.lcssa, i64 %indvars.iv18
  %0 = load i32, ptr %arrayidx5, align 4
  %add = add nsw i32 %add15, %0
  %indvars.iv.next19 = add nuw nsw i64 %indvars.iv18, 1
  %lftr.wideiv20 = trunc i64 %indvars.iv.next19 to i32
  %exitcond21 = icmp eq i32 %lftr.wideiv20, %N
  br i1 %exitcond21, label %for.end8, label %outer.header

for.end8:                                         ; preds = %outer.inc
  %add.lcssa = phi i32 [ %add, %outer.inc ]
  ret i32 %add.lcssa
}
