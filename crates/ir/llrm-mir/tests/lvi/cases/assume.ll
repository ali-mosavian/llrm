declare void @llvm.assume(i1)

define i32 @range(i32 %x) {
entry:
  %c = icmp ult i32 %x, 100
  call void @llvm.assume(i1 %c)
  %r = add i32 %x, 1
  ret i32 %r
}

define i32 @after(i32 %x) {
entry:
  %r = add i32 %x, 1
  %c = icmp ult i32 %x, 100
  call void @llvm.assume(i1 %c)
  %s = add i32 %x, 2
  ret i32 %s
}

define i32 @other_block(i32 %x, i1 %p) {
entry:
  %c = icmp ugt i32 %x, 7
  call void @llvm.assume(i1 %c)
  br i1 %p, label %a, label %b
a:
  %r = add i32 %x, 1
  ret i32 %r
b:
  %s = sub i32 %x, 7
  ret i32 %s
}

define i32 @conj(i32 %x) {
entry:
  %a = icmp ugt i32 %x, 7
  %b = icmp ult i32 %x, 30
  %c = and i1 %a, %b
  call void @llvm.assume(i1 %c)
  %r = add i32 %x, 1
  ret i32 %r
}

define i32 @eq(i32 %x) {
entry:
  %c = icmp eq i32 %x, 17
  call void @llvm.assume(i1 %c)
  %r = add i32 %x, 1
  ret i32 %r
}

define ptr @nonnull(ptr %p) {
entry:
  %c = icmp ne ptr %p, null
  call void @llvm.assume(i1 %c)
  ret ptr %p
}

define i32 @two(i32 %x, i32 %y) {
entry:
  %a = icmp ult i32 %x, %y
  call void @llvm.assume(i1 %a)
  %b = icmp ult i32 %y, 10
  call void @llvm.assume(i1 %b)
  %s = add i32 %x, %y
  ret i32 %s
}
