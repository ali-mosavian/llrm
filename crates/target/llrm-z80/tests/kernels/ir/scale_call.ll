target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
@buf = global [100 x i16] zeroinitializer
define void @scale(i16 %k) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %n, %loop ]
  %p = getelementptr i16, ptr @buf, i16 %i
  %v = load i16, ptr %p
  %m = mul i16 %v, %k
  %a = add i16 %m, 7
  %x = xor i16 %a, %i
  store i16 %x, ptr %p
  %n = add i16 %i, 1
  %c = icmp ult i16 %n, 100
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
define void @caller(i16 %k) {
  call void @scale(i16 %k)
  ret void
}
