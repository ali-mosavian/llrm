; ModuleID = '<stdin>'
source_filename = "<stdin>"

@a = common global i64 0, align 4
@b = common global i64 0, align 4
@c = common global [10 x [10 x i32]] zeroinitializer, align 16

define void @test_lcssa_indvars1() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %iv.outer = phi i64 [ 0, %entry ], [ %iv.outer.next, %outer.latch ]
  br label %inner.body

inner.body:                                       ; preds = %inner.body, %outer.header
  %iv.inner = phi i64 [ 5, %outer.header ], [ %iv.inner.next, %inner.body ]
  %v7 = getelementptr inbounds [10 x [10 x i32]], ptr @c, i64 0, i64 %iv.inner, i64 %iv.outer
  store i32 0, ptr %v7, align 4
  %iv.inner.next = add nsw i64 %iv.inner, -1
  %v9 = icmp eq i64 %iv.inner, 0
  br i1 %v9, label %outer.latch, label %inner.body

outer.latch:                                      ; preds = %inner.body
  %v8.lcssa = phi i64 [ %iv.inner.next, %inner.body ]
  %iv.outer.next = add nuw nsw i64 %iv.outer, 1
  %v5 = icmp ult i64 %iv.outer, 2
  br i1 %v5, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  %v4.lcssa = phi i64 [ %iv.outer.next, %outer.latch ]
  %v8.lcssa.lcssa = phi i64 [ %v8.lcssa, %outer.latch ]
  store i64 %v8.lcssa.lcssa, ptr @b, align 4
  store i64 %v4.lcssa, ptr @a, align 4
  ret void
}

define void @test_lcssa_indvars2() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %iv.outer = phi i64 [ 0, %entry ], [ %iv.outer.next, %outer.latch ]
  br label %inner.body

inner.body:                                       ; preds = %inner.body, %outer.header
  %iv.inner = phi i64 [ 5, %outer.header ], [ %iv.inner.next, %inner.body ]
  %v7 = getelementptr inbounds [10 x [10 x i32]], ptr @c, i64 0, i64 %iv.inner, i64 %iv.outer
  store i32 0, ptr %v7, align 4
  %iv.inner.next = add nsw i64 %iv.inner, -1
  %v9 = icmp eq i64 %iv.inner.next, 0
  br i1 %v9, label %outer.latch, label %inner.body

outer.latch:                                      ; preds = %inner.body
  %v8.lcssa = phi i64 [ %iv.inner, %inner.body ]
  %iv.outer.next = add nuw nsw i64 %iv.outer, 1
  %v5 = icmp ult i64 %iv.outer.next, 2
  br i1 %v5, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  %v4.lcssa = phi i64 [ %iv.outer, %outer.latch ]
  %v8.lcssa.lcssa = phi i64 [ %v8.lcssa, %outer.latch ]
  store i64 %v8.lcssa.lcssa, ptr @b, align 4
  store i64 %v4.lcssa, ptr @a, align 4
  ret void
}

define void @test_lcssa_indvars3() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %iv.outer = phi i64 [ 0, %entry ], [ %iv.outer.next, %outer.latch ]
  br label %inner.body

inner.body:                                       ; preds = %inner.body, %outer.header
  %iv.inner = phi i64 [ 5, %outer.header ], [ %iv.inner.next, %inner.body ]
  %v7 = getelementptr inbounds [10 x [10 x i32]], ptr @c, i64 0, i64 %iv.inner, i64 %iv.outer
  store i32 0, ptr %v7, align 4
  %iv.inner.next = add nsw i64 %iv.inner, -1
  %v9 = icmp eq i64 %iv.inner, 0
  br i1 %v9, label %outer.latch, label %inner.body

outer.latch:                                      ; preds = %inner.body
  %v8.lcssa = phi i64 [ %iv.inner.next, %inner.body ]
  %iv.outer.next = add nuw nsw i64 %iv.outer, 1
  %v5 = icmp ult i64 %iv.outer, 2
  br i1 %v5, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  %v4.lcssa = phi i64 [ %iv.outer.next, %outer.latch ]
  %v8.lcssa.lcssa = phi i64 [ %v8.lcssa, %outer.latch ]
  %v8.lcssa.lcssa.2 = phi i64 [ %v8.lcssa, %outer.latch ]
  %r1 = add i64 %v8.lcssa.lcssa, %v8.lcssa.lcssa.2
  store i64 %r1, ptr @b, align 4
  store i64 %v4.lcssa, ptr @a, align 4
  ret void
}

define void @no_reachable_exits() {
bb:
  br label %outer.ph

outer.ph:                                         ; preds = %bb
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %outer.ph
  %tmp2 = phi i32 [ 0, %outer.ph ], [ %tmp8, %outer.latch ]
  br i1 undef, label %inner.ph, label %outer.latch

inner.ph:                                         ; preds = %outer.header
  br label %inner.body

inner.body:                                       ; preds = %inner.body, %inner.ph
  %tmp31 = phi i32 [ 0, %inner.ph ], [ %tmp6, %inner.body ]
  %tmp5 = load ptr, ptr undef, align 8
  %tmp6 = add nsw i32 %tmp31, 1
  br i1 false, label %inner.body, label %outer.latch.loopexit

outer.latch.loopexit:                             ; preds = %inner.body
  br label %outer.latch

outer.latch:                                      ; preds = %outer.latch.loopexit, %outer.header
  %tmp8 = add nsw i32 %tmp2, 1
  br i1 false, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  unreachable
}
