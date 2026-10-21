; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @reduction_01(ptr noalias readonly %a, ptr noalias readonly %b, ptr noalias writeonly %s) {
entry:
  br label %outerloop_header

outerloop_header:                                 ; preds = %outerloop_latch, %entry
  %index_i = phi i64 [ 0, %entry ], [ %index_i.next, %outerloop_latch ]
  %addr_s = getelementptr inbounds nuw double, ptr %s, i64 %index_i
  %invariant.gep.us = getelementptr inbounds nuw double, ptr %a, i64 %index_i
  %invariant.gep32.us = getelementptr inbounds nuw double, ptr %b, i64 %index_i
  %s_init = load double, ptr %addr_s, align 8
  br label %innerloop

innerloop:                                        ; preds = %innerloop, %outerloop_header
  %index_j = phi i64 [ 0, %outerloop_header ], [ %index_j.next, %innerloop ]
  %reduction = phi double [ %s_init, %outerloop_header ], [ %add, %innerloop ]
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

define void @reduction_02(ptr noalias readonly %a, ptr noalias readonly %b, ptr noalias writeonly %s, ptr noalias writeonly %s2) {
entry:
  br label %outerloop_header

outerloop_header:                                 ; preds = %outerloop_latch, %entry
  %index_i = phi i64 [ 0, %entry ], [ %index_i.next, %outerloop_latch ]
  %addr_s = getelementptr inbounds nuw double, ptr %s, i64 %index_i
  %addr_s2 = getelementptr inbounds nuw double, ptr %s2, i64 %index_i
  %invariant.gep.us = getelementptr inbounds nuw double, ptr %a, i64 %index_i
  %invariant.gep32.us = getelementptr inbounds nuw double, ptr %b, i64 %index_i
  br label %innerloop

innerloop:                                        ; preds = %innerloop, %outerloop_header
  %index_j = phi i64 [ 0, %outerloop_header ], [ %index_j.next, %innerloop ]
  %reduction = phi double [ 0.000000e+00, %outerloop_header ], [ %add, %innerloop ]
  %reduction2 = phi double [ 0.000000e+00, %outerloop_header ], [ %add2, %innerloop ]
  %addr_a_j_i = getelementptr inbounds nuw [100 x double], ptr %invariant.gep.us, i64 %index_j
  %0 = load double, ptr %addr_a_j_i, align 8
  %addr_b_j_i = getelementptr inbounds nuw [100 x double], ptr %invariant.gep32.us, i64 %index_j
  %1 = load double, ptr %addr_b_j_i, align 8
  %mul = fmul fast double %1, %0
  %add = fadd fast double %mul, %reduction
  %add2 = fadd fast double %reduction2, %0
  %index_j.next = add nuw nsw i64 %index_j, 1
  %cond1 = icmp eq i64 %index_j.next, 100
  br i1 %cond1, label %outerloop_latch, label %innerloop

outerloop_latch:                                  ; preds = %innerloop
  %lcssa = phi double [ %add, %innerloop ]
  %lcssa2 = phi double [ %add2, %innerloop ]
  store double %lcssa, ptr %addr_s, align 8
  store double %lcssa2, ptr %addr_s2, align 8
  %index_i.next = add nuw nsw i64 %index_i, 1
  %cond2 = icmp eq i64 %index_i.next, 100
  br i1 %cond2, label %exit, label %outerloop_header

exit:                                             ; preds = %outerloop_latch
  ret void
}

define void @reduction_03(ptr noalias readonly %a, ptr noalias readonly %b, ptr noalias writeonly %s) {
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
  %add17.us = fadd fast double %lcssa, 1.000000e+00
  store double %add17.us, ptr %addr_s, align 8
  %index_i.next = add nuw nsw i64 %index_i, 1
  %cond2 = icmp eq i64 %index_i.next, 100
  br i1 %cond2, label %exit, label %outerloop_header

exit:                                             ; preds = %outerloop_latch
  ret void
}

define void @reduction_04(ptr noalias readonly %a, ptr noalias readonly %b, ptr noalias writeonly %c, ptr noalias writeonly %s) {
entry:
  br label %i_loop_header

i_loop_header:                                    ; preds = %i_loop_latch, %entry
  %index_i = phi i64 [ 0, %entry ], [ %index_i.next, %i_loop_latch ]
  %addr_s = getelementptr inbounds nuw double, ptr %s, i64 %index_i
  %invariant.gep.us = getelementptr inbounds nuw double, ptr %a, i64 %index_i
  %invariant.gep32.us = getelementptr inbounds nuw double, ptr %b, i64 %index_i
  br label %j_loop

j_loop:                                           ; preds = %j_loop_latch, %i_loop_header
  %index_j = phi i64 [ 0, %i_loop_header ], [ %index_j.next, %j_loop_latch ]
  %reduction = phi double [ 0.000000e+00, %i_loop_header ], [ %add, %j_loop_latch ]
  %addr_a_j_i = getelementptr inbounds nuw [100 x double], ptr %invariant.gep.us, i64 %index_j
  %0 = load double, ptr %addr_a_j_i, align 8
  %addr_b_j_i = getelementptr inbounds nuw [100 x double], ptr %invariant.gep32.us, i64 %index_j
  %1 = load double, ptr %addr_b_j_i, align 8
  %mul = fmul fast double %1, %0
  %add = fadd fast double %mul, %reduction
  br label %k_loop

k_loop:                                           ; preds = %k_loop, %j_loop
  %index_k = phi i64 [ %index_k.next, %k_loop ], [ 0, %j_loop ]
  %arrayidx22.us.us = getelementptr inbounds nuw double, ptr %c, i64 %index_k
  %index_k.next = add nuw nsw i64 %index_k, 1
  %exitcond.not = icmp eq i64 %index_k.next, 100
  br i1 %exitcond.not, label %j_loop_latch, label %k_loop

j_loop_latch:                                     ; preds = %k_loop
  %index_j.next = add nuw nsw i64 %index_j, 1
  %cond1 = icmp eq i64 %index_j.next, 100
  br i1 %cond1, label %i_loop_latch, label %j_loop

i_loop_latch:                                     ; preds = %j_loop_latch
  %lcssa = phi double [ %add, %j_loop_latch ]
  store double %lcssa, ptr %addr_s, align 8
  %index_i.next = add nuw nsw i64 %index_i, 1
  %cond2 = icmp eq i64 %index_i.next, 100
  br i1 %cond2, label %exit, label %i_loop_header

exit:                                             ; preds = %i_loop_latch
  ret void
}

define void @reduction_05(ptr noalias readonly %a, ptr noalias readonly %b, ptr noalias writeonly %s) {
entry:
  br label %outerloop_header

outerloop_header:                                 ; preds = %outerloop_latch, %entry
  %index_i = phi i64 [ 0, %entry ], [ %index_i.next, %outerloop_latch ]
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
  %addr_s = getelementptr inbounds nuw double, ptr %s, i64 %index_i
  store double %lcssa, ptr %addr_s, align 8
  %index_i.next = add nuw nsw i64 %index_i, 1
  %cond2 = icmp eq i64 %index_i.next, 100
  br i1 %cond2, label %exit, label %outerloop_header

exit:                                             ; preds = %outerloop_latch
  ret void
}
