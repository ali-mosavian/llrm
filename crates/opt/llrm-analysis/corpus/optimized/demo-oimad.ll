target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"DSPBASE%" = internal global [2 x i8] zeroinitializer
@BUFFER$ = internal global [32767 x i8] zeroinitializer
@"BUFOFS&" = internal global [4 x i8] zeroinitializer
@"TEXT%" = internal global [9062 x i8] zeroinitializer, align 2
@TEXT$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"TEXT%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"TEXT%", [6 x i8] c"\02\00\B3\11\00\00" }>
@"MASK%" = internal global [9062 x i8] zeroinitializer, align 2
@MASK$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"MASK%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"MASK%", [6 x i8] c"\02\00\B3\11\00\00" }>
@b$seg = external global [2 x i8]
@$string7 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string7, i16 4), [8 x i8] c"text.bsv" }>
@$string8 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string8, i16 4), [8 x i8] c"mask.bsv" }>
@$string9 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string9, i16 4), [8 x i8] c"dem1.raw" }>
@$arguments$20 = internal global [36 x i8] zeroinitializer
@"X2%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@$string11 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string11, i16 4), [20 x i8] c"press Escape to exit" }>
@$string13 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string13, i16 4), [8 x i8] c"-NOSOUND" }>
@$string14 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string14, i16 4), [8 x i8] c"pal.dat\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string15, i16 4), [2 x i8] c"V\00" }>
@$string16 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string16, i16 4), [4 x i8] c".BIN" }>
@$string17 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string17, i16 4), [2 x i8] c"P\00" }>
@$string18 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string18, i16 4), [4 x i8] c".BIN" }>
@$string19 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string19, i16 4), [2 x i8] c"T\00" }>
@$string20 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string20, i16 4), [4 x i8] c".BIN" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn {
b1:
  %0 = sext i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @BUFFER$ to ptr addrspace(2)) to i16) to i32
  %1 = shl nsw i32 %0, 4
  %2 = sext i16 ptrtoint (ptr addrspace(1) addrspacecast (ptr @BUFFER$ to ptr addrspace(1)) to i16) to i32
  %3 = add nsw i32 %1, %2
  store i32 %3, ptr @"BUFOFS&", !tbaa !2
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"TEXT%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string7, i16 ptrtoint (ptr addrspace(1) addrspacecast (ptr @"TEXT%" to ptr addrspace(1)) to i16), i16 1)
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"MASK%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string8, i16 ptrtoint (ptr addrspace(1) addrspacecast (ptr @"MASK%" to ptr addrspace(1)) to i16), i16 1)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  store i16 544, ptr @"DSPBASE%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string9, i16 1, i16 -1, i16 32)
  tail call cc 1000 addrspace(1) void @INIT()
  store i16 0, ptr @$arguments$20, align 16, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr @$arguments$20)
  %4 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %5 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %promoted1 = load i16, ptr @"X%", align 1
  %promoted2 = load i16, ptr @"Y%", align 1
  %promoted3 = load i16, ptr @"X2%", align 1, !tbaa !2
  %promoted4 = load i32, ptr @"BENCHFRAME&", align 1
  br label %b3

b3:
  %6 = phi i32 [ %77, %b22 ], [ %promoted4, %b1 ]
  %7 = phi i16 [ %11, %b22 ], [ %promoted3, %b1 ]
  %8 = phi i16 [ %74, %b22 ], [ %promoted2, %b1 ]
  %9 = phi i16 [ %75, %b22 ], [ %promoted1, %b1 ]
  %10 = phi i16 [ %15, %b22 ], [ 1, %b1 ]
  %11 = add i16 %10, %7
  store i16 %11, ptr @"X2%", !tbaa !2
  %12 = icmp sgt i16 %11, 150
  br i1 %12, label %14, label %b7

b4:
  store i16 1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 32), align 16, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 32))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b7:
  %13 = icmp slt i16 %11, 20
  br i1 %13, label %14, label %b10

