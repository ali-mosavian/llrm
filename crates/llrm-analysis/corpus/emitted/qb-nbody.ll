target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [28 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"POSX&" = internal global [28 x i8] zeroinitializer
@POSX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"POSY&" = internal global [28 x i8] zeroinitializer
@POSY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSY&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSY&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELX&" = internal global [28 x i8] zeroinitializer
@VELX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELY&" = internal global [28 x i8] zeroinitializer
@VELY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELY&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELY&", [6 x i8] c"\04\00\07\00\00\00" }>
@"DELTAX&" = internal global [4 x i8] zeroinitializer
@"DELTAY&" = internal global [4 x i8] zeroinitializer
@"DIST2&" = internal global [4 x i8] zeroinitializer
@"FALLOFF&" = internal global [4 x i8] zeroinitializer
@"ACCX&" = internal global [4 x i8] zeroinitializer
@"ACCY&" = internal global [4 x i8] zeroinitializer
@"STEPCOUNT&" = internal global [4 x i8] zeroinitializer
@"STEPNO&" = internal global [4 x i8] zeroinitializer
@"BODY%" = internal global [2 x i8] zeroinitializer
@"OTHER%" = internal global [2 x i8] zeroinitializer
@TAG$ = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string7$payload to ptr addrspace(2))
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [4 x i8] c"\02\00PX" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [4 x i8] c"\02\00PY" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [4 x i8] c"\02\00VX" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>
@$string20$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 4) to i16), [4 x i8] c"\02\00VY" }>
@$string20$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 2) to i16), ptr @$fslSegment }>
@$string22$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string22$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 2) to i16), ptr @$fslSegment }>
@$string24$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string24$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  %0 = call cc1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %1 = call cc1000 addrspace(1) ptr @llrm.qb.B$FVAL(ptr %0)
  %2 = load double, ptr %1
  %3 = call i32 @llvm.lrint.i32.f64(double %2)
  store i32 %3, ptr @"STEPCOUNT&", !tbaa !2
  %4 = load i32, ptr @"STEPCOUNT&", !tbaa !2
  %5 = icmp sle i32 %4, 0
  %6 = sext i1 %5 to i16
  %7 = icmp ne i16 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  store i32 100, ptr @"STEPCOUNT&", !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  store i16 0, ptr @"BODY%", !tbaa !2
  %8 = sub i16 6, 1
  store i16 %8, ptr @$data, !tbaa !2
  %9 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %9, !tbaa !2
  br label %b5

b5:
  %10 = getelementptr i8, ptr @$data, i16 2
  %11 = load i16, ptr %10, !tbaa !2
  %12 = icmp sge i16 %11, 0
  %13 = sext i1 %12 to i16
  %14 = icmp ne i16 %13, 0
  br i1 %14, label %b6, label %b7

b6:
  %15 = load i16, ptr @"BODY%", !tbaa !2
  %16 = load i16, ptr @$data, !tbaa !2
  %17 = icmp sle i16 %15, %16
  %18 = sext i1 %17 to i16
  %19 = icmp ne i16 %18, 0
  br i1 %19, label %b8, label %b9

b7:
  %20 = load i16, ptr @"BODY%", !tbaa !2
  %21 = load i16, ptr @$data, !tbaa !2
  %22 = icmp sge i16 %20, %21
  %23 = sext i1 %22 to i16
  %24 = icmp ne i16 %23, 0
  br i1 %24, label %b8, label %b9

