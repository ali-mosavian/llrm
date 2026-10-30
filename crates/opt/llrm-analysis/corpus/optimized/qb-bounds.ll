target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"G%" = internal global [46 x i8] zeroinitializer
@"E%" = internal global [46 x i8] zeroinitializer
@$arguments$5 = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string5, i16 4), [8 x i8] c"resumed\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"again\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"error\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"grown\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"shared" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"local\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string11, i16 4), [12 x i8] c"after print\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"loop" }>
@$string13 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string13, i16 4), [6 x i8] c"maybe\00" }>
@$string14 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string14, i16 4), [4 x i8] c"dim\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string15, i16 4), [6 x i8] c"param\00" }>
@$string16 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string16, i16 4), [4 x i8] c"pdim" }>
@$string17 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string17, i16 4), [6 x i8] c"before" }>
@$string18 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string18, i16 4), [6 x i8] c"after\00" }>
@$string19 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string19, i16 4), [2 x i8] c"s\00" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %1)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %2)
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 1, i16 2, i16 2, i16 257, ptr @"G%")
          to label %26 unwind label %landing

b3:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i32 589830, ptr @$arguments$5
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @LOCALS(ptr @$arguments$5, ptr getelementptr inbounds nuw (i8, ptr @$arguments$5, i16 2))
          to label %b5 unwind label %landing

b5:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @HANDED()
          to label %b6 unwind label %landing

b6:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 5, i16 2, i16 257, ptr @"G%")
          to label %27 unwind label %landing

b7:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %28 unwind label %landing

b9:
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 16), align 16, !tbaa !2
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 14), !tbaa !2
  %5 = add i16 %3, -1
  %6 = add i16 %5, %4
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %6)
          to label %b12 unwind label %landing

b12:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @RAISES()
          to label %b13 unwind label %landing

b13:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 4, i16 2, i16 257, ptr @"E%")
          to label %29 unwind label %landing

b14:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr @"E%")
          to label %b15 unwind label %landing

b15:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 -3, i16 3, i16 2, i16 257, ptr @"E%")
          to label %30 unwind label %landing

b16:
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %b17 unwind label %landing

b17:
  %7 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"E%", i16 16), align 16, !tbaa !2
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %7)
          to label %b20 unwind label %landing

b20:
  %8 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"E%", i16 16), align 16, !tbaa !2
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"E%", i16 14), !tbaa !2
  %10 = add i16 %8, -1
  %11 = add i16 %10, %9
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %11)
          to label %b23 unwind label %landing

b23:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %12 = extractvalue { ptr, i32 } %landed, 1
  %13 = trunc i32 %12 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %13)
  %14 = load i16, ptr %site
  switch table i16 %14, label %15 [
    i16 0, label %15
    i16 1, label %16
    i16 2, label %17
    i16 3, label %18
    i16 4, label %19
    i16 5, label %20
    i16 6, label %21
    i16 7, label %22
    i16 8, label %23
    i16 9, label %24
    i16 10, label %25
  ]

15:
  br label %b3

16:
  br label %b5

17:
  br label %b6

18:
  br label %b7

19:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %b9 unwind label %landing

20:
  br label %b12

21:
  br label %b13

22:
  br label %b14

23:
  br label %b15

24:
  br label %b16

25:
  br label %b23

26:
  store i16 2, ptr %2, !tbaa !2
  br label %b3

27:
  store i16 5, ptr %2, !tbaa !2
  br label %b7

28:
  unreachable

29:
  store i16 0, ptr %1, !tbaa !2
  store i16 4, ptr %0, !tbaa !2
  br label %b14

30:
  store i16 -3, ptr %1, !tbaa !2
  store i16 3, ptr %0, !tbaa !2
  br label %b16
}

define cc 1000 void @HANDED() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [46 x i8]
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %1)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %2)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %3)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %4)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %5)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %6)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %7)
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr nonnull %7, i8 0, i16 46, i1 false)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %6, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 5, i16 7, i16 0, i16 2, i16 2, i16 258, ptr nonnull %7)
          to label %40 unwind label %landing

b2:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @PARAM(ptr nonnull %7)
          to label %41 unwind label %landing

b3:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @GROW(ptr nonnull %7)
          to label %42 unwind label %landing

b4:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %43 unwind label %landing