14:
  %sink = phi i16 [ -1, %b3 ], [ 1, %b7 ]
  br label %b10

b10:
  %15 = phi i16 [ %10, %b7 ], [ %sink, %14 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %11, i16 100)
  store i16 16, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 2), !tbaa !2
  store float 1.600000e+01, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 4), !tbaa !2
  %16 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %17 = load float, ptr %16
  store i16 64, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 8), align 8, !tbaa !2
  %18 = fmul float %17, 6.400000e+01
  %19 = tail call float @llvm.rint.f32(float %18)
  %20 = fcmp olt float %18, %19
  %21 = sext i1 %20 to i16
  store i16 %21, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 10), !tbaa !2
  %22 = sitofp i16 %21 to float
  %23 = fadd float %19, %22
  store i16 200, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 12), align 4, !tbaa !2
  %24 = fadd float %23, 2.000000e+02
  %25 = tail call i16 @llvm.lrint.i16.f32(float %24)
  %26 = load float, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 4), !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %26, i16 %25)
  %27 = srem i32 %6, 8
  %28 = icmp eq i32 %27, 0
  br i1 %28, label %b11, label %b16

b11:
  %29 = add i16 %9, 15
  %30 = add i16 %8, 15
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %29, i16 %30)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) addrspacecast (ptr @"MASK%" to ptr addrspace(1)), ptr @MASK$descriptor, i16 1)
  %31 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %32 = load float, ptr %31
  store i16 10, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 14), !tbaa !2
  %33 = fmul float %32, 1.000000e+01
  %34 = tail call float @llvm.rint.f32(float %33)
  %35 = fcmp olt float %33, %34
  %36 = sext i1 %35 to i16
  store i16 %36, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 16), align 16, !tbaa !2
  %37 = sitofp i16 %36 to float
  %38 = fadd float %34, %37
  %39 = tail call i16 @llvm.lrint.i16.f32(float %38)
  store i16 %39, ptr @"X%", !tbaa !2
  %40 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %41 = load float, ptr %40
  store i16 10, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 18), !tbaa !2
  %42 = fmul float %41, 1.000000e+01
  %43 = tail call float @llvm.rint.f32(float %42)
  %44 = fcmp olt float %42, %43
  %45 = sext i1 %44 to i16
  store i16 %45, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 20), align 4, !tbaa !2
  %46 = sitofp i16 %45 to float
  %47 = fadd float %43, %46
  %48 = tail call i16 @llvm.lrint.i16.f32(float %47)
  store i16 %48, ptr @"Y%", !tbaa !2
  %49 = add i16 %39, 15
  %50 = add i16 %48, 15
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %49, i16 %50)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) addrspacecast (ptr @"MASK%" to ptr addrspace(1)), ptr @MASK$descriptor, i16 1)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %49, i16 %50)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) addrspacecast (ptr @"TEXT%" to ptr addrspace(1)), ptr @TEXT$descriptor, i16 0)
  %51 = add i16 %39, 80
  %52 = add i16 %48, 150
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %51, i16 %52)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @"MASK%", i16 3000) to ptr addrspace(1)), ptr @MASK$descriptor, i16 1)
  %53 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %54 = load float, ptr %53
  store i16 10, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 22), !tbaa !2
  %55 = fmul float %54, 1.000000e+01
  %56 = tail call float @llvm.rint.f32(float %55)
  %57 = fcmp olt float %55, %56
  %58 = sext i1 %57 to i16
  store i16 %58, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 24), align 8, !tbaa !2
  %59 = sitofp i16 %58 to float
  %60 = fadd float %56, %59
  %61 = tail call i16 @llvm.lrint.i16.f32(float %60)
  store i16 %61, ptr @"X%", !tbaa !2
  %62 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %63 = load float, ptr %62
  store i16 10, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 26), !tbaa !2
  %64 = fmul float %63, 1.000000e+01
  %65 = tail call float @llvm.rint.f32(float %64)
  %66 = fcmp olt float %64, %65
  %67 = sext i1 %66 to i16
  store i16 %67, ptr getelementptr inbounds nuw (i8, ptr @$arguments$20, i16 28), align 4, !tbaa !2
  %68 = sitofp i16 %67 to float
  %69 = fadd float %65, %68
  %70 = tail call i16 @llvm.lrint.i16.f32(float %69)
  store i16 %70, ptr @"Y%", !tbaa !2
  %71 = add i16 %61, 80
  %72 = add i16 %70, 150
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %71, i16 %72)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @"MASK%", i16 3000) to ptr addrspace(1)), ptr @MASK$descriptor, i16 1)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %71, i16 %72)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @"TEXT%", i16 3000) to ptr addrspace(1)), ptr @TEXT$descriptor, i16 0)
  %73 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  br label %b16

