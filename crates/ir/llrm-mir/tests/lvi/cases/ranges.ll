declare i32 @opaque()
declare range(i32 5, 10) i32 @ranged()
declare i32 @plain(i32)

define i32 @load_range(ptr %p) {
entry:
  %v = load i32, ptr %p, !range !0
  %r = add i32 %v, 1
  ret i32 %r
}

define i32 @load_range2(ptr %p) {
entry:
  %v = load i32, ptr %p, !range !1
  %r = add i32 %v, 1
  ret i32 %r
}

define i32 @call_range() {
entry:
  %v = call i32 @ranged()
  %w = call range(i32 0, 3) i32 @opaque()
  %r = add i32 %v, %w
  ret i32 %r
}

define i32 @call_md() {
entry:
  %v = call i32 @opaque(), !range !0
  %r = mul i32 %v, 2
  ret i32 %r
}

define i32 @arg_range(i32 range(i32 3, 9) %x, i8 range(i8 -4, 4) %y) {
entry:
  %a = add i32 %x, 1
  %b = sext i8 %y to i32
  %c = add i32 %a, %b
  ret i32 %c
}

define i32 @arg_range_edge(i32 range(i32 0, 100) %x) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %lo, label %hi
lo:
  %a = add i32 %x, 1
  ret i32 %a
hi:
  %b = sub i32 %x, 10
  ret i32 %b
}

!0 = !{i32 3, i32 8}
!1 = !{i32 3, i32 8, i32 20, i32 30}
