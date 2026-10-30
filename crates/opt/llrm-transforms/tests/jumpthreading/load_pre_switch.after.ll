define i32 @f(i1 %c, ptr %p) {
entry:
  br i1 %c, label %join.thread, label %join

join.thread:
  store i32 2, ptr %p
  call void @sink(i32 1)
  br label %b

join:
  call void @sink(i32 2)
  %x.pr = load i32, ptr %p
  switch i32 %x.pr, label %d [
    i32 1, label %a
    i32 2, label %b
  ]

a:
  ret i32 10

b:
  ret i32 20

d:
  ret i32 %x.pr
}

declare void @sink(i32) nounwind willreturn memory(none)
