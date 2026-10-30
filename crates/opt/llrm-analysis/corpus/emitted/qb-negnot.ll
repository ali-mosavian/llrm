target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"A=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"B=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"C=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"D=" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %0 = load i32, ptr @"A&", !tbaa !2
  %1 = xor i32 %0, -1
  %2 = sub i32 0, %1
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %3 = load i32, ptr @"A&", !tbaa !2
  %4 = load i32, ptr @"B&", !tbaa !2
  %5 = and i32 %3, %4
  %6 = xor i32 %5, -1
  %7 = sub i32 0, %6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %7)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %8 = load i32, ptr @"A&", !tbaa !2
  %9 = load i32, ptr @"B&", !tbaa !2
  %10 = or i32 %8, %9
  %11 = xor i32 %10, -1
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %12 = load i32, ptr @"A&", !tbaa !2
  %13 = load i32, ptr @"B&", !tbaa !2
  %14 = and i32 %12, %13
  %15 = sub i32 0, %14
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %15)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
