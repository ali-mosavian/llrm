define i32 @diamond(i1 %c) {
entry:
  br i1 %c, label %a, label %b, !prof !0
a:
  br label %join
b:
  br label %join
join:
  ret i32 0
}

define i32 @nest(i32 %n) {
entry:
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %i1, %latch ]
  br label %inner
inner:
  %j = phi i32 [ 0, %outer ], [ %j1, %inner ]
  %j1 = add i32 %j, 1
  %c = icmp slt i32 %j1, %n
  br i1 %c, label %inner, label %latch, !prof !1
latch:
  %i1 = add i32 %i, 1
  %d = icmp slt i32 %i1, %n
  br i1 %d, label %outer, label %exit, !prof !2
exit:
  ret i32 %i1
}

!0 = !{!"branch_weights", i32 3, i32 1}
!1 = !{!"branch_weights", i32 15, i32 1}
!2 = !{!"branch_weights", i32 9, i32 1}
