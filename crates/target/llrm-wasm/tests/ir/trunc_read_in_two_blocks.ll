target triple = "wasm32-unknown-unknown"

declare void @h(i8 zeroext)
declare i32 @next()

define void @f(i1 %c) {
entry:
  %x = call i32 @next()
  %t = trunc i32 %x to i8
  call void @h(i8 zeroext %t)
  br i1 %c, label %a, label %out

a:
  call void @h(i8 zeroext %t)
  br label %out

out:
  ret void
}
