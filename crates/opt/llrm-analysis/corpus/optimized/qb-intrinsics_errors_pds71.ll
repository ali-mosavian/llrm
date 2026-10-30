target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"CAUGHT%" = internal global [2 x i8] zeroinitializer
@P$ = internal global [4 x i8] zeroinitializer
@E$ = internal global [4 x i8] zeroinitializer
@"V%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"abc\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"EMPTY=" }>
@$arguments$10 = internal global [6 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"ZERO=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"PAST=\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"LAST=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string9, i16 4), [4 x i8] c"NEG=" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"BIG=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string11, i16 4), [8 x i8] c"NEGCHR=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @P$)
          to label %b4 unwind label %landing

b4:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i16 0, ptr @"V%", !tbaa !2
  store i16 1, ptr %site
  %0 = invoke cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr @E$)
          to label %26 unwind label %landing

b7:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %27 unwind label %landing

b8:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i16 0, ptr @"V%", !tbaa !2
  store i16 0, ptr @$arguments$10, !tbaa !2
  store i16 3, ptr %site
  %1 = invoke cc 1000 addrspace(1) i16 @"PROBE%"(ptr @P$, ptr @$arguments$10)
          to label %31 unwind label %landing

b11:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %32 unwind label %landing

b12:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i16 0, ptr @"V%", !tbaa !2
  store i16 4, ptr getelementptr inbounds nuw (i8, ptr @$arguments$10, i16 2), !tbaa !2
  store i16 5, ptr %site
  %2 = invoke cc 1000 addrspace(1) i16 @"PROBE%"(ptr @P$, ptr getelementptr inbounds nuw (i8, ptr @$arguments$10, i16 2))
          to label %36 unwind label %landing

b15:
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %37 unwind label %landing

b16:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i16 0, ptr @"V%", !tbaa !2
  store i16 3, ptr getelementptr inbounds nuw (i8, ptr @$arguments$10, i16 4), !tbaa !2
  store i16 7, ptr %site
  %3 = invoke cc 1000 addrspace(1) i16 @"PROBE%"(ptr @P$, ptr getelementptr inbounds nuw (i8, ptr @$arguments$10, i16 4))
          to label %41 unwind label %landing

b19:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %42 unwind label %landing

b20:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i16 0, ptr @"V%", !tbaa !2
  store i16 9, ptr %site
  %4 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$FMID(ptr @P$, i16 2, i16 -1)
          to label %46 unwind label %landing

b23:
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %49 unwind label %landing

b24:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i16 0, ptr @"V%", !tbaa !2
  store i16 11, ptr %site
  %5 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$FCHR(i16 256)
          to label %53 unwind label %landing

b28:
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %56 unwind label %landing

b29:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i16 0, ptr @"V%", !tbaa !2
  store i16 13, ptr %site
  %6 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$FCHR(i16 -1)
          to label %60 unwind label %landing

b33:
  store i16 14, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
          to label %63 unwind label %landing

b34:
  store i16 15, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
          to label %b35 unwind label %landing

b35:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %7 = extractvalue { ptr, i32 } %landed, 1
  %8 = trunc i32 %7 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %8)
  %9 = load i16, ptr %site
  switch table i16 %9, label %10 [
    i16 0, label %10
    i16 1, label %11
    i16 2, label %12
    i16 3, label %13
    i16 4, label %14
    i16 5, label %15
    i16 6, label %16
    i16 7, label %17
    i16 8, label %18
    i16 9, label %19
    i16 10, label %20
    i16 11, label %21
    i16 12, label %22
    i16 13, label %23
    i16 14, label %24
    i16 15, label %25
  ]

10:
  br label %b4

11:
  br label %b7

12:
  br label %b8

13:
  br label %b11

14:
  br label %b12

15:
  br label %b15

16:
  br label %b16

17:
  br label %b19

18:
  br label %b20

19:
  br label %b23

20:
  br label %b24

21:
  br label %b28

22:
  br label %b29

23:
  br label %b33

24:
  br label %b34

25:
  br label %b35

26:
  store i16 %0, ptr @"V%", !tbaa !2
  br label %b7

27:
  %28 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %28)
          to label %29 unwind label %landing

29:
  %30 = load i16, ptr @"V%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %30)
          to label %b8 unwind label %landing

31:
  store i16 %1, ptr @"V%", !tbaa !2
  br label %b11

32:
  %33 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %33)
          to label %34 unwind label %landing

34:
  %35 = load i16, ptr @"V%", !tbaa !2
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %35)
          to label %b12 unwind label %landing

36:
  store i16 %2, ptr @"V%", !tbaa !2
  br label %b15

37:
  %38 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %38)
          to label %39 unwind label %landing

39:
  %40 = load i16, ptr @"V%", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %40)
          to label %b16 unwind label %landing

41:
  store i16 %3, ptr @"V%", !tbaa !2
  br label %b19

42:
  %43 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %43)
          to label %44 unwind label %landing

44:
  %45 = load i16, ptr @"V%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %45)
          to label %b20 unwind label %landing

46:
  store i16 9, ptr %site
  %47 = invoke cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr %4)
          to label %48 unwind label %landing

48:
  store i16 %47, ptr @"V%", !tbaa !2
  br label %b23

49:
  %50 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %50)
          to label %51 unwind label %landing

51:
  %52 = load i16, ptr @"V%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %52)
          to label %b24 unwind label %landing

53:
  store i16 11, ptr %site
  %54 = invoke cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr %5)
          to label %55 unwind label %landing

55:
  store i16 %54, ptr @"V%", !tbaa !2
  br label %b28

56:
  %57 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %57)
          to label %58 unwind label %landing

58:
  %59 = load i16, ptr @"V%", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %59)
          to label %b29 unwind label %landing

60:
  store i16 13, ptr %site
  %61 = invoke cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr %6)
          to label %62 unwind label %landing

62:
  store i16 %61, ptr @"V%", !tbaa !2
  br label %b33

63:
  %64 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 14, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %64)
          to label %65 unwind label %landing

65:
  %66 = load i16, ptr @"V%", !tbaa !2
  store i16 14, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %66)
          to label %b34 unwind label %landing
}

define cc 1000 i16 @"PROBE%"(ptr dereferenceable(4) %0, ptr dereferenceable(2) nocapture readonly %1) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %2 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %2)
  store i16 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  %3 = load i16, ptr %1
  store i16 0, ptr %site
  %4 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$FMID(ptr nonnull %0, i16 %3, i16 1)
          to label %9 unwind label %landing

b3:
  %5 = phi i16 [ %10, %11 ], [ %8, %landing ]
  ret i16 %5

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %6 = extractvalue { ptr, i32 } %landed, 1
  %7 = trunc i32 %6 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %7)
  %8 = load i16, ptr %2, !tbaa !2
  br label %b3

9:
  store i16 0, ptr %site
  %10 = invoke cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr %4)
          to label %11 unwind label %landing

11:
  store i16 %10, ptr %2, !tbaa !2
  br label %b3
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1005 i16 @llrm.qb.B$FASC(ptr releases) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended"

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 ptr @llrm.qb.B$FMID(ptr releases, i16, i16) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended"

declare cc 1005 ptr @llrm.qb.B$FCHR(i16) addrspace(1) "llrm-fp-eval"="x87-extended"

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 poison
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
