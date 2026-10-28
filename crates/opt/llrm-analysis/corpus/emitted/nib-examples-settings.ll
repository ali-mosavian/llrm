target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00= \00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00level\00"
@$str3 = internal constant [31 x i8] c"\08\00\18\00\18\00          twice that is \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00name=Ada;role=pilot;level=7\00"
@$str5 = internal constant [23 x i8] c"\08\00\10\00\10\00 settings, kept \00"

define internal cc 1005 i16 @digits(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
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
  %8 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %4
  %9 = load i16, ptr %2, !tbaa !2
  %10 = mul i16 %9, 10
  %11 = load i8, ptr addrspace(1) %8
  %12 = zext i8 %11 to i16
  %13 = add i16 %10, %12
  %14 = sub i16 %13, 48
  store i16 %14, ptr %2, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %1, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %1, !tbaa !2
  br label %b2

b5:
  %17 = load i16, ptr %2, !tbaa !2
  ret i16 %17
}

define internal cc 1005 void @show(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca [8 x i8]
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 8, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %4 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %4)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %1)
  call cc 1005 addrspace(1) void @N$PN()
  %5 = getelementptr i8, ptr @$str2, i16 6
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 %7, ptr %3, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %7, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %3 to ptr addrspace(1)
  %12 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %0, ptr addrspace(1) %11)
  %13 = icmp eq i8 %12, 0
  br i1 %13, label %b2, label %b3

b2:
  %14 = call cc 1005 addrspace(1) i16 @digits(ptr addrspace(1) %1)
  %15 = mul i16 %14, 2
  store i16 %15, ptr %2, !tbaa !2
  %16 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %16)
  %17 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %17)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b3:
  br label %b4

b4:
  ret void
}

define internal cc 1005 ptr @keep(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %0)
  ret ptr %1
}

define internal cc 1005 i16 @each_setting(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = load i16, ptr addrspace(1) %0
  %9 = icmp ule i16 %7, %8
  br i1 %9, label %b3, label %b4

b3:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = load i16, ptr addrspace(1) %0
  %12 = icmp eq i16 %10, %11
  br i1 %12, label %b5, label %b8

b4:
  %13 = load i16, ptr %6, !tbaa !2
  ret i16 %13

b5:
  %14 = load i16, ptr %5, !tbaa !2
  store i16 %14, ptr %3, !tbaa !2
  br label %b11

b6:
  br label %b7

b7:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %4, !tbaa !2
  br label %b2

b8:
  %17 = load i16, ptr %4, !tbaa !2
  %18 = load i16, ptr addrspace(1) %0
  %19 = icmp ult i16 %17, %18
  br i1 %19, label %b9, label %b10

b9:
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20
  %22 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 %17
  %23 = load i8, ptr addrspace(1) %22
  %24 = icmp eq i8 %23, 59
  br i1 %24, label %b5, label %b6

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  %25 = load i16, ptr %3, !tbaa !2
  %26 = load i16, ptr %4, !tbaa !2
  %27 = icmp ult i16 %25, %26
  br i1 %27, label %b14, label %b13

b12:
  %28 = load i16, ptr %3, !tbaa !2
  %29 = add i16 %28, 1
  store i16 %29, ptr %3, !tbaa !2
  br label %b11

b13:
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %31 = load ptr addrspace(1), ptr addrspace(1) %30
  %32 = load i16, ptr addrspace(1) %0
  %33 = load i16, ptr %5, !tbaa !2
  %34 = load i16, ptr %3, !tbaa !2
  %35 = icmp ule i16 %34, %32
  br i1 %35, label %b17, label %b18

b14:
  %36 = load i16, ptr %3, !tbaa !2
  %37 = load i16, ptr addrspace(1) %0
  %38 = icmp ult i16 %36, %37
  br i1 %38, label %b15, label %b16

b15:
  %39 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %40 = load ptr addrspace(1), ptr addrspace(1) %39
  %41 = getelementptr inbounds i8, ptr addrspace(1) %40, i16 %36
  %42 = load i8, ptr addrspace(1) %41
  %43 = icmp ne i8 %42, 61
  br i1 %43, label %b12, label %b13

b16:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b17:
  %44 = icmp ule i16 %33, %34
  br i1 %44, label %b19, label %b20

b18:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  %45 = getelementptr inbounds i8, ptr addrspace(1) %31, i16 %33
  %46 = sub i16 %34, %33
  store i16 %46, ptr %2, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %46, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %45, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %2 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %51 = load ptr addrspace(1), ptr addrspace(1) %50
  %52 = load i16, ptr addrspace(1) %0
  %53 = load i16, ptr %3, !tbaa !2
  %54 = add i16 %53, 1
  %55 = load i16, ptr %4, !tbaa !2
  %56 = icmp ule i16 %55, %52
  br i1 %56, label %b21, label %b22

b20:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b21:
  %57 = icmp ule i16 %54, %55
  br i1 %57, label %b23, label %b24

b22:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b23:
  %58 = getelementptr inbounds i8, ptr addrspace(1) %51, i16 %54
  %59 = sub i16 %55, %54
  store i16 %59, ptr %1, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %59, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %58, ptr %61, !tbaa !2
  %62 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @show(ptr addrspace(1) %49, ptr addrspace(1) %62)
  %63 = load i16, ptr %6, !tbaa !2
  %64 = add i16 %63, 1
  store i16 %64, ptr %6, !tbaa !2
  %65 = load i16, ptr %4, !tbaa !2
  %66 = add i16 %65, 1
  store i16 %66, ptr %5, !tbaa !2
  br label %b7

b24:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca ptr
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca ptr
  %5 = getelementptr i8, ptr @$str4, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = load ptr, ptr %4, !tbaa !2
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr %3, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %8, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  %13 = call cc 1005 addrspace(1) i16 @each_setting(ptr addrspace(1) %12)
  store i16 %13, ptr %2, !tbaa !2
  %14 = load ptr, ptr %4, !tbaa !2
  %15 = getelementptr i8, ptr %14, i16 -4
  %16 = load i16, ptr %15
  %17 = addrspacecast ptr %14 to ptr addrspace(1)
  %18 = icmp ule i16 8, %16
  br i1 %18, label %b2, label %b3

b2:
  %19 = getelementptr inbounds i8, ptr addrspace(1) %17, i16 5
  store i16 3, ptr %1, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %19, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %1 to ptr addrspace(1)
  %23 = call cc 1005 addrspace(1) ptr @keep(ptr addrspace(1) %22)
  store ptr %23, ptr %0, !tbaa !2
  %24 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %24)
  %25 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %25)
  %26 = load ptr, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %26)
  call cc 1005 addrspace(1) void @N$PN()
  %27 = load ptr, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %27)
  %28 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %28)
  ret i16 0

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
