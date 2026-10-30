define i32 @sel(i32 %x, i1 %c) {
entry:
  %s = select i1 %c, i32 undef, i32 %x
  %t = select i1 %c, i32 7, i32 undef
  %r = add i32 %s, %t
  ret i32 %r
}

define i32 @edge(i32 %x) {
entry:
  %c = icmp ne i32 %x, undef
  br i1 %c, label %yes, label %no
yes:
  %a = add i32 %x, 1
  ret i32 %a
no:
  %e = icmp eq i32 %x, undef
  br i1 %e, label %eq, label %out
eq:
  %b = add i32 %x, 2
  ret i32 %b
out:
  ret i32 0
}

define i32 @phi(i32 %x, i1 %c) {
entry:
  br i1 %c, label %a, label %b
a:
  br label %j
b:
  br label %j
j:
  %p = phi i32 [ undef, %a ], [ 5, %b ]
  %q = add i32 %p, 1
  ret i32 %q
}

define i32 @ops(i32 %x) {
entry:
  %a = add i32 undef, 1
  %b = and i32 %x, undef
  %c = trunc i32 undef to i16
  %d = zext i16 %c to i32
  %e = add i32 %a, %d
  ret i32 %e
}
