target triple = "wasm32-unknown-unknown"

declare void @g(i32)

define void @f(i32 %a, i32 %b) {
entry:
  %lt = icmp ult i32 %b, %a
  br i1 %lt, label %then, label %out

then:
  %d = sub i32 %a, %b
  call void @g(i32 %d)
  br label %out

out:
  ret void
}
