target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
; tests/run/c/x87_arguments' `mix` after tail-recursion elimination: no header
; phi starts at an integer constant, so the pin runs the extra iterations first
; (a prolog) and the prolog's exit phis have a poison edge.
define double @run(double %a, float %b, i32 %n, double %c, float %d) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %done, label %loop
loop:
  %a.p = phi double [ %a1, %loop ], [ %a, %entry ]
  %b.p = phi float [ %b1, %loop ], [ %b, %entry ]
  %n.p = phi i32 [ %n1, %loop ], [ %n, %entry ]
  %c.p = phi double [ %c1, %loop ], [ %c, %entry ]
  %d.p = phi float [ %d1, %loop ], [ %d, %entry ]
  %m = fmul double %a.p, 5.0e-01
  %a1 = fadd double %m, 1.0
  %b1 = fadd float %b.p, 1.0
  %n1 = add nsw i32 %n.p, -1
  %c1 = fsub double %c.p, %a.p
  %d1 = fadd float %d.p, %d.p
  %e = icmp eq i32 %n1, 0
  br i1 %e, label %exit, label %loop
exit:
  br label %done
done:
  %af = phi double [ %a, %entry ], [ %a1, %exit ]
  %bf = phi float [ %b, %entry ], [ %b1, %exit ]
  %cf = phi double [ %c, %entry ], [ %c1, %exit ]
  %df = phi float [ %d, %entry ], [ %d1, %exit ]
  %bx = fpext float %bf to double
  %dx = fpext float %df to double
  %s0 = fadd double %af, %bx
  %s1 = fadd double %s0, %cf
  %s2 = fadd double %s1, %dx
  ret double %s2
}
