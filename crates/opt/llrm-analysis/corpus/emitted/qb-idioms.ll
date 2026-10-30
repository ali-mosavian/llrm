target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [16 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [320 x i8] zeroinitializer, align 2
@"A%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"A%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"A%", [6 x i8] c"\02\00\A0\00\00\00" }>
@"B%" = internal global [320 x i8] zeroinitializer, align 2
@"B%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"B%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"B%", [6 x i8] c"\02\00\A0\00\00\00" }>
@"C%" = internal global [320 x i8] zeroinitializer, align 2
@"C%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"C%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"C%", [6 x i8] c"\02\00\A0\00\00\00" }>
@"PAL%" = internal global [128 x i8] zeroinitializer, align 2
@"PAL%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"PAL%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"PAL%", [6 x i8] c"\02\00@\00\00\00" }>
@"L&" = internal global [160 x i8] zeroinitializer, align 2
@L$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"L&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"L&", [6 x i8] c"\04\00(\00\00\00" }>
@"M&" = internal global [160 x i8] zeroinitializer, align 2
@M$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"M&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"M&", [6 x i8] c"\04\00(\00\00\00" }>
@$qb$readData = internal constant [22 x i8] c" 0, 1, 2, 17, 64, 100\00"
@"T%" = internal global [2 x i8] zeroinitializer
@"N%" = internal global [2 x i8] zeroinitializer
@$arguments$17 = internal global [16 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  store i16 0, ptr @"T%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"T%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"T%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = addrspacecast ptr @"N%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %10)
  call cc 1000 addrspace(1) void @INIT()
  %11 = load i16, ptr @"N%", !tbaa !2
  %12 = sext i16 %11 to i32
  %13 = sdiv i32 %12, 10
  %14 = trunc i32 %13 to i16
  store i16 %14, ptr @$arguments$17, !tbaa !2
  call cc 1000 addrspace(1) void @CLEARROWS(ptr @$arguments$17)
  %15 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %16 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %15)
  %17 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %16)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %17)
  store i16 0, ptr @"I%", !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 4
  store i16 63, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %19, !tbaa !2
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b7:
  %20 = getelementptr i8, ptr @$data, i16 6
  %21 = load i16, ptr %20, !tbaa !2
  %22 = icmp sge i16 %21, 0
  br i1 %22, label %b8, label %b9

b8:
  %23 = load i16, ptr @"I%", !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 4
  %25 = load i16, ptr %24, !tbaa !2
  %26 = icmp sle i16 %23, %25
  br i1 %26, label %b10, label %b11

b9:
  %27 = load i16, ptr @"I%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 4
  %29 = load i16, ptr %28, !tbaa !2
  %30 = icmp sge i16 %27, %29
  br i1 %30, label %b10, label %b11

b10:
  %31 = load i16, ptr @"I%", !tbaa !2
  %32 = sub i16 %31, 0
  %33 = getelementptr inbounds i16, ptr @"PAL%", i16 %32
  store i16 -1, ptr %33, !tbaa !2
  %34 = load i16, ptr @"I%", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 6
  %36 = load i16, ptr %35, !tbaa !2
  %37 = add nsw i16 %34, %36
  store i16 %37, ptr @"I%", !tbaa !2
  br label %b7

