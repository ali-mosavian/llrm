; processBranchOnXOR: the xor of a phi (true from %p1) and a compare is cloned into %p1 as the inverted compare.
define i32 @f(i32 %a, i32 %b, i1 %q, i1 %k) {
entry:
  br i1 %k, label %p1, label %p2

p1:
  call void @sink(i32 1)
  br label %bb

p2:
  call void @sink(i32 2)
  br label %bb

bb:
  %x = phi i1 [ true, %p1 ], [ %q, %p2 ]
  %y = icmp eq i32 %a, %b
  %z = xor i1 %x, %y
  br i1 %z, label %t, label %e

t:
  ret i32 1

e:
  ret i32 2
}

declare void @sink(i32)
