target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [48 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"C&" = internal global [4 x i8] zeroinitializer
@"D&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"ONE=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"CONST=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string5, i16 4), [8 x i8] c"CONST2=\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string6, i16 4), [8 x i8] c"MODMOD=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string7, i16 4), [8 x i8] c"DIVDIV=\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string8, i16 4), [8 x i8] c"NEGMOD=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string9, i16 4), [8 x i8] c"NEGDIV=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 1073741831, ptr @"A&", !tbaa !2
  store i32 39678839, ptr @"B&", !tbaa !2
  store i32 -1049330653, ptr @"C&", !tbaa !2
  store i32 100003, ptr @"D&", !tbaa !2
  %0 = load i32, ptr @"A&", !tbaa !2
  %1 = load i32, ptr @"B&", !tbaa !2
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b3:
  %3 = icmp eq i32 %1, -1
  br i1 %3, label %b4, label %b5

b4:
  store i32 0, ptr @$data, !tbaa !2
  br label %b6

b5:
  %4 = srem i32 %0, %1
  store i32 %4, ptr @$data, !tbaa !2
  br label %b6

b6:
  %5 = load i32, ptr @$data, !tbaa !2
  %6 = load i32, ptr @"C&", !tbaa !2
  %7 = load i32, ptr @"C&", !tbaa !2
  %8 = xor i32 %6, %7
  %9 = and i32 %8, 2147483647
  %10 = or i32 %9, 1
  %11 = icmp eq i32 %10, 0
  br i1 %11, label %b7, label %b8

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b8:
  %12 = icmp eq i32 %10, -1
  br i1 %12, label %b9, label %b10

b9:
  %13 = getelementptr i8, ptr @$data, i16 4
  store i32 0, ptr %13, !tbaa !2
  br label %b11

b10:
  %14 = srem i32 %5, %10
  %15 = getelementptr i8, ptr @$data, i16 4
  store i32 %14, ptr %15, !tbaa !2
  br label %b11

b11:
  %16 = getelementptr i8, ptr @$data, i16 4
  %17 = load i32, ptr %16, !tbaa !2
  store i32 %17, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %18 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %18)
  %19 = load i32, ptr @"A&", !tbaa !2
  %20 = srem i32 %19, 39678839
  %21 = load i32, ptr @"C&", !tbaa !2
  %22 = load i32, ptr @"C&", !tbaa !2
  %23 = xor i32 %21, %22
  %24 = and i32 %23, 2147483647
  %25 = or i32 %24, 1
  %26 = icmp eq i32 %25, 0
  br i1 %26, label %b12, label %b13

b12:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b13:
  %27 = icmp eq i32 %25, -1
  br i1 %27, label %b14, label %b15

b14:
  %28 = getelementptr i8, ptr @$data, i16 8
  store i32 0, ptr %28, !tbaa !2
  br label %b16

b15:
  %29 = srem i32 %20, %25
  %30 = getelementptr i8, ptr @$data, i16 8
  store i32 %29, ptr %30, !tbaa !2
  br label %b16

b16:
  %31 = getelementptr i8, ptr @$data, i16 8
  %32 = load i32, ptr %31, !tbaa !2
  store i32 %32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %33 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %33)
  %34 = load i32, ptr @"A&", !tbaa !2
  %35 = srem i32 %34, 39678839
  %36 = load i32, ptr @"D&", !tbaa !2
  %37 = and i32 %36, 2147483647
  %38 = or i32 %37, 1
  %39 = icmp eq i32 %38, 0
  br i1 %39, label %b17, label %b18

b17:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b18:
  %40 = icmp eq i32 %38, -1
  br i1 %40, label %b19, label %b20

b19:
  %41 = getelementptr i8, ptr @$data, i16 12
  store i32 0, ptr %41, !tbaa !2
  br label %b21

b20:
  %42 = srem i32 %35, %38
  %43 = getelementptr i8, ptr @$data, i16 12
  store i32 %42, ptr %43, !tbaa !2
  br label %b21

b21:
  %44 = getelementptr i8, ptr @$data, i16 12
  %45 = load i32, ptr %44, !tbaa !2
  store i32 %45, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %46 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %46)
  %47 = load i32, ptr @"A&", !tbaa !2
  %48 = load i32, ptr @"B&", !tbaa !2
  %49 = icmp eq i32 %48, 0
  br i1 %49, label %b22, label %b23

b22:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b23:
  %50 = icmp eq i32 %48, -1
  br i1 %50, label %b24, label %b25

b24:
  %51 = getelementptr i8, ptr @$data, i16 16
  store i32 0, ptr %51, !tbaa !2
  br label %b26

b25:
  %52 = srem i32 %47, %48
  %53 = getelementptr i8, ptr @$data, i16 16
  store i32 %52, ptr %53, !tbaa !2
  br label %b26

b26:
  %54 = getelementptr i8, ptr @$data, i16 16
  %55 = load i32, ptr %54, !tbaa !2
  %56 = load i32, ptr @"D&", !tbaa !2
  %57 = and i32 %56, 2147483647
  %58 = or i32 %57, 1
  %59 = icmp eq i32 %58, 0
  br i1 %59, label %b27, label %b28

b27:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b28:
  %60 = icmp eq i32 %58, -1
  br i1 %60, label %b29, label %b30

b29:
  %61 = getelementptr i8, ptr @$data, i16 20
  store i32 0, ptr %61, !tbaa !2
  br label %b31

b30:
  %62 = srem i32 %55, %58
  %63 = getelementptr i8, ptr @$data, i16 20
  store i32 %62, ptr %63, !tbaa !2
  br label %b31

