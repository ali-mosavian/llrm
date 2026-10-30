target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [256 x i8]
  %1 = alloca [256 x i8]
  %2 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr align 2 %2, i8 0, i16 256, i1 false)
  call void @llvm.memset.p0.i16(ptr align 2 %1, i8 0, i16 256, i1 false)
  call void @llvm.memset.p0.i16(ptr align 2 %0, i8 0, i16 256, i1 false)
  br label %b27

b27:
  %3 = phi i32 [ %101, %b33 ], [ 128, %b1 ]
  %4 = phi ptr [ %100, %b33 ], [ %2, %b1 ]
  %5 = phi ptr [ %99, %b33 ], [ %1, %b1 ]
  %6 = phi i16 [ %98, %b33 ], [ 0, %b1 ]
  %7 = phi i16 [ %97, %b33 ], [ 0, %b1 ]
  %8 = phi i16 [ 0, %b1 ], [ %96, %b33 ]
  br label %b36

9:
  %10 = addrspacecast ptr %2 to ptr addrspace(5)
  %11 = addrspacecast ptr %1 to ptr addrspace(5)
  %12 = addrspacecast ptr %0 to ptr addrspace(5)
  br label %13

13:
  %14 = phi ptr addrspace(5) [ %93, %91 ], [ %12, %9 ]
  %15 = phi i16 [ 0, %9 ], [ %92, %91 ]
  %16 = shl nuw nsw i16 %15, 5
  %17 = getelementptr inbounds nuw i8, ptr addrspace(5) %10, i16 %16
  %18 = load i32, ptr addrspace(5) %17
  %19 = shl i16 %15, 5
  %20 = getelementptr i8, ptr addrspace(5) %10, i16 %19
  %21 = getelementptr i8, ptr addrspace(5) %20, i16 4
  %22 = load i32, ptr addrspace(5) %21
  %23 = shl i16 %15, 5
  %24 = getelementptr i8, ptr addrspace(5) %10, i16 %23
  %25 = getelementptr i8, ptr addrspace(5) %24, i16 8
  %26 = load i32, ptr addrspace(5) %25
  %27 = shl i16 %15, 5
  %28 = getelementptr i8, ptr addrspace(5) %10, i16 %27
  %29 = getelementptr i8, ptr addrspace(5) %28, i16 12
  %30 = load i32, ptr addrspace(5) %29
  %31 = shl i16 %15, 5
  %32 = getelementptr i8, ptr addrspace(5) %10, i16 %31
  %33 = getelementptr i8, ptr addrspace(5) %32, i16 16
  %34 = load i32, ptr addrspace(5) %33
  %35 = shl i16 %15, 5
  %36 = getelementptr i8, ptr addrspace(5) %10, i16 %35
  %37 = getelementptr i8, ptr addrspace(5) %36, i16 20
  %38 = load i32, ptr addrspace(5) %37
  %39 = shl i16 %15, 5
  %40 = getelementptr i8, ptr addrspace(5) %10, i16 %39
  %41 = getelementptr i8, ptr addrspace(5) %40, i16 24
  %42 = load i32, ptr addrspace(5) %41
  %43 = shl i16 %15, 5
  %44 = getelementptr i8, ptr addrspace(5) %10, i16 %43
  %45 = getelementptr i8, ptr addrspace(5) %44, i16 28
  %46 = load i32, ptr addrspace(5) %45
  br label %47

47:
  %48 = phi i16 [ -32, %13 ], [ %90, %47 ]
  %49 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %50 = getelementptr i8, ptr addrspace(5) %49, i16 32
  %51 = load i32, ptr addrspace(5) %50
  %52 = tail call i32 @llvm.smul.fix.i32(i32 %18, i32 %51, i32 8)
  %53 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %54 = getelementptr i8, ptr addrspace(5) %53, i16 64
  %55 = load i32, ptr addrspace(5) %54
  %56 = tail call i32 @llvm.smul.fix.i32(i32 %22, i32 %55, i32 8)
  %57 = add i32 %56, %52
  %58 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %59 = getelementptr i8, ptr addrspace(5) %58, i16 96
  %60 = load i32, ptr addrspace(5) %59
  %61 = tail call i32 @llvm.smul.fix.i32(i32 %26, i32 %60, i32 8)
  %62 = add i32 %61, %57
  %63 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %64 = getelementptr i8, ptr addrspace(5) %63, i16 128
  %65 = load i32, ptr addrspace(5) %64
  %66 = tail call i32 @llvm.smul.fix.i32(i32 %30, i32 %65, i32 8)
  %67 = add i32 %66, %62
  %68 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %69 = getelementptr i8, ptr addrspace(5) %68, i16 160
  %70 = load i32, ptr addrspace(5) %69
  %71 = tail call i32 @llvm.smul.fix.i32(i32 %34, i32 %70, i32 8)
  %72 = add i32 %71, %67
  %73 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %74 = getelementptr i8, ptr addrspace(5) %73, i16 192
  %75 = load i32, ptr addrspace(5) %74
  %76 = tail call i32 @llvm.smul.fix.i32(i32 %38, i32 %75, i32 8)
  %77 = add i32 %76, %72
  %78 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %79 = getelementptr i8, ptr addrspace(5) %78, i16 224
  %80 = load i32, ptr addrspace(5) %79
  %81 = tail call i32 @llvm.smul.fix.i32(i32 %42, i32 %80, i32 8)
  %82 = add i32 %81, %77
  %83 = getelementptr i8, ptr addrspace(5) %11, i16 %48
  %84 = getelementptr i8, ptr addrspace(5) %83, i16 256
  %85 = load i32, ptr addrspace(5) %84
  %86 = tail call i32 @llvm.smul.fix.i32(i32 %46, i32 %85, i32 8)
  %87 = add i32 %86, %82
  %88 = getelementptr i8, ptr addrspace(5) %14, i16 %48
  %89 = getelementptr i8, ptr addrspace(5) %88, i16 32
  store i32 %87, ptr addrspace(5) %89
  %90 = add nsw i16 %48, 4
  %exitcond2 = icmp eq i16 %90, 0
  br i1 %exitcond2, label %91, label %47

