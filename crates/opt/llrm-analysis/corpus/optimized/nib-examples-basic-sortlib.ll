target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_abi.qb45.result = internal global [4 x i8] zeroinitializer

define cc 1000 void @SORTSCORES(ptr nocapture readonly %0, ptr nocapture readonly %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, inaccessiblemem: none) nounwind nofree nosync norecurse {
  %3 = load ptr addrspace(1), ptr %0
  %4 = load i16, ptr %1
  %5 = icmp ugt i16 %4, 1
  br i1 %5, label %6, label %b4

6:
  br label %7

7:
  %8 = phi i16 [ -1, %6 ], [ %22, %b7 ]
  %9 = phi i16 [ 2, %6 ], [ %21, %b7 ]
  %10 = phi i16 [ %20, %b7 ], [ 1, %6 ]
  %11 = shl i16 %10, 1
  %12 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 %11
  %13 = load i16, ptr addrspace(1) %12
  br label %b8

b4:
  ret void

b6:
  %14 = getelementptr i8, ptr addrspace(1) %3, i16 %24
  store i16 %27, ptr addrspace(1) %14
  %15 = add i16 %24, -2
  %16 = add i16 %23, 1
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %b7, label %b8

b7:
  %18 = phi i16 [ 0, %b6 ], [ %24, %b8 ]
  %19 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 %18
  store i16 %13, ptr addrspace(1) %19
  %20 = add nuw i16 %10, 1
  %exitcond = icmp eq i16 %20, %4
  %21 = add i16 %9, 2
  %22 = add i16 %8, -1
  br i1 %exitcond, label %b4, label %7

b8:
  %23 = phi i16 [ %8, %7 ], [ %16, %b6 ]
  %24 = phi i16 [ %9, %7 ], [ %15, %b6 ]
  %25 = getelementptr i8, ptr addrspace(1) %3, i16 %24
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 -2
  %27 = load i16, ptr addrspace(1) %26
  %28 = icmp slt i16 %27, %13
  br i1 %28, label %b6, label %b7
}

define cc 1000 void @UPPER(ptr nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
  %2 = getelementptr i8, ptr %0, i16 2
  %3 = load ptr, ptr %2
  %4 = load i16, ptr %0
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b4, label %6

6:
  br label %b3

b3:
  %7 = phi ptr [ %3, %6 ], [ %15, %b7 ]
  %8 = phi i16 [ %4, %6 ], [ %14, %b7 ]
  %9 = load i8, ptr %7
  %10 = icmp ugt i8 %9, 96
  %11 = icmp ult i8 %9, 123
  %12 = and i1 %10, %11
  br i1 %12, label %b5, label %b7

b4:
  ret void

b5:
  %13 = add nsw i8 %9, -32
  store i8 %13, ptr %7
  br label %b7

b7:
  %14 = add i16 %8, -1
  %15 = getelementptr i8, ptr %7, i16 1
  %exitcond = icmp eq i16 %14, 0
  br i1 %exitcond, label %b4, label %b3
}

define cc 1000 ptr @AVERAGE(ptr nocapture readonly %0, ptr nocapture readonly %1, ptr writeonly %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %4 = alloca double
  %5 = alloca i16
  %6 = alloca i16
  %7 = load ptr addrspace(1), ptr %0
  %8 = load i16, ptr %1
  store i16 %8, ptr %5, !tbaa !2
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %b4, label %10

10:
  br label %b3

b3:
  %11 = phi ptr addrspace(1) [ %7, %10 ], [ %17, %b3 ]
  %12 = phi i16 [ %8, %10 ], [ %16, %b3 ]
  %13 = phi i16 [ %15, %b3 ], [ 0, %10 ]
  %14 = load i16, ptr addrspace(1) %11
  %15 = add i16 %14, %13
  %16 = add i16 %12, -1
  %17 = getelementptr i8, ptr addrspace(1) %11, i16 2
  %exitcond = icmp eq i16 %16, 0
  br i1 %exitcond, label %b4, label %b3

b4:
  %18 = phi i16 [ 0, %3 ], [ %15, %b3 ]
  store i16 %18, ptr %6, align 2
  %19 = call cc 1000 addrspace(1) ptr @MEAN(ptr nonnull %6, ptr nonnull %5, ptr nonnull %4)
  %20 = load double, ptr %19
  store double %20, ptr %2
  ret ptr %2
}

