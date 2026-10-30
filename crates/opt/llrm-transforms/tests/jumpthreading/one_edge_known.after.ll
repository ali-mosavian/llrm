define i32 @f(i1 %x, i1 %y, i32 %a, i32 %b) {
entry:
  br i1 %x, label %join.thread, label %join

join.thread:
  call void @sink(i32 1)
  br label %t

join:
  call void @sink(i32 2)
  br i1 %y, label %t, label %e

t:
  ret i32 %a

e:
  ret i32 %b
}

declare void @sink(i32)
