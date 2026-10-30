target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00north\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00east\00"
@$str3 = internal constant [16 x i8] c"\08\00\09\00\09\00southwest\00"
@$str4 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal cc 1005 void @longer(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = load i16, ptr addrspace(1) %1
  %4 = load i16, ptr addrspace(1) %2
  %5 = icmp uge i16 %3, %4
  br i1 %5, label %b2, label %b3

b2:
  %6 = load i16, ptr addrspace(1) %1
  store i16 %6, ptr addrspace(1) %0
  %7 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %8 = load i16, ptr addrspace(1) %7
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %8, ptr addrspace(1) %9
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %11, ptr addrspace(1) %12
  ret void

b3:
  br label %b4

b4:
  %13 = load i16, ptr addrspace(1) %2
  store i16 %13, ptr addrspace(1) %0
  %14 = getelementptr i8, ptr addrspace(1) %2, i16 2
  %15 = load i16, ptr addrspace(1) %14
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %15, ptr addrspace(1) %16
  %17 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %18 = load ptr addrspace(1), ptr addrspace(1) %17
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %18, ptr addrspace(1) %19
  ret void
}

define internal cc 1005 void @add_into(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = load i16, ptr addrspace(1) %1
  %4 = add i16 %2, %3
  store i16 %4, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 void @Bag.tally(ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = load ptr, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 %5, ptr %2, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %5, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %6, ptr %8, !tbaa !2
  %9 = addrspacecast ptr %2 to ptr addrspace(1)
  %10 = load i16, ptr addrspace(1) %9
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %11 = load i16, ptr %1, !tbaa !2
  %12 = icmp ult i16 %11, %10
  br i1 %12, label %b3, label %b5

b3:
  %13 = getelementptr i8, ptr addrspace(1) %9, i16 4
  %14 = load ptr addrspace(1), ptr addrspace(1) %13, !tbaa !2
  %15 = mul i16 %11, 2
  %16 = getelementptr inbounds i8, ptr addrspace(1) %14, i16 %15
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %18 = load i16, ptr addrspace(1) %17
  %19 = load i16, ptr addrspace(1) %16
  %20 = add i16 %18, %19
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %20, ptr addrspace(1) %21
  br label %b4

b4:
  %22 = load i16, ptr %1, !tbaa !2
  %23 = add i16 %22, 1
  store i16 %23, ptr %1, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca ptr
  %4 = alloca [4 x i8]
  %5 = alloca [4 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca ptr
  %12 = alloca ptr
  %13 = alloca ptr
  %14 = getelementptr i8, ptr @$str1, i16 6
  store ptr %14, ptr %13, !tbaa !2
  %15 = getelementptr i8, ptr @$str2, i16 6
  store ptr %15, ptr %12, !tbaa !2
  %16 = getelementptr i8, ptr @$str3, i16 6
  store ptr %16, ptr %11, !tbaa !2
  %17 = addrspacecast ptr %10 to ptr addrspace(1)
  %18 = addrspacecast ptr %9 to ptr addrspace(1)
  %19 = load ptr, ptr %13, !tbaa !2
  %20 = getelementptr i8, ptr %19, i16 -4
  %21 = load i16, ptr %20
  %22 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 %21, ptr %8, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %21, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %22, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %8 to ptr addrspace(1)
  %26 = load ptr, ptr %12, !tbaa !2
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 %28, ptr %7, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %28, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @longer(ptr addrspace(1) %18, ptr addrspace(1) %25, ptr addrspace(1) %32)
  %33 = load ptr, ptr %11, !tbaa !2
  %34 = getelementptr i8, ptr %33, i16 -4
  %35 = load i16, ptr %34
  %36 = addrspacecast ptr %33 to ptr addrspace(1)
  store i16 %35, ptr %6, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %35, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %36, ptr %38, !tbaa !2
  %39 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @longer(ptr addrspace(1) %17, ptr addrspace(1) %18, ptr addrspace(1) %39)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %17)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 1, ptr %5, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 2, ptr %40, !tbaa !2
  %41 = addrspacecast ptr %5 to ptr addrspace(1)
  %42 = getelementptr inbounds i8, ptr %5, i16 2
  %43 = addrspacecast ptr %42 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @add_into(ptr addrspace(1) %41, ptr addrspace(1) %43)
  %44 = load i16, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %44)
  call cc 1005 addrspace(1) void @N$PN()
  %45 = getelementptr i8, ptr @$str4, i16 6
  %46 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %45, i16 3, i16 2)
  %47 = getelementptr inbounds i8, ptr %46, i16 0
  store i16 1, ptr %47
  %48 = getelementptr inbounds i8, ptr %46, i16 2
  store i16 2, ptr %48
  %49 = getelementptr inbounds i8, ptr %46, i16 4
  store i16 3, ptr %49
  store ptr %46, ptr %4, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 0, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %4 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Bag.tally(ptr addrspace(1) %51)
  %52 = getelementptr inbounds i8, ptr %4, i16 2
  %53 = load i16, ptr %52, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %53)
  call cc 1005 addrspace(1) void @N$PN()
  %54 = getelementptr i8, ptr @$str4, i16 6
  %55 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %54, i16 4, i16 2)
  %56 = getelementptr inbounds i8, ptr %55, i16 0
  store i16 1, ptr %56
  %57 = getelementptr inbounds i8, ptr %55, i16 2
  store i16 2, ptr %57
  %58 = getelementptr inbounds i8, ptr %55, i16 4
  store i16 3, ptr %58
  %59 = getelementptr inbounds i8, ptr %55, i16 6
  store i16 4, ptr %59
  store ptr %55, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %60 = addrspacecast ptr %3 to ptr addrspace(1)
  %61 = load ptr, ptr addrspace(1) %60, !tbaa !2
  %62 = getelementptr i8, ptr %61, i16 -4
  %63 = load i16, ptr %62
  store i16 0, ptr %1, !tbaa !2
  br label %b3

