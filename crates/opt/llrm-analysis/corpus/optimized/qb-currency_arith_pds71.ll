target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A@" = internal global [8 x i8] zeroinitializer
@"B@" = internal global [8 x i8] zeroinitializer
@"C@" = internal global [8 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$arguments$9 = internal global [702 x i8] zeroinitializer
@$string19 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string19, i16 4), [4 x i8] c"err\00" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
b7:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i64 15000, ptr @"A@", !tbaa !2
  store i64 22500, ptr @"B@", !tbaa !2
  store i64 37500, ptr @"C@", !tbaa !2
  %0 = fdiv double 1.000000e+00, 1.000000e+04
  %1 = fmul double %0, 0x3BF0000000000000
  %2 = fcmp ogt double %1, 5.000000e-01
  %3 = sext i1 %2 to i16
  %4 = fcmp olt double %1, -5.000000e-01
  %5 = sext i1 %4 to i16
  %6 = or i16 %3, %5
  %7 = icmp eq i16 %6, 0
  %8 = select i1 %7, i64 0, i64 -9223372036854775808
  br label %b8

b8:
  %9 = load i64, ptr @"C@", !tbaa !2
  store i64 %9, ptr @$arguments$9, align 16, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %9)
          to label %b9 unwind label %landing

b9:
  %10 = load i64, ptr @"A@", !tbaa !2
  %11 = load i64, ptr @"B@", !tbaa !2
  %12 = sub i64 %10, %11
  %13 = xor i64 %11, %10
  %14 = xor i64 %12, %10
  %15 = and i64 %14, %13
  %16 = icmp slt i64 %15, 0
  br i1 %16, label %b10, label %b11

b10:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %215 unwind label %landing

b11:
  store i64 %12, ptr @"C@", !tbaa !2
  br label %b12

b12:
  %17 = phi i64 [ %185, %184 ], [ %12, %b11 ]
  store i64 %17, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 8), align 8, !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %17)
          to label %b13 unwind label %landing

b13:
  %18 = load i64, ptr @"A@", !tbaa !2
  %19 = load i64, ptr @"B@", !tbaa !2
  %20 = sdiv i64 %18, 10000
  %21 = mul nsw i64 %20, -10000
  %22 = add i64 %21, %18
  %23 = sdiv i64 %19, 10000
  %24 = mul nsw i64 %23, -10000
  %25 = add i64 %24, %19
  %26 = mul i64 %20, %19
  %27 = mul i64 %22, %23
  %28 = add i64 %27, %26
  %29 = mul i64 %25, %22
  %30 = sdiv i64 %29, 10000
  %31 = mul nsw i64 %30, -10000
  %32 = add i64 %31, %29
  %33 = icmp slt i64 %32, 0
  br i1 %33, label %b17, label %b18

b17:
  %34 = add nsw i64 %30, -1
  store i64 %34, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 24), align 8, !tbaa !2
  %35 = add nsw i64 %32, 10000
  br label %b19

b18:
  store i64 %30, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 24), align 8, !tbaa !2
  br label %b19

