; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64"

@f = global i32 0, align 4
@d6 = global i16 5, align 2
@f6 = global ptr @d6, align 4

define void @disabled1(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp127 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp127, %cmp
  br i1 %or.cond, label %for.preheader, label %return

for.preheader:                                    ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.preheader
  %i.029 = phi i32 [ %add10, %for.latch ], [ 0, %for.preheader ]
  %b.028 = phi i32 [ %inc8, %for.latch ], [ 1, %for.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i.029
  %0 = load i32, ptr %arrayidx, align 4
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.026 = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1.025 = phi i32 [ %0, %for.outer ], [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %B, i32 %j.026
  %1 = load i32, ptr %arrayidx6, align 4
  %add = add i32 %1, %sum1.025
  %inc = add nuw i32 %j.026, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx7 = getelementptr inbounds i32, ptr %A, i32 %b.028
  store i32 %add.lcssa, ptr %arrayidx7, align 4
  %inc8 = add nuw nsw i32 %b.028, 1
  %add10 = add nuw nsw i32 %i.029, 1
  %exitcond30 = icmp eq i32 %add10, %I
  br i1 %exitcond30, label %return.loopexit, label %for.outer

return.loopexit:                                  ; preds = %for.latch
  br label %return

return:                                           ; preds = %return.loopexit, %entry
  ret void
}

define void @disabled2(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp131 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp131, %cmp
  br i1 %or.cond, label %for.preheader, label %for.end14

for.preheader:                                    ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.preheader
  %i.032 = phi i32 [ %add13, %for.latch ], [ 0, %for.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %i.032
  %0 = load i32, ptr %arrayidx, align 4
  %tobool = icmp eq i32 %0, 0
  br i1 %tobool, label %for.latch, label %for.inner.preheader

for.inner.preheader:                              ; preds = %for.outer
  br label %for.inner

for.inner:                                        ; preds = %for.inner.preheader, %for.inner
  %j.030 = phi i32 [ %inc, %for.inner ], [ 0, %for.inner.preheader ]
  %sum1.029 = phi i32 [ %sum1.1, %for.inner ], [ 0, %for.inner.preheader ]
  %arrayidx6 = getelementptr inbounds i32, ptr %B, i32 %j.030
  %1 = load i32, ptr %arrayidx6, align 4
  %tobool7 = icmp eq i32 %1, 0
  %sub = add i32 %sum1.029, 10
  %add = sub i32 %sub, %1
  %sum1.1 = select i1 %tobool7, i32 %sum1.029, i32 %add
  %inc = add nuw i32 %j.030, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch.loopexit, label %for.inner

for.latch.loopexit:                               ; preds = %for.inner
  %sum1.1.lcssa1 = phi i32 [ %sum1.1, %for.inner ]
  br label %for.latch

for.latch:                                        ; preds = %for.latch.loopexit, %for.outer
  %sum1.1.lcssa = phi i32 [ 0, %for.outer ], [ %sum1.1.lcssa1, %for.latch.loopexit ]
  %arrayidx11 = getelementptr inbounds i32, ptr %A, i32 %i.032
  store i32 %sum1.1.lcssa, ptr %arrayidx11, align 4
  %add13 = add nuw i32 %i.032, 1
  %exitcond33 = icmp eq i32 %add13, %I
  br i1 %exitcond33, label %for.end14.loopexit, label %for.outer

for.end14.loopexit:                               ; preds = %for.latch
  br label %for.end14

for.end14:                                        ; preds = %for.end14.loopexit, %entry
  ret void
}

define void @disabled3(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %S = alloca [4 x i32], align 4
  %cmp = icmp eq i32 %J, 0
  br i1 %cmp, label %return, label %if.end

if.end:                                           ; preds = %entry
  %cmp128 = icmp eq i32 %I, 0
  br i1 %cmp128, label %for.cond.cleanup, label %for.preheader

for.preheader:                                    ; preds = %if.end
  br label %for.outer

for.cond.cleanup.loopexit:                        ; preds = %for.latch
  br label %for.cond.cleanup

for.cond.cleanup:                                 ; preds = %for.cond.cleanup.loopexit, %if.end
  br label %return

for.outer:                                        ; preds = %for.latch, %for.preheader
  %i.029 = phi i32 [ 0, %for.preheader ], [ %add12, %for.latch ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.027 = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j.027
  %l2 = load i32, ptr %arrayidx, align 4
  %add = add i32 %j.027, %i.029
  %rem = urem i32 %add, %J
  %arrayidx6 = getelementptr inbounds i32, ptr %B, i32 %rem
  %l3 = load i32, ptr %arrayidx6, align 4
  %mul = mul i32 %l3, %l2
  %rem7 = urem i32 %j.027, 3
  %arrayidx8 = getelementptr inbounds [4 x i32], ptr %S, i32 0, i32 %rem7
  store i32 %mul, ptr %arrayidx8, align 4
  %inc = add nuw i32 %j.027, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %l1 = load i32, ptr %S, align 4
  %arrayidx10 = getelementptr inbounds i32, ptr %A, i32 %i.029
  store i32 %l1, ptr %arrayidx10, align 4
  %add12 = add nuw i32 %i.029, 1
  %exitcond31 = icmp eq i32 %add12, %I
  br i1 %exitcond31, label %for.cond.cleanup.loopexit, label %for.outer

return:                                           ; preds = %for.cond.cleanup, %entry
  ret void
}

define void @disabled4(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ugt i32 %I, 1
  %or.cond = and i1 %cmp122, %cmp
  br i1 %or.cond, label %for.preheader, label %for.end9

for.preheader:                                    ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.preheader
  %indvars.iv = phi i32 [ %indvars.iv.next, %for.latch ], [ 1, %for.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.021 = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1.020 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j.021
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1.020
  %inc = add nuw i32 %j.021, 1
  %exitcond = icmp eq i32 %inc, %indvars.iv
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %indvars.iv
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %indvars.iv.next = add nuw i32 %indvars.iv, 1
  %exitcond24 = icmp eq i32 %indvars.iv.next, %I
  br i1 %exitcond24, label %for.end9.loopexit, label %for.outer

for.end9.loopexit:                                ; preds = %for.latch
  br label %for.end9

for.end9:                                         ; preds = %for.end9.loopexit, %entry
  ret void
}

define i32 @disabled5() {
entry:
  %f.promoted10 = load i32, ptr @f, align 4
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %0 = phi i32 [ %f.promoted10, %entry ], [ 2, %for.latch ]
  %d.018 = phi i16 [ 0, %entry ], [ %odd.lcssa, %for.latch ]
  %inc5.sink9 = phi i32 [ 2, %entry ], [ %inc5, %for.latch ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %1 = phi i32 [ %0, %for.outer ], [ 2, %for.inner ]
  %inc.sink8 = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %inc = add nuw nsw i32 %inc.sink8, 1
  %exitcond = icmp ne i32 %inc, 7
  br i1 %exitcond, label %for.inner, label %for.latch

for.latch:                                        ; preds = %for.inner
  %.lcssa = phi i32 [ %1, %for.inner ]
  %odd.lcssa = phi i16 [ 1, %for.inner ]
  %inc5 = add nuw nsw i32 %inc5.sink9, 1
  %exitcond11 = icmp ne i32 %inc5, 7
  br i1 %exitcond11, label %for.outer, label %for.end

for.end:                                          ; preds = %for.latch
  %.lcssa.lcssa = phi i32 [ %.lcssa, %for.latch ]
  %inc.lcssa.lcssa = phi i32 [ 7, %for.latch ]
  ret i32 0
}

define i32 @disabled6() {
entry:
  store i16 1, ptr @d6, align 2
  %0 = load ptr, ptr @f6, align 4
  br label %for.body.i

for.body.i:                                       ; preds = %for.cond.cleanup.i, %entry
  %inc8.sink14.i = phi i16 [ 1, %entry ], [ %inc8.i, %for.cond.cleanup.i ]
  %1 = load i16, ptr %0, align 2
  br label %for.body6.i

for.cond.cleanup.i:                               ; preds = %for.body6.i
  %inc8.i = add nuw nsw i16 %inc8.sink14.i, 1
  store i16 %inc8.i, ptr @d6, align 2
  %cmp.i = icmp ult i16 %inc8.i, 6
  br i1 %cmp.i, label %for.body.i, label %test.exit

for.body6.i:                                      ; preds = %for.body6.i, %for.body.i
  %c.013.i = phi i32 [ 0, %for.body.i ], [ %inc.i, %for.body6.i ]
  %inc.i = add nuw nsw i32 %c.013.i, 1
  %exitcond.i = icmp eq i32 %inc.i, 7
  br i1 %exitcond.i, label %for.cond.cleanup.i, label %for.body6.i

test.exit:                                        ; preds = %for.cond.cleanup.i
  %.lcssa = phi i16 [ %1, %for.cond.cleanup.i ]
  %conv2.i = sext i16 %.lcssa to i32
  ret i32 0
}

define void @disabled7(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp127 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp127, %cmp
  br i1 %or.cond, label %for.body.preheader, label %for.end12

for.body.preheader:                               ; preds = %entry
  br label %for.body

for.body:                                         ; preds = %for.cond3.for.cond.cleanup5_crit_edge, %for.body.preheader
  %i.028 = phi i32 [ %add11, %for.cond3.for.cond.cleanup5_crit_edge ], [ 0, %for.body.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i.028
  store i32 0, ptr %arrayidx, align 4
  %sub = add i32 %i.028, -1
  %arrayidx2 = getelementptr inbounds i32, ptr %A, i32 %sub
  store i32 2, ptr %arrayidx2, align 4
  br label %for.body6

for.cond3.for.cond.cleanup5_crit_edge:            ; preds = %for.body6
  %add.lcssa = phi i32 [ %add, %for.body6 ]
  store i32 %add.lcssa, ptr %arrayidx, align 4
  %add11 = add nuw i32 %i.028, 1
  %exitcond29 = icmp eq i32 %add11, %I
  br i1 %exitcond29, label %for.end12.loopexit, label %for.body

for.body6:                                        ; preds = %for.body6, %for.body
  %0 = phi i32 [ 0, %for.body ], [ %add, %for.body6 ]
  %j.026 = phi i32 [ 0, %for.body ], [ %add9, %for.body6 ]
  %arrayidx7 = getelementptr inbounds i32, ptr %B, i32 %j.026
  %1 = load i32, ptr %arrayidx7, align 4
  %add = add i32 %1, %0
  %add9 = add nuw i32 %j.026, 1
  %exitcond = icmp eq i32 %add9, %J
  br i1 %exitcond, label %for.cond3.for.cond.cleanup5_crit_edge, label %for.body6

for.end12.loopexit:                               ; preds = %for.cond3.for.cond.cleanup5_crit_edge
  br label %for.end12

for.end12:                                        ; preds = %for.end12.loopexit, %entry
  ret void
}

define void @disabled8(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp eq i32 %J, 0
  %cmp335 = icmp eq i32 %I, 0
  %or.cond = or i1 %cmp, %cmp335
  br i1 %or.cond, label %for.end18, label %for.body.preheader

for.body.preheader:                               ; preds = %entry
  br label %for.body

for.body:                                         ; preds = %for.cond.cleanup4, %for.body.preheader
  %x.037 = phi i32 [ %inc, %for.cond.cleanup4 ], [ 0, %for.body.preheader ]
  br label %for.outer

for.cond.cleanup4:                                ; preds = %for.latch
  %inc = add nuw nsw i32 %x.037, 1
  %exitcond40 = icmp eq i32 %inc, 5
  br i1 %exitcond40, label %for.end18.loopexit, label %for.body

for.outer:                                        ; preds = %for.latch, %for.body
  %i.036 = phi i32 [ %add15, %for.latch ], [ 0, %for.body ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i.036
  store i32 0, ptr %arrayidx, align 4
  %sub = add i32 %i.036, -1
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %sub
  store i32 2, ptr %arrayidx6, align 4
  br label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  store i32 %add.lcssa, ptr %arrayidx, align 4
  %add15 = add nuw i32 %i.036, 1
  %exitcond38 = icmp eq i32 %add15, %I
  br i1 %exitcond38, label %for.cond.cleanup4, label %for.outer

for.inner:                                        ; preds = %for.inner, %for.outer
  %0 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %j.034 = phi i32 [ 0, %for.outer ], [ %add13, %for.inner ]
  %arrayidx11 = getelementptr inbounds i32, ptr %B, i32 %j.034
  %1 = load i32, ptr %arrayidx11, align 4
  %add = add i32 %1, %0
  %add13 = add nuw i32 %j.034, 1
  %exitcond = icmp eq i32 %add13, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.end18.loopexit:                               ; preds = %for.cond.cleanup4
  br label %for.end18

for.end18:                                        ; preds = %for.end18.loopexit, %entry
  ret void
}

define void @disabled9(i32 %I, i32 %J, ptr nocapture %A, ptr readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

declare void @f10(i32, i32)

define void @disable10(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  tail call void @f10(i32 %i, i32 %j) #0
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @disable11(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load volatile i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @disable12(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch3, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch3 ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %cmpl = icmp eq i32 %add.lcssa, 10
  br i1 %cmpl, label %for.latch2, label %for.latch3

for.latch2:                                       ; preds = %for.latch
  br label %for.latch3

for.latch3:                                       ; preds = %for.latch2, %for.latch
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch3
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @disable13(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.inner2.preheader, label %for.inner

for.inner2.preheader:                             ; preds = %for.inner
  %add.lcssa1 = phi i32 [ %add, %for.inner ]
  br label %for.inner2

for.inner2:                                       ; preds = %for.inner2.preheader, %for.inner2
  %j2 = phi i32 [ %inc2, %for.inner2 ], [ 0, %for.inner2.preheader ]
  %sum12 = phi i32 [ %add2, %for.inner2 ], [ 0, %for.inner2.preheader ]
  %arrayidx2 = getelementptr inbounds i32, ptr %B, i32 %j2
  %l0 = load i32, ptr %arrayidx2, align 4
  %add2 = add i32 %l0, %sum12
  %inc2 = add nuw i32 %j2, 1
  %exitcond2 = icmp eq i32 %inc2, %J
  br i1 %exitcond2, label %for.latch, label %for.inner2

for.latch:                                        ; preds = %for.inner2
  %add.lcssa = phi i32 [ %add.lcssa1, %for.inner2 ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @disable14(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  %add8 = add nuw i32 %i, 1
  %exitcond23 = icmp eq i32 %add8, %I
  br i1 %exitcond23, label %for.end.loopexit, label %for.inner.preheader

for.inner.preheader:                              ; preds = %for.outer
  br label %for.inner

for.inner:                                        ; preds = %for.inner.preheader, %for.inner
  %j = phi i32 [ %inc, %for.inner ], [ 0, %for.inner.preheader ]
  %sum1 = phi i32 [ %add, %for.inner ], [ 0, %for.inner.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %exitcond25 = icmp eq i32 %add8, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch, %for.outer
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @disable15(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.inner.preheader

for.inner.preheader:                              ; preds = %for.outer
  br label %for.inner

for.inner:                                        ; preds = %for.inner.preheader, %for.inner
  %j = phi i32 [ %inc, %for.inner ], [ 0, %for.inner.preheader ]
  %sum1 = phi i32 [ %add, %for.inner ], [ 0, %for.inner.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  br label %for.outer

for.end.loopexit:                                 ; preds = %for.outer
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @disable16(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  %otherphi = phi i32 [ %other, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum1 = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4
  %add = add i32 %0, %sum1
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %I
  %loadarr = getelementptr inbounds i32, ptr %A, i32 %i
  %load = load i32, ptr %arrayidx6, align 4
  %other = add i32 %otherphi, %load
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

attributes #0 = { nounwind }
