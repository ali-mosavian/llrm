target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00 is busy\00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00open \00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00close \00"
@$str4 = internal constant [12 x i8] c"\08\00\05\00\05\00link \00"
@$str5 = internal constant [8 x i8] c"\08\00\01\00\01\00-\00"
@$str6 = internal constant [12 x i8] c"\08\00\05\00\05\00 down\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00log\00"
@$str8 = internal constant [11 x i8] c"\08\00\04\00\04\00stop\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00 saw \00"
@$str10 = internal constant [15 x i8] c"\08\00\08\00\08\00visited \00"
@$str11 = internal constant [19 x i8] c"\08\00\0C\00\0C\00visit failed\00"
@$str12 = internal constant [8 x i8] c"\08\00\01\00\01\00a\00"
@$str13 = internal constant [8 x i8] c"\08\00\01\00\01\00b\00"
@$str14 = internal constant [11 x i8] c"\08\00\04\00\04\00 to \00"
@$str15 = internal constant [14 x i8] c"\08\00\07\00\07\00no link\00"
@$str16 = internal constant [9 x i8] c"\08\00\02\00\02\00--\00"
@$str17 = internal constant [8 x i8] c"\08\00\01\00\01\00c\00"
@$str18 = internal constant [8 x i8] c"\08\00\01\00\01\00d\00"
@$str19 = internal constant [13 x i8] c"\08\00\06\00\06\00linked\00"
@$str20 = internal constant [8 x i8] c"\08\00\01\00\01\00e\00"
@$str21 = internal constant [12 x i8] c"\08\00\05\00\05\00kept \00"
@$str22 = internal constant [13 x i8] c"\08\00\06\00\06\00closed\00"
@$str23 = internal constant [8 x i8] c"\08\00\01\00\01\00f\00"
@$str24 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str25 = internal constant [8 x i8] c"\08\00\01\00\01\00p\00"
@$str26 = internal constant [8 x i8] c"\08\00\01\00\01\00q\00"
@$str27 = internal constant [8 x i8] c"\08\00\01\00\01\00r\00"
@$str28 = internal constant [8 x i8] c"\08\00\01\00\01\00z\00"

define internal cc 1006 void @Channel.open(ptr addrspace(5) nocapture writeonly initializes((0, 4)) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i8 range(i8 0, 2) %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = icmp eq i8 %2, 0
  br i1 %3, label %b2, label %b4

common.ret:
  ret void

b2:
  tail call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %4 = getelementptr i8, ptr addrspace(5) %0, i16 3
  store i8 0, ptr addrspace(5) %4
  store i16 1, ptr addrspace(5) %0
  %5 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i8 0, ptr addrspace(5) %5, align 2
  br label %common.ret

b4:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %1)
  tail call cc 1005 addrspace(1) void @N$PN()
  %6 = tail call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store i16 0, ptr addrspace(5) %0
  %7 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr %6, ptr addrspace(5) %7
  br label %common.ret
}

define internal cc 1006 void @connect(ptr addrspace(5) nocapture writeonly initializes((0, 1), (2, 3)) %0, ptr addrspace(5) %1, ptr addrspace(5) nocapture readonly %2, i8 range(i8 0, 2) %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b3:
  %4 = addrspacecast ptr addrspace(5) %1 to ptr addrspace(1)
  %5 = addrspacecast ptr addrspace(5) %2 to ptr addrspace(1)
  %6 = alloca [4 x i8]
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  tail call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %4)
  tail call cc 1005 addrspace(1) void @N$PN()
  %7 = tail call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %4)
  %8 = addrspacecast ptr %6 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @Channel.open(ptr addrspace(5) %8, ptr addrspace(1) %5, i8 %3)
  %9 = load i8, ptr %6, !tbaa !2, !range !17
  %10 = icmp eq i8 %9, 1
  br i1 %10, label %11, label %b5

common.ret:
  ret void

11:
  %12 = getelementptr i8, ptr %6, i16 2
  %13 = load i8, ptr %12, !tbaa !2
  store i8 1, ptr addrspace(5) %0
  %14 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i8 %13, ptr addrspace(5) %14
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %7)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %7)
  br label %common.ret

