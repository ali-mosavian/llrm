target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@__main_void = hidden alias i32 (), ptr @main

declare i32 @g()
declare void @h(i32, i32)

define hidden i32 @main() {
entry:
  %a = call i32 (i32) @g(i32 7)
  call void @h(i32 %a, i32 1)
  %b = call i64 (i32) @h(i32 %a)
  ret i32 %a
}
