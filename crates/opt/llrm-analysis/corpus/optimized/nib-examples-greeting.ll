target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00hello\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00, world\00"
@$str4 = internal constant [8 x i8] c"\08\00\01\00\01\00!\00"
@$str5 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00] has \00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00 chars\00"
@$str9 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str10 = internal constant [22 x i8] c"\08\00\0F\00\0F\00ada sorts first\00"
@$str11 = internal constant [12 x i8] c"\08\00\05\00\05\00reset\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = tail call cc 1005 addrspace(1) ptr @N$TCAT(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  %4 = tail call cc 1005 addrspace(1) ptr @N$TAPP(ptr %3, ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %4)
  tail call cc 1005 addrspace(1) void @N$PN()
  %5 = tail call cc 1005 addrspace(1) ptr @N$BCLN(ptr %4, i16 1)
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %33, label %9

9:
  br label %10

10:
  %11 = phi ptr [ %24, %23 ], [ %5, %9 ]
  %12 = phi i16 [ %25, %23 ], [ 0, %9 ]
  %13 = getelementptr i8, ptr %11, i16 %12
  %14 = load i8, ptr %13
  %15 = icmp ugt i8 %14, 96
  %16 = icmp ult i8 %14, 123
  %17 = and i1 %15, %16
  br i1 %17, label %18, label %23

18:
  %19 = tail call cc 1005 addrspace(1) ptr @N$BRES(ptr nonnull %11, i16 0, i16 1)
  %20 = getelementptr i8, ptr %19, i16 -4
  %21 = load i16, ptr %20
  %22 = icmp ult i16 %12, %21
  br i1 %22, label %29, label %32

23:
  %24 = phi ptr [ %19, %29 ], [ %11, %10 ]
  %25 = add nuw i16 %12, 1
  %26 = getelementptr i8, ptr %24, i16 -4
  %27 = load i16, ptr %26
  %28 = icmp ult i16 %25, %27
  br i1 %28, label %10, label %33

29:
  %30 = getelementptr i8, ptr %19, i16 %12
  %31 = add nsw i8 %14, -32
  store i8 %31, ptr %30
  br label %23

32:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

33:
  %34 = phi ptr [ %5, %0 ], [ %24, %23 ]
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  tail call cc 1005 addrspace(1) void @N$PS(ptr nonnull %34)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PBEG()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 42)
  %35 = tail call cc 1005 addrspace(1) ptr @N$PEND()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %35)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  %36 = getelementptr i8, ptr %35, i16 -4
  %37 = load i16, ptr %36
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %37)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PBEG()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 1)
  %38 = tail call cc 1005 addrspace(1) ptr @N$PEND()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  %39 = load i16, ptr %36
  %40 = addrspacecast ptr %35 to ptr addrspace(1)
  store i16 %39, ptr %2, !tbaa !2
  %41 = getelementptr inbounds nuw i8, ptr %2, i16 2
  store i16 %39, ptr %41, !tbaa !2
  %42 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %2 to ptr addrspace(1)
  %44 = getelementptr i8, ptr %38, i16 -4
  %45 = load i16, ptr %44
  %46 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %45, ptr %1, !tbaa !2
  %47 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store i16 %45, ptr %47, !tbaa !2
  %48 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr addrspace(1) %46, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %1 to ptr addrspace(1)
  %50 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %43, ptr addrspace(1) %49)
  %51 = icmp slt i8 %50, 0
  call cc 1005 addrspace(1) void @N$BDRP(ptr %38)
  br i1 %51, label %b2, label %b4

b2:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %4)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %35)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %34)
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  ret i16 0
}

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BRES(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 ptr @N$TCAT(ptr, ptr) addrspace(1)

declare cc 1005 ptr @N$TAPP(ptr, ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
