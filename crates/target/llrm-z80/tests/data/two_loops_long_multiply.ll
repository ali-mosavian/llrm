target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
@buf = internal global [256 x i8] zeroinitializer
declare i32 @bump(i16)
declare void @report(i32)
define i16 @main() optsize {
entry:
  br label %a
a:
  %k = phi i16 [ 0, %entry ], [ %k1, %a ]
  %s = phi i32 [ 0, %entry ], [ %s2, %a ]
  %m = mul i32 %s, 31
  %b = call i32 @bump(i16 %k)
  %s2 = add i32 %m, %b
  %k1 = add i16 %k, 1
  %c = icmp ult i16 %k1, 24
  br i1 %c, label %a, label %mid
mid:
  br label %b2
b2:
  %j = phi i16 [ 0, %mid ], [ %j1, %b2 ]
  %t = phi i32 [ %s2, %mid ], [ %t2, %b2 ]
  %m7 = mul i32 %t, 7
  %p = getelementptr [256 x i8], ptr @buf, i16 0, i16 %j
  %v = load i8, ptr %p
  %ve = sext i8 %v to i32
  %t2 = add i32 %m7, %ve
  %j1 = add i16 %j, 1
  %d = icmp ult i16 %j1, 256
  br i1 %d, label %b2, label %done
done:
  call void @report(i32 %t2)
  ret i16 0
}
