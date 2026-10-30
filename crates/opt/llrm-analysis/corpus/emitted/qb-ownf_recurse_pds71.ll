target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"T&" = internal global [4 x i8] zeroinitializer
@$arguments$2 = internal global [6 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 15, ptr @$arguments$2, !tbaa !2
  %0 = call cc 1000 addrspace(1) i16 @"FIB%"(ptr @$arguments$2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  %1 = getelementptr i8, ptr @$arguments$2, i16 2
  store i16 30, ptr %1, !tbaa !2
  %2 = getelementptr i8, ptr @$arguments$2, i16 2
  %3 = call cc 1000 addrspace(1) i32 @"DEPTH&"(ptr %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %3)
  store i32 0, ptr @"T&", !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$2, i16 4
  store i16 20, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$arguments$2, i16 4
  call cc 1000 addrspace(1) void @DOWN(ptr %5, ptr @"T&")
  %6 = load i32, ptr @"T&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %6)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 i16 @"FIB%"(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = load i16, ptr %0
  %5 = icmp slt i16 %4, 2
  br i1 %5, label %b2, label %b3

b2:
  %6 = load i16, ptr %0
  store i16 %6, ptr %3, !tbaa !2
  br label %b4

b3:
  %7 = load i16, ptr %0
  %8 = sub i16 %7, 1
  store i16 %8, ptr %2, !tbaa !2
  %9 = call cc 1000 addrspace(1) i16 @"FIB%"(ptr %2)
  %10 = load i16, ptr %0
  %11 = sub i16 %10, 2
  store i16 %11, ptr %1, !tbaa !2
  %12 = call cc 1000 addrspace(1) i16 @"FIB%"(ptr %1)
  %13 = add i16 %9, %12
  store i16 %13, ptr %3, !tbaa !2
  br label %b4

b4:
  br label %b5

b5:
  %14 = load i16, ptr %3, !tbaa !2
  ret i16 %14
}

define cc 1000 i32 @"DEPTH&"(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i32
  %4 = load i16, ptr %0
  %5 = sext i16 %4 to i32
  store i32 %5, ptr %2, !tbaa !2
  %6 = load i16, ptr %0
  %7 = icmp eq i16 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  store i32 0, ptr %3, !tbaa !2
  br label %b4

b3:
  %8 = load i16, ptr %0
  %9 = sub i16 %8, 1
  store i16 %9, ptr %1, !tbaa !2
  %10 = call cc 1000 addrspace(1) i32 @"DEPTH&"(ptr %1)
  %11 = load i32, ptr %2, !tbaa !2
  %12 = add i32 %10, %11
  store i32 %12, ptr %3, !tbaa !2
  br label %b4

b4:
  br label %b5

b5:
  %13 = load i32, ptr %3, !tbaa !2
  ret i32 %13
}

define cc 1000 void @DOWN(ptr dereferenceable(2) %0, ptr dereferenceable(4) %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = load i16, ptr %0
  store i16 %4, ptr %3, !tbaa !2
  %5 = load i16, ptr %0
  %6 = icmp sgt i16 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  %7 = load i16, ptr %0
  %8 = sub i16 %7, 1
  store i16 %8, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @DOWN(ptr %2, ptr %1)
  br label %b4

b3:
  br label %b4

b4:
  %9 = load i32, ptr %1
  %10 = load i16, ptr %3, !tbaa !2
  %11 = sext i16 %10 to i32
  %12 = add i32 %9, %11
  store i32 %12, ptr %1
  ret void
}

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
