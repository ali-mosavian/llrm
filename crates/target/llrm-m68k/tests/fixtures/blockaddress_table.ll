target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k"

@t = global [3 x ptr] [ptr blockaddress(@f, %a), ptr blockaddress(@f, %b), ptr blockaddress(@f, %c)]

define i32 @f(i32 %i, ptr %p, ptr %q) {
entry:
  %s = getelementptr ptr, ptr @t, i32 %i
  %t = load ptr, ptr %s
  indirectbr ptr %t, [label %a, label %b, label %c]
a:
  %x = load i32, ptr %p
  %y = mul i32 %x, 7
  store i32 %y, ptr %q
  br label %j
b:
  %x1 = load i32, ptr %p
  %y1 = mul i32 %x1, 7
  store i32 %y1, ptr %q
  br label %j
c:
  %x2 = load i32, ptr %p
  %y2 = mul i32 %x2, 7
  store i32 %y2, ptr %q
  br label %j
j:
  ret i32 1
}
