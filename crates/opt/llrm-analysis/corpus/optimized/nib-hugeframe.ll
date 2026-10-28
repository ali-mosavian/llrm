target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @last(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode noipa "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr addrspace(1) %0, !range !5
  %2 = icmp samesign ugt i16 %1, 31999
  br i1 %2, label %b2, label %b3

b2:
  %3 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  %5 = getelementptr inbounds i8, ptr addrspace(1) %4, i16 -1538
  %6 = load i16, ptr addrspace(1) %5
  ret i16 %6

b3:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [64000 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %1, i8 0, i16 -1536, i1 false)
  br label %b10

b9:
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 2097184000, ptr %0
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store ptr addrspace(1) %2, ptr %3, !tbaa !2
  %4 = addrspacecast ptr %0 to ptr addrspace(1)
  %5 = call cc 1005 addrspace(1) i16 @last(ptr addrspace(1) %4)
  call cc 1005 addrspace(1) void @N$PI2(i16 %5)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b10:
  %6 = phi i16 [ %11, %b10 ], [ 0, %b1 ]
  %7 = phi i16 [ %10, %b10 ], [ -32000, %b1 ]
  %8 = add i16 %7, 32000
  %9 = getelementptr i8, ptr %1, i16 %6
  store i16 %8, ptr %9, !tbaa !2
  %10 = add nsw i16 %7, 1
  %11 = add nuw i16 %6, 2
  %exitcond = icmp eq i16 %10, 0
  br i1 %exitcond, label %b9, label %b10
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
