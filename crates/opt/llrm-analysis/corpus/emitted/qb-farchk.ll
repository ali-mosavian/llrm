target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [294 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\04\00\00\00\05\00\00\00\00\00\01\00\00\00\05\00\00\00\06\00\00\00\00\00\01\00\00\00\06\00\00\00\07\00\00\00\00\00\01\00\00\00\07\00\00\00\12\00\00\00\00\00\01\00\00\00\08\00\00\00\1C\00\00\00\00\00\01\00\00\00\09\00\00\005\00\00\00\00\00\01\00\00\00\0A\00\00\007\00\00\00\00\00\01\00\00\00\0B\00\00\009\00\00\00\00\00\01\00\00\00\0C\00\00\00D\00\00\00\00\00\01\00\00\00\0D\00\00\00P\00\00\00\00\00\01\00\00\00\0E\00\00\00[\00\00\00\00\00\01\00\00\00\0F\00\00\00\\\00\00\00\00\00\01\00\00\00\10\00\00\00i\00\00\00\00\00\01\00\00\00\11\00\00\00k\00\00\00\00\00\01\00\00\00\12\00\00\00v\00\00\00\00\00\01\00\00\00\13\00\00\00x\00\00\00\00\00\01\00\00\00\02\00\00\00y\00\00\00\00\00\01\00\00\00\15\00\00\00}\00\00\00\00\00\01\00\00\00\16\00\00\00~\00\00\00\00\00"
@"S%" = internal global [20 x i8] zeroinitializer, align 2
@"S%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"S%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"S%", i16 -2), [6 x i8] c"\02\00\0A\00\01\00" }>
@"D%" = internal global [46 x i8] zeroinitializer
@"N%" = internal global [2 x i8] zeroinitializer
@"I&" = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"ERR\00" }>
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 5, ptr @"N%", !tbaa !2
  br label %b3

b2:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %111 unwind label %landing

b3:
  %0 = load i16, ptr @"N%", !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 -2, i16 %0, i16 0, i16 3, i16 2, i16 258, ptr @"D%")
          to label %115 unwind label %landing

b4:
  store i16 1, ptr @$QB$ACTIVE
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b5

b5:
  %1 = getelementptr inbounds i16, ptr @"S%", i16 9
  store i16 1, ptr %1, !tbaa !2
  br label %b6

b6:
  %2 = getelementptr i8, ptr @"D%", i16 18
  %3 = load i16, ptr %2, !tbaa !2
  %4 = mul i16 0, %3
  %5 = add i16 %4, -2
  %6 = mul i16 %5, 2
  %7 = getelementptr i8, ptr @"D%", i16 2
  %8 = load i16, ptr %7, !tbaa !2
  %9 = getelementptr i8, ptr @"D%", i16 10
  %10 = load i16, ptr %9, !tbaa !2
  %11 = add i16 %10, %6
  %12 = inttoptr i16 %8 to ptr addrspace(2)
  %13 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr addrspace(1) %13, i16 %11
  store i16 4, ptr addrspace(1) %14, !tbaa !6
  br label %b7

b7:
  %15 = getelementptr i8, ptr @"D%", i16 18
  %16 = load i16, ptr %15, !tbaa !2
  %17 = mul i16 3, %16
  %18 = add i16 %17, 5
  %19 = mul i16 %18, 2
  %20 = getelementptr i8, ptr @"D%", i16 2
  %21 = load i16, ptr %20, !tbaa !2
  %22 = getelementptr i8, ptr @"D%", i16 10
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add i16 %23, %19
  %25 = inttoptr i16 %21 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 %24
  store i16 6, ptr addrspace(1) %27, !tbaa !6
  br label %b8

b8:
  %28 = getelementptr inbounds i16, ptr @"S%", i16 9
  %29 = load i16, ptr %28, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %29)
          to label %116 unwind label %landing

b9:
  %30 = getelementptr inbounds i16, ptr @"S%", i16 10
  %31 = load i16, ptr %30, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %31)
          to label %147 unwind label %landing

b10:
  %32 = getelementptr inbounds i16, ptr @"S%", i16 -1
  %33 = load i16, ptr %32, !tbaa !2
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %33)
          to label %148 unwind label %landing

