declare void @sink(i16)
declare void @abort() noreturn
declare void @report() cold

define void @pointer(ptr %p) {
entry:
  %c = icmp eq ptr %p, null
  br i1 %c, label %yes, label %no
yes:
  call void @sink(i16 1)
  br label %no
no:
  ret void
}

define void @zero(i16 %x) {
entry:
  %c = icmp eq i16 %x, 0
  br i1 %c, label %a, label %b
a:
  %n = icmp slt i16 %x, 0
  br i1 %n, label %b, label %c1
b:
  %m = icmp sgt i16 %x, -1
  br i1 %m, label %c1, label %d
c1:
  %o = icmp slt i16 %x, 1
  br i1 %o, label %d, label %e
d:
  call void @sink(i16 2)
  br label %e
e:
  ret void
}

define void @mask(i16 %x) {
entry:
  %bit = and i16 %x, 8
  %c = icmp eq i16 %bit, 0
  br i1 %c, label %yes, label %no
yes:
  call void @sink(i16 3)
  br label %no
no:
  ret void
}

define void @float(double %x, double %y) {
entry:
  %c = fcmp oeq double %x, %y
  br i1 %c, label %a, label %b
a:
  %n = fcmp une double %x, %y
  br i1 %n, label %b, label %c1
b:
  %o = fcmp ord double %x, %y
  br i1 %o, label %c1, label %d
c1:
  %u = fcmp uno double %x, %y
  br i1 %u, label %d, label %e
d:
  call void @sink(i16 4)
  br label %e
e:
  ret void
}

define void @weights(i16 %x) {
entry:
  %c = icmp ult i16 %x, 7
  br i1 %c, label %yes, label %no, !prof !0
yes:
  call void @sink(i16 5)
  br label %no
no:
  %d = icmp ult i16 %x, 3
  br i1 %d, label %dead, label %done, !prof !1
dead:
  unreachable
done:
  ret void
}

define void @unreachable(i16 %x) {
entry:
  %c = icmp ult i16 %x, 9
  br i1 %c, label %ok, label %fail
fail:
  call void @abort()
  unreachable
ok:
  %d = icmp ult i16 %x, 5
  br i1 %d, label %warm, label %cold
cold:
  call void @report()
  br label %warm
warm:
  ret void
}

define void @loop(i16 %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %next, %body ]
  %c = icmp ult i16 %i, %n
  br i1 %c, label %body, label %exit
body:
  call void @sink(i16 %i)
  %next = add i16 %i, 1
  br label %head
exit:
  ret void
}

define void @unlikely(i16 %n) {
entry:
  br label %head
head:
  %k = phi i16 [ 0, %entry ], [ %k.next, %latch ]
  %i = phi i16 [ 0, %entry ], [ %i.next, %latch ]
  %k1 = add i16 %k, 1
  %full = icmp sge i16 %k1, 10
  br i1 %full, label %reset, label %latch
reset:
  call void @sink(i16 %k1)
  br label %latch
latch:
  %k.next = phi i16 [ 0, %reset ], [ %k1, %head ]
  %i.next = add i16 %i, 1
  %go = icmp ult i16 %i.next, %n
  br i1 %go, label %head, label %exit
exit:
  ret void
}

define void @switched(i16 %x) {
entry:
  switch i16 %x, label %other [
    i16 0, label %zero
    i16 1, label %one
  ], !prof !2
zero:
  call void @sink(i16 0)
  br label %other
one:
  call void @abort()
  unreachable
other:
  ret void
}

!0 = !{!"branch_weights", i32 3, i32 1}
!1 = !{!"branch_weights", i32 1000, i32 1}
!2 = !{!"branch_weights", i32 5, i32 3, i32 8}
