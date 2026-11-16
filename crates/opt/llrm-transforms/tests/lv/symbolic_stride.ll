target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

; stft_single's butterfly: the twiddle index moves by a run-time stride.
define void @f(ptr noalias %re, ptr noalias %im, ptr noalias %twr, ptr noalias %twi, i64 %half, i64 %stride) {
entry:
  br label %loop
loop:
  %j = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pre = getelementptr inbounds float, ptr %re, i64 %j
  %pim = getelementptr inbounds float, ptr %im, i64 %j
  %ar = load float, ptr %pre, align 4
  %ai = load float, ptr %pim, align 4
  %jh = add i64 %j, %half
  %pbr = getelementptr inbounds float, ptr %re, i64 %jh
  %pbi = getelementptr inbounds float, ptr %im, i64 %jh
  %br = load float, ptr %pbr, align 4
  %bi = load float, ptr %pbi, align 4
  %at = mul i64 %j, %stride
  %pwr = getelementptr inbounds float, ptr %twr, i64 %at
  %pwi = getelementptr inbounds float, ptr %twi, i64 %at
  %wr = load float, ptr %pwr, align 4
  %wi = load float, ptr %pwi, align 4
  %t1 = fmul float %wr, %br
  %t2 = fmul float %wi, %bi
  %tr = fsub float %t1, %t2
  %t3 = fmul float %wr, %bi
  %t4 = fmul float %wi, %br
  %ti = fadd float %t3, %t4
  %s1 = fadd float %ar, %tr
  %o1 = fmul float %s1, 0.5
  %s2 = fadd float %ai, %ti
  %o2 = fmul float %s2, 0.5
  %d1 = fsub float %ar, %tr
  %o3 = fmul float %d1, 0.5
  %d2 = fsub float %ai, %ti
  %o4 = fmul float %d2, 0.5
  store float %o1, ptr %pre, align 4
  store float %o2, ptr %pim, align 4
  store float %o3, ptr %pbr, align 4
  store float %o4, ptr %pbi, align 4
  %next = add nuw nsw i64 %j, 1
  %done = icmp eq i64 %next, 128
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