b16:
  %74 = phi i16 [ %70, %b11 ], [ %8, %b10 ]
  %75 = phi i16 [ %61, %b11 ], [ %9, %b10 ]
  %76 = icmp sgt i32 %6, 1500
  br i1 %76, label %b20, label %b22

b20:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 150, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 8, i16 1, i16 7, i16 4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11)
  br label %b22

b22:
  %77 = add i32 %6, 1
  store i32 %77, ptr @"BENCHFRAME&", !tbaa !2
  %78 = icmp sgt i32 %77, 2999
  br i1 %78, label %b4, label %b3
}

define cc 1000 range(i16 0, 2) i16 @DMADONE(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" willreturn nounwind nofree norecurse {
b1:
  %1 = tail call i8 @llrm.ia16.in.i8(i16 3)
  %2 = tail call i8 @llrm.ia16.in.i8(i16 3)
  %3 = zext i8 %2 to i32
  %4 = shl nuw nsw i32 %3, 8
  %5 = zext i8 %1 to i32
  %6 = or disjoint i32 %4, %5
  %7 = load i16, ptr %0
  %8 = add i16 %7, -1
  %9 = sext i16 %8 to i32
  %10 = icmp sgt i32 %6, %9
  br i1 %10, label %b2, label %b5

b2:
  %11 = load i16, ptr @"DSPBASE%", !tbaa !2
  %12 = add i16 %11, 14
  %13 = tail call i8 @llrm.ia16.in.i8(i16 %12)
  br label %b5

b5:
  %14 = phi i16 [ 1, %b2 ], [ 0, %b1 ]
  ret i16 %14
}

define cc 1000 void @INIT() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = load i16, ptr @"DSPBASE%", !tbaa !2
  %1 = add i16 %0, 6
  tail call void @llrm.ia16.out.i8(i16 %1, i8 1)
  %2 = tail call i8 @llrm.ia16.in.i8(i16 %1)
  %3 = tail call i8 @llrm.ia16.in.i8(i16 %1)
  %4 = tail call i8 @llrm.ia16.in.i8(i16 %1)
  %5 = tail call i8 @llrm.ia16.in.i8(i16 %1)
  tail call void @llrm.ia16.out.i8(i16 %1, i8 0)
  %6 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %7 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$INS2(ptr %6, ptr @$string13)
  %8 = icmp eq i16 %7, 0
  %9 = load i16, ptr @"DSPBASE%", !tbaa !2
  %10 = add i16 %9, 14
  %11 = tail call i8 @llrm.ia16.in.i8(i16 %10)
  %12 = ashr i8 %11, 7
  %13 = add i16 %9, 10
  %14 = tail call i8 @llrm.ia16.in.i8(i16 %13)
  %15 = icmp eq i8 %14, -86
  %16 = select i1 %15, i8 %12, i8 0
  %17 = icmp eq i8 %16, 0
  %18 = select i1 %17, i1 %8, i1 false
  %19 = select i1 %18, float -1.000000e+00, float 0.000000e+00
  %20 = add i16 %9, 12
  br label %21

