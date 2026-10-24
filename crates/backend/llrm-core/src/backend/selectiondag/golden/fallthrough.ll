target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

define i32 @f(i32 %x, i32 %y) {
entry:
  %c = icmp slt i32 %x, %y
  br i1 %c, label %then, label %else

else:
  %a = add i32 %x, 1
  br label %done

then:
  %b = add i32 %y, 2
  br label %done

done:
  %r = phi i32 [ %a, %else ], [ %b, %then ]
  ret i32 %r
}

define i32 @g(i32 %x) {
entry:
  %c = icmp eq i32 %x, 0
  br i1 %c, label %zero, label %other

zero:
  ret i32 1

other:
  ret i32 2
}
