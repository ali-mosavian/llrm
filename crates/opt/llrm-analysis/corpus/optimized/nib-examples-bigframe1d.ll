target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(inaccessiblemem: readwrite) {
b1:
  %0 = alloca [8192 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %0, i8 0, i16 8192, i1 false)
  call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr nonnull %0, i16 7, i16 4096, i1 false)
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 8190
  %2 = load i16, ptr %1, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %2)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.experimental.memset.pattern.p0.i16.i16(ptr nocapture writeonly, i16, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
