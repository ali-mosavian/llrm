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

define internal cc 1005 ptr @shout(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca ptr
  %5 = alloca ptr
  store ptr %0, ptr %5, !tbaa !2
  %6 = load ptr, ptr %5, !tbaa !2
  store ptr null, ptr %5, !tbaa !2
  store ptr %6, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %3, !tbaa !2
  %8 = load ptr, ptr %4, !tbaa !2
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = icmp ult i16 %7, %10
  br i1 %11, label %b3, label %b4

b3:
  %12 = load ptr, ptr %4, !tbaa !2
  %13 = load i16, ptr %3, !tbaa !2
  %14 = getelementptr i8, ptr %12, i16 -4
  %15 = load i16, ptr %14
  %16 = icmp ult i16 %13, %15
  br i1 %16, label %b5, label %b6

b4:
  %17 = load ptr, ptr %4, !tbaa !2
  store ptr null, ptr %4, !tbaa !2
  %18 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %18)
  %19 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %19)
  ret ptr %17

b5:
  %20 = getelementptr inbounds i8, ptr %12, i16 %13
  %21 = load i8, ptr %20
  store i8 %21, ptr %2, !tbaa !2
  %22 = load i8, ptr %2, !tbaa !2
  %23 = icmp ule i8 97, %22
  %24 = zext i1 %23 to i8
  store i8 %24, ptr %1, !tbaa !2
  br i1 %23, label %b11, label %b10

b6:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b7:
  %25 = load ptr, ptr %4, !tbaa !2
  %26 = call cc 1005 addrspace(1) ptr @N$BRES(ptr %25, i16 0, i16 1)
  store ptr %26, ptr %4, !tbaa !2
  %27 = load i16, ptr %3, !tbaa !2
  %28 = getelementptr i8, ptr %26, i16 -4
  %29 = load i16, ptr %28
  %30 = icmp ult i16 %27, %29
  br i1 %30, label %b12, label %b13

b8:
  br label %b9

b9:
  %31 = load i16, ptr %3, !tbaa !2
  %32 = add i16 %31, 1
  store i16 %32, ptr %3, !tbaa !2
  br label %b2

b10:
  %33 = load i8, ptr %1, !tbaa !2, !range !5
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b7, label %b8

b11:
  %35 = icmp ule i8 %22, 122
  %36 = zext i1 %35 to i8
  store i8 %36, ptr %1, !tbaa !2
  br label %b10

b12:
  %37 = getelementptr inbounds i8, ptr %26, i16 %27
  %38 = load i8, ptr %2, !tbaa !2
  %39 = zext i8 %38 to i16
  %40 = sub i16 %39, 32
  %41 = trunc i16 %40 to i8
  store i8 %41, ptr %37
  br label %b9

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 ptr @label(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  store ptr %0, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PBEG()
  %3 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %3)
  %4 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %4)
  call cc 1005 addrspace(1) void @N$PI2(i16 %1)
  %5 = call cc 1005 addrspace(1) ptr @N$PEND()
  %6 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %6)
  ret ptr %5
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca ptr
  %3 = alloca ptr
  %4 = alloca ptr
  %5 = alloca ptr
  %6 = getelementptr i8, ptr @$str2, i16 6
  store ptr %6, ptr %5, !tbaa !2
  %7 = load ptr, ptr %5, !tbaa !2
  %8 = getelementptr i8, ptr @$str3, i16 6
  %9 = call cc 1005 addrspace(1) ptr @N$TCAT(ptr %7, ptr %8)
  store ptr %9, ptr %4, !tbaa !2
  %10 = getelementptr i8, ptr @$str4, i16 6
  %11 = load ptr, ptr %4, !tbaa !2
  %12 = call cc 1005 addrspace(1) ptr @N$TAPP(ptr %11, ptr %10)
  store ptr %12, ptr %4, !tbaa !2
  %13 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %13)
  call cc 1005 addrspace(1) void @N$PN()
  %14 = load ptr, ptr %4, !tbaa !2
  %15 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %14, i16 1)
  %16 = call cc 1005 addrspace(1) ptr @shout(ptr %15)
  store ptr %16, ptr %3, !tbaa !2
  %17 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %17)
  call cc 1005 addrspace(1) void @N$PN()
  %18 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %18)
  call cc 1005 addrspace(1) void @N$PN()
  %19 = getelementptr i8, ptr @$str5, i16 6
  %20 = call cc 1005 addrspace(1) ptr @label(ptr %19, i16 42)
  store ptr %20, ptr %2, !tbaa !2
  %21 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  %22 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  %23 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %23)
  %24 = load ptr, ptr %2, !tbaa !2
  %25 = getelementptr i8, ptr %24, i16 -4
  %26 = load i16, ptr %25
  call cc 1005 addrspace(1) void @N$PU2(i16 %26)
  %27 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %27)
  call cc 1005 addrspace(1) void @N$PN()
  %28 = load ptr, ptr %2, !tbaa !2
  %29 = getelementptr i8, ptr @$str9, i16 6
  %30 = call cc 1005 addrspace(1) ptr @label(ptr %29, i16 1)
  %31 = getelementptr i8, ptr %28, i16 -4
  %32 = load i16, ptr %31
  %33 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 %32, ptr %1, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %32, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %1 to ptr addrspace(1)
  %37 = getelementptr i8, ptr %30, i16 -4
  %38 = load i16, ptr %37
  %39 = addrspacecast ptr %30 to ptr addrspace(1)
  store i16 %38, ptr %0, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %38, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %39, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %0 to ptr addrspace(1)
  %43 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %36, ptr addrspace(1) %42)
  %44 = icmp slt i8 %43, 0
  call cc 1005 addrspace(1) void @N$BDRP(ptr %30)
  br i1 %44, label %b2, label %b3

b2:
  %45 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %45)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b3:
  br label %b4

b4:
  %46 = getelementptr i8, ptr @$str11, i16 6
  %47 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %47)
  store ptr %46, ptr %4, !tbaa !2
  %48 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %48)
  call cc 1005 addrspace(1) void @N$PN()
  %49 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %49)
  %50 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %50)
  %51 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %51)
  %52 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %52)
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
