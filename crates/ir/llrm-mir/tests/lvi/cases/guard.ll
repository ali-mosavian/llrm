declare void @llvm.experimental.guard(i1, ...)

define i32 @guarded(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  call void (i1, ...) @llvm.experimental.guard(i1 %c)
  %a = add i32 %x, 1
  ret i32 %a
}
