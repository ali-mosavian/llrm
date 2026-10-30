target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"I%" = internal global [2 x i8] zeroinitializer
@$arguments$4 = internal global [6 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"arrays" }>
@$string5 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string5, i16 4), [8 x i8] c"survived" }>
@$string6 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string6, i16 4), [8 x i8] c"strings\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"bad\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"x\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string9, i16 4), [4 x i8] c"bad\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  store i16 1, ptr @$arguments$4, !tbaa !2
  tail call cc 1000 addrspace(1) void @ARRAYS(ptr @$arguments$4)
  store i16 1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$4, i16 2), !tbaa !2
  tail call cc 1000 addrspace(1) void @STRINGS(ptr getelementptr inbounds nuw (i8, ptr @$arguments$4, i16 2))
  store i16 1, ptr @"I%", !tbaa !2
  br label %b5

b5:
  %0 = phi i16 [ 1, %b1 ], [ %3, %b5 ]
  %1 = srem i16 %0, 50
  store i16 %1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$4, i16 4), !tbaa !2
  tail call cc 1000 addrspace(1) void @ARRAYS(ptr getelementptr inbounds nuw (i8, ptr @$arguments$4, i16 4))
  %2 = load i16, ptr @"I%", !tbaa !2
  %3 = add nsw i16 %2, 1
  store i16 %3, ptr @"I%", !tbaa !2
  %4 = icmp slt i16 %2, 3000
  br i1 %4, label %b5, label %b6

b6:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  %5 = tail call cc 1005 addrspace(1) i32 @llrm.qb.B$FRSD(i16 0)
  store i16 1, ptr @"I%", !tbaa !2
  br label %b10

b10:
  tail call cc 1000 addrspace(1) void @STRINGS(ptr @"I%")
  %6 = load i16, ptr @"I%", !tbaa !2
  %7 = add nsw i16 %6, 1
  store i16 %7, ptr @"I%", !tbaa !2
  %8 = icmp slt i16 %6, 50
  br i1 %8, label %b10, label %b11

b11:
  %9 = tail call cc 1005 addrspace(1) i32 @llrm.qb.B$FRSD(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCSD(ptr @$string6)
  %10 = icmp eq i32 %5, %9
  %11 = sext i1 %10 to i16
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @ARRAYS(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca [64 x i8]
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 2
  %3 = getelementptr inbounds nuw i8, ptr %1, i16 10
  %4 = getelementptr inbounds nuw i8, ptr %1, i16 46
  %5 = getelementptr inbounds nuw i8, ptr %1, i16 48
  call void @llvm.memset.p0.i16(ptr align 2 %1, i8 0, i16 64, i1 false)
  %6 = load i16, ptr %0
  %7 = add i16 %6, 300
  call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 %7, i16 2, i16 257, ptr nonnull %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 40, i16 4, i16 257, ptr nonnull %4)
  %8 = load i16, ptr %0
  %9 = shl i16 %8, 1
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = inttoptr i16 %10 to ptr addrspace(2)
  %13 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr addrspace(1) %13, i16 %11
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 %9
  store i16 %8, ptr addrspace(1) %15, !tbaa !6
  %16 = load i16, ptr %5, !tbaa !2
  %17 = inttoptr i16 %16 to ptr addrspace(2)
  %18 = addrspacecast ptr addrspace(2) %17 to ptr addrspace(1)
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 8
  %20 = sext i16 %8 to i32
  store i32 %20, ptr addrspace(1) %19, !tbaa !8
  %21 = load i16, ptr %0
  %22 = shl i16 %21, 1
  %23 = getelementptr i8, ptr addrspace(1) %14, i16 %22
  %24 = load i16, ptr addrspace(1) %23, !tbaa !6
  %25 = sext i16 %24 to i32
  %26 = add nsw i32 %25, %20
  %27 = sext i16 %22 to i32
  %28 = icmp eq i32 %26, %27
  br i1 %28, label %b4, label %b2

b2:
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  br label %b4

b4:
  call cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %1)
  call cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %4)
  ret void
}

define cc 1000 void @STRINGS(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" norecurse {
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull %2, i8 0, i16 4, i1 false)
  %3 = load i16, ptr %0
  %4 = add i16 %3, 100
  %5 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STRS(i16 %4, ptr @$string8)
  call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %5, ptr nonnull %2)
  %length = load i16, ptr %2
  %6 = load i16, ptr %0
  %7 = add i16 %6, 100
  %8 = icmp eq i16 %length, %7
  br i1 %8, label %b4, label %b2

b2:
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string9)
  br label %b4

b4:
  call cc 1005 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %2)
  ret void
}

declare cc 1005 void @llrm.qb.B$PCSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 i32 @llrm.qb.B$FRSD(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1) nocallback

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 ptr @llrm.qb.B$STRS(i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 i16 @llrm.qb.B$FLEN(ptr) addrspace(1) "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 void @llrm.qb.B$STDL(ptr) addrspace(1) nocallback nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
