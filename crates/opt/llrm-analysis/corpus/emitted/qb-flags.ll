target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string3, i16 4), [10 x i8] c"SPLIT=zero" }>
@$string4 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0D\00", ptr getelementptr (i8, ptr @$string4, i16 4), [14 x i8] c"SPLIT=nonzero\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string5, i16 4), [10 x i8] c"FUSED=zero" }>
@$string6 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0D\00", ptr getelementptr (i8, ptr @$string6, i16 4), [14 x i8] c"FUSED=nonzero\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string7, i16 4), [12 x i8] c"MIRROR=zero\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0E\00", ptr getelementptr (i8, ptr @$string8, i16 4), [14 x i8] c"MIRROR=nonzero" }>
@$string9 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string9, i16 4), [10 x i8] c"BOTH=zero\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string10, i16 4), [12 x i8] c"BOTH=nonzero" }>
@$string11 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string11, i16 4), [8 x i8] c"SIGN=neg" }>
@$string12 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string12, i16 4), [8 x i8] c"SIGN=pos" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 65535, ptr @"A&", !tbaa !2
  store i32 61680, ptr @"B&", !tbaa !2
  %0 = load i32, ptr @"A&", !tbaa !2
  %1 = load i32, ptr @"B&", !tbaa !2
  %2 = and i32 %0, %1
  store i32 %2, ptr @"R&", !tbaa !2
  %3 = load i32, ptr @"R&", !tbaa !2
  %4 = icmp eq i32 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  br label %b4

b3:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  br label %b4

b4:
  %5 = load i32, ptr @"A&", !tbaa !2
  %6 = load i32, ptr @"B&", !tbaa !2
  %7 = and i32 %5, %6
  %8 = icmp eq i32 %7, 0
  br i1 %8, label %b5, label %b6

b5:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  br label %b7

b7:
  store i32 -65536, ptr @"A&", !tbaa !2
  store i32 -65536, ptr @"B&", !tbaa !2
  %9 = load i32, ptr @"A&", !tbaa !2
  %10 = load i32, ptr @"B&", !tbaa !2
  %11 = and i32 %9, %10
  store i32 %11, ptr @"R&", !tbaa !2
  %12 = load i32, ptr @"R&", !tbaa !2
  %13 = icmp eq i32 %12, 0
  br i1 %13, label %b8, label %b9

b8:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  br label %b10

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
  br label %b10

b10:
  store i32 0, ptr @"A&", !tbaa !2
  store i32 0, ptr @"B&", !tbaa !2
  %14 = load i32, ptr @"A&", !tbaa !2
  %15 = load i32, ptr @"B&", !tbaa !2
  %16 = and i32 %14, %15
  store i32 %16, ptr @"R&", !tbaa !2
  %17 = load i32, ptr @"R&", !tbaa !2
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %b11, label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string9)
  br label %b13

b12:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string10)
  br label %b13

b13:
  store i32 65536, ptr @"A&", !tbaa !2
  store i32 1, ptr @"B&", !tbaa !2
  %19 = load i32, ptr @"A&", !tbaa !2
  %20 = load i32, ptr @"B&", !tbaa !2
  %21 = sub i32 %19, %20
  store i32 %21, ptr @"R&", !tbaa !2
  %22 = load i32, ptr @"R&", !tbaa !2
  %23 = icmp slt i32 %22, 0
  br i1 %23, label %b14, label %b15

b14:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11)
  br label %b16

b15:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
  br label %b16

b16:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string13)
  ret void
}

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
