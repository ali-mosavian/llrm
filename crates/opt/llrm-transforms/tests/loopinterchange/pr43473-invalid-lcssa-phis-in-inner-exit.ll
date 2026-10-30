; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @test1(i1 %arg) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %outer.iv = phi i64 [ undef, %entry ], [ %outer.iv.next, %outer.latch ]
  %idx = getelementptr inbounds double, ptr undef, i64 %outer.iv
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %inner.iv = phi i64 [ 0, %outer.header ], [ %inner.iv.next, %inner ]
  %0 = load double, ptr %idx, align 8
  store double undef, ptr %idx, align 8
  %inner.iv.next = add nuw nsw i64 %inner.iv, 1
  br i1 %arg, label %inner, label %outer.latch

outer.latch:                                      ; preds = %inner
  %inc43.lcssa.wide.us = phi i64 [ %inner.iv.next, %inner ]
  %1 = trunc i64 %inc43.lcssa.wide.us to i32
  %outer.iv.next = add nsw i64 %outer.iv, 1
  br i1 %arg, label %outer.header, label %outer.exit

outer.exit:                                       ; preds = %outer.latch
  ret void
}

define void @test2(i1 %arg) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %outer.iv = phi i64 [ undef, %entry ], [ %outer.iv.next, %outer.latch ]
  %idx = getelementptr inbounds double, ptr undef, i64 %outer.iv
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %inner.iv = phi i64 [ 0, %outer.header ], [ %inner.iv.next, %inner ]
  %0 = load double, ptr %idx, align 8
  store double undef, ptr %idx, align 8
  %inner.iv.next = add nuw nsw i64 %inner.iv, 1
  br i1 %arg, label %inner, label %inner.exit

inner.exit:                                       ; preds = %inner
  %inc43.lcssa.wide.us = phi i64 [ %inner.iv.next, %inner ]
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %1 = trunc i64 %inc43.lcssa.wide.us to i32
  %outer.iv.next = add nsw i64 %outer.iv, 1
  br i1 %arg, label %outer.header, label %outer.exit

outer.exit:                                       ; preds = %outer.latch
  ret void
}
