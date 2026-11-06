target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(i32 %x, i32 %lo, i32 %hi) {
  %a = icmp slt i32 %x, %lo
  %b = select i1 %a, i32 %lo, i32 %x
  %c = icmp sgt i32 %b, %hi
  %r = select i1 %c, i32 %hi, i32 %b
  ret i32 %r
}