b8:
  %25 = load i16, ptr @"BODY%", !tbaa !2
  %26 = load i16, ptr @"BODY%", !tbaa !2
  %27 = mul i16 %26, 7
  %28 = sub i16 %27, 15
  %29 = sext i16 %28 to i32
  %30 = mul i32 %29, 512
  %31 = sub i16 %25, 0
  %32 = getelementptr inbounds i32, ptr @"POSX&", i16 %31
  store i32 %30, ptr %32, !tbaa !2
  %33 = load i16, ptr @"BODY%", !tbaa !2
  %34 = load i16, ptr @"BODY%", !tbaa !2
  %35 = mul i16 %34, 5
  %36 = sub i16 %35, 12
  %37 = sext i16 %36 to i32
  %38 = mul i32 %37, 512
  %39 = sub i16 %33, 0
  %40 = getelementptr inbounds i32, ptr @"POSY&", i16 %39
  store i32 %38, ptr %40, !tbaa !2
  %41 = load i16, ptr @"BODY%", !tbaa !2
  %42 = sub i16 %41, 0
  %43 = getelementptr inbounds i32, ptr @"VELX&", i16 %42
  store i32 0, ptr %43, !tbaa !2
  %44 = load i16, ptr @"BODY%", !tbaa !2
  %45 = sub i16 %44, 0
  %46 = getelementptr inbounds i32, ptr @"VELY&", i16 %45
  store i32 0, ptr %46, !tbaa !2
  %47 = load i16, ptr @"BODY%", !tbaa !2
  %48 = getelementptr i8, ptr @$data, i16 2
  %49 = load i16, ptr %48, !tbaa !2
  %50 = add i16 %47, %49
  store i16 %50, ptr @"BODY%", !tbaa !2
  br label %b5

b9:
  store i32 1, ptr @"STEPNO&", !tbaa !2
  %51 = load i32, ptr @"STEPCOUNT&", !tbaa !2
  %52 = getelementptr i8, ptr @$data, i16 4
  store i32 %51, ptr %52, !tbaa !2
  %53 = getelementptr i8, ptr @$data, i16 8
  store i32 1, ptr %53, !tbaa !2
  br label %b10

b10:
  %54 = getelementptr i8, ptr @$data, i16 8
  %55 = load i32, ptr %54, !tbaa !2
  %56 = icmp sge i32 %55, 0
  %57 = sext i1 %56 to i16
  %58 = icmp ne i16 %57, 0
  br i1 %58, label %b11, label %b12

b11:
  %59 = load i32, ptr @"STEPNO&", !tbaa !2
  %60 = getelementptr i8, ptr @$data, i16 4
  %61 = load i32, ptr %60, !tbaa !2
  %62 = icmp sle i32 %59, %61
  %63 = sext i1 %62 to i16
  %64 = icmp ne i16 %63, 0
  br i1 %64, label %b13, label %b14

b12:
  %65 = load i32, ptr @"STEPNO&", !tbaa !2
  %66 = getelementptr i8, ptr @$data, i16 4
  %67 = load i32, ptr %66, !tbaa !2
  %68 = icmp sge i32 %65, %67
  %69 = sext i1 %68 to i16
  %70 = icmp ne i16 %69, 0
  br i1 %70, label %b13, label %b14

b13:
  store i16 0, ptr @"BODY%", !tbaa !2
  %71 = sub i16 6, 1
  %72 = getelementptr i8, ptr @$data, i16 12
  store i16 %71, ptr %72, !tbaa !2
  %73 = getelementptr i8, ptr @$data, i16 14
  store i16 1, ptr %73, !tbaa !2
  br label %b15

b14:
  store i16 0, ptr @"BODY%", !tbaa !2
  %74 = sub i16 6, 1
  %75 = getelementptr i8, ptr @$data, i16 24
  store i16 %74, ptr %75, !tbaa !2
  %76 = getelementptr i8, ptr @$data, i16 26
  store i16 1, ptr %76, !tbaa !2
  br label %b33

b15:
  %77 = getelementptr i8, ptr @$data, i16 14
  %78 = load i16, ptr %77, !tbaa !2
  %79 = icmp sge i16 %78, 0
  %80 = sext i1 %79 to i16
  %81 = icmp ne i16 %80, 0
  br i1 %81, label %b16, label %b17