91:
  %92 = add nuw nsw i16 %15, 1
  %exitcond3 = icmp eq i16 %92, 8
  %93 = getelementptr i8, ptr addrspace(5) %14, i16 32
  br i1 %exitcond3, label %94, label %13

94:
  %95 = getelementptr i8, ptr %0, i16 28
  br label %b50

b33:
  %96 = add nuw nsw i16 %8, 1
  %exitcond7 = icmp eq i16 %96, 8
  %97 = add nuw nsw i16 %7, 128
  %98 = add nuw nsw i16 %6, 4
  %99 = getelementptr i8, ptr %5, i16 32
  %100 = getelementptr i8, ptr %4, i16 32
  %101 = add nuw nsw i32 %3, 192
  br i1 %exitcond7, label %9, label %b27

b36:
  %102 = phi i32 [ %115, %b40 ], [ %3, %b27 ]
  %103 = phi i16 [ %114, %b40 ], [ 0, %b27 ]
  %104 = phi i16 [ %113, %b40 ], [ %8, %b27 ]
  %105 = phi i16 [ %112, %b40 ], [ %7, %b27 ]
  %106 = udiv i16 %104, 3
  %107 = mul nuw nsw i16 %106, 384
  %108 = sub i16 %105, %107
  %109 = getelementptr i8, ptr %4, i16 %103
  store i32 %102, ptr %109, !tbaa !2
  %110 = icmp eq i16 %6, %103
  br i1 %110, label %b40, label %b47

b40:
  %sink = phi i32 [ %116, %b47 ], [ 512, %b36 ]
  %111 = getelementptr i8, ptr %5, i16 %103
  store i32 %sink, ptr %111, !tbaa !2
  %112 = add nuw nsw i16 %105, 128
  %113 = add nuw nsw i16 %104, 1
  %114 = add nuw nsw i16 %103, 4
  %115 = add nuw nsw i32 %102, 64
  %exitcond6 = icmp eq i16 %114, 32
  br i1 %exitcond6, label %b33, label %b36

b47:
  %116 = zext nneg i16 %108 to i32
  br label %b40

b50:
  %117 = phi ptr [ %95, %94 ], [ %159, %b50 ]
  %118 = phi i32 [ 2048, %94 ], [ %158, %b50 ]
  %119 = phi i32 [ %157, %b50 ], [ 0, %94 ]
  %120 = getelementptr i8, ptr %117, i16 -28
  %121 = load i32, ptr %120, !tbaa !2
  %122 = add i32 %118, -1792
  %123 = tail call i32 @llvm.smul.fix.i32(i32 %121, i32 %122, i32 8)
  %124 = add i32 %123, %119
  %125 = getelementptr i8, ptr %117, i16 -24
  %126 = load i32, ptr %125, !tbaa !2
  %127 = add i32 %118, -1536
  %128 = tail call i32 @llvm.smul.fix.i32(i32 %126, i32 %127, i32 8)
  %129 = add i32 %128, %124
  %130 = getelementptr i8, ptr %117, i16 -20
  %131 = load i32, ptr %130, !tbaa !2
  %132 = add i32 %118, -1280
  %133 = tail call i32 @llvm.smul.fix.i32(i32 %131, i32 %132, i32 8)
  %134 = add i32 %133, %129
  %135 = getelementptr i8, ptr %117, i16 -16
  %136 = load i32, ptr %135, !tbaa !2
  %137 = add i32 %118, -1024
  %138 = tail call i32 @llvm.smul.fix.i32(i32 %136, i32 %137, i32 8)
  %139 = add i32 %138, %134
  %140 = getelementptr i8, ptr %117, i16 -12
  %141 = load i32, ptr %140, !tbaa !2
  %142 = add i32 %118, -768
  %143 = tail call i32 @llvm.smul.fix.i32(i32 %141, i32 %142, i32 8)
  %144 = add i32 %143, %139
  %145 = getelementptr i8, ptr %117, i16 -8
  %146 = load i32, ptr %145, !tbaa !2
  %147 = add i32 %118, -512
  %148 = tail call i32 @llvm.smul.fix.i32(i32 %146, i32 %147, i32 8)
  %149 = add i32 %148, %144
  %150 = getelementptr i8, ptr %117, i16 -4
  %151 = load i32, ptr %150, !tbaa !2
  %152 = add i32 %118, -256
  %153 = tail call i32 @llvm.smul.fix.i32(i32 %151, i32 %152, i32 8)
  %154 = add i32 %153, %149
  %155 = load i32, ptr %117, !tbaa !2
  %156 = tail call i32 @llvm.smul.fix.i32(i32 %155, i32 %118, i32 8)
  %157 = add i32 %156, %154
  %158 = add nuw nsw i32 %118, 2048
  %tmp = trunc i32 %158 to i16
  %159 = getelementptr i8, ptr %117, i16 32
  %exitcond5 = icmp eq i16 %tmp, 18432
  br i1 %exitcond5, label %b52, label %b50

b52:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 %157, i8 zeroext 8)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 16384}
!6 = !{i16 0, i16 16384}
!7 = !{i16 0, i16 16384}
!8 = !{i16 0, i16 16384}
!9 = !{i16 0, i16 16384}
!10 = !{i16 0, i16 16384}