b8:
  %8 = phi i16 [ %9, %b9 ], [ %15, %b10 ]
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %8)
          to label %53 unwind label %landing

b9:
  %9 = load i16, ptr %4, !tbaa !2
  br label %b8

b10:
  %10 = getelementptr inbounds nuw i8, ptr %7, i16 20
  %11 = load i16, ptr %10, !tbaa !2
  %12 = getelementptr inbounds nuw i8, ptr %7, i16 18
  %13 = load i16, ptr %12, !tbaa !2
  %14 = add i16 %11, -1
  %15 = add i16 %14, %13
  br label %b8

b14:
  %16 = phi i16 [ %17, %b15 ], [ %23, %b16 ]
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %16)
          to label %b17 unwind label %landing

b15:
  %17 = load i16, ptr %2, !tbaa !2
  br label %b14

b16:
  %18 = getelementptr inbounds nuw i8, ptr %7, i16 16
  %19 = load i16, ptr %18, !tbaa !2
  %20 = getelementptr inbounds nuw i8, ptr %7, i16 14
  %21 = load i16, ptr %20, !tbaa !2
  %22 = add i16 %19, -1
  %23 = add i16 %22, %21
  br label %b14

b17:
  store i16 0, ptr %1, !tbaa !2
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 1, i16 3, i16 2, i16 257, ptr @"G%")
          to label %63 unwind label %landing

b18:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @SHARED2()
          to label %64 unwind label %landing

b19:
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %65 unwind label %landing

b20:
  %24 = phi i16 [ %25, %b21 ], [ %29, %b22 ]
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %24)
          to label %b23 unwind label %landing

b21:
  %25 = load i16, ptr %0, !tbaa !2
  br label %b20

b22:
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 16), align 16, !tbaa !2
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 14), !tbaa !2
  %28 = add i16 %26, -1
  %29 = add i16 %28, %27
  br label %b20

b23:
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %7)
          to label %68 unwind label %landing

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %30 = extractvalue { ptr, i32 } %landed, 1
  %31 = trunc i32 %30 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %31)
  %32 = load i16, ptr %site
  switch table i16 %32, label %33 [
    i16 0, label %33
    i16 1, label %34
    i16 2, label %35
    i16 3, label %36
    i16 4, label %37
    i16 5, label %38
    i16 6, label %39
    i16 7, label %39
  ]

33:
  br label %b2

34:
  br label %b3

35:
  br label %b4

36:
  br label %b17

37:
  br label %b18

38:
  br label %b19

39:
  br label %b23

40:
  store i16 5, ptr %5, !tbaa !2
  store i16 7, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  store i16 2, ptr %6, !tbaa !2
  br label %b2

41:
  store i16 0, ptr %1, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b3

42:
  store i16 0, ptr %1, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b4

43:
  %44 = load i16, ptr %6, !tbaa !2
  %45 = icmp sgt i16 %44, 0
  %46 = load i16, ptr %5
  %47 = getelementptr inbounds nuw i8, ptr %7, i16 20
  %48 = load i16, ptr %47
  %49 = select i1 %45, i16 %46, i16 %48
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %49)
          to label %50 unwind label %landing

50:
  %51 = load i16, ptr %6, !tbaa !2
  %52 = icmp sgt i16 %51, 0
  br i1 %52, label %b9, label %b10

53:
  %54 = load i16, ptr %6, !tbaa !2
  %55 = icmp sgt i16 %54, 1
  %56 = load i16, ptr %3
  %57 = getelementptr inbounds nuw i8, ptr %7, i16 16
  %58 = load i16, ptr %57
  %59 = select i1 %55, i16 %56, i16 %58
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %59)
          to label %60 unwind label %landing

60:
  %61 = load i16, ptr %6, !tbaa !2
  %62 = icmp sgt i16 %61, 1
  br i1 %62, label %b15, label %b16

63:
  store i16 3, ptr %0, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b18

64:
  store i16 0, ptr %1, !tbaa !2
  br label %b19

65:
  %66 = load i16, ptr %1, !tbaa !2
  %67 = icmp sgt i16 %66, 0
  br i1 %67, label %b21, label %b22

68:
  ret void
}

define cc 1000 void @GROW(ptr dereferenceable(18) %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 -1, i16 1, i16 4, i16 6, i16 2, i16 258, ptr nonnull %0)
          to label %b2 unwind label %landing

