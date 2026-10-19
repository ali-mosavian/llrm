define i32 @f(i32 %x) {
entry:
  %small = icmp ult i32 %x, 5
  %lt = icmp ult i32 %x, 10
  br i1 %small, label %test.thread, label %test

test.thread:
  call void @sink(i32 1)
  br label %a

test:
  call void @sink(i32 2)
  br i1 %lt, label %a, label %b

a:
  ret i32 1

b:
  ret i32 2
}

declare void @sink(i32)
