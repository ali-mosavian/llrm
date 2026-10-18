declare ptr @opaque()
declare i32 @opaque32()
declare void @take(ptr nonnull noundef)
declare void @take_any(ptr nonnull)
declare void @take_deref(ptr dereferenceable(4))
declare void @take_site(ptr)
declare ptr @id(ptr returned)
declare void @llvm.experimental.guard(i1, ...)
declare void @may_throw()
declare i32 @__gxx_personality_v0(...)

define void @site(ptr %p, i32 %x) {
  %a = call nonnull ptr @opaque()
  %b = call dereferenceable(4) ptr @opaque()
  %c = call dereferenceable_or_null(4) ptr @opaque()
  %d = call ptr @opaque()
  %e = call range(i32 1, 10) i32 @opaque32()
  %f = call range(i32 0, 10) i32 @opaque32()
  %g = call i32 @opaque32()
  %h = call ptr @id(ptr nonnull %p)
  %i = call ptr @id(ptr %p)
  ret void
}

define void @argattrs(ptr %a, ptr %b, ptr %c, ptr %d) {
entry:
  call void @take(ptr %a)
  call void @take_any(ptr %b)
  call void @take_deref(ptr %c)
  call void @take_site(ptr nonnull noundef %d)
  br label %next
next:
  %a2 = bitcast ptr %a to ptr
  %b2 = bitcast ptr %b to ptr
  %c2 = bitcast ptr %c to ptr
  %d2 = bitcast ptr %d to ptr
  ret void
}

define void @unreach(ptr %a) {
entry:
  ret void
dead:
  call void @take(ptr %a)
  %a2 = bitcast ptr %a to ptr
  br label %dead
}

define void @inv(ptr %a, ptr %b) personality ptr @__gxx_personality_v0 {
entry:
  invoke void @take(ptr %a) to label %ok unwind label %bad
ok:
  %a2 = bitcast ptr %a to ptr
  ret void
bad:
  %lp = landingpad { ptr, i32 } cleanup
  %a3 = bitcast ptr %a to ptr
  ret void
}

define void @guarded(ptr %a, ptr %b) {
  %c = icmp ne ptr %a, null
  call void (i1, ...) @llvm.experimental.guard(i1 %c)
  %a2 = bitcast ptr %a to ptr
  %c2 = icmp ne ptr %b, null
  %c3 = icmp eq i32 0, 0
  %c4 = and i1 %c2, %c3
  call void (i1, ...) @llvm.experimental.guard(i1 %c4)
  %b2 = bitcast ptr %b to ptr
  ret void
}

define void @edges(ptr %a, i32 %x, i32 %y) {
entry:
  %c = icmp ne ptr %a, null
  %z = icmp eq i32 %x, 0
  br i1 %c, label %t, label %t
t:
  %a2 = bitcast ptr %a to ptr
  br i1 %z, label %u, label %v
u:
  %x2 = add i32 %x, 1
  br label %w
v:
  br label %w
w:
  %a3 = bitcast ptr %a to ptr
  %p = phi i32 [ %x, %u ], [ %y, %v ]
  ret void
}

define void @switches(ptr %a, i32 %x) {
entry:
  %c = icmp ne ptr %a, null
  br i1 %c, label %t, label %f
t:
  switch i32 %x, label %m [ i32 1, label %m
                            i32 2, label %f ]
m:
  %a2 = bitcast ptr %a to ptr
  ret void
f:
  %a3 = bitcast ptr %a to ptr
  ret void
}