b11:
  %34 = getelementptr i8, ptr @"D%", i16 18
  %35 = load i16, ptr %34, !tbaa !2
  %36 = mul i16 0, %35
  %37 = add i16 %36, 6
  %38 = mul i16 %37, 2
  %39 = getelementptr i8, ptr @"D%", i16 2
  %40 = load i16, ptr %39, !tbaa !2
  %41 = getelementptr i8, ptr @"D%", i16 10
  %42 = load i16, ptr %41, !tbaa !2
  %43 = add i16 %42, %38
  %44 = inttoptr i16 %40 to ptr addrspace(2)
  %45 = addrspacecast ptr addrspace(2) %44 to ptr addrspace(1)
  %46 = getelementptr i8, ptr addrspace(1) %45, i16 %43
  %47 = load i16, ptr addrspace(1) %46, !tbaa !6
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %47)
          to label %149 unwind label %landing

b12:
  %48 = getelementptr i8, ptr @"D%", i16 18
  %49 = load i16, ptr %48, !tbaa !2
  %50 = mul i16 0, %49
  %51 = add i16 %50, -3
  %52 = mul i16 %51, 2
  %53 = getelementptr i8, ptr @"D%", i16 2
  %54 = load i16, ptr %53, !tbaa !2
  %55 = getelementptr i8, ptr @"D%", i16 10
  %56 = load i16, ptr %55, !tbaa !2
  %57 = add i16 %56, %52
  %58 = inttoptr i16 %54 to ptr addrspace(2)
  %59 = addrspacecast ptr addrspace(2) %58 to ptr addrspace(1)
  %60 = getelementptr i8, ptr addrspace(1) %59, i16 %57
  %61 = load i16, ptr addrspace(1) %60, !tbaa !6
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %61)
          to label %150 unwind label %landing

b13:
  %62 = getelementptr i8, ptr @"D%", i16 18
  %63 = load i16, ptr %62, !tbaa !2
  %64 = mul i16 4, %63
  %65 = add i16 %64, 0
  %66 = mul i16 %65, 2
  %67 = getelementptr i8, ptr @"D%", i16 2
  %68 = load i16, ptr %67, !tbaa !2
  %69 = getelementptr i8, ptr @"D%", i16 10
  %70 = load i16, ptr %69, !tbaa !2
  %71 = add i16 %70, %66
  %72 = inttoptr i16 %68 to ptr addrspace(2)
  %73 = addrspacecast ptr addrspace(2) %72 to ptr addrspace(1)
  %74 = getelementptr i8, ptr addrspace(1) %73, i16 %71
  %75 = load i16, ptr addrspace(1) %74, !tbaa !6
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %75)
          to label %151 unwind label %landing

b14:
  store i32 70000, ptr @"I&", !tbaa !2
  br label %b15

b15:
  %76 = load i32, ptr @"I&", !tbaa !2
  %77 = trunc i32 %76 to i16
  %78 = getelementptr i8, ptr @"D%", i16 18
  %79 = load i16, ptr %78, !tbaa !2
  %80 = mul i16 0, %79
  %81 = add i16 %80, %77
  %82 = mul i16 %81, 2
  %83 = getelementptr i8, ptr @"D%", i16 2
  %84 = load i16, ptr %83, !tbaa !2
  %85 = getelementptr i8, ptr @"D%", i16 10
  %86 = load i16, ptr %85, !tbaa !2
  %87 = add i16 %86, %82
  %88 = inttoptr i16 %84 to ptr addrspace(2)
  %89 = addrspacecast ptr addrspace(2) %88 to ptr addrspace(1)
  %90 = getelementptr i8, ptr addrspace(1) %89, i16 %87
  %91 = load i16, ptr addrspace(1) %90, !tbaa !6
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %91)
          to label %152 unwind label %landing

b16:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"D%")
          to label %153 unwind label %landing

