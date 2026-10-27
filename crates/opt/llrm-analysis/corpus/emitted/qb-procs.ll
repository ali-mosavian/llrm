target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"AND\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"TWICE\00" }>
@$arguments$10 = internal global [12 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"NESTED" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"=\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  %0 = load i32, ptr @"A&", !tbaa !2
  %1 = load i32, ptr @"B&", !tbaa !2
  %2 = and i32 %0, %1
  store i32 %2, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @$data)
  call cc 1000 addrspace(1) void @REPORT(ptr @$data, ptr @"R&")
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr @$data)
  %3 = getelementptr i8, ptr @$data, i16 4
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string4, ptr %3)
  %4 = call cc 1000 addrspace(1) i32 @"TWICE&"(ptr @"R&")
  store i32 %4, ptr @$arguments$10, !tbaa !2
  call cc 1000 addrspace(1) void @REPORT(ptr %3, ptr @$arguments$10)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %3)
  %5 = getelementptr i8, ptr @$data, i16 8
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string6, ptr %5)
  %6 = call cc 1000 addrspace(1) i32 @"TWICE&"(ptr @"R&")
  %7 = getelementptr i8, ptr @$arguments$10, i16 4
  store i32 %6, ptr %7, !tbaa !2
  %8 = getelementptr i8, ptr @$arguments$10, i16 4
  %9 = call cc 1000 addrspace(1) i32 @"TWICE&"(ptr %8)
  %10 = getelementptr i8, ptr @$arguments$10, i16 8
  store i32 %9, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$arguments$10, i16 8
  call cc 1000 addrspace(1) void @REPORT(ptr %5, ptr %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %5)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void
}

define cc 1000 i32 @"TWICE&"(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = alloca i32
  %3 = load i32, ptr %0
  %4 = load i32, ptr %0
  %5 = add i32 %3, %4
  store i32 %5, ptr %1, !tbaa !2
  %6 = load i32, ptr %1, !tbaa !2
  store i32 %6, ptr %2, !tbaa !2
  br label %b2

b2:
  %7 = load i32, ptr %2, !tbaa !2
  ret i32 %7
}

define cc 1000 void @REPORT(ptr dereferenceable(4) %0, ptr dereferenceable(4) %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %2 = load i32, ptr %1
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  ret void
}

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$STDL(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
