define fp128 @sum(i32 %n, ptr %p) {
entry:
  br label %l
l:
  %i = phi i32 [0, %entry], [%j, %l]
  %a = phi fp128 [0xL00000000000000000000000000000000, %entry], [%b, %l]
  %x = load fp128, ptr %p, align 16
  %b = fadd fp128 %a, %x
  %j = add i32 %i, 1
  %c = icmp eq i32 %j, %n
  br i1 %c, label %e, label %l
e:
  ret fp128 %b
}
