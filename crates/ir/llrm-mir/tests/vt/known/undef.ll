define i32 @mix(i32 %x) {
  %o = or i32 %x, 8
  %a = and i32 %o, undef
  %b = add i32 undef, 5
  %c = or i32 undef, 3
  ret i32 %c
}

define i32 @phi_undef(i1 %c, i32 %x) {
entry:
  br i1 %c, label %a, label %j
a:
  br label %j
j:
  %p = phi i32 [ undef, %entry ], [ undef, %a ]
  %q = phi i32 [ 12, %entry ], [ undef, %a ]
  %r = add i32 %p, %q
  ret i32 %r
}
