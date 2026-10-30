; The unknowns and the loops the fold oracle's expressions name:
; arguments 0-5 are i16 a, i16 b, i16 c, i32 d, i32 e, i64 f; loop 0 is %h0
; with %h1 inside it, loop 2 is %h2.
define void @f(i16 %a, i16 %b, i16 %c, i32 %d, i32 %e, i64 %f, i8 %g, i1 %p) {
entry:
  br label %h0
h0:
  br label %h1
h1:
  br i1 %p, label %h1, label %l0
l0:
  br i1 %p, label %h0, label %m
m:
  br label %h2
h2:
  br i1 %p, label %h2, label %exit
exit:
  ret void
}
