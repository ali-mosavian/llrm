target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @h(i32 %0) {
  %2 = shl i32 %0, 1
  %3 = or disjoint i32 %2, 1
  ret i32 %3
}