b2:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %2)
  br label %b2
}

define cc 1000 void @LOCALS(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i32
  %22 = alloca [46 x i8]
  %23 = alloca [46 x i8]
  %24 = alloca [18 x i8]
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %2)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %3)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %4)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %5)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %6)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %7)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %8)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %9)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %10)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %11)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %12)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %13)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %14)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %15)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %16)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %17)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %18)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %19)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %20)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %21)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %22)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %23)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %24)
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i32 0, ptr %21
  call void @llvm.memset.p0.i16(ptr nonnull %22, i8 0, i16 46, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %23, i8 0, i16 46, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %24, i8 0, i16 18, i1 false)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %20, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 10, i16 99, i16 2, i16 257, ptr nonnull %24)
          to label %125 unwind label %landing

b2:
  %25 = load i16, ptr %0
  %26 = load i16, ptr %1
  store i16 0, ptr %17, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 %25, i16 5, i16 %26, i16 2, i16 258, ptr nonnull %23)
          to label %126 unwind label %landing

b3:
  %27 = load i16, ptr %20, !tbaa !2
  %28 = icmp sgt i16 %27, 0
  br i1 %28, label %b8, label %32

b8:
  %29 = load i16, ptr %19, !tbaa !2
  %30 = load i16, ptr %18, !tbaa !2
  %31 = icmp sgt i16 %29, %30
  br i1 %31, label %b15, label %32

32:
  %33 = phi i16 [ %30, %b8 ], [ 99, %b3 ]
  %34 = phi i16 [ %29, %b8 ], [ 10, %b3 ]
  %35 = sext i16 %34 to i32
  %36 = add nsw i32 %35, 1
  %37 = sub i16 %33, %34
  %38 = zext i16 %37 to i32
  %39 = mul nsw i32 %36, %38
  %40 = add nsw i32 %39, %35
  %41 = xor i16 %34, -1
  %42 = add i16 %33, %41
  %43 = zext i16 %42 to i32
  %44 = zext i16 %37 to i32
  %45 = mul nuw i32 %43, %44
  %46 = lshr i32 %45, 1
  %47 = add i32 %40, %46
  br label %b15

b15:
  %48 = phi i32 [ %47, %32 ], [ 0, %b8 ]
  store i32 %48, ptr %21, align 2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %127 unwind label %landing

b17:
  %49 = getelementptr inbounds nuw i8, ptr %23, i16 14
  %50 = load i16, ptr %49, !tbaa !2
  %51 = add i16 %50, 4
  %52 = icmp eq i16 %130, 1
  br i1 %52, label %b21, label %b20

b17.thread:
  %53 = load i16, ptr %13, !tbaa !2
  br label %b21

b20:
  %54 = phi i16 [ %58, %b21 ], [ 0, %b17 ]
  %55 = phi i16 [ %57, %b21 ], [ %51, %b17 ]
  %56 = sub i16 %55, %54
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %56)
          to label %132 unwind label %landing

b21:
  %57 = phi i16 [ %53, %b17.thread ], [ %51, %b17 ]
  %58 = load i16, ptr %16, !tbaa !2
  br label %b20

b29:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
          to label %145 unwind label %landing

b36:
  store i16 1, ptr %12, !tbaa !2
  store i16 3, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b37

b37:
  %59 = phi i16 [ %102, %b68 ], [ 1, %b36 ]
  %60 = phi i16 [ %101, %b68 ], [ 1, %b36 ]
  %61 = icmp sgt i16 %60, -1
  %62 = load i16, ptr %11, !tbaa !2
  br i1 %61, label %b38, label %b39

b38:
  %63 = icmp sgt i16 %59, %62
  br i1 %63, label %b41, label %b40

b39:
  %64 = icmp slt i16 %59, %62
  br i1 %64, label %b41, label %b40

b40:
  %65 = shl i16 %59, 1
  store i16 0, ptr %9, !tbaa !2
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 %59, i16 %65, i16 1, i16 %59, i16 2, i16 258, ptr nonnull %22)
          to label %186 unwind label %landing

b41:
  store i16 1, ptr %4, !tbaa !2
  store i16 2, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b69

b42:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
          to label %187 unwind label %landing

