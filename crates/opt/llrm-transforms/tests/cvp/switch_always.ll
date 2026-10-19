; processSwitch: the one value the switch can take picks the case.
define i32 @f(i32 %x) {
entry:
  %c = icmp eq i32 %x, 7
  br i1 %c, label %sw, label %out

sw:
  switch i32 %x, label %dflt [
    i32 3, label %a
    i32 7, label %b
  ]

a:
  ret i32 1

b:
  ret i32 2

dflt:
  ret i32 3

out:
  ret i32 0
}
