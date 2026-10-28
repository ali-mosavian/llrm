target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"E1&" = internal global [4 x i8] zeroinitializer
@"E2&" = internal global [4 x i8] zeroinitializer
@"F1&" = internal global [4 x i8] zeroinitializer
@"F2&" = internal global [4 x i8] zeroinitializer
@"G1&" = internal global [4 x i8] zeroinitializer
@"G2&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"ELT=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"ELE=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"EGT=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"EGE=" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"EEQ=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"ENE=" }>
@$string9 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string9, i16 4), [4 x i8] c"FLT=" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"FLE=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string11, i16 4), [4 x i8] c"FGT=" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"FGE=" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"FEQ=" }>
@$string14 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string14, i16 4), [4 x i8] c"FNE=" }>
@$string15 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string15, i16 4), [4 x i8] c"GLT=" }>
@$string16 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string16, i16 4), [4 x i8] c"GLE=" }>
@$string17 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string17, i16 4), [4 x i8] c"GGT=" }>
@$string18 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string18, i16 4), [4 x i8] c"GGE=" }>
@$string19 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string19, i16 4), [4 x i8] c"GEQ=" }>
@$string20 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string20, i16 4), [4 x i8] c"GNE=" }>
@$string21 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string21, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 305430527, ptr @"E1&", !tbaa !2
  store i32 305430528, ptr @"E2&", !tbaa !2
  store i32 2147450879, ptr @"F1&", !tbaa !2
  store i32 2147450880, ptr @"F2&", !tbaa !2
  store i32 -268402689, ptr @"G1&", !tbaa !2
  store i32 -268402688, ptr @"G2&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %0 = load i32, ptr @"E1&", !tbaa !2
  %1 = load i32, ptr @"E2&", !tbaa !2
  %2 = icmp slt i32 %0, %1
  %3 = sext i1 %2 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %3)
  %4 = load i32, ptr @"E2&", !tbaa !2
  %5 = load i32, ptr @"E1&", !tbaa !2
  %6 = icmp slt i32 %4, %5
  %7 = sext i1 %6 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %7)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %8 = load i32, ptr @"E1&", !tbaa !2
  %9 = load i32, ptr @"E2&", !tbaa !2
  %10 = icmp sle i32 %8, %9
  %11 = sext i1 %10 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %11)
  %12 = load i32, ptr @"E2&", !tbaa !2
  %13 = load i32, ptr @"E1&", !tbaa !2
  %14 = icmp sle i32 %12, %13
  %15 = sext i1 %14 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %15)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %16 = load i32, ptr @"E1&", !tbaa !2
  %17 = load i32, ptr @"E2&", !tbaa !2
  %18 = icmp sgt i32 %16, %17
  %19 = sext i1 %18 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %19)
  %20 = load i32, ptr @"E2&", !tbaa !2
  %21 = load i32, ptr @"E1&", !tbaa !2
  %22 = icmp sgt i32 %20, %21
  %23 = sext i1 %22 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %23)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %24 = load i32, ptr @"E1&", !tbaa !2
  %25 = load i32, ptr @"E2&", !tbaa !2
  %26 = icmp sge i32 %24, %25
  %27 = sext i1 %26 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %27)
  %28 = load i32, ptr @"E2&", !tbaa !2
  %29 = load i32, ptr @"E1&", !tbaa !2
  %30 = icmp sge i32 %28, %29
  %31 = sext i1 %30 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %31)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %32 = load i32, ptr @"E1&", !tbaa !2
  %33 = load i32, ptr @"E2&", !tbaa !2
  %34 = icmp eq i32 %32, %33
  %35 = sext i1 %34 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %35)
  %36 = load i32, ptr @"E2&", !tbaa !2
  %37 = load i32, ptr @"E1&", !tbaa !2
  %38 = icmp eq i32 %36, %37
  %39 = sext i1 %38 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %39)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %40 = load i32, ptr @"E1&", !tbaa !2
  %41 = load i32, ptr @"E2&", !tbaa !2
  %42 = icmp ne i32 %40, %41
  %43 = sext i1 %42 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %43)
  %44 = load i32, ptr @"E2&", !tbaa !2
  %45 = load i32, ptr @"E1&", !tbaa !2
  %46 = icmp ne i32 %44, %45
  %47 = sext i1 %46 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %47)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  %48 = load i32, ptr @"F1&", !tbaa !2
  %49 = load i32, ptr @"F2&", !tbaa !2
  %50 = icmp slt i32 %48, %49
  %51 = sext i1 %50 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %51)
  %52 = load i32, ptr @"F2&", !tbaa !2
  %53 = load i32, ptr @"F1&", !tbaa !2
  %54 = icmp slt i32 %52, %53
  %55 = sext i1 %54 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %55)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  %56 = load i32, ptr @"F1&", !tbaa !2
  %57 = load i32, ptr @"F2&", !tbaa !2
  %58 = icmp sle i32 %56, %57
  %59 = sext i1 %58 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %59)
  %60 = load i32, ptr @"F2&", !tbaa !2
  %61 = load i32, ptr @"F1&", !tbaa !2
  %62 = icmp sle i32 %60, %61
  %63 = sext i1 %62 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %63)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
  %64 = load i32, ptr @"F1&", !tbaa !2
  %65 = load i32, ptr @"F2&", !tbaa !2
  %66 = icmp sgt i32 %64, %65
  %67 = sext i1 %66 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %67)
  %68 = load i32, ptr @"F2&", !tbaa !2
  %69 = load i32, ptr @"F1&", !tbaa !2
  %70 = icmp sgt i32 %68, %69
  %71 = sext i1 %70 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %71)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  %72 = load i32, ptr @"F1&", !tbaa !2
  %73 = load i32, ptr @"F2&", !tbaa !2
  %74 = icmp sge i32 %72, %73
  %75 = sext i1 %74 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %75)
  %76 = load i32, ptr @"F2&", !tbaa !2
  %77 = load i32, ptr @"F1&", !tbaa !2
  %78 = icmp sge i32 %76, %77
  %79 = sext i1 %78 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %79)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  %80 = load i32, ptr @"F1&", !tbaa !2
  %81 = load i32, ptr @"F2&", !tbaa !2
  %82 = icmp eq i32 %80, %81
  %83 = sext i1 %82 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %83)
  %84 = load i32, ptr @"F2&", !tbaa !2
  %85 = load i32, ptr @"F1&", !tbaa !2
  %86 = icmp eq i32 %84, %85
  %87 = sext i1 %86 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %87)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14)
  %88 = load i32, ptr @"F1&", !tbaa !2
  %89 = load i32, ptr @"F2&", !tbaa !2
  %90 = icmp ne i32 %88, %89
  %91 = sext i1 %90 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %91)
  %92 = load i32, ptr @"F2&", !tbaa !2
  %93 = load i32, ptr @"F1&", !tbaa !2
  %94 = icmp ne i32 %92, %93
  %95 = sext i1 %94 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %95)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15)
  %96 = load i32, ptr @"G1&", !tbaa !2
  %97 = load i32, ptr @"G2&", !tbaa !2
  %98 = icmp slt i32 %96, %97
  %99 = sext i1 %98 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %99)
  %100 = load i32, ptr @"G2&", !tbaa !2
  %101 = load i32, ptr @"G1&", !tbaa !2
  %102 = icmp slt i32 %100, %101
  %103 = sext i1 %102 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %103)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16)
  %104 = load i32, ptr @"G1&", !tbaa !2
  %105 = load i32, ptr @"G2&", !tbaa !2
  %106 = icmp sle i32 %104, %105
  %107 = sext i1 %106 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %107)
  %108 = load i32, ptr @"G2&", !tbaa !2
  %109 = load i32, ptr @"G1&", !tbaa !2
  %110 = icmp sle i32 %108, %109
  %111 = sext i1 %110 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %111)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17)
  %112 = load i32, ptr @"G1&", !tbaa !2
  %113 = load i32, ptr @"G2&", !tbaa !2
  %114 = icmp sgt i32 %112, %113
  %115 = sext i1 %114 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %115)
  %116 = load i32, ptr @"G2&", !tbaa !2
  %117 = load i32, ptr @"G1&", !tbaa !2
  %118 = icmp sgt i32 %116, %117
  %119 = sext i1 %118 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %119)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18)
  %120 = load i32, ptr @"G1&", !tbaa !2
  %121 = load i32, ptr @"G2&", !tbaa !2
  %122 = icmp sge i32 %120, %121
  %123 = sext i1 %122 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %123)
  %124 = load i32, ptr @"G2&", !tbaa !2
  %125 = load i32, ptr @"G1&", !tbaa !2
  %126 = icmp sge i32 %124, %125
  %127 = sext i1 %126 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %127)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19)
  %128 = load i32, ptr @"G1&", !tbaa !2
  %129 = load i32, ptr @"G2&", !tbaa !2
  %130 = icmp eq i32 %128, %129
  %131 = sext i1 %130 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %131)
  %132 = load i32, ptr @"G2&", !tbaa !2
  %133 = load i32, ptr @"G1&", !tbaa !2
  %134 = icmp eq i32 %132, %133
  %135 = sext i1 %134 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %135)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20)
  %136 = load i32, ptr @"G1&", !tbaa !2
  %137 = load i32, ptr @"G2&", !tbaa !2
  %138 = icmp ne i32 %136, %137
  %139 = sext i1 %138 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %139)
  %140 = load i32, ptr @"G2&", !tbaa !2
  %141 = load i32, ptr @"G1&", !tbaa !2
  %142 = icmp ne i32 %140, %141
  %143 = sext i1 %142 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %143)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string21)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
