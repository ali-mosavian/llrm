target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str2 = internal constant [16 x i8] c"\08\00\09\00\09\00first six\00"

define internal cc 1005 i32 @Dice.iter(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [4 x i8]
  %2 = load i16, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  store i16 %2, ptr %1, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %4, ptr %5, !tbaa !2
  %6 = addrspacecast ptr %1 to ptr addrspace(1)
  %7 = load i32, ptr addrspace(1) %6, !tbaa !2
  ret i32 %7
}

define internal cc 1005 i32 @Rolls.next(ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [4 x i8]
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  store i32 1, ptr %1, !tbaa !2
  %5 = addrspacecast ptr %1 to ptr addrspace(1)
  %6 = load i32, ptr addrspace(1) %5, !tbaa !2
  ret i32 %6

b3:
  br label %b4

b4:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %8 = load i16, ptr addrspace(1) %7
  %9 = sub i16 %8, 1
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %9, ptr addrspace(1) %10
  %11 = load i16, ptr addrspace(1) %0
  %12 = mul i16 %11, 25173
  %13 = add i16 %12, 13849
  store i16 %13, ptr addrspace(1) %0
  %14 = load i16, ptr addrspace(1) %0
  %15 = lshr i16 %14, 8
  %16 = urem i16 %15, 6
  %17 = add i16 %16, 1
  store i16 0, ptr %1, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %17, ptr %18, !tbaa !2
  %19 = addrspacecast ptr %1 to ptr addrspace(1)
  %20 = load i32, ptr addrspace(1) %19, !tbaa !2
  ret i32 %20
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [4 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [4 x i8]
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca [12 x i8]
  %14 = alloca i16
  %15 = alloca [4 x i8]
  store i16 2024, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 60, ptr %16, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  store i16 6, ptr %11, !tbaa !2
  store i16 6, ptr %12, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 6, ptr %9, !tbaa !2
  br label %b2

b2:
  %17 = load i16, ptr %10, !tbaa !2
  %18 = load i16, ptr %9, !tbaa !2
  %19 = icmp slt i16 %17, %18
  br i1 %19, label %b3, label %b5

b3:
  %20 = load i16, ptr %10, !tbaa !2
  %21 = load i16, ptr %14, !tbaa !2
  %22 = sub i16 %20, 0
  %23 = getelementptr inbounds i16, ptr %13, i16 %22
  store i16 %21, ptr %23, !tbaa !2
  br label %b4

b4:
  %24 = load i16, ptr %10, !tbaa !2
  %25 = add nsw i16 %24, 1
  store i16 %25, ptr %10, !tbaa !2
  br label %b2

b5:
  %26 = addrspacecast ptr %15 to ptr addrspace(1)
  %27 = call cc 1005 addrspace(1) i32 @Dice.iter(ptr addrspace(1) %26)
  %28 = addrspacecast ptr %7 to ptr addrspace(1)
  store i32 %27, ptr addrspace(1) %28, !tbaa !2
  %29 = load i16, ptr %7, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %7, i16 2
  %31 = load i16, ptr %30, !tbaa !2
  store i16 %29, ptr %8, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %31, ptr %32, !tbaa !2
  br label %b6

b6:
  br label %b7

b7:
  %33 = addrspacecast ptr %8 to ptr addrspace(1)
  %34 = call cc 1005 addrspace(1) i32 @Rolls.next(ptr addrspace(1) %33)
  %35 = addrspacecast ptr %6 to ptr addrspace(1)
  store i32 %34, ptr addrspace(1) %35, !tbaa !2
  %36 = load i8, ptr %6, !tbaa !2, !range !5
  %37 = icmp eq i8 %36, 0
  br i1 %37, label %b11, label %b10

b8:
  store i16 0, ptr %4, !tbaa !2
  store i16 6, ptr %3, !tbaa !2
  br label %b15

b9:
  br label %b6

b10:
  br label %b8

b11:
  %38 = getelementptr inbounds i8, ptr %6, i16 2
  %39 = load i16, ptr %38, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %6, i16 2
  %41 = load i16, ptr %40, !tbaa !2
  store i16 %41, ptr %5, !tbaa !2
  %42 = load i16, ptr %5, !tbaa !2
  %43 = sub i16 %42, 1
  %44 = icmp ult i16 %43, 6
  br i1 %44, label %b12, label %b13

b12:
  %45 = sub i16 %43, 0
  %46 = getelementptr inbounds i16, ptr %13, i16 %45
  %47 = load i16, ptr %46, !tbaa !2
  %48 = add i16 %47, 1
  %49 = sub i16 %43, 0
  %50 = getelementptr inbounds i16, ptr %13, i16 %49
  store i16 %48, ptr %50, !tbaa !2
  br label %b9

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b15:
  %51 = load i16, ptr %4, !tbaa !2
  %52 = load i16, ptr %3, !tbaa !2
  %53 = icmp slt i16 %51, %52
  br i1 %53, label %b16, label %b18

b16:
  %54 = load i16, ptr %4, !tbaa !2
  %55 = add i16 %54, 1
  call cc 1005 addrspace(1) void @N$PI2(i16 %55)
  %56 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %56)
  %57 = load i16, ptr %4, !tbaa !2
  %58 = icmp ult i16 %57, 6
  br i1 %58, label %b19, label %b20

b17:
  %59 = load i16, ptr %4, !tbaa !2
  %60 = add nsw i16 %59, 1
  store i16 %60, ptr %4, !tbaa !2
  br label %b15

b18:
  store i16 7, ptr %2, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 100, ptr %61, !tbaa !2
  br label %b21

b19:
  %62 = sub i16 %57, 0
  %63 = getelementptr inbounds i16, ptr %13, i16 %62
  %64 = load i16, ptr %63, !tbaa !2
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 2, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PU2(i16 %64)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b17

b20:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b21:
  br label %b22

b22:
  %65 = addrspacecast ptr %2 to ptr addrspace(1)
  %66 = call cc 1005 addrspace(1) i32 @Rolls.next(ptr addrspace(1) %65)
  %67 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %66, ptr addrspace(1) %67, !tbaa !2
  %68 = load i8, ptr %1, !tbaa !2, !range !5
  %69 = icmp eq i8 %68, 0
  br i1 %69, label %b26, label %b25

b23:
  ret i16 0

b24:
  br label %b21

b25:
  br label %b23

b26:
  %70 = getelementptr inbounds i8, ptr %1, i16 2
  %71 = load i16, ptr %70, !tbaa !2
  %72 = getelementptr inbounds i8, ptr %1, i16 2
  %73 = load i16, ptr %72, !tbaa !2
  store i16 %73, ptr %0, !tbaa !2
  %74 = load i16, ptr %0, !tbaa !2
  %75 = icmp eq i16 %74, 6
  br i1 %75, label %b27, label %b28

b27:
  %76 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %76)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b23

b28:
  br label %b29

b29:
  br label %b24
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
