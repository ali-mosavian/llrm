define range(i32 0, 12) i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 2
  br i1 %c, label %sw, label %out

sw:
  switch i32 %x, label %default.unreachable [
    i32 0, label %a
    i32 1, label %b
  ]

a:
  ret i32 10

b:
  ret i32 11

c5:
  ret i32 15

default.unreachable:
  unreachable

dflt:
  ret i32 99

out:
  ret i32 0
}
