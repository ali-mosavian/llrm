define i32 @count(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %body ]
  %c = icmp ult i32 %i, 100
  br i1 %c, label %body, label %exit
body:
  %next = add nuw i32 %i, 1
  br label %loop
exit:
  ret i32 %i
}

define i32 @count_down(i32 %n) {
entry:
  %ok = icmp ult i32 %n, 50
  br i1 %ok, label %loop, label %out
loop:
  %i = phi i32 [ %n, %entry ], [ %next, %loop ]
  %next = sub i32 %i, 1
  %c = icmp ne i32 %i, 0
  br i1 %c, label %loop, label %out
out:
  ret i32 0
}

define i32 @two_phis(i1 %p, i32 %x) {
entry:
  %c = icmp ult i32 %x, 4
  br i1 %c, label %a, label %b
a:
  br label %m
b:
  %d = icmp ugt i32 %x, 100
  br i1 %d, label %m, label %e
e:
  ret i32 0
m:
  %v = phi i32 [ 1, %a ], [ 200, %b ]
  %w = phi i32 [ %x, %a ], [ %x, %b ]
  %s = add i32 %v, %w
  %t = icmp eq i32 %v, 8
  ret i32 %s
}

define i32 @nested(i32 %n) {
entry:
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %i.next, %outer.latch ]
  %oc = icmp ult i32 %i, 10
  br i1 %oc, label %inner, label %done
inner:
  %j = phi i32 [ 0, %outer ], [ %j.next, %inner ]
  %j.next = add i32 %j, 2
  %ic = icmp ult i32 %j.next, 20
  br i1 %ic, label %inner, label %outer.latch
outer.latch:
  %i.next = add i32 %i, 1
  br label %outer
done:
  %r = add i32 %i, %n
  ret i32 %r
}

define i32 @self_loop(i32 %x) {
entry:
  br label %l
l:
  %p = phi i32 [ %x, %entry ], [ %p, %l ]
  %c = icmp eq i32 %p, 0
  br i1 %c, label %l, label %e
e:
  ret i32 %p
}

define i32 @unreachable_pred(i32 %x) {
entry:
  br label %m
dead:
  br label %m
m:
  %p = phi i32 [ 5, %entry ], [ 9, %dead ]
  ret i32 %p
}
