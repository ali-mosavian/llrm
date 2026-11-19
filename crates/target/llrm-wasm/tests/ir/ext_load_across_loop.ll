target triple = "wasm32-unknown-unknown"

declare void @g(i32)

define void @f(ptr %p, ptr %q, i32 %n) {
entry:
  %v = load i8, ptr %p, align 1
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  store volatile i32 %i, ptr %q, align 4
  %next = add i32 %i, 1
  %again = icmp ult i32 %next, %n
  br i1 %again, label %loop, label %exit

exit:
  %s = sext i8 %v to i32
  call void @g(i32 %s)
  ret void
}