21:
  %22 = tail call i8 @llrm.ia16.in.i8(i16 %20)
  %23 = icmp sgt i8 %22, -1
  br i1 %23, label %24, label %21

24:
  tail call void @llrm.ia16.out.i8(i16 %20, i8 -47)
  %25 = fcmp une float %19, 0.000000e+00
  br i1 %25, label %b13, label %b15

b13:
  tail call cc 1000 addrspace(1) void @PLAYMUSIC()
  br label %b15

b15:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string14, i16 -1536, i16 1)
  %26 = load i16, ptr @b$seg, !tbaa !2
  %27 = inttoptr i16 %26 to ptr addrspace(2)
  br label %b19

b19:
  %28 = phi float [ 0.000000e+00, %b15 ], [ %49, %b19 ]
  %29 = addrspacecast ptr addrspace(2) %27 to ptr addrspace(1)
  %30 = tail call i16 @llvm.lrint.i16.f32(float %28)
  %31 = trunc i16 %30 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %31) memory(inaccessiblemem: readwrite)
  %32 = fmul float %28, 3.000000e+00
  %33 = fadd float %32, 6.400000e+04
  %34 = tail call i32 @llvm.lrint.i32.f32(float %33)
  %35 = trunc i32 %34 to i16
  %36 = getelementptr i8, ptr addrspace(1) %29, i16 %35
  %37 = load volatile i8, ptr addrspace(1) %36
  tail call void @llrm.ia16.out.i8(i16 969, i8 %37) memory(inaccessiblemem: readwrite)
  %38 = fadd float %33, 1.000000e+00
  %39 = tail call i32 @llvm.lrint.i32.f32(float %38)
  %40 = trunc i32 %39 to i16
  %41 = getelementptr i8, ptr addrspace(1) %29, i16 %40
  %42 = load volatile i8, ptr addrspace(1) %41
  tail call void @llrm.ia16.out.i8(i16 969, i8 %42) memory(inaccessiblemem: readwrite)
  %43 = fadd float %33, 2.000000e+00
  %44 = tail call i32 @llvm.lrint.i32.f32(float %43)
  %45 = trunc i32 %44 to i16
  %46 = getelementptr i8, ptr addrspace(1) %29, i16 %45
  %47 = load volatile i8, ptr addrspace(1) %46
  tail call void @llrm.ia16.out.i8(i16 969, i8 %47) memory(inaccessiblemem: readwrite)
  %48 = fadd float %28, 1.000000e+00
  %49 = fpround float %48 to float
  %50 = fcmp ole float %49, 2.550000e+02
  br i1 %50, label %b19, label %b20

b20:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  ret void
}

define cc 1000 void @PLAYMUSIC() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) addrspacecast (ptr @BUFFER$ to ptr addrspace(1)), i16 32767)
  tail call void @llrm.ia16.out.i8(i16 10, i8 5)
  tail call void @llrm.ia16.out.i8(i16 12, i8 2)
  tail call void @llrm.ia16.out.i8(i16 11, i8 73)
  tail call void @llrm.ia16.out.i8(i16 2, i8 0)
  tail call void @llrm.ia16.out.i8(i16 2, i8 0)
  %1 = load i32, ptr @"BUFOFS&", !tbaa !2
  %2 = trunc i32 %1 to i8
  %3 = and i32 %1, 255
  %4 = sub i32 %1, %3
  %5 = sdiv i32 %4, 255
  tail call void @llrm.ia16.out.i8(i16 3, i8 %2)
  %6 = trunc i32 %5 to i8
  tail call void @llrm.ia16.out.i8(i16 3, i8 %6)
  %7 = sdiv i32 %1, 65536
  %8 = trunc i32 %7 to i8
  tail call void @llrm.ia16.out.i8(i16 131, i8 %8)
  tail call void @llrm.ia16.out.i8(i16 10, i8 1)
  %9 = load i16, ptr @"DSPBASE%", !tbaa !2
  %10 = add i16 %9, 12
  br label %11

