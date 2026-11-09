target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

; the borrow of the low words is `0 - 0`: false, folded
define i32 @sign_branch(i32 %m, i32 %a) {
  %t = and i32 %a, -2147483648
  %c = icmp ne i32 %t, 0
  br i1 %c, label %y, label %n
y:
  %s = add i32 %m, 1
  br label %j
n:
  br label %j
j:
  %p = phi i32 [ %s, %y ], [ %m, %n ]
  ret i32 %p
}

