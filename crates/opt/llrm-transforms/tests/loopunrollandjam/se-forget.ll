; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-p:64:64-n32:64-S128-v256:256:256-v512:512:512"

define void @main() {
entry:
  %x = alloca [216 x float], align 4
  %y = alloca [216 x float], align 4
  %x_ix_dim_0 = getelementptr i8, ptr %x, i64 -52
  %y_ix_dim_0 = getelementptr i8, ptr %y, i64 -52
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %outer.iv = phi i64 [ 1, %entry ], [ %outer.iv.next, %outer.latch ]
  %_ix_x_len.1 = shl nuw nsw i64 %outer.iv, 2
  %x_ix_dim_1 = getelementptr i8, ptr %x_ix_dim_0, i64 20
  %y_ix_dim_1 = getelementptr i8, ptr %y_ix_dim_0, i64 %_ix_x_len.1
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %inner.iv.1 = phi i64 [ 1, %outer.header ], [ 0, %inner ]
  %inner.iv = phi i64 [ 1, %outer.header ], [ %inner.iv.next, %inner ]
  %_ix_x_len = mul nuw nsw i64 %inner.iv.1, 144
  %x_ix_dim_2 = getelementptr i8, ptr %x_ix_dim_1, i64 %_ix_x_len
  %y_ix_dim_2 = getelementptr i8, ptr %y_ix_dim_1, i64 %_ix_x_len
  %_val_y = load float, ptr %y_ix_dim_2, align 4
  %_add_tmp = fadd fast float %_val_y, 0.000000e+00
  store float %_add_tmp, ptr %x_ix_dim_2, align 8
  %inner.iv.next = add nuw nsw i64 %inner.iv, 1
  %exitcond.not = icmp eq i64 %inner.iv.next, 7
  br i1 %exitcond.not, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %outer.iv.next = add nuw nsw i64 %outer.iv, 1
  %exitcond.not.1 = icmp eq i64 %outer.iv.next, 7
  br i1 %exitcond.not.1, label %exit, label %outer.header, !llvm.loop !0

exit:                                             ; preds = %outer.latch
  ret void
}

!0 = distinct !{!0, !1}
!1 = !{!"llvm.loop.unroll_and_jam.enable"}