b16:
  %82 = load i16, ptr @"BODY%", !tbaa !2
  %83 = getelementptr i8, ptr @$data, i16 12
  %84 = load i16, ptr %83, !tbaa !2
  %85 = icmp sle i16 %82, %84
  %86 = sext i1 %85 to i16
  %87 = icmp ne i16 %86, 0
  br i1 %87, label %b18, label %b19

b17:
  %88 = load i16, ptr @"BODY%", !tbaa !2
  %89 = getelementptr i8, ptr @$data, i16 12
  %90 = load i16, ptr %89, !tbaa !2
  %91 = icmp sge i16 %88, %90
  %92 = sext i1 %91 to i16
  %93 = icmp ne i16 %92, 0
  br i1 %93, label %b18, label %b19

b18:
  store i32 0, ptr @"ACCX&", !tbaa !2
  store i32 0, ptr @"ACCY&", !tbaa !2
  store i16 0, ptr @"OTHER%", !tbaa !2
  %94 = sub i16 6, 1
  %95 = getelementptr i8, ptr @$data, i16 16
  store i16 %94, ptr %95, !tbaa !2
  %96 = getelementptr i8, ptr @$data, i16 18
  store i16 1, ptr %96, !tbaa !2
  br label %b20

b19:
  store i16 0, ptr @"BODY%", !tbaa !2
  %97 = sub i16 6, 1
  %98 = getelementptr i8, ptr @$data, i16 20
  store i16 %97, ptr %98, !tbaa !2
  %99 = getelementptr i8, ptr @$data, i16 22
  store i16 1, ptr %99, !tbaa !2
  br label %b28

b20:
  %100 = getelementptr i8, ptr @$data, i16 18
  %101 = load i16, ptr %100, !tbaa !2
  %102 = icmp sge i16 %101, 0
  %103 = sext i1 %102 to i16
  %104 = icmp ne i16 %103, 0
  br i1 %104, label %b21, label %b22

b21:
  %105 = load i16, ptr @"OTHER%", !tbaa !2
  %106 = getelementptr i8, ptr @$data, i16 16
  %107 = load i16, ptr %106, !tbaa !2
  %108 = icmp sle i16 %105, %107
  %109 = sext i1 %108 to i16
  %110 = icmp ne i16 %109, 0
  br i1 %110, label %b23, label %b24

b22:
  %111 = load i16, ptr @"OTHER%", !tbaa !2
  %112 = getelementptr i8, ptr @$data, i16 16
  %113 = load i16, ptr %112, !tbaa !2
  %114 = icmp sge i16 %111, %113
  %115 = sext i1 %114 to i16
  %116 = icmp ne i16 %115, 0
  br i1 %116, label %b23, label %b24

b23:
  %117 = load i16, ptr @"OTHER%", !tbaa !2
  %118 = load i16, ptr @"BODY%", !tbaa !2
  %119 = icmp ne i16 %117, %118
  %120 = sext i1 %119 to i16
  %121 = icmp ne i16 %120, 0
  br i1 %121, label %b25, label %b26

