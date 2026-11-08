target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
@bad = global i16 0, align 2
declare void @report(i16)
define void @g(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ %n, %entry ], [ %j, %body ]
  %b = load i1, ptr @bad, align 2
  br i1 %b, label %exit, label %body
body:
  call void @report(i16 %i)
  %j = add i16 %i, -1
  %c = icmp eq i16 %j, 0
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
define void @setbad() {
  store i16 1, ptr @bad, align 2
  ret void
}
