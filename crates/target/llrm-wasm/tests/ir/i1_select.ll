target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define i1 @both(i1 %c, i1 %d) {
  %s = select i1 %c, i1 %d, i1 false
  ret i1 %s
}

define i32 @both_widened(i1 %c, i1 %d) {
  %s = select i1 %c, i1 %d, i1 false
  %z = zext i1 %s to i32
  ret i32 %z
}
