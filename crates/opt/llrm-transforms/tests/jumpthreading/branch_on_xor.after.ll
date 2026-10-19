define i32 @f(i32 %a, i32 %b, i1 %q, i1 %k) {
entry:
  br i1 %k, label %p1, label %bb

p1:
  call void @sink(i32 1)
  %y1 = icmp eq i32 %a, %b
  %z1 = xor i1 true, %y1
  br i1 %z1, label %t, label %e

bb:
  call void @sink(i32 2)
  %y = icmp eq i32 %a, %b
  %z = xor i1 %q, %y
  br i1 %z, label %t, label %e

t:
  ret i32 1

e:
  ret i32 2
}

declare void @sink(i32)
