target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"POSX&" = internal global [28 x i8] zeroinitializer, align 2
@"POSY&" = internal global [28 x i8] zeroinitializer, align 2
@"VELX&" = internal global [28 x i8] zeroinitializer, align 2
@"VELY&" = internal global [28 x i8] zeroinitializer, align 2
@TAG$ = internal global [4 x i8] zeroinitializer
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"PX" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c"PY" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"=\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string11, i16 4), [2 x i8] c"VX" }>
@$string12 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string12, i16 4), [2 x i8] c"=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string13, i16 4), [2 x i8] c"VY" }>
@$string14 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string14, i16 4), [2 x i8] c"=\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string15, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  %0 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FCMD()
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FVAL(ptr %0)
  %2 = load double, ptr %1
  %3 = tail call i32 @llvm.lrint.i32.f64(double %2)
  %4 = icmp slt i32 %3, 1
  %spec.store.select = select i1 %4, i32 100, i32 %3
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
  %5 = tail call i32 @llvm.smax.i32(i32 %spec.store.select, i32 1)
  br label %b13

b13:
  %6 = phi i32 [ 6656, %b1 ], [ %44, %b19 ]
  %7 = phi i32 [ 10240, %b1 ], [ %42, %b19 ]
  %8 = phi i32 [ 4096, %b1 ], [ %40, %b19 ]
  %9 = phi i32 [ 6656, %b1 ], [ %38, %b19 ]
  %10 = phi i32 [ 1536, %b1 ], [ %36, %b19 ]
  %11 = phi i32 [ 3072, %b1 ], [ %34, %b19 ]
  %12 = phi i32 [ -1024, %b1 ], [ %32, %b19 ]
  %13 = phi i32 [ -512, %b1 ], [ %30, %b19 ]
  %14 = phi i32 [ -3584, %b1 ], [ %28, %b19 ]
  %15 = phi i32 [ -4096, %b1 ], [ %26, %b19 ]
  %16 = phi i32 [ 1, %b1 ], [ %45, %b19 ]
  br label %b18

b14:
  br label %b43

b18:
  %17 = phi i16 [ %63, %b24 ], [ 0, %b13 ]
  %18 = phi i16 [ 0, %b13 ], [ %62, %b24 ]
  br label %b23

b19:
  %19 = load i32, ptr @"POSX&", !tbaa !2
  %20 = load i32, ptr @"VELX&", !tbaa !2
  %21 = add i32 %20, %19
  store i32 %21, ptr @"POSX&", !tbaa !2
  %22 = load i32, ptr @"POSY&", !tbaa !2
  %23 = load i32, ptr @"VELY&", !tbaa !2
  %24 = add i32 %23, %22
  store i32 %24, ptr @"POSY&", !tbaa !2
  %25 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 4), !tbaa !2
  %26 = add i32 %25, %15
  store i32 %26, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 4), !tbaa !2
  %27 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 4), !tbaa !2
  %28 = add i32 %27, %14
  store i32 %28, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 4), !tbaa !2
  %29 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 8), !tbaa !2
  %30 = add i32 %29, %13
  store i32 %30, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 8), !tbaa !2
  %31 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 8), !tbaa !2
  %32 = add i32 %31, %12
  store i32 %32, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 8), !tbaa !2
  %33 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 12), !tbaa !2
  %34 = add i32 %33, %11
  store i32 %34, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 12), !tbaa !2
  %35 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 12), !tbaa !2
  %36 = add i32 %35, %10
  store i32 %36, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 12), !tbaa !2
  %37 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 16), !tbaa !2
  %38 = add i32 %37, %9
  store i32 %38, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 16), !tbaa !2
  %39 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 16), !tbaa !2
  %40 = add i32 %39, %8
  store i32 %40, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 16), !tbaa !2
  %41 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELX&", i16 20), !tbaa !2
  %42 = add i32 %41, %7
  store i32 %42, ptr getelementptr inbounds nuw (i8, ptr @"POSX&", i16 20), !tbaa !2
  %43 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"VELY&", i16 20), !tbaa !2
  %44 = add i32 %43, %6
  store i32 %44, ptr getelementptr inbounds nuw (i8, ptr @"POSY&", i16 20), !tbaa !2
  %45 = add nuw i32 %16, 1
  %46 = icmp eq i32 %16, %5
  br i1 %46, label %b14, label %b13

b23:
  %47 = phi i16 [ %87, %b27 ], [ -24, %b18 ]
  %48 = phi i32 [ 0, %b18 ], [ %86, %b27 ]
  %49 = phi i32 [ 0, %b18 ], [ %85, %b27 ]
  %50 = add i16 %17, %47
  %51 = icmp eq i16 %50, -24
  br i1 %51, label %b27, label %b25

