define i32 @f(i32 %cond, i1 %k) {
entry:
  br i1 %k, label %pred, label %pred.thread

pred:
  call void @sink(i32 1)
  %tobool = icmp eq i32 %cond, 0
  br i1 %tobool, label %y, label %other

pred.thread:
  call void @sink(i32 2)
  %tobool1 = icmp eq i32 %cond, 0
  br i1 %tobool1, label %n, label %other

y:
  %var2 = phi i32 [ 0, %pred ]
  ret i32 1

n:
  ret i32 2

other:
  ret i32 3
}

declare void @sink(i32)
