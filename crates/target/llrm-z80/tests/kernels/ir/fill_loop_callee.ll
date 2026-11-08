target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
@b = global [64 x i32] zeroinitializer
define void @fill(i16 %n, i32 %v) {
entry:
  %c = icmp sgt i16 %n, 0
  br i1 %c, label %loop, label %done
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds [64 x i32], ptr @b, i16 0, i16 %i
  store i32 %v, ptr %p, align 1
  %next = add nuw nsw i16 %i, 1
  %again = icmp eq i16 %next, %n
  br i1 %again, label %done, label %loop
done:
  ret void
}
define void @caller(i16 %n) {
  call void @fill(i16 %n, i32 305419896)
  ret void
}
