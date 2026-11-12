target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

declare void @report(i32)

define void @products(ptr %v) {
entry:
  br label %outer

outer:
  %i = phi i16 [ 0, %entry ], [ %i1, %outer.next ]
  %pi = getelementptr i16, ptr %v, i16 %i
  %a = load i16, ptr %pi
  %x = zext i16 %a to i32
  %sq = mul i32 %x, %x
  call void @report(i32 %sq)
  br label %inner

inner:
  %j = phi i16 [ 0, %outer ], [ %j1, %inner ]
  %pj = getelementptr i16, ptr %v, i16 %j
  %b = load i16, ptr %pj
  %y = zext i16 %b to i32
  %m = mul i32 %x, %y
  call void @report(i32 %m)
  %j1 = add i16 %j, 1
  %dj = icmp eq i16 %j1, 10
  br i1 %dj, label %outer.next, label %inner

outer.next:
  %i1 = add i16 %i, 1
  %di = icmp eq i16 %i1, 10
  br i1 %di, label %exit, label %outer

exit:
  ret void
}
