target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"E1&" = internal global [4 x i8] zeroinitializer
@"E2&" = internal global [4 x i8] zeroinitializer
@"F1&" = internal global [4 x i8] zeroinitializer
@"F2&" = internal global [4 x i8] zeroinitializer
@"G1&" = internal global [4 x i8] zeroinitializer
@"G2&" = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [6 x i8] c"\04\00ELT=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00ELE=" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [6 x i8] c"\04\00EGT=" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [6 x i8] c"\04\00EGE=" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [6 x i8] c"\04\00EEQ=" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [6 x i8] c"\04\00ENE=" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [6 x i8] c"\04\00FLT=" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [6 x i8] c"\04\00FLE=" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>
@$string20$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 4) to i16), [6 x i8] c"\04\00FGT=" }>
@$string20$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 2) to i16), ptr @$fslSegment }>
@$string22$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 4) to i16), [6 x i8] c"\04\00FGE=" }>
@$string22$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 2) to i16), ptr @$fslSegment }>
@$string24$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 4) to i16), [6 x i8] c"\04\00FEQ=" }>
@$string24$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 2) to i16), ptr @$fslSegment }>
@$string26$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string26$payload, i16 4) to i16), [6 x i8] c"\04\00FNE=" }>
@$string26$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string26$payload, i16 2) to i16), ptr @$fslSegment }>
@$string28$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string28$payload, i16 4) to i16), [6 x i8] c"\04\00GLT=" }>
@$string28$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string28$payload, i16 2) to i16), ptr @$fslSegment }>
@$string30$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string30$payload, i16 4) to i16), [6 x i8] c"\04\00GLE=" }>
@$string30$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string30$payload, i16 2) to i16), ptr @$fslSegment }>
@$string32$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string32$payload, i16 4) to i16), [6 x i8] c"\04\00GGT=" }>
@$string32$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string32$payload, i16 2) to i16), ptr @$fslSegment }>
@$string34$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string34$payload, i16 4) to i16), [6 x i8] c"\04\00GGE=" }>
@$string34$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string34$payload, i16 2) to i16), ptr @$fslSegment }>
@$string36$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string36$payload, i16 4) to i16), [6 x i8] c"\04\00GEQ=" }>
@$string36$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string36$payload, i16 2) to i16), ptr @$fslSegment }>
@$string38$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string38$payload, i16 4) to i16), [6 x i8] c"\04\00GNE=" }>
@$string38$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string38$payload, i16 2) to i16), ptr @$fslSegment }>
@$string40$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string40$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string40$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string40$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 305430527, ptr @"E1&", !tbaa !2
  store i32 305430528, ptr @"E2&", !tbaa !2
  store i32 2147450879, ptr @"F1&", !tbaa !2
  store i32 2147450880, ptr @"F2&", !tbaa !2
  %0 = sub i32 0, 268402689
  store i32 %0, ptr @"G1&", !tbaa !2
  %1 = sub i32 0, 268402688
  store i32 %1, ptr @"G2&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %2 = load i32, ptr @"E1&", !tbaa !2
  %3 = load i32, ptr @"E2&", !tbaa !2
  %4 = icmp slt i32 %2, %3
  %5 = sext i1 %4 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %5)
  %6 = load i32, ptr @"E2&", !tbaa !2
  %7 = load i32, ptr @"E1&", !tbaa !2
  %8 = icmp slt i32 %6, %7
  %9 = sext i1 %8 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %9)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %10 = load i32, ptr @"E1&", !tbaa !2
  %11 = load i32, ptr @"E2&", !tbaa !2
  %12 = icmp sle i32 %10, %11
  %13 = sext i1 %12 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %13)
  %14 = load i32, ptr @"E2&", !tbaa !2
  %15 = load i32, ptr @"E1&", !tbaa !2
  %16 = icmp sle i32 %14, %15
  %17 = sext i1 %16 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %17)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %18 = load i32, ptr @"E1&", !tbaa !2
  %19 = load i32, ptr @"E2&", !tbaa !2
  %20 = icmp sgt i32 %18, %19
  %21 = sext i1 %20 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %21)
  %22 = load i32, ptr @"E2&", !tbaa !2
  %23 = load i32, ptr @"E1&", !tbaa !2
  %24 = icmp sgt i32 %22, %23
  %25 = sext i1 %24 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %25)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10$descriptor)
  %26 = load i32, ptr @"E1&", !tbaa !2
  %27 = load i32, ptr @"E2&", !tbaa !2
  %28 = icmp sge i32 %26, %27
  %29 = sext i1 %28 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %29)
  %30 = load i32, ptr @"E2&", !tbaa !2
  %31 = load i32, ptr @"E1&", !tbaa !2
  %32 = icmp sge i32 %30, %31
  %33 = sext i1 %32 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %33)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %34 = load i32, ptr @"E1&", !tbaa !2
  %35 = load i32, ptr @"E2&", !tbaa !2
  %36 = icmp eq i32 %34, %35
  %37 = sext i1 %36 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %37)
  %38 = load i32, ptr @"E2&", !tbaa !2
  %39 = load i32, ptr @"E1&", !tbaa !2
  %40 = icmp eq i32 %38, %39
  %41 = sext i1 %40 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %41)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14$descriptor)
  %42 = load i32, ptr @"E1&", !tbaa !2
  %43 = load i32, ptr @"E2&", !tbaa !2
  %44 = icmp ne i32 %42, %43
  %45 = sext i1 %44 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %45)
  %46 = load i32, ptr @"E2&", !tbaa !2
  %47 = load i32, ptr @"E1&", !tbaa !2
  %48 = icmp ne i32 %46, %47
  %49 = sext i1 %48 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %49)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16$descriptor)
  %50 = load i32, ptr @"F1&", !tbaa !2
  %51 = load i32, ptr @"F2&", !tbaa !2
  %52 = icmp slt i32 %50, %51
  %53 = sext i1 %52 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %53)
  %54 = load i32, ptr @"F2&", !tbaa !2
  %55 = load i32, ptr @"F1&", !tbaa !2
  %56 = icmp slt i32 %54, %55
  %57 = sext i1 %56 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %57)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18$descriptor)
  %58 = load i32, ptr @"F1&", !tbaa !2
  %59 = load i32, ptr @"F2&", !tbaa !2
  %60 = icmp sle i32 %58, %59
  %61 = sext i1 %60 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %61)
  %62 = load i32, ptr @"F2&", !tbaa !2
  %63 = load i32, ptr @"F1&", !tbaa !2
  %64 = icmp sle i32 %62, %63
  %65 = sext i1 %64 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %65)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20$descriptor)
  %66 = load i32, ptr @"F1&", !tbaa !2
  %67 = load i32, ptr @"F2&", !tbaa !2
  %68 = icmp sgt i32 %66, %67
  %69 = sext i1 %68 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %69)
  %70 = load i32, ptr @"F2&", !tbaa !2
  %71 = load i32, ptr @"F1&", !tbaa !2
  %72 = icmp sgt i32 %70, %71
  %73 = sext i1 %72 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %73)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string22$descriptor)
  %74 = load i32, ptr @"F1&", !tbaa !2
  %75 = load i32, ptr @"F2&", !tbaa !2
  %76 = icmp sge i32 %74, %75
  %77 = sext i1 %76 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %77)
  %78 = load i32, ptr @"F2&", !tbaa !2
  %79 = load i32, ptr @"F1&", !tbaa !2
  %80 = icmp sge i32 %78, %79
  %81 = sext i1 %80 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %81)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string24$descriptor)
  %82 = load i32, ptr @"F1&", !tbaa !2
  %83 = load i32, ptr @"F2&", !tbaa !2
  %84 = icmp eq i32 %82, %83
  %85 = sext i1 %84 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %85)
  %86 = load i32, ptr @"F2&", !tbaa !2
  %87 = load i32, ptr @"F1&", !tbaa !2
  %88 = icmp eq i32 %86, %87
  %89 = sext i1 %88 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %89)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string26$descriptor)
  %90 = load i32, ptr @"F1&", !tbaa !2
  %91 = load i32, ptr @"F2&", !tbaa !2
  %92 = icmp ne i32 %90, %91
  %93 = sext i1 %92 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %93)
  %94 = load i32, ptr @"F2&", !tbaa !2
  %95 = load i32, ptr @"F1&", !tbaa !2
  %96 = icmp ne i32 %94, %95
  %97 = sext i1 %96 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %97)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string28$descriptor)
  %98 = load i32, ptr @"G1&", !tbaa !2
  %99 = load i32, ptr @"G2&", !tbaa !2
  %100 = icmp slt i32 %98, %99
  %101 = sext i1 %100 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %101)
  %102 = load i32, ptr @"G2&", !tbaa !2
  %103 = load i32, ptr @"G1&", !tbaa !2
  %104 = icmp slt i32 %102, %103
  %105 = sext i1 %104 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %105)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string30$descriptor)
  %106 = load i32, ptr @"G1&", !tbaa !2
  %107 = load i32, ptr @"G2&", !tbaa !2
  %108 = icmp sle i32 %106, %107
  %109 = sext i1 %108 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %109)
  %110 = load i32, ptr @"G2&", !tbaa !2
  %111 = load i32, ptr @"G1&", !tbaa !2
  %112 = icmp sle i32 %110, %111
  %113 = sext i1 %112 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %113)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string32$descriptor)
  %114 = load i32, ptr @"G1&", !tbaa !2
  %115 = load i32, ptr @"G2&", !tbaa !2
  %116 = icmp sgt i32 %114, %115
  %117 = sext i1 %116 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %117)
  %118 = load i32, ptr @"G2&", !tbaa !2
  %119 = load i32, ptr @"G1&", !tbaa !2
  %120 = icmp sgt i32 %118, %119
  %121 = sext i1 %120 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %121)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string34$descriptor)
  %122 = load i32, ptr @"G1&", !tbaa !2
  %123 = load i32, ptr @"G2&", !tbaa !2
  %124 = icmp sge i32 %122, %123
  %125 = sext i1 %124 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %125)
  %126 = load i32, ptr @"G2&", !tbaa !2
  %127 = load i32, ptr @"G1&", !tbaa !2
  %128 = icmp sge i32 %126, %127
  %129 = sext i1 %128 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %129)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string36$descriptor)
  %130 = load i32, ptr @"G1&", !tbaa !2
  %131 = load i32, ptr @"G2&", !tbaa !2
  %132 = icmp eq i32 %130, %131
  %133 = sext i1 %132 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %133)
  %134 = load i32, ptr @"G2&", !tbaa !2
  %135 = load i32, ptr @"G1&", !tbaa !2
  %136 = icmp eq i32 %134, %135
  %137 = sext i1 %136 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %137)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string38$descriptor)
  %138 = load i32, ptr @"G1&", !tbaa !2
  %139 = load i32, ptr @"G2&", !tbaa !2
  %140 = icmp ne i32 %138, %139
  %141 = sext i1 %140 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %141)
  %142 = load i32, ptr @"G2&", !tbaa !2
  %143 = load i32, ptr @"G1&", !tbaa !2
  %144 = icmp ne i32 %142, %143
  %145 = sext i1 %144 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %145)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string40$descriptor)
  ret void
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
