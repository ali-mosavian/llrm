target triple = "wasm32-unknown-unknown"

declare void @g(i32)

define void @f(i32 %x, i32 %y, i1 %c) {
entry:
  %m = and i32 %x, %y
  %z1 = icmp eq i32 %m, 0
  br i1 %z1, label %a, label %b

a:
  call void @g(i32 1)
  br i1 %c, label %b, label %out

b:
  %z2 = icmp ne i32 %m, 0
  br i1 %z2, label %out, label %a

out:
  ret void
}
