target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"POSX&" = internal global [28 x i8] zeroinitializer, align 2
@"POSY&" = internal global [28 x i8] zeroinitializer, align 2
@"VELX&" = internal global [28 x i8] zeroinitializer, align 2
@"VELY&" = internal global [28 x i8] zeroinitializer, align 2

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  %0 = tail call cc 1000 addrspace(1) i32 @"BENCHNBODYFIXED&"()
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %0)
  %2 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 i32 @"BENCHNBODYFIXED&"() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call void @llvm.memset.p0.i16(ptr align 2 @"VELX&", i8 0, i16 24, i1 false)
  tail call void @llvm.memset.p0.i16(ptr align 2 @"VELY&", i8 0, i16 24, i1 false)
  store i32 -7680, ptr @"POSX&", !tbaa !2
  store i32 -6144, ptr @"POSY&", !tbaa !2
  store i32 -4096, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 4), !tbaa !2
  store i32 -3584, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 4), !tbaa !2
  store i32 -512, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 8), !tbaa !2
  store i32 -1024, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 8), !tbaa !2
  store i32 3072, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 12), !tbaa !2
  store i32 1536, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 12), !tbaa !2
  store i32 6656, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 16), !tbaa !2
  store i32 4096, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 16), !tbaa !2
  store i32 10240, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 20), !tbaa !2
  store i32 6656, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 20), !tbaa !2
  br label %b10

b10:
  %0 = phi i32 [ 6656, %b1 ], [ %84, %b16 ]
  %1 = phi i32 [ 10240, %b1 ], [ %82, %b16 ]
  %2 = phi i32 [ 4096, %b1 ], [ %80, %b16 ]
  %3 = phi i32 [ 6656, %b1 ], [ %78, %b16 ]
  %4 = phi i32 [ 1536, %b1 ], [ %76, %b16 ]
  %5 = phi i32 [ 3072, %b1 ], [ %74, %b16 ]
  %6 = phi i32 [ -1024, %b1 ], [ %72, %b16 ]
  %7 = phi i32 [ -512, %b1 ], [ %70, %b16 ]
  %8 = phi i32 [ -3584, %b1 ], [ %68, %b16 ]
  %9 = phi i32 [ -4096, %b1 ], [ %66, %b16 ]
  %10 = phi i16 [ 1, %b1 ], [ %85, %b16 ]
  br label %b15

b11:
  %11 = mul i32 %64, 3
  %12 = add i32 %11, %61
  %13 = mul i32 %60, 5
  %14 = add i32 %12, %13
  %15 = mul i32 %63, 7
  %16 = add i32 %14, %15
  %17 = mul i32 %68, 3
  %18 = add i32 %17, %66
  %19 = mul i32 %65, 5
  %20 = add i32 %18, %19
  %21 = mul i32 %67, 7
  %22 = add i32 %20, %21
  %23 = shl i32 %22, 1
  %24 = add i32 %23, %16
  %25 = mul i32 %72, 3
  %26 = add i32 %25, %70
  %27 = mul i32 %69, 5
  %28 = add i32 %26, %27
  %29 = mul i32 %71, 7
  %30 = add i32 %28, %29
  %31 = mul i32 %30, 3
  %32 = add i32 %31, %24
  %33 = mul i32 %76, 3
  %34 = add i32 %33, %74
  %35 = mul i32 %73, 5
  %36 = add i32 %34, %35
  %37 = mul i32 %75, 7
  %38 = add i32 %36, %37
  %39 = shl i32 %38, 2
  %40 = add i32 %39, %32
  %41 = mul i32 %80, 3
  %42 = add i32 %41, %78
  %43 = mul i32 %77, 5
  %44 = add i32 %42, %43
  %45 = mul i32 %79, 7
  %46 = add i32 %44, %45
  %47 = mul i32 %46, 5
  %48 = add i32 %47, %40
  %49 = mul i32 %84, 3
  %50 = add i32 %49, %82
  %51 = mul i32 %81, 5
  %52 = add i32 %50, %51
  %53 = mul i32 %83, 7
  %54 = add i32 %52, %53
  %55 = mul i32 %54, 6
  %56 = add i32 %55, %48
  ret i32 %56

b15:
  %57 = phi i16 [ %102, %b21 ], [ 0, %b10 ]
  %58 = phi i16 [ 0, %b10 ], [ %101, %b21 ]
  br label %b20

