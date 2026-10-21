; ModuleID = '<stdin>'
source_filename = "<stdin>"

@b = common local_unnamed_addr global [200 x [200 x i32]] zeroinitializer, align 4
@a = common local_unnamed_addr global i32 0, align 4

define void @test1() {
entry:
  %a = load i32, ptr @a, align 4
  br label %for.body

for.body:                                         ; preds = %for.inc7, %entry
  %indvars.outer = phi i64 [ 0, %entry ], [ %indvars.outer.next, %for.inc7 ]
  %or.reduction.outer = phi i32 [ %a, %entry ], [ %or.lcssa, %for.inc7 ]
  %index = add nsw i64 %indvars.outer, 9
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %for.body
  %or.reduction.inner = phi i32 [ %or.reduction.outer, %for.body ], [ %or, %for.body3 ]
  %indvar0 = phi i64 [ 5, %for.body ], [ %indvar0.next, %for.body3 ]
  %indvar1 = phi i32 [ 5, %for.body ], [ %indvar1.next, %for.body3 ]
  %arrayidx5 = getelementptr inbounds [200 x [200 x i32]], ptr @b, i64 0, i64 %indvar0, i64 %index
  %load.val = load i32, ptr %arrayidx5, align 4
  %or = or i32 %or.reduction.inner, %load.val
  %indvar0.next = add nsw i64 %indvar0, -1
  %indvar1.next = add nsw i32 %indvar1, -1
  %tobool2 = icmp eq i32 %indvar1.next, 0
  br i1 %tobool2, label %for.inc7, label %for.body3

for.inc7:                                         ; preds = %for.body3
  %or.lcssa = phi i32 [ %or, %for.body3 ]
  %indvars.outer.next = add nsw i64 %indvars.outer, 1
  %indvars.outer.next.trunc = trunc i64 %indvars.outer.next to i32
  %tobool = icmp eq i32 %indvars.outer.next.trunc, 100
  br i1 %tobool, label %for.cond.for.end8_crit_edge, label %for.body

for.cond.for.end8_crit_edge:                      ; preds = %for.inc7
  %or.lcssa.lcssa = phi i32 [ %or.lcssa, %for.inc7 ]
  store i32 %or.lcssa.lcssa, ptr @a, align 4
  br label %for.end8

for.end8:                                         ; preds = %for.cond.for.end8_crit_edge
  ret void
}

define void @test2() {
entry:
  %a = load i32, ptr @a, align 4
  br label %for.body

for.body:                                         ; preds = %for.inc7, %entry
  %indvars.outer = phi i64 [ 0, %entry ], [ %indvars.outer.next, %for.inc7 ]
  %or.reduction.outer = phi i32 [ %a, %entry ], [ %or.lcssa, %for.inc7 ]
  %index = add nsw i64 %indvars.outer, 9
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %for.body
  %or.reduction.inner = phi i32 [ %or.reduction.outer, %for.body ], [ %or, %for.body3 ]
  %indvar0 = phi i64 [ 5, %for.body ], [ %indvar0.next, %for.body3 ]
  %indvar1 = phi i32 [ 6, %for.body ], [ %indvar1.next, %for.body3 ]
  %arrayidx5 = getelementptr inbounds [200 x [200 x i32]], ptr @b, i64 0, i64 %indvar0, i64 %index
  %load.val = load i32, ptr %arrayidx5, align 4
  %or = or i32 %or.reduction.inner, %load.val
  %indvar0.next = add nsw i64 %indvar0, -1
  %indvar1.next = add nsw i32 %indvar1, -2
  %indvar1.next.ext = sext i32 %indvar1.next to i64
  %indvars.add = add nsw i64 %indvar0.next, %indvar1.next.ext
  %tobool2 = icmp sle i64 %indvars.add, 0
  br i1 %tobool2, label %for.inc7, label %for.body3

for.inc7:                                         ; preds = %for.body3
  %or.lcssa = phi i32 [ %or, %for.body3 ]
  %indvars.outer.next = add nsw i64 %indvars.outer, 1
  %indvars.outer.next.trunc = trunc i64 %indvars.outer.next to i32
  %tobool = icmp eq i32 %indvars.outer.next.trunc, 100
  br i1 %tobool, label %for.cond.for.end8_crit_edge, label %for.body

for.cond.for.end8_crit_edge:                      ; preds = %for.inc7
  %or.lcssa.lcssa = phi i32 [ %or.lcssa, %for.inc7 ]
  store i32 %or.lcssa.lcssa, ptr @a, align 4
  br label %for.end8

for.end8:                                         ; preds = %for.cond.for.end8_crit_edge
  ret void
}

define void @test3() {
entry:
  %a = load i32, ptr @a, align 4
  br label %for.body

for.body:                                         ; preds = %for.inc7, %entry
  %indvars.outer = phi i64 [ 0, %entry ], [ %indvars.outer.next, %for.inc7 ]
  %or.reduction.outer = phi i32 [ %a, %entry ], [ %or.lcssa, %for.inc7 ]
  %index = add nsw i64 %indvars.outer, 9
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %for.body
  %or.reduction.inner = phi i32 [ %or.reduction.outer, %for.body ], [ %or, %for.body3 ]
  %indvar0 = phi i32 [ 5, %for.body ], [ %indvar0.next, %for.body3 ]
  %indvar1 = phi i32 [ 49, %for.body ], [ %indvar1.next, %for.body3 ]
  %arrayidx5 = getelementptr inbounds [200 x [200 x i32]], ptr @b, i64 0, i32 %indvar0, i64 %index
  %load.val = load i32, ptr %arrayidx5, align 4
  %or = or i32 %or.reduction.inner, %load.val
  %indvar0.next = add nsw i32 %indvar0, 1
  %indvar1.next = add nsw i32 %indvar1, -1
  %tobool2 = icmp eq i32 %indvar0.next, %indvar1.next
  br i1 %tobool2, label %for.inc7, label %for.body3

for.inc7:                                         ; preds = %for.body3
  %or.lcssa = phi i32 [ %or, %for.body3 ]
  %indvars.outer.next = add nsw i64 %indvars.outer, 1
  %tobool = icmp eq i64 %indvars.outer.next, 100
  br i1 %tobool, label %for.cond.for.end8_crit_edge, label %for.body

for.cond.for.end8_crit_edge:                      ; preds = %for.inc7
  %or.lcssa.lcssa = phi i32 [ %or.lcssa, %for.inc7 ]
  store i32 %or.lcssa.lcssa, ptr @a, align 4
  br label %for.end8

for.end8:                                         ; preds = %for.cond.for.end8_crit_edge
  ret void
}
