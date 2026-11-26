target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@a = global [1216 x i8] zeroinitializer
@b = global [1216 x i8] zeroinitializer

define i64 @f(i64 %k) {
entry:
  %in = icmp ult i64 %k, 1200
  %at = select i1 %in, i64 %k, i64 0
  %mark = select i1 %in, i8 1, i8 0
  %planted = add i64 %at, 1
  %pk = getelementptr inbounds i8, ptr @a, i64 %planted
  store i8 %mark, ptr %pk, align 1
  br label %body
latch:
  %stepped = phi i64 [ %i.next, %body ]
  %done = icmp eq i64 %stepped, 1200
  br i1 %done, label %same, label %body
body:
  %i = phi i64 [ 0, %entry ], [ %stepped, %latch ]
  %i.next = add nuw nsw i64 %i, 1
  %pa = getelementptr inbounds i8, ptr @a, i64 %i.next
  %pb0 = getelementptr inbounds i8, ptr @b, i64 %i
  %pb = getelementptr inbounds i8, ptr %pb0, i64 2
  %x = load i8, ptr %pa, align 1
  %y = load i8, ptr %pb, align 1
  %same.byte = icmp eq i8 %x, %y
  br i1 %same.byte, label %latch, label %found
found:
  %r = phi i64 [ %i, %body ]
  ret i64 %r
same:
  ret i64 -1
}
