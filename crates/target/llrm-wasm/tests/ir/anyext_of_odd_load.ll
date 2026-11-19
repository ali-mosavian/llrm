target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@g = global i64 0, align 8
define void @f() {
  %l = load i24, ptr getelementptr inbounds nuw (i8, ptr @g, i32 5), align 1
  %z = zext i24 %l to i64
  %s = shl nuw i64 %z, 40
  %o = or disjoint i64 %s, 884479243276
  store i64 %o, ptr @g, align 8
  ret void
}
