define i32 @hot(i32 %a, i16 %n) {
 entry:
  br label %l1
 l1:
  %i = phi i16 [ 0, %entry ], [ %i1, %l1e ]
  %acc = phi i32 [ %a, %entry ], [ %acc1, %l1e ]
  br label %l2
 l2:
  %j = phi i16 [ 0, %l1 ], [ %j1, %l2e ]
  %acc_j = phi i32 [ %acc, %l1 ], [ %acc2, %l2e ]
  br label %l3
 l3:
  %k = phi i16 [ 0, %l2 ], [ %k1, %l3 ]
  %acc_k = phi i32 [ %acc_j, %l2 ], [ %m4, %l3 ]
  %m1 = mul i32 %acc_k, %a
  %m2 = mul i32 %m1, %acc_j
  %m3 = mul i32 %m2, %acc
  %m4 = add i32 %m3, %a
  %k1 = add i16 %k, 1
  %dk = icmp eq i16 %k1, %n
  br i1 %dk, label %l2e, label %l3
 l2e:
  %acc2 = add i32 %m4, 1
  %j1 = add i16 %j, 1
  %dj = icmp eq i16 %j1, %n
  br i1 %dj, label %l1e, label %l2
 l1e:
  %acc1 = add i32 %acc2, 1
  %i1 = add i16 %i, 1
  %di = icmp eq i16 %i1, %n
  br i1 %di, label %out, label %l1
 out:
  ret i32 %acc1
}
define i32 @step1(i32 %x) {
  %a = shl i32 %x, 13
  %b = lshr i32 %x, 7
  %c = shl i32 %x, 17
  %d = xor i32 %x, %a
  %e = xor i32 %d, %b
  %f = xor i32 %e, %c
  ret i32 %f
}
define i32 @step2(i32 %x) {
  %a = shl i32 %x, 13
  %b = lshr i32 %x, 7
  %c = shl i32 %x, 17
  %d = xor i32 %x, %a
  %e = xor i32 %d, %b
  %f = xor i32 %e, %c
  ret i32 %f
}
define i32 @step3(i32 %x) {
  %a = shl i32 %x, 13
  %b = lshr i32 %x, 7
  %c = shl i32 %x, 17
  %d = xor i32 %x, %a
  %e = xor i32 %d, %b
  %f = xor i32 %e, %c
  ret i32 %f
}