b31:
  %64 = getelementptr i8, ptr @$data, i16 20
  %65 = load i32, ptr %64, !tbaa !2
  store i32 %65, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %66 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %66)
  %67 = load i32, ptr @"A&", !tbaa !2
  %68 = load i32, ptr @"B&", !tbaa !2
  %69 = icmp eq i32 %68, 0
  br i1 %69, label %b32, label %b33

b32:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b33:
  %70 = icmp eq i32 %68, -1
  br i1 %70, label %b34, label %b35

b34:
  %71 = sub i32 0, %67
  %72 = getelementptr i8, ptr @$data, i16 24
  store i32 %71, ptr %72, !tbaa !2
  br label %b36

b35:
  %73 = sdiv i32 %67, %68
  %74 = getelementptr i8, ptr @$data, i16 24
  store i32 %73, ptr %74, !tbaa !2
  br label %b36

b36:
  %75 = getelementptr i8, ptr @$data, i16 24
  %76 = load i32, ptr %75, !tbaa !2
  %77 = load i32, ptr @"C&", !tbaa !2
  %78 = load i32, ptr @"C&", !tbaa !2
  %79 = xor i32 %77, %78
  %80 = and i32 %79, 32767
  %81 = or i32 %80, 3
  %82 = icmp eq i32 %81, 0
  br i1 %82, label %b37, label %b38

b37:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b38:
  %83 = icmp eq i32 %81, -1
  br i1 %83, label %b39, label %b40

b39:
  %84 = sub i32 0, %76
  %85 = getelementptr i8, ptr @$data, i16 28
  store i32 %84, ptr %85, !tbaa !2
  br label %b41

b40:
  %86 = sdiv i32 %76, %81
  %87 = getelementptr i8, ptr @$data, i16 28
  store i32 %86, ptr %87, !tbaa !2
  br label %b41

b41:
  %88 = getelementptr i8, ptr @$data, i16 28
  %89 = load i32, ptr %88, !tbaa !2
  store i32 %89, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %90 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %90)
  store i32 -1073741831, ptr @"A&", !tbaa !2
  %91 = load i32, ptr @"A&", !tbaa !2
  %92 = load i32, ptr @"B&", !tbaa !2
  %93 = icmp eq i32 %92, 0
  br i1 %93, label %b42, label %b43

b42:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b43:
  %94 = icmp eq i32 %92, -1
  br i1 %94, label %b44, label %b45

b44:
  %95 = getelementptr i8, ptr @$data, i16 32
  store i32 0, ptr %95, !tbaa !2
  br label %b46

b45:
  %96 = srem i32 %91, %92
  %97 = getelementptr i8, ptr @$data, i16 32
  store i32 %96, ptr %97, !tbaa !2
  br label %b46

b46:
  %98 = getelementptr i8, ptr @$data, i16 32
  %99 = load i32, ptr %98, !tbaa !2
  %100 = load i32, ptr @"D&", !tbaa !2
  %101 = and i32 %100, 2147483647
  %102 = or i32 %101, 1
  %103 = icmp eq i32 %102, 0
  br i1 %103, label %b47, label %b48

b47:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b48:
  %104 = icmp eq i32 %102, -1
  br i1 %104, label %b49, label %b50

b49:
  %105 = getelementptr i8, ptr @$data, i16 36
  store i32 0, ptr %105, !tbaa !2
  br label %b51

b50:
  %106 = srem i32 %99, %102
  %107 = getelementptr i8, ptr @$data, i16 36
  store i32 %106, ptr %107, !tbaa !2
  br label %b51

b51:
  %108 = getelementptr i8, ptr @$data, i16 36
  %109 = load i32, ptr %108, !tbaa !2
  store i32 %109, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %110 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %110)
  %111 = load i32, ptr @"A&", !tbaa !2
  %112 = load i32, ptr @"B&", !tbaa !2
  %113 = icmp eq i32 %112, 0
  br i1 %113, label %b52, label %b53

b52:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b53:
  %114 = icmp eq i32 %112, -1
  br i1 %114, label %b54, label %b55

b54:
  %115 = sub i32 0, %111
  %116 = getelementptr i8, ptr @$data, i16 40
  store i32 %115, ptr %116, !tbaa !2
  br label %b56

b55:
  %117 = sdiv i32 %111, %112
  %118 = getelementptr i8, ptr @$data, i16 40
  store i32 %117, ptr %118, !tbaa !2
  br label %b56

b56:
  %119 = getelementptr i8, ptr @$data, i16 40
  %120 = load i32, ptr %119, !tbaa !2
  %121 = load i32, ptr @"C&", !tbaa !2
  %122 = load i32, ptr @"C&", !tbaa !2
  %123 = xor i32 %121, %122
  %124 = and i32 %123, 32767
  %125 = or i32 %124, 3
  %126 = icmp eq i32 %125, 0
  br i1 %126, label %b57, label %b58

b57:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b58:
  %127 = icmp eq i32 %125, -1
  br i1 %127, label %b59, label %b60

b59:
  %128 = sub i32 0, %120
  %129 = getelementptr i8, ptr @$data, i16 44
  store i32 %128, ptr %129, !tbaa !2
  br label %b61

b60:
  %130 = sdiv i32 %120, %125
  %131 = getelementptr i8, ptr @$data, i16 44
  store i32 %130, ptr %131, !tbaa !2
  br label %b61

b61:
  %132 = getelementptr i8, ptr @$data, i16 44
  %133 = load i32, ptr %132, !tbaa !2
  store i32 %133, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  %134 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %134)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string10)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
