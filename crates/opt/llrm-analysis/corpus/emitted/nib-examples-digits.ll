target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str2 = internal constant [18 x i8] c"\08\00\0B\00\0B\00not a digit\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00too big\00"
@$str4 = internal constant [11 x i8] c"\08\00\04\00\04\001234\00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\0012x4\00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\0099999\00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00first even \00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00no even value\00"

define internal cc 1005 void @digit(ptr addrspace(1) %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = icmp ult i8 %1, 48
  br i1 %2, label %b2, label %b5

b2:
  store i8 1, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 1, ptr addrspace(1) %3
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr addrspace(1) %4
  ret void

b3:
  br label %b4

b4:
  %5 = zext i8 %1 to i16
  %6 = sub i16 %5, 48
  %7 = trunc i16 %6 to i8
  store i8 0, ptr addrspace(1) %0
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 %7, ptr addrspace(1) %8
  ret void

b5:
  %9 = icmp ugt i8 %1, 57
  br i1 %9, label %b2, label %b3
}

define internal cc 1005 void @parse(ptr addrspace(1) %0, ptr %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i32
  %3 = alloca [6 x i8]
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca ptr
  store ptr %1, ptr %6, !tbaa !2
  %7 = load ptr, ptr %6, !tbaa !2
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = icmp eq i16 %9, 0
  br i1 %10, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 0, ptr addrspace(1) %11
  %12 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %12)
  ret void

b3:
  br label %b4

b4:
  store i32 0, ptr %5, !tbaa !2
  %13 = load ptr, ptr %6, !tbaa !2
  %14 = getelementptr i8, ptr %13, i16 -4
  %15 = load i16, ptr %14
  store i16 0, ptr %4, !tbaa !2
  br label %b5

b5:
  %16 = load i16, ptr %4, !tbaa !2
  %17 = icmp ult i16 %16, %15
  br i1 %17, label %b6, label %b8

b6:
  %18 = getelementptr inbounds i8, ptr %13, i16 %16
  %19 = load i32, ptr %5, !tbaa !2
  %20 = mul i32 %19, 10
  %21 = addrspacecast ptr %3 to ptr addrspace(1)
  %22 = load i8, ptr %18
  call cc 1005 addrspace(1) void @digit(ptr addrspace(1) %21, i8 zeroext %22)
  %23 = load i8, ptr %3, !tbaa !2, !range !5
  %24 = icmp eq i8 %23, 1
  br i1 %24, label %b9, label %b10

b7:
  %25 = load i16, ptr %4, !tbaa !2
  %26 = add i16 %25, 1
  store i16 %26, ptr %4, !tbaa !2
  br label %b5

b8:
  %27 = load i32, ptr %5, !tbaa !2
  %28 = trunc i32 %27 to i16
  store i8 0, ptr addrspace(1) %0
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %28, ptr addrspace(1) %29
  %30 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %30)
  ret void

b9:
  store i8 1, ptr addrspace(1) %0
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %32 = getelementptr inbounds i8, ptr %3, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %31, ptr %32, i16 4, i1 false)
  %33 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %33)
  ret void

b10:
  %34 = getelementptr inbounds i8, ptr %3, i16 2
  %35 = load i8, ptr %34, !tbaa !2
  %36 = zext i8 %35 to i32
  %37 = add i32 %20, %36
  store i32 %37, ptr %2, !tbaa !2
  %38 = load i32, ptr %2, !tbaa !2
  store i32 %38, ptr %5, !tbaa !2
  %39 = load i32, ptr %5, !tbaa !2
  %40 = icmp ugt i32 %39, 65535
  br i1 %40, label %b11, label %b12

b11:
  store i8 1, ptr addrspace(1) %0
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 2, ptr addrspace(1) %41
  %42 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %42)
  ret void

b12:
  br label %b13

b13:
  br label %b7
}

