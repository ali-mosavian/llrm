target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"W%" = internal global [2 x i8] zeroinitializer
@b$seg = external global [2 x i8]

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  store i16 1, ptr @"W%", !tbaa !2
  %0 = add i16 ptrtoint (ptr addrspace(1) addrspacecast (ptr @"W%" to ptr addrspace(1)) to i16), -1024
  %1 = tail call i16 @llvm.smax.i16(i16 %0, i16 0)
  %2 = add nuw i16 %1, 2048
  %3 = icmp sgt i16 %1, %2
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  br i1 %3, label %22, label %4

4:
  br label %b13

b13:
  %5 = phi i16 [ 2049, %4 ], [ %21, %b17 ]
  %6 = phi i16 [ %20, %b17 ], [ %1, %4 ]
  %7 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to ptr addrspace(1)), i16 %6
  %8 = load volatile i8, ptr addrspace(1) %7
  %9 = icmp eq i8 %8, 9
  %10 = getelementptr i8, ptr addrspace(1) %7, i16 1
  %11 = load volatile i8, ptr addrspace(1) %10
  %12 = icmp eq i8 %11, 83
  %13 = select i1 %9, i1 %12, i1 false
  br i1 %13, label %b14, label %b17

b14:
  %14 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to ptr addrspace(1)), i16 %6
  store volatile i8 100, ptr addrspace(1) %14
  %15 = load i16, ptr @b$seg, !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2)
  %17 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 1
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 %6
  store volatile i8 0, ptr addrspace(1) %19
  br label %b21

b17:
  %20 = add nuw i16 %6, 1
  %21 = add nsw i16 %5, -1
  %exitcond = icmp eq i16 %21, 0
  br i1 %exitcond, label %b21, label %b13

22:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$DSG0()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 21257)
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  br label %45

b21:
  %23 = phi i16 [ -1, %b14 ], [ 0, %b17 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$DSG0()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %23)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 21257)
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  br label %24

24:
  %25 = phi i16 [ %43, %41 ], [ 2049, %b21 ]
  %26 = phi i16 [ %1, %b21 ], [ %42, %41 ]
  %27 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to ptr addrspace(1)), i16 %26
  %28 = load volatile i8, ptr addrspace(1) %27
  %29 = icmp eq i8 %28, 10
  %30 = getelementptr i8, ptr addrspace(1) %27, i16 1
  %31 = load volatile i8, ptr addrspace(1) %30
  %32 = icmp eq i8 %31, 84
  %33 = select i1 %29, i1 %32, i1 false
  br i1 %33, label %34, label %41

34:
  %35 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to ptr addrspace(1)), i16 %26
  store volatile i8 101, ptr addrspace(1) %35
  %36 = load i16, ptr @b$seg, !tbaa !2
  %37 = inttoptr i16 %36 to ptr addrspace(2)
  %38 = addrspacecast ptr addrspace(2) %37 to ptr addrspace(1)
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 1
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 %26
  store volatile i8 0, ptr addrspace(1) %40
  br label %45

41:
  %42 = add nuw i16 %26, 1
  %43 = add nsw i16 %25, -1
  %44 = icmp eq i16 %43, 0
  br i1 %44, label %45, label %24

45:
  %46 = phi i16 [ -1, %34 ], [ 0, %22 ], [ 0, %41 ]
  %47 = icmp sgt i16 %1, %2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$DSG0()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %46)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 21514)
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  br i1 %47, label %70, label %48

48:
  br label %49

49:
  %50 = phi i16 [ 2049, %48 ], [ %68, %66 ]
  %51 = phi i16 [ %67, %66 ], [ %1, %48 ]
  %52 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to ptr addrspace(1)), i16 %51
  %53 = load volatile i8, ptr addrspace(1) %52
  %54 = icmp eq i8 %53, 11
  %55 = getelementptr i8, ptr addrspace(1) %52, i16 1
  %56 = load volatile i8, ptr addrspace(1) %55
  %57 = icmp eq i8 %56, 85
  %58 = select i1 %54, i1 %57, i1 false
  br i1 %58, label %59, label %66

59:
  %60 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) addrspacecast (ptr @"W%" to ptr addrspace(2)) to ptr addrspace(1)), i16 %51
  store volatile i8 102, ptr addrspace(1) %60
  %61 = load i16, ptr @b$seg, !tbaa !2
  %62 = inttoptr i16 %61 to ptr addrspace(2)
  %63 = addrspacecast ptr addrspace(2) %62 to ptr addrspace(1)
  %64 = getelementptr i8, ptr addrspace(1) %63, i16 1
  %65 = getelementptr i8, ptr addrspace(1) %64, i16 %51
  store volatile i8 0, ptr addrspace(1) %65
  br label %70

66:
  %67 = add nuw i16 %51, 1
  %68 = add nsw i16 %50, -1
  %69 = icmp eq i16 %68, 0
  br i1 %69, label %70, label %49

70:
  %71 = phi i16 [ -1, %59 ], [ 0, %45 ], [ 0, %66 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$DSG0()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %71)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 21771)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 21771)
  ret void
}

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$DSG0() addrspace(1) nocallback nounwind

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
