; Two entries into one cycle: neither b nor c dominates the other, so the
; cycle is not a natural loop; the loop around it (outer) is.
define void @irreducible(i1 %p, i1 %q) {
entry:
  br label %outer
outer:
  br i1 %p, label %b, label %c
b:
  br i1 %q, label %c, label %latch
c:
  br i1 %q, label %b, label %latch
latch:
  br i1 %p, label %outer, label %exit
exit:
  ret void
}

; An irreducible cycle with no natural loop around it.
define void @bare(i1 %p) {
entry:
  br i1 %p, label %b, label %c
b:
  br label %c
c:
  br i1 %p, label %b, label %exit
exit:
  ret void
}
