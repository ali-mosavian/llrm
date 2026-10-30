target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [32 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"POSX&" = internal global [28 x i8] zeroinitializer, align 2
@POSX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"POSY&" = internal global [28 x i8] zeroinitializer, align 2
@POSY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSY&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSY&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELX&" = internal global [28 x i8] zeroinitializer, align 2
@VELX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELY&" = internal global [28 x i8] zeroinitializer, align 2
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
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"PX" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c"PY" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"=\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string11, i16 4), [2 x i8] c"VX" }>
@$string12 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string12, i16 4), [2 x i8] c"=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string13, i16 4), [2 x i8] c"VY" }>
@$string14 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string14, i16 4), [2 x i8] c"=\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string15, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %1 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FVAL(ptr %0)
  %2 = load double, ptr %1
  %3 = call i32 @llvm.lrint.i32.f64(double %2)
  store i32 %3, ptr @"STEPCOUNT&", !tbaa !2
  %4 = load i32, ptr @"STEPCOUNT&", !tbaa !2
  %5 = icmp sle i32 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i32 100, ptr @"STEPCOUNT&", !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  store i16 0, ptr @"BODY%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
  %6 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %6, !tbaa !2
  br label %b5

b5:
  %7 = getelementptr i8, ptr @$data, i16 2
  %8 = load i16, ptr %7, !tbaa !2
  %9 = icmp sge i16 %8, 0
  br i1 %9, label %b6, label %b7

b6:
  %10 = load i16, ptr @"BODY%", !tbaa !2
  %11 = load i16, ptr @$data, !tbaa !2
  %12 = icmp sle i16 %10, %11
  br i1 %12, label %b8, label %b9

b7:
  %13 = load i16, ptr @"BODY%", !tbaa !2
  %14 = load i16, ptr @$data, !tbaa !2
  %15 = icmp sge i16 %13, %14
  br i1 %15, label %b8, label %b9

b8:
  %16 = load i16, ptr @"BODY%", !tbaa !2
  %17 = load i16, ptr @"BODY%", !tbaa !2
  %18 = mul i16 %17, 7
  %19 = sub i16 %18, 15
  %20 = sext i16 %19 to i32
  %21 = mul i32 %20, 512
  %22 = sub i16 %16, 0
  %23 = getelementptr inbounds i32, ptr @"POSX&", i16 %22
  store i32 %21, ptr %23, !tbaa !2
  %24 = load i16, ptr @"BODY%", !tbaa !2
  %25 = load i16, ptr @"BODY%", !tbaa !2
  %26 = mul i16 %25, 5
  %27 = sub i16 %26, 12
  %28 = sext i16 %27 to i32
  %29 = mul i32 %28, 512
  %30 = sub i16 %24, 0
  %31 = getelementptr inbounds i32, ptr @"POSY&", i16 %30
  store i32 %29, ptr %31, !tbaa !2
  %32 = load i16, ptr @"BODY%", !tbaa !2
  %33 = sub i16 %32, 0
  %34 = getelementptr inbounds i32, ptr @"VELX&", i16 %33
  store i32 0, ptr %34, !tbaa !2
  %35 = load i16, ptr @"BODY%", !tbaa !2
  %36 = sub i16 %35, 0
  %37 = getelementptr inbounds i32, ptr @"VELY&", i16 %36
  store i32 0, ptr %37, !tbaa !2
  %38 = load i16, ptr @"BODY%", !tbaa !2
  %39 = getelementptr i8, ptr @$data, i16 2
  %40 = load i16, ptr %39, !tbaa !2
  %41 = add nsw i16 %38, %40
  store i16 %41, ptr @"BODY%", !tbaa !2
  br label %b5

b9:
  store i32 1, ptr @"STEPNO&", !tbaa !2
  %42 = load i32, ptr @"STEPCOUNT&", !tbaa !2
  %43 = getelementptr i8, ptr @$data, i16 4
  store i32 %42, ptr %43, !tbaa !2
  %44 = getelementptr i8, ptr @$data, i16 8
  store i32 1, ptr %44, !tbaa !2
  br label %b10

b10:
  %45 = getelementptr i8, ptr @$data, i16 8
  %46 = load i32, ptr %45, !tbaa !2
  %47 = icmp sge i32 %46, 0
  br i1 %47, label %b11, label %b12

