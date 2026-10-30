; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @func(ptr noalias readonly %a, ptr noalias readonly %b, ptr noalias writeonly %s) {
entry:
  br label %outerloop_header

outerloop_header:                                 ; preds = %outerloop_latch, %entry
  %index_i = phi i64 [ 0, %entry ], [ %index_i.next, %outerloop_latch ]
  %addr_s = getelementptr inbounds nuw double, ptr %s, i64 %index_i
  %invariant.gep.us = getelementptr inbounds nuw double, ptr %a, i64 %index_i
  %invariant.gep32.us = getelementptr inbounds nuw double, ptr %b, i64 %index_i
  br label %innerloop

innerloop:                                        ; preds = %innerloop, %outerloop_header
  %index_j = phi i64 [ 0, %outerloop_header ], [ %index_j.next, %innerloop ]
  %reduction = phi double [ 0.000000e+00, %outerloop_header ], [ %add, %innerloop ]
  %addr_a_j_i = getelementptr inbounds nuw [100 x double], ptr %invariant.gep.us, i64 %index_j
  %0 = load double, ptr %addr_a_j_i, align 8
  %addr_b_j_i = getelementptr inbounds nuw [100 x double], ptr %invariant.gep32.us, i64 %index_j
  %1 = load double, ptr %addr_b_j_i, align 8
  %mul = fmul fast double %1, %0
  %add = fadd fast double %mul, %reduction
  %index_j.next = add nuw nsw i64 %index_j, 1
  %cond1 = icmp eq i64 %index_j.next, 100
  br i1 %cond1, label %outerloop_latch, label %innerloop

outerloop_latch:                                  ; preds = %innerloop
  %lcssa = phi double [ %add, %innerloop ]
  store double %lcssa, ptr %addr_s, align 8
  %index_i.next = add nuw nsw i64 %index_i, 1
  %cond2 = icmp eq i64 %index_i.next, 100
  br i1 %cond2, label %exit, label %outerloop_header

exit:                                             ; preds = %outerloop_latch
  ret void
}
