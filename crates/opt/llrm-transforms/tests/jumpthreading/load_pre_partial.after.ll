define i32 @f(i1 %c, ptr %p) {
entry:
  br i1 %c, label %join.thread, label %join

join.thread:
  store i32 0, ptr %p
  call void @sink(i32 1)
  br label %a

join:
  call void @sink(i32 2)
  %x.pr = load i32, ptr %p
  %z = icmp eq i32 %x.pr, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %x.pr
}

declare void @sink(i32) nounwind willreturn memory(none)