b11:
  %48 = load i32, ptr @"STEPNO&", !tbaa !2
  %49 = getelementptr i8, ptr @$data, i16 4
  %50 = load i32, ptr %49, !tbaa !2
  %51 = icmp sle i32 %48, %50
  br i1 %51, label %b13, label %b14

b12:
  %52 = load i32, ptr @"STEPNO&", !tbaa !2
  %53 = getelementptr i8, ptr @$data, i16 4
  %54 = load i32, ptr %53, !tbaa !2
  %55 = icmp sge i32 %52, %54
  br i1 %55, label %b13, label %b14

b13:
  store i16 0, ptr @"BODY%", !tbaa !2
  %56 = getelementptr i8, ptr @$data, i16 12
  store i16 5, ptr %56, !tbaa !2
  %57 = getelementptr i8, ptr @$data, i16 14
  store i16 1, ptr %57, !tbaa !2
  br label %b15

b14:
  store i16 0, ptr @"BODY%", !tbaa !2
  %58 = getelementptr i8, ptr @$data, i16 28
  store i16 5, ptr %58, !tbaa !2
  %59 = getelementptr i8, ptr @$data, i16 30
  store i16 1, ptr %59, !tbaa !2
  br label %b40

b15:
  %60 = getelementptr i8, ptr @$data, i16 14
  %61 = load i16, ptr %60, !tbaa !2
  %62 = icmp sge i16 %61, 0
  br i1 %62, label %b16, label %b17

b16:
  %63 = load i16, ptr @"BODY%", !tbaa !2
  %64 = getelementptr i8, ptr @$data, i16 12
  %65 = load i16, ptr %64, !tbaa !2
  %66 = icmp sle i16 %63, %65
  br i1 %66, label %b18, label %b19

b17:
  %67 = load i16, ptr @"BODY%", !tbaa !2
  %68 = getelementptr i8, ptr @$data, i16 12
  %69 = load i16, ptr %68, !tbaa !2
  %70 = icmp sge i16 %67, %69
  br i1 %70, label %b18, label %b19

b18:
  store i32 0, ptr @"ACCX&", !tbaa !2
  store i32 0, ptr @"ACCY&", !tbaa !2
  store i16 0, ptr @"OTHER%", !tbaa !2
  %71 = getelementptr i8, ptr @$data, i16 16
  store i16 5, ptr %71, !tbaa !2
  %72 = getelementptr i8, ptr @$data, i16 18
  store i16 1, ptr %72, !tbaa !2
  br label %b20

b19:
  store i16 0, ptr @"BODY%", !tbaa !2
  %73 = getelementptr i8, ptr @$data, i16 24
  store i16 5, ptr %73, !tbaa !2
  %74 = getelementptr i8, ptr @$data, i16 26
  store i16 1, ptr %74, !tbaa !2
  br label %b35

b20:
  %75 = getelementptr i8, ptr @$data, i16 18
  %76 = load i16, ptr %75, !tbaa !2
  %77 = icmp sge i16 %76, 0
  br i1 %77, label %b21, label %b22

b21:
  %78 = load i16, ptr @"OTHER%", !tbaa !2
  %79 = getelementptr i8, ptr @$data, i16 16
  %80 = load i16, ptr %79, !tbaa !2
  %81 = icmp sle i16 %78, %80
  br i1 %81, label %b23, label %b24

b22:
  %82 = load i16, ptr @"OTHER%", !tbaa !2
  %83 = getelementptr i8, ptr @$data, i16 16
  %84 = load i16, ptr %83, !tbaa !2
  %85 = icmp sge i16 %82, %84
  br i1 %85, label %b23, label %b24

b23:
  %86 = load i16, ptr @"OTHER%", !tbaa !2
  %87 = load i16, ptr @"BODY%", !tbaa !2
  %88 = icmp ne i16 %86, %87
  br i1 %88, label %b25, label %b26

