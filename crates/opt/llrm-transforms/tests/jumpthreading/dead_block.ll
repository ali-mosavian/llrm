; runImpl: the test is folded away and %dead, with no predecessor left, is deleted.
define i32 @f(i32 %a) {
entry:
  br i1 true, label %live, label %dead

live:
  ret i32 %a

dead:
  %v = add i32 %a, 1
  br label %live2

live2:
  ret i32 %v
}
