target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @at(ptr addrspace(1) noalias readonly dereferenceable(10) %0, i8 %1) addrspace(1) {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = zext i8 %1 to i16
  %4 = icmp ult i16 %3, %2
  br i1 %4, label %b2, label %b3

b2:
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = icmp ugt i16 %6, 1
  br i1 %7, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %8 = mul i16 %3, %6
  %9 = add i16 %8, 1
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = shl i16 %9, 1
  %13 = getelementptr i8, ptr addrspace(1) %11, i16 %12
  %14 = load i16, ptr addrspace(1) %13
  ret i16 %14

b5:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @value() addrspace(1) memory(none) willreturn {
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