b19:
  %sink = phi i64 [ %35, %b17 ], [ %32, %b18 ]
  %36 = phi i64 [ %34, %b17 ], [ %30, %b18 ]
  store i64 %sink, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 32), align 16, !tbaa !2
  %37 = add i64 %28, %36
  %38 = trunc i64 %37 to i16
  %39 = icmp sgt i64 %sink, 5000
  %40 = sext i1 %39 to i16
  %41 = icmp eq i64 %sink, 5000
  %42 = and i16 %38, 1
  %43 = sub nsw i16 0, %42
  %44 = select i1 %41, i16 %43, i16 0
  %45 = or i16 %44, %40
  %46 = icmp eq i16 %45, 0
  %47 = xor i1 %46, true
  %48 = zext i1 %47 to i64
  %49 = add i64 %37, %48
  store i64 %49, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 40), align 8, !tbaa !2
  %50 = sitofp i64 %18 to double
  %51 = sitofp i64 %19 to double
  %52 = fmul double %50, %51
  %53 = fdiv double %52, 1.000000e+04
  %54 = sitofp i64 %49 to double
  %55 = fsub double %53, %54
  %56 = fmul double %55, 0x3BF0000000000000
  %57 = fcmp ogt double %56, 5.000000e-01
  %58 = sext i1 %57 to i16
  %59 = fcmp olt double %56, -5.000000e-01
  %60 = sext i1 %59 to i16
  %61 = or i16 %58, %60
  %62 = icmp eq i16 %61, 0
  %63 = select i1 %62, i64 %49, i64 -9223372036854775808
  store i64 %63, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 48), align 16, !tbaa !2
  store i64 %63, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 16), align 16, !tbaa !2
  store i64 %63, ptr @"C@", !tbaa !2
  store i64 %63, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 56), align 8, !tbaa !2
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %63)
          to label %b27 unwind label %landing

b27:
  %64 = load i64, ptr @"A@", !tbaa !2
  %65 = load i64, ptr @"B@", !tbaa !2
  %66 = sitofp i64 %64 to double
  %67 = fdiv double %66, 1.000000e+04
  %68 = sitofp i64 %65 to double
  %69 = fdiv double %68, 1.000000e+04
  %70 = fdiv double %67, %69
  %71 = fpext double %70 to x86_fp80
  %72 = fmul x86_fp80 %71, 1.000000e+04
  %73 = call i64 @llvm.lrint.i64.f80(x86_fp80 %72)
  store i64 %73, ptr @"C@", !tbaa !2
  store i64 %73, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 64), align 16, !tbaa !2
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %73)
          to label %b40 unwind label %landing

b40:
  store i64 1, ptr @"A@", !tbaa !2
  store i64 5000, ptr @"B@", !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 80), align 16, !tbaa !2
  store i64 5000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 88), align 8, !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 72), align 8, !tbaa !2
  call void @llvm.memset.p0.i16(ptr align 16 getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 96), i8 0, i16 16, i1 false)
  store i64 0, ptr @"C@", !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 112), align 16, !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 0)
          to label %b56 unwind label %landing

b56:
  store i64 3, ptr @"A@", !tbaa !2
  store i64 5000, ptr @"B@", !tbaa !2
  store i64 1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 128), align 16, !tbaa !2
  store i64 5000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 136), align 8, !tbaa !2
  store i64 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 144), align 16, !tbaa !2
  store i64 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 152), align 8, !tbaa !2
  store i64 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 120), align 8, !tbaa !2
  store i64 2, ptr @"C@", !tbaa !2
  store i64 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 160), align 16, !tbaa !2
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 2)
          to label %b72 unwind label %landing

b72:
  store i64 -1, ptr @"A@", !tbaa !2
  store i64 5000, ptr @"B@", !tbaa !2
  store i64 -1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 176), align 16, !tbaa !2
  store i64 5000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 184), align 8, !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 168), align 8, !tbaa !2
  call void @llvm.memset.p0.i16(ptr align 16 getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 192), i8 0, i16 16, i1 false)
  store i64 0, ptr @"C@", !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 208), align 16, !tbaa !2
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 0)
          to label %b88 unwind label %landing

b88:
  store i64 -3, ptr @"A@", !tbaa !2
  store i64 5000, ptr @"B@", !tbaa !2
  store i64 -2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 224), align 16, !tbaa !2
  store i64 5000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 232), align 8, !tbaa !2
  store i64 -2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 240), align 16, !tbaa !2
  store i64 -2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 248), align 8, !tbaa !2
  store i64 -2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 216), align 8, !tbaa !2
  store i64 -2, ptr @"C@", !tbaa !2
  store i64 -2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 256), align 16, !tbaa !2
  store i16 14, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 -2)
          to label %b104 unwind label %landing