b46:
  %66 = phi i16 [ %67, %b47 ], [ %73, %b48 ]
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %66)
          to label %197 unwind label %landing

b47:
  %67 = load i16, ptr %7, !tbaa !2
  br label %b46

b48:
  %68 = getelementptr inbounds nuw i8, ptr %22, i16 20
  %69 = load i16, ptr %68, !tbaa !2
  %70 = getelementptr inbounds nuw i8, ptr %22, i16 18
  %71 = load i16, ptr %70, !tbaa !2
  %72 = add i16 %69, -1
  %73 = add i16 %72, %71
  br label %b46

b49:
  %74 = phi i16 [ %75, %b50 ], [ %81, %b51 ]
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %74)
          to label %b52 unwind label %landing

b50:
  %75 = load i16, ptr %5, !tbaa !2
  br label %b49

b51:
  %76 = getelementptr inbounds nuw i8, ptr %22, i16 16
  %77 = load i16, ptr %76, !tbaa !2
  %78 = getelementptr inbounds nuw i8, ptr %22, i16 14
  %79 = load i16, ptr %78, !tbaa !2
  %80 = add i16 %77, -1
  %81 = add i16 %80, %79
  br label %b49

b52:
  %82 = load i16, ptr %12, !tbaa !2
  %83 = icmp eq i16 %82, 2
  br i1 %83, label %b53, label %b55

b53:
  store i16 0, ptr %9, !tbaa !2
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 -2, i16 2, i16 0, i16 1, i16 2, i16 258, ptr nonnull %22)
          to label %200 unwind label %landing

b55:
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
          to label %201 unwind label %landing

b59:
  %84 = phi i16 [ %85, %b60 ], [ %91, %b61 ]
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %84)
          to label %211 unwind label %landing

b60:
  %85 = load i16, ptr %7, !tbaa !2
  br label %b59

b61:
  %86 = getelementptr inbounds nuw i8, ptr %22, i16 20
  %87 = load i16, ptr %86, !tbaa !2
  %88 = getelementptr inbounds nuw i8, ptr %22, i16 18
  %89 = load i16, ptr %88, !tbaa !2
  %90 = add i16 %87, -1
  %91 = add i16 %90, %89
  br label %b59

b65:
  %92 = phi i16 [ %93, %b66 ], [ %99, %b67 ]
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %92)
          to label %b68 unwind label %landing

b66:
  %93 = load i16, ptr %5, !tbaa !2
  br label %b65

b67:
  %94 = getelementptr inbounds nuw i8, ptr %22, i16 16
  %95 = load i16, ptr %94, !tbaa !2
  %96 = getelementptr inbounds nuw i8, ptr %22, i16 14
  %97 = load i16, ptr %96, !tbaa !2
  %98 = add i16 %95, -1
  %99 = add i16 %98, %97
  br label %b65

b68:
  %100 = load i16, ptr %12, !tbaa !2
  %101 = load i16, ptr %10, !tbaa !2
  %102 = add nsw i16 %101, %100
  store i16 %102, ptr %12, !tbaa !2
  br label %b37

b69:
  %103 = phi i16 [ %111, %b76 ], [ 1, %b41 ]
  %104 = phi i16 [ %110, %b76 ], [ 1, %b41 ]
  %105 = icmp sgt i16 %104, -1
  %106 = load i16, ptr %3, !tbaa !2
  br i1 %105, label %b70, label %b71

b70:
  %107 = icmp sgt i16 %103, %106
  br i1 %107, label %b73, label %b72

b71:
  %108 = icmp slt i16 %103, %106
  br i1 %108, label %b73, label %b72

b72:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14)
          to label %164 unwind label %landing

b73:
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %24)
          to label %161 unwind label %landing

b76:
  %109 = load i16, ptr %4, !tbaa !2
  %110 = load i16, ptr %2, !tbaa !2
  %111 = add nsw i16 %110, %109
  store i16 %111, ptr %4, !tbaa !2
  br label %b69

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %112 = extractvalue { ptr, i32 } %landed, 1
  %113 = trunc i32 %112 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %113)
  %114 = load i16, ptr %site
  switch table i16 %114, label %115 [
    i16 0, label %115
    i16 1, label %116
    i16 2, label %117
    i16 3, label %118
    i16 4, label %119
    i16 5, label %120
    i16 6, label %121
    i16 7, label %122
    i16 8, label %123
    i16 9, label %124
  ]

