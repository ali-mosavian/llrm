; invalid: has no input from %no
define i16 @f(i1 %c) {
entry:
  br i1 %c, label %yes, label %no
yes:
  br label %join
no:
  br label %join
join:
  %x = phi i16 [ 1, %yes ]
  ret i16 %x
}
