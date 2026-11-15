target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)
declare void @report(i32)

define void @f() {
  %outer = call ptr @llvm.stacksave.p0()
  %inner = call ptr @llvm.stacksave.p0()
  call void @llvm.stackrestore.p0(ptr %inner)
  call void @llvm.stackrestore.p0(ptr %outer)
  call void @report(i32 1)
  ret void
}