115:
  br label %b2

116:
  br label %b3

117:
  br label %b29

118:
  br label %b36

119:
  br label %b42

120:
  br label %b52

121:
  br label %b55

122:
  br label %b68

123:
  br label %b76

124:
  br label %b73

125:
  store i16 10, ptr %19, !tbaa !2
  store i16 99, ptr %18, !tbaa !2
  store i16 1, ptr %20, !tbaa !2
  br label %b2

126:
  store i16 0, ptr %16, !tbaa !2
  store i16 %25, ptr %15, !tbaa !2
  store i16 5, ptr %14, !tbaa !2
  store i16 %26, ptr %13, !tbaa !2
  store i16 2, ptr %17, !tbaa !2
  br label %b3

127:
  %128 = load i32, ptr %21, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %128)
          to label %129 unwind label %landing

129:
  %130 = load i16, ptr %17, !tbaa !2
  %131 = icmp sgt i16 %130, 1
  br i1 %131, label %b17.thread, label %b17

132:
  %133 = load i16, ptr %17, !tbaa !2
  %134 = icmp sgt i16 %133, 0
  %135 = load i16, ptr %15
  %136 = getelementptr inbounds nuw i8, ptr %23, i16 18
  %137 = load i16, ptr %136
  %138 = add i16 %137, -1
  %139 = select i1 %134, i16 %135, i16 %138
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %139)
          to label %140 unwind label %landing

140:
  %141 = load i16, ptr %17, !tbaa !2
  %142 = icmp sgt i16 %141, 1
  %143 = load i16, ptr %14
  %144 = select i1 %142, i16 %143, i16 5
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %144)
          to label %b29 unwind label %landing

145:
  %146 = load i16, ptr %17, !tbaa !2
  %147 = icmp sgt i16 %146, 0
  %148 = load i16, ptr %15
  %149 = getelementptr inbounds nuw i8, ptr %23, i16 18
  %150 = load i16, ptr %149
  %151 = add i16 %150, -1
  %152 = select i1 %147, i16 %148, i16 %151
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %152)
          to label %153 unwind label %landing

153:
  %154 = load i16, ptr %17, !tbaa !2
  %155 = icmp sgt i16 %154, 1
  %156 = load i16, ptr %13
  %157 = getelementptr inbounds nuw i8, ptr %23, i16 14
  %158 = load i16, ptr %157
  %159 = add i16 %158, 4
  %160 = select i1 %155, i16 %156, i16 %159
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %160)
          to label %b36 unwind label %landing

161:
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %23)
          to label %162 unwind label %landing

162:
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %22)
          to label %163 unwind label %landing

163:
  ret void

164:
  %165 = load i16, ptr %4, !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %165)
          to label %166 unwind label %landing

166:
  %167 = load i16, ptr %4, !tbaa !2
  %168 = shl i16 %167, 2
  %169 = sub i16 0, %168
  %170 = getelementptr i8, ptr %23, i16 8
  %171 = getelementptr i8, ptr %170, i16 %169
  %172 = getelementptr i8, ptr %171, i16 16
  %173 = load i16, ptr %172
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %173)
          to label %174 unwind label %landing

174:
  %175 = load i16, ptr %4, !tbaa !2
  %176 = shl i16 %175, 2
  %177 = sub i16 0, %176
  %178 = getelementptr i8, ptr %23, i16 8
  %179 = getelementptr i8, ptr %178, i16 %177
  %180 = getelementptr i8, ptr %179, i16 16
  %181 = load i16, ptr %180
  %182 = getelementptr i8, ptr %179, i16 14
  %183 = load i16, ptr %182
  %184 = add i16 %181, -1
  %185 = add i16 %184, %183
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %185)
          to label %b76 unwind label %landing

186:
  store i16 %59, ptr %8, !tbaa !2
  store i16 %65, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  store i16 %59, ptr %5, !tbaa !2
  store i16 2, ptr %9, !tbaa !2
  br label %b42

187:
  %188 = load i16, ptr %9, !tbaa !2
  %189 = icmp sgt i16 %188, 0
  %190 = load i16, ptr %8
  %191 = getelementptr inbounds nuw i8, ptr %22, i16 20
  %192 = load i16, ptr %191
  %193 = select i1 %189, i16 %190, i16 %192
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %193)
          to label %194 unwind label %landing

