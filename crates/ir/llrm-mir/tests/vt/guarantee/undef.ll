define i32 @f(i32 %x, i1 %c) {
  %a = add i32 undef, 1
  %b = freeze i32 undef
  %d = select i1 %c, i32 undef, i32 %x
  %e = add i32 %b, 1
  ret i32 %a
}

define i32 @g(i1 %c) {
entry:
  br i1 %c, label %a, label %j
a:
  br label %j
j:
  %p = phi i32 [ undef, %entry ], [ 3, %a ]
  ret i32 %p
}
