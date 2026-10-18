target datalayout = "e-p:64:64-p1:32:32:32:32-i64:64"
@g = global [4 x i32] zeroinitializer
@h = global i32 0
@g1 = addrspace(1) global i32 0

define void @c() {
  %a = icmp eq ptr getelementptr (i8, ptr @g, i64 4), getelementptr (i8, ptr @g, i64 8)
  %b = icmp ult ptr getelementptr inbounds (i8, ptr @g, i64 4), getelementptr inbounds (i8, ptr @g, i64 8)
  %c = icmp slt ptr getelementptr inbounds (i8, ptr @g, i64 4), getelementptr inbounds (i8, ptr @g, i64 8)
  %d = icmp ugt ptr getelementptr (i8, ptr @g, i64 4), getelementptr (i8, ptr @g, i64 8)
  %e = icmp eq ptr inttoptr (i64 0 to ptr), null
  %f = icmp eq ptr inttoptr (i64 4 to ptr), null
  %g = icmp ne ptr inttoptr (i64 4 to ptr), inttoptr (i64 4 to ptr)
  %h = icmp eq i64 ptrtoint (ptr @h to i64), 0
  %i = icmp eq i64 ptrtoint (ptr @h to i64), ptrtoint (ptr @h to i64)
  %j = icmp eq ptr @g, getelementptr inbounds ([4 x i32], ptr @g, i64 0, i64 0)
  %k = icmp eq ptr @g, @h
  %l = icmp eq ptr null, getelementptr (i8, ptr null, i64 4)
  %m = icmp ult i32 1, 2
  %n = icmp slt i8 -1, 0
  %o = icmp eq ptr getelementptr inbounds (i8, ptr @g, i64 16), getelementptr inbounds ([4 x i32], ptr @g, i64 1)
  %p = icmp ule ptr getelementptr (i8, ptr @g, i64 -4), getelementptr (i8, ptr @g, i64 4)
  %q = icmp eq ptr addrspace(1) inttoptr (i32 4 to ptr addrspace(1)), inttoptr (i64 4 to ptr addrspace(1))
  %r = icmp eq i32 ptrtoint (ptr addrspace(1) @g1 to i32), 0
  ret void
}