define cc 1000 i16 @ROWTOTAL(ptr nocapture readonly %0, ptr nocapture readonly %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
  %3 = load ptr addrspace(1), ptr %0
  %4 = getelementptr i8, ptr %0, i16 14
  %5 = load i16, ptr %4
  %6 = getelementptr i8, ptr %0, i16 18
  %7 = load i16, ptr %6
  %8 = icmp eq i16 %5, 0
  br i1 %8, label %b4, label %9

9:
  %10 = load i16, ptr %1
  %11 = shl i16 %10, 1
  %12 = getelementptr i8, ptr addrspace(1) %3, i16 %11
  %13 = shl i16 %7, 1
  br label %b3

b3:
  %14 = phi ptr addrspace(1) [ %20, %b3 ], [ %12, %9 ]
  %15 = phi i16 [ %19, %b3 ], [ %5, %9 ]
  %16 = phi i16 [ 0, %9 ], [ %18, %b3 ]
  %17 = load i16, ptr addrspace(1) %14
  %18 = add i16 %17, %16
  %19 = add i16 %15, -1
  %20 = getelementptr i8, ptr addrspace(1) %14, i16 %13
  %exitcond = icmp eq i16 %19, 0
  br i1 %exitcond, label %b4, label %b3

b4:
  %21 = phi i16 [ 0, %2 ], [ %18, %b3 ]
  ret i16 %21
}

define cc 1000 ptr @INITIALS(ptr nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = alloca [2 x i8]
  %3 = getelementptr i8, ptr %0, i16 2
  %4 = load ptr, ptr %3
  %5 = load i16, ptr %0
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %2, i8 63, i16 2, i1 false)
  %6 = icmp eq i16 %5, 0
  br i1 %6, label %14, label %7

7:
  br label %b7

b7:
  %8 = phi ptr [ %4, %7 ], [ %25, %b12 ]
  %9 = phi i16 [ %5, %7 ], [ %24, %b12 ]
  %10 = phi i8 [ %23, %b12 ], [ 1, %7 ]
  %11 = phi i16 [ %22, %b12 ], [ 0, %7 ]
  %12 = load i8, ptr %8
  %13 = icmp eq i8 %12, 32
  br i1 %13, label %b12, label %b11

14:
  %15 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 2, ptr @$var_abi.qb45.result, !tbaa !2
  %16 = ptrtoint ptr addrspace(1) %15 to i32
  %17 = trunc i32 %16 to i16
  %18 = inttoptr i16 %17 to ptr
  store ptr %18, ptr getelementptr inbounds nuw (i8, ptr @$var_abi.qb45.result, i16 2), !tbaa !2
  %19 = call cc 1000 addrspace(1) ptr @B$SCPY(ptr @$var_abi.qb45.result)
  ret ptr %19

b11:
  %20 = icmp ne i8 %10, 0
  %21 = icmp ult i16 %11, 2
  %or.cond = select i1 %20, i1 %21, i1 false
  br i1 %or.cond, label %b13, label %b12

b12:
  %22 = phi i16 [ %11, %b7 ], [ %27, %b13 ], [ %11, %b11 ]
  %23 = phi i8 [ 1, %b7 ], [ 0, %b11 ], [ 0, %b13 ]
  %24 = add i16 %9, -1
  %25 = getelementptr i8, ptr %8, i16 1
  %exitcond = icmp eq i16 %24, 0
  br i1 %exitcond, label %14, label %b7

b13:
  %26 = getelementptr inbounds nuw i8, ptr %2, i16 %11
  store i8 %12, ptr %26, !tbaa !2
  %27 = add nuw nsw i16 %11, 1
  br label %b12
}

declare cc 1000 ptr @B$SCPY(ptr) addrspace(1)

declare cc 1000 ptr @MEAN(ptr, ptr, ptr) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
