@g = global [8 x i32] zeroinitializer
@h = global i32 0

declare ptr @id(ptr returned)
declare ptr @opaque(ptr)

define void @peel(ptr %a, ptr addrspace(1) %q, i64 %i) {
entry:
  %s = alloca [16 x i8]
  %g1 = getelementptr i8, ptr %a, i64 4
  %g2 = getelementptr i32, ptr %g1, i64 %i
  %g3 = getelementptr inbounds [16 x i8], ptr %s, i64 0, i64 3
  %c1 = addrspacecast ptr addrspace(1) %q to ptr
  %g4 = getelementptr i8, ptr %c1, i64 1
  %r = call ptr @id(ptr %g2)
  %r2 = call ptr @id(ptr %r)
  %o = call ptr @opaque(ptr %g1)
  %ld = load ptr, ptr %a
  %g5 = getelementptr i8, ptr %ld, i64 2
  %sel = select i1 poison, ptr %g1, ptr %g3
  %k1 = getelementptr i32, ptr @g, i64 3
  %k2 = getelementptr i32, ptr getelementptr (i32, ptr @g, i64 1), i64 %i
  %k3 = getelementptr i8, ptr @h, i64 %i
  %n = getelementptr i8, ptr null, i64 %i
  br label %next

next:
  %ph = phi ptr [ %g1, %entry ]
  %pg = getelementptr i8, ptr %ph, i64 8
  br label %two

two:
  %ph2 = phi ptr [ %pg, %next ], [ %g3, %next ]
  ret void
}

define void @deep(ptr %a) {
  %p1 = getelementptr i8, ptr %a, i64 1
  %p2 = getelementptr i8, ptr %p1, i64 1
  %p3 = getelementptr i8, ptr %p2, i64 1
  %p4 = getelementptr i8, ptr %p3, i64 1
  %p5 = getelementptr i8, ptr %p4, i64 1
  %p6 = getelementptr i8, ptr %p5, i64 1
  %p7 = getelementptr i8, ptr %p6, i64 1
  %p8 = getelementptr i8, ptr %p7, i64 1
  %p9 = getelementptr i8, ptr %p8, i64 1
  %p10 = getelementptr i8, ptr %p9, i64 1
  %p11 = getelementptr i8, ptr %p10, i64 1
  %p12 = getelementptr i8, ptr %p11, i64 1
  ret void
}