194:
  %195 = load i16, ptr %9, !tbaa !2
  %196 = icmp sgt i16 %195, 0
  br i1 %196, label %b47, label %b48

197:
  %198 = load i16, ptr %9, !tbaa !2
  %199 = icmp sgt i16 %198, 1
  br i1 %199, label %b50, label %b51

200:
  store i16 -2, ptr %8, !tbaa !2
  store i16 2, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  store i16 2, ptr %9, !tbaa !2
  br label %b55

201:
  %202 = load i16, ptr %9, !tbaa !2
  %203 = icmp sgt i16 %202, 0
  %204 = load i16, ptr %8
  %205 = getelementptr inbounds nuw i8, ptr %22, i16 20
  %206 = load i16, ptr %205
  %207 = select i1 %203, i16 %204, i16 %206
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %207)
          to label %208 unwind label %landing

208:
  %209 = load i16, ptr %9, !tbaa !2
  %210 = icmp sgt i16 %209, 0
  br i1 %210, label %b60, label %b61

211:
  %212 = load i16, ptr %9, !tbaa !2
  %213 = icmp sgt i16 %212, 1
  %214 = load i16, ptr %6
  %215 = getelementptr inbounds nuw i8, ptr %22, i16 16
  %216 = load i16, ptr %215
  %217 = select i1 %213, i16 %214, i16 %216
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %217)
          to label %218 unwind label %landing

218:
  %219 = load i16, ptr %9, !tbaa !2
  %220 = icmp sgt i16 %219, 1
  br i1 %220, label %b66, label %b67
}

define cc 1000 void @PARAM(ptr dereferenceable(18) nocapture readonly %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [2 x i8]
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %1)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %2)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %3)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %4)
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr nonnull %4, i8 0, i16 2, i1 false)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15)
          to label %b2 unwind label %landing

b2:
  %5 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %6 = load i8, ptr %5
  %7 = zext i8 %6 to i16
  %8 = shl nuw nsw i16 %7, 2
  %9 = getelementptr i8, ptr %0, i16 %8
  %10 = getelementptr i8, ptr %9, i16 12
  %11 = load i16, ptr %10
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %11)
          to label %b5 unwind label %landing

b5:
  %12 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %13 = load i8, ptr %12
  %14 = zext i8 %13 to i16
  %15 = shl nuw nsw i16 %14, 2
  %16 = getelementptr i8, ptr %0, i16 %15
  %17 = getelementptr i8, ptr %16, i16 12
  %18 = load i16, ptr %17
  %19 = add i16 %18, -1
  %20 = getelementptr i8, ptr %16, i16 10
  %21 = load i16, ptr %20
  %22 = add i16 %19, %21
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %22)
          to label %b8 unwind label %landing

b8:
  %23 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %24 = load i8, ptr %23
  %25 = zext i8 %24 to i16
  %26 = shl nuw nsw i16 %25, 2
  %27 = getelementptr i8, ptr %0, i16 %26
  %28 = getelementptr i8, ptr %27, i16 8
  %29 = load i16, ptr %28
  store i16 %29, ptr %4, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %29)
          to label %b11 unwind label %landing

b11:
  %30 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %31 = load i8, ptr %30
  %32 = zext i8 %31 to i16
  %33 = shl nuw nsw i16 %32, 2
  %34 = getelementptr i8, ptr %0, i16 %33
  %35 = getelementptr i8, ptr %34, i16 8
  %36 = load i16, ptr %35
  %37 = add i16 %36, -1
  %38 = getelementptr i8, ptr %34, i16 6
  %39 = load i16, ptr %38
  %40 = add i16 %37, %39
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %40)
          to label %b14 unwind label %landing

b14:
  store i16 1, ptr %3, !tbaa !2
  store i16 2, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b15

b15:
  %41 = phi i16 [ %49, %b22 ], [ 1, %b14 ]
  %42 = phi i16 [ %48, %b22 ], [ 1, %b14 ]
  %43 = icmp sgt i16 %42, -1
  %44 = load i16, ptr %2, !tbaa !2
  br i1 %43, label %b16, label %b17