b24:
  %122 = load i16, ptr @"BODY%", !tbaa !2
  %123 = load i16, ptr @"BODY%", !tbaa !2
  %124 = sub i16 %123, 0
  %125 = getelementptr inbounds i32, ptr @"VELX&", i16 %124
  %126 = load i32, ptr %125, !tbaa !2
  %127 = load i32, ptr @"ACCX&", !tbaa !2
  %128 = add i32 %126, %127
  %129 = sub i16 %122, 0
  %130 = getelementptr inbounds i32, ptr @"VELX&", i16 %129
  store i32 %128, ptr %130, !tbaa !2
  %131 = load i16, ptr @"BODY%", !tbaa !2
  %132 = load i16, ptr @"BODY%", !tbaa !2
  %133 = sub i16 %132, 0
  %134 = getelementptr inbounds i32, ptr @"VELY&", i16 %133
  %135 = load i32, ptr %134, !tbaa !2
  %136 = load i32, ptr @"ACCY&", !tbaa !2
  %137 = add i32 %135, %136
  %138 = sub i16 %131, 0
  %139 = getelementptr inbounds i32, ptr @"VELY&", i16 %138
  store i32 %137, ptr %139, !tbaa !2
  %140 = load i16, ptr @"BODY%", !tbaa !2
  %141 = load i16, ptr @"BODY%", !tbaa !2
  %142 = sub i16 %141, 0
  %143 = getelementptr inbounds i32, ptr @"VELX&", i16 %142
  %144 = load i32, ptr %143, !tbaa !2
  %145 = load i16, ptr @"BODY%", !tbaa !2
  %146 = sub i16 %145, 0
  %147 = getelementptr inbounds i32, ptr @"VELX&", i16 %146
  %148 = load i32, ptr %147, !tbaa !2
  %149 = sdiv i32 %148, 16
  %150 = sub i32 %144, %149
  %151 = sub i16 %140, 0
  %152 = getelementptr inbounds i32, ptr @"VELX&", i16 %151
  store i32 %150, ptr %152, !tbaa !2
  %153 = load i16, ptr @"BODY%", !tbaa !2
  %154 = load i16, ptr @"BODY%", !tbaa !2
  %155 = sub i16 %154, 0
  %156 = getelementptr inbounds i32, ptr @"VELY&", i16 %155
  %157 = load i32, ptr %156, !tbaa !2
  %158 = load i16, ptr @"BODY%", !tbaa !2
  %159 = sub i16 %158, 0
  %160 = getelementptr inbounds i32, ptr @"VELY&", i16 %159
  %161 = load i32, ptr %160, !tbaa !2
  %162 = sdiv i32 %161, 16
  %163 = sub i32 %157, %162
  %164 = sub i16 %153, 0
  %165 = getelementptr inbounds i32, ptr @"VELY&", i16 %164
  store i32 %163, ptr %165, !tbaa !2
  %166 = load i16, ptr @"BODY%", !tbaa !2
  %167 = getelementptr i8, ptr @$data, i16 14
  %168 = load i16, ptr %167, !tbaa !2
  %169 = add i16 %166, %168
  store i16 %169, ptr @"BODY%", !tbaa !2
  br label %b15

b25:
  %170 = load i16, ptr @"OTHER%", !tbaa !2
  %171 = sub i16 %170, 0
  %172 = getelementptr inbounds i32, ptr @"POSX&", i16 %171
  %173 = load i32, ptr %172, !tbaa !2
  %174 = load i16, ptr @"BODY%", !tbaa !2
  %175 = sub i16 %174, 0
  %176 = getelementptr inbounds i32, ptr @"POSX&", i16 %175
  %177 = load i32, ptr %176, !tbaa !2
  %178 = sub i32 %173, %177
  store i32 %178, ptr @"DELTAX&", !tbaa !2
  %179 = load i16, ptr @"OTHER%", !tbaa !2
  %180 = sub i16 %179, 0
  %181 = getelementptr inbounds i32, ptr @"POSY&", i16 %180
  %182 = load i32, ptr %181, !tbaa !2
  %183 = load i16, ptr @"BODY%", !tbaa !2
  %184 = sub i16 %183, 0
  %185 = getelementptr inbounds i32, ptr @"POSY&", i16 %184
  %186 = load i32, ptr %185, !tbaa !2
  %187 = sub i32 %182, %186
  store i32 %187, ptr @"DELTAY&", !tbaa !2
  %188 = load i32, ptr @"DELTAX&", !tbaa !2
  %189 = load i32, ptr @"DELTAX&", !tbaa !2
  %190 = mul i32 %188, %189
  %191 = load i32, ptr @"DELTAY&", !tbaa !2
  %192 = load i32, ptr @"DELTAY&", !tbaa !2
  %193 = mul i32 %191, %192
  %194 = add i32 %190, %193
  %195 = add i32 %194, 262144
  store i32 %195, ptr @"DIST2&", !tbaa !2
  %196 = load i32, ptr @"DIST2&", !tbaa !2
  %197 = mul i32 512, 512
  %198 = sdiv i32 %196, %197
  %199 = add i32 %198, 1
  %200 = sdiv i32 512, %199
  store i32 %200, ptr @"FALLOFF&", !tbaa !2
  %201 = load i32, ptr @"ACCX&", !tbaa !2
  %202 = load i32, ptr @"DELTAX&", !tbaa !2
  %203 = load i32, ptr @"FALLOFF&", !tbaa !2
  %204 = mul i32 %202, %203
  %205 = sdiv i32 %204, 512
  %206 = add i32 %201, %205
  store i32 %206, ptr @"ACCX&", !tbaa !2
  %207 = load i32, ptr @"ACCY&", !tbaa !2
  %208 = load i32, ptr @"DELTAY&", !tbaa !2
  %209 = load i32, ptr @"FALLOFF&", !tbaa !2
  %210 = mul i32 %208, %209
  %211 = sdiv i32 %210, 512
  %212 = add i32 %207, %211
  store i32 %212, ptr @"ACCY&", !tbaa !2
  br label %b27

