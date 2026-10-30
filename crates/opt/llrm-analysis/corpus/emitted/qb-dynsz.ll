target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"ROWCOUNT%" = internal global [2 x i8] zeroinitializer
@"COLUMNCOUNT%" = internal global [2 x i8] zeroinitializer
@"ITEMS%" = internal global [22 x i8] zeroinitializer
@$qb$readData = internal constant [6 x i8] c" 3, 5\00"
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"ROWCOUNT%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"COLUMNCOUNT%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %1)
  %2 = load i16, ptr @"ROWCOUNT%", !tbaa !2
  %3 = load i16, ptr @"COLUMNCOUNT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -2, i16 %2, i16 4, i16 %3, i16 2, i16 258, ptr @"ITEMS%")
  %4 = getelementptr i8, ptr @"ITEMS%", i16 18
  %5 = load i16, ptr %4, !tbaa !2
  %6 = mul i16 5, %5
  %7 = add i16 %6, -1
  %8 = mul i16 %7, 2
  %9 = getelementptr i8, ptr @"ITEMS%", i16 2
  %10 = load i16, ptr %9, !tbaa !2
  %11 = getelementptr i8, ptr @"ITEMS%", i16 10
  %12 = load i16, ptr %11, !tbaa !2
  %13 = add i16 %12, %8
  %14 = inttoptr i16 %10 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr addrspace(1) %15, i16 %13
  store i16 123, ptr addrspace(1) %16, !tbaa !6
  %17 = getelementptr i8, ptr @"ITEMS%", i16 18
  %18 = load i16, ptr %17, !tbaa !2
  %19 = mul i16 4, %18
  %20 = add i16 %19, 3
  %21 = mul i16 %20, 2
  %22 = getelementptr i8, ptr @"ITEMS%", i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = getelementptr i8, ptr @"ITEMS%", i16 10
  %25 = load i16, ptr %24, !tbaa !2
  %26 = add i16 %25, %21
  %27 = inttoptr i16 %23 to ptr addrspace(2)
  %28 = addrspacecast ptr addrspace(2) %27 to ptr addrspace(1)
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 %26
  store i16 456, ptr addrspace(1) %29, !tbaa !6
  %30 = getelementptr i8, ptr @"ITEMS%", i16 18
  %31 = load i16, ptr %30, !tbaa !2
  %32 = mul i16 5, %31
  %33 = add i16 %32, -1
  %34 = mul i16 %33, 2
  %35 = getelementptr i8, ptr @"ITEMS%", i16 2
  %36 = load i16, ptr %35, !tbaa !2
  %37 = getelementptr i8, ptr @"ITEMS%", i16 10
  %38 = load i16, ptr %37, !tbaa !2
  %39 = add i16 %38, %34
  %40 = inttoptr i16 %36 to ptr addrspace(2)
  %41 = addrspacecast ptr addrspace(2) %40 to ptr addrspace(1)
  %42 = getelementptr i8, ptr addrspace(1) %41, i16 %39
  %43 = load i16, ptr addrspace(1) %42, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %43)
  %44 = getelementptr i8, ptr @"ITEMS%", i16 18
  %45 = load i16, ptr %44, !tbaa !2
  %46 = mul i16 4, %45
  %47 = add i16 %46, 3
  %48 = mul i16 %47, 2
  %49 = getelementptr i8, ptr @"ITEMS%", i16 2
  %50 = load i16, ptr %49, !tbaa !2
  %51 = getelementptr i8, ptr @"ITEMS%", i16 10
  %52 = load i16, ptr %51, !tbaa !2
  %53 = add i16 %52, %48
  %54 = inttoptr i16 %50 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 %53
  %57 = load i16, ptr addrspace(1) %56, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %57)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
