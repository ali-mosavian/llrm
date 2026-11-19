define void @cpy(ptr %d, ptr %s, i32 %n) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)
  ret void
}
define void @cpyc(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 16, i1 false)
  ret void
}
define void @mov(ptr %d, ptr %s, i32 %n) {
  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)
  ret void
}
define void @set(ptr %d, i8 %v, i32 %n) {
  call void @llvm.memset.p0.i32(ptr %d, i8 %v, i32 %n, i1 false)
  ret void
}
define void @setc(ptr %d) {
  call void @llvm.memset.p0.i32(ptr %d, i8 0, i32 16, i1 false)
  ret void
}
define void @set16(ptr %d) {
  call void @llvm.memset.p0.i16(ptr %d, i8 0, i16 400, i1 false)
  ret void
}
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @cpy15(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 15, i1 false)
  ret void
}
define void @cpy100(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 100, i1 false)
  ret void
}
define void @set7(ptr %d, i8 %v) {
  call void @llvm.memset.p0.i32(ptr %d, i8 %v, i32 7, i1 false)
  ret void
}
define void @set3(ptr %d) {
  call void @llvm.memset.p0.i32(ptr %d, i8 -1, i32 3, i1 false)
  ret void
}
define void @mov8(ptr %d, ptr %s) {
  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 8, i1 false)
  ret void
}
define i32 @mem_phi(ptr %d, ptr %s, i32 %n) {
entry:
  %c = icmp ne i32 %n, 5
  br i1 %c, label %a, label %b
a:
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)
  br label %j
b:
  br label %j
j:
  %r = phi i32 [1, %a], [2, %b]
  ret i32 %r
}
define i32 @mem_loop(ptr %d, i32 %n) {
entry:
  br label %l
l:
  %i = phi i32 [0, %entry], [%i1, %l]
  call void @llvm.memset.p0.i32(ptr %d, i8 0, i32 %n, i1 false)
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, %n
  br i1 %c, label %l, label %out
out:
  ret i32 %i1
}
define void @mem_zero(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 0, i1 false)
  call void @llvm.memset.p0.i32(ptr %d, i8 1, i32 200, i1 false)
  ret void
}
