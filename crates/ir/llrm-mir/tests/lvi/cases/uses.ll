define i32 @select_use(i32 %x, i1 %p) {
entry:
  %c = icmp ult i32 %x, 10
  %s = select i1 %c, i32 %x, i32 0
  %t = select i1 %c, i32 0, i32 %x
  %u = add i32 %s, %t
  ret i32 %u
}

define i32 @chain(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  %a = add i32 %x, 1
  %b = mul i32 %a, 3
  %s = select i1 %c, i32 %b, i32 0
  ret i32 %s
}

define i32 @phi_use(i32 %x, i1 %p) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %a, label %b
a:
  br label %m
b:
  br label %m
m:
  %r = phi i32 [ %x, %a ], [ %x, %b ]
  ret i32 %r
}

define i32 @phi_use_cond(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %m, label %b
b:
  br label %m
m:
  %r = phi i32 [ %x, %entry ], [ 0, %b ]
  ret i32 %r
}

define i32 @div_not_speculatable(i32 %x, i32 %y) {
entry:
  %c = icmp ult i32 %x, 10
  %d = udiv i32 %x, %y
  %s = select i1 %c, i32 %d, i32 0
  ret i32 %s
}

define i32 @div_by_const(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  %d = udiv i32 %x, 3
  %s = select i1 %c, i32 %d, i32 0
  ret i32 %s
}

define i32 @two_uses(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  %a = add i32 %x, 1
  %b = mul i32 %a, 3
  %s = select i1 %c, i32 %b, i32 0
  %t = add i32 %a, 7
  %u = add i32 %s, %t
  ret i32 %u
}