b24:
  %89 = load i16, ptr @"BODY%", !tbaa !2
  %90 = load i16, ptr @"BODY%", !tbaa !2
  %91 = sub i16 %90, 0
  %92 = getelementptr inbounds i32, ptr @"VELX&", i16 %91
  %93 = load i32, ptr %92, !tbaa !2
  %94 = load i32, ptr @"ACCX&", !tbaa !2
  %95 = add i32 %93, %94
  %96 = sub i16 %89, 0
  %97 = getelementptr inbounds i32, ptr @"VELX&", i16 %96
  store i32 %95, ptr %97, !tbaa !2
  %98 = load i16, ptr @"BODY%", !tbaa !2
  %99 = load i16, ptr @"BODY%", !tbaa !2
  %100 = sub i16 %99, 0
  %101 = getelementptr inbounds i32, ptr @"VELY&", i16 %100
  %102 = load i32, ptr %101, !tbaa !2
  %103 = load i32, ptr @"ACCY&", !tbaa !2
  %104 = add i32 %102, %103
  %105 = sub i16 %98, 0
  %106 = getelementptr inbounds i32, ptr @"VELY&", i16 %105
  store i32 %104, ptr %106, !tbaa !2
  %107 = load i16, ptr @"BODY%", !tbaa !2
  %108 = load i16, ptr @"BODY%", !tbaa !2
  %109 = sub i16 %108, 0
  %110 = getelementptr inbounds i32, ptr @"VELX&", i16 %109
  %111 = load i32, ptr %110, !tbaa !2
  %112 = load i16, ptr @"BODY%", !tbaa !2
  %113 = sub i16 %112, 0
  %114 = getelementptr inbounds i32, ptr @"VELX&", i16 %113
  %115 = load i32, ptr %114, !tbaa !2
  %116 = sdiv i32 %115, 16
  %117 = sub i32 %111, %116
  %118 = sub i16 %107, 0
  %119 = getelementptr inbounds i32, ptr @"VELX&", i16 %118
  store i32 %117, ptr %119, !tbaa !2
  %120 = load i16, ptr @"BODY%", !tbaa !2
  %121 = load i16, ptr @"BODY%", !tbaa !2
  %122 = sub i16 %121, 0
  %123 = getelementptr inbounds i32, ptr @"VELY&", i16 %122
  %124 = load i32, ptr %123, !tbaa !2
  %125 = load i16, ptr @"BODY%", !tbaa !2
  %126 = sub i16 %125, 0
  %127 = getelementptr inbounds i32, ptr @"VELY&", i16 %126
  %128 = load i32, ptr %127, !tbaa !2
  %129 = sdiv i32 %128, 16
  %130 = sub i32 %124, %129
  %131 = sub i16 %120, 0
  %132 = getelementptr inbounds i32, ptr @"VELY&", i16 %131
  store i32 %130, ptr %132, !tbaa !2
  %133 = load i16, ptr @"BODY%", !tbaa !2
  %134 = getelementptr i8, ptr @$data, i16 14
  %135 = load i16, ptr %134, !tbaa !2
  %136 = add nsw i16 %133, %135
  store i16 %136, ptr @"BODY%", !tbaa !2
  br label %b15

b25:
  %137 = load i16, ptr @"OTHER%", !tbaa !2
  %138 = sub i16 %137, 0
  %139 = getelementptr inbounds i32, ptr @"POSX&", i16 %138
  %140 = load i32, ptr %139, !tbaa !2
  %141 = load i16, ptr @"BODY%", !tbaa !2
  %142 = sub i16 %141, 0
  %143 = getelementptr inbounds i32, ptr @"POSX&", i16 %142
  %144 = load i32, ptr %143, !tbaa !2
  %145 = sub i32 %140, %144
  store i32 %145, ptr @"DELTAX&", !tbaa !2
  %146 = load i16, ptr @"OTHER%", !tbaa !2
  %147 = sub i16 %146, 0
  %148 = getelementptr inbounds i32, ptr @"POSY&", i16 %147
  %149 = load i32, ptr %148, !tbaa !2
  %150 = load i16, ptr @"BODY%", !tbaa !2
  %151 = sub i16 %150, 0
  %152 = getelementptr inbounds i32, ptr @"POSY&", i16 %151
  %153 = load i32, ptr %152, !tbaa !2
  %154 = sub i32 %149, %153
  store i32 %154, ptr @"DELTAY&", !tbaa !2
  %155 = load i32, ptr @"DELTAX&", !tbaa !2
  %156 = load i32, ptr @"DELTAX&", !tbaa !2
  %157 = mul i32 %155, %156
  %158 = load i32, ptr @"DELTAY&", !tbaa !2
  %159 = load i32, ptr @"DELTAY&", !tbaa !2
  %160 = mul i32 %158, %159
  %161 = add i32 %157, %160
  %162 = add i32 %161, 262144
  store i32 %162, ptr @"DIST2&", !tbaa !2
  %163 = load i32, ptr @"DIST2&", !tbaa !2
  br i1 false, label %b28, label %b29

