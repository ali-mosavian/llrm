target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00area \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00mode \00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00 has \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 columns\00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00text is \00"

define internal cc 1005 i32 @area(ptr addrspace(1) nonnull dereferenceable(10) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = load i8, ptr addrspace(1) %0, !range !5
  %3 = icmp eq i8 %2, 0
  br i1 %3, label %b4, label %b3

b3:
  %4 = load i8, ptr addrspace(1) %0, !range !5
  %5 = icmp eq i8 %4, 1
  br i1 %5, label %b6, label %b5

b4:
  ret i32 0

b5:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %13 = load i16, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 8
  %15 = load i16, ptr addrspace(1) %14
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 8
  %18 = load i16, ptr addrspace(1) %16
  %19 = load i16, ptr addrspace(1) %10
  %20 = sub i16 %18, %19
  %21 = sext i16 %20 to i32
  %22 = load i16, ptr addrspace(1) %17
  %23 = load i16, ptr addrspace(1) %11
  %24 = sub i16 %22, %23
  %25 = sext i16 %24 to i32
  %26 = mul i32 %21, %25
  ret i32 %26

b6:
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %28 = load i16, ptr addrspace(1) %27
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %30 = load i16, ptr addrspace(1) %29
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %32 = load i16, ptr addrspace(1) %31
  %33 = zext i16 %32 to i32
  store i32 %33, ptr %1, !tbaa !2
  %34 = load i32, ptr %1, !tbaa !2
  %35 = mul i32 3, %34
  %36 = load i32, ptr %1, !tbaa !2
  %37 = mul i32 %35, %36
  ret i32 %37
}

define internal cc 1005 i16 @columns(i8 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = icmp eq i8 %0, 0
  br i1 %1, label %b4, label %b3

b3:
  %2 = icmp eq i8 %0, 1
  br i1 %2, label %b6, label %b5

b4:
  ret i16 80

b5:
  ret i16 320

b6:
  ret i16 40
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i8
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [30 x i8]
  store i16 3, ptr %6, !tbaa !2
  store i16 3, ptr %7, !tbaa !2
  %9 = getelementptr inbounds [10 x i8], ptr %8, i16 0
  store i8 0, ptr %9, !tbaa !2
  %10 = getelementptr inbounds [10 x i8], ptr %8, i16 0
  %11 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 1, ptr %11, !tbaa !2
  %12 = getelementptr inbounds [10 x i8], ptr %8, i16 0
  %13 = getelementptr inbounds i8, ptr %12, i16 4
  store i16 2, ptr %13, !tbaa !2
  %14 = getelementptr inbounds [10 x i8], ptr %8, i16 1
  store i8 1, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [10 x i8], ptr %8, i16 1
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 0, ptr %16, !tbaa !2
  %17 = getelementptr inbounds [10 x i8], ptr %8, i16 1
  %18 = getelementptr inbounds i8, ptr %17, i16 4
  store i16 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds [10 x i8], ptr %8, i16 1
  %20 = getelementptr inbounds i8, ptr %19, i16 6
  store i16 5, ptr %20, !tbaa !2
  %21 = getelementptr inbounds [10 x i8], ptr %8, i16 2
  store i8 2, ptr %21, !tbaa !2
  %22 = getelementptr inbounds [10 x i8], ptr %8, i16 2
  %23 = getelementptr inbounds i8, ptr %22, i16 2
  store i16 1, ptr %23, !tbaa !2
  %24 = getelementptr inbounds [10 x i8], ptr %8, i16 2
  %25 = getelementptr inbounds i8, ptr %24, i16 4
  store i16 1, ptr %25, !tbaa !2
  %26 = getelementptr inbounds [10 x i8], ptr %8, i16 2
  %27 = getelementptr inbounds i8, ptr %26, i16 6
  store i16 4, ptr %27, !tbaa !2
  %28 = getelementptr inbounds [10 x i8], ptr %8, i16 2
  %29 = getelementptr inbounds i8, ptr %28, i16 8
  store i16 5, ptr %29, !tbaa !2
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  br label %b2

b2:
  %30 = load i16, ptr %4, !tbaa !2
  %31 = icmp ult i16 %30, 3
  br i1 %31, label %b3, label %b5

b3:
  %32 = load i32, ptr %5, !tbaa !2
  %33 = sub i16 %30, 0
  %34 = getelementptr inbounds [10 x i8], ptr %8, i16 %33
  %35 = addrspacecast ptr %34 to ptr addrspace(1)
  %36 = call cc 1005 addrspace(1) i32 @area(ptr addrspace(1) %35)
  %37 = add i32 %32, %36
  store i32 %37, ptr %5, !tbaa !2
  br label %b4

b4:
  %38 = load i16, ptr %4, !tbaa !2
  %39 = add i16 %38, 1
  store i16 %39, ptr %4, !tbaa !2
  br label %b2

b5:
  %40 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %40)
  %41 = load i32, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %41)
  call cc 1005 addrspace(1) void @N$PN()
  store i8 19, ptr %3, !tbaa !2
  %42 = load i8, ptr %3, !tbaa !2
  store i8 %42, ptr %2, !tbaa !2
  %43 = load i8, ptr %3, !tbaa !2
  %44 = call cc 1005 addrspace(1) i16 @columns(i8 zeroext %43)
  store i16 %44, ptr %1, !tbaa !2
  %45 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %45)
  %46 = load i8, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %46)
  %47 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %47)
  %48 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %48)
  %49 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %49)
  call cc 1005 addrspace(1) void @N$PN()
  %50 = call cc 1005 addrspace(1) i16 @columns(i8 zeroext 0)
  store i16 %50, ptr %0, !tbaa !2
  %51 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %51)
  %52 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %52)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 3}