b5:
  %15 = getelementptr i8, ptr %6, i16 2
  %16 = load ptr, ptr %15, !tbaa !2
  store i8 0, ptr addrspace(5) %0
  %17 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr %7, ptr addrspace(5) %17
  %18 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store ptr %16, ptr addrspace(5) %18
  br label %common.ret
}

define internal cc 1005 range(i32 0, -65535) i32 @visit(ptr addrspace(5) nocapture readonly %0, ptr addrspace(5) readonly %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b3:
  %2 = addrspacecast ptr addrspace(5) %1 to ptr addrspace(1)
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [8 x i8]
  store i32 196611, ptr %8
  %9 = getelementptr inbounds nuw i8, ptr %8, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6) to ptr addrspace(1)), ptr %9, !tbaa !2
  %10 = addrspacecast ptr %8 to ptr addrspace(1)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %10)
  call cc 1005 addrspace(1) void @N$PN()
  %11 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %10)
  %12 = load ptr, ptr addrspace(5) %0
  %13 = getelementptr i8, ptr %12, i16 -4
  %14 = load i16, ptr %13
  %15 = icmp eq i16 %14, 0
  br i1 %15, label %32, label %16

16:
  %17 = addrspacecast ptr %7 to ptr addrspace(5)
  %18 = addrspacecast ptr %6 to ptr addrspace(1)
  %19 = addrspacecast ptr %5 to ptr addrspace(1)
  %20 = addrspacecast ptr %4 to ptr addrspace(1)
  %21 = addrspacecast ptr %3 to ptr addrspace(1)
  br label %b5

common.ret:
  %common.op = phi i32 [ %insert.shift, %32 ], [ %48, %44 ]
  ret i32 %common.op

b5:
  %22 = phi ptr [ %31, %b6 ], [ %12, %16 ]
  %23 = phi i16 [ %30, %b6 ], [ %14, %16 ]
  %24 = phi i16 [ 0, %16 ], [ %29, %b6 ]
  %25 = load ptr, ptr %22
  %26 = getelementptr i8, ptr %25, i16 -4
  %27 = load i16, ptr %26
  %28 = icmp eq i16 %27, 0
  br i1 %28, label %b6, label %b10

b6:
  %29 = phi i16 [ %24, %b5 ], [ %61, %60 ]
  %30 = add i16 %23, -1
  %31 = getelementptr i8, ptr %22, i16 2
  %exitcond = icmp eq i16 %30, 0
  br i1 %exitcond, label %32, label %b5

32:
  %33 = phi i16 [ %24, %59 ], [ 0, %b3 ], [ %29, %b6 ]
  call cc 1005 addrspace(1) void @N$PS(ptr %11)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %33)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr %11)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %11)
  %insert.ext3 = zext i16 %33 to i32
  %insert.shift = shl nuw i32 %insert.ext3, 16
  br label %common.ret

b10:
  %34 = addrspacecast ptr %25 to ptr addrspace(1)
  store i16 %27, ptr %6, !tbaa !2
  %35 = getelementptr i8, ptr %6, i16 2
  store i16 %27, ptr %35, !tbaa !2
  %36 = getelementptr i8, ptr %6, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  store i16 %27, ptr %5, !tbaa !2
  %37 = getelementptr i8, ptr %5, i16 2
  store i16 %27, ptr %37, !tbaa !2
  %38 = getelementptr i8, ptr %5, i16 4
  store ptr addrspace(1) %34, ptr %38, !tbaa !2
  %39 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %19, ptr addrspace(1) %2)
  %40 = icmp ne i8 %39, 0
  %41 = zext i1 %40 to i8
  call cc 1006 addrspace(1) void @Channel.open(ptr addrspace(5) %17, ptr addrspace(1) %18, i8 %41)
  %42 = load i8, ptr %7, !tbaa !2, !range !18
  %43 = icmp eq i8 %42, 1
  br i1 %43, label %44, label %b12

44:
  %45 = getelementptr i8, ptr %7, i16 2
  %46 = load i8, ptr %45, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr %11)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %11)
  %47 = zext i8 %46 to i32
  %insert.shift1 = shl nuw nsw i32 %47, 16
  %48 = or disjoint i32 %insert.shift1, 1
  br label %common.ret

