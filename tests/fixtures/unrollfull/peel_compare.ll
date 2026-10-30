define i32 @f(i32 %n, i32 %seed) {
b0:
  br label %head

head:
  %i = phi i32 [ 1, %b0 ], [ %next, %latch ]
  %t = phi i32 [ %seed, %b0 ], [ %u, %latch ]
  %add = add i32 %t, %i
  %big = icmp sgt i32 %i, 1
  br i1 %big, label %hot, label %latch

hot:
  %more = add i32 %add, 100
  br label %latch

latch:
  %u = phi i32 [ %add, %head ], [ %more, %hot ]
  %next = add nsw i32 %i, 1
  %go = icmp sle i32 %next, %n
  br i1 %go, label %head, label %out

out:
  %r = phi i32 [ %u, %latch ]
  ret i32 %r
}