b26:
  br label %b27

b27:
  %213 = load i16, ptr @"OTHER%", !tbaa !2
  %214 = getelementptr i8, ptr @$data, i16 18
  %215 = load i16, ptr %214, !tbaa !2
  %216 = add i16 %213, %215
  store i16 %216, ptr @"OTHER%", !tbaa !2
  br label %b20

b28:
  %217 = getelementptr i8, ptr @$data, i16 22
  %218 = load i16, ptr %217, !tbaa !2
  %219 = icmp sge i16 %218, 0
  %220 = sext i1 %219 to i16
  %221 = icmp ne i16 %220, 0
  br i1 %221, label %b29, label %b30

b29:
  %222 = load i16, ptr @"BODY%", !tbaa !2
  %223 = getelementptr i8, ptr @$data, i16 20
  %224 = load i16, ptr %223, !tbaa !2
  %225 = icmp sle i16 %222, %224
  %226 = sext i1 %225 to i16
  %227 = icmp ne i16 %226, 0
  br i1 %227, label %b31, label %b32

b30:
  %228 = load i16, ptr @"BODY%", !tbaa !2
  %229 = getelementptr i8, ptr @$data, i16 20
  %230 = load i16, ptr %229, !tbaa !2
  %231 = icmp sge i16 %228, %230
  %232 = sext i1 %231 to i16
  %233 = icmp ne i16 %232, 0
  br i1 %233, label %b31, label %b32

b31:
  %234 = load i16, ptr @"BODY%", !tbaa !2
  %235 = load i16, ptr @"BODY%", !tbaa !2
  %236 = sub i16 %235, 0
  %237 = getelementptr inbounds i32, ptr @"POSX&", i16 %236
  %238 = load i32, ptr %237, !tbaa !2
  %239 = load i16, ptr @"BODY%", !tbaa !2
  %240 = sub i16 %239, 0
  %241 = getelementptr inbounds i32, ptr @"VELX&", i16 %240
  %242 = load i32, ptr %241, !tbaa !2
  %243 = add i32 %238, %242
  %244 = sub i16 %234, 0
  %245 = getelementptr inbounds i32, ptr @"POSX&", i16 %244
  store i32 %243, ptr %245, !tbaa !2
  %246 = load i16, ptr @"BODY%", !tbaa !2
  %247 = load i16, ptr @"BODY%", !tbaa !2
  %248 = sub i16 %247, 0
  %249 = getelementptr inbounds i32, ptr @"POSY&", i16 %248
  %250 = load i32, ptr %249, !tbaa !2
  %251 = load i16, ptr @"BODY%", !tbaa !2
  %252 = sub i16 %251, 0
  %253 = getelementptr inbounds i32, ptr @"VELY&", i16 %252
  %254 = load i32, ptr %253, !tbaa !2
  %255 = add i32 %250, %254
  %256 = sub i16 %246, 0
  %257 = getelementptr inbounds i32, ptr @"POSY&", i16 %256
  store i32 %255, ptr %257, !tbaa !2
  %258 = load i16, ptr @"BODY%", !tbaa !2
  %259 = getelementptr i8, ptr @$data, i16 22
  %260 = load i16, ptr %259, !tbaa !2
  %261 = add i16 %258, %260
  store i16 %261, ptr @"BODY%", !tbaa !2
  br label %b28

