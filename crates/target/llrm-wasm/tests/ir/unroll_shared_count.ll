target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@a = hidden global [64 x i16] zeroinitializer, align 16
@b = hidden global [64 x i32] zeroinitializer, align 16
@counts = internal unnamed_addr constant [6 x i16] [i16 0, i16 1, i16 2, i16 17, i16 63, i16 64], align 2

define void @f(i32 %t) #0 {
  %p = getelementptr inbounds nuw [2 x i8], ptr @counts, i32 %t
  %n = load i16, ptr %p, align 2
  %w = zext nneg i16 %n to i32
  br label %l1

l1:
  %i = phi i32 [ 0, %0 ], [ %i1, %l1 ]
  %pa = getelementptr inbounds nuw [2 x i8], ptr @a, i32 %i
  store i16 4660, ptr %pa, align 2
  %i1 = add nuw nsw i32 %i, 1
  %c1 = icmp eq i32 %i1, %w
  br i1 %c1, label %m, label %l1

m:
  br label %l2

l2:
  %j = phi i32 [ 0, %m ], [ %j1, %l2 ]
  %pb = getelementptr inbounds nuw [4 x i8], ptr @b, i32 %j
  store i32 305419896, ptr %pb, align 4
  %j1 = add nuw nsw i32 %j, 1
  %c2 = icmp eq i32 %j1, %w
  br i1 %c2, label %out, label %l2

out:
  ret void
}

attributes #0 = { "no-builtins" }
