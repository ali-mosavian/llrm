declare void @use(ptr nonnull)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)

@g = global i32 0

define i32 @null_edge(ptr %p) {
entry:
  %c = icmp eq ptr %p, null
  br i1 %c, label %isnull, label %notnull
isnull:
  ret i32 0
notnull:
  %v = load i32, ptr %p
  ret i32 %v
}

define i32 @deref(ptr %p, i1 %c) {
entry:
  %v = load i32, ptr %p
  br i1 %c, label %a, label %b
a:
  %d = icmp eq ptr %p, null
  %e = zext i1 %d to i32
  ret i32 %e
b:
  ret i32 %v
}

define i32 @deref_store(ptr %p, ptr %q, i32 %x) {
entry:
  store i32 %x, ptr %q
  %g = getelementptr inbounds i32, ptr %p, i64 3
  %v = load i32, ptr %g
  %c = icmp eq ptr %q, null
  %d = icmp ne ptr %p, null
  %e = icmp eq ptr %g, null
  ret i32 %v
}

define i32 @nonnull_arg(ptr %p) {
entry:
  call void @use(ptr %p)
  %c = icmp eq ptr %p, null
  %e = zext i1 %c to i32
  ret i32 %e
}

define i32 @memintrin(ptr %p, ptr %q) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %p, ptr %q, i64 16, i1 false)
  %c = icmp eq ptr %p, null
  %d = icmp eq ptr %q, null
  %e = zext i1 %c to i32
  ret i32 %e
}

define i32 @memset_vol(ptr %p) {
entry:
  call void @llvm.memset.p0.i64(ptr %p, i8 0, i64 16, i1 true)
  %c = icmp eq ptr %p, null
  %e = zext i1 %c to i32
  ret i32 %e
}

define i32 @alloca_nonnull() {
entry:
  %a = alloca i32
  %c = icmp eq ptr %a, null
  %e = zext i1 %c to i32
  ret i32 %e
}

define i32 @global_eq(ptr %p) {
entry:
  %c = icmp eq ptr %p, @g
  br i1 %c, label %same, label %diff
same:
  %d = icmp eq ptr %p, null
  %e = zext i1 %d to i32
  ret i32 %e
diff:
  %f = icmp ne ptr %p, @g
  ret i32 0
}

define i32 @phi_ptr(ptr %p, i1 %c) {
entry:
  br i1 %c, label %a, label %b
a:
  %x = getelementptr inbounds i32, ptr %p, i64 1
  br label %m
b:
  %y = alloca i32
  br label %m
m:
  %r = phi ptr [ %x, %a ], [ %y, %b ]
  %d = icmp eq ptr %r, null
  %e = zext i1 %d to i32
  ret i32 %e
}
