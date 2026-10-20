define i32 @f(i1 %c, ptr %p, i32 %v) {
entry:
  br i1 %c, label %p1, label %p2

p1:
  call void @sink(i32 1)
  br label %join

p2:
  call void @sink(i32 2)
  br label %join

join:
  store i32 %v, ptr %p
  %z = icmp eq i32 %v, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %v
}

declare void @sink(i32) nounwind willreturn memory(none)
