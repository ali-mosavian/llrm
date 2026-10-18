@g = global i32 0
@w = extern_weak global i32
@g1 = addrspace(1) global i32 0

%S = type { i32, [4 x i16], i64 }

declare ptr @opaque()
declare nonnull ptr @nn()
declare ptr @id(ptr returned)
declare dereferenceable(8) ptr @deref()
declare void @use(ptr)
declare void @take(ptr nonnull noundef)

define void @sources(ptr nonnull %a, ptr dereferenceable(4) %b, ptr %c, ptr byval(i32) %d, ptr addrspace(1) dereferenceable(4) %e) {
  %al = alloca i32
  %al1 = alloca i32, addrspace(1)
  %c1 = call ptr @nn()
  %c2 = call ptr @opaque()
  %c3 = call ptr @id(ptr %a)
  %c4 = call ptr @id(ptr %c)
  %c5 = call ptr @deref()
  %l1 = load ptr, ptr %c, !nonnull !0
  %l2 = load ptr, ptr %c, !dereferenceable !1
  %l3 = load ptr, ptr %c
  %i = ptrtoint ptr %a to i64
  %p = inttoptr i64 %i to ptr
  %i2 = ptrtoint ptr %c to i64
  %p2 = inttoptr i64 %i2 to ptr
  %b1 = bitcast ptr %a to ptr
  %fr = freeze ptr %a
  ret void
}

define void @geps(ptr %p, i64 %n, ptr nonnull %nn) {
  %g1 = getelementptr inbounds i32, ptr %p, i64 1
  %g2 = getelementptr inbounds i32, ptr %p, i64 0
  %g3 = getelementptr i32, ptr %p, i64 1
  %g4 = getelementptr nuw i32, ptr %p, i64 %n
  %g5 = getelementptr inbounds %S, ptr %p, i64 0, i32 1
  %g6 = getelementptr inbounds %S, ptr %p, i64 0, i32 0
  %g7 = getelementptr inbounds %S, ptr %p, i64 0, i32 1, i64 2
  %g8 = getelementptr inbounds i32, ptr %nn, i64 %n
  %g9 = getelementptr inbounds i8, ptr %p, i64 %n
  %nz = or i64 %n, 1
  %g10 = getelementptr inbounds i8, ptr %p, i64 %nz
  %g11 = getelementptr inbounds [0 x i32], ptr %p, i64 0, i64 %nz
  %g12 = getelementptr inbounds i32, ptr @g, i64 %n
  %g13 = getelementptr inbounds i32, ptr @w, i64 %n
  %g14 = getelementptr inbounds i32, ptr @w, i64 1
  %g15 = ptrtoint ptr getelementptr inbounds (i8, ptr @g, i64 4) to i64
  %g16 = ptrtoint ptr @g to i64
  %g17 = ptrtoint ptr addrspace(1) @g1 to i64
  ret void
}

define void @dominating(ptr %p, ptr %q, ptr %r, ptr %s, i32 %x) {
entry:
  %c = icmp ne ptr %p, null
  br i1 %c, label %a, label %b
a:
  %pa = bitcast ptr %p to ptr
  %i = ptrtoint ptr %p to i64
  br label %b
b:
  %pb = bitcast ptr %p to ptr
  %c2 = icmp eq ptr %q, null
  br i1 %c2, label %e, label %d
d:
  %qd = bitcast ptr %q to ptr
  %c3 = icmp ugt ptr %r, null
  %c4 = icmp ne ptr %s, null
  %both = and i1 %c3, %c4
  br i1 %both, label %f, label %e
f:
  %rf = bitcast ptr %r to ptr
  %sf = bitcast ptr %s to ptr
  %qf = bitcast ptr %q to ptr
  br label %e
e:
  %qe = bitcast ptr %q to ptr
  ret void
}

define void @uses(ptr %p, ptr %q, ptr %r, i32 %y) {
  %v = load i32, ptr %p
  %pp = bitcast ptr %p to ptr
  store i32 0, ptr %q
  %qq = bitcast ptr %q to ptr
  call void @take(ptr %r)
  %rr = bitcast ptr %r to ptr
  ret void
}

!0 = !{}
!1 = !{i64 4}
