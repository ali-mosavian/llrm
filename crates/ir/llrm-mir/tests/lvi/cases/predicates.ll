define i1 @merge(i1 %c, i32 %x) {
entry:
  br i1 %c, label %a, label %b
a:
  %v1 = and i32 %x, 3
  br label %m
b:
  %t = and i32 %x, 7
  %v2 = add i32 %t, 10
  br label %m
m:
  %phi = phi i32 [ %v1, %a ], [ %v2, %b ]
  %p = icmp eq i32 %phi, 8
  ret i1 %p
}

define i1 @outside_block(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %a, label %b
a:
  br label %m
b:
  br label %m
m:
  %p = icmp ugt i32 %x, 10
  %q = icmp ult i32 %x, 5
  ret i1 %p
}

define i1 @two_values(i32 %x, i32 %y) {
entry:
  %a = icmp ult i32 %x, 10
  %b = icmp ugt i32 %y, 20
  %c = and i1 %a, %b
  br i1 %c, label %ok, label %out
ok:
  %p = icmp ult i32 %x, %y
  %q = icmp eq i32 %x, %y
  %r = icmp sgt i32 %x, %y
  ret i1 %p
out:
  ret i1 false
}

define i1 @ptr_null(ptr %p) {
entry:
  %c = icmp eq ptr %p, null
  br i1 %c, label %n, label %nn
n:
  %a = icmp eq ptr %p, null
  ret i1 %a
nn:
  %b = icmp ne ptr %p, null
  ret i1 %b
}

define i1 @fold_const(i32 %x) {
entry:
  %a = icmp ult i32 3, 5
  %b = icmp ugt i32 %x, 0
  ret i1 %a
}