b16:
  %45 = icmp sgt i16 %41, %44
  br i1 %45, label %b19, label %b18

b17:
  %46 = icmp slt i16 %41, %44
  br i1 %46, label %b19, label %b18

b18:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16)
          to label %54 unwind label %landing

b19:
  ret void

b22:
  %47 = load i16, ptr %3, !tbaa !2
  %48 = load i16, ptr %1, !tbaa !2
  %49 = add nsw i16 %48, %47
  store i16 %49, ptr %3, !tbaa !2
  br label %b15

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %50 = extractvalue { ptr, i32 } %landed, 1
  %51 = trunc i32 %50 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %51)
  %52 = load i16, ptr %site
  %53 = icmp eq i16 %52, 0
  br i1 %53, label %b14, label %b22

54:
  %55 = load i16, ptr %3, !tbaa !2
  %56 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %57 = load i8, ptr %56
  %58 = zext i8 %57 to i16
  %59 = sub i16 %58, %55
  %60 = shl i16 %59, 2
  %61 = getelementptr i8, ptr %0, i16 %60
  %62 = getelementptr i8, ptr %61, i16 16
  %63 = load i16, ptr %62
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %63)
          to label %64 unwind label %landing

64:
  %65 = load i16, ptr %3, !tbaa !2
  %66 = getelementptr i8, ptr %0, i16 8
  %67 = load i8, ptr %66
  %68 = zext i8 %67 to i16
  %69 = sub i16 %68, %65
  %70 = shl i16 %69, 2
  %71 = getelementptr i8, ptr %0, i16 %70
  %72 = getelementptr i8, ptr %71, i16 16
  %73 = load i16, ptr %72
  %74 = getelementptr i8, ptr %71, i16 14
  %75 = load i16, ptr %74
  %76 = add i16 %73, -1
  %77 = add i16 %76, %75
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %77)
          to label %b22 unwind label %landing
}

define cc 1000 void @RAISES() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca [2 x i8]
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  call void @llvm.memset.p0.i16(ptr nonnull %0, i8 0, i16 2, i1 false)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17)
          to label %b2 unwind label %landing

b2:
  %1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 16), align 16, !tbaa !2
  %2 = add i16 %1, -1
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 14), !tbaa !2
  %4 = add i16 %2, %3
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %4)
          to label %b5 unwind label %landing

b5:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %17 unwind label %landing

b6:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18)
          to label %b7 unwind label %landing

b7:
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 16), align 16, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %5)
          to label %b10 unwind label %landing

b10:
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 16), align 16, !tbaa !2
  %7 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 14), !tbaa !2
  %8 = add i16 %6, -1
  %9 = add i16 %8, %7
  store i16 %9, ptr %0, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %9)
          to label %b13 unwind label %landing

b13:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %10 = extractvalue { ptr, i32 } %landed, 1
  %11 = trunc i32 %10 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %11)
  %12 = load i16, ptr %site
  %13 = icmp eq i16 %12, 0
  br i1 %13, label %b5, label %14

14:
  %15 = icmp eq i16 %12, 1
  br i1 %15, label %b6, label %16

16:
  br label %b13

17:
  unreachable
}

define cc 1000 void @SHARED2() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca [2 x i8]
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  call void @llvm.memset.p0.i16(ptr nonnull %0, i8 0, i16 2, i1 false)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19)
          to label %b2 unwind label %landing

b2:
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 3)
          to label %b5 unwind label %landing

b5:
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 4)
          to label %b8 unwind label %landing

b8:
  store i16 -2, ptr %0, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -2)
          to label %b11 unwind label %landing

b11:
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 8)
          to label %b14 unwind label %landing

b14:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 7, i16 9, i16 2, i16 257, ptr @"G%")
          to label %b15 unwind label %landing

b15:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %2)
  %3 = load i16, ptr %site
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b14, label %b15
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1)

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  %1 = icmp eq i16 %0, 6
  br i1 %1, label %b29, label %2

2:
  %3 = icmp eq i16 %0, 5
  br i1 %3, label %b26, label %b31

b26:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 1, i16 12, i16 2, i16 257, ptr @"G%")
  br label %b31

b29:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 2, i16 13, i16 2, i16 257, ptr @"G%")
  br label %b31

b31:
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 poison
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