11:
  %12 = tail call i8 @llrm.ia16.in.i8(i16 %10)
  %13 = icmp sgt i8 %12, -1
  br i1 %13, label %14, label %11

14:
  tail call void @llrm.ia16.out.i8(i16 %10, i8 64)
  br label %15

15:
  %16 = tail call i8 @llrm.ia16.in.i8(i16 %10)
  %17 = icmp sgt i8 %16, -1
  br i1 %17, label %18, label %15

18:
  tail call void @llrm.ia16.out.i8(i16 %10, i8 6)
  br label %19

19:
  %20 = tail call i8 @llrm.ia16.in.i8(i16 %10)
  %21 = icmp sgt i8 %20, -1
  br i1 %21, label %22, label %19

22:
  tail call void @llrm.ia16.out.i8(i16 %10, i8 20)
  br label %23

23:
  %24 = tail call i8 @llrm.ia16.in.i8(i16 %10)
  %25 = icmp sgt i8 %24, -1
  br i1 %25, label %26, label %23

26:
  tail call void @llrm.ia16.out.i8(i16 %10, i8 %2)
  br label %27

27:
  %28 = tail call i8 @llrm.ia16.in.i8(i16 %10)
  %29 = icmp sgt i8 %28, -1
  br i1 %29, label %30, label %27

30:
  tail call void @llrm.ia16.out.i8(i16 %10, i8 %6)
  ret void
}

define cc 1000 void @SENDDSP(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree norecurse {
b1:
  %1 = load i16, ptr @"DSPBASE%", !tbaa !2
  %2 = add i16 %1, 12
  br label %b2

b2:
  %3 = tail call i8 @llrm.ia16.in.i8(i16 %2)
  %4 = icmp sgt i8 %3, -1
  br i1 %4, label %b4, label %b2

b4:
  %5 = load i16, ptr %0
  %6 = trunc i16 %5 to i8
  tail call void @llrm.ia16.out.i8(i16 %2, i8 %6)
  ret void
}

define cc 1000 void @BENCHMARK(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca [36 x i8]
  %4 = getelementptr inbounds nuw i8, ptr %3, i16 18
  call void @llvm.memset.p0.i16(ptr align 2 %3, i8 0, i16 36, i1 false)
  store i32 0, ptr %1, !tbaa !2
  store i32 0, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 767, i16 2, i16 257, ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 5, i16 4, i16 257, ptr nonnull %4)
  call cc 1000 addrspace(1) void @TSCSNAP(ptr nonnull %1, ptr nonnull %2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  br label %b5

b5:
  %5 = phi ptr addrspace(1) [ %16, %b5 ], [ addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), %b1 ]
  %6 = phi i32 [ %15, %b5 ], [ 64000, %b1 ]
  %7 = phi i32 [ 1, %b1 ], [ %12, %b5 ]
  %8 = phi i32 [ 0, %b1 ], [ %14, %b5 ]
  %9 = load volatile i8, ptr addrspace(1) %5
  %10 = zext i8 %9 to i32
  %11 = add i32 %7, %10
  %12 = srem i32 %11, 65521
  %13 = add i32 %12, %8
  %14 = srem i32 %13, 65521
  %15 = add nsw i32 %6, -1
  %16 = getelementptr i8, ptr addrspace(1) %5, i16 1
  %exitcond1 = icmp eq i32 %15, 0
  br i1 %exitcond1, label %b6, label %b5

b6:
  %17 = load i16, ptr %0
  %18 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %17)
  %19 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %18)
  %20 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string15, ptr %19)
  %21 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %20, ptr @$string16)
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %21, i16 0, i16 -1536)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %22 = getelementptr i8, ptr %3, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  br label %b10