b32:
  %262 = load i32, ptr @"STEPNO&", !tbaa !2
  %263 = getelementptr i8, ptr @$data, i16 8
  %264 = load i32, ptr %263, !tbaa !2
  %265 = add i32 %262, %264
  store i32 %265, ptr @"STEPNO&", !tbaa !2
  br label %b10

b33:
  %266 = getelementptr i8, ptr @$data, i16 26
  %267 = load i16, ptr %266, !tbaa !2
  %268 = icmp sge i16 %267, 0
  %269 = sext i1 %268 to i16
  %270 = icmp ne i16 %269, 0
  br i1 %270, label %b34, label %b35

b34:
  %271 = load i16, ptr @"BODY%", !tbaa !2
  %272 = getelementptr i8, ptr @$data, i16 24
  %273 = load i16, ptr %272, !tbaa !2
  %274 = icmp sle i16 %271, %273
  %275 = sext i1 %274 to i16
  %276 = icmp ne i16 %275, 0
  br i1 %276, label %b36, label %b37

b35:
  %277 = load i16, ptr @"BODY%", !tbaa !2
  %278 = getelementptr i8, ptr @$data, i16 24
  %279 = load i16, ptr %278, !tbaa !2
  %280 = icmp sge i16 %277, %279
  %281 = sext i1 %280 to i16
  %282 = icmp ne i16 %281, 0
  br i1 %282, label %b36, label %b37

b36:
  %283 = load i16, ptr @"BODY%", !tbaa !2
  %284 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %283)
  %285 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %284)
  call cc1000 addrspace(1) void @llrm.qb.B$SASS(ptr %285, ptr @TAG$)
  %286 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string7$descriptor, ptr @TAG$)
  %287 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %286, ptr @$string10$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %287)
  %288 = load i16, ptr @"BODY%", !tbaa !2
  %289 = sub i16 %288, 0
  %290 = getelementptr inbounds i32, ptr @"POSX&", i16 %289
  %291 = load i32, ptr %290, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %291)
  %292 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string12$descriptor, ptr @TAG$)
  %293 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %292, ptr @$string14$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %293)
  %294 = load i16, ptr @"BODY%", !tbaa !2
  %295 = sub i16 %294, 0
  %296 = getelementptr inbounds i32, ptr @"POSY&", i16 %295
  %297 = load i32, ptr %296, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %297)
  %298 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string16$descriptor, ptr @TAG$)
  %299 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %298, ptr @$string18$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %299)
  %300 = load i16, ptr @"BODY%", !tbaa !2
  %301 = sub i16 %300, 0
  %302 = getelementptr inbounds i32, ptr @"VELX&", i16 %301
  %303 = load i32, ptr %302, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %303)
  %304 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string20$descriptor, ptr @TAG$)
  %305 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %304, ptr @$string22$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %305)
  %306 = load i16, ptr @"BODY%", !tbaa !2
  %307 = sub i16 %306, 0
  %308 = getelementptr inbounds i32, ptr @"VELY&", i16 %307
  %309 = load i32, ptr %308, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %309)
  %310 = load i16, ptr @"BODY%", !tbaa !2
  %311 = getelementptr i8, ptr @$data, i16 26
  %312 = load i16, ptr %311, !tbaa !2
  %313 = add i16 %310, %312
  store i16 %313, ptr @"BODY%", !tbaa !2
  br label %b33

b37:
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string24$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 ptr @llrm.qb.B$FCMD() addrspace(1)

declare cc1000 ptr @llrm.qb.B$FVAL(ptr) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
