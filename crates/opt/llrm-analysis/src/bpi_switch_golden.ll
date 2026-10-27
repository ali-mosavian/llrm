declare void @cold_fn() cold
declare void @abort_fn() noreturn
define i32 @sw(i32 %x) {
entry:
  switch i32 %x, label %dflt [ i32 0, label %a
                               i32 1, label %b
                               i32 2, label %c
                               i32 3, label %d ]
a:
  ret i32 10
b:
  call void @cold_fn()
  ret i32 11
c:
  call void @abort_fn()
  unreachable
d:
  unreachable
dflt:
  ret i32 99
}
define i32 @sw2(i32 %x) {
entry:
  switch i32 %x, label %dflt [ i32 0, label %a
                               i32 1, label %b
                               i32 2, label %a ], !prof !0
a:
  ret i32 1
b:
  ret i32 2
dflt:
  ret i32 3
}
define i32 @sw3(i32 %x) {
entry:
  switch i32 %x, label %dflt [ i32 0, label %a
                               i32 1, label %b ]
a:
  ret i32 1
b:
  ret i32 2
dflt:
  unreachable
}
!0 = !{!"branch_weights", i32 5, i32 10, i32 20, i32 30}
