target triple = "wasm32-unknown-unknown"

declare i1 @llvm.is.constant.i32(i32)

define i32 @variable(i32 %x) {
  %c = call i1 @llvm.is.constant.i32(i32 %x)
  %r = zext i1 %c to i32
  ret i32 %r
}

define i32 @literal() {
  %c = call i1 @llvm.is.constant.i32(i32 5)
  %r = zext i1 %c to i32
  ret i32 %r
}
