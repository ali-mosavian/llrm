target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [364 x i8] c"\02\00\00\00\01\00\00\00\01\00\00\00\00\00\02\00\00\00\03\00\00\00\02\00\00\00\00\00\02\00\00\00\04\00\00\00\03\00\00\00\00\00\02\00\00\00\05\00\00\00\04\00\00\00\00\00\02\00\00\00\06\00\00\00\08\00\00\00\00\00\02\00\00\00\02\00\00\00\08\00\00\00\00\00\02\00\00\00\08\00\00\00\0C\00\00\00\00\00\02\00\00\00\09\00\00\00\0D\00\00\00\00\00\03\00\00\00\01\00\00\00\01\00\00\00\00\00\03\00\00\00\03\00\00\00\02\00\00\00\00\00\03\00\00\00\04\00\00\00\03\00\00\00\00\00\03\00\00\00\07\00\00\00\0A\00\00\00\00\00\03\00\00\00\08\00\00\00\10\00\00\00\00\00\03\00\00\00\02\00\00\00\10\00\00\00\00\00\03\00\00\00\0A\00\00\00\14\00\00\00\00\00\03\00\00\00\0B\00\00\00\15\00\00\00\00\00\03\00\00\00\0C\00\00\00\16\00\00\00\00\00\04\00\00\00\01\00\00\00\01\00\00\00\00\00\04\00\00\00\04\00\00\00\02\00\00\00\00\00\04\00\00\00\05\00\00\00\11\00\00\00\00\00\04\00\00\00\06\00\00\00\12\00\00\00\00\00\04\00\00\00\02\00\00\00\12\00\00\00\00\00\04\00\00\00\08\00\00\00\16\00\00\00\00\00\04\00\00\00\03\00\00\00\16\00\00\00\00\00\04\00\00\00\0A\00\00\00\1A\00\00\00\00\00\04\00\00\00\0B\00\00\00\1B\00\00\00\00\00"
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"next" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"err\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"again\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"err\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"label\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"err\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @NXT()
  call cc 1000 addrspace(1) void @AGN()
  call cc 1000 addrspace(1) void @LAB()
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @NXT() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
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
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
  br label %b8

b3:
  store i16 7, ptr %0, !tbaa !2
  br label %b4

b4:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %4 unwind label %landing

b5:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %5 unwind label %landing

b6:
  ret void

b8:
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  %3 = load i16, ptr %site
  switch i16 %3, label %8 [
    i16 0, label %b5
    i16 1, label %b6
  ]

b9:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 false)
  br label %b2

4:
  unreachable

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

define cc 1000 void @AGN() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %site = alloca i16
  %active = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %0)
  call addrspace(1) void @llrm.qb.frame(ptr %1)
  call addrspace(1) void @llrm.qb.frame(ptr %2)
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  store i16 0, ptr %active
  br label %b1

b1:
  store i16 1, ptr %active
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  br label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %3 = extractvalue { ptr, i32 } %landed, 1
  %4 = trunc i32 %3 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %4)
  br label %b10

b3:
  store i16 20, ptr %2, !tbaa !2
  br label %b4

b4:
  %5 = load i16, ptr %0, !tbaa !2
  %6 = sext i16 %5 to i32
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %b5, label %b6

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %14 unwind label %landing

b6:
  %8 = icmp eq i32 %6, 0
  br i1 %8, label %trap, label %10

b7:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %15 unwind label %landing

b8:
  ret void

b10:
  store i16 2, ptr %0, !tbaa !2
  br label %b11

b11:
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  %9 = load i16, ptr %site
  switch i16 %9, label %20 [
    i16 0, label %b4
    i16 1, label %b7
  ]

b12:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 false)
  br label %b2

trap:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %13 unwind label %landing

10:
  %11 = sdiv i32 10, %6
  %12 = trunc i32 %11 to i16
  store i16 %12, ptr %1, !tbaa !2
  br label %b7

13:
  unreachable

14:
  unreachable

15:
  %16 = load i16, ptr %2, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %16)
          to label %17 unwind label %landing

17:
  %18 = load i16, ptr %1, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %18)
          to label %19 unwind label %landing

19:
  br label %b8

20:
  unreachable
}

define cc 1000 void @LAB() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %site = alloca i16
  %active = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %3)
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  store i16 0, ptr %active
  br label %b1

b1:
  store i16 1, ptr %active
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  br label %b4

b2:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %7 unwind label %landing

b3:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %4 = extractvalue { ptr, i32 } %landed, 1
  %5 = trunc i32 %4 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %5)
  br label %b10

b4:
  store i16 0, ptr %site
  %6 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
          to label %10 unwind label %landing

b5:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 7)
          to label %25 unwind label %landing

b6:
  ret void

b8:
  ret void

b10:
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  br label %b2

b11:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 false)
  br label %b3

7:
  %8 = load i16, ptr %3, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %8)
          to label %9 unwind label %landing

9:
  br label %b8

10:
  %11 = load float, ptr %6
  store i16 0, ptr %2, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = sitofp i16 %12 to float
  %14 = fmul float %11, %13
  %15 = call float @llvm.rint.f32(float %14)
  %16 = fcmp olt float %14, %15
  %17 = sext i1 %16 to i16
  store i16 %17, ptr %1, !tbaa !2
  %18 = load i16, ptr %1, !tbaa !2
  %19 = sitofp i16 %18 to float
  %20 = fadd float %15, %19
  store i16 41, ptr %0, !tbaa !2
  %21 = load i16, ptr %0, !tbaa !2
  %22 = sitofp i16 %21 to float
  %23 = fadd float %22, %20
  %24 = call i16 @llvm.lrint.i16.f32(float %23)
  store i16 %24, ptr %3, !tbaa !2
  br label %b5

25:
  unreachable
}

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$RND0() addrspace(1)

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onlocalerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
