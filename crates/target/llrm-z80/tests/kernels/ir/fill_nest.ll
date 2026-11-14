target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
@t = global [64 x [64 x i8]] zeroinitializer

; `for y < 64: for x < 64: t[y][x] = 65`: 4096 bytes, one fill.
define void @g() {
entry:
  br label %outer
outer:
  %y = phi i16 [ 0, %entry ], [ %y1, %next ]
  br label %inner
inner:
  %x = phi i16 [ 0, %outer ], [ %x1, %inner ]
  %p = getelementptr inbounds [64 x [64 x i8]], ptr @t, i16 0, i16 %y, i16 %x
  store i8 65, ptr %p, align 1
  %x1 = add nuw nsw i16 %x, 1
  %xc = icmp ult i16 %x1, 64
  br i1 %xc, label %inner, label %next
next:
  %y1 = add nuw nsw i16 %y, 1
  %yc = icmp ult i16 %y1, 64
  br i1 %yc, label %outer, label %done
done:
  ret void
}