define internal cc 1005 i32 @first_even(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [4 x i8]
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = mul i16 %4, 2
  %9 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %8
  %10 = load i16, ptr addrspace(1) %9
  %11 = srem i16 %10, 2
  %12 = icmp eq i16 %11, 0
  br i1 %12, label %b6, label %b7

b4:
  %13 = load i16, ptr %1, !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr %1, !tbaa !2
  br label %b2

b5:
  store i32 1, ptr %2, !tbaa !2
  %15 = addrspacecast ptr %2 to ptr addrspace(1)
  %16 = load i32, ptr addrspace(1) %15, !tbaa !2
  ret i32 %16

b6:
  %17 = load i16, ptr addrspace(1) %9
  store i16 0, ptr %2, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %17, ptr %18, !tbaa !2
  %19 = addrspacecast ptr %2 to ptr addrspace(1)
  %20 = load i32, ptr addrspace(1) %19, !tbaa !2
  ret i32 %20

b7:
  br label %b8

b8:
  br label %b4
}

define internal cc 1005 void @report(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [6 x i8]
  %3 = alloca ptr
  store ptr %0, ptr %3, !tbaa !2
  %4 = addrspacecast ptr %2 to ptr addrspace(1)
  %5 = load ptr, ptr %3, !tbaa !2
  store ptr null, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @parse(ptr addrspace(1) %4, ptr %5)
  %6 = load i8, ptr %2, !tbaa !2, !range !6
  %7 = icmp eq i8 %6, 0
  br i1 %7, label %b4, label %b3

b2:
  %8 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %8)
  ret void

b3:
  %9 = load i8, ptr %2, !tbaa !2, !range !6
  %10 = icmp eq i8 %9, 1
  br i1 %10, label %b6, label %b5

b4:
  %11 = getelementptr inbounds i8, ptr %2, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %2, i16 2
  %14 = load i16, ptr %13, !tbaa !2
  store i16 %14, ptr %1, !tbaa !2
  %15 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %15)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b5:
  %16 = load i8, ptr %2, !tbaa !2, !range !6
  %17 = icmp eq i8 %16, 1
  br i1 %17, label %b9, label %b8

b6:
  %18 = getelementptr inbounds i8, ptr %2, i16 2
  %19 = load i8, ptr %18, !tbaa !2, !range !7
  %20 = icmp eq i8 %19, 0
  br i1 %20, label %b7, label %b5

b7:
  %21 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b8:
  %22 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b9:
  %23 = getelementptr inbounds i8, ptr %2, i16 2
  %24 = load i8, ptr %23, !tbaa !2, !range !7
  %25 = icmp eq i8 %24, 1
  br i1 %25, label %b10, label %b8

b10:
  %26 = getelementptr inbounds i8, ptr %2, i16 4
  %27 = load i16, ptr %26, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %2, i16 4
  %29 = load i16, ptr %28, !tbaa !2
  %30 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %30)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [8 x i8]
  %6 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @report(ptr %6)
  %7 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @report(ptr %7)
  %8 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @report(ptr %8)
  %9 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @report(ptr %9)
  store i16 4, ptr %3, !tbaa !2
  store i16 4, ptr %4, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %5, i16 0
  store i16 3, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %5, i16 1
  store i16 7, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i16, ptr %5, i16 2
  store i16 8, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i16, ptr %5, i16 3
  store i16 9, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %14, ptr %16, !tbaa !2
  %17 = addrspacecast ptr %1 to ptr addrspace(1)
  %18 = call cc 1005 addrspace(1) i32 @first_even(ptr addrspace(1) %17)
  %19 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 %18, ptr addrspace(1) %19, !tbaa !2
  %20 = load i8, ptr %2, !tbaa !2, !range !8
  %21 = icmp eq i8 %20, 0
  br i1 %21, label %b4, label %b3

b2:
  ret i16 0

b3:
  %22 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %23 = getelementptr inbounds i8, ptr %2, i16 2
  %24 = load i16, ptr %23, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %2, i16 2
  %26 = load i16, ptr %25, !tbaa !2
  store i16 %26, ptr %0, !tbaa !2
  %27 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %27)
  %28 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %28)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 3}
!8 = !{i8 0, i8 2}
