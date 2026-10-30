define i32 @and_cond(i32 %x) {
entry:
  %a = icmp ugt i32 %x, 5
  %b = icmp ult i32 %x, 50
  %c = and i1 %a, %b
  br i1 %c, label %in, label %out
in:
  %r = add i32 %x, 1
  ret i32 %r
out:
  %r2 = add i32 %x, 2
  ret i32 %r2
}

define i32 @or_cond(i32 %x) {
entry:
  %a = icmp ult i32 %x, 5
  %b = icmp ugt i32 %x, 50
  %c = or i1 %a, %b
  br i1 %c, label %out, label %in
out:
  %r = add i32 %x, 1
  ret i32 %r
in:
  %r2 = add i32 %x, 2
  ret i32 %r2
}

define i32 @logical(i32 %x) {
entry:
  %a = icmp sgt i32 %x, 0
  %b = icmp slt i32 %x, 100
  %c = select i1 %a, i1 %b, i1 false
  br i1 %c, label %in, label %out
in:
  %r = add i32 %x, 1
  ret i32 %r
out:
  %d = select i1 %a, i1 true, i1 %b
  br i1 %d, label %p, label %q
p:
  ret i32 %x
q:
  ret i32 0
}

define i32 @not_cond(i32 %x) {
entry:
  %a = icmp ult i32 %x, 7
  %n = xor i1 %a, true
  br i1 %n, label %ge, label %lt
ge:
  %r = add i32 %x, 1
  ret i32 %r
lt:
  %r2 = add i32 %x, 2
  ret i32 %r2
}

define i1 @cond_itself(i32 %x) {
entry:
  %a = icmp ult i32 %x, 7
  br i1 %a, label %t, label %f
t:
  %r = and i1 %a, true
  ret i1 %r
f:
  %s = zext i1 %a to i32
  %q = icmp eq i32 %s, 0
  ret i1 %q
}

define i32 @trunc_cond(i32 %x) {
entry:
  %t = trunc i32 %x to i1
  br i1 %t, label %one, label %zero
one:
  %r = add i32 %x, 1
  ret i32 %r
zero:
  %r2 = add i32 %x, 2
  ret i32 %r2
}

define i32 @trunc_nuw_cond(i8 %x) {
entry:
  %t = trunc nuw i8 %x to i1
  br i1 %t, label %one, label %zero
one:
  %r = zext i8 %x to i32
  ret i32 %r
zero:
  %r2 = zext i8 %x to i32
  %r3 = add i32 %r2, 5
  ret i32 %r3
}

define i32 @user_of_edge(i8 %x) {
entry:
  %c = icmp eq i8 %x, 93
  %y = add i8 %x, 1
  br i1 %c, label %then, label %else
then:
  %z = zext i8 %y to i32
  ret i32 %z
else:
  ret i32 0
}

define i32 @cast_of_edge(i8 %x) {
entry:
  %c = icmp ult i8 %x, 20
  %y = zext i8 %x to i32
  %s = sext i8 %x to i32
  %t = trunc i8 %x to i4
  br i1 %c, label %then, label %else
then:
  %a = add i32 %y, %s
  %b = zext i4 %t to i32
  %c2 = add i32 %a, %b
  ret i32 %c2
else:
  ret i32 0
}
