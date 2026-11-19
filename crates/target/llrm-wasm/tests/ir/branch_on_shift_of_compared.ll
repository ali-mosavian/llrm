target triple = "wasm32-unknown-unknown"

declare void @g(i32)

define void @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 8
  br i1 %c, label %small, label %out

small:
  %t = lshr i32 %x, 3
  call void @g(i32 %t)
  br label %out

out:
  ret void
}
