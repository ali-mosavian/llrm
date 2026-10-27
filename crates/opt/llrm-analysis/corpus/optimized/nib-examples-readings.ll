target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00raw\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00avg3\00"
@$str3 = internal constant [27 x i8] c"\08\00\14\00\14\00first pair averages \00"
@$str4 = internal constant [23 x i8] c"\08\00\10\00\10\00too few readings\00"
@$str5 = internal constant [25 x i8] c"\08\00\12\00\12\00peak of the rest: \00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [16 x i8]
  %2 = alloca [16 x i8]
  %3 = alloca [16 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [16 x i8]
  store i32 983052, ptr %6
  %7 = getelementptr inbounds nuw i8, ptr %6, i16 4
  store i32 1966091, ptr %7
  %8 = getelementptr inbounds nuw i8, ptr %6, i16 8
  store i32 1769500, ptr %8
  %9 = getelementptr inbounds nuw i8, ptr %6, i16 12
  store i32 655369, ptr %9
  store i32 196611, ptr %5
  %10 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6) to ptr addrspace(1)), ptr %10, !tbaa !2
  %11 = addrspacecast ptr %5 to ptr addrspace(1)
  %12 = addrspacecast ptr %6 to ptr addrspace(5)
  %13 = addrspacecast ptr %6 to ptr addrspace(1)
  tail call cc 1005 addrspace(1) void @N$PBEG()
  tail call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 8, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %11)
  %14 = call cc 1005 addrspace(1) ptr @N$PEND()
  br label %15

15:
  %16 = phi i16 [ %22, %15 ], [ -16, %0 ]
  %sink2 = phi ptr [ %21, %15 ], [ %14, %0 ]
  %17 = getelementptr i8, ptr addrspace(5) %12, i16 %16
  %18 = getelementptr i8, ptr addrspace(5) %17, i16 16
  %19 = load i16, ptr addrspace(5) %18
  call cc 1005 addrspace(1) void @N$PBEG()
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %19)
  %20 = call cc 1005 addrspace(1) ptr @N$PEND()
  %21 = call cc 1005 addrspace(1) ptr @N$TAPP(ptr %sink2, ptr %20)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %20)
  %22 = add nsw i16 %16, 2
  %exitcond = icmp eq i16 %22, 0
  br i1 %exitcond, label %23, label %15

23:
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %21)
  store i32 262148, ptr %4
  %24 = getelementptr inbounds nuw i8, ptr %4, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), ptr %24, !tbaa !2
  %25 = addrspacecast ptr %4 to ptr addrspace(1)
  call void @llvm.lifetime.start.p0(i64 16, ptr nonnull %2)
  call cc 1005 addrspace(1) void @N$PBEG()
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 8, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %25)
  %26 = call cc 1005 addrspace(1) ptr @N$PEND()
  store i32 524288, ptr %2
  %27 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store i16 8, ptr %27, !tbaa !2
  %28 = getelementptr inbounds nuw i8, ptr %2, i16 6
  store ptr addrspace(1) %13, ptr %28, !tbaa !2
  %29 = getelementptr inbounds nuw i8, ptr %2, i16 10
  store i16 3, ptr %29, !tbaa !2
  %30 = getelementptr inbounds nuw i8, ptr %2, i16 12
  store i32 0, ptr %30
  %31 = addrspacecast ptr %2 to ptr addrspace(5)
  %32 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(5) %31)
  %33 = trunc i32 %32 to i8
  %34 = icmp eq i8 %33, 0
  br i1 %34, label %35, label %43

35:
  br label %36

36:
  %37 = phi i32 [ %41, %36 ], [ %32, %35 ]
  %38 = phi ptr [ %40, %36 ], [ %26, %35 ]
  %extract.shift12 = lshr i32 %37, 16
  %extract.trunc22 = trunc nuw i32 %extract.shift12 to i16
  call cc 1005 addrspace(1) void @N$PBEG()
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc22)
  %39 = call cc 1005 addrspace(1) ptr @N$PEND()
  %40 = call cc 1005 addrspace(1) ptr @N$TAPP(ptr %38, ptr %39)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %39)
  %41 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(5) %31)
  %extract.trunc4 = trunc i32 %41 to i8
  %42 = icmp eq i8 %extract.trunc4, 0
  br i1 %42, label %36, label %43