b12:
  %49 = getelementptr i8, ptr %7, i16 2
  %50 = load ptr, ptr %49, !tbaa !2
  %51 = getelementptr i8, ptr %50, i16 -4
  %52 = load i16, ptr %51
  %53 = addrspacecast ptr %50 to ptr addrspace(1)
  store i16 %52, ptr %4, !tbaa !2
  %54 = getelementptr i8, ptr %4, i16 2
  store i16 %52, ptr %54, !tbaa !2
  %55 = getelementptr i8, ptr %4, i16 4
  store ptr addrspace(1) %53, ptr %55, !tbaa !2
  store i32 262148, ptr %3
  %56 = getelementptr i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6) to ptr addrspace(1)), ptr %56, !tbaa !2
  %57 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %20, ptr addrspace(1) %21)
  %58 = icmp eq i8 %57, 0
  br i1 %58, label %59, label %60

59:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr nonnull %50)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %50)
  br label %32

60:
  %61 = add i16 %24, 1
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr nonnull %50)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %50)
  br label %b6
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca ptr
  %2 = alloca [8 x i8]
  %3 = alloca ptr
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [6 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [6 x i8]
  %10 = addrspacecast ptr %9 to ptr addrspace(5)
  store i32 65537, ptr %8
  %11 = getelementptr inbounds nuw i8, ptr %8, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6) to ptr addrspace(1)), ptr %11, !tbaa !2
  %12 = addrspacecast ptr %8 to ptr addrspace(5)
  store i32 65537, ptr %7
  %13 = getelementptr inbounds nuw i8, ptr %7, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6) to ptr addrspace(1)), ptr %13, !tbaa !2
  %14 = addrspacecast ptr %7 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @connect(ptr addrspace(5) %10, ptr addrspace(5) %12, ptr addrspace(5) %14, i8 1)
  %15 = load i8, ptr %9, !tbaa !2, !range !20
  %16 = icmp eq i8 %15, 0
  br i1 %16, label %24, label %b3

b2:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %17 = addrspacecast ptr %6 to ptr addrspace(5)
  store i32 65537, ptr %5
  %18 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str17, i16 6) to ptr addrspace(1)), ptr %18, !tbaa !2
  %19 = addrspacecast ptr %5 to ptr addrspace(5)
  store i32 65537, ptr %4
  %20 = getelementptr inbounds nuw i8, ptr %4, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), ptr %20, !tbaa !2
  %21 = addrspacecast ptr %4 to ptr addrspace(5)
  call cc 1006 addrspace(1) void @connect(ptr addrspace(5) %17, ptr addrspace(5) %19, ptr addrspace(5) %21, i8 0)
  %22 = load i8, ptr %6, !tbaa !2, !range !20
  %23 = icmp eq i8 %22, 0
  br i1 %23, label %29, label %35

b3:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %b2

24:
  %25 = getelementptr inbounds nuw i8, ptr %9, i16 2
  %26 = load ptr, ptr %25, !tbaa !2
  %27 = getelementptr inbounds nuw i8, ptr %9, i16 4
  %28 = load ptr, ptr %27, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PS(ptr %26)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %28)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %26)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %28)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %28)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %28)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %26)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %26)
  br label %b2

29:
  %30 = getelementptr inbounds nuw i8, ptr %6, i16 2
  %31 = addrspacecast ptr %30 to ptr addrspace(5)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  %32 = load ptr, ptr addrspace(5) %31
  tail call cc 1005 addrspace(1) void @N$PS(ptr %32)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  %33 = getelementptr inbounds nuw i8, ptr addrspace(5) %31, i16 2
  %34 = load ptr, ptr addrspace(5) %33
  tail call cc 1005 addrspace(1) void @N$PS(ptr %34)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %34)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %34)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr %32)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %32)
  br label %35