b11:
  %38 = getelementptr i8, ptr @$arguments$17, i16 2
  store i16 64, ptr %38, !tbaa !2
  %39 = getelementptr i8, ptr @$arguments$17, i16 2
  %40 = call cc 1000 addrspace(1) i16 @LIM(ptr @"N%", ptr %39)
  store i16 %40, ptr @"K%", !tbaa !2
  %41 = load i16, ptr @"N%", !tbaa !2
  %42 = and i16 %41, 255
  %43 = mul i16 %42, 256
  %44 = or i16 %43, 52
  %45 = getelementptr i8, ptr @$arguments$17, i16 4
  store i16 %44, ptr %45, !tbaa !2
  %46 = getelementptr i8, ptr @$arguments$17, i16 4
  call cc 1000 addrspace(1) void @SETPAL(ptr @"K%", ptr %46)
  %47 = call cc 1000 addrspace(1) i32 @"SUMPAL&"()
  %48 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %47)
  %49 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %48)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %49)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @DOCOPY(ptr @"N%")
  %50 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %51 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %50)
  %52 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %51)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %52)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @SCROLLUP(ptr @"N%")
  %53 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %54 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %53)
  %55 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %54)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %55)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @SCROLLDOWN(ptr @"N%")
  %56 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %57 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %56)
  %58 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %57)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %58)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @SMEAR(ptr @"N%")
  %59 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %60 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %59)
  %61 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %60)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %61)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @CHANGED(ptr @"N%")
  %62 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %63 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %62)
  %64 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %63)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %64)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @TWOSTORES(ptr @"N%")
  %65 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %66 = call cc 1000 addrspace(1) i32 @"SUMC&"()
  %67 = add i32 %65, %66
  %68 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %67)
  %69 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %68)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %69)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @STRIDES(ptr @"N%")
  %70 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %71 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %70)
  %72 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %71)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %72)
  call cc 1000 addrspace(1) void @INIT()
  call cc 1000 addrspace(1) void @WRITTEN(ptr @"N%")
  %73 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %74 = call cc 1000 addrspace(1) i32 @"SUMB&"()
  %75 = add i32 %73, %74
  %76 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %75)
  %77 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %76)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %77)
  call cc 1000 addrspace(1) void @INIT()
  %78 = load i16, ptr @"N%", !tbaa !2
  %79 = sext i16 %78 to i32
  %80 = sdiv i32 %79, 10
  %81 = trunc i32 %80 to i16
  %82 = getelementptr i8, ptr @$arguments$17, i16 6
  store i16 %81, ptr %82, !tbaa !2
  %83 = getelementptr i8, ptr @$arguments$17, i16 6
  call cc 1000 addrspace(1) void @BLK(ptr %83)
  %84 = call cc 1000 addrspace(1) i32 @"SUMA&"()
  %85 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %84)
  %86 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %85)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %86)
  store i16 0, ptr @"I%", !tbaa !2
  %87 = getelementptr i8, ptr @$data, i16 8
  store i16 39, ptr %87, !tbaa !2
  %88 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %88, !tbaa !2
  br label %b12

b12:
  %89 = getelementptr i8, ptr @$data, i16 10
  %90 = load i16, ptr %89, !tbaa !2
  %91 = icmp sge i16 %90, 0
  br i1 %91, label %b13, label %b14

b13:
  %92 = load i16, ptr @"I%", !tbaa !2
  %93 = getelementptr i8, ptr @$data, i16 8
  %94 = load i16, ptr %93, !tbaa !2
  %95 = icmp sle i16 %92, %94
  br i1 %95, label %b15, label %b16

b14:
  %96 = load i16, ptr @"I%", !tbaa !2
  %97 = getelementptr i8, ptr @$data, i16 8
  %98 = load i16, ptr %97, !tbaa !2
  %99 = icmp sge i16 %96, %98
  br i1 %99, label %b15, label %b16

b15:
  %100 = load i16, ptr @"I%", !tbaa !2
  %101 = load i16, ptr @"I%", !tbaa !2
  %102 = add i16 4096, %101
  %103 = sext i16 %102 to i32
  %104 = sub i16 %100, 0
  %105 = getelementptr inbounds i32, ptr @"L&", i16 %104
  store i32 %103, ptr %105, !tbaa !2
  %106 = load i16, ptr @"I%", !tbaa !2
  %107 = load i16, ptr @"I%", !tbaa !2
  %108 = mul i16 3, %107
  %109 = add i16 8192, %108
  %110 = sext i16 %109 to i32
  %111 = sub i16 %106, 0
  %112 = getelementptr inbounds i32, ptr @"M&", i16 %111
  store i32 %110, ptr %112, !tbaa !2
  %113 = load i16, ptr @"I%", !tbaa !2
  %114 = getelementptr i8, ptr @$data, i16 10
  %115 = load i16, ptr %114, !tbaa !2
  %116 = add nsw i16 %113, %115
  store i16 %116, ptr @"I%", !tbaa !2
  br label %b12

