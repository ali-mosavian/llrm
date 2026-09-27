target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00ok\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00bad\00"

define internal void @bump(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = icmp ugt i16 %1, 1
  br i1 %2, label %b2, label %b3

b2:
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  %5 = getelementptr i8, ptr addrspace(1) %4, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = add i16 %6, 3
  store i16 %7, ptr addrspace(1) %5
  ret void

b3:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
