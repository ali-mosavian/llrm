; The value is stored just above the load in its own block: the load is that value, with no predecessor asked.
define i32 @f(i1 %c, ptr %p, i32 %v) {
entry:
  br i1 %c, label %p1, label %p2

p1:
  call void @sink(i32 1)
  br label %join

p2:
  call void @sink(i32 2)
  br label %join

join:
  store i32 %v, ptr %p
  %x = load i32, ptr %p
  %z = icmp eq i32 %x, 0
  br i1 %z, label %a, label %b

a:
  ret i32 1

b:
  ret i32 %x
}

declare void @sink(i32) nounwind willreturn memory(none)
