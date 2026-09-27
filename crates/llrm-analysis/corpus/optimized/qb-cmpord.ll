target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A1&" = internal global [4 x i8] zeroinitializer
@"A2&" = internal global [4 x i8] zeroinitializer
@"B1&" = internal global [4 x i8] zeroinitializer
@"B2&" = internal global [4 x i8] zeroinitializer
@"C1&" = internal global [4 x i8] zeroinitializer
@"C2&" = internal global [4 x i8] zeroinitializer
@"D1&" = internal global [4 x i8] zeroinitializer
@"D2&" = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [6 x i8] c"\04\00ALT=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00ALE=" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [6 x i8] c"\04\00AGT=" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [6 x i8] c"\04\00AGE=" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [6 x i8] c"\04\00AEQ=" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [6 x i8] c"\04\00ANE=" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [6 x i8] c"\04\00BLT=" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [6 x i8] c"\04\00BLE=" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>
@$string20$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 4) to i16), [6 x i8] c"\04\00BGT=" }>
@$string20$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 2) to i16), ptr @$fslSegment }>
@$string22$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 4) to i16), [6 x i8] c"\04\00BGE=" }>
@$string22$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 2) to i16), ptr @$fslSegment }>
@$string24$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 4) to i16), [6 x i8] c"\04\00BEQ=" }>
@$string24$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 2) to i16), ptr @$fslSegment }>
@$string26$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string26$payload, i16 4) to i16), [6 x i8] c"\04\00BNE=" }>
@$string26$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string26$payload, i16 2) to i16), ptr @$fslSegment }>
@$string28$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string28$payload, i16 4) to i16), [6 x i8] c"\04\00CLT=" }>
@$string28$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string28$payload, i16 2) to i16), ptr @$fslSegment }>
@$string30$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string30$payload, i16 4) to i16), [6 x i8] c"\04\00CLE=" }>
@$string30$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string30$payload, i16 2) to i16), ptr @$fslSegment }>
@$string32$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string32$payload, i16 4) to i16), [6 x i8] c"\04\00CGT=" }>
@$string32$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string32$payload, i16 2) to i16), ptr @$fslSegment }>
@$string34$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string34$payload, i16 4) to i16), [6 x i8] c"\04\00CGE=" }>
@$string34$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string34$payload, i16 2) to i16), ptr @$fslSegment }>
@$string36$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string36$payload, i16 4) to i16), [6 x i8] c"\04\00CEQ=" }>
@$string36$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string36$payload, i16 2) to i16), ptr @$fslSegment }>
@$string38$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string38$payload, i16 4) to i16), [6 x i8] c"\04\00CNE=" }>
@$string38$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string38$payload, i16 2) to i16), ptr @$fslSegment }>
@$string40$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string40$payload, i16 4) to i16), [6 x i8] c"\04\00DLT=" }>
@$string40$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string40$payload, i16 2) to i16), ptr @$fslSegment }>
@$string42$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string42$payload, i16 4) to i16), [6 x i8] c"\04\00DLE=" }>
@$string42$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string42$payload, i16 2) to i16), ptr @$fslSegment }>
@$string44$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string44$payload, i16 4) to i16), [6 x i8] c"\04\00DGT=" }>
@$string44$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string44$payload, i16 2) to i16), ptr @$fslSegment }>
@$string46$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string46$payload, i16 4) to i16), [6 x i8] c"\04\00DGE=" }>
@$string46$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string46$payload, i16 2) to i16), ptr @$fslSegment }>
@$string48$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string48$payload, i16 4) to i16), [6 x i8] c"\04\00DEQ=" }>
@$string48$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string48$payload, i16 2) to i16), ptr @$fslSegment }>
@$string50$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string50$payload, i16 4) to i16), [6 x i8] c"\04\00DNE=" }>
@$string50$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string50$payload, i16 2) to i16), ptr @$fslSegment }>
@$string52$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string52$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string52$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string52$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 -1, ptr @"A1&", !tbaa !2
  store i32 1, ptr @"A2&", !tbaa !2
  store i32 -2147483648, ptr @"B1&", !tbaa !2
  store i32 2147483647, ptr @"B2&", !tbaa !2
  store i32 65535, ptr @"C1&", !tbaa !2
  store i32 65536, ptr @"C2&", !tbaa !2
  store i32 305397760, ptr @"D1&", !tbaa !2
  store i32 305463295, ptr @"D2&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %0 = load i32, ptr @"A1&", !tbaa !2
  %1 = load i32, ptr @"A2&", !tbaa !2
  %2 = icmp slt i32 %0, %1
  %3 = sext i1 %2 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %3)
  %4 = load i32, ptr @"A2&", !tbaa !2
  %5 = load i32, ptr @"A1&", !tbaa !2
  %6 = icmp slt i32 %4, %5
  %7 = sext i1 %6 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %7)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %8 = load i32, ptr @"A1&", !tbaa !2
  %9 = load i32, ptr @"A2&", !tbaa !2
  %10 = icmp sle i32 %8, %9
  %11 = sext i1 %10 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %11)
  %12 = load i32, ptr @"A2&", !tbaa !2
  %13 = load i32, ptr @"A1&", !tbaa !2
  %14 = icmp sle i32 %12, %13
  %15 = sext i1 %14 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %15)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %16 = load i32, ptr @"A1&", !tbaa !2
  %17 = load i32, ptr @"A2&", !tbaa !2
  %18 = icmp sgt i32 %16, %17
  %19 = sext i1 %18 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %19)
  %20 = load i32, ptr @"A2&", !tbaa !2
  %21 = load i32, ptr @"A1&", !tbaa !2
  %22 = icmp sgt i32 %20, %21
  %23 = sext i1 %22 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %23)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10$descriptor)
  %24 = load i32, ptr @"A1&", !tbaa !2
  %25 = load i32, ptr @"A2&", !tbaa !2
  %26 = icmp sge i32 %24, %25
  %27 = sext i1 %26 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %27)
  %28 = load i32, ptr @"A2&", !tbaa !2
  %29 = load i32, ptr @"A1&", !tbaa !2
  %30 = icmp sge i32 %28, %29
  %31 = sext i1 %30 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %31)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %32 = load i32, ptr @"A1&", !tbaa !2
  %33 = load i32, ptr @"A2&", !tbaa !2
  %34 = icmp eq i32 %32, %33
  %35 = sext i1 %34 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %35)
  %36 = load i32, ptr @"A2&", !tbaa !2
  %37 = load i32, ptr @"A1&", !tbaa !2
  %38 = icmp eq i32 %36, %37
  %39 = sext i1 %38 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %39)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14$descriptor)
  %40 = load i32, ptr @"A1&", !tbaa !2
  %41 = load i32, ptr @"A2&", !tbaa !2
  %42 = icmp ne i32 %40, %41
  %43 = sext i1 %42 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %43)
  %44 = load i32, ptr @"A2&", !tbaa !2
  %45 = load i32, ptr @"A1&", !tbaa !2
  %46 = icmp ne i32 %44, %45
  %47 = sext i1 %46 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %47)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16$descriptor)
  %48 = load i32, ptr @"B1&", !tbaa !2
  %49 = load i32, ptr @"B2&", !tbaa !2
  %50 = icmp slt i32 %48, %49
  %51 = sext i1 %50 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %51)
  %52 = load i32, ptr @"B2&", !tbaa !2
  %53 = load i32, ptr @"B1&", !tbaa !2
  %54 = icmp slt i32 %52, %53
  %55 = sext i1 %54 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %55)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18$descriptor)
  %56 = load i32, ptr @"B1&", !tbaa !2
  %57 = load i32, ptr @"B2&", !tbaa !2
  %58 = icmp sle i32 %56, %57
  %59 = sext i1 %58 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %59)
  %60 = load i32, ptr @"B2&", !tbaa !2
  %61 = load i32, ptr @"B1&", !tbaa !2
  %62 = icmp sle i32 %60, %61
  %63 = sext i1 %62 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %63)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20$descriptor)
  %64 = load i32, ptr @"B1&", !tbaa !2
  %65 = load i32, ptr @"B2&", !tbaa !2
  %66 = icmp sgt i32 %64, %65
  %67 = sext i1 %66 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %67)
  %68 = load i32, ptr @"B2&", !tbaa !2
  %69 = load i32, ptr @"B1&", !tbaa !2
  %70 = icmp sgt i32 %68, %69
  %71 = sext i1 %70 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %71)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string22$descriptor)
  %72 = load i32, ptr @"B1&", !tbaa !2
  %73 = load i32, ptr @"B2&", !tbaa !2
  %74 = icmp sge i32 %72, %73
  %75 = sext i1 %74 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %75)
  %76 = load i32, ptr @"B2&", !tbaa !2
  %77 = load i32, ptr @"B1&", !tbaa !2
  %78 = icmp sge i32 %76, %77
  %79 = sext i1 %78 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %79)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string24$descriptor)
  %80 = load i32, ptr @"B1&", !tbaa !2
  %81 = load i32, ptr @"B2&", !tbaa !2
  %82 = icmp eq i32 %80, %81
  %83 = sext i1 %82 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %83)
  %84 = load i32, ptr @"B2&", !tbaa !2
  %85 = load i32, ptr @"B1&", !tbaa !2
  %86 = icmp eq i32 %84, %85
  %87 = sext i1 %86 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %87)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string26$descriptor)
  %88 = load i32, ptr @"B1&", !tbaa !2
  %89 = load i32, ptr @"B2&", !tbaa !2
  %90 = icmp ne i32 %88, %89
  %91 = sext i1 %90 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %91)
  %92 = load i32, ptr @"B2&", !tbaa !2
  %93 = load i32, ptr @"B1&", !tbaa !2
  %94 = icmp ne i32 %92, %93
  %95 = sext i1 %94 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %95)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string28$descriptor)
  %96 = load i32, ptr @"C1&", !tbaa !2
  %97 = load i32, ptr @"C2&", !tbaa !2
  %98 = icmp slt i32 %96, %97
  %99 = sext i1 %98 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %99)
  %100 = load i32, ptr @"C2&", !tbaa !2
  %101 = load i32, ptr @"C1&", !tbaa !2
  %102 = icmp slt i32 %100, %101
  %103 = sext i1 %102 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %103)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string30$descriptor)
  %104 = load i32, ptr @"C1&", !tbaa !2
  %105 = load i32, ptr @"C2&", !tbaa !2
  %106 = icmp sle i32 %104, %105
  %107 = sext i1 %106 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %107)
  %108 = load i32, ptr @"C2&", !tbaa !2
  %109 = load i32, ptr @"C1&", !tbaa !2
  %110 = icmp sle i32 %108, %109
  %111 = sext i1 %110 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %111)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string32$descriptor)
  %112 = load i32, ptr @"C1&", !tbaa !2
  %113 = load i32, ptr @"C2&", !tbaa !2
  %114 = icmp sgt i32 %112, %113
  %115 = sext i1 %114 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %115)
  %116 = load i32, ptr @"C2&", !tbaa !2
  %117 = load i32, ptr @"C1&", !tbaa !2
  %118 = icmp sgt i32 %116, %117
  %119 = sext i1 %118 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %119)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string34$descriptor)
  %120 = load i32, ptr @"C1&", !tbaa !2
  %121 = load i32, ptr @"C2&", !tbaa !2
  %122 = icmp sge i32 %120, %121
  %123 = sext i1 %122 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %123)
  %124 = load i32, ptr @"C2&", !tbaa !2
  %125 = load i32, ptr @"C1&", !tbaa !2
  %126 = icmp sge i32 %124, %125
  %127 = sext i1 %126 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %127)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string36$descriptor)
  %128 = load i32, ptr @"C1&", !tbaa !2
  %129 = load i32, ptr @"C2&", !tbaa !2
  %130 = icmp eq i32 %128, %129
  %131 = sext i1 %130 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %131)
  %132 = load i32, ptr @"C2&", !tbaa !2
  %133 = load i32, ptr @"C1&", !tbaa !2
  %134 = icmp eq i32 %132, %133
  %135 = sext i1 %134 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %135)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string38$descriptor)
  %136 = load i32, ptr @"C1&", !tbaa !2
  %137 = load i32, ptr @"C2&", !tbaa !2
  %138 = icmp ne i32 %136, %137
  %139 = sext i1 %138 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %139)
  %140 = load i32, ptr @"C2&", !tbaa !2
  %141 = load i32, ptr @"C1&", !tbaa !2
  %142 = icmp ne i32 %140, %141
  %143 = sext i1 %142 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %143)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string40$descriptor)
  %144 = load i32, ptr @"D1&", !tbaa !2
  %145 = load i32, ptr @"D2&", !tbaa !2
  %146 = icmp slt i32 %144, %145
  %147 = sext i1 %146 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %147)
  %148 = load i32, ptr @"D2&", !tbaa !2
  %149 = load i32, ptr @"D1&", !tbaa !2
  %150 = icmp slt i32 %148, %149
  %151 = sext i1 %150 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %151)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string42$descriptor)
  %152 = load i32, ptr @"D1&", !tbaa !2
  %153 = load i32, ptr @"D2&", !tbaa !2
  %154 = icmp sle i32 %152, %153
  %155 = sext i1 %154 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %155)
  %156 = load i32, ptr @"D2&", !tbaa !2
  %157 = load i32, ptr @"D1&", !tbaa !2
  %158 = icmp sle i32 %156, %157
  %159 = sext i1 %158 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %159)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string44$descriptor)
  %160 = load i32, ptr @"D1&", !tbaa !2
  %161 = load i32, ptr @"D2&", !tbaa !2
  %162 = icmp sgt i32 %160, %161
  %163 = sext i1 %162 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %163)
  %164 = load i32, ptr @"D2&", !tbaa !2
  %165 = load i32, ptr @"D1&", !tbaa !2
  %166 = icmp sgt i32 %164, %165
  %167 = sext i1 %166 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %167)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string46$descriptor)
  %168 = load i32, ptr @"D1&", !tbaa !2
  %169 = load i32, ptr @"D2&", !tbaa !2
  %170 = icmp sge i32 %168, %169
  %171 = sext i1 %170 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %171)
  %172 = load i32, ptr @"D2&", !tbaa !2
  %173 = load i32, ptr @"D1&", !tbaa !2
  %174 = icmp sge i32 %172, %173
  %175 = sext i1 %174 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %175)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string48$descriptor)
  %176 = load i32, ptr @"D1&", !tbaa !2
  %177 = load i32, ptr @"D2&", !tbaa !2
  %178 = icmp eq i32 %176, %177
  %179 = sext i1 %178 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %179)
  %180 = load i32, ptr @"D2&", !tbaa !2
  %181 = load i32, ptr @"D1&", !tbaa !2
  %182 = icmp eq i32 %180, %181
  %183 = sext i1 %182 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %183)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string50$descriptor)
  %184 = load i32, ptr @"D1&", !tbaa !2
  %185 = load i32, ptr @"D2&", !tbaa !2
  %186 = icmp ne i32 %184, %185
  %187 = sext i1 %186 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %187)
  %188 = load i32, ptr @"D2&", !tbaa !2
  %189 = load i32, ptr @"D1&", !tbaa !2
  %190 = icmp ne i32 %188, %189
  %191 = sext i1 %190 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %191)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string52$descriptor)
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
