target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@$arguments$1 = internal global [4 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 1, ptr @$arguments$1, !tbaa !2
  %0 = call cc 1000 addrspace(1) i32 @"BENCHBINTREE&"(ptr @$arguments$1)
  %1 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %0)
  %2 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %2)
  ret void
}

define internal cc 1000 void @ADDNODE(ptr dereferenceable(18) %0, ptr dereferenceable(2) %1, ptr dereferenceable(2) %2) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = load i16, ptr %2
  %6 = mul i16 3, %5
  %7 = mul i16 %6, 2
  %8 = getelementptr i8, ptr %0, i16 2
  %9 = load i16, ptr %8
  %10 = add i16 0, %7
  %11 = inttoptr i16 %9 to ptr addrspace(2), !llrm.descriptor !7
  %12 = addrspacecast ptr addrspace(2) %11 to ptr addrspace(1)
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 %10
  %14 = load i16, ptr addrspace(1) %13
  %15 = load i16, ptr %1
  %16 = mul i16 3, %15
  %17 = mul i16 %16, 2
  %18 = add i16 0, %17
  %19 = inttoptr i16 %9 to ptr addrspace(2), !llrm.descriptor !7
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %18
  %22 = load i16, ptr addrspace(1) %21
  %23 = icmp slt i16 %14, %22
  br i1 %23, label %b2, label %b3

b2:
  store i16 1, ptr %4, !tbaa !2
  br label %b4

b3:
  store i16 2, ptr %4, !tbaa !2
  br label %b4

b4:
  %24 = load i16, ptr %1
  %25 = mul i16 3, %24
  %26 = load i16, ptr %4, !tbaa !2
  %27 = add i16 %25, %26
  %28 = mul i16 %27, 2
  %29 = getelementptr i8, ptr %0, i16 2
  %30 = load i16, ptr %29
  %31 = add i16 0, %28
  %32 = inttoptr i16 %30 to ptr addrspace(2), !llrm.descriptor !7
  %33 = addrspacecast ptr addrspace(2) %32 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %33, i16 %31
  %35 = load i16, ptr addrspace(1) %34
  %36 = icmp eq i16 %35, 0
  br i1 %36, label %b5, label %b6

b5:
  %37 = load i16, ptr %1
  %38 = mul i16 3, %37
  %39 = load i16, ptr %4, !tbaa !2
  %40 = add i16 %38, %39
  %41 = mul i16 %40, 2
  %42 = getelementptr i8, ptr %0, i16 2
  %43 = load i16, ptr %42
  %44 = add i16 0, %41
  %45 = inttoptr i16 %43 to ptr addrspace(2), !llrm.descriptor !7
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %44
  %48 = load i16, ptr %2
  store i16 %48, ptr addrspace(1) %47
  br label %b7

b6:
  %49 = load i16, ptr %1
  %50 = mul i16 3, %49
  %51 = load i16, ptr %4, !tbaa !2
  %52 = add i16 %50, %51
  %53 = mul i16 %52, 2
  %54 = getelementptr i8, ptr %0, i16 2
  %55 = load i16, ptr %54
  %56 = add i16 0, %53
  %57 = inttoptr i16 %55 to ptr addrspace(2), !llrm.descriptor !7
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 %56
  %60 = load i16, ptr addrspace(1) %59
  store i16 %60, ptr %3, !tbaa !2
  call cc 1000 void @ADDNODE(ptr %0, ptr %3, ptr %2)
  %61 = load i16, ptr %3, !tbaa !2
  store i16 %61, ptr addrspace(1) %59
  br label %b7

b7:
  ret void
}

define internal cc 1000 i32 @"WALK&"(ptr dereferenceable(18) %0, ptr dereferenceable(2) %1, ptr dereferenceable(2) %2) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i32
  %8 = load i16, ptr %1
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  store i32 0, ptr %7, !tbaa !2
  br label %b4

b3:
  %10 = load i16, ptr %1
  %11 = mul i16 3, %10
  %12 = add i16 %11, 1
  %13 = mul i16 %12, 2
  %14 = getelementptr i8, ptr %0, i16 2
  %15 = load i16, ptr %14
  %16 = add i16 0, %13
  %17 = inttoptr i16 %15 to ptr addrspace(2), !llrm.descriptor !8
  %18 = addrspacecast ptr addrspace(2) %17 to ptr addrspace(1)
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 %16
  %20 = load i16, ptr addrspace(1) %19
  store i16 %20, ptr %6, !tbaa !2
  %21 = load i16, ptr %2
  %22 = add i16 %21, 1
  store i16 %22, ptr %5, !tbaa !2
  %23 = call cc 1000 i32 @"WALK&"(ptr %0, ptr %6, ptr %5)
  %24 = load i16, ptr %6, !tbaa !2
  store i16 %24, ptr addrspace(1) %19
  %25 = load i16, ptr %1
  %26 = mul i16 3, %25
  %27 = mul i16 %26, 2
  %28 = getelementptr i8, ptr %0, i16 2
  %29 = load i16, ptr %28
  %30 = add i16 0, %27
  %31 = inttoptr i16 %29 to ptr addrspace(2), !llrm.descriptor !8
  %32 = addrspacecast ptr addrspace(2) %31 to ptr addrspace(1)
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 %30
  %34 = load i16, ptr addrspace(1) %33
  %35 = load i16, ptr %2
  %36 = sext i16 %35 to i32
  %37 = sext i16 %34 to i32
  %38 = mul i32 %37, %36
  %39 = add i32 %23, %38
  %40 = load i16, ptr %1
  %41 = mul i16 3, %40
  %42 = add i16 %41, 2
  %43 = mul i16 %42, 2
  %44 = add i16 0, %43
  %45 = inttoptr i16 %29 to ptr addrspace(2), !llrm.descriptor !8
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %44
  %48 = load i16, ptr addrspace(1) %47
  store i16 %48, ptr %4, !tbaa !2
  %49 = load i16, ptr %2
  %50 = add i16 %49, 1
  store i16 %50, ptr %3, !tbaa !2
  %51 = call cc 1000 i32 @"WALK&"(ptr %0, ptr %4, ptr %3)
  %52 = load i16, ptr %4, !tbaa !2
  store i16 %52, ptr addrspace(1) %47
  %53 = add i32 %39, %51
  store i32 %53, ptr %7, !tbaa !2
  br label %b4

