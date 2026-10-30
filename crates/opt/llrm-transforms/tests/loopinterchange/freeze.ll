; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @outer_preheader_freeze(ptr noalias %A, ptr noalias %R) {
entry:
  br label %outer.preheader

outer.preheader:                                  ; preds = %entry
  %choice = freeze i1 poison
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %outer.preheader
  %i = phi i64 [ 0, %outer.preheader ], [ %i.next, %outer.latch ]
  %sum.i = phi double [ 0.000000e+00, %outer.preheader ], [ %sum.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %sum.j = phi double [ %sum.i, %outer.header ], [ %sum.next.j, %inner.header ]
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  %value = load double, ptr %idx, align 8
  %selected = select i1 %choice, double %value, double 0.000000e+00
  %sum.next.j = fadd reassoc double %sum.j, %selected
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %sum.next = phi double [ %sum.next.j, %inner.header ]
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %sum.result = phi double [ %sum.next, %outer.latch ]
  store double %sum.result, ptr %R, align 8
  ret void
}

define void @inner_body_freeze(ptr noalias %A, ptr noalias %R) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %sum.i = phi double [ 0.000000e+00, %entry ], [ %sum.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %sum.j = phi double [ %sum.i, %outer.header ], [ %sum.next.j, %inner.header ]
  %choice = freeze i1 poison
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  %value = load double, ptr %idx, align 8
  %selected = select i1 %choice, double %value, double 0.000000e+00
  %sum.next.j = fadd reassoc double %sum.j, %selected
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %sum.next = phi double [ %sum.next.j, %inner.header ]
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %sum.result = phi double [ %sum.next, %outer.latch ]
  store double %sum.result, ptr %R, align 8
  ret void
}

define void @header_preheader_control(ptr noalias %A, i1 %choice) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %header.bit = xor i1 %choice, true
  br label %inner.preheader

inner.preheader:                                  ; preds = %outer.header
  %preheader.bit = xor i1 %choice, false
  %selected.bit = and i1 %header.bit, %preheader.bit
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %inner.preheader
  %j = phi i64 [ 0, %inner.preheader ], [ %j.next, %inner.header ]
  %selected = select i1 %selected.bit, double 1.000000e+00, double 0.000000e+00
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double %selected, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @outer_header_freeze(ptr noalias %A, i1 %choice) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %header.bit = freeze i1 poison
  br label %inner.preheader

inner.preheader:                                  ; preds = %outer.header
  %preheader.bit = xor i1 %choice, false
  %selected.bit = and i1 %header.bit, %preheader.bit
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %inner.preheader
  %j = phi i64 [ 0, %inner.preheader ], [ %j.next, %inner.header ]
  %selected = select i1 %selected.bit, double 1.000000e+00, double 0.000000e+00
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double %selected, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @inner_preheader_freeze(ptr noalias %A, i1 %choice) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  %header.bit = xor i1 %choice, true
  br label %inner.preheader

inner.preheader:                                  ; preds = %outer.header
  %preheader.bit = freeze i1 poison
  %selected.bit = and i1 %header.bit, %preheader.bit
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %inner.preheader
  %j = phi i64 [ 0, %inner.preheader ], [ %j.next, %inner.header ]
  %selected = select i1 %selected.bit, double 1.000000e+00, double 0.000000e+00
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double %selected, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @exit_latch_control(ptr noalias %A, ptr noalias %R, i1 %choice) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double 1.000000e+00, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %inner.exit, label %inner.header

inner.exit:                                       ; preds = %inner.header
  %exit.bit = xor i1 %choice, true
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %exit.bit.lcssa = phi i1 [ %exit.bit, %inner.exit ]
  %i.next = add i64 %i, 1
  %latch.bit = xor i1 %choice, false
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %exit.out = phi i1 [ %exit.bit.lcssa, %outer.latch ]
  %latch.out = phi i1 [ %latch.bit, %outer.latch ]
  %selected.out = and i1 %exit.out, %latch.out
  %value = select i1 %selected.out, double 1.000000e+00, double 0.000000e+00
  store double %value, ptr %R, align 8
  ret void
}

define void @inner_exit_freeze(ptr noalias %A, ptr noalias %R, i1 %choice) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double 1.000000e+00, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %inner.exit, label %inner.header

inner.exit:                                       ; preds = %inner.header
  %exit.bit = freeze i1 poison
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %exit.bit.lcssa = phi i1 [ %exit.bit, %inner.exit ]
  %i.next = add i64 %i, 1
  %latch.bit = xor i1 %choice, false
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %exit.out = phi i1 [ %exit.bit.lcssa, %outer.latch ]
  %latch.out = phi i1 [ %latch.bit, %outer.latch ]
  %selected.out = and i1 %exit.out, %latch.out
  %value = select i1 %selected.out, double 1.000000e+00, double 0.000000e+00
  store double %value, ptr %R, align 8
  ret void
}

define void @outer_latch_freeze(ptr noalias %A, ptr noalias %R, i1 %choice) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double 1.000000e+00, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, 4
  br i1 %j.ec, label %inner.exit, label %inner.header

