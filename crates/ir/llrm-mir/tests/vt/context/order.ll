declare void @llvm.assume(i1)

define i32 @before_assume(i32 %x) {
  %f_before = freeze i32 %x
  %c = icmp ult i32 %x, 8
  call void @llvm.assume(i1 %c)
  ret i32 %f_before
}
