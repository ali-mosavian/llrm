define i32 @f(i1 %c, ptr %p, ptr %q) {
entry:
  br i1 %c, label %p1, label %p2

p1:
  store i32 0, ptr %p
  store i32 7, ptr %q
  br label %join

p2:
  store i32 1, ptr %p
  store i32 8, ptr %q
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