b104:
  store i64 1, ptr @"A@", !tbaa !2
  store i64 1, ptr @"B@", !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 272), align 16, !tbaa !2
  store i64 1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 280), align 8, !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 288), align 16, !tbaa !2
  store i64 %8, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 296), align 8, !tbaa !2
  store i64 %8, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 264), align 8, !tbaa !2
  store i64 %8, ptr @"C@", !tbaa !2
  store i64 %8, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 304), align 16, !tbaa !2
  store i16 16, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %8)
          to label %b112 unwind label %landing

b112:
  store i64 9223372036854775807, ptr @"A@", !tbaa !2
  store i64 10000, ptr @"B@", !tbaa !2
  store i16 17, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %216 unwind label %landing

b126:
  store i64 9223372036854775807, ptr @"A@", !tbaa !2
  store i64 20000, ptr @"B@", !tbaa !2
  call void @llvm.memset.p0.i16(ptr align 8 getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 328), i8 0, i16 16, i1 false)
  store i64 -2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 344), align 8, !tbaa !2
  %74 = fsub double 0x43F0000000000000, -2.000000e+00
  %75 = fmul double %74, 0x3BF0000000000000
  %76 = fcmp ogt double %75, 5.000000e-01
  %77 = sext i1 %76 to i16
  %78 = fcmp olt double %75, -5.000000e-01
  %79 = sext i1 %78 to i16
  %80 = or i16 %77, %79
  %81 = icmp eq i16 %80, 0
  %82 = select i1 %81, i64 -2, i64 -9223372036854775808
  store i64 %82, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 352), align 16, !tbaa !2
  store i64 %82, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 320), align 16, !tbaa !2
  store i64 %82, ptr @"C@", !tbaa !2
  store i64 %82, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 360), align 8, !tbaa !2
  store i16 20, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %82)
          to label %b142 unwind label %landing

b142:
  store i64 9223372036854775807, ptr @"A@", !tbaa !2
  store i64 10001, ptr @"B@", !tbaa !2
  store i64 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 376), align 8, !tbaa !2
  store i64 5807, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 384), align 16, !tbaa !2
  store i64 -9222449699651090331, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 392), align 8, !tbaa !2
  %83 = fdiv double 0x44B3888000000000, 1.000000e+04
  %84 = fsub double %83, 0xC3DFFF2E48E8A71E
  %85 = fmul double %84, 0x3BF0000000000000
  %86 = fcmp ogt double %85, 5.000000e-01
  %87 = sext i1 %86 to i16
  %88 = fcmp olt double %85, -5.000000e-01
  %89 = sext i1 %88 to i16
  %90 = or i16 %87, %89
  %91 = icmp eq i16 %90, 0
  %92 = select i1 %91, i64 -9222449699651090331, i64 -9223372036854775808
  store i64 %92, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 400), align 16, !tbaa !2
  store i64 %92, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 368), align 16, !tbaa !2
  store i64 %92, ptr @"C@", !tbaa !2
  store i64 %92, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 408), align 8, !tbaa !2
  store i16 22, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %92)
          to label %b150 unwind label %landing

b150:
  store i64 -9223372036854775807, ptr @"A@", !tbaa !2
  store i64 10000, ptr @"B@", !tbaa !2
  store i16 23, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %217 unwind label %landing

b164:
  store i64 10000000000, ptr @"A@", !tbaa !2
  store i64 10000000000, ptr @"B@", !tbaa !2
  call void @llvm.memset.p0.i16(ptr align 16 getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 432), i8 0, i16 16, i1 false)
  store i64 10000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 448), align 16, !tbaa !2
  store i64 10000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 456), align 8, !tbaa !2
  store i64 10000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 424), align 8, !tbaa !2
  store i64 10000000000000000, ptr @"C@", !tbaa !2
  store i64 10000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 464), align 16, !tbaa !2
  store i16 26, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 10000000000000000)
          to label %b180 unwind label %landing