43:
  %44 = phi ptr [ %26, %23 ], [ %40, %36 ]
  call cc 1005 addrspace(1) void @N$PS(ptr %44)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %44)
  call void @llvm.lifetime.end.p0(i64 16, ptr nonnull %2)
  %45 = getelementptr inbounds nuw i8, ptr %3, i16 2
  %46 = addrspacecast ptr %45 to ptr addrspace(5)
  store i16 8, ptr addrspace(5) %46
  %47 = getelementptr inbounds nuw i8, ptr addrspace(5) %46, i16 2
  store i16 8, ptr addrspace(5) %47
  %48 = getelementptr inbounds nuw i8, ptr addrspace(5) %46, i16 4
  store ptr addrspace(1) %13, ptr addrspace(5) %48
  store i16 0, ptr %3, !tbaa !2
  %49 = getelementptr inbounds nuw i8, ptr %3, i16 10
  store i16 2, ptr %49, !tbaa !2
  %50 = getelementptr inbounds nuw i8, ptr %3, i16 12
  store i32 0, ptr %50
  %51 = addrspacecast ptr %3 to ptr addrspace(5)
  %52 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(5) %51)
  %extract.trunc = trunc i32 %52 to i8
  %53 = icmp eq i8 %extract.trunc, 0
  br i1 %53, label %b4, label %b3

54:
  call cc 1005 addrspace(1) void @N$PN()
  %55 = load i16, ptr %3, !tbaa !2
  %56 = getelementptr i8, ptr %3, i16 2
  %57 = load i16, ptr %56, !tbaa !2
  %58 = getelementptr inbounds nuw i8, ptr %3, i16 4
  %59 = load i16, ptr %58, !tbaa !2
  %60 = getelementptr inbounds nuw i8, ptr %3, i16 6
  %61 = load ptr addrspace(1), ptr %60, !tbaa !2
  %62 = getelementptr i8, ptr %3, i16 10
  %63 = load i16, ptr %62, !tbaa !2
  %64 = getelementptr i8, ptr %3, i16 12
  %65 = load i16, ptr %64, !tbaa !2
  %66 = getelementptr i8, ptr %3, i16 14
  %67 = load i16, ptr %66, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 16, ptr nonnull %1)
  store i16 %55, ptr %1, !tbaa !2
  %68 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store i16 %57, ptr %68, !tbaa !2
  %69 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store i16 %59, ptr %69, !tbaa !2
  %70 = getelementptr inbounds nuw i8, ptr %1, i16 6
  store ptr addrspace(1) %61, ptr %70, !tbaa !2
  %71 = getelementptr inbounds nuw i8, ptr %1, i16 10
  store i16 %63, ptr %71, !tbaa !2
  %72 = getelementptr inbounds nuw i8, ptr %1, i16 12
  store i16 %65, ptr %72, !tbaa !2
  %73 = getelementptr inbounds nuw i8, ptr %1, i16 14
  store i16 %67, ptr %73, !tbaa !2
  %74 = addrspacecast ptr %1 to ptr addrspace(5)
  %75 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(5) %74)
  %76 = trunc i32 %75 to i8
  %77 = icmp eq i8 %76, 0
  br i1 %77, label %78, label %85

78:
  br label %79

79:
  %80 = phi i32 [ %83, %79 ], [ %75, %78 ]
  %81 = phi i16 [ %82, %79 ], [ -32768, %78 ]
  %extract.shift11 = lshr i32 %80, 16
  %extract.trunc21 = trunc nuw i32 %extract.shift11 to i16
  %82 = call i16 @llvm.smax.i16(i16 %81, i16 %extract.trunc21)
  %83 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(5) %74)
  %extract.trunc3 = trunc i32 %83 to i8
  %84 = icmp eq i8 %extract.trunc3, 0
  br i1 %84, label %79, label %85

85:
  %86 = phi i16 [ -32768, %54 ], [ %82, %79 ]
  call void @llvm.lifetime.end.p0(i64 16, ptr nonnull %1)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %86)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b3:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  br label %54

