target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare i32 @next()
define i32 @g() optsize {
entry:
  %dv = alloca i32, align 2
  store volatile i32 7, ptr %dv, align 2
  %n6 = call i32 @next()
  %n7 = call i32 @next()
  %m8 = and i32 %n7, 7
  %c9 = icmp eq i32 %m8, 3
  br i1 %c9, label %b18, label %b10
b10:
  %c11 = icmp eq i32 %m8, 2
  br i1 %c11, label %b16, label %b12
b12:
  %c13 = icmp eq i32 %m8, 1
  br i1 %c13, label %b14, label %b20
b14:
  %s15 = lshr i32 %n6, 16
  br label %b20
b16:
  %s17 = lshr i32 %n6, 24
  br label %b20
b18:
  %s19 = lshr i32 %n6, 29
  br label %b20
b20:
  %p21 = phi i32 [ %s15, %b14 ], [ %s17, %b16 ], [ %s19, %b18 ], [ %n6, %b12 ]
  %n23 = call i32 @next()
  %v = load volatile i32, ptr %dv, align 2
  %q = udiv i32 %p21, %v
  %r = add i32 %q, %n23
  ret i32 %r
}
