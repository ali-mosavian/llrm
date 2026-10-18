declare range(i8 3, 9) i8 @ranged()
declare i8 @plain()
@g = global i32 0

define void @args(i8 range(i8 10, 20) %x, i16 range(i16 -5, 5) %y, i32 %z) {
  %a = add i8 %x, 1
  ret void
}

define void @calls() {
  %a = call i8 @ranged()
  %b = call range(i8 5, 7) i8 @plain()
  %c = call range(i8 0, 6) i8 @ranged()
  %d = call i8 @plain(), !range !0
  %e = call i8 @plain(), !range !1
  %f = load i8, ptr @g, !range !0
  %h = load i8, ptr @g, !range !1
  ret void
}

define void @consts(i8 %x) {
  %a = add i8 3, 4
  ret void
}

!0 = !{i8 1, i8 5}
!1 = !{i8 1, i8 5, i8 20, i8 30}