b16:
  %117 = getelementptr i8, ptr @$arguments$17, i16 8
  store i16 40, ptr %117, !tbaa !2
  %118 = getelementptr i8, ptr @$arguments$17, i16 8
  %119 = call cc 1000 addrspace(1) i16 @LIM(ptr @"N%", ptr %118)
  store i16 %119, ptr @"K%", !tbaa !2
  call cc 1000 addrspace(1) void @COPYLONG(ptr @"K%")
  %120 = call cc 1000 addrspace(1) i32 @"SUML&"()
  %121 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %120)
  %122 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %121)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %122)
  store i16 0, ptr @"I%", !tbaa !2
  %123 = getelementptr i8, ptr @$data, i16 12
  store i16 39, ptr %123, !tbaa !2
  %124 = getelementptr i8, ptr @$data, i16 14
  store i16 1, ptr %124, !tbaa !2
  br label %b17

b17:
  %125 = getelementptr i8, ptr @$data, i16 14
  %126 = load i16, ptr %125, !tbaa !2
  %127 = icmp sge i16 %126, 0
  br i1 %127, label %b18, label %b19

b18:
  %128 = load i16, ptr @"I%", !tbaa !2
  %129 = getelementptr i8, ptr @$data, i16 12
  %130 = load i16, ptr %129, !tbaa !2
  %131 = icmp sle i16 %128, %130
  br i1 %131, label %b20, label %b21

b19:
  %132 = load i16, ptr @"I%", !tbaa !2
  %133 = getelementptr i8, ptr @$data, i16 12
  %134 = load i16, ptr %133, !tbaa !2
  %135 = icmp sge i16 %132, %134
  br i1 %135, label %b20, label %b21

b20:
  %136 = load i16, ptr @"I%", !tbaa !2
  %137 = sub i16 %136, 0
  %138 = getelementptr inbounds i32, ptr @"L&", i16 %137
  store i32 -1, ptr %138, !tbaa !2
  %139 = load i16, ptr @"I%", !tbaa !2
  %140 = getelementptr i8, ptr @$data, i16 14
  %141 = load i16, ptr %140, !tbaa !2
  %142 = add nsw i16 %139, %141
  store i16 %142, ptr @"I%", !tbaa !2
  br label %b17

b21:
  %143 = getelementptr i8, ptr @$arguments$17, i16 10
  store i16 40, ptr %143, !tbaa !2
  %144 = getelementptr i8, ptr @$arguments$17, i16 10
  %145 = call cc 1000 addrspace(1) i16 @LIM(ptr @"N%", ptr %144)
  store i16 %145, ptr @"K%", !tbaa !2
  %146 = load i16, ptr @"N%", !tbaa !2
  %147 = sext i16 %146 to i32
  %148 = add i32 1193046, %147
  %149 = getelementptr i8, ptr @$arguments$17, i16 12
  store i32 %148, ptr %149, !tbaa !2
  %150 = getelementptr i8, ptr @$arguments$17, i16 12
  call cc 1000 addrspace(1) void @FILLLONG(ptr @"K%", ptr %150)
  %151 = call cc 1000 addrspace(1) i32 @"SUML&"()
  %152 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %151)
  %153 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %152)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %153)
  %154 = load i16, ptr @"T%", !tbaa !2
  %155 = getelementptr i8, ptr @$data, i16 2
  %156 = load i16, ptr %155, !tbaa !2
  %157 = add nsw i16 %154, %156
  store i16 %157, ptr @"T%", !tbaa !2
  br label %b2
}

define cc 1000 i16 @LIM(ptr dereferenceable(2) %0, ptr dereferenceable(2) %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = load i16, ptr %0
  %4 = load i16, ptr %1
  %5 = icmp slt i16 %3, %4
  br i1 %5, label %b2, label %b3

b2:
  %6 = load i16, ptr %0
  store i16 %6, ptr %2, !tbaa !2
  br label %b4

b3:
  %7 = load i16, ptr %1
  store i16 %7, ptr %2, !tbaa !2
  br label %b4

b4:
  br label %b5

b5:
  %8 = load i16, ptr %2, !tbaa !2
  ret i16 %8
}

