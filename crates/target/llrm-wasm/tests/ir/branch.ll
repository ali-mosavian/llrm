declare void @side()
define i32 @diamond(i32 %a, i32 %b) {
entry:
  %c = icmp slt i32 %a, %b
  br i1 %c, label %then, label %else
then:
  %x = add i32 %a, 1
  br label %join
else:
  %y = sub i32 %b, 1
  br label %join
join:
  %p = phi i32 [ %x, %then ], [ %y, %else ]
  ret i32 %p
}
define void @skip(i32 %a) {
entry:
  %c = icmp eq i32 %a, 0
  br i1 %c, label %done, label %body
body:
  call void @side()
  br label %done
done:
  ret void
}
define i32 @count(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %next = add i32 %i, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %next
}