b2:
  %64 = load ptr, ptr %3, !tbaa !2
  %65 = getelementptr i8, ptr %64, i16 -4
  %66 = load i16, ptr %65
  %67 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %64, i16 1, i16 2)
  store ptr %67, ptr %3, !tbaa !2
  %68 = mul i16 %66, 2
  %69 = getelementptr inbounds i8, ptr %67, i16 %68
  %70 = load i16, ptr %2, !tbaa !2
  store i16 %70, ptr %69
  %71 = load ptr, ptr %3, !tbaa !2
  %72 = getelementptr i8, ptr %71, i16 -4
  %73 = load i16, ptr %72
  %74 = icmp ult i16 4, %73
  br i1 %74, label %b11, label %b12

b3:
  %75 = load i16, ptr %1, !tbaa !2
  %76 = icmp ult i16 %75, %63
  br i1 %76, label %b4, label %b6

b4:
  %77 = mul i16 %75, 2
  %78 = getelementptr inbounds i8, ptr %61, i16 %77
  %79 = load i16, ptr %78
  %80 = srem i16 %79, 2
  %81 = icmp eq i16 %80, 0
  br i1 %81, label %b7, label %b8

b5:
  %82 = load i16, ptr %1, !tbaa !2
  %83 = add i16 %82, 1
  store i16 %83, ptr %1, !tbaa !2
  br label %b3

b6:
  br label %b2

b7:
  %84 = load i16, ptr %78
  store i16 %84, ptr %0, !tbaa !2
  %85 = load i16, ptr %2, !tbaa !2
  %86 = load i16, ptr %0, !tbaa !2
  %87 = add i16 %85, %86
  store i16 %87, ptr %2, !tbaa !2
  br label %b10

b8:
  br label %b9

b9:
  br label %b5

b10:
  br label %b9

b11:
  %88 = getelementptr inbounds i8, ptr %71, i16 8
  %89 = load i16, ptr %88
  call cc 1005 addrspace(1) void @N$PI2(i16 %89)
  call cc 1005 addrspace(1) void @N$PN()
  %90 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %90)
  %91 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %91)
  %92 = load ptr, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %92)
  %93 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %93)
  %94 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %94)
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

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