define cc 1000 void @INIT() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
  store i16 159, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %3 = load i16, ptr %0, !tbaa !2
  %4 = icmp sge i16 %3, 0
  br i1 %4, label %b3, label %b4

b3:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sle i16 %5, %6
  br i1 %7, label %b5, label %b6

b4:
  %8 = load i16, ptr %2, !tbaa !2
  %9 = load i16, ptr %1, !tbaa !2
  %10 = icmp sge i16 %8, %9
  br i1 %10, label %b5, label %b6

b5:
  %11 = load i16, ptr %2, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = add i16 %12, 1000
  %14 = sub i16 %11, 0
  %15 = getelementptr inbounds i16, ptr @"A%", i16 %14
  store i16 %13, ptr %15, !tbaa !2
  %16 = load i16, ptr %2, !tbaa !2
  %17 = load i16, ptr %2, !tbaa !2
  %18 = mul i16 %17, 3
  %19 = add i16 %18, 1
  %20 = sub i16 %16, 0
  %21 = getelementptr inbounds i16, ptr @"B%", i16 %20
  store i16 %19, ptr %21, !tbaa !2
  %22 = load i16, ptr %2, !tbaa !2
  %23 = load i16, ptr %2, !tbaa !2
  %24 = mul i16 %23, 5
  %25 = add i16 %24, 2
  %26 = sub i16 %22, 0
  %27 = getelementptr inbounds i16, ptr @"C%", i16 %26
  store i16 %25, ptr %27, !tbaa !2
  %28 = load i16, ptr %2, !tbaa !2
  %29 = load i16, ptr %0, !tbaa !2
  %30 = add nsw i16 %28, %29
  store i16 %30, ptr %2, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 i32 @"SUMA&"() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  store i32 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 159, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %0, !tbaa !2
  %6 = icmp sge i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp sle i16 %7, %8
  br i1 %9, label %b5, label %b6

b4:
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr %1, !tbaa !2
  %12 = icmp sge i16 %10, %11
  br i1 %12, label %b5, label %b6

b5:
  %13 = load i32, ptr %3, !tbaa !2
  %14 = load i16, ptr %2, !tbaa !2
  %15 = add i16 %14, 1
  %16 = sext i16 %15 to i32
  %17 = load i16, ptr %2, !tbaa !2
  %18 = sub i16 %17, 0
  %19 = getelementptr inbounds i16, ptr @"A%", i16 %18
  %20 = load i16, ptr %19, !tbaa !2
  %21 = sext i16 %20 to i32
  %22 = mul i32 %16, %21
  %23 = add i32 %13, %22
  store i32 %23, ptr %3, !tbaa !2
  %24 = load i16, ptr %2, !tbaa !2
  %25 = load i16, ptr %0, !tbaa !2
  %26 = add nsw i16 %24, %25
  store i16 %26, ptr %2, !tbaa !2
  br label %b2

b6:
  %27 = load i32, ptr %3, !tbaa !2
  store i32 %27, ptr %4, !tbaa !2
  br label %b7

b7:
  %28 = load i32, ptr %4, !tbaa !2
  ret i32 %28
}

define cc 1000 i32 @"SUMB&"() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  store i32 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 159, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %0, !tbaa !2
  %6 = icmp sge i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp sle i16 %7, %8
  br i1 %9, label %b5, label %b6

b4:
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr %1, !tbaa !2
  %12 = icmp sge i16 %10, %11
  br i1 %12, label %b5, label %b6

b5:
  %13 = load i32, ptr %3, !tbaa !2
  %14 = load i16, ptr %2, !tbaa !2
  %15 = add i16 %14, 1
  %16 = sext i16 %15 to i32
  %17 = load i16, ptr %2, !tbaa !2
  %18 = sub i16 %17, 0
  %19 = getelementptr inbounds i16, ptr @"B%", i16 %18
  %20 = load i16, ptr %19, !tbaa !2
  %21 = sext i16 %20 to i32
  %22 = mul i32 %16, %21
  %23 = add i32 %13, %22
  store i32 %23, ptr %3, !tbaa !2
  %24 = load i16, ptr %2, !tbaa !2
  %25 = load i16, ptr %0, !tbaa !2
  %26 = add nsw i16 %24, %25
  store i16 %26, ptr %2, !tbaa !2
  br label %b2

