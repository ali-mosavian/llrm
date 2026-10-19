declare void @llvm.assume(i1)

define i32 @ult(i32 %x) {
  %c = icmp ult i32 %x, 16
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @eq_const(i32 %x) {
  %c = icmp eq i32 %x, 77
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @eq_and(i32 %x) {
  %a = and i32 %x, 7
  %c = icmp eq i32 %a, 5
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @eq_or(i32 %x) {
  %a = or i32 %x, 240
  %c = icmp eq i32 %a, 245
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @eq_shl(i32 %x) {
  %a = shl i32 %x, 4
  %c = icmp eq i32 %a, 80
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @eq_lshr(i32 %x) {
  %a = lshr i32 %x, 4
  %c = icmp eq i32 %a, 3
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @ne_pow2(i32 %x) {
  %a = and i32 %x, 8
  %c = icmp ne i32 %a, 0
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @offset(i32 %x) {
  %a = add i32 %x, 3
  %c = icmp ult i32 %a, 10
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @ugt_and(i32 %x, i32 %y) {
  %a = and i32 %x, %y
  %c = icmp ugt i32 %a, -65537
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  %f_y = freeze i32 %y
  ret i32 %x
}

define i32 @ult_or(i32 %x, i32 %y) {
  %a = or i32 %x, %y
  %c = icmp ult i32 %a, 256
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  %f_y = freeze i32 %y
  ret i32 %y
}

define i32 @uge_nuw_sub(i32 %x, i32 %y) {
  %a = sub nuw i32 %x, %y
  %c = icmp uge i32 %a, -65536
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  %f_y = freeze i32 %y
  ret i32 %x
}

define i32 @ule_nuw_add(i32 %x, i32 %y) {
  %a = add nuw i32 %x, %y
  %c = icmp ule i32 %a, 1000
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  %f_y = freeze i32 %y
  ret i32 %y
}

define i1 @bool_true(i1 %b) {
  call void @llvm.assume(i1 %b)
  %f_b = freeze i1 %b
  ret i1 %b
}

define i1 @bool_false(i1 %b) {
  %n = xor i1 %b, true
  call void @llvm.assume(i1 %n)
  %f_b = freeze i1 %b
  ret i1 %b
}

define i32 @trunc_bit(i32 %x) {
  %t = trunc i32 %x to i1
  call void @llvm.assume(i1 %t)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @trunc_nuw_bit(i32 %x) {
  %t = trunc nuw i32 %x to i1
  call void @llvm.assume(i1 %t)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @trunc_cmp(i32 %x) {
  %t = trunc i32 %x to i8
  %c = icmp eq i8 %t, 5
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @trunc_nuw_cmp(i32 %x) {
  %t = trunc nuw i32 %x to i8
  %c = icmp ult i8 %t, 5
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define ptr @null_eq(ptr %p) {
  %c = icmp eq ptr %p, null
  call void @llvm.assume(i1 %c)
  %f_p = freeze ptr %p
  ret ptr %p
}

define i32 @conflict(i32 %x) {
  %c = icmp eq i32 %x, 1
  %d = icmp eq i32 %x, 2
  call void @llvm.assume(i1 %c)
  call void @llvm.assume(i1 %d)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @swapped(i32 %x) {
  %c = icmp ugt i32 16, %x
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @signed(i32 %x) {
  %c = icmp sgt i32 %x, -1
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @later_block(i32 %x) {
  br label %next
next:
  %c = icmp ult i32 %x, 100
  call void @llvm.assume(i1 %c)
  %f_x = freeze i32 %x
  ret i32 %x
}

define i32 @dominates(i32 %x, i1 %k) {
  %c = icmp ult i32 %x, 8
  call void @llvm.assume(i1 %c)
  br i1 %k, label %a, label %b
a:
  %f_a = freeze i32 %x
  ret i32 %f_a
b:
  %f_b = freeze i32 %x
  ret i32 %f_b
}

define i32 @not_dominating(i32 %x, i1 %k) {
  br i1 %k, label %a, label %b
a:
  %c = icmp ult i32 %x, 8
  call void @llvm.assume(i1 %c)
  br label %m
b:
  br label %m
m:
  %f_m = freeze i32 %x
  ret i32 %f_m
}