b4:
  br label %b5

b5:
  %54 = load i32, ptr %7, !tbaa !2
  ret i32 %54
}

define cc 1000 i32 @"BENCHBINTREE&"(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" noinline {
b1:
  %1 = alloca i32
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [18 x i8]
  store i16 0, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 0, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %11, i16 4
  store i16 0, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %11, i16 6
  store i16 0, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %11, i16 8
  store i16 0, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %11, i16 10
  store i16 0, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %11, i16 12
  store i16 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %11, i16 14
  store i16 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %11, i16 16
  store i16 0, ptr %19, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 902, i16 2, i16 257, ptr %11)
  store i16 0, ptr %2, !tbaa !2
  store i16 902, ptr %4, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b2

b2:
  %20 = load i16, ptr %5, !tbaa !2
  %21 = icmp sge i16 %20, 0
  br i1 %21, label %b3, label %b4

b3:
  %22 = load i16, ptr %2, !tbaa !2
  %23 = load i16, ptr %4, !tbaa !2
  %24 = icmp sle i16 %22, %23
  br i1 %24, label %b5, label %b6

b4:
  %25 = load i16, ptr %2, !tbaa !2
  %26 = load i16, ptr %4, !tbaa !2
  %27 = icmp sge i16 %25, %26
  br i1 %27, label %b5, label %b6

b5:
  %28 = load i16, ptr %2, !tbaa !2
  %29 = mul i16 %28, 2
  %30 = getelementptr i8, ptr %11, i16 2
  %31 = load i16, ptr %30, !tbaa !2
  %32 = add i16 0, %29
  %33 = inttoptr i16 %31 to ptr addrspace(2), !llrm.descriptor !9
  %34 = addrspacecast ptr addrspace(2) %33 to ptr addrspace(1)
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 %32
  store i16 0, ptr addrspace(1) %35, !tbaa !6
  %36 = load i16, ptr %2, !tbaa !2
  %37 = load i16, ptr %5, !tbaa !2
  %38 = add nsw i16 %36, %37
  store i16 %38, ptr %2, !tbaa !2
  br label %b2

b6:
  %39 = load i32, ptr %0
  store i32 %39, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  store i16 300, ptr %6, !tbaa !2
  store i16 1, ptr %7, !tbaa !2
  br label %b7

b7:
  %40 = load i16, ptr %7, !tbaa !2
  %41 = icmp sge i16 %40, 0
  br i1 %41, label %b8, label %b9

b8:
  %42 = load i16, ptr %2, !tbaa !2
  %43 = load i16, ptr %6, !tbaa !2
  %44 = icmp sle i16 %42, %43
  br i1 %44, label %b10, label %b11

b9:
  %45 = load i16, ptr %2, !tbaa !2
  %46 = load i16, ptr %6, !tbaa !2
  %47 = icmp sge i16 %45, %46
  br i1 %47, label %b10, label %b11

b10:
  %48 = load i32, ptr %3, !tbaa !2
  %49 = mul i32 %48, 25173
  %50 = add i32 %49, 13849
  %51 = and i32 %50, 65535
  store i32 %51, ptr %3, !tbaa !2
  %52 = load i16, ptr %2, !tbaa !2
  %53 = mul i16 3, %52
  %54 = mul i16 %53, 2
  %55 = getelementptr i8, ptr %11, i16 2
  %56 = load i16, ptr %55, !tbaa !2
  %57 = add i16 0, %54
  %58 = inttoptr i16 %56 to ptr addrspace(2), !llrm.descriptor !9
  %59 = addrspacecast ptr addrspace(2) %58 to ptr addrspace(1)
  %60 = getelementptr i8, ptr addrspace(1) %59, i16 %57
  %61 = load i32, ptr %3, !tbaa !2
  %62 = and i32 %61, 4095
  %63 = trunc i32 %62 to i16
  store i16 %63, ptr addrspace(1) %60, !tbaa !6
  %64 = load i16, ptr %2, !tbaa !2
  %65 = icmp sgt i16 %64, 1
  br i1 %65, label %b12, label %b13

b11:
  store i16 1, ptr %9, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  %66 = call cc 1000 i32 @"WALK&"(ptr %11, ptr %9, ptr %10)
  store i32 %66, ptr %1, !tbaa !2
  br label %b15

b12:
  store i16 1, ptr %8, !tbaa !2
  call cc 1000 void @ADDNODE(ptr %11, ptr %8, ptr %2)
  br label %b14

b13:
  br label %b14

b14:
  %67 = load i16, ptr %2, !tbaa !2
  %68 = load i16, ptr %7, !tbaa !2
  %69 = add nsw i16 %67, %68
  store i16 %69, ptr %2, !tbaa !2
  br label %b7

b15:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %11)
  %70 = load i32, ptr %1, !tbaa !2
  ret i32 %70
}

declare cc 1000 ptr @llrm.qb.B$STI4(i32) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{}
!8 = !{}
!9 = !{}
