define i32 @f(i32 %a) {
entry:
  %c = icmp ne i32 %a, 0
  br i1 %c, label %test, label %test

test:
  br i1 %c, label %yes, label %no

yes:
  ret i32 1

no:
  ret i32 0
}
