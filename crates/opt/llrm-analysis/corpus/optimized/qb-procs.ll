target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [24 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [6 x i8] c"\03\00AND\00" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [8 x i8] c"\05\00TWICE\00" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [8 x i8] c"\06\00NESTED" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  store i32 33818120, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3$descriptor, ptr @$data)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$data)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %0 = load i32, ptr @"R&"
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %0)
  call cc1000 addrspace(1) void @llrm.qb.B$STDL(ptr @$data)
  %1 = getelementptr i8, ptr @$data, i16 4
  call cc1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string6$descriptor, ptr %1)
  %2 = load i32, ptr @"R&"
  %3 = add i32 %2, %2
  %4 = getelementptr i8, ptr @$data, i16 8
  store i32 %3, ptr %4, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %1)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %5 = load i32, ptr %4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %5)
  call cc1000 addrspace(1) void @llrm.qb.B$STDL(ptr %1)
  %6 = getelementptr i8, ptr @$data, i16 12
  call cc1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string8$descriptor, ptr %6)
  %7 = load i32, ptr @"R&"
  %8 = add i32 %7, %7
  %9 = getelementptr i8, ptr @$data, i16 16
  store i32 %8, ptr %9, !tbaa !2
  %10 = add i32 %8, %8
  %11 = getelementptr i8, ptr @$data, i16 20
  store i32 %10, ptr %11, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %6)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %12 = load i32, ptr %11
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %12)
  call cc1000 addrspace(1) void @llrm.qb.B$STDL(ptr %6)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string10$descriptor)
  ret void
}

define cc1000 i32 @"TWICE&"(ptr %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = load i32, ptr %0
  %2 = add i32 %1, %1
  ret i32 %2
}

define cc1000 void @REPORT(ptr %0, ptr %1) addrspace(1) {
b1:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %0)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %2 = load i32, ptr %1
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  ret void
}

declare cc1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$STDL(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
