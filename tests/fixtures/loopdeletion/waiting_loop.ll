@g = global i16 7

define i16 @f(i16 %x) {
b0:
  br label %loop

loop:
  %v = load i16, ptr @g
  %go = icmp ne i16 %v, 7
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}

define i16 @h(i16 %x) mustprogress {
b0:
  br label %loop

loop:
  %v = load i16, ptr @g
  %go = icmp ne i16 %v, 7
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
