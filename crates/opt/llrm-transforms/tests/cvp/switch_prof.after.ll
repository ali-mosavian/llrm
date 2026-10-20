define range(i32 0, 12) i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 2
  br i1 %c, label %sw, label %out

sw:
  switch i32 %x, label %default.unreachable [
    i32 0, label %a
    i32 1, label %b
  ], !prof !1

a:
  ret i32 10

b:
  ret i32 11

c9:
  ret i32 19

default.unreachable:
  unreachable

dflt:
  ret i32 99

out:
  ret i32 0
}

!0 = !{!"branch_weights", i32 1, i32 20, i32 30, i32 40}
!1 = !{!"branch_weights", i32 1, i32 20, i32 30}
