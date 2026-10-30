; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64"

%struct.a = type { i64 }

@a = global [1 x i32] zeroinitializer, align 4
@f = global i32 0, align 4
@g = common global %struct.a zeroinitializer, align 8
@c = common global [1 x i8] zeroinitializer, align 1

define void @test1(i32 %N, i32 %M, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %M, 0
  %cmpJ = icmp ne i32 %N, 0
  %or.cond = and i1 %cmp, %cmpJ
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4, !tbaa !0
  %add = add i32 %0, %sum
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %M
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4, !tbaa !0
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %N
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @test2(i32 %N, i32 %M, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %M, 0
  %cmp125 = icmp ne i32 %N, 0
  %or.cond = and i1 %cmp, %cmp125
  br i1 %or.cond, label %for.outer.preheader, label %for.end10

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add9, %for.latch ], [ 0, %for.outer.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  %0 = load i32, ptr %arrayidx, align 4, !tbaa !0
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum = phi i32 [ %0, %for.outer ], [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %B, i32 %j
  %1 = load i32, ptr %arrayidx6, align 4, !tbaa !0
  %add = add i32 %1, %sum
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %M
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  store i32 %add.lcssa, ptr %arrayidx, align 4, !tbaa !0
  %add9 = add nuw i32 %i, 1
  %exitcond28 = icmp eq i32 %add9, %N
  br i1 %exitcond28, label %for.end10.loopexit, label %for.outer

for.end10.loopexit:                               ; preds = %for.latch
  br label %for.end10

for.end10:                                        ; preds = %for.end10.loopexit, %entry
  ret void
}

define void @test2_consts(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer.preheader

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add9, %for.latch ], [ 0, %for.outer.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  %0 = load i32, ptr %arrayidx, align 4, !tbaa !0
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum = phi i32 [ %0, %for.outer ], [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %B, i32 %j
  %1 = load i32, ptr %arrayidx6, align 4, !tbaa !0
  %add = add i32 %1, %sum
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, 101
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  store i32 %add.lcssa, ptr %arrayidx, align 4, !tbaa !0
  %add9 = add nuw i32 %i, 1
  %exitcond28 = icmp eq i32 %add9, 101
  br i1 %exitcond28, label %for.end10.loopexit, label %for.outer

for.end10.loopexit:                               ; preds = %for.latch
  br label %for.end10

for.end10:                                        ; preds = %for.end10.loopexit
  ret void
}

define void @test3(i32 %M, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp eq i32 %M, 0
  br i1 %cmp, label %for.end, label %for.preheader

for.preheader:                                    ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4, !tbaa !0
  %sub = add i32 %sum, 10
  %add = sub i32 %sub, %0
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %M
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4, !tbaa !0
  %add8 = add nuw nsw i32 %i, 1
  %exitcond23 = icmp eq i32 %add8, 4
  br i1 %exitcond23, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @test4(i32 %M, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp eq i32 %M, 0
  br i1 %cmp, label %for.end, label %for.preheader

for.preheader:                                    ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx, align 4, !tbaa !0
  %sub = add i32 %sum, 10
  %add = sub i32 %sub, %0
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %M
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4, !tbaa !0
  %add8 = add nuw nsw i32 %i, 1
  %exitcond23 = icmp eq i32 %add8, 1
  br i1 %exitcond23, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define i32 @test5() {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %.sink16 = phi i32 [ 0, %entry ], [ %add, %for.latch ]
  br label %for.inner

for.inner:                                        ; preds = %for.inc, %for.outer
  %inc8.sink15 = phi i32 [ 0, %for.outer ], [ %inc8, %for.inc ]
  br label %for.inner2

for.inner2:                                       ; preds = %for.inner
  %l1 = load i32, ptr @a, align 4
  %tobool = icmp eq i32 %l1, 0
  br i1 %tobool, label %for.cond4, label %for.inc

for.cond4:                                        ; preds = %for.inner2
  %l0 = load i32, ptr getelementptr inbounds ([1 x i32], ptr @a, i32 1, i32 0), align 4
  %tobool.1 = icmp eq i32 %l0, 0
  br i1 %tobool.1, label %for.cond4a, label %for.inc

for.cond4a:                                       ; preds = %for.cond4
  br label %for.inc

for.inc:                                          ; preds = %for.cond4a, %for.cond4, %for.inner2
  %l2 = phi i32 [ 0, %for.inner2 ], [ 1, %for.cond4 ], [ 2, %for.cond4a ]
  %inc8 = add nuw nsw i32 %inc8.sink15, 1
  %exitcond = icmp eq i32 %inc8, 3
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inc
  %.lcssa = phi i32 [ %l2, %for.inc ]
  %conv11 = and i32 %.sink16, 255
  %add = add nuw nsw i32 %conv11, 4
  %cmp = icmp eq i32 %add, 8
  br i1 %cmp, label %for.end, label %for.outer

for.end:                                          ; preds = %for.latch
  %.lcssa.lcssa = phi i32 [ %.lcssa, %for.latch ]
  ret i32 0
}

define i32 @test6() {
entry:
  %f.promoted10 = load i32, ptr @f, align 4, !tbaa !0
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %p0 = phi i32 [ %f.promoted10, %entry ], [ 2, %for.latch ]
  %inc5.sink9 = phi i32 [ 2, %entry ], [ %inc5, %for.latch ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %p1 = phi i32 [ %p0, %for.outer ], [ 2, %for.inner ]
  %inc.sink8 = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %inc = add nuw nsw i32 %inc.sink8, 1
  %exitcond = icmp ne i32 %inc, 7
  br i1 %exitcond, label %for.inner, label %for.latch

for.latch:                                        ; preds = %for.inner
  %.lcssa = phi i32 [ %p1, %for.inner ]
  %inc5 = add nuw nsw i32 %inc5.sink9, 1
  %exitcond11 = icmp ne i32 %inc5, 7
  br i1 %exitcond11, label %for.outer, label %for.end

for.end:                                          ; preds = %for.latch
  %.lcssa.lcssa = phi i32 [ %.lcssa, %for.latch ]
  %inc.lcssa.lcssa = phi i32 [ 7, %for.latch ]
  ret i32 0
}

define void @test7(i32 %N, i32 %M, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %M, 0
  %cmp128 = icmp ne i32 %N, 0
  %or.cond = and i1 %cmp128, %cmp
  br i1 %or.cond, label %for.preheader, label %for.end

for.preheader:                                    ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.preheader
  %i = phi i32 [ %add, %for.latch ], [ 0, %for.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 0, ptr %arrayidx, align 4, !tbaa !0
  %add = add nuw i32 %i, 1
  %arrayidx2 = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 2, ptr %arrayidx2, align 4, !tbaa !0
  br label %for.inner

for.latch:                                        ; preds = %for.inner
  %add9.lcssa = phi i32 [ %add9, %for.inner ]
  store i32 %add9.lcssa, ptr %arrayidx, align 4, !tbaa !0
  %exitcond30 = icmp eq i32 %add, %N
  br i1 %exitcond30, label %for.end.loopexit, label %for.outer

for.inner:                                        ; preds = %for.inner, %for.outer
  %sum = phi i32 [ 0, %for.outer ], [ %add9, %for.inner ]
  %j = phi i32 [ 0, %for.outer ], [ %add10, %for.inner ]
  %arrayidx7 = getelementptr inbounds i32, ptr %B, i32 %j
  %l1 = load i32, ptr %arrayidx7, align 4, !tbaa !0
  %add9 = add i32 %l1, %sum
  %add10 = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %add10, %M
  br i1 %exitcond, label %for.latch, label %for.inner

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @test7_consts(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.preheader

for.preheader:                                    ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.preheader
  %i = phi i32 [ %add, %for.latch ], [ 0, %for.preheader ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 0, ptr %arrayidx, align 4, !tbaa !0
  %add = add nuw i32 %i, 1
  %arrayidx2 = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 2, ptr %arrayidx2, align 4, !tbaa !0
  br label %for.inner

for.latch:                                        ; preds = %for.inner
  %add9.lcssa = phi i32 [ %add9, %for.inner ]
  store i32 %add9.lcssa, ptr %arrayidx, align 4, !tbaa !0
  %exitcond30 = icmp eq i32 %add, 101
  br i1 %exitcond30, label %for.end, label %for.outer

for.inner:                                        ; preds = %for.inner, %for.outer
  %sum = phi i32 [ 0, %for.outer ], [ %add9, %for.inner ]
  %j = phi i32 [ 0, %for.outer ], [ %add10, %for.inner ]
  %arrayidx7 = getelementptr inbounds i32, ptr %B, i32 %j
  %l1 = load i32, ptr %arrayidx7, align 4, !tbaa !0
  %add9 = add i32 %l1, %sum
  %add10 = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %add10, 101
  br i1 %exitcond, label %for.latch, label %for.inner

for.end:                                          ; preds = %for.latch
  ret void
}

define void @test8(i32 %N, i32 %M, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp eq i32 %M, 0
  %cmp336 = icmp eq i32 %N, 0
  %or.cond = or i1 %cmp, %cmp336
  br i1 %or.cond, label %for.end, label %for.preheader

for.preheader:                                    ; preds = %entry
  br label %for.outest

for.outest:                                       ; preds = %for.cleanup, %for.preheader
  %x.038 = phi i32 [ %inc, %for.cleanup ], [ 0, %for.preheader ]
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outest
  %i = phi i32 [ %add, %for.latch ], [ 0, %for.outest ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 0, ptr %arrayidx, align 4, !tbaa !0
  %add = add nuw i32 %i, 1
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 2, ptr %arrayidx6, align 4, !tbaa !0
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %sum = phi i32 [ 0, %for.outer ], [ %add9, %for.inner ]
  %j = phi i32 [ 0, %for.outer ], [ %add10, %for.inner ]
  %arrayidx11 = getelementptr inbounds i32, ptr %B, i32 %j
  %l1 = load i32, ptr %arrayidx11, align 4, !tbaa !0
  %add9 = add i32 %l1, %sum
  %add10 = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %add10, %M
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add9.lcssa = phi i32 [ %add9, %for.inner ]
  store i32 %add9.lcssa, ptr %arrayidx, align 4, !tbaa !0
  %exitcond39 = icmp eq i32 %add, %N
  br i1 %exitcond39, label %for.cleanup, label %for.outer

for.cleanup:                                      ; preds = %for.latch
  %inc = add nuw nsw i32 %x.038, 1
  %exitcond41 = icmp eq i32 %inc, 5
  br i1 %exitcond41, label %for.end.loopexit, label %for.outest

for.end.loopexit:                                 ; preds = %for.cleanup
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @test8_consts(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.end

for.preheader:                                    ; No predecessors!
  br label %for.outest

for.outest:                                       ; preds = %for.cleanup, %for.preheader
  %x.038 = phi i32 [ %inc, %for.cleanup ], [ 0, %for.preheader ]
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outest
  %i = phi i32 [ %add, %for.latch ], [ 0, %for.outest ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 0, ptr %arrayidx, align 4, !tbaa !0
  %add = add nuw i32 %i, 1
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 2, ptr %arrayidx6, align 4, !tbaa !0
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %sum = phi i32 [ 0, %for.outer ], [ %add9, %for.inner ]
  %j = phi i32 [ 0, %for.outer ], [ %add10, %for.inner ]
  %arrayidx11 = getelementptr inbounds i32, ptr %B, i32 %j
  %l1 = load i32, ptr %arrayidx11, align 4, !tbaa !0
  %add9 = add i32 %l1, %sum
  %add10 = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %add10, 101
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  store i32 %add9, ptr %arrayidx, align 4, !tbaa !0
  %exitcond39 = icmp eq i32 %add, 101
  br i1 %exitcond39, label %for.cleanup, label %for.outer

for.cleanup:                                      ; preds = %for.latch
  %inc = add nuw nsw i32 %x.038, 1
  %exitcond41 = icmp eq i32 %inc, 5
  br i1 %exitcond41, label %for.end, label %for.outest

for.end:                                          ; preds = %for.cleanup, %entry
  ret void
}

define void @test9(i32 %N, i32 %M, ptr nocapture %A, ptr readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %M, 0
  %cmpJ = icmp ne i32 %N, 0
  %or.cond = and i1 %cmp, %cmpJ
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i = phi i32 [ %add8, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ 0, %for.outer ], [ %inc, %for.inner ]
  %sum = phi i32 [ 0, %for.outer ], [ %add, %for.inner ]
  %arrayidx = getelementptr inbounds i16, ptr %B, i32 %j
  %0 = load i16, ptr %arrayidx, align 4, !tbaa !4
  %sext = sext i16 %0 to i32
  %add = add i32 %sext, %sum
  %inc = add nuw i32 %j, 1
  %exitcond = icmp eq i32 %inc, %M
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx6, align 4, !tbaa !0
  %add8 = add nuw i32 %i, 1
  %exitcond25 = icmp eq i32 %add8, %N
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define signext i16 @test10(i32 %k) {
entry:
  %0 = load i8, ptr @c, align 1
  %tobool9 = icmp eq i8 %0, 0
  %tobool13 = icmp ne i32 %k, 0
  br label %for.body

for.body:                                         ; preds = %for.inc24, %entry
  %storemerge82 = phi i64 [ 0, %entry ], [ %inc25, %for.inc24 ]
  br label %for.body2

for.body2:                                        ; preds = %for.inc21, %for.body
  %storemerge = phi i64 [ 4, %for.body ], [ %dec, %for.inc21 ]
  br i1 %tobool9, label %for.body2.split, label %for.body2.split2

for.body2.split2:                                 ; preds = %for.body2
  br i1 %tobool13, label %for.inc21, label %for.inc21.if

for.body2.split:                                  ; preds = %for.body2
  br i1 %tobool13, label %for.inc21, label %for.inc21.then

for.inc21.if:                                     ; preds = %for.body2.split2
  %storemerge.1 = phi i64 [ 0, %for.body2.split2 ]
  br label %for.inc21

for.inc21.then:                                   ; preds = %for.body2.split
  %storemerge.2 = phi i64 [ 0, %for.body2.split ]
  %storemerge.3 = phi i32 [ 0, %for.body2.split ]
  br label %for.inc21

for.inc21:                                        ; preds = %for.inc21.then, %for.inc21.if, %for.body2.split, %for.body2.split2
  %storemerge.4 = phi i64 [ %storemerge.1, %for.inc21.if ], [ %storemerge.2, %for.inc21.then ], [ 4, %for.body2.split2 ], [ 4, %for.body2.split ]
  %storemerge.5 = phi i32 [ 0, %for.inc21.if ], [ %storemerge.3, %for.inc21.then ], [ 0, %for.body2.split2 ], [ 0, %for.body2.split ]
  %dec = add nsw i64 %storemerge, -1
  %tobool = icmp eq i64 %dec, 0
  br i1 %tobool, label %for.inc24, label %for.body2

for.inc24:                                        ; preds = %for.inc21
  %storemerge.4.lcssa = phi i64 [ %storemerge.4, %for.inc21 ]
  %storemerge.5.lcssa = phi i32 [ %storemerge.5, %for.inc21 ]
  %inc25 = add nuw nsw i64 %storemerge82, 1
  %exitcond = icmp ne i64 %inc25, 5
  br i1 %exitcond, label %for.body, label %for.end26

for.end26:                                        ; preds = %for.inc24
  %dec.lcssa.lcssa = phi i64 [ 0, %for.inc24 ]
  %storemerge.4.lcssa.lcssa = phi i64 [ %storemerge.4.lcssa, %for.inc24 ]
  %storemerge.5.lcssa.lcssa = phi i32 [ %storemerge.5.lcssa, %for.inc24 ]
  store i64 %dec.lcssa.lcssa, ptr @g, align 8
  ret i16 0
}

!0 = !{!1, !1, i64 0}
!1 = !{!"int", !2, i64 0}
!2 = !{!"omnipotent char", !3, i64 0}
!3 = !{!"Simple C/C++ TBAA"}
!4 = !{!5, !5, i64 0}
!5 = !{!"short", !2, i64 0}
