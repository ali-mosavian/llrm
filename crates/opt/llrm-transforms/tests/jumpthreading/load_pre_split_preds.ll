; Two predecessors lack the value: they are split off to one block, which holds the one inserted load.
define i32 @f(i32 %s, ptr %p) {
entry:
  switch i32 %s, label %p3 [ i32 0, label %p1
                             i32 1, label %p2 ]

p1:
  store i32 0, ptr %p
  call void @sink(i32 1)
  br label %join

p2:
  call void @sink(i32 2)
  br label %join

p3:
  call void @sink(i32 3)
  br label %join

join:
  %x = load i32, ptr %p
  %z = icmp eq i32 %x, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %x
}

declare void @sink(i32) nounwind willreturn memory(none)