b180:
  store i64 10000000000, ptr @"A@", !tbaa !2
  store i64 100000000000, ptr @"B@", !tbaa !2
  call void @llvm.memset.p0.i16(ptr align 16 getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 480), i8 0, i16 16, i1 false)
  store i64 100000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 496), align 16, !tbaa !2
  store i64 100000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 504), align 8, !tbaa !2
  store i64 100000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 472), align 8, !tbaa !2
  store i64 100000000000000000, ptr @"C@", !tbaa !2
  store i64 100000000000000000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 512), align 16, !tbaa !2
  store i16 28, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 100000000000000000)
          to label %b185 unwind label %landing

b185:
  store i64 50000, ptr @"A@", !tbaa !2
  store i64 0, ptr @"B@", !tbaa !2
  %93 = tail call i64 @llvm.lrint.i64.f80(x86_fp80 0x7FF0000000000000)
  store i64 %93, ptr @"C@", !tbaa !2
  store i64 %93, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 520), align 8, !tbaa !2
  store i16 29, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %93)
          to label %b189 unwind label %landing

b189:
  %94 = load i64, ptr @"A@", !tbaa !2
  %95 = add i64 %94, 10000
  %96 = xor i64 %95, %94
  %97 = xor i64 %95, 10000
  %98 = and i64 %96, %97
  %99 = icmp slt i64 %98, 0
  br i1 %99, label %b190, label %b191

b190:
  store i16 30, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %238 unwind label %landing

b191:
  store i64 %95, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 528), align 16, !tbaa !2
  store i16 30, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSCY(i64 %95)
          to label %b200 unwind label %landing

b200:
  %100 = load i64, ptr @"A@", !tbaa !2
  %101 = sdiv i64 %100, 10000
  %102 = mul nsw i64 %101, -10000
  %103 = add i64 %102, %100
  %104 = mul i64 %101, 20000
  %105 = shl i64 %103, 1
  %106 = add i64 %105, %104
  call void @llvm.memset.p0.i16(ptr align 16 getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 544), i8 0, i16 16, i1 false)
  store i64 %106, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 560), align 16, !tbaa !2
  %107 = sitofp i64 %100 to double
  %108 = fmul double %107, 2.000000e+04
  %109 = fdiv double %108, 1.000000e+04
  %110 = sitofp i64 %106 to double
  %111 = fsub double %109, %110
  %112 = fmul double %111, 0x3BF0000000000000
  %113 = fcmp ogt double %112, 5.000000e-01
  %114 = sext i1 %113 to i16
  %115 = fcmp olt double %112, -5.000000e-01
  %116 = sext i1 %115 to i16
  %117 = or i16 %114, %116
  %118 = icmp eq i16 %117, 0
  %119 = select i1 %118, i64 %106, i64 -9223372036854775808
  store i64 %119, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 568), align 8, !tbaa !2
  store i64 %119, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 536), align 8, !tbaa !2
  store i64 %119, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 576), align 16, !tbaa !2
  store i16 30, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSCY(i64 %119)
          to label %218 unwind label %landing

b212:
  %120 = load i64, ptr @"A@", !tbaa !2
  %121 = load i16, ptr @"I%", !tbaa !2
  %122 = sext i16 %121 to i64
  %123 = mul nsw i64 %122, 10000
  %124 = sdiv i64 %120, 10000
  %125 = mul nsw i64 %124, -10000
  %126 = add i64 %125, %120
  %127 = mul i64 %123, %124
  %128 = mul i64 %126, %122
  %129 = add i64 %127, %128
  call void @llvm.memset.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 618), i8 0, i16 16, i1 false)
  store i64 %129, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 634), !tbaa !2
  %130 = sitofp i64 %120 to double
  %131 = sitofp i64 %123 to double
  %132 = fmul double %130, %131
  %133 = fdiv double %132, 1.000000e+04
  %134 = sitofp i64 %129 to double
  %135 = fsub double %133, %134
  %136 = fmul double %135, 0x3BF0000000000000
  %137 = fcmp ogt double %136, 5.000000e-01
  %138 = sext i1 %137 to i16
  %139 = fcmp olt double %136, -5.000000e-01
  %140 = sext i1 %139 to i16
  %141 = or i16 %138, %140
  %142 = icmp eq i16 %141, 0
  %143 = select i1 %142, i64 %129, i64 -9223372036854775808
  store i64 %143, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 642), !tbaa !2
  store i64 %143, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 610), !tbaa !2
  store i64 %143, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 650), !tbaa !2
  store i16 30, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSCY(i64 %143)
          to label %236 unwind label %landing

