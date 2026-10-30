; ModuleID = '<stdin>'
source_filename = "<stdin>"

@a = common global i32 0, align 4
@d = common local_unnamed_addr global [1 x [6 x i32]] zeroinitializer, align 4

define void @innermost_latch_uses_values_in_middle_header() {
entry:
  %0 = load i32, ptr @a, align 4
  %b = add i32 80, 1
  br label %outermost.header

outermost.header:                                 ; preds = %outermost.latch, %entry
  %indvar.outermost = phi i32 [ 10, %entry ], [ %indvar.outermost.next, %outermost.latch ]
  %tobool71.i = icmp eq i32 %0, 0
  br i1 %tobool71.i, label %middle.header.preheader, label %outermost.latch

middle.header.preheader:                          ; preds = %outermost.header
  br label %middle.header

middle.header:                                    ; preds = %middle.header.preheader, %middle.latch
  %indvar.middle = phi i64 [ %indvar.middle.next, %middle.latch ], [ 4, %middle.header.preheader ]
  %indvar.middle.wide = zext i32 %b to i64
  br label %innermost.header

innermost.header:                                 ; preds = %innermost.latch, %middle.header
  %indvar.innermost = phi i64 [ %indvar.innermost.next, %innermost.latch ], [ 4, %middle.header ]
  br label %innermost.body

innermost.body:                                   ; preds = %innermost.header
  %arrayidx9.i = getelementptr inbounds [1 x [6 x i32]], ptr @d, i64 0, i64 %indvar.innermost, i64 %indvar.middle
  store i32 0, ptr %arrayidx9.i, align 4
  br label %innermost.latch

innermost.latch:                                  ; preds = %innermost.body
  %indvar.innermost.next = add nsw i64 %indvar.innermost, 1
  %tobool5.i = icmp eq i64 %indvar.innermost.next, %indvar.middle.wide
  br i1 %tobool5.i, label %middle.latch, label %innermost.header

middle.latch:                                     ; preds = %innermost.latch
  %indvar.middle.next = add nsw i64 %indvar.middle, -1
  %tobool2.i = icmp eq i64 %indvar.middle.next, 0
  br i1 %tobool2.i, label %outermost.latch.loopexit, label %middle.header

outermost.latch.loopexit:                         ; preds = %middle.latch
  br label %outermost.latch

outermost.latch:                                  ; preds = %outermost.latch.loopexit, %outermost.header
  %indvar.outermost.next = add nsw i32 %indvar.outermost, -5
  %tobool.i = icmp eq i32 %indvar.outermost.next, 0
  br i1 %tobool.i, label %outermost.exit, label %outermost.header

outermost.exit:                                   ; preds = %outermost.latch
  ret void
}
