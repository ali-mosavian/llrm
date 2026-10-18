target datalayout = "e-p:64:64-p1:32:32:32:32-i64:64"
%S = type { i32, i8, i64, [3 x i16] }
@g = constant [4 x i32] [i32 1, i32 2, i32 3, i32 4]
@s = constant %S { i32 7, i8 9, i64 11, [3 x i16] [i16 1, i16 2, i16 3] }
@w = global [4 x i32] zeroinitializer
@ext = external global i32
@g1 = addrspace(1) constant [2 x i32] [i32 5, i32 6]

define void @f(i32 %x) {
  %a = getelementptr inbounds [4 x i32], ptr @g, i64 0, i64 2
  %b = getelementptr [4 x i32], ptr @g, i64 0, i64 2
  %c = getelementptr i8, ptr @g, i64 0
  %d = getelementptr i32, ptr @g, i32 3
  %e = getelementptr inbounds %S, ptr @s, i32 0, i32 3, i32 1
  %f = getelementptr %S, ptr null, i32 1
  %h = getelementptr i8, ptr null, i64 50
  %i = getelementptr inbounds i8, ptr getelementptr inbounds (i8, ptr @g, i64 4), i64 8
  %j = getelementptr i8, ptr getelementptr (i8, ptr @g, i64 4), i64 -8
  %k = getelementptr inbounds [4 x i32], ptr @w, i64 1, i64 1
  %l = getelementptr i32, ptr @ext, i64 1
  %m = getelementptr inbounds i32, ptr @ext, i64 0
  %n = getelementptr i8, ptr inttoptr (i64 100 to ptr), i64 20
  %o = getelementptr [4 x i32], ptr @g, i16 0, i16 1
  %p = getelementptr inbounds [2 x i32], ptr addrspace(1) @g1, i32 0, i32 1
  %q = getelementptr i32, ptr @g, i64 poison
  %r = getelementptr i8, ptr poison, i64 4
  ret void
}
