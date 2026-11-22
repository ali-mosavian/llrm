target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
@buf = global [20000 x i8] zeroinitializer
declare i32 @total()
define i32 @fill(i8 noundef %v) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  %p = getelementptr inbounds [20000 x i8], ptr @buf, i32 0, i32 %i
  store i8 %v, ptr %p
  %n = add nuw i32 %i, 1
  %c = icmp eq i32 %n, 20000
  br i1 %c, label %done, label %loop
done:
  %r = call i32 @total()
  ret i32 %r
}
