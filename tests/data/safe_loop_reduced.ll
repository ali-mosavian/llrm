target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define i16 @safe(ptr %p, ptr %np, ptr %row) {
b1:
  %n = load i16, ptr %np
  %m = add i16 %n, -1
  %neg = icmp slt i16 %m, 0
  br i1 %neg, label %done, label %pre

pre:
  %c = load i16, ptr %row
  %d = sub i16 0, %n
  br label %loop

next:
  %d1 = add i16 %d2, 1
  %n1 = add i16 %n2, -1
  %q1 = getelementptr i8, ptr %q, i16 2
  %end = icmp eq i16 %n1, 0
  br i1 %end, label %done, label %loop

loop:
  %q = phi ptr [ %q1, %next ], [ %p, %pre ]
  %n2 = phi i16 [ %n1, %next ], [ %n, %pre ]
  %d2 = phi i16 [ %d1, %next ], [ %d, %pre ]
  %v = load i16, ptr %q
  %t = sub i16 %v, %c
  %a = icmp eq i16 %v, %c
  %b = icmp eq i16 %n2, %t
  %ab = or i1 %a, %b
  %e = icmp eq i16 %d2, %t
  %hit = or i1 %e, %ab
  br i1 %hit, label %done, label %next

done:
  %r = phi i16 [ -1, %b1 ], [ 0, %loop ], [ -1, %next ]
  ret i16 %r
}
