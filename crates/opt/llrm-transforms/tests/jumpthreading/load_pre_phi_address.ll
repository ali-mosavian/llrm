; The address is a phi of the block: each predecessor is asked about its own input, and both stored it.
define i32 @f(i1 %c, ptr %a, ptr %b) {
entry:
  br i1 %c, label %p1, label %p2

p1:
  store i32 0, ptr %a
  call void @sink(i32 1)
  br label %join

p2:
  store i32 5, ptr %b
  call void @sink(i32 2)
  br label %join

join:
  %p = phi ptr [ %a, %p1 ], [ %b, %p2 ]
  %x = load i32, ptr %p
  %z = icmp eq i32 %x, 0
  br i1 %z, label %t, label %e

t:
  ret i32 1

e:
  ret i32 %x
}

declare void @sink(i32) nounwind willreturn memory(none)
