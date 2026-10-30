; ModuleID = '<stdin>'
source_filename = "<stdin>"

@f = local_unnamed_addr global [4 x [9 x i32]] [[9 x i32] [i32 5, i32 3, i32 0, i32 0, i32 0, i32 0, i32 0, i32 0, i32 0], [9 x i32] zeroinitializer, [9 x i32] zeroinitializer, [9 x i32] zeroinitializer], align 4
@g = common local_unnamed_addr global i32 0, align 4

define i32 @test1(i1 %cond) {
for.preheader:
  br label %outerloop

outerloop:                                        ; preds = %for.latch, %for.preheader
  %i = phi i64 [ 0, %for.preheader ], [ %indvars.iv.next21.i, %for.latch ]
  br label %innerloop

innerloop:                                        ; preds = %if.end, %outerloop
  %j = phi i64 [ 0, %outerloop ], [ %j.next, %if.end ]
  %arrayidx6.i = getelementptr inbounds [4 x [9 x i32]], ptr @f, i64 0, i64 %j, i64 %i
  %i1 = load i32, ptr %arrayidx6.i, align 4
  %tobool.i = icmp eq i32 %i1, 0
  br i1 %tobool.i, label %land.end, label %land.rhs

land.rhs:                                         ; preds = %innerloop
  store i32 3, ptr @g, align 4
  br label %land.end

land.end:                                         ; preds = %land.rhs, %innerloop
  br i1 %cond, label %if.end, label %if.then

if.then:                                          ; preds = %land.end
  %i2 = load i32, ptr @g, align 4
  %inc.i = add i32 %i2, 1
  store i32 %inc.i, ptr @g, align 4
  br label %if.end

if.end:                                           ; preds = %if.then, %land.end
  %j.next = add nuw nsw i64 %j, 1
  %exitcond.i = icmp eq i64 %j.next, 3
  br i1 %exitcond.i, label %for.latch, label %innerloop

for.latch:                                        ; preds = %if.end
  %indvars.iv.next21.i = add nsw i64 %i, 1
  %cmp.i = icmp slt i64 %i, 2
  br i1 %cmp.i, label %outerloop, label %exit

exit:                                             ; preds = %for.latch
  %i3 = load i32, ptr @g, align 4
  ret i32 %i3
}

define i32 @test2(i1 %cond) {
for.preheader:
  br label %outerloop

outerloop:                                        ; preds = %for.latch, %for.preheader
  %i = phi i64 [ 0, %for.preheader ], [ %indvars.iv.next21.i, %for.latch ]
  br label %innerloop

innerloop:                                        ; preds = %if.end, %outerloop
  %j = phi i64 [ 0, %outerloop ], [ %j.next, %if.end ]
  %arrayidx6.i = getelementptr inbounds [4 x [9 x i32]], ptr @f, i64 0, i64 %j, i64 %i
  %i1 = load i32, ptr %arrayidx6.i, align 4
  %tobool.i = icmp eq i32 %i1, 0
  store i32 3, ptr @g, align 4
  br i1 %tobool.i, label %land.end, label %land.rhs

land.rhs:                                         ; preds = %innerloop
  br label %land.end

land.end:                                         ; preds = %land.rhs, %innerloop
  br i1 %cond, label %if.end, label %if.then

if.then:                                          ; preds = %land.end
  %i2 = load i32, ptr @g, align 4
  %inc.i = add i32 %i2, 1
  store i32 %inc.i, ptr @g, align 4
  br label %if.end

if.end:                                           ; preds = %if.then, %land.end
  %j.next = add nuw nsw i64 %j, 1
  %exitcond.i = icmp eq i64 %j.next, 3
  br i1 %exitcond.i, label %for.latch, label %innerloop

for.latch:                                        ; preds = %if.end
  %indvars.iv.next21.i = add nsw i64 %i, 1
  %cmp.i = icmp slt i64 %i, 2
  br i1 %cmp.i, label %outerloop, label %exit

exit:                                             ; preds = %for.latch
  %i3 = load i32, ptr @g, align 4
  ret i32 %i3
}
