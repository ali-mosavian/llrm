define i32 @f(i32 %s, ptr %p) {
entry:
  switch i32 %s, label %p3 [
    i32 0, label %join.thread
    i32 1, label %p2
  ]

join.thread:
  store i32 0, ptr %p
  call void @sink(i32 1)
  br label %a

p2:
  call void @sink(i32 2)
  br label %join

p3:
  call void @sink(i32 3)
  br label %join

join:
  %x.pr = load i32, ptr %p
  %z = icmp eq i32 %x.pr, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %x.pr
}

declare void @sink(i32) nounwind willreturn memory(none)
