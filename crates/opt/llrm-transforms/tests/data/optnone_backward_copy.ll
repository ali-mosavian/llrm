target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @plain(ptr %dst, ptr %src, i64 %n) noinline {
entry:
  %d = alloca ptr
  %s = alloca ptr
  %i = alloca i64
  store ptr %dst, ptr %d
  store ptr %src, ptr %s
  store i64 %n, ptr %i
  br label %cond
cond:
  %a = load i64, ptr %i
  %c = icmp sgt i64 %a, 0
  br i1 %c, label %body, label %end
body:
  %sp = load ptr, ptr %s
  %b = load i64, ptr %i
  %b1 = sub nsw i64 %b, 1
  %pa = getelementptr inbounds i8, ptr %sp, i64 %b1
  %v = load i8, ptr %pa
  %dp = load ptr, ptr %d
  %e = load i64, ptr %i
  %e1 = sub nsw i64 %e, 1
  %pb = getelementptr inbounds i8, ptr %dp, i64 %e1
  store i8 %v, ptr %pb
  %f = load i64, ptr %i
  %f1 = add nsw i64 %f, -1
  store i64 %f1, ptr %i
  br label %cond
end:
  ret void
}

define void @kept(ptr %dst, ptr %src, i64 %n) optnone noinline {
entry:
  %d = alloca ptr
  %s = alloca ptr
  %i = alloca i64
  store ptr %dst, ptr %d
  store ptr %src, ptr %s
  store i64 %n, ptr %i
  br label %cond
cond:
  %a = load i64, ptr %i
  %c = icmp sgt i64 %a, 0
  br i1 %c, label %body, label %end
body:
  %sp = load ptr, ptr %s
  %b = load i64, ptr %i
  %b1 = sub nsw i64 %b, 1
  %pa = getelementptr inbounds i8, ptr %sp, i64 %b1
  %v = load i8, ptr %pa
  %dp = load ptr, ptr %d
  %e = load i64, ptr %i
  %e1 = sub nsw i64 %e, 1
  %pb = getelementptr inbounds i8, ptr %dp, i64 %e1
  store i8 %v, ptr %pb
  %f = load i64, ptr %i
  %f1 = add nsw i64 %f, -1
  store i64 %f1, ptr %i
  br label %cond
end:
  ret void
}

define void @caller(ptr %dst, ptr %src) optnone noinline {
  call void @leaf(ptr %dst, ptr %src)
  ret void
}

define void @leaf(ptr %dst, ptr %src) {
  %v = load i8, ptr %src
  store i8 %v, ptr %dst
  ret void
}