35:
  %sink = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str19, i16 6), %29 ], [ getelementptr inbounds nuw (i8, ptr @$str15, i16 6), %b2 ]
  tail call cc 1005 addrspace(1) void @N$PS(ptr nonnull %sink)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str21, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str20, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str20, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str20, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str23, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str23, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str22, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %36 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str24, i16 6), i16 5, i16 2)
  store ptr getelementptr inbounds nuw (i8, ptr @$str25, i16 6), ptr %36
  %37 = getelementptr inbounds nuw i8, ptr %36, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str24, i16 6), ptr %37
  %38 = getelementptr inbounds nuw i8, ptr %36, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str26, i16 6), ptr %38
  %39 = getelementptr inbounds nuw i8, ptr %36, i16 6
  store ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6), ptr %39
  %40 = getelementptr inbounds nuw i8, ptr %36, i16 8
  store ptr getelementptr inbounds nuw (i8, ptr @$str27, i16 6), ptr %40
  store ptr %36, ptr %3, !tbaa !2
  %41 = addrspacecast ptr %3 to ptr addrspace(5)
  store i32 65537, ptr %2
  %42 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str28, i16 6) to ptr addrspace(1)), ptr %42, !tbaa !2
  %43 = addrspacecast ptr %2 to ptr addrspace(5)
  %44 = call cc 1005 addrspace(1) i32 @visit(ptr addrspace(5) %41, ptr addrspace(5) %43)
  %extract.trunc12 = trunc i32 %44 to i8
  %45 = icmp eq i8 %extract.trunc12, 0
  br i1 %45, label %47, label %46

46:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  br label %48

47:
  %extract.shift7 = lshr i32 %44, 16
  %extract.trunc14 = trunc nuw i32 %extract.shift7 to i16
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %extract.trunc14)
  br label %48

48:
  tail call cc 1005 addrspace(1) void @N$PN()
  %49 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str24, i16 6), i16 3, i16 2)
  store ptr getelementptr inbounds nuw (i8, ptr @$str25, i16 6), ptr %49
  %50 = getelementptr inbounds nuw i8, ptr %49, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str26, i16 6), ptr %50
  %51 = getelementptr inbounds nuw i8, ptr %49, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str27, i16 6), ptr %51
  store ptr %49, ptr %1, !tbaa !2
  %52 = addrspacecast ptr %1 to ptr addrspace(5)
  store i32 65537, ptr %0
  %53 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str26, i16 6) to ptr addrspace(1)), ptr %53, !tbaa !2
  %54 = addrspacecast ptr %0 to ptr addrspace(5)
  %55 = call cc 1005 addrspace(1) i32 @visit(ptr addrspace(5) %52, ptr addrspace(5) %54)
  %extract.trunc15 = trunc i32 %55 to i8
  %56 = icmp eq i8 %extract.trunc15, 0
  br i1 %56, label %58, label %57

57:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  br label %b33

58:
  %extract.shift9 = lshr i32 %55, 16
  %extract.trunc17 = trunc nuw i32 %extract.shift9 to i16
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %extract.trunc17)
  br label %b33

b33:
  tail call cc 1005 addrspace(1) void @N$PN()
  %59 = getelementptr i8, ptr %49, i16 -4
  %60 = load i16, ptr %59
  %61 = icmp eq i16 %60, 0
  br i1 %61, label %b38, label %62

62:
  br label %b36

b36:
  %63 = phi ptr [ %49, %62 ], [ %67, %b36 ]
  %64 = phi i16 [ %60, %62 ], [ %66, %b36 ]
  %65 = load ptr, ptr %63
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %65)
  %66 = add i16 %64, -1
  %67 = getelementptr i8, ptr %63, i16 2
  %exitcond1 = icmp eq i16 %66, 0
  br i1 %exitcond1, label %b38, label %b36

b37:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %36)
  ret i16 0

b38:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %49)
  %68 = getelementptr i8, ptr %36, i16 -4
  %69 = load i16, ptr %68
  %70 = icmp eq i16 %69, 0
  br i1 %70, label %b37, label %71

71:
  br label %b41

b41:
  %72 = phi ptr [ %36, %71 ], [ %76, %b41 ]
  %73 = phi i16 [ %69, %71 ], [ %75, %b41 ]
  %74 = load ptr, ptr %72
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %74)
  %75 = add i16 %73, -1
  %76 = getelementptr i8, ptr %72, i16 2
  %exitcond = icmp eq i16 %75, 0
  br i1 %exitcond, label %b37, label %b41
}

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

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
!15 = !{i8 0, i8 2}
!16 = !{i8 0, i8 2}
!17 = !{i8 0, i8 2}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
!20 = !{i8 0, i8 2}
