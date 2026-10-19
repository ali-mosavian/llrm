; getSinglePredecessor counts edges: once the empty %skip is merged away, %entry reaches %test twice and %test has no single predecessor, so its branch on the same condition is not implied.
define i32 @f(i32 %a) {
entry:
  %c = icmp ne i32 %a, 0
  br i1 %c, label %skip, label %test

skip:
  br label %test

test:
  br i1 %c, label %yes, label %no

yes:
  ret i32 1

no:
  ret i32 0
}
