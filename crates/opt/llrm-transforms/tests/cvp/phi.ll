; processPHI: a select incoming value is looked through on its edge; a phi
; whose constants match the edge values of one variable is that variable.
define i32 @f(i32 %x, i32 %y) {
entry:
  %isz = icmp eq i32 %x, 0
  br i1 %isz, label %join, label %body

body:
  br label %join

join:
  %r = phi i32 [ %x, %body ], [ 0, %entry ]
  %q = add i32 %r, %y
  ret i32 %q
}
