; maybethreadThroughTwoBasicBlocks, the comment's example: %var is 0 from %bb1 and 7 from %bb2, so %bb1 can skip both %pred's test and %b's compare.
define i32 @f(i32 %cond, i1 %k) {
entry:
  br i1 %k, label %bb1, label %bb2

bb1:
  call void @sink(i32 1)
  br label %pred

bb2:
  call void @sink(i32 2)
  br label %pred

pred:
  %var = phi i32 [ 0, %bb1 ], [ 7, %bb2 ]
  %tobool = icmp eq i32 %cond, 0
  br i1 %tobool, label %b, label %other

b:
  %cmp = icmp eq i32 %var, 0
  br i1 %cmp, label %y, label %n

y:
  ret i32 1

n:
  ret i32 2

other:
  ret i32 3
}

declare void @sink(i32)
