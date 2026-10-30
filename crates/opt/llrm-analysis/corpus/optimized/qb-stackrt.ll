target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"start\00" }>
@$arguments$3 = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string5, i16 4), [12 x i8] c"unreachable\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"x\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  store i16 30000, ptr @$arguments$3, !tbaa !2
  tail call cc 1000 addrspace(1) void @DEEP(ptr @$arguments$3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

define cc 1000 void @DEEP(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull %2, i8 0, i16 4, i1 false)
  call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string6, ptr nonnull %2)
  %3 = load i16, ptr %0
  %4 = icmp sgt i16 %3, 0
  br i1 %4, label %b2, label %b4

b2:
  %5 = add nsw i16 %3, -1
  store i16 %5, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @DEEP(ptr nonnull %1)
  br label %b4

b4:
  call cc 1005 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %2)
  ret void
}

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$STDL(ptr) addrspace(1) nocallback nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
