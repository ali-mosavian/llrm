target triple = "wasm32-unknown-unknown"

define void @f(ptr %p, i32 %n) "target-features"="+multivalue,+sign-ext" {
entry:
  %a = load fp128, ptr %p
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = phi fp128 [ %a, %entry ], [ %sum, %loop ]
  %sum = fadd fp128 %s, %a
  %next = add i32 %i, 1
  %again = icmp ne i32 %next, %n
  br i1 %again, label %loop, label %exit

exit:
  store fp128 %sum, ptr %p
  ret void
}
