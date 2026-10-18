declare i32 @llvm.abs.i32(i32, i1)

define i32 @smax(i32 %x) {
entry:
  %c = icmp slt i32 %x, 10
  %s = select i1 %c, i32 10, i32 %x
  ret i32 %s
}

define i32 @umin(i32 %x, i32 %y) {
entry:
  %a = icmp ult i32 %x, 100
  br i1 %a, label %ok, label %out
ok:
  %b = icmp ult i32 %y, 50
  br i1 %b, label %ok2, label %out
ok2:
  %c = icmp ult i32 %x, %y
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
out:
  ret i32 0
}

define i32 @abs(i32 %x) {
entry:
  %a = icmp ult i32 %x, 20
  br i1 %a, label %ok, label %out
ok:
  %s = add i32 %x, -10
  %n = sub i32 0, %s
  %c = icmp sgt i32 %s, -1
  %r = select i1 %c, i32 %s, i32 %n
  ret i32 %r
out:
  ret i32 0
}

define i32 @nabs(i32 %x) {
entry:
  %a = icmp ult i32 %x, 20
  br i1 %a, label %ok, label %out
ok:
  %s = add i32 %x, -10
  %n = sub i32 0, %s
  %c = icmp sgt i32 %s, -1
  %r = select i1 %c, i32 %n, i32 %s
  ret i32 %r
out:
  ret i32 0
}

define i32 @cond_arms(i32 %x) {
entry:
  %c = icmp ult i32 %x, 7
  %s = select i1 %c, i32 %x, i32 7
  %t = add i32 %s, 1
  ret i32 %t
}

define i32 @binop_over_select(i1 %c, i32 %x) {
entry:
  %ok = icmp ult i32 %x, 1000
  br i1 %ok, label %b, label %out
b:
  %s = select i1 %c, i32 3, i32 5
  %m = mul i32 %x, %s
  %n = add i32 %s, %x
  ret i32 %m
out:
  ret i32 0
}

define i32 @intrinsic_abs(i32 %x) {
entry:
  %a = icmp ult i32 %x, 20
  br i1 %a, label %ok, label %out
ok:
  %s = add i32 %x, -10
  %r = call i32 @llvm.abs.i32(i32 %s, i1 false)
  ret i32 %r
out:
  ret i32 0
}
