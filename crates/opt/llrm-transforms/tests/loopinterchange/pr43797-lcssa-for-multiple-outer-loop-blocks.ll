; ModuleID = '<stdin>'
source_filename = "<stdin>"

@wdtdr = external global [5 x [5 x double]], align 16

define void @test1() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %outer.idx = phi i64 [ 0, %entry ], [ %outer.idx.inc, %outer.latch ]
  %arrayidx8 = getelementptr inbounds [5 x [5 x double]], ptr @wdtdr, i64 0, i64 0, i64 %outer.idx
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %inner.idx = phi i64 [ 0, %outer.header ], [ %inner.idx.inc, %inner.latch ]
  %0 = load double, ptr %arrayidx8, align 8
  store double undef, ptr %arrayidx8, align 8
  br label %inner.latch

inner.latch:                                      ; preds = %inner.header
  %inner.idx.inc = add nsw i64 %inner.idx, 1
  br i1 false, label %inner.header, label %inner.exit

inner.exit:                                       ; preds = %inner.latch
  %outer.v = add nsw i64 %outer.idx, 1
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %outer.idx.inc = add nsw i64 %outer.idx, 1
  br i1 false, label %outer.header, label %outer.exit

outer.exit:                                       ; preds = %outer.latch
  %exit1.lcssa = phi i64 [ %outer.v, %outer.latch ]
  %exit2.lcssa = phi i64 [ %outer.idx.inc, %outer.latch ]
  ret void
}

define void @test2(i1 %cond) {
entry:
  br i1 %cond, label %outer.header.preheader, label %outer.exit

outer.header.preheader:                           ; preds = %entry
  br label %outer.header

outer.header:                                     ; preds = %outer.header.preheader, %outer.latch
  %outer.idx = phi i64 [ %outer.idx.inc, %outer.latch ], [ 0, %outer.header.preheader ]
  %arrayidx8 = getelementptr inbounds [5 x [5 x double]], ptr @wdtdr, i64 0, i64 0, i64 %outer.idx
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %inner.idx = phi i64 [ 0, %outer.header ], [ %inner.idx.inc, %inner.latch ]
  %0 = load double, ptr %arrayidx8, align 8
  store double undef, ptr %arrayidx8, align 8
  br label %inner.latch

inner.latch:                                      ; preds = %inner.header
  %inner.idx.inc = add nsw i64 %inner.idx, 1
  br i1 false, label %inner.header, label %inner.exit

inner.exit:                                       ; preds = %inner.latch
  %outer.v = add nsw i64 %outer.idx, 1
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %outer.idx.inc = add nsw i64 %outer.idx, 1
  br i1 false, label %outer.header, label %outer.exit.loopexit

outer.exit.loopexit:                              ; preds = %outer.latch
  %outer.idx.inc.lcssa = phi i64 [ %outer.idx.inc, %outer.latch ]
  %outer.v.lcssa = phi i64 [ %outer.v, %outer.latch ]
  br label %outer.exit

outer.exit:                                       ; preds = %outer.exit.loopexit, %entry
  %exit1.lcssa = phi i64 [ 0, %entry ], [ %outer.v.lcssa, %outer.exit.loopexit ]
  %exit2.lcssa = phi i64 [ 0, %entry ], [ %outer.idx.inc.lcssa, %outer.exit.loopexit ]
  ret void
}