b6:
  %27 = load i32, ptr %3, !tbaa !2
  store i32 %27, ptr %4, !tbaa !2
  br label %b7

b7:
  %28 = load i32, ptr %4, !tbaa !2
  ret i32 %28
}

define cc 1000 i32 @"SUMC&"() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  store i32 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 159, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %0, !tbaa !2
  %6 = icmp sge i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp sle i16 %7, %8
  br i1 %9, label %b5, label %b6

b4:
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr %1, !tbaa !2
  %12 = icmp sge i16 %10, %11
  br i1 %12, label %b5, label %b6

b5:
  %13 = load i32, ptr %3, !tbaa !2
  %14 = load i16, ptr %2, !tbaa !2
  %15 = add i16 %14, 1
  %16 = sext i16 %15 to i32
  %17 = load i16, ptr %2, !tbaa !2
  %18 = sub i16 %17, 0
  %19 = getelementptr inbounds i16, ptr @"C%", i16 %18
  %20 = load i16, ptr %19, !tbaa !2
  %21 = sext i16 %20 to i32
  %22 = mul i32 %16, %21
  %23 = add i32 %13, %22
  store i32 %23, ptr %3, !tbaa !2
  %24 = load i16, ptr %2, !tbaa !2
  %25 = load i16, ptr %0, !tbaa !2
  %26 = add nsw i16 %24, %25
  store i16 %26, ptr %2, !tbaa !2
  br label %b2

b6:
  %27 = load i32, ptr %3, !tbaa !2
  store i32 %27, ptr %4, !tbaa !2
  br label %b7

b7:
  %28 = load i32, ptr %4, !tbaa !2
  ret i32 %28
}

define cc 1000 i32 @"SUMPAL&"() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  store i32 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 63, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %0, !tbaa !2
  %6 = icmp sge i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp sle i16 %7, %8
  br i1 %9, label %b5, label %b6

b4:
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr %1, !tbaa !2
  %12 = icmp sge i16 %10, %11
  br i1 %12, label %b5, label %b6

b5:
  %13 = load i32, ptr %3, !tbaa !2
  %14 = load i16, ptr %2, !tbaa !2
  %15 = add i16 %14, 1
  %16 = sext i16 %15 to i32
  %17 = load i16, ptr %2, !tbaa !2
  %18 = sub i16 %17, 0
  %19 = getelementptr inbounds i16, ptr @"PAL%", i16 %18
  %20 = load i16, ptr %19, !tbaa !2
  %21 = sext i16 %20 to i32
  %22 = mul i32 %16, %21
  %23 = add i32 %13, %22
  store i32 %23, ptr %3, !tbaa !2
  %24 = load i16, ptr %2, !tbaa !2
  %25 = load i16, ptr %0, !tbaa !2
  %26 = add nsw i16 %24, %25
  store i16 %26, ptr %2, !tbaa !2
  br label %b2

b6:
  %27 = load i32, ptr %3, !tbaa !2
  store i32 %27, ptr %4, !tbaa !2
  br label %b7

b7:
  %28 = load i32, ptr %4, !tbaa !2
  ret i32 %28
}

define cc 1000 i32 @"SUML&"() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  store i32 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 39, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %0, !tbaa !2
  %6 = icmp sge i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp sle i16 %7, %8
  br i1 %9, label %b5, label %b6

b4:
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr %1, !tbaa !2
  %12 = icmp sge i16 %10, %11
  br i1 %12, label %b5, label %b6

