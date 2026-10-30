define i32 @f(i1 %c, ptr %p, float %v) {
entry:
  br i1 %c, label %p1, label %p2

p1:
  store float %v, ptr %p
  call void @sink(i32 1)
  %0 = bitcast float %v to i32
  br label %join

p2:
  call void @sink(i32 2)
  %x.pr = load i32, ptr %p
  br label %join

join:
  %x = phi i32 [ %x.pr, %p2 ], [ %0, %p1 ]
  %z = icmp eq i32 %x, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %x
}

declare void @sink(i32) nounwind willreturn memory(none)
