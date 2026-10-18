; processSwitch: a case outside the range goes, and the default of a
; switch that covers the range is unreachable.
define i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 2
  br i1 %c, label %sw, label %out

sw:
  switch i32 %x, label %dflt [
    i32 0, label %a
    i32 1, label %b
    i32 5, label %c5
  ]

a:
  ret i32 10

b:
  ret i32 11

c5:
  ret i32 15

dflt:
  ret i32 99

out:
  ret i32 0
}
