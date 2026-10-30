target datalayout = "e-p:64:64-p1:32:32:32:32-i64:64"
@g = global i32 0
@h = global [4 x i32] zeroinitializer
@g1 = addrspace(1) global i32 0

define void @c() {
  %a = ptrtoint ptr @g to i64
  %b = ptrtoint ptr inttoptr (i64 4660 to ptr) to i64
  %c = ptrtoint ptr inttoptr (i32 4660 to ptr) to i64
  %d = ptrtoint ptr inttoptr (i64 4660 to ptr) to i16
  %e = inttoptr i64 ptrtoint (ptr @g to i64) to ptr
  %f = inttoptr i32 ptrtoint (ptr @g to i32) to ptr
  %g = inttoptr i128 ptrtoint (ptr @g to i128) to ptr
  %h = ptrtoint ptr getelementptr (i32, ptr null, i64 4) to i64
  %i = ptrtoint ptr getelementptr inbounds ([4 x i32], ptr null, i64 0, i64 3) to i32
  %j = ptrtoint ptr getelementptr (i8, ptr @g, i64 4) to i64
  %k = bitcast ptr @g to ptr
  %l = addrspacecast ptr addrspace(1) @g1 to ptr
  %m = addrspacecast ptr addrspacecast (ptr addrspace(1) @g1 to ptr) to ptr addrspace(1)
  %n = trunc i64 ptrtoint (ptr @g to i64) to i32
  %o = zext i32 4660 to i64
  %p = sext i8 -3 to i128
  %q = bitcast i32 1078530011 to float
  %r = bitcast double 3.14 to i64
  %s = bitcast i64 -1 to double
  %t = ptrtoint ptr getelementptr (i8, ptr @g, i64 8) to i64
  %u = inttoptr i64 0 to ptr
  %v = ptrtoint ptr null to i64
  %w = trunc i64 ptrtoint (ptr getelementptr (i32, ptr null, i64 3) to i64) to i8
  ret void
}
