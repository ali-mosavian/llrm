; runImpl: a `range` return attribute only for an exact definition.
define weak i32 @w(i32 %x) {
entry:
  %a = and i32 %x, 15
  ret i32 %a
}

define i32 @e(i32 %x) {
entry:
  %a = and i32 %x, 15
  ret i32 %a
}

define i32 @n(i32 %x) noipa {
entry:
  %a = and i32 %x, 15
  ret i32 %a
}