b5:
  %13 = load i32, ptr %3, !tbaa !2
  %14 = load i16, ptr %2, !tbaa !2
  %15 = add i16 %14, 1
  %16 = sext i16 %15 to i32
  %17 = load i16, ptr %2, !tbaa !2
  %18 = sub i16 %17, 0
  %19 = getelementptr inbounds i32, ptr @"L&", i16 %18
  %20 = load i32, ptr %19, !tbaa !2
  %21 = mul i32 %16, %20
  %22 = add i32 %13, %21
  store i32 %22, ptr %3, !tbaa !2
  %23 = load i16, ptr %2, !tbaa !2
  %24 = load i16, ptr %0, !tbaa !2
  %25 = add nsw i16 %23, %24
  store i16 %25, ptr %2, !tbaa !2
  br label %b2

b6:
  %26 = load i32, ptr %3, !tbaa !2
  store i32 %26, ptr %4, !tbaa !2
  br label %b7

b7:
  %27 = load i32, ptr %4, !tbaa !2
  ret i32 %27
}

define cc 1000 void @CLEARROWS(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  %7 = load i16, ptr %0
  %8 = sub i16 %7, 1
  store i16 %8, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = icmp sge i16 %9, 0
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr %6, !tbaa !2
  %12 = load i16, ptr %5, !tbaa !2
  %13 = icmp sle i16 %11, %12
  br i1 %13, label %b5, label %b6

b4:
  %14 = load i16, ptr %6, !tbaa !2
  %15 = load i16, ptr %5, !tbaa !2
  %16 = icmp sge i16 %14, %15
  br i1 %16, label %b5, label %b6

b5:
  store i16 0, ptr %3, !tbaa !2
  store i16 15, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %17 = load i16, ptr %1, !tbaa !2
  %18 = icmp sge i16 %17, 0
  br i1 %18, label %b8, label %b9

b8:
  %19 = load i16, ptr %3, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = icmp sle i16 %19, %20
  br i1 %21, label %b10, label %b11

b9:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %2, !tbaa !2
  %24 = icmp sge i16 %22, %23
  br i1 %24, label %b10, label %b11

b10:
  %25 = load i16, ptr %6, !tbaa !2
  %26 = mul i16 %25, 16
  %27 = load i16, ptr %3, !tbaa !2
  %28 = add i16 %26, %27
  %29 = sub i16 %28, 0
  %30 = getelementptr inbounds i16, ptr @"A%", i16 %29
  store i16 4660, ptr %30, !tbaa !2
  %31 = load i16, ptr %3, !tbaa !2
  %32 = load i16, ptr %1, !tbaa !2
  %33 = add nsw i16 %31, %32
  store i16 %33, ptr %3, !tbaa !2
  br label %b7

b11:
  %34 = load i16, ptr %6, !tbaa !2
  %35 = load i16, ptr %4, !tbaa !2
  %36 = add nsw i16 %34, %35
  store i16 %36, ptr %6, !tbaa !2
  br label %b2
}

