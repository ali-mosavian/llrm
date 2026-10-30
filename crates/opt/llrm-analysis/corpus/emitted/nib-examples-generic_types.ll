target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str3 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str4 = internal constant [11 x i8] c"\08\00\04\00\04\00top \00"
@$str5 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str6 = internal constant [14 x i8] c"\08\00\07\00\07\00stored \00"
@$str7 = internal constant [15 x i8] c"\08\00\08\00\08\00full at \00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00first roll \00"
@$str9 = internal constant [15 x i8] c"\08\00\08\00\08\00no rolls\00"
@$str10 = internal constant [11 x i8] c"\08\00\04\00\04\00held\00"
@$str11 = internal constant [19 x i8] c"\08\00\0C\00\0C\00nothing held\00"

define internal cc 1005 i32 @store(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [4 x i8]
  %3 = load ptr, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = icmp eq i16 %5, 3
  br i1 %6, label %b2, label %b3

b2:
  %7 = getelementptr inbounds i8, ptr %2, i16 3
  store i8 0, ptr %7, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 2
  store i8 0, ptr %8, !tbaa !2
  %9 = addrspacecast ptr %2 to ptr addrspace(1)
  %10 = load i32, ptr addrspace(1) %9, !tbaa !2
  ret i32 %10

b3:
  br label %b4

b4:
  %11 = load ptr, ptr addrspace(1) %0
  %12 = getelementptr i8, ptr %11, i16 -4
  %13 = load i16, ptr %12
  %14 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %11, i16 1, i16 2)
  store ptr %14, ptr addrspace(1) %0
  %15 = mul i16 %13, 2
  %16 = getelementptr inbounds i8, ptr %14, i16 %15
  store i16 %1, ptr %16
  store i32 0, ptr %2, !tbaa !2
  %17 = addrspacecast ptr %2 to ptr addrspace(1)
  %18 = load i32, ptr addrspace(1) %17, !tbaa !2
  ret i32 %18
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [6 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [6 x i8]
  %6 = alloca i16
  %7 = alloca i8
  %8 = alloca i8
  %9 = alloca i8
  %10 = alloca [4 x i8]
  %11 = alloca i16
  %12 = alloca ptr
  %13 = alloca i16
  %14 = alloca [4 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = alloca [4 x i8]
  %18 = getelementptr i8, ptr @$str1, i16 6
  store ptr %18, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 42, ptr %19, !tbaa !2
  store i8 7, ptr %16, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %16, i16 1
  store i8 1, ptr %20, !tbaa !2
  %21 = load ptr, ptr %17, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  %22 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  %23 = getelementptr inbounds i8, ptr %17, i16 2
  %24 = load i16, ptr %23, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %24)
  %25 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %25)
  %26 = load i8, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %26)
  %27 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %27)
  %28 = getelementptr inbounds i8, ptr %16, i16 1
  %29 = load i8, ptr %28, !tbaa !2, !range !5
  call cc 1005 addrspace(1) void @N$PB(i8 %29)
  call cc 1005 addrspace(1) void @N$PN()
  %30 = getelementptr i8, ptr @$str3, i16 6
  %31 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %30, i16 2, i16 2)
  %32 = getelementptr inbounds i8, ptr %31, i16 0
  store i16 5, ptr %32
  %33 = getelementptr inbounds i8, ptr %31, i16 2
  store i16 9, ptr %33
  store ptr %31, ptr %15, !tbaa !2
  %34 = addrspacecast ptr %15 to ptr addrspace(1)
  %35 = call cc 1005 addrspace(1) i32 @"Stack.top[i16]"(ptr addrspace(1) %34)
  %36 = addrspacecast ptr %14 to ptr addrspace(1)
  store i32 %35, ptr addrspace(1) %36, !tbaa !2
  %37 = load i8, ptr %14, !tbaa !2, !range !6
  %38 = icmp eq i8 %37, 1
  br i1 %38, label %b4, label %b3

b2:
  %39 = getelementptr i8, ptr @$str3, i16 6
  %40 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %39, i16 3, i16 2)
  %41 = getelementptr inbounds i8, ptr %40, i16 0
  store i16 11, ptr %41
  %42 = getelementptr inbounds i8, ptr %40, i16 2
  store i16 12, ptr %42
  %43 = getelementptr inbounds i8, ptr %40, i16 4
  store i16 13, ptr %43
  store ptr %40, ptr %12, !tbaa !2
  %44 = load ptr, ptr %12, !tbaa !2
  %45 = getelementptr i8, ptr %44, i16 -4
  %46 = load i16, ptr %45
  store i16 0, ptr %11, !tbaa !2
  br label %b6

b3:
  %47 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %47)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %48 = getelementptr inbounds i8, ptr %14, i16 2
  %49 = load i16, ptr %48, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %14, i16 2
  %51 = load i16, ptr %50, !tbaa !2
  store i16 %51, ptr %13, !tbaa !2
  %52 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %52)
  %53 = load i16, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %53)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b6:
  %54 = load i16, ptr %11, !tbaa !2
  %55 = icmp ult i16 %54, %46
  br i1 %55, label %b7, label %b9

b7:
  %56 = mul i16 %54, 2
  %57 = getelementptr inbounds i8, ptr %44, i16 %56
  %58 = addrspacecast ptr %15 to ptr addrspace(1)
  %59 = load i16, ptr %57
  %60 = call cc 1005 addrspace(1) i32 @store(ptr addrspace(1) %58, i16 %59)
  %61 = addrspacecast ptr %10 to ptr addrspace(1)
  store i32 %60, ptr addrspace(1) %61, !tbaa !2
  %62 = load i8, ptr %10, !tbaa !2, !range !6
  %63 = icmp eq i8 %62, 0
  br i1 %63, label %b12, label %b11