b24:
  %52 = getelementptr inbounds nuw [4 x i8], ptr @"VELX&", i16 %18
  %53 = load i32, ptr %52, !tbaa !2
  %54 = add i32 %53, %85
  %55 = getelementptr inbounds nuw [4 x i8], ptr @"VELY&", i16 %18
  %56 = load i32, ptr %55, !tbaa !2
  %57 = add i32 %56, %86
  %58 = sdiv i32 %54, -16
  %59 = add i32 %58, %54
  store i32 %59, ptr %52, !tbaa !2
  %60 = sdiv i32 %57, -16
  %61 = add i32 %60, %57
  store i32 %61, ptr %55, !tbaa !2
  %62 = add nuw nsw i16 %18, 1
  %exitcond2 = icmp eq i16 %62, 6
  %63 = add nsw i16 %17, -4
  br i1 %exitcond2, label %b19, label %b18

b25:
  %64 = getelementptr i8, ptr @"POSX&", i16 %47
  %65 = getelementptr i8, ptr %64, i16 24
  %66 = load i32, ptr %65, !tbaa !2
  %67 = mul i16 %18, 4
  %68 = getelementptr i8, ptr @"POSX&", i16 %67
  %69 = load i32, ptr %68, !tbaa !2
  %70 = sub i32 %66, %69
  %71 = getelementptr i8, ptr @"POSY&", i16 %47
  %72 = getelementptr i8, ptr %71, i16 24
  %73 = load i32, ptr %72, !tbaa !2
  %74 = mul i16 %18, 4
  %75 = getelementptr i8, ptr @"POSY&", i16 %74
  %76 = load i32, ptr %75, !tbaa !2
  %77 = sub i32 %73, %76
  %78 = mul i32 %70, %70
  %79 = mul i32 %77, %77
  %80 = add i32 %78, 262144
  %81 = add i32 %80, %79
  %82 = sdiv i32 %81, 262144
  %83 = add nsw i32 %82, 1
  %84 = icmp eq i32 %83, 0
  br i1 %84, label %b33, label %b34

b27:
  %85 = phi i32 [ %91, %b34 ], [ %49, %b23 ]
  %86 = phi i32 [ %94, %b34 ], [ %48, %b23 ]
  %87 = add nsw i16 %47, 4
  %exitcond1 = icmp eq i16 %87, 0
  br i1 %exitcond1, label %b24, label %b23

b33:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b34:
  %.rhs.trunc = trunc nsw i32 %83 to i16
  %88 = sdiv i16 512, %.rhs.trunc
  %.sext = sext i16 %88 to i32
  %89 = mul i32 %70, %.sext
  %90 = sdiv i32 %89, 512
  %91 = add i32 %90, %49
  %92 = mul i32 %77, %.sext
  %93 = sdiv i32 %92, 512
  %94 = add i32 %93, %48
  br label %b27

b43:
  %95 = phi i16 [ %121, %b43 ], [ -6, %b14 ]
  %96 = phi i16 [ %120, %b43 ], [ -24, %b14 ]
  %97 = add i16 %95, 6
  %98 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI2(i16 %97)
  %99 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %98)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %99, ptr @TAG$)
  %100 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string7, ptr @TAG$)
  %101 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %100, ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %101)
  %102 = getelementptr i8, ptr @"POSX&", i16 %96
  %103 = getelementptr i8, ptr %102, i16 24
  %104 = load i32, ptr %103, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %104)
  %105 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string9, ptr @TAG$)
  %106 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %105, ptr @$string10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %106)
  %107 = getelementptr i8, ptr @"POSY&", i16 %96
  %108 = getelementptr i8, ptr %107, i16 24
  %109 = load i32, ptr %108, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %109)
  %110 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string11, ptr @TAG$)
  %111 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %110, ptr @$string12)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %111)
  %112 = getelementptr i8, ptr @"VELX&", i16 %96
  %113 = getelementptr i8, ptr %112, i16 24
  %114 = load i32, ptr %113, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %114)
  %115 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string13, ptr @TAG$)
  %116 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %115, ptr @$string14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %116)
  %117 = getelementptr i8, ptr @"VELY&", i16 %96
  %118 = getelementptr i8, ptr %117, i16 24
  %119 = load i32, ptr %118, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %119)
  %120 = add nsw i16 %96, 4
  %121 = add nsw i16 %95, 1
  %exitcond5 = icmp eq i16 %120, 0
  br i1 %exitcond5, label %b44, label %b43

b44:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string15)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 ptr @llrm.qb.B$FCMD() addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$FVAL(ptr) addrspace(1) nocallback

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare cc 1005 ptr @llrm.qb.B$STI2(i16) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i32 @llvm.smax.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
