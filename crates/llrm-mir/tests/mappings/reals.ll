; SINGLE and DOUBLE are float and double, with plain arithmetic.
; expect: 7
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

define i32 @main() {
  %f = fadd float 1.5, 2.0
  %d = fpext float %f to double
  %s = fadd double %d, 3.5
  %i = fptosi double %s to i32
  ret i32 %i
}