b26:
  br label %b27

b27:
  %164 = load i16, ptr @"OTHER%", !tbaa !2
  %165 = getelementptr i8, ptr @$data, i16 18
  %166 = load i16, ptr %165, !tbaa !2
  %167 = add nsw i16 %164, %166
  store i16 %167, ptr @"OTHER%", !tbaa !2
  br label %b20

b28:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b29:
  br i1 false, label %b30, label %b31

b30:
  %168 = sub i32 0, %163
  %169 = getelementptr i8, ptr @$data, i16 20
  store i32 %168, ptr %169, !tbaa !2
  br label %b32

b31:
  %170 = sdiv i32 %163, 262144
  %171 = getelementptr i8, ptr @$data, i16 20
  store i32 %170, ptr %171, !tbaa !2
  br label %b32

b32:
  %172 = getelementptr i8, ptr @$data, i16 20
  %173 = load i32, ptr %172, !tbaa !2
  %174 = add i32 %173, 1
  %175 = icmp eq i32 %174, 0
  br i1 %175, label %b33, label %b34

b33:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b34:
  %176 = sdiv i32 512, %174
  store i32 %176, ptr @"FALLOFF&", !tbaa !2
  %177 = load i32, ptr @"ACCX&", !tbaa !2
  %178 = load i32, ptr @"DELTAX&", !tbaa !2
  %179 = load i32, ptr @"FALLOFF&", !tbaa !2
  %180 = mul i32 %178, %179
  %181 = sdiv i32 %180, 512
  %182 = add i32 %177, %181
  store i32 %182, ptr @"ACCX&", !tbaa !2
  %183 = load i32, ptr @"ACCY&", !tbaa !2
  %184 = load i32, ptr @"DELTAY&", !tbaa !2
  %185 = load i32, ptr @"FALLOFF&", !tbaa !2
  %186 = mul i32 %184, %185
  %187 = sdiv i32 %186, 512
  %188 = add i32 %183, %187
  store i32 %188, ptr @"ACCY&", !tbaa !2
  br label %b27

b35:
  %189 = getelementptr i8, ptr @$data, i16 26
  %190 = load i16, ptr %189, !tbaa !2
  %191 = icmp sge i16 %190, 0
  br i1 %191, label %b36, label %b37

b36:
  %192 = load i16, ptr @"BODY%", !tbaa !2
  %193 = getelementptr i8, ptr @$data, i16 24
  %194 = load i16, ptr %193, !tbaa !2
  %195 = icmp sle i16 %192, %194
  br i1 %195, label %b38, label %b39

b37:
  %196 = load i16, ptr @"BODY%", !tbaa !2
  %197 = getelementptr i8, ptr @$data, i16 24
  %198 = load i16, ptr %197, !tbaa !2
  %199 = icmp sge i16 %196, %198
  br i1 %199, label %b38, label %b39

