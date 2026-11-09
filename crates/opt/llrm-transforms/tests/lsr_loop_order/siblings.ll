target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define i32 @f(ptr %a, ptr %b, i32 %n) {
entry:
  br label %outer

outer:
  %i = phi i32 [ 0, %entry ], [ %i.next, %outer.latch ]
  %s = phi i32 [ 0, %entry ], [ %s2, %outer.latch ]
  %ra = mul i32 %i, 40
  %rb = mul i32 %i, 56
  br label %l1

l1:
  %j = phi i32 [ 0, %outer ], [ %j.next, %l1 ]
  %t = phi i32 [ %s, %outer ], [ %t.next, %l1 ]
  %o1 = add i32 %ra, %j
  %p1 = getelementptr i8, ptr %a, i32 %o1
  %v1 = load i8, ptr %p1
  %z1 = zext i8 %v1 to i32
  %t.next = add i32 %t, %z1
  %j.next = add nuw nsw i32 %j, 1
  %c1 = icmp eq i32 %j.next, 10
  br i1 %c1, label %mid, label %l1

mid:
  br label %l2

l2:
  %k = phi i32 [ 0, %mid ], [ %k.next, %l2 ]
  %u = phi i32 [ %t.next, %mid ], [ %u.next, %l2 ]
  %o2 = add i32 %rb, %k
  %p2 = getelementptr i8, ptr %b, i32 %o2
  %v2 = load i8, ptr %p2
  %z2 = zext i8 %v2 to i32
  %u.next = add i32 %u, %z2
  %k.next = add nuw nsw i32 %k, 1
  %c2 = icmp eq i32 %k.next, 12
  br i1 %c2, label %outer.latch, label %l2

outer.latch:
  %s2 = phi i32 [ %u.next, %l2 ]
  %i.next = add nuw nsw i32 %i, 1
  %c3 = icmp eq i32 %i.next, %n
  br i1 %c3, label %exit, label %outer

exit:
  ret i32 %s2
}
