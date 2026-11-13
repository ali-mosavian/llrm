target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
@src = global [1200 x i8] zeroinitializer
@dst = global [1200 x i8] zeroinitializer

; `src[i] = i * 7; dst[i] = 0xEE` over 1200 trips: the constant store is one fill.
define void @g() {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %n, %loop ]
  %p = getelementptr inbounds [1200 x i8], ptr @src, i16 0, i16 %i
  %m = mul i16 %i, 7
  %t = trunc i16 %m to i8
  store i8 %t, ptr %p
  %q = getelementptr inbounds [1200 x i8], ptr @dst, i16 0, i16 %i
  store i8 -18, ptr %q
  %n = add nuw nsw i16 %i, 1
  %c = icmp eq i16 %n, 1200
  br i1 %c, label %done, label %loop
done:
  ret void
}
