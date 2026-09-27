target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @at(ptr addrspace(1) noalias readonly dereferenceable(10) %0, i8 %1) addrspace(1) {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp ugt i16 %2, 19
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = icmp ugt i16 %5, 1
  br i1 %6, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %7 = mul i16 %5, 19
  %8 = add i16 %7, 1
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %10 = load ptr addrspace(1), ptr addrspace(1) %9
  %11 = shl i16 %8, 1
  %12 = getelementptr i8, ptr addrspace(1) %10, i16 %11
  %13 = load i16, ptr addrspace(1) %12
  ret i16 %13

b5:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @value() addrspace(1) {
b1:
  ret i16 7
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
