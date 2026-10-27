target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00\0D\0A\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00closed \00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00 after \00"
@$str5 = internal constant [13 x i8] c"\08\00\06\00\06\00 lines\00"
@$str6 = internal constant [14 x i8] c"\08\00\07\00\07\00LOG.TXT\00"
@$str7 = internal constant [14 x i8] c"\08\00\07\00\07\00started\00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00SCRATCH.TXT\00"
@$str9 = internal constant [16 x i8] c"\08\00\09\00\09\00temporary\00"
@$str10 = internal constant [24 x i8] c"\08\00\11\00\11\00moved, still open\00"
@$str11 = internal constant [11 x i8] c"\08\00\04\00\04\00done\00"

define internal cc 1005 void @Log.create(ptr addrspace(5) nocapture writeonly %0, ptr %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %3 = addrspacecast ptr addrspace(5) %0 to ptr addrspace(1)
  %4 = alloca [80 x i8]
  %5 = getelementptr i8, ptr %1, i16 -4
  %6 = load i16, ptr %5
  call void @llvm.lifetime.start.p0(i64 80, ptr nonnull %4)
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %4, i8 0, i16 80, i1 false)
  %7 = addrspacecast ptr %4 to ptr addrspace(5)
  %8 = addrspacecast ptr %4 to ptr addrspace(1)
  %9 = icmp eq i16 %6, 0
  br i1 %9, label %27, label %10

10:
  br label %11

11:
  %12 = phi i16 [ %6, %10 ], [ %21, %18 ]
  %13 = phi ptr [ %1, %10 ], [ %20, %18 ]
  %14 = phi i16 [ %19, %18 ], [ 0, %10 ]
  %15 = icmp ult i16 %14, 79
  br i1 %15, label %22, label %18

16:
  %17 = icmp ult i16 %19, 80
  br i1 %17, label %27, label %26

18:
  %19 = phi i16 [ %25, %22 ], [ %14, %11 ]
  %20 = getelementptr i8, ptr %13, i16 1
  %21 = add i16 %12, -1
  %exitcond = icmp eq i16 %21, 0
  br i1 %exitcond, label %16, label %11

22:
  %23 = getelementptr inbounds nuw i8, ptr addrspace(5) %7, i16 %14
  %24 = load i8, ptr %13
  store i8 %24, ptr addrspace(5) %23
  %25 = add nuw nsw i16 %14, 1
  br label %18

26:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

27:
  %28 = phi i16 [ %19, %16 ], [ 0, %2 ]
  %29 = getelementptr inbounds nuw i8, ptr addrspace(5) %7, i16 %28
  store i8 0, ptr addrspace(5) %29
  %30 = call addrspace(1) i16 @LL$CREATE(ptr addrspace(1) %8)
  %31 = icmp slt i16 %30, 0
  br i1 %31, label %32, label %b3

32:
  %33 = sub i16 %30, -3
  %34 = icmp ule i16 %33, 1
  br i1 %34, label %b2, label %35

35:
  %36 = icmp eq i16 %30, -5
  br i1 %36, label %37, label %38

37:
  br label %b2

38:
  %39 = sub i16 0, %30
  %insert.ext3 = zext i16 %39 to i32
  %insert.shift = shl nuw i32 %insert.ext3, 16
  %40 = or disjoint i32 %insert.shift, 2
  br label %b2

common.ret:
  %sink = phi ptr [ null, %b3 ], [ %1, %b2 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr %sink)
  ret void

b2:
  %common.op = phi i32 [ 1, %37 ], [ %40, %38 ], [ 0, %32 ]
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %4)
  store i8 1, ptr addrspace(5) %0
  %41 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i32 %common.op, ptr addrspace(5) %41, align 1
  br label %common.ret

b3:
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %4)
  store i8 0, ptr addrspace(5) %0
  %42 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr %1, ptr addrspace(5) %42
  %43 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store i16 %30, ptr addrspace(5) %43
  %44 = getelementptr i8, ptr addrspace(1) %3, i16 6
  call void @llvm.memset.p1.i16(ptr addrspace(1) align 2 %44, i8 0, i16 134, i1 false)
  br label %common.ret
}

define cc 1005 range(i16 0, 2) i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [140 x i8], align 1
  %2 = alloca [140 x i8]
  call void @llvm.lifetime.start.p0(i64 140, ptr nonnull %2)
  call void @llvm.lifetime.start.p0(i64 140, ptr nonnull %1)
  %3 = addrspacecast ptr %1 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @Log.create(ptr addrspace(5) %3, ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  %4 = load i8, ptr %1, !range !13
  %5 = icmp eq i8 %4, 1
  br i1 %5, label %36, label %6

6:
  %7 = getelementptr inbounds nuw i8, ptr %1, i16 2
  %8 = load ptr, ptr %7
  %9 = getelementptr inbounds nuw i8, ptr %1, i16 4
  %10 = load i16, ptr %9
  %11 = getelementptr inbounds nuw i8, ptr %1, i16 138
  %12 = load i16, ptr %11
  %13 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 %10, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6) to ptr addrspace(1)), i16 7)
  %14 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 %10, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), i16 2)
  %15 = addrspacecast ptr %2 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @Log.create(ptr addrspace(5) %15, ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  %16 = load i8, ptr %2, !tbaa !2, !range !13
  %17 = icmp eq i8 %16, 1
  br i1 %17, label %18, label %20

18:
  %19 = add i16 %12, 1
  br label %34

20:
  %21 = getelementptr inbounds nuw i8, ptr %2, i16 2
  %22 = load ptr, ptr %21, !tbaa !2
  %23 = getelementptr inbounds nuw i8, ptr %2, i16 4
  %24 = load i16, ptr %23, !tbaa !2
  %25 = getelementptr inbounds nuw i8, ptr %2, i16 138
  %26 = load i16, ptr %25, !tbaa !2
  %27 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 %24, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6) to ptr addrspace(1)), i16 9)
  %28 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 %24, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), i16 2)
  %29 = add i16 %26, 1
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %22)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %29)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %30 = tail call addrspace(1) i16 @LL$CLOSE(i16 %24)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %22)
  %31 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 %10, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6) to ptr addrspace(1)), i16 17)
  %32 = tail call addrspace(1) i32 @LL$WRITE_FILE(i16 %10, ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), i16 2)
  %33 = add i16 %12, 2
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %34

34:
  %sink = phi i16 [ %19, %18 ], [ %33, %20 ]
  %.ph = phi i16 [ 1, %18 ], [ 0, %20 ]
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %8)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %sink)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %35 = tail call addrspace(1) i16 @LL$CLOSE(i16 %10)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %8)
  br label %36

36:
  %37 = phi i16 [ 1, %0 ], [ %.ph, %34 ]
  call void @llvm.lifetime.end.p0(i64 140, ptr nonnull %1)
  call void @llvm.lifetime.end.p0(i64 140, ptr nonnull %2)
  ret i16 %37
}

declare i32 @LL$WRITE_FILE(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare i16 @LL$OPEN(ptr addrspace(1), i8) addrspace(1)

declare i16 @LL$CREATE(ptr addrspace(1)) addrspace(1)

declare i32 @LL$READ(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare i16 @LL$CLOSE(i16) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) noalias nocapture writeonly, ptr addrspace(1) noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
!11 = !{i8 0, i8 2}
!12 = !{i8 0, i8 2}
!13 = !{i8 0, i8 2}
!14 = !{i8 0, i8 2}
