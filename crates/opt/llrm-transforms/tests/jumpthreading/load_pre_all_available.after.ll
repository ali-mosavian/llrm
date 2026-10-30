define i32 @f(i1 %c, ptr %p, i32 %v) {
entry:
  br i1 %c, label %join.thread, label %join

join.thread:
  store i32 0, ptr %p
  call void @sink(i32 1)
  br label %a

join:
  store i32 %v, ptr %p
  call void @sink(i32 2)
  %z = icmp eq i32 %v, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %v
}

declare void @sink(i32) nounwind willreturn memory(none)
