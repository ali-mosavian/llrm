target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00PX=\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00PY=\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00VX=\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00VY=\00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00DONE\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [96 x i8]
  call void @llvm.lifetime.start.p0(i64 96, ptr nonnull %1)
  store i32 -7680, ptr %1, !tbaa !2
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store i32 -6144, ptr %2, !tbaa !2
  %3 = getelementptr inbounds nuw i8, ptr %1, i16 8
  store i32 0, ptr %3, !tbaa !2
  %4 = getelementptr inbounds nuw i8, ptr %1, i16 12
  store i32 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds nuw i8, ptr %1, i16 16
  store i32 -4096, ptr %5, !tbaa !2
  %6 = getelementptr inbounds nuw i8, ptr %1, i16 20
  store i32 -3584, ptr %6, !tbaa !2
  %7 = getelementptr inbounds nuw i8, ptr %1, i16 24
  store i32 0, ptr %7, !tbaa !2
  %8 = getelementptr inbounds nuw i8, ptr %1, i16 28
  store i32 0, ptr %8, !tbaa !2
  %9 = getelementptr inbounds nuw i8, ptr %1, i16 32
  store i32 -512, ptr %9, !tbaa !2
  %10 = getelementptr inbounds nuw i8, ptr %1, i16 36
  store i32 -1024, ptr %10, !tbaa !2
  %11 = getelementptr inbounds nuw i8, ptr %1, i16 40
  store i32 0, ptr %11, !tbaa !2
  %12 = getelementptr inbounds nuw i8, ptr %1, i16 44
  store i32 0, ptr %12, !tbaa !2
  %13 = getelementptr inbounds nuw i8, ptr %1, i16 48
  store i32 3072, ptr %13, !tbaa !2
  %14 = getelementptr inbounds nuw i8, ptr %1, i16 52
  store i32 1536, ptr %14, !tbaa !2
  %15 = getelementptr inbounds nuw i8, ptr %1, i16 56
  store i32 0, ptr %15, !tbaa !2
  %16 = getelementptr inbounds nuw i8, ptr %1, i16 60
  store i32 0, ptr %16, !tbaa !2
  %17 = getelementptr inbounds nuw i8, ptr %1, i16 64
  store i32 6656, ptr %17, !tbaa !2
  %18 = getelementptr inbounds nuw i8, ptr %1, i16 68
  store i32 4096, ptr %18, !tbaa !2
  %19 = getelementptr inbounds nuw i8, ptr %1, i16 72
  store i32 0, ptr %19, !tbaa !2
  %20 = getelementptr inbounds nuw i8, ptr %1, i16 76
  store i32 0, ptr %20, !tbaa !2
  %21 = getelementptr inbounds nuw i8, ptr %1, i16 80
  store i32 10240, ptr %21, !tbaa !2
  %22 = getelementptr inbounds nuw i8, ptr %1, i16 84
  store i32 6656, ptr %22, !tbaa !2
  %23 = getelementptr inbounds nuw i8, ptr %1, i16 88
  store i32 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds nuw i8, ptr %1, i16 92
  store i32 0, ptr %24, !tbaa !2
  br label %25

25:
  %26 = phi i16 [ %95, %83 ], [ 0, %0 ]
  %27 = phi i16 [ 0, %0 ], [ %94, %83 ]
  %28 = getelementptr inbounds nuw [16 x i8], ptr %1, i16 %27
  br label %77

29:
  %30 = getelementptr i8, ptr %1, i16 8
  %31 = load i32, ptr %30, !tbaa !2
  %32 = add i32 %31, -7680
  store i32 %32, ptr %1, !tbaa !2
  %33 = getelementptr i8, ptr %1, i16 12
  %34 = load i32, ptr %33, !tbaa !2
  %35 = add i32 %34, -6144
  %36 = getelementptr i8, ptr %1, i16 4
  store i32 %35, ptr %36, !tbaa !2
  %37 = getelementptr i8, ptr %1, i16 24
  %38 = load i32, ptr %37, !tbaa !2
  %39 = add i32 %38, -4096
  %40 = getelementptr i8, ptr %1, i16 16
  store i32 %39, ptr %40, !tbaa !2
  %41 = getelementptr i8, ptr %1, i16 28
  %42 = load i32, ptr %41, !tbaa !2
  %43 = add i32 %42, -3584
  %44 = getelementptr i8, ptr %1, i16 20
  store i32 %43, ptr %44, !tbaa !2
  %45 = getelementptr i8, ptr %1, i16 40
  %46 = load i32, ptr %45, !tbaa !2
  %47 = add i32 %46, -512
  %48 = getelementptr i8, ptr %1, i16 32
  store i32 %47, ptr %48, !tbaa !2
  %49 = getelementptr i8, ptr %1, i16 44
  %50 = load i32, ptr %49, !tbaa !2
  %51 = add i32 %50, -1024
  %52 = getelementptr i8, ptr %1, i16 36
  store i32 %51, ptr %52, !tbaa !2
  %53 = getelementptr i8, ptr %1, i16 56
  %54 = load i32, ptr %53, !tbaa !2
  %55 = add i32 %54, 3072
  %56 = getelementptr i8, ptr %1, i16 48
  store i32 %55, ptr %56, !tbaa !2
  %57 = getelementptr i8, ptr %1, i16 60
  %58 = load i32, ptr %57, !tbaa !2
  %59 = add i32 %58, 1536
  %60 = getelementptr i8, ptr %1, i16 52
  store i32 %59, ptr %60, !tbaa !2
  %61 = getelementptr i8, ptr %1, i16 72
  %62 = load i32, ptr %61, !tbaa !2
  %63 = add i32 %62, 6656
  %64 = getelementptr i8, ptr %1, i16 64
  store i32 %63, ptr %64, !tbaa !2
  %65 = getelementptr i8, ptr %1, i16 76
  %66 = load i32, ptr %65, !tbaa !2
  %67 = add i32 %66, 4096
  %68 = getelementptr i8, ptr %1, i16 68
  store i32 %67, ptr %68, !tbaa !2
  %69 = getelementptr i8, ptr %1, i16 88
  %70 = load i32, ptr %69, !tbaa !2
  %71 = add i32 %70, 10240
  %72 = getelementptr i8, ptr %1, i16 80
  store i32 %71, ptr %72, !tbaa !2
  %73 = getelementptr i8, ptr %1, i16 92
  %74 = load i32, ptr %73, !tbaa !2
  %75 = add i32 %74, 6656
  %76 = getelementptr i8, ptr %1, i16 84
  store i32 %75, ptr %76, !tbaa !2
  br label %121