b38:
  %200 = load i16, ptr @"BODY%", !tbaa !2
  %201 = load i16, ptr @"BODY%", !tbaa !2
  %202 = sub i16 %201, 0
  %203 = getelementptr inbounds i32, ptr @"POSX&", i16 %202
  %204 = load i32, ptr %203, !tbaa !2
  %205 = load i16, ptr @"BODY%", !tbaa !2
  %206 = sub i16 %205, 0
  %207 = getelementptr inbounds i32, ptr @"VELX&", i16 %206
  %208 = load i32, ptr %207, !tbaa !2
  %209 = add i32 %204, %208
  %210 = sub i16 %200, 0
  %211 = getelementptr inbounds i32, ptr @"POSX&", i16 %210
  store i32 %209, ptr %211, !tbaa !2
  %212 = load i16, ptr @"BODY%", !tbaa !2
  %213 = load i16, ptr @"BODY%", !tbaa !2
  %214 = sub i16 %213, 0
  %215 = getelementptr inbounds i32, ptr @"POSY&", i16 %214
  %216 = load i32, ptr %215, !tbaa !2
  %217 = load i16, ptr @"BODY%", !tbaa !2
  %218 = sub i16 %217, 0
  %219 = getelementptr inbounds i32, ptr @"VELY&", i16 %218
  %220 = load i32, ptr %219, !tbaa !2
  %221 = add i32 %216, %220
  %222 = sub i16 %212, 0
  %223 = getelementptr inbounds i32, ptr @"POSY&", i16 %222
  store i32 %221, ptr %223, !tbaa !2
  %224 = load i16, ptr @"BODY%", !tbaa !2
  %225 = getelementptr i8, ptr @$data, i16 26
  %226 = load i16, ptr %225, !tbaa !2
  %227 = add nsw i16 %224, %226
  store i16 %227, ptr @"BODY%", !tbaa !2
  br label %b35

b39:
  %228 = load i32, ptr @"STEPNO&", !tbaa !2
  %229 = getelementptr i8, ptr @$data, i16 8
  %230 = load i32, ptr %229, !tbaa !2
  %231 = add nsw i32 %228, %230
  store i32 %231, ptr @"STEPNO&", !tbaa !2
  br label %b10

b40:
  %232 = getelementptr i8, ptr @$data, i16 30
  %233 = load i16, ptr %232, !tbaa !2
  %234 = icmp sge i16 %233, 0
  br i1 %234, label %b41, label %b42

b41:
  %235 = load i16, ptr @"BODY%", !tbaa !2
  %236 = getelementptr i8, ptr @$data, i16 28
  %237 = load i16, ptr %236, !tbaa !2
  %238 = icmp sle i16 %235, %237
  br i1 %238, label %b43, label %b44

b42:
  %239 = load i16, ptr @"BODY%", !tbaa !2
  %240 = getelementptr i8, ptr @$data, i16 28
  %241 = load i16, ptr %240, !tbaa !2
  %242 = icmp sge i16 %239, %241
  br i1 %242, label %b43, label %b44

b43:
  %243 = load i16, ptr @"BODY%", !tbaa !2
  %244 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %243)
  %245 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %244)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %245, ptr @TAG$)
  %246 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string7, ptr @TAG$)
  %247 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %246, ptr @$string8)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %247)
  %248 = load i16, ptr @"BODY%", !tbaa !2
  %249 = sub i16 %248, 0
  %250 = getelementptr inbounds i32, ptr @"POSX&", i16 %249
  %251 = load i32, ptr %250, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %251)
  %252 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string9, ptr @TAG$)
  %253 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %252, ptr @$string10)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %253)
  %254 = load i16, ptr @"BODY%", !tbaa !2
  %255 = sub i16 %254, 0
  %256 = getelementptr inbounds i32, ptr @"POSY&", i16 %255
  %257 = load i32, ptr %256, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %257)
  %258 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string11, ptr @TAG$)
  %259 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %258, ptr @$string12)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %259)
  %260 = load i16, ptr @"BODY%", !tbaa !2
  %261 = sub i16 %260, 0
  %262 = getelementptr inbounds i32, ptr @"VELX&", i16 %261
  %263 = load i32, ptr %262, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %263)
  %264 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string13, ptr @TAG$)
  %265 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %264, ptr @$string14)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %265)
  %266 = load i16, ptr @"BODY%", !tbaa !2
  %267 = sub i16 %266, 0
  %268 = getelementptr inbounds i32, ptr @"VELY&", i16 %267
  %269 = load i32, ptr %268, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %269)
  %270 = load i16, ptr @"BODY%", !tbaa !2
  %271 = getelementptr i8, ptr @$data, i16 30
  %272 = load i16, ptr %271, !tbaa !2
  %273 = add nsw i16 %270, %272
  store i16 %273, ptr @"BODY%", !tbaa !2
  br label %b40

b44:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string15)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 ptr @llrm.qb.B$FCMD() addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FVAL(ptr) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
