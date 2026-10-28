target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00= \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00level\00"
@$str3 = internal constant [31 x i8] c"\08\00\18\00\18\00          twice that is \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00name=Ada;role=pilot;level=7\00"
@$str5 = internal constant [23 x i8] c"\08\00\10\00\10\00 settings, kept \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %3)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %2)
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  %6 = addrspacecast ptr %2 to ptr addrspace(5)
  %7 = addrspacecast ptr %2 to ptr addrspace(1)
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %9

9:
  %10 = phi i16 [ %22, %17 ], [ 0, %0 ]
  %11 = phi i16 [ 0, %0 ], [ %20, %17 ]
  %12 = phi i16 [ 0, %0 ], [ %19, %17 ]
  %13 = icmp eq i16 %10, 27
  br i1 %13, label %14, label %23

14:
  %15 = icmp ult i16 %12, %10
  br i1 %15, label %16, label %31

16:
  br label %34

17:
  %18 = phi i16 [ %28, %27 ], [ %79, %77 ]
  %19 = phi i16 [ %79, %77 ], [ %12, %27 ]
  %20 = phi i16 [ %78, %77 ], [ %11, %27 ]
  %21 = icmp samesign ugt i16 %18, 27
  %22 = add nuw nsw i16 %10, 1
  br i1 %21, label %81, label %9

23:
  %24 = getelementptr inbounds nuw i8, ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), i16 %10
  %25 = load i8, ptr %24
  %26 = icmp eq i8 %25, 59
  br i1 %26, label %14, label %27

27:
  %28 = add nuw nsw i16 %10, 1
  br label %17

29:
  %30 = add i16 %35, 1
  %exitcond1 = icmp eq i16 %10, %30
  br i1 %exitcond1, label %31, label %34

31:
  %32 = phi i16 [ %12, %14 ], [ %35, %34 ], [ %30, %29 ]
  %33 = icmp ugt i16 %32, 27
  br i1 %33, label %40, label %41

34:
  %35 = phi i16 [ %30, %29 ], [ %12, %16 ]
  %36 = getelementptr i8, ptr @$str4, i16 %35
  %37 = getelementptr i8, ptr %36, i16 6
  %38 = load i8, ptr %37
  %39 = icmp eq i8 %38, 61
  br i1 %39, label %31, label %29

40:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

41:
  %42 = getelementptr inbounds i8, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6) to ptr addrspace(1)), i16 %12
  %43 = sub nuw i16 %32, %12
  store i16 %43, ptr %3, !tbaa !2
  %44 = getelementptr i8, ptr %3, i16 2
  store i16 %43, ptr %44, !tbaa !2
  %45 = getelementptr i8, ptr %3, i16 4
  store ptr addrspace(1) %42, ptr %45, !tbaa !2
  %46 = icmp ult i16 %32, %10
  br i1 %46, label %47, label %80

47:
  %48 = add nuw nsw i16 %32, 1
  %49 = getelementptr inbounds nuw i8, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6) to ptr addrspace(1)), i16 %48
  %50 = sub nuw nsw i16 %10, %48
  store i16 %50, ptr %2, !tbaa !2
  %51 = getelementptr i8, ptr %2, i16 2
  store i16 %50, ptr %51, !tbaa !2
  %52 = getelementptr i8, ptr %2, i16 4
  store ptr addrspace(1) %49, ptr %52, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %1)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 8, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %5)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %7)
  call cc 1005 addrspace(1) void @N$PN()
  store i32 327685, ptr %1
  %53 = getelementptr i8, ptr %1, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), ptr %53, !tbaa !2
  %54 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %5, ptr addrspace(1) %8)
  %55 = icmp eq i8 %54, 0
  br i1 %55, label %56, label %77

56:
  %57 = load i16, ptr addrspace(5) %6
  %58 = icmp eq i16 %57, 0
  br i1 %58, label %75, label %59

59:
  %60 = getelementptr i8, ptr addrspace(5) %6, i16 4
  %61 = load ptr addrspace(1), ptr addrspace(5) %60
  br label %62

62:
  %63 = phi ptr addrspace(1) [ %72, %62 ], [ %61, %59 ]
  %64 = phi i16 [ %71, %62 ], [ %57, %59 ]
  %65 = phi i16 [ 0, %59 ], [ %70, %62 ]
  %66 = mul i16 %65, 10
  %67 = load i8, ptr addrspace(1) %63
  %68 = zext i8 %67 to i16
  %69 = add i16 %66, -48
  %70 = add i16 %69, %68
  %71 = add i16 %64, -1
  %72 = getelementptr i8, ptr addrspace(1) %63, i16 1
  %exitcond = icmp eq i16 %71, 0
  br i1 %exitcond, label %73, label %62

73:
  %74 = shl i16 %70, 1
  br label %75

75:
  %76 = phi i16 [ 0, %56 ], [ %74, %73 ]
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %76)
  call cc 1005 addrspace(1) void @N$PN()
  br label %77

77:
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %1)
  %78 = add i16 %11, 1
  %79 = add nuw nsw i16 %10, 1
  br label %17

80:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

81:
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %2)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %3)
  store i32 196611, ptr %4
  %82 = getelementptr inbounds nuw i8, ptr %4, i16 4
  store ptr addrspace(1) getelementptr inbounds nuw (i8, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6) to ptr addrspace(1)), i16 5), ptr %82, !tbaa !2
  %83 = addrspacecast ptr %4 to ptr addrspace(1)
  %84 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %83)
  call cc 1005 addrspace(1) void @N$PU2(i16 %20)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr %84)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %84)
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  ret i16 0
}

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
