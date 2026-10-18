declare void @sink(i32) nounwind willreturn
declare void @take(i32 noundef)
declare noundef i32 @defined()
declare i32 @plain()

define i32 @args(i32 noundef %a, i32 %b) {
  %x = add i32 %a, 1
  %y = add i32 %b, 1
  ret i32 %x
}

define i32 @flags(i32 %a, i32 %b) {
  %plain = add i32 %a, %b
  %nuw = add nuw i32 %a, %b
  %fr = freeze i32 %nuw
  %use = add nuw i32 %fr, 1
  ret i32 %use
}

define i32 @constants(i32 %a) {
  %p = add i32 %a, poison
  %c = add i32 %a, 7
  ret i32 %c
}

define i32 @calls(i32 %a) {
  %d = call i32 @defined()
  %p = call i32 @plain()
  %s = add i32 %d, %a
  ret i32 %s
}

define void @next_use(i32 %a, i32 %b) {
  %x = add i32 %a, 1
  call void @sink(i32 %b)
  call void @take(i32 %a)
  ret void
}

define void @blocked(i32 %a) {
  %x = call i32 @plain()
  call void @take(i32 %a)
  ret void
}

define i32 @divide(i32 %a, i32 %b) {
  %q = udiv i32 %a, %b
  ret i32 %q
}

define i32 @dominated(i32 %a, i32 %b) {
entry:
  %cmp = icmp eq i32 %a, 0
  br i1 %cmp, label %then, label %else
then:
  %t = add i32 %a, %b
  ret i32 %t
else:
  %e = add i32 %b, 1
  ret i32 %e
}

define i32 @phis(i1 %c, i32 noundef %a, i32 %b) {
entry:
  br i1 %c, label %l, label %r
l:
  %x = add i32 %a, 1
  br label %join
r:
  %y = add i32 %b, 1
  br label %join
join:
  %p = phi i32 [ %x, %l ], [ %a, %r ]
  %q = phi i32 [ %x, %l ], [ %y, %r ]
  ret i32 %p
}

define ptr @pointers(ptr %p, i64 %i) {
  %slot = alloca i32
  %g0 = getelementptr i32, ptr %slot, i64 0
  %g = getelementptr inbounds i32, ptr %p, i64 %i
  %ld = load i32, ptr %g, !noundef !0
  ret ptr %g0
}

define i32 @selects(i1 %c, i32 %a, i32 %b) {
  %s = select i1 %c, i32 %a, i32 %b
  %t = select i1 %c, i32 %a, i32 3
  ret i32 %s
}

!0 = !{}
