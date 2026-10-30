target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$arguments$1 = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string4, i16 4), [10 x i8] c"123456789\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  store i32 0, ptr @$arguments$1, !tbaa !2
  %0 = tail call cc 1000 addrspace(1) i32 @"BENCHCRC&"(ptr @$arguments$1)
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %0)
  %2 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %2)
  ret void
}

define cc 1000 i32 @"BENCHCRC&"(ptr dereferenceable(4) nocapture readonly %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull %1, i8 0, i16 4, i1 false)
  call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string4, ptr nonnull %1)
  %2 = load i32, ptr %0
  %3 = xor i32 %2, -1
  %4 = getelementptr inbounds nuw i8, ptr %1, i16 2
  %data1 = load ptr, ptr %4
  br label %5

5:
  %6 = phi i16 [ %49, %5 ], [ -9, %b1 ]
  %7 = phi i32 [ %3, %b1 ], [ %48, %5 ]
  %8 = getelementptr i8, ptr %data1, i16 %6
  %9 = getelementptr i8, ptr %8, i16 9
  %byte = load i8, ptr %9
  %10 = zext i8 %byte to i32
  %11 = xor i32 %7, %10
  %12 = lshr i32 %11, 1
  %13 = and i32 %11, 1
  %14 = icmp eq i32 %13, 0
  %15 = select i1 %14, i32 0, i32 -306674912
  %16 = xor i32 %15, %12
  %17 = lshr i32 %16, 1
  %18 = and i32 %11, 2
  %19 = icmp eq i32 %18, 0
  %20 = select i1 %19, i32 0, i32 -306674912
  %21 = xor i32 %20, %17
  %22 = and i32 %11, 4
  %23 = icmp eq i32 %22, 0
  %24 = select i1 %23, i32 0, i32 1994146192
  %25 = lshr i32 %21, 2
  %26 = xor i32 %25, %24
  %27 = and i32 %11, 8
  %28 = icmp eq i32 %27, 0
  %29 = select i1 %28, i32 0, i32 -306674912
  %30 = xor i32 %29, %26
  %31 = and i32 %11, 16
  %32 = icmp eq i32 %31, 0
  %33 = select i1 %32, i32 0, i32 1994146192
  %34 = lshr i32 %30, 2
  %35 = xor i32 %34, %33
  %36 = and i32 %11, 32
  %37 = icmp eq i32 %36, 0
  %38 = select i1 %37, i32 0, i32 -306674912
  %39 = xor i32 %38, %35
  %40 = and i32 %16, 32
  %41 = icmp eq i32 %40, 0
  %42 = select i1 %41, i32 0, i32 1994146192
  %43 = lshr i32 %39, 2
  %44 = xor i32 %43, %42
  %45 = and i32 %21, 32
  %46 = icmp eq i32 %45, 0
  %47 = select i1 %46, i32 0, i32 -306674912
  %48 = xor i32 %47, %44
  %49 = add nsw i16 %6, 1
  %exitcond1 = icmp eq i16 %49, 0
  br i1 %exitcond1, label %b6, label %5

b6:
  %50 = xor i32 %48, -1
  call cc 1005 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %1)
  ret i32 %50
}

declare cc 1005 ptr @llrm.qb.B$STI4(i32) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$FMID(ptr releases, i16, i16) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 i16 @llrm.qb.B$FASC(ptr releases) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 void @llrm.qb.B$STDL(ptr) addrspace(1) nocallback nounwind

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
