; A load tested by a switch is partially redundant as one tested by a branch is.
define i32 @f(i1 %c, ptr %p) {
entry:
  br i1 %c, label %p1, label %p2

p1:
  store i32 2, ptr %p
  call void @sink(i32 1)
  br label %join

p2:
  call void @sink(i32 2)
  br label %join

join:
  %x = load i32, ptr %p
  switch i32 %x, label %d [ i32 1, label %a
                            i32 2, label %b ]

a:
  ret i32 10

b:
  ret i32 20

d:
  ret i32 %x
}

declare void @sink(i32) nounwind willreturn memory(none)