b17:
  %92 = getelementptr i8, ptr @"D%", i16 18
  %93 = load i16, ptr %92, !tbaa !2
  %94 = mul i16 0, %93
  %95 = add i16 %94, 0
  %96 = mul i16 %95, 2
  %97 = getelementptr i8, ptr @"D%", i16 2
  %98 = load i16, ptr %97, !tbaa !2
  %99 = getelementptr i8, ptr @"D%", i16 10
  %100 = load i16, ptr %99, !tbaa !2
  %101 = add i16 %100, %96
  %102 = inttoptr i16 %98 to ptr addrspace(2)
  %103 = addrspacecast ptr addrspace(2) %102 to ptr addrspace(1)
  %104 = getelementptr i8, ptr addrspace(1) %103, i16 %101
  %105 = load i16, ptr addrspace(1) %104, !tbaa !6
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %105)
          to label %154 unwind label %landing

b18:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
          to label %155 unwind label %landing

b19:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %156 unwind label %landing

b21:
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %157 unwind label %landing

b22:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %106 = extractvalue { ptr, i32 } %landed, 1
  %107 = trunc i32 %106 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %107, i16 0)
  switch i16 %resumed, label %110 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %108 = load i16, ptr %site
  switch i16 %108, label %158 [
    i16 0, label %b3
    i16 1, label %b8
    i16 2, label %b9
    i16 3, label %b10
    i16 4, label %b11
    i16 5, label %b12
    i16 6, label %b13
    i16 7, label %b15
    i16 8, label %b16
    i16 9, label %b17
    i16 10, label %b18
    i16 11, label %b19
    i16 12, label %b2
    i16 13, label %b21
  ]

next:
  %109 = load i16, ptr %site
  switch i16 %109, label %159 [
    i16 0, label %b4
    i16 1, label %b9
    i16 2, label %b10
    i16 3, label %b11
    i16 4, label %b12
    i16 5, label %b13
    i16 6, label %b14
    i16 7, label %b16
    i16 8, label %b17
    i16 9, label %b18
    i16 10, label %b19
    i16 11, label %b2
    i16 12, label %b21
    i16 13, label %b22
  ]

110:
  unreachable

111:
  store i16 12, ptr %site
  %112 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %113 unwind label %landing

113:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %112)
          to label %114 unwind label %landing

114:
  br label %b21

115:
  br label %b4

116:
  %117 = getelementptr i8, ptr @"D%", i16 18
  %118 = load i16, ptr %117, !tbaa !2
  %119 = mul i16 0, %118
  %120 = add i16 %119, -2
  %121 = mul i16 %120, 2
  %122 = getelementptr i8, ptr @"D%", i16 2
  %123 = load i16, ptr %122, !tbaa !2
  %124 = getelementptr i8, ptr @"D%", i16 10
  %125 = load i16, ptr %124, !tbaa !2
  %126 = add i16 %125, %121
  %127 = inttoptr i16 %123 to ptr addrspace(2)
  %128 = addrspacecast ptr addrspace(2) %127 to ptr addrspace(1)
  %129 = getelementptr i8, ptr addrspace(1) %128, i16 %126
  %130 = load i16, ptr addrspace(1) %129, !tbaa !6
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %130)
          to label %131 unwind label %landing

131:
  %132 = getelementptr i8, ptr @"D%", i16 18
  %133 = load i16, ptr %132, !tbaa !2
  %134 = mul i16 3, %133
  %135 = add i16 %134, 5
  %136 = mul i16 %135, 2
  %137 = getelementptr i8, ptr @"D%", i16 2
  %138 = load i16, ptr %137, !tbaa !2
  %139 = getelementptr i8, ptr @"D%", i16 10
  %140 = load i16, ptr %139, !tbaa !2
  %141 = add i16 %140, %136
  %142 = inttoptr i16 %138 to ptr addrspace(2)
  %143 = addrspacecast ptr addrspace(2) %142 to ptr addrspace(1)
  %144 = getelementptr i8, ptr addrspace(1) %143, i16 %141
  %145 = load i16, ptr addrspace(1) %144, !tbaa !6
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %145)
          to label %146 unwind label %landing

146:
  br label %b9

147:
  br label %b10

148:
  br label %b11

149:
  br label %b12

150:
  br label %b13

151:
  br label %b14

152:
  br label %b16

153:
  br label %b17

154:
  br label %b18

155:
  br label %b19

156:
  unreachable

157:
  unreachable

158:
  unreachable

159:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b21

b21:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 1

3:
  unreachable

4:
  unreachable
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