b16:
  %59 = load i32, ptr @"POSX&", !tbaa !2
  %60 = load i32, ptr @"VELX&", !tbaa !2
  %61 = add i32 %60, %59
  store i32 %61, ptr @"POSX&", !tbaa !2
  %62 = load i32, ptr @"POSY&", !tbaa !2
  %63 = load i32, ptr @"VELY&", !tbaa !2
  %64 = add i32 %63, %62
  store i32 %64, ptr @"POSY&", !tbaa !2
  %65 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 4), !tbaa !2
  %66 = add i32 %65, %9
  store i32 %66, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 4), !tbaa !2
  %67 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 4), !tbaa !2
  %68 = add i32 %67, %8
  store i32 %68, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 4), !tbaa !2
  %69 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 8), !tbaa !2
  %70 = add i32 %69, %7
  store i32 %70, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 8), !tbaa !2
  %71 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 8), !tbaa !2
  %72 = add i32 %71, %6
  store i32 %72, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 8), !tbaa !2
  %73 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 12), !tbaa !2
  %74 = add i32 %73, %5
  store i32 %74, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 12), !tbaa !2
  %75 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 12), !tbaa !2
  %76 = add i32 %75, %4
  store i32 %76, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 12), !tbaa !2
  %77 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 16), !tbaa !2
  %78 = add i32 %77, %3
  store i32 %78, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 16), !tbaa !2
  %79 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 16), !tbaa !2
  %80 = add i32 %79, %2
  store i32 %80, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 16), !tbaa !2
  %81 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 20), !tbaa !2
  %82 = add i32 %81, %1
  store i32 %82, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 20), !tbaa !2
  %83 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 20), !tbaa !2
  %84 = add i32 %83, %0
  store i32 %84, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 20), !tbaa !2
  %85 = add nuw nsw i16 %10, 1
  %exitcond4 = icmp eq i16 %85, 1001
  br i1 %exitcond4, label %b11, label %b10

b20:
  %86 = phi i16 [ %126, %b24 ], [ -24, %b15 ]
  %87 = phi i32 [ 0, %b15 ], [ %125, %b24 ]
  %88 = phi i32 [ 0, %b15 ], [ %124, %b24 ]
  %89 = add i16 %57, %86
  %90 = icmp eq i16 %89, -24
  br i1 %90, label %b24, label %b22

b21:
  %91 = getelementptr inbounds nuw [4 x i8], ptr @"VELX&", i16 %58
  %92 = load i32, ptr %91, !tbaa !2
  %93 = add i32 %92, %125
  %94 = getelementptr inbounds nuw [4 x i8], ptr @"VELY&", i16 %58
  %95 = load i32, ptr %94, !tbaa !2
  %96 = add i32 %95, %124
  %97 = sdiv i32 %93, -16
  %98 = add i32 %97, %93
  store i32 %98, ptr %91, !tbaa !2
  %99 = sdiv i32 %96, -16
  %100 = add i32 %99, %96
  store i32 %100, ptr %94, !tbaa !2
  %101 = add nuw nsw i16 %58, 1
  %exitcond3 = icmp eq i16 %101, 6
  %102 = add nsw i16 %57, -4
  br i1 %exitcond3, label %b16, label %b15

b22:
  %103 = getelementptr i8, ptr @"POSX&", i16 %86
  %104 = getelementptr i8, ptr %103, i16 24
  %105 = load i32, ptr %104, !tbaa !2
  %106 = mul i16 %58, 4
  %107 = getelementptr i8, ptr @"POSX&", i16 %106
  %108 = load i32, ptr %107, !tbaa !2
  %109 = sub i32 %105, %108
  %110 = getelementptr i8, ptr @"POSY&", i16 %86
  %111 = getelementptr i8, ptr %110, i16 24
  %112 = load i32, ptr %111, !tbaa !2
  %113 = mul i16 %58, 4
  %114 = getelementptr i8, ptr @"POSY&", i16 %113
  %115 = load i32, ptr %114, !tbaa !2
  %116 = sub i32 %112, %115
  %117 = mul i32 %109, %109
  %118 = mul i32 %116, %116
  %119 = add i32 %117, 262144
  %120 = add i32 %119, %118
  %121 = sdiv i32 %120, 262144
  %122 = add nsw i32 %121, 1
  %123 = icmp eq i32 %122, 0
  br i1 %123, label %b30, label %b31

b24:
  %124 = phi i32 [ %133, %b31 ], [ %88, %b20 ]
  %125 = phi i32 [ %130, %b31 ], [ %87, %b20 ]
  %126 = add nsw i16 %86, 4
  %exitcond2 = icmp eq i16 %126, 0
  br i1 %exitcond2, label %b21, label %b20

b30:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b31:
  %.rhs.trunc = trunc nsw i32 %122 to i16
  %127 = sdiv i16 512, %.rhs.trunc
  %.sext = sext i16 %127 to i32
  %128 = mul i32 %109, %.sext
  %129 = sdiv i32 %128, 512
  %130 = add i32 %129, %87
  %131 = mul i32 %116, %.sext
  %132 = sdiv i32 %131, 512
  %133 = add i32 %132, %88
  br label %b24
}

declare cc 1005 ptr @llrm.qb.B$STI4(i32) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
