; Array accesses to delinearize: parametric sizes, fixed sizes, and accesses that do not divide.
define void @parametric3(i64 %n, i64 %m, i64 %o, ptr %A) {
entry:
  br label %for.i
for.i:
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j
for.j:
  %j = phi i64 [ 0, %for.i ], [ %j.next, %for.j.latch ]
  br label %for.k
for.k:
  %k = phi i64 [ 0, %for.j ], [ %k.next, %for.k ]
  %im = mul nsw i64 %i, %m
  %imj = add nsw i64 %im, %j
  %imjo = mul nsw i64 %imj, %o
  %idx = add nsw i64 %imjo, %k
  %p = getelementptr inbounds double, ptr %A, i64 %idx
  store double 1.0, ptr %p
  %k.next = add nuw nsw i64 %k, 1
  %kc = icmp slt i64 %k.next, %o
  br i1 %kc, label %for.k, label %for.j.latch
for.j.latch:
  %j.next = add nuw nsw i64 %j, 1
  %jc = icmp slt i64 %j.next, %m
  br i1 %jc, label %for.j, label %for.i.latch
for.i.latch:
  %i.next = add nuw nsw i64 %i, 1
  %ic = icmp slt i64 %i.next, %n
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @parametric2(i32 %n, i32 %m, ptr %A) {
entry:
  br label %for.i
for.i:
  %i = phi i32 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j
for.j:
  %j = phi i32 [ 0, %for.i ], [ %j.next, %for.j ]
  %im = mul nsw i32 %i, %m
  %idx = add nsw i32 %im, %j
  %p = getelementptr inbounds i32, ptr %A, i32 %idx
  %v = load i32, ptr %p
  %w = add i32 %v, 1
  store i32 %w, ptr %p
  %j.next = add nuw nsw i32 %j, 1
  %jc = icmp slt i32 %j.next, %m
  br i1 %jc, label %for.j, label %for.i.latch
for.i.latch:
  %i.next = add nuw nsw i32 %i, 1
  %ic = icmp slt i32 %i.next, %n
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @fixed3(ptr %A) {
entry:
  br label %for.i
for.i:
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j
for.j:
  %j = phi i64 [ 0, %for.i ], [ %j.next, %for.j.latch ]
  br label %for.k
for.k:
  %k = phi i64 [ 0, %for.j ], [ %k.next, %for.k ]
  %i256 = mul nsw i64 %i, 256
  %j32 = mul nsw i64 %j, 32
  %t = add nsw i64 %i256, %j32
  %idx = add nsw i64 %t, %k
  %p = getelementptr inbounds double, ptr %A, i64 %idx
  store double 1.0, ptr %p
  %k.next = add nuw nsw i64 %k, 1
  %kc = icmp ult i64 %k.next, 32
  br i1 %kc, label %for.k, label %for.j.latch
for.j.latch:
  %j.next = add nuw nsw i64 %j, 1
  %jc = icmp ult i64 %j.next, 8
  br i1 %jc, label %for.j, label %for.i.latch
for.i.latch:
  %i.next = add nuw nsw i64 %i, 1
  %ic = icmp ult i64 %i.next, 42
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @fixed2(ptr %A) {
entry:
  br label %for.i
for.i:
  %i = phi i32 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j
for.j:
  %j = phi i32 [ 0, %for.i ], [ %j.next, %for.j ]
  %i100 = mul nsw i32 %i, 100
  %idx = add nsw i32 %i100, %j
  %p = getelementptr inbounds i16, ptr %A, i32 %idx
  %v = load i16, ptr %p
  %j.next = add nuw nsw i32 %j, 1
  %jc = icmp ult i32 %j.next, 100
  br i1 %jc, label %for.j, label %for.i.latch
for.i.latch:
  %i.next = add nuw nsw i32 %i, 1
  %ic = icmp ult i32 %i.next, 50
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @arraygep(ptr %A) {
entry:
  br label %for.i
for.i:
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j
for.j:
  %j = phi i64 [ 0, %for.i ], [ %j.next, %for.j ]
  %p = getelementptr inbounds [100 x [200 x float]], ptr %A, i64 0, i64 %i, i64 %j
  store float 1.0, ptr %p
  %j.next = add nuw nsw i64 %j, 1
  %jc = icmp ult i64 %j.next, 200
  br i1 %jc, label %for.j, label %for.i.latch
for.i.latch:
  %i.next = add nuw nsw i64 %i, 1
  %ic = icmp ult i64 %i.next, 100
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @notarray(i64 %n, ptr %A) {
entry:
  br label %for.i
for.i:
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i ]
  %i3 = mul nsw i64 %i, 3
  %p = getelementptr inbounds i32, ptr %A, i64 %i3
  %v = load i32, ptr %p
  %i.next = add nuw nsw i64 %i, 1
  %ic = icmp slt i64 %i.next, %n
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @offsets(i64 %n, i64 %m, ptr %A) {
entry:
  br label %for.i
for.i:
  %i = phi i64 [ 1, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j
for.j:
  %j = phi i64 [ 1, %for.i ], [ %j.next, %for.j ]
  %im = mul nsw i64 %i, %m
  %i1 = add nsw i64 %i, 1
  %i1m = mul nsw i64 %i1, %m
  %jm1 = add nsw i64 %j, -1
  %a = add nsw i64 %im, %j
  %b = add nsw i64 %i1m, %jm1
  %pa = getelementptr inbounds float, ptr %A, i64 %a
  %pb = getelementptr inbounds float, ptr %A, i64 %b
  %v = load float, ptr %pa
  store float %v, ptr %pb
  %j.next = add nuw nsw i64 %j, 1
  %jc = icmp slt i64 %j.next, %m
  br i1 %jc, label %for.j, label %for.i.latch
for.i.latch:
  %i.next = add nuw nsw i64 %i, 1
  %ic = icmp slt i64 %i.next, %n
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @constants(ptr %A, i64 %n) {
entry:
  br label %for.i
for.i:
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i ]
  %t = shl nsw i64 %i, 3
  %q = add nsw i64 %t, 5
  %p = getelementptr inbounds i8, ptr %A, i64 %q
  %v = load i8, ptr %p
  %i.next = add nuw nsw i64 %i, 1
  %ic = icmp slt i64 %i.next, %n
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}

define void @badstep(ptr %A, i64 %n) {
entry:
  br label %for.i
for.i:
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i ]
  %i6 = mul nsw i64 %i, 6
  %p = getelementptr inbounds i8, ptr %A, i64 %i6
  %v = load i32, ptr %p, align 1
  %i.next = add nuw nsw i64 %i, 1
  %ic = icmp slt i64 %i.next, %n
  br i1 %ic, label %for.i, label %exit
exit:
  ret void
}
