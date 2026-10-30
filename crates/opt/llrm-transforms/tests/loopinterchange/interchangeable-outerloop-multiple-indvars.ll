; ModuleID = '<stdin>'
source_filename = "<stdin>"

@b = constant [200 x [100 x i32]] zeroinitializer, align 4
@a = constant i32 0, align 4

define void @test1() {
entry:
  %a = load i32, ptr @a, align 4
  br label %for.cond2.preheader

for.cond2.preheader:                              ; preds = %for.inc7, %entry
  %indvar0 = phi i64 [ 0, %entry ], [ %indvar0.next, %for.inc7 ]
  %or.reduction = phi i32 [ %a, %entry ], [ %or.lcssa, %for.inc7 ]
  %indvar1 = phi i32 [ 1, %entry ], [ %indvar1.next, %for.inc7 ]
  %index = add nsw i64 %indvar0, 9
  br label %for.body4

for.body4:                                        ; preds = %for.body4, %for.cond2.preheader
  %indvars.iv = phi i64 [ 5, %for.cond2.preheader ], [ %indvars.iv.next, %for.body4 ]
  %or13 = phi i32 [ %or.reduction, %for.cond2.preheader ], [ %or, %for.body4 ]
  %arrayidx6 = getelementptr inbounds [200 x [100 x i32]], ptr @b, i64 0, i64 %indvars.iv, i64 %index
  %load.val = load i32, ptr %arrayidx6, align 4
  %or = or i32 %or13, %load.val
  %indvars.iv.next = add nsw i64 %indvars.iv, -1
  %tobool3 = icmp eq i64 %indvars.iv.next, 0
  br i1 %tobool3, label %for.inc7, label %for.body4

for.inc7:                                         ; preds = %for.body4
  %or.lcssa = phi i32 [ %or, %for.body4 ]
  %indvar0.next = add nsw i64 %indvar0, 1
  %indvar1.next = add nsw i32 %indvar1, 1
  %indvar0.next.trunc = trunc i64 %indvar0.next to i32
  %tobool = icmp ne i32 %indvar0.next.trunc, 100
  %tobool1 = icmp ne i32 %indvar1.next, 150
  %outer.cond = and i1 %tobool, %tobool1
  br i1 %outer.cond, label %for.cond2.preheader, label %for.cond.for.end9_crit_edge

for.cond.for.end9_crit_edge:                      ; preds = %for.inc7
  %or.lcssa.lcssa = phi i32 [ %or.lcssa, %for.inc7 ]
  store i32 %or.lcssa.lcssa, ptr @a, align 4
  br label %for.end9

for.end9:                                         ; preds = %for.cond.for.end9_crit_edge
  ret void
}

define void @test2() {
entry:
  %a = load i32, ptr @a, align 4
  br label %for.cond2.preheader

for.cond2.preheader:                              ; preds = %for.inc7, %entry
  %indvar0 = phi i64 [ 0, %entry ], [ %indvar0.next, %for.inc7 ]
  %or.reduction = phi i32 [ %a, %entry ], [ %or.lcssa, %for.inc7 ]
  %indvar1 = phi i64 [ 1, %entry ], [ %indvar1.next, %for.inc7 ]
  %index0 = add nsw i64 %indvar0, 9
  br label %for.body4

for.body4:                                        ; preds = %for.body4, %for.cond2.preheader
  %indvars.iv = phi i64 [ 5, %for.cond2.preheader ], [ %indvars.iv.next, %for.body4 ]
  %or13 = phi i32 [ %or.reduction, %for.cond2.preheader ], [ %or, %for.body4 ]
  %index1 = add nsw i64 %indvars.iv, %indvar1
  %arrayidx6 = getelementptr inbounds [200 x [100 x i32]], ptr @b, i64 0, i64 %index1, i64 %index0
  %load.val = load i32, ptr %arrayidx6, align 4
  %or = or i32 %or13, %load.val
  %indvars.iv.next = add nsw i64 %indvars.iv, -1
  %tobool3 = icmp eq i64 %indvars.iv.next, 0
  br i1 %tobool3, label %for.inc7, label %for.body4

for.inc7:                                         ; preds = %for.body4
  %or.lcssa = phi i32 [ %or, %for.body4 ]
  %indvar0.next = add nsw i64 %indvar0, 1
  %indvar1.next = add nsw i64 %indvar1, 1
  %tobool = icmp ne i64 %indvar0.next, 100
  %tobool1 = icmp ne i64 %indvar1.next, 150
  %outer.cond = and i1 %tobool, %tobool1
  br i1 %outer.cond, label %for.cond2.preheader, label %for.cond.for.end9_crit_edge

for.cond.for.end9_crit_edge:                      ; preds = %for.inc7
  %or.lcssa.lcssa = phi i32 [ %or.lcssa, %for.inc7 ]
  store i32 %or.lcssa.lcssa, ptr @a, align 4
  br label %for.end9

for.end9:                                         ; preds = %for.cond.for.end9_crit_edge
  ret void
}

define void @test3() {
entry:
  %a = load i32, ptr @a, align 4
  br label %for.cond2.preheader

for.cond2.preheader:                              ; preds = %for.inc7, %entry
  %indvar0 = phi i64 [ 0, %entry ], [ %indvar0.next, %for.inc7 ]
  %or.reduction = phi i32 [ %a, %entry ], [ %or.lcssa, %for.inc7 ]
  %indvar1 = phi i64 [ 1, %entry ], [ %indvar1.next, %for.inc7 ]
  %index0 = add nsw i64 %indvar0, 9
  br label %for.body4

for.body4:                                        ; preds = %for.body4, %for.cond2.preheader
  %indvars.iv = phi i64 [ 5, %for.cond2.preheader ], [ %indvars.iv.next, %for.body4 ]
  %or13 = phi i32 [ %or.reduction, %for.cond2.preheader ], [ %or, %for.body4 ]
  %index1 = add nsw i64 %indvars.iv, %indvar1
  %arrayidx6 = getelementptr inbounds [200 x [100 x i32]], ptr @b, i64 0, i64 %index1, i64 %index0
  %load.val = load i32, ptr %arrayidx6, align 4
  %or = or i32 %or13, %load.val
  %indvars.iv.next = add nsw i64 %indvars.iv, -1
  %tobool3 = icmp eq i64 %indvars.iv.next, 0
  br i1 %tobool3, label %for.inc7, label %for.body4

for.inc7:                                         ; preds = %for.body4
  %or.lcssa = phi i32 [ %or, %for.body4 ]
  %indvar0.next = add nsw i64 %indvar0, 1
  %indvar1.next = add nsw i64 %indvar1, 1
  %outer.indvar.add = add nsw i64 %indvar0.next, %indvar1.next
  %outer.cond = icmp slt i64 %outer.indvar.add, 150
  br i1 %outer.cond, label %for.cond2.preheader, label %for.cond.for.end9_crit_edge

for.cond.for.end9_crit_edge:                      ; preds = %for.inc7
  %or.lcssa.lcssa = phi i32 [ %or.lcssa, %for.inc7 ]
  store i32 %or.lcssa.lcssa, ptr @a, align 4
  br label %for.end9

for.end9:                                         ; preds = %for.cond.for.end9_crit_edge
  ret void
}