inner.exit:                                       ; preds = %inner.header
  %exit.bit = xor i1 %choice, true
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %exit.bit.lcssa = phi i1 [ %exit.bit, %inner.exit ]
  %i.next = add i64 %i, 1
  %latch.bit = freeze i1 poison
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %exit.out = phi i1 [ %exit.bit.lcssa, %outer.latch ]
  %latch.out = phi i1 [ %latch.bit, %outer.latch ]
  %selected.out = and i1 %exit.out, %latch.out
  %value = select i1 %selected.out, double 1.000000e+00, double 0.000000e+00
  store double %value, ptr %R, align 8
  ret void
}

define void @inner_latch_clone_control(ptr noalias %A, ptr noalias %R, i1 %condition.arg, i1 %step.arg) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %k = phi i64 [ 0, %outer.header ], [ %k.next, %inner.header ]
  %condition.choice = xor i1 %condition.arg, true
  %step.choice = xor i1 %step.arg, true
  %condition.ext = zext i1 %condition.choice to i64
  %condition.zero = mul i64 %condition.ext, 0
  %bound = add i64 4, %condition.zero
  %step.ext = zext i1 %step.choice to i64
  %step.zero = mul i64 %step.ext, 0
  %step = add i64 1, %step.zero
  %k.next = add i64 %k, %step
  %k.fp = sitofp i64 %k to double
  %selected.bit = and i1 %condition.choice, %step.choice
  %selected = select i1 %selected.bit, double 1.000000e+00, double 0.000000e+00
  %value = fadd double %selected, %k.fp
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double %value, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, %bound
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %condition.lcssa = phi i1 [ %condition.choice, %inner.header ]
  %step.lcssa = phi i1 [ %step.choice, %inner.header ]
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %condition.out = phi i1 [ %condition.lcssa, %outer.latch ]
  %step.out = phi i1 [ %step.lcssa, %outer.latch ]
  %selected.out = and i1 %condition.out, %step.out
  %result = select i1 %selected.out, double 2.000000e+00, double 3.000000e+00
  store double %result, ptr %R, align 8
  ret void
}

define void @inner_latch_condition_freeze(ptr noalias %A, ptr noalias %R, i1 %step.arg) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %k = phi i64 [ 0, %outer.header ], [ %k.next, %inner.header ]
  %condition.choice = freeze i1 poison
  %step.choice = xor i1 %step.arg, true
  %condition.ext = zext i1 %condition.choice to i64
  %condition.zero = mul i64 %condition.ext, 0
  %bound = add i64 4, %condition.zero
  %step.ext = zext i1 %step.choice to i64
  %step.zero = mul i64 %step.ext, 0
  %step = add i64 1, %step.zero
  %k.next = add i64 %k, %step
  %k.fp = sitofp i64 %k to double
  %selected.bit = and i1 %condition.choice, %step.choice
  %selected = select i1 %selected.bit, double 1.000000e+00, double 0.000000e+00
  %value = fadd double %selected, %k.fp
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double %value, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, %bound
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %condition.lcssa = phi i1 [ %condition.choice, %inner.header ]
  %step.lcssa = phi i1 [ %step.choice, %inner.header ]
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %condition.out = phi i1 [ %condition.lcssa, %outer.latch ]
  %step.out = phi i1 [ %step.lcssa, %outer.latch ]
  %selected.out = and i1 %condition.out, %step.out
  %result = select i1 %selected.out, double 2.000000e+00, double 3.000000e+00
  store double %result, ptr %R, align 8
  ret void
}

define void @inner_latch_induction_freeze(ptr noalias %A, ptr noalias %R, i1 %condition.arg) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.header ]
  %k = phi i64 [ 0, %outer.header ], [ %k.next, %inner.header ]
  %condition.choice = xor i1 %condition.arg, true
  %step.choice = freeze i1 poison
  %condition.ext = zext i1 %condition.choice to i64
  %condition.zero = mul i64 %condition.ext, 0
  %bound = add i64 4, %condition.zero
  %step.ext = zext i1 %step.choice to i64
  %step.zero = mul i64 %step.ext, 0
  %step = add i64 1, %step.zero
  %k.next = add i64 %k, %step
  %k.fp = sitofp i64 %k to double
  %selected.bit = and i1 %condition.choice, %step.choice
  %selected = select i1 %selected.bit, double 1.000000e+00, double 0.000000e+00
  %value = fadd double %selected, %k.fp
  %idx = getelementptr inbounds [4 x double], ptr %A, i64 %j, i64 %i
  store double %value, ptr %idx, align 8
  %j.next = add i64 %j, 1
  %j.ec = icmp eq i64 %j.next, %bound
  br i1 %j.ec, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.header
  %condition.lcssa = phi i1 [ %condition.choice, %inner.header ]
  %step.lcssa = phi i1 [ %step.choice, %inner.header ]
  %i.next = add i64 %i, 1
  %i.ec = icmp eq i64 %i.next, 4
  br i1 %i.ec, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %condition.out = phi i1 [ %condition.lcssa, %outer.latch ]
  %step.out = phi i1 [ %step.lcssa, %outer.latch ]
  %selected.out = and i1 %condition.out, %step.out
  %result = select i1 %selected.out, double 2.000000e+00, double 3.000000e+00
  store double %result, ptr %R, align 8
  ret void
}
