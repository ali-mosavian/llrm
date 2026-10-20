; The load is available in both predecessors: no load is inserted, the phi holds the two stored values.
define i32 @f(i1 %c, ptr %p, i32 %v) {
entry:
  br i1 %c, label %p1, label %p2

p1:
  store i32 0, ptr %p
  call void @sink(i32 1)
  br label %join

p2:
  store i32 %v, ptr %p
  call void @sink(i32 2)
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
