; both halves of a phi must be copied on the back edge
define i128 @swap(i32 %n, i128 %x, i128 %y) {
entry:
  br label %l
l:
  %i = phi i32 [0, %entry], [%j, %l]
  %a = phi i128 [%x, %entry], [%b, %l]
  %b = phi i128 [%y, %entry], [%a, %l]
  %j = add i32 %i, 1
  %c = icmp eq i32 %j, %n
  br i1 %c, label %e, label %l
e:
  ret i128 %a
}
