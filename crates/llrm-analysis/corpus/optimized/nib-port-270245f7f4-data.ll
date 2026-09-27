target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal ptr addrspace(1) @data(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  ret ptr addrspace(1) %2
}

define internal i16 @main() addrspace(1) memory(none) willreturn {
b1:
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