b218:
  %144 = load i64, ptr @"A@", !tbaa !2
  %145 = load i64, ptr @"B@", !tbaa !2
  %146 = icmp slt i64 %144, %145
  %147 = sext i1 %146 to i16
  store i16 31, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %147)
          to label %239 unwind label %landing

b219:
  store i64 15000, ptr @"A@", !tbaa !2
  store i64 15000, ptr @"B@", !tbaa !2
  store i16 32, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 0)
          to label %264 unwind label %landing

b222:
  store i16 3, ptr @"I%", !tbaa !2
  %148 = load i64, ptr @"A@", !tbaa !2
  %149 = icmp slt i64 %148, 30000
  %150 = sext i1 %149 to i16
  store i16 33, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %150)
          to label %274 unwind label %landing

b224:
  %151 = load i64, ptr @"A@", !tbaa !2
  %152 = sub i64 0, %151
  store i64 %152, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 666), !tbaa !2
  store i16 34, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSCY(i64 %152)
          to label %296 unwind label %landing

b225:
  store i16 34, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %305 unwind label %landing

b226:
  store i64 %300, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 674), !tbaa !2
  store i16 34, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %300)
          to label %b227 unwind label %landing

b227:
  %153 = load i64, ptr @"A@", !tbaa !2
  %154 = load i16, ptr @"I%", !tbaa !2
  %155 = sext i16 %154 to i64
  %156 = mul nsw i64 %155, 10000
  %157 = add i64 %156, %153
  %158 = xor i64 %157, %153
  %159 = xor i64 %157, %156
  %160 = and i64 %158, %159
  %161 = icmp slt i64 %160, 0
  br i1 %161, label %b228, label %b230

b228:
  store i16 35, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %306 unwind label %landing

b230:
  %162 = phi i64 [ %212, %211 ], [ %153, %b227 ]
  %163 = sitofp i64 %162 to double
  %164 = fdiv double %163, 1.000000e+04
  %165 = fmul double %164, 2.500000e+00
  %166 = fpext double %165 to x86_fp80
  %167 = fmul x86_fp80 %166, 1.000000e+04
  %168 = call i64 @llvm.lrint.i64.f80(x86_fp80 %167)
  store i64 %168, ptr @"C@", !tbaa !2
  store i64 %168, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 682), !tbaa !2
  store i16 36, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %168)
          to label %b232 unwind label %landing

b232:
  store i64 35000, ptr @"C@", !tbaa !2
  store i64 35000, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 690), !tbaa !2
  store i16 37, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 35000)
          to label %b234 unwind label %landing

