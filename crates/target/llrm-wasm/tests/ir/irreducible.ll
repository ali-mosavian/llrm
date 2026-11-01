define i32 @two_entries(i32 %c, i32 %n) {
entry:
  %t = icmp eq i32 %c, 0
  br i1 %t, label %a, label %b
a:
  %pa = phi i32 [ 0, %entry ], [ %nb, %b ]
  %na = add i32 %pa, 1
  %ca = icmp slt i32 %na, %n
  br i1 %ca, label %b, label %exit
b:
  %pb = phi i32 [ 5, %entry ], [ %na, %a ]
  %nb = mul i32 %pb, 3
  %cb = icmp slt i32 %nb, %n
  br i1 %cb, label %a, label %exit
exit:
  %r = phi i32 [ %na, %a ], [ %nb, %b ]
  ret i32 %r
}

define i32 @nested(i32 %c, i32 %n) {
entry:
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %i2, %latch ]
  %t = icmp eq i32 %c, 0
  br i1 %t, label %a, label %b
a:
  %pa = phi i32 [ 0, %outer ], [ %nb, %b ]
  %na = add i32 %pa, %i
  %ca = icmp slt i32 %na, %n
  br i1 %ca, label %b, label %latch
b:
  %pb = phi i32 [ 5, %outer ], [ %na, %a ]
  %nb = mul i32 %pb, 3
  %cb = icmp slt i32 %nb, %n
  br i1 %cb, label %a, label %latch
latch:
  %v = phi i32 [ %na, %a ], [ %nb, %b ]
  %i2 = add i32 %i, 1
  %done = icmp sgt i32 %i2, %v
  br i1 %done, label %exit, label %outer
exit:
  ret i32 %i2
}