define cc 1000 void @SETPAL(ptr dereferenceable(2) %0, ptr dereferenceable(2) %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %4, !tbaa !2
  %5 = load i16, ptr %0
  %6 = sub i16 %5, 1
  store i16 %6, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %4, !tbaa !2
  %13 = load i16, ptr %3, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = load i16, ptr %1
  %17 = sub i16 %15, 0
  %18 = getelementptr inbounds i16, ptr @"PAL%", i16 %17
  store i16 %16, ptr %18, !tbaa !2
  %19 = load i16, ptr %4, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = add nsw i16 %19, %20
  store i16 %21, ptr %4, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @DOCOPY(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = sub i16 %15, 0
  %17 = getelementptr inbounds i16, ptr @"B%", i16 %16
  %18 = load i16, ptr %17, !tbaa !2
  %19 = sub i16 %14, 0
  %20 = getelementptr inbounds i16, ptr @"A%", i16 %19
  store i16 %18, ptr %20, !tbaa !2
  %21 = load i16, ptr %3, !tbaa !2
  %22 = load i16, ptr %1, !tbaa !2
  %23 = add nsw i16 %21, %22
  store i16 %23, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @SCROLLUP(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = add i16 %15, 16
  %17 = sub i16 %16, 0
  %18 = getelementptr inbounds i16, ptr @"A%", i16 %17
  %19 = load i16, ptr %18, !tbaa !2
  %20 = sub i16 %14, 0
  %21 = getelementptr inbounds i16, ptr @"A%", i16 %20
  store i16 %19, ptr %21, !tbaa !2
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %1, !tbaa !2
  %24 = add nsw i16 %22, %23
  store i16 %24, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @SCROLLDOWN(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 -1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = add i16 %14, 16
  %16 = load i16, ptr %3, !tbaa !2
  %17 = sub i16 %16, 0
  %18 = getelementptr inbounds i16, ptr @"A%", i16 %17
  %19 = load i16, ptr %18, !tbaa !2
  %20 = sub i16 %15, 0
  %21 = getelementptr inbounds i16, ptr @"A%", i16 %20
  store i16 %19, ptr %21, !tbaa !2
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %1, !tbaa !2
  %24 = add nsw i16 %22, %23
  store i16 %24, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @SMEAR(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = add i16 %14, 1
  %16 = load i16, ptr %3, !tbaa !2
  %17 = sub i16 %16, 0
  %18 = getelementptr inbounds i16, ptr @"A%", i16 %17
  %19 = load i16, ptr %18, !tbaa !2
  %20 = sub i16 %15, 0
  %21 = getelementptr inbounds i16, ptr @"A%", i16 %20
  store i16 %19, ptr %21, !tbaa !2
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %1, !tbaa !2
  %24 = add nsw i16 %22, %23
  store i16 %24, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @CHANGED(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = sub i16 %15, 0
  %17 = getelementptr inbounds i16, ptr @"B%", i16 %16
  %18 = load i16, ptr %17, !tbaa !2
  %19 = add i16 %18, 1
  %20 = sub i16 %14, 0
  %21 = getelementptr inbounds i16, ptr @"A%", i16 %20
  store i16 %19, ptr %21, !tbaa !2
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %1, !tbaa !2
  %24 = add nsw i16 %22, %23
  store i16 %24, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @TWOSTORES(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = sub i16 %15, 0
  %17 = getelementptr inbounds i16, ptr @"B%", i16 %16
  %18 = load i16, ptr %17, !tbaa !2
  %19 = sub i16 %14, 0
  %20 = getelementptr inbounds i16, ptr @"A%", i16 %19
  store i16 %18, ptr %20, !tbaa !2
  %21 = load i16, ptr %3, !tbaa !2
  %22 = sub i16 %21, 0
  %23 = getelementptr inbounds i16, ptr @"C%", i16 %22
  store i16 0, ptr %23, !tbaa !2
  %24 = load i16, ptr %3, !tbaa !2
  %25 = load i16, ptr %1, !tbaa !2
  %26 = add nsw i16 %24, %25
  store i16 %26, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @STRIDES(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sext i16 %4 to i32
  %6 = sdiv i32 %5, 2
  %7 = trunc i32 %6 to i16
  %8 = sub i16 %7, 1
  store i16 %8, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %9 = load i16, ptr %1, !tbaa !2
  %10 = icmp sge i16 %9, 0
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sle i16 %11, %12
  br i1 %13, label %b5, label %b6

b4:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = load i16, ptr %2, !tbaa !2
  %16 = icmp sge i16 %14, %15
  br i1 %16, label %b5, label %b6

b5:
  %17 = load i16, ptr %3, !tbaa !2
  %18 = mul i16 2, %17
  %19 = load i16, ptr %3, !tbaa !2
  %20 = sub i16 %19, 0
  %21 = getelementptr inbounds i16, ptr @"B%", i16 %20
  %22 = load i16, ptr %21, !tbaa !2
  %23 = sub i16 %18, 0
  %24 = getelementptr inbounds i16, ptr @"A%", i16 %23
  store i16 %22, ptr %24, !tbaa !2
  %25 = load i16, ptr %3, !tbaa !2
  %26 = load i16, ptr %1, !tbaa !2
  %27 = add nsw i16 %25, %26
  store i16 %27, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @WRITTEN(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = sub i16 %14, 0
  %16 = getelementptr inbounds i16, ptr @"B%", i16 %15
  store i16 0, ptr %16, !tbaa !2
  %17 = load i16, ptr %3, !tbaa !2
  %18 = load i16, ptr %3, !tbaa !2
  %19 = sub i16 %18, 0
  %20 = getelementptr inbounds i16, ptr @"B%", i16 %19
  %21 = load i16, ptr %20, !tbaa !2
  %22 = sub i16 %17, 0
  %23 = getelementptr inbounds i16, ptr @"A%", i16 %22
  store i16 %21, ptr %23, !tbaa !2
  %24 = load i16, ptr %3, !tbaa !2
  %25 = load i16, ptr %1, !tbaa !2
  %26 = add nsw i16 %24, %25
  store i16 %26, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @BLK(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  store i16 3, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %6, !tbaa !2
  %10 = load i16, ptr %5, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %6, !tbaa !2
  %13 = load i16, ptr %5, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  store i16 0, ptr %3, !tbaa !2
  %15 = load i16, ptr %0
  %16 = sub i16 %15, 1
  store i16 %16, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %17 = load i16, ptr %1, !tbaa !2
  %18 = icmp sge i16 %17, 0
  br i1 %18, label %b8, label %b9

b8:
  %19 = load i16, ptr %3, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = icmp sle i16 %19, %20
  br i1 %21, label %b10, label %b11

b9:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %2, !tbaa !2
  %24 = icmp sge i16 %22, %23
  br i1 %24, label %b10, label %b11

b10:
  %25 = load i16, ptr %6, !tbaa !2
  %26 = mul i16 %25, 16
  %27 = load i16, ptr %3, !tbaa !2
  %28 = add i16 %26, %27
  %29 = load i16, ptr %6, !tbaa !2
  %30 = mul i16 %29, 20
  %31 = load i16, ptr %3, !tbaa !2
  %32 = add i16 %30, %31
  %33 = sub i16 %32, 0
  %34 = getelementptr inbounds i16, ptr @"B%", i16 %33
  %35 = load i16, ptr %34, !tbaa !2
  %36 = sub i16 %28, 0
  %37 = getelementptr inbounds i16, ptr @"A%", i16 %36
  store i16 %35, ptr %37, !tbaa !2
  %38 = load i16, ptr %3, !tbaa !2
  %39 = load i16, ptr %1, !tbaa !2
  %40 = add nsw i16 %38, %39
  store i16 %40, ptr %3, !tbaa !2
  br label %b7

b11:
  %41 = load i16, ptr %6, !tbaa !2
  %42 = load i16, ptr %4, !tbaa !2
  %43 = add nsw i16 %41, %42
  store i16 %43, ptr %6, !tbaa !2
  br label %b2
}

define cc 1000 void @COPYLONG(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sge i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp sle i16 %8, %9
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp sge i16 %11, %12
  br i1 %13, label %b5, label %b6

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = sub i16 %15, 0
  %17 = getelementptr inbounds i32, ptr @"M&", i16 %16
  %18 = load i32, ptr %17, !tbaa !2
  %19 = sub i16 %14, 0
  %20 = getelementptr inbounds i32, ptr @"L&", i16 %19
  store i32 %18, ptr %20, !tbaa !2
  %21 = load i16, ptr %3, !tbaa !2
  %22 = load i16, ptr %1, !tbaa !2
  %23 = add nsw i16 %21, %22
  store i16 %23, ptr %3, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @FILLLONG(ptr dereferenceable(2) %0, ptr dereferenceable(4) %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %4, !tbaa !2
  %5 = load i16, ptr %0
  %6 = sub i16 %5, 1
  store i16 %6, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %4, !tbaa !2
  %13 = load i16, ptr %3, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = load i32, ptr %1
  %17 = sub i16 %15, 0
  %18 = getelementptr inbounds i32, ptr @"L&", i16 %17
  store i32 %16, ptr %18, !tbaa !2
  %19 = load i16, ptr %4, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = add nsw i16 %19, %20
  store i16 %21, ptr %4, !tbaa !2
  br label %b2

b6:
  ret void
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI4(i32) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
