define i32 @f(i1 %c, ptr %a, ptr %b) {
entry:
  br i1 %c, label %t, label %e

t:
  store i32 0, ptr %a
  call void @sink(i32 1)
  ret i32 1

e:
  store i32 5, ptr %b
  call void @sink(i32 2)
  ret i32 5
}

declare void @sink(i32) nounwind willreturn memory(none)
