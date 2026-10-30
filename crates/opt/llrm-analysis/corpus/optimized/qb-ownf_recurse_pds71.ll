target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"T&" = internal global [4 x i8] zeroinitializer
@$arguments$2 = internal global [6 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn {
b1:
  store i16 15, ptr @$arguments$2, !tbaa !2
  %0 = tail call cc 1000 addrspace(1) i16 @"FIB%"(ptr @$arguments$2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  store i16 30, ptr getelementptr inbounds nuw (i8, ptr @$arguments$2, i16 2), !tbaa !2
  %1 = tail call cc 1000 addrspace(1) i32 @"DEPTH&"(ptr getelementptr inbounds nuw (i8, ptr @$arguments$2, i16 2))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %1)
  store i32 0, ptr @"T&", !tbaa !2
  store i16 20, ptr getelementptr inbounds nuw (i8, ptr @$arguments$2, i16 4), !tbaa !2
  tail call cc 1000 addrspace(1) void @DOWN(ptr getelementptr inbounds nuw (i8, ptr @$arguments$2, i16 4), ptr @"T&")
  %2 = load i32, ptr @"T&", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 i16 @"FIB%"(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: read) nounwind nofree nosync {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = load i16, ptr %0
  %4 = icmp slt i16 %3, 2
  br i1 %4, label %b5, label %b3

b3:
  %5 = add nsw i16 %3, -1
  store i16 %5, ptr %2, !tbaa !2
  %6 = call cc 1000 addrspace(1) i16 @"FIB%"(ptr nonnull %2)
  %7 = add nsw i16 %3, -2
  store i16 %7, ptr %1, !tbaa !2
  %8 = call cc 1000 addrspace(1) i16 @"FIB%"(ptr nonnull %1)
  %9 = add i16 %8, %6
  br label %b5

b5:
  %10 = phi i16 [ %9, %b3 ], [ %3, %b1 ]
  ret i16 %10
}

define cc 1000 i32 @"DEPTH&"(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: read) nounwind nofree nosync {
b1:
  %1 = alloca i16
  %2 = load i16, ptr %0
  %3 = sext i16 %2 to i32
  %4 = icmp eq i32 %3, 0
  br i1 %4, label %b5, label %b3

b3:
  %5 = trunc i32 %3 to i16
  %6 = add i16 %5, -1
  store i16 %6, ptr %1, !tbaa !2
  %7 = call cc 1000 addrspace(1) i32 @"DEPTH&"(ptr nonnull %1)
  %8 = add i32 %7, %3
  br label %b5

b5:
  %9 = phi i32 [ %8, %b3 ], [ 0, %b1 ]
  ret i32 %9
}

define cc 1000 void @DOWN(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(4) nocapture %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: readwrite) nounwind nofree nosync {
b1:
  %2 = alloca i16
  %3 = load i16, ptr %0
  %4 = sext i16 %3 to i32
  %5 = icmp sgt i32 %4, 0
  br i1 %5, label %b2, label %b4

b2:
  %6 = trunc i32 %4 to i16
  %7 = add nsw i16 %6, -1
  store i16 %7, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @DOWN(ptr nonnull %2, ptr nonnull %1)
  br label %b4

b4:
  %8 = load i32, ptr %1
  %9 = add i32 %8, %4
  store i32 %9, ptr %1
  ret void
}

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
