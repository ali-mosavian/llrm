; getPredicateOnEdge: x < 5 on the edge from %low proves %lt, which the entry computes, at the test.
define i32 @f(i32 %x) {
entry:
  %small = icmp ult i32 %x, 5
  %lt = icmp ult i32 %x, 10
  br i1 %small, label %low, label %other

low:
  call void @sink(i32 1)
  br label %test

other:
  call void @sink(i32 2)
  br label %test

test:
  br i1 %lt, label %a, label %b

a:
  ret i32 1

b:
  ret i32 2
}

declare void @sink(i32)
