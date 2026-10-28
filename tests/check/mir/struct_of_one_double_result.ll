; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; A function returning `struct D { double d; }` by value was refused ("a %struct.D value": the named struct was no struct to the size of
; a result) and, past that, "an aggregate result that is not one register a member". A struct of one float leaves in st(0) as the float does.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

%struct.D = type { double }

; CHECK-LABEL: _f@3 proc
; CHECK: fld qword ptr
; CHECK-NOT: eax
; CHECK: ret
define %struct.D @f(double %a) noinline {
  %m = fmul double %a, 2.000000e+00
  %r = insertvalue %struct.D poison, double %m, 0
  ret %struct.D %r
}

; CHECK-LABEL: _g@3 proc
; CHECK: call
; CHECK: fadd
define double @g(double %a) {
  %s = call %struct.D @f(double %a)
  %d = extractvalue %struct.D %s, 0
  %r = fadd double %d, 1.000000e+00
  ret double %r
}