b234:
  %169 = load i64, ptr @"A@", !tbaa !2
  %170 = sitofp i64 %169 to double
  %171 = fdiv double %170, 1.000000e+04
  %172 = fptrunc double %171 to float
  %173 = fpround float %172 to float
  store float %173, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 698), !tbaa !2
  %174 = fmul float %173, %173
  %175 = fpext float %174 to double
  %176 = fpext double %175 to x86_fp80
  %177 = fmul x86_fp80 %176, 1.000000e+04
  %178 = call i64 @llvm.lrint.i64.f80(x86_fp80 %177)
  store i64 %178, ptr @"C@", !tbaa !2
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %179 = extractvalue { ptr, i32 } %landed, 1
  %180 = trunc i32 %179 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %180)
  %181 = load i16, ptr %site
  switch table i16 %181, label %182 [
    i16 0, label %182
    i16 1, label %183
    i16 2, label %184
    i16 3, label %186
    i16 4, label %187
    i16 5, label %188
    i16 6, label %189
    i16 7, label %187
    i16 8, label %190
    i16 9, label %187
    i16 10, label %191
    i16 11, label %187
    i16 12, label %192
    i16 13, label %187
    i16 14, label %193
    i16 15, label %187
    i16 16, label %194
    i16 17, label %195
    i16 18, label %197
    i16 19, label %187
    i16 20, label %198
    i16 21, label %187
    i16 22, label %199
    i16 23, label %200
    i16 24, label %202
    i16 25, label %187
    i16 26, label %203
    i16 27, label %187
    i16 28, label %204
    i16 29, label %205
    i16 30, label %206
    i16 31, label %207
    i16 32, label %208
    i16 33, label %209
    i16 34, label %210
    i16 35, label %211
    i16 36, label %213
    i16 37, label %214
  ]

182:
  br label %b8

183:
  br label %b9

184:
  %185 = load i64, ptr @"C@", !tbaa !2
  br label %b12

186:
  br label %b13

187:
  unreachable

188:
  br label %b27

189:
  br label %b40

190:
  br label %b56

191:
  br label %b72

192:
  br label %b88

193:
  br label %b104

194:
  br label %b112

195:
  %196 = load i64, ptr @"C@", !tbaa !2
  store i64 %196, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 312), align 8, !tbaa !2
  store i16 18, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %196)
          to label %b126 unwind label %landing

197:
  br label %b126

198:
  br label %b142

199:
  br label %b150

200:
  %201 = load i64, ptr @"C@", !tbaa !2
  store i64 %201, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 416), align 16, !tbaa !2
  store i16 24, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %201)
          to label %b164 unwind label %landing

202:
  br label %b164

203:
  br label %b180

204:
  br label %b185

205:
  br label %b189

206:
  br label %b218

207:
  br label %b219

208:
  br label %b222

209:
  br label %b224

210:
  br label %b227

211:
  %212 = load i64, ptr @"A@", !tbaa !2
  br label %b230

213:
  br label %b232

214:
  br label %b234

215:
  unreachable

216:
  unreachable

217:
  unreachable

218:
  %219 = load i64, ptr @"A@", !tbaa !2
  %220 = sitofp i64 %219 to double
  %221 = fdiv double %220, 1.000000e+04
  %222 = fsub double %221, 5.000000e-01
  %223 = fpround double %222 to double
  store double %223, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 584), !tbaa !2
  store i16 30, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %223)
          to label %224 unwind label %landing

224:
  %225 = load i64, ptr @"A@", !tbaa !2
  %226 = sitofp i64 %225 to double
  %227 = fdiv double %226, 1.000000e+04
  store i16 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 592), align 16, !tbaa !2
  %228 = fmul double %227, 5.000000e-01
  %229 = fpround double %228 to double
  store double %229, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 594), !tbaa !2
  store i16 30, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %229)
          to label %230 unwind label %landing

230:
  %231 = load i64, ptr @"A@", !tbaa !2
  %232 = sitofp i64 %231 to double
  %233 = fdiv double %232, 1.000000e+04
  %234 = fadd double %233, 1.000000e-01
  %235 = fpround double %234 to double
  store double %235, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 602), !tbaa !2
  store i16 30, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %235)
          to label %b212 unwind label %landing

236:
  %237 = load i64, ptr @"A@", !tbaa !2
  store i64 %237, ptr getelementptr inbounds nuw (i8, ptr @$arguments$9, i16 658), !tbaa !2
  store i16 30, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %237)
          to label %b218 unwind label %landing

238:
  unreachable

