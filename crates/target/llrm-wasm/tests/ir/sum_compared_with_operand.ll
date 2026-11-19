target triple = "wasm32-unknown-unknown"

declare void @g(i32)

define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, %b
  %c = icmp ult i32 %s, %a
  br i1 %c, label %wrap, label %ok

wrap:
  call void @g(i32 %s)
  ret i32 0

ok:
  ret i32 %s
}
