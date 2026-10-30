; ModuleID = '<stdin>'
source_filename = "<stdin>"

@global = external local_unnamed_addr global [400 x [400 x i32]], align 16

define void @test1() local_unnamed_addr {
bb:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %bb
  %outer.iv = phi i64 [ 0, %bb ], [ %outer.iv.next, %outer.latch ]
  %outer.red = phi i32 [ 0, %bb ], [ %red.next.lcssa, %outer.latch ]
  br label %inner.ph

inner.ph:                                         ; preds = %outer.header
  %add = add nsw i64 %outer.iv, 9
  br label %inner

inner:                                            ; preds = %inner, %inner.ph
  %inner.iv = phi i64 [ 0, %inner.ph ], [ %inner.iv.next, %inner ]
  %inner.red = phi i32 [ %outer.red, %inner.ph ], [ %red.next, %inner ]
  %ptr = getelementptr inbounds [400 x [400 x i32]], ptr @global, i64 0, i64 %inner.iv, i64 %add
  store i32 0, ptr %ptr, align 4
  %red.next = or i32 %inner.red, 20
  %inner.iv.next = add nsw i64 %inner.iv, 1
  %ec.1 = icmp eq i64 %inner.iv.next, 400
  br i1 %ec.1, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %red.next.lcssa = phi i32 [ %red.next, %inner ]
  %outer.iv.next = add nsw i64 %outer.iv, 1
  %ec.2 = icmp eq i64 %outer.iv.next, 391
  br i1 %ec.2, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

declare void @side_effect()

define void @test2() {
bb:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %bb
  %outer.iv = phi i64 [ 0, %bb ], [ %outer.iv.next, %outer.latch ]
  %outer.red = phi i32 [ 0, %bb ], [ %red.next.lcssa, %outer.latch ]
  br label %inner.ph

inner.ph:                                         ; preds = %outer.header
  %add = add nsw i64 %outer.iv, 9
  call void @side_effect()
  br label %inner

inner:                                            ; preds = %inner, %inner.ph
  %inner.iv = phi i64 [ 0, %inner.ph ], [ %inner.iv.next, %inner ]
  %inner.red = phi i32 [ %outer.red, %inner.ph ], [ %red.next, %inner ]
  %ptr = getelementptr inbounds [400 x [400 x i32]], ptr @global, i64 0, i64 %inner.iv, i64 %add
  store i32 0, ptr %ptr, align 4
  %red.next = or i32 %inner.red, 20
  %inner.iv.next = add nsw i64 %inner.iv, 1
  %ec.1 = icmp eq i64 %inner.iv.next, 400
  br i1 %ec.1, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %red.next.lcssa = phi i32 [ %red.next, %inner ]
  %outer.iv.next = add nsw i64 %outer.iv, 1
  %ec.2 = icmp eq i64 %outer.iv.next, 391
  br i1 %ec.2, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
