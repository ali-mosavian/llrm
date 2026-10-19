define i32 @f(i1 %x, i32 %a, i32 %b) {
entry:
  br i1 %x, label %t, label %e

t:
  call void @sink(i32 1)
  ret i32 %a

e:
  call void @sink(i32 2)
  ret i32 %b
}

declare void @sink(i32)