b10:
  %24 = phi i16 [ %31, %b10 ], [ -1536, %b6 ]
  %25 = inttoptr i16 %23 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 %24
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 1536
  %29 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %30 = zext i8 %29 to i16
  store i16 %30, ptr addrspace(1) %28, !tbaa !6
  %31 = add nsw i16 %24, 2
  %exitcond = icmp eq i16 %31, 0
  br i1 %exitcond, label %b11, label %b10

b11:
  %32 = getelementptr inbounds nuw i8, ptr %3, i16 20
  %33 = getelementptr i8, ptr %3, i16 2
  %34 = load i16, ptr %33, !tbaa !2
  %35 = inttoptr i16 %34 to ptr addrspace(2)
  %36 = addrspacecast ptr addrspace(2) %35 to ptr addrspace(1)
  %37 = addrspacecast ptr addrspace(1) %36 to ptr addrspace(2)
  %38 = ptrtoint ptr addrspace(2) %37 to i16
  store i16 %38, ptr @b$seg, !tbaa !2
  %39 = load i16, ptr %0
  %40 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %39)
  %41 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %40)
  %42 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string17, ptr %41)
  %43 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %42, ptr @$string18)
  %44 = load i16, ptr %33, !tbaa !2
  %45 = inttoptr i16 %44 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = ptrtoint ptr addrspace(1) %46 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %43, i16 %47, i16 1536)
  %48 = load i16, ptr %32, !tbaa !2
  %49 = inttoptr i16 %48 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = load i32, ptr @"BENCHHI&", !tbaa !2
  store i32 %51, ptr addrspace(1) %50, !tbaa !8
  %52 = getelementptr i8, ptr addrspace(1) %50, i16 4
  %53 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %53, ptr addrspace(1) %52, !tbaa !8
  %54 = getelementptr i8, ptr addrspace(1) %50, i16 8
  %55 = load i32, ptr %1, !tbaa !2
  store i32 %55, ptr addrspace(1) %54, !tbaa !8
  %56 = getelementptr i8, ptr addrspace(1) %50, i16 12
  %57 = load i32, ptr %2, !tbaa !2
  store i32 %57, ptr addrspace(1) %56, !tbaa !8
  %58 = getelementptr i8, ptr addrspace(1) %50, i16 16
  store i32 %14, ptr addrspace(1) %58, !tbaa !8
  %59 = getelementptr i8, ptr addrspace(1) %50, i16 20
  store i32 %12, ptr addrspace(1) %59, !tbaa !8
  %60 = addrspacecast ptr addrspace(1) %50 to ptr addrspace(2)
  %61 = ptrtoint ptr addrspace(2) %60 to i16
  store i16 %61, ptr @b$seg, !tbaa !2
  %62 = load i16, ptr %0
  %63 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %62)
  %64 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %63)
  %65 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string19, ptr %64)
  %66 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %65, ptr @$string20)
  %67 = load i16, ptr %32, !tbaa !2
  %68 = inttoptr i16 %67 to ptr addrspace(2)
  %69 = addrspacecast ptr addrspace(2) %68 to ptr addrspace(1)
  %70 = ptrtoint ptr addrspace(1) %69 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %66, i16 %70, i16 24)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc 1000 addrspace(1) void @TSCSNAP(ptr @"BENCHHI&", ptr @"BENCHLO&")
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %3)
  ret void
}

declare cc 1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1) nocallback nounwind

declare cc 1000 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$TIMR() addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare cc 1000 void @llrm.qb.B$N1I2(i16, i16) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$RND0() addrspace(1) nocallback

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$CIRC(float, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$GPUT(ptr addrspace(1), ptr, i16) addrspace(1) nocallback

declare cc 1000 i16 @"DMADONE%"(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$COLR(i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$LOCT(i16, i16, i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$FCMD() addrspace(1) nocallback

declare cc 1000 i16 @llrm.qb.B$INS2(ptr, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1) nocallback

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$GET3(i16, ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1) nocallback

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
