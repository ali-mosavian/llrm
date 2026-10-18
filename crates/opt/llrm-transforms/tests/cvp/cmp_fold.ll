; constantFoldCmp: the branch condition decides a compare in the successor.
define i1 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %then, label %else

then:
  %d = icmp ult i32 %x, 20
  ret i1 %d

else:
  %e = icmp ult i32 %x, 5
  ret i1 %e
}