77:
  %78 = phi i16 [ %120, %117 ], [ -96, %25 ]
  %79 = phi i32 [ 0, %25 ], [ %119, %117 ]
  %80 = phi i32 [ 0, %25 ], [ %118, %117 ]
  %81 = add i16 %26, %78
  %82 = icmp eq i16 %81, -96
  br i1 %82, label %117, label %96

83:
  %84 = getelementptr inbounds nuw i8, ptr %28, i16 8
  %85 = load i32, ptr %84, !tbaa !2
  %86 = add i32 %85, %119
  %87 = getelementptr inbounds nuw i8, ptr %28, i16 12
  %88 = load i32, ptr %87, !tbaa !2
  %89 = add i32 %88, %118
  %90 = sdiv i32 %86, -16
  %91 = add i32 %90, %86
  store i32 %91, ptr %84, !tbaa !2
  %92 = sdiv i32 %89, -16
  %93 = add i32 %92, %89
  store i32 %93, ptr %87, !tbaa !2
  %94 = add nuw nsw i16 %27, 1
  %exitcond3 = icmp eq i16 %94, 6
  %95 = add nsw i16 %26, -16
  br i1 %exitcond3, label %29, label %25

96:
  %97 = getelementptr i8, ptr %1, i16 %78
  %98 = getelementptr i8, ptr %97, i16 96
  %99 = load i32, ptr %98, !tbaa !2
  %100 = load i32, ptr %28, !tbaa !2
  %101 = sub i32 %99, %100
  %102 = getelementptr i8, ptr %1, i16 %78
  %103 = getelementptr i8, ptr %102, i16 100
  %104 = load i32, ptr %103, !tbaa !2
  %105 = getelementptr i8, ptr %28, i16 4
  %106 = load i32, ptr %105, !tbaa !2
  %107 = sub i32 %104, %106
  %108 = tail call i32 @llvm.smul.fix.i32(i32 %101, i32 %101, i32 9)
  %109 = tail call i32 @llvm.smul.fix.i32(i32 %107, i32 %107, i32 9)
  %110 = add i32 %108, 512
  %111 = add i32 %110, %109
  %112 = sdiv i32 262144, %111
  %113 = tail call i32 @llvm.smul.fix.i32(i32 %101, i32 %112, i32 9)
  %114 = add i32 %113, %79
  %115 = tail call i32 @llvm.smul.fix.i32(i32 %107, i32 %112, i32 9)
  %116 = add i32 %115, %80
  br label %117

117:
  %118 = phi i32 [ %116, %96 ], [ %80, %77 ]
  %119 = phi i32 [ %114, %96 ], [ %79, %77 ]
  %120 = add nsw i16 %78, 16
  %exitcond2 = icmp eq i16 %120, 0
  br i1 %exitcond2, label %83, label %77

121:
  %122 = phi i16 [ %135, %121 ], [ -96, %29 ]
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %123 = getelementptr i8, ptr %1, i16 %122
  %124 = getelementptr i8, ptr %123, i16 96
  %125 = load i32, ptr %124, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 %125, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  %126 = getelementptr i8, ptr %1, i16 %122
  %127 = getelementptr i8, ptr %126, i16 100
  %128 = load i32, ptr %127, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 %128, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  %129 = getelementptr i8, ptr %1, i16 %122
  %130 = getelementptr i8, ptr %129, i16 104
  %131 = load i32, ptr %130, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 %131, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  %132 = getelementptr i8, ptr %1, i16 %122
  %133 = getelementptr i8, ptr %132, i16 108
  %134 = load i32, ptr %133, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 %134, i8 zeroext 9)
  tail call cc 1005 addrspace(1) void @N$PN()
  %135 = add nsw i16 %122, 16
  %exitcond = icmp eq i16 %135, 0
  br i1 %exitcond, label %136, label %121

136:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  call void @llvm.lifetime.end.p0(i64 96, ptr nonnull %1)
  ret i16 0
}

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
