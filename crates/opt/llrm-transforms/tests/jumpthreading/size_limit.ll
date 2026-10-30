; getJumpThreadDuplicationCost: the block holds more than 6 instructions, so it is not copied.
define i32 @f(i1 %x, i1 %y, i32 %a) {
entry:
  br i1 %x, label %p1, label %p2

p1:
  call void @sink(i32 1)
  br label %join

p2:
  call void @sink(i32 2)
  br label %join

join:
  %c = phi i1 [ true, %p1 ], [ %y, %p2 ]
  %v1 = add i32 %a, 1
  %v2 = mul i32 %v1, 3
  %v3 = add i32 %v2, %a
  %v4 = mul i32 %v3, 5
  %v5 = add i32 %v4, %v1
  %v6 = mul i32 %v5, 7
  %v7 = add i32 %v6, %v2
  call void @sink(i32 %v7)
  br i1 %c, label %t, label %e

t:
  ret i32 %a

e:
  ret i32 %v7
}

declare void @sink(i32)
