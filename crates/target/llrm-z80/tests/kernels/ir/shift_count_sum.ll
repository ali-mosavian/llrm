target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i32 @g(i32 %val, i32 %start, i16 %off) {
entry:
  br label %l
l:
  %iv = phi i32 [ %start, %entry ], [ %n, %l ]
  %o = sext i16 %off to i32
  %s = add nsw i32 %iv, %o
  %v = lshr i32 %val, %s
  %c = icmp ne i32 %v, 0
  %n = add nsw i32 %iv, 1
  br i1 %c, label %l, label %e
e:
  ret i32 %iv
}
