; processThreadableEdges: both predecessors decide the branch on the phi.
define i32 @f(i1 %x, i32 %a, i32 %b) {
entry:
  br i1 %x, label %p1, label %p2

p1:
  call void @sink(i32 1)
  br label %join

p2:
  call void @sink(i32 2)
  br label %join

join:
  %c = phi i1 [ true, %p1 ], [ false, %p2 ]
  br i1 %c, label %t, label %e

t:
  ret i32 %a

e:
  ret i32 %b
}

declare void @sink(i32)