b8:
  %64 = load i16, ptr %11, !tbaa !2
  %65 = add i16 %64, 1
  store i16 %65, ptr %11, !tbaa !2
  br label %b6

b9:
  %66 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %66)
  store i8 3, ptr %9, !tbaa !2
  store i8 -56, ptr %8, !tbaa !2
  %67 = load i8, ptr %9, !tbaa !2
  %68 = load i8, ptr %8, !tbaa !2
  %69 = call cc 1005 addrspace(1) i8 @"larger[u8]"(i8 zeroext %67, i8 zeroext %68)
  store i8 %69, ptr %7, !tbaa !2
  %70 = call cc 1005 addrspace(1) i16 @"larger[i16]"(i16 -1, i16 -5)
  store i16 %70, ptr %6, !tbaa !2
  %71 = load i8, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %71)
  %72 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %72)
  %73 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %73)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 3, ptr %3, !tbaa !2
  store i16 3, ptr %4, !tbaa !2
  %74 = getelementptr inbounds i16, ptr %5, i16 0
  store i16 4, ptr %74, !tbaa !2
  %75 = getelementptr inbounds i16, ptr %5, i16 1
  store i16 6, ptr %75, !tbaa !2
  %76 = getelementptr inbounds i16, ptr %5, i16 2
  store i16 1, ptr %76, !tbaa !2
  %77 = addrspacecast ptr %2 to ptr addrspace(1)
  %78 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %79, !tbaa !2
  %80 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %78, ptr %80, !tbaa !2
  %81 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @"first[i16]"(ptr addrspace(1) %77, ptr addrspace(1) %81)
  %82 = load i8, ptr %2, !tbaa !2, !range !6
  %83 = icmp eq i8 %82, 0
  br i1 %83, label %b16, label %b15

b10:
  br label %b8

b11:
  %84 = getelementptr inbounds i8, ptr %10, i16 2
  %85 = load i8, ptr %84, !tbaa !2
  %86 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %86)
  %87 = load i16, ptr %57
  call cc 1005 addrspace(1) void @N$PI2(i16 %87)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b10

b12:
  %88 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %88)
  %89 = load i16, ptr %57
  call cc 1005 addrspace(1) void @N$PI2(i16 %89)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b10

b14:
  store i32 0, ptr %0, !tbaa !2
  %90 = load i8, ptr %0, !tbaa !2, !range !6
  %91 = icmp eq i8 %90, 1
  br i1 %91, label %b20, label %b19

b15:
  %92 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %92)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b14

b16:
  %93 = getelementptr inbounds i8, ptr %2, i16 2
  %94 = load ptr addrspace(1), ptr %93, !tbaa !2
  %95 = getelementptr inbounds i8, ptr %2, i16 2
  %96 = load ptr addrspace(1), ptr %95, !tbaa !2
  %97 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %97)
  %98 = load i16, ptr addrspace(1) %96
  call cc 1005 addrspace(1) void @N$PI2(i16 %98)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b14

b18:
  %99 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %99)
  %100 = load ptr, ptr %17, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %100)
  ret i16 0

b19:
  %101 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %101)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b18

b20:
  %102 = getelementptr inbounds i8, ptr %0, i16 2
  %103 = load i16, ptr %102, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %0, i16 2
  %105 = load i16, ptr %104, !tbaa !2
  %106 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %106)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b18
}

define internal cc 1005 void @"first[i16]"(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %1
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b4, label %b3

b3:
  %6 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 0
  %7 = load i16, ptr addrspace(1) %6
  store i8 0, ptr addrspace(1) %0
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %6, ptr addrspace(1) %8
  ret void

b4:
  store i8 1, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 i16 @"larger[i16]"(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = icmp sgt i16 %0, %1
  br i1 %3, label %b2, label %b3

b2:
  store i16 %0, ptr %2, !tbaa !2
  br label %b4

b3:
  store i16 %1, ptr %2, !tbaa !2
  br label %b4

b4:
  %4 = load i16, ptr %2, !tbaa !2
  ret i16 %4
}

define internal cc 1005 i8 @"larger[u8]"(i8 %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = zext i8 %0 to i16
  %4 = zext i8 %1 to i16
  %5 = icmp sgt i16 %3, %4
  br i1 %5, label %b2, label %b3

b2:
  store i8 %0, ptr %2, !tbaa !2
  br label %b4

b3:
  store i8 %1, ptr %2, !tbaa !2
  br label %b4

b4:
  %6 = load i8, ptr %2, !tbaa !2
  ret i8 %6
}

define internal cc 1005 i32 @"Stack.top[i16]"(ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [4 x i8]
  %2 = load ptr, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr %2, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i32 0, ptr %1, !tbaa !2
  %6 = addrspacecast ptr %1 to ptr addrspace(1)
  %7 = load i32, ptr addrspace(1) %6, !tbaa !2
  ret i32 %7

b3:
  br label %b4

b4:
  %8 = load ptr, ptr addrspace(1) %0
  %9 = load ptr, ptr addrspace(1) %0
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = sub i16 %11, 1
  %13 = getelementptr i8, ptr %8, i16 -4
  %14 = load i16, ptr %13
  %15 = icmp ult i16 %12, %14
  br i1 %15, label %b5, label %b6

b5:
  %16 = mul i16 %12, 2
  %17 = getelementptr inbounds i8, ptr %8, i16 %16
  %18 = load i16, ptr %17
  store i16 1, ptr %1, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %18, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %1 to ptr addrspace(1)
  %21 = load i32, ptr addrspace(1) %20, !tbaa !2
  ret i32 %21

b6:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
