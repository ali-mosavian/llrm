declare ptr @llvm.frameaddress.p0(i32)

define ptr @where() {
  %frame = call ptr @llvm.frameaddress.p0(i32 0)
  ret ptr %frame
}
