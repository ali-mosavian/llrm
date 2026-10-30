target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00north\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00east\00"
@$str3 = internal constant [16 x i8] c"\08\00\09\00\09\00southwest\00"
@$str4 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = addrspacecast ptr %1 to ptr addrspace(5)
  %3 = addrspacecast ptr %1 to ptr addrspace(1)
  %4 = getelementptr inbounds nuw i8, ptr addrspace(5) %2, i16 4
  store i32 589833, ptr addrspace(5) %2
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6) to ptr addrspace(1)), ptr addrspace(5) %4
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %3)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$PI2(i16 3)
  call cc 1005 addrspace(1) void @N$PN()
  %5 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), i16 3, i16 2)
  store i16 1, ptr %5
  %6 = getelementptr inbounds nuw i8, ptr %5, i16 2
  store i16 2, ptr %6
  %7 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store i16 3, ptr %7
  %8 = getelementptr i8, ptr %5, i16 -4
  %9 = load i16, ptr %8
  %10 = icmp eq i16 %9, 0
  br i1 %10, label %20, label %11

11:
  br label %12

12:
  %13 = phi ptr [ %5, %11 ], [ %19, %12 ]
  %14 = phi i16 [ %9, %11 ], [ %18, %12 ]
  %15 = phi i16 [ %17, %12 ], [ 0, %11 ]
  %16 = load i16, ptr %13
  %17 = add i16 %16, %15
  %18 = add i16 %14, -1
  %19 = getelementptr i8, ptr %13, i16 2
  %exitcond = icmp eq i16 %18, 0
  br i1 %exitcond, label %20, label %12

20:
  %21 = phi i16 [ 0, %0 ], [ %17, %12 ]
  call cc 1005 addrspace(1) void @N$PI2(i16 %21)
  call cc 1005 addrspace(1) void @N$PN()
  %22 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), i16 4, i16 2)
  store i16 1, ptr %22
  %23 = getelementptr inbounds nuw i8, ptr %22, i16 2
  store i16 2, ptr %23
  %24 = getelementptr inbounds nuw i8, ptr %22, i16 4
  store i16 3, ptr %24
  %25 = getelementptr inbounds nuw i8, ptr %22, i16 6
  store i16 4, ptr %25
  %26 = getelementptr i8, ptr %22, i16 -4
  %27 = load i16, ptr %26
  %28 = icmp eq i16 %27, 0
  br i1 %28, label %b6, label %29

29:
  br label %b4

b4:
  %30 = phi ptr [ %22, %29 ], [ %39, %b4 ]
  %31 = phi i16 [ %27, %29 ], [ %38, %b4 ]
  %32 = phi i16 [ %37, %b4 ], [ 0, %29 ]
  %33 = load i16, ptr %30
  %34 = srem i16 %33, 2
  %35 = icmp eq i16 %34, 0
  %36 = select i1 %35, i16 %33, i16 0
  %37 = add i16 %36, %32
  %38 = add i16 %31, -1
  %39 = getelementptr i8, ptr %30, i16 2
  %exitcond1 = icmp eq i16 %38, 0
  br i1 %exitcond1, label %b6, label %b4

b6:
  %40 = phi i16 [ 0, %20 ], [ %37, %b4 ]
  %41 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %22, i16 1, i16 2)
  %42 = shl i16 %27, 1
  %43 = getelementptr inbounds i8, ptr %41, i16 %42
  store i16 %40, ptr %43
  %44 = getelementptr i8, ptr %41, i16 -4
  %45 = load i16, ptr %44
  %46 = icmp ugt i16 %45, 4
  br i1 %46, label %b11, label %b12

b11:
  %47 = getelementptr inbounds nuw i8, ptr %41, i16 8
  %48 = load i16, ptr %47
  call cc 1005 addrspace(1) void @N$PI2(i16 %48)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %41)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %5)
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  ret i16 0

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
