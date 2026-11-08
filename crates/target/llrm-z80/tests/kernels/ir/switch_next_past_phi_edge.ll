target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare i16 @next()
define i16 @h() optsize {
entry:
  %dv = alloca i16, align 2
  store volatile i16 7, ptr %dv, align 2
  %n6 = call i16 @next()
  br label %loop
loop:
  %acc = phi i16 [ %n6, %entry ], [ %r, %b20 ]
  %n7 = call i16 @next()
  %m8 = and i16 %n7, 7
  switch i16 %m8, label %b20 [ i16 3, label %b18 i16 2, label %b16 i16 1, label %b14 ]
b14:
  br label %b20
b16:
  %s17 = lshr i16 %n6, 8
  br label %b20
b18:
  %s19 = lshr i16 %n6, 13
  br label %b20
b20:
  %p21 = phi i16 [ %n6, %b14 ], [ %s17, %b16 ], [ %s19, %b18 ], [ %acc, %loop ]
  %n23 = call i16 @next()
  %v = load volatile i16, ptr %dv, align 2
  %q = udiv i16 %p21, %v
  %r = add i16 %q, %n23
  %c = icmp eq i16 %r, 100
  br i1 %c, label %done, label %loop
done:
  ret i16 %r
}
