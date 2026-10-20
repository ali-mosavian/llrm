define i32 @f(i1 %c, i1 %d, ptr %p) {
entry:
  br i1 %c, label %join.thread, label %p2

join.thread:
  store i32 0, ptr %p
  br label %a

p2:
  call void @sink(i32 2)
  br i1 %d, label %join, label %out

join:
  %x.pr = load i32, ptr %p
  %z = icmp eq i32 %x.pr, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %x.pr

out:
  ret i32 0
}

declare void @sink(i32) nounwind willreturn memory(none)