b4:
  %extract.shift1 = lshr i32 %52, 16
  %extract.trunc2 = trunc nuw i32 %extract.shift1 to i16
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %extract.trunc2)
  br label %54
}

define internal cc 1005 range(i32 0, -65535) i32 @$state1.next(ptr addrspace(5) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b2:
  %1 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 2
  %2 = load i16, ptr addrspace(5) %0
  %3 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 12
  %4 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 10
  %5 = load i16, ptr addrspace(5) %4
  %6 = load i16, ptr addrspace(5) %1
  %7 = load i16, ptr addrspace(5) %3, align 2
  %8 = icmp ugt i16 %2, 5
  br i1 %8, label %b23, label %9

9:
  switch table i16 %2, label %10 [
    i16 0, label %10
    i16 1, label %11
    i16 2, label %12
    i16 3, label %13
    i16 4, label %11
    i16 5, label %15
  ]

10:
  br label %17

11:
  br label %b23

12:
  br label %b9

13:
  %14 = add i16 %7, %5
  br label %b14

15:
  %16 = add i16 %7, 1
  br label %17

17:
  %sink = phi i16 [ %16, %15 ], [ 0, %10 ]
  %18 = getelementptr i8, ptr addrspace(5) %0, i16 12
  store i16 %sink, ptr addrspace(5) %18
  br label %b9

b9:
  %19 = phi i16 [ %7, %12 ], [ %sink, %17 ]
  %20 = add i16 %19, %5
  %21 = icmp ugt i16 %20, %6
  br i1 %21, label %b23, label %b14

b14:
  %22 = phi i16 [ %14, %13 ], [ %20, %b9 ]
  %23 = phi i16 [ %7, %13 ], [ %19, %b9 ]
  %24 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 14
  store i16 0, ptr addrspace(5) %24
  %25 = icmp ugt i16 %22, %23
  br i1 %25, label %26, label %b18

26:
  %27 = getelementptr inbounds nuw i8, ptr addrspace(5) %0, i16 6
  %28 = load ptr addrspace(1), ptr addrspace(5) %27
  %29 = tail call i16 @llvm.umax.i16(i16 %23, i16 %6)
  %30 = sub nuw i16 %29, %23
  %31 = add i16 %5, -1
  %32 = tail call i16 @llvm.umin.i16(i16 %30, i16 %31)
  %33 = icmp eq i16 %30, %32
  br i1 %33, label %b20, label %34

34:
  %35 = sub i16 %22, %23
  %36 = shl i16 %23, 1
  %37 = getelementptr i8, ptr addrspace(1) %28, i16 %36
  br label %b16

b16:
  %38 = phi ptr addrspace(1) [ %37, %34 ], [ %45, %b16 ]
  %39 = phi i16 [ %35, %34 ], [ %44, %b16 ]
  %40 = phi i16 [ 0, %34 ], [ %42, %b16 ]
  %41 = load i16, ptr addrspace(1) %38
  %42 = add i16 %41, %40
  %43 = getelementptr i8, ptr addrspace(5) %0, i16 14
  store i16 %42, ptr addrspace(5) %43
  %44 = add i16 %39, -1
  %45 = getelementptr i8, ptr addrspace(1) %38, i16 2
  %exitcond = icmp eq i16 %44, 0
  br i1 %exitcond, label %b18, label %b16

common.ret:
  %common.op = phi i32 [ %insert.shift1, %b18 ], [ 1, %b23 ]
  ret i32 %common.op

b18:
  %46 = phi i16 [ 0, %b14 ], [ %42, %b16 ]
  store i16 5, ptr addrspace(5) %0
  %47 = sdiv i16 %46, %5
  %48 = srem i16 %46, %5
  %49 = icmp eq i16 %48, 0
  %50 = xor i16 %48, %5
  %51 = ashr i16 %50, 15
  %52 = select i1 %49, i16 0, i16 %51
  %53 = add i16 %52, %47
  %insert.ext3 = zext i16 %53 to i32
  %insert.shift1 = shl nuw i32 %insert.ext3, 16
  br label %common.ret

b20:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b23:
  store i16 1, ptr addrspace(5) %0
  br label %common.ret
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$TAPP(ptr, ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
