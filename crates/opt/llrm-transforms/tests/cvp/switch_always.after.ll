define range(i32 0, 3) i32 @f(i32 %x) {
entry:
  %c = icmp eq i32 %x, 7
  br i1 %c, label %sw, label %out

sw:
  br label %b

a:
  ret i32 1

b:
  ret i32 2

dflt:
  ret i32 3

out:
  ret i32 0
}
