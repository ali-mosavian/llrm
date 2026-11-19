target triple = "wasm32-unknown-unknown"

define void @f(ptr %p, float %f, i32 %i) {
  %b = bitcast float %f to i32
  %lz = zext i32 %b to i64
  %hz = zext i32 %i to i64
  %hs = shl i64 %hz, 32
  %o = or i64 %lz, %hs
  store i64 %o, ptr %p, align 8
  ret void
}
