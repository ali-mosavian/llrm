target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"M%" = internal global [802 x i8] zeroinitializer, align 2
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"T=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = getelementptr i8, ptr @"M%", i16 38
  br label %b5

b5:
  %1 = phi ptr [ %41, %b5 ], [ %0, %b1 ]
  %2 = phi i16 [ 0, %b1 ], [ %42, %b5 ]
  %3 = getelementptr i8, ptr %1, i16 -38
  store i16 %2, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr %1, i16 -36
  %5 = add i16 %2, 1
  store i16 %5, ptr %4, !tbaa !2
  %6 = getelementptr i8, ptr %1, i16 -34
  %7 = add i16 %5, 1
  store i16 %7, ptr %6, !tbaa !2
  %8 = getelementptr i8, ptr %1, i16 -32
  %9 = add i16 %7, 1
  store i16 %9, ptr %8, !tbaa !2
  %10 = getelementptr i8, ptr %1, i16 -30
  %11 = add i16 %9, 1
  store i16 %11, ptr %10, !tbaa !2
  %12 = getelementptr i8, ptr %1, i16 -28
  %13 = add i16 %11, 1
  store i16 %13, ptr %12, !tbaa !2
  %14 = getelementptr i8, ptr %1, i16 -26
  %15 = add i16 %13, 1
  store i16 %15, ptr %14, !tbaa !2
  %16 = getelementptr i8, ptr %1, i16 -24
  %17 = add i16 %15, 1
  store i16 %17, ptr %16, !tbaa !2
  %18 = getelementptr i8, ptr %1, i16 -22
  %19 = add i16 %17, 1
  store i16 %19, ptr %18, !tbaa !2
  %20 = getelementptr i8, ptr %1, i16 -20
  %21 = add i16 %19, 1
  store i16 %21, ptr %20, !tbaa !2
  %22 = getelementptr i8, ptr %1, i16 -18
  %23 = add i16 %21, 1
  store i16 %23, ptr %22, !tbaa !2
  %24 = getelementptr i8, ptr %1, i16 -16
  %25 = add i16 %23, 1
  store i16 %25, ptr %24, !tbaa !2
  %26 = getelementptr i8, ptr %1, i16 -14
  %27 = add i16 %25, 1
  store i16 %27, ptr %26, !tbaa !2
  %28 = getelementptr i8, ptr %1, i16 -12
  %29 = add i16 %27, 1
  store i16 %29, ptr %28, !tbaa !2
  %30 = getelementptr i8, ptr %1, i16 -10
  %31 = add i16 %29, 1
  store i16 %31, ptr %30, !tbaa !2
  %32 = getelementptr i8, ptr %1, i16 -8
  %33 = add i16 %31, 1
  store i16 %33, ptr %32, !tbaa !2
  %34 = getelementptr i8, ptr %1, i16 -6
  %35 = add i16 %33, 1
  store i16 %35, ptr %34, !tbaa !2
  %36 = getelementptr i8, ptr %1, i16 -4
  %37 = add i16 %35, 1
  store i16 %37, ptr %36, !tbaa !2
  %38 = getelementptr i8, ptr %1, i16 -2
  %39 = add i16 %37, 1
  store i16 %39, ptr %38, !tbaa !2
  %40 = add i16 %39, 1
  store i16 %40, ptr %1, !tbaa !2
  %41 = getelementptr i8, ptr %1, i16 40
  %42 = add i16 %40, -18
  %exitcond2 = icmp eq i16 %42, 20
  br i1 %exitcond2, label %b6, label %b5

b6:
  %43 = load i16, ptr @"M%", !tbaa !2
  %44 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 42), !tbaa !2
  %45 = add i16 %44, %43
  %46 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 84), !tbaa !2
  %47 = add i16 %46, %45
  %48 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 126), !tbaa !2
  %49 = add i16 %48, %47
  %50 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 168), !tbaa !2
  %51 = add i16 %50, %49
  %52 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 210), !tbaa !2
  %53 = add i16 %52, %51
  %54 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 252), !tbaa !2
  %55 = add i16 %54, %53
  %56 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 294), !tbaa !2
  %57 = add i16 %56, %55
  %58 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 336), !tbaa !2
  %59 = add i16 %58, %57
  %60 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 378), !tbaa !2
  %61 = add i16 %60, %59
  %62 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 420), !tbaa !2
  %63 = add i16 %62, %61
  %64 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 462), !tbaa !2
  %65 = add i16 %64, %63
  %66 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 504), !tbaa !2
  %67 = add i16 %66, %65
  %68 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 546), !tbaa !2
  %69 = add i16 %68, %67
  %70 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 588), !tbaa !2
  %71 = add i16 %70, %69
  %72 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 630), !tbaa !2
  %73 = add i16 %72, %71
  %74 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 672), !tbaa !2
  %75 = add i16 %74, %73
  %76 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 714), !tbaa !2
  %77 = add i16 %76, %75
  %78 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 756), !tbaa !2
  %79 = add i16 %78, %77
  %80 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 798), !tbaa !2
  %81 = add i16 %80, %79
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %81)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
