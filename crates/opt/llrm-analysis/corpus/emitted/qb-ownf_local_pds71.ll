target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [126 x i8] c"\02\00\00\00\01\00\00\00\01\00\00\00\00\00\02\00\00\00\03\00\00\00\02\00\00\00\00\00\02\00\00\00\04\00\00\00\03\00\00\00\00\00\02\00\00\00\05\00\00\00\05\00\00\00\00\00\02\00\00\00\06\00\00\00\09\00\00\00\00\00\02\00\00\00\02\00\00\00\09\00\00\00\00\00\02\00\00\00\08\00\00\00\0D\00\00\00\00\00\02\00\00\00\09\00\00\00\0E\00\00\00\00\00\02\00\00\00\0A\00\00\00\0F\00\00\00\00\00"
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"done" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"top\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string5, i16 4), [8 x i8] c"handler\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"mid\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"low\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @TOP()
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @TOP() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %site = alloca i16
  %active = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %0)
  store i16 0, ptr %0
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  store i16 0, ptr %active
  br label %b1

b1:
  store i16 1, ptr %active
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  br label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
  br label %b8

b3:
  store i16 0, ptr %0, !tbaa !2
  br label %b4

b4:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @MID(ptr %0)
          to label %4 unwind label %landing

b5:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %5 unwind label %landing

b6:
  ret void

b8:
  store i16 1, ptr %0, !tbaa !2
  br label %b9

b9:
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  %3 = load i16, ptr %site
  switch i16 %3, label %8 [
    i16 0, label %b5
    i16 1, label %b6
  ]

b10:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 false)
  br label %b2

4:
  br label %b5

5:
  %6 = load i16, ptr %0, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %6)
          to label %7 unwind label %landing

7:
  br label %b6

8:
  unreachable
}

define cc 1000 void @MID(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  store i16 7, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @LOW(ptr %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %2 = load i16, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
  ret void
}

define cc 1000 void @LOW(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = load i16, ptr %0
  %3 = sext i16 %2 to i32
  %4 = icmp eq i32 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b3:
  %5 = sdiv i32 10, %3
  %6 = trunc i32 %5 to i16
  store i16 %6, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %7 = load i16, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %7)
  ret void
}

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onlocalerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
