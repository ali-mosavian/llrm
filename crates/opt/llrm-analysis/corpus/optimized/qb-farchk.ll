target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"S%" = internal global [20 x i8] zeroinitializer, align 2
@"D%" = internal global [46 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"ERR\00" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 -2, i16 5, i16 0, i16 3, i16 2, i16 258, ptr @"D%")
          to label %b4 unwind label %landing

b4:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 1, ptr getelementptr inbounds nuw (i8, ptr @"S%", i16 18), !tbaa !2
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %2 = inttoptr i16 %0 to ptr addrspace(2)
  %3 = addrspacecast ptr addrspace(2) %2 to ptr addrspace(1)
  %4 = getelementptr i8, ptr addrspace(1) %3, i16 %1
  %5 = getelementptr i8, ptr addrspace(1) %4, i16 -4
  store i16 4, ptr addrspace(1) %5, !tbaa !6
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 18), !tbaa !2
  %7 = mul i16 %6, 6
  %8 = getelementptr i8, ptr addrspace(1) %4, i16 %7
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 10
  store i16 6, ptr addrspace(1) %9, !tbaa !6
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 1)
          to label %62 unwind label %landing

b9:
  %10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"S%", i16 20), !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %10)
          to label %b10 unwind label %landing

b10:
  %11 = load i16, ptr getelementptr inbounds (i8, ptr @"S%", i16 -2), !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %11)
          to label %b11 unwind label %landing

b11:
  %12 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %14 = inttoptr i16 %12 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr addrspace(1) %15, i16 %13
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 12
  %18 = load i16, ptr addrspace(1) %17, !tbaa !6
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %18)
          to label %b12 unwind label %landing

b12:
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %21 = inttoptr i16 %19 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 %20
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 -6
  %25 = load i16, ptr addrspace(1) %24, !tbaa !6
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %25)
          to label %b13 unwind label %landing

b13:
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 18), !tbaa !2
  %27 = shl i16 %26, 3
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %29 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %30 = inttoptr i16 %28 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 %27
  %34 = load i16, ptr addrspace(1) %33, !tbaa !6
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %34)
          to label %b14 unwind label %landing

b14:
  %35 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %36 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %37 = inttoptr i16 %35 to ptr addrspace(2)
  %38 = addrspacecast ptr addrspace(2) %37 to ptr addrspace(1)
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 %36
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 8928
  %41 = load i16, ptr addrspace(1) %40, !tbaa !6
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %41)
          to label %b16 unwind label %landing

b16:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr @"D%")
          to label %b17 unwind label %landing

b17:
  %42 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %43 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %44 = inttoptr i16 %42 to ptr addrspace(2)
  %45 = addrspacecast ptr addrspace(2) %44 to ptr addrspace(1)
  %46 = getelementptr i8, ptr addrspace(1) %45, i16 %43
  %47 = load i16, ptr addrspace(1) %46, !tbaa !6
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %47)
          to label %b18 unwind label %landing

b18:
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
          to label %b19 unwind label %landing

b19:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %48 = extractvalue { ptr, i32 } %landed, 1
  %49 = trunc i32 %48 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %49)
  %50 = load i16, ptr %site
  switch table i16 %50, label %51 [
    i16 0, label %51
    i16 1, label %52
    i16 2, label %53
    i16 3, label %54
    i16 4, label %55
    i16 5, label %56
    i16 6, label %57
    i16 7, label %58
    i16 8, label %59
    i16 9, label %60
    i16 10, label %61
  ]

51:
  br label %b4

52:
  br label %b9

53:
  br label %b10

54:
  br label %b11

55:
  br label %b12

56:
  br label %b13

57:
  br label %b14

58:
  br label %b16

59:
  br label %b17

60:
  br label %b18

61:
  br label %b19

62:
  %63 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %64 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %65 = inttoptr i16 %63 to ptr addrspace(2)
  %66 = addrspacecast ptr addrspace(2) %65 to ptr addrspace(1)
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 %64
  %68 = getelementptr i8, ptr addrspace(1) %67, i16 -4
  %69 = load i16, ptr addrspace(1) %68, !tbaa !6
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %69)
          to label %70 unwind label %landing

70:
  %71 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 18), !tbaa !2
  %72 = mul i16 %71, 6
  %73 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 2), !tbaa !2
  %74 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"D%", i16 10), !tbaa !2
  %75 = inttoptr i16 %73 to ptr addrspace(2)
  %76 = addrspacecast ptr addrspace(2) %75 to ptr addrspace(1)
  %77 = getelementptr i8, ptr addrspace(1) %76, i16 %74
  %78 = getelementptr i8, ptr addrspace(1) %77, i16 %72
  %79 = getelementptr i8, ptr addrspace(1) %78, i16 10
  %80 = load i16, ptr addrspace(1) %79, !tbaa !6
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %80)
          to label %b9 unwind label %landing
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
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
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
