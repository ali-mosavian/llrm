; invalid: whose definition does not dominate it
define i16 @f(i1 %c) {
entry:
  br i1 %c, label %yes, label %join
yes:
  %x = add i16 1, 2
  br label %join
join:
  ret i16 %x
}