239:
  %240 = load i64, ptr @"A@", !tbaa !2
  %241 = load i64, ptr @"B@", !tbaa !2
  %242 = icmp sgt i64 %240, %241
  %243 = sext i1 %242 to i16
  store i16 31, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %243)
          to label %244 unwind label %landing

244:
  %245 = load i64, ptr @"A@", !tbaa !2
  %246 = load i64, ptr @"B@", !tbaa !2
  %247 = icmp eq i64 %245, %246
  %248 = sext i1 %247 to i16
  store i16 31, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %248)
          to label %249 unwind label %landing

249:
  %250 = load i64, ptr @"A@", !tbaa !2
  %251 = load i64, ptr @"B@", !tbaa !2
  %252 = icmp ne i64 %250, %251
  %253 = sext i1 %252 to i16
  store i16 31, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %253)
          to label %254 unwind label %landing

254:
  %255 = load i64, ptr @"A@", !tbaa !2
  %256 = load i64, ptr @"B@", !tbaa !2
  %257 = icmp sle i64 %255, %256
  %258 = sext i1 %257 to i16
  store i16 31, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %258)
          to label %259 unwind label %landing

259:
  %260 = load i64, ptr @"A@", !tbaa !2
  %261 = load i64, ptr @"B@", !tbaa !2
  %262 = icmp sge i64 %260, %261
  %263 = sext i1 %262 to i16
  store i16 31, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %263)
          to label %b219 unwind label %landing

264:
  %265 = load i64, ptr @"A@", !tbaa !2
  %266 = load i64, ptr @"B@", !tbaa !2
  %267 = icmp eq i64 %265, %266
  %268 = sext i1 %267 to i16
  store i16 32, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %268)
          to label %269 unwind label %landing

269:
  %270 = load i64, ptr @"A@", !tbaa !2
  %271 = load i64, ptr @"B@", !tbaa !2
  %272 = icmp sge i64 %270, %271
  %273 = sext i1 %272 to i16
  store i16 32, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %273)
          to label %b222 unwind label %landing

274:
  %275 = load i64, ptr @"A@", !tbaa !2
  %276 = icmp sgt i64 %275, 10000
  %277 = sext i1 %276 to i16
  store i16 33, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %277)
          to label %278 unwind label %landing

278:
  %279 = load i64, ptr @"A@", !tbaa !2
  %280 = sitofp i64 %279 to double
  %281 = fdiv double %280, 1.000000e+04
  %282 = fcmp oeq double %281, 1.500000e+00
  %283 = sext i1 %282 to i16
  store i16 33, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %283)
          to label %284 unwind label %landing

284:
  %285 = load i64, ptr @"A@", !tbaa !2
  %286 = sitofp i64 %285 to double
  %287 = fdiv double %286, 1.000000e+04
  %288 = fcmp olt double %287, 1.600000e+00
  %289 = sext i1 %288 to i16
  store i16 33, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %289)
          to label %290 unwind label %landing

290:
  %291 = load i64, ptr @"A@", !tbaa !2
  %292 = sitofp i64 %291 to double
  %293 = fdiv double %292, 1.000000e+04
  %294 = fcmp olt double %293, 0x3FF6666660000000
  %295 = sext i1 %294 to i16
  store i16 33, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %295)
          to label %b224 unwind label %landing

296:
  %297 = load i64, ptr @"A@", !tbaa !2
  %298 = load i64, ptr @"B@", !tbaa !2
  %299 = sub i64 0, %298
  %300 = sub i64 %297, %298
  %301 = xor i64 %300, %297
  %302 = xor i64 %300, %299
  %303 = and i64 %301, %302
  %304 = icmp slt i64 %303, 0
  br i1 %304, label %b225, label %b226

305:
  unreachable

306:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PECY(i64) addrspace(1)

declare i64 @llvm.lrint.i64.f80(x86_fp80) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PSCY(i64) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSR8(double) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 poison
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
