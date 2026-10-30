target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"POSX&" = internal global [28 x i8] zeroinitializer, align 2
@POSX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"POSY&" = internal global [28 x i8] zeroinitializer, align 2
@POSY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSY&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSY&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELX&" = internal global [28 x i8] zeroinitializer, align 2
@VELX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELY&" = internal global [28 x i8] zeroinitializer, align 2
@VELY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELY&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELY&", [6 x i8] c"\04\00\07\00\00\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1000 addrspace(1) i32 @"BENCHNBODYFIXED&"()
  %1 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %0)
  %2 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 i32 @"BENCHNBODYFIXED&"() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i32
  %17 = alloca i32
  %18 = alloca i32
  %19 = alloca i32
  %20 = alloca i32
  %21 = alloca i32
  %22 = alloca i32
  %23 = alloca i32
  store i16 0, ptr %15, !tbaa !2
  store i16 5, ptr %14, !tbaa !2
  store i16 1, ptr %13, !tbaa !2
  br label %b2

b2:
  %24 = load i16, ptr %13, !tbaa !2
  %25 = icmp sge i16 %24, 0
  br i1 %25, label %b3, label %b4

b3:
  %26 = load i16, ptr %15, !tbaa !2
  %27 = load i16, ptr %14, !tbaa !2
  %28 = icmp sle i16 %26, %27
  br i1 %28, label %b5, label %b6

b4:
  %29 = load i16, ptr %15, !tbaa !2
  %30 = load i16, ptr %14, !tbaa !2
  %31 = icmp sge i16 %29, %30
  br i1 %31, label %b5, label %b6

b5:
  %32 = load i16, ptr %15, !tbaa !2
  %33 = load i16, ptr %15, !tbaa !2
  %34 = mul i16 %33, 7
  %35 = sub i16 %34, 15
  %36 = sext i16 %35 to i32
  %37 = mul i32 %36, 512
  %38 = sub i16 %32, 0
  %39 = getelementptr inbounds i32, ptr @"POSX&", i16 %38
  store i32 %37, ptr %39, !tbaa !2
  %40 = load i16, ptr %15, !tbaa !2
  %41 = load i16, ptr %15, !tbaa !2
  %42 = mul i16 %41, 5
  %43 = sub i16 %42, 12
  %44 = sext i16 %43 to i32
  %45 = mul i32 %44, 512
  %46 = sub i16 %40, 0
  %47 = getelementptr inbounds i32, ptr @"POSY&", i16 %46
  store i32 %45, ptr %47, !tbaa !2
  %48 = load i16, ptr %15, !tbaa !2
  %49 = sub i16 %48, 0
  %50 = getelementptr inbounds i32, ptr @"VELX&", i16 %49
  store i32 0, ptr %50, !tbaa !2
  %51 = load i16, ptr %15, !tbaa !2
  %52 = sub i16 %51, 0
  %53 = getelementptr inbounds i32, ptr @"VELY&", i16 %52
  store i32 0, ptr %53, !tbaa !2
  %54 = load i16, ptr %15, !tbaa !2
  %55 = load i16, ptr %13, !tbaa !2
  %56 = add nsw i16 %54, %55
  store i16 %56, ptr %15, !tbaa !2
  br label %b2

b6:
  store i16 1, ptr %12, !tbaa !2
  store i16 1000, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b7

b7:
  %57 = load i16, ptr %10, !tbaa !2
  %58 = icmp sge i16 %57, 0
  br i1 %58, label %b8, label %b9

b8:
  %59 = load i16, ptr %12, !tbaa !2
  %60 = load i16, ptr %11, !tbaa !2
  %61 = icmp sle i16 %59, %60
  br i1 %61, label %b10, label %b11

b9:
  %62 = load i16, ptr %12, !tbaa !2
  %63 = load i16, ptr %11, !tbaa !2
  %64 = icmp sge i16 %62, %63
  br i1 %64, label %b10, label %b11

b10:
  store i16 0, ptr %15, !tbaa !2
  store i16 5, ptr %9, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  br label %b12

b11:
  store i32 0, ptr %16, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i16 5, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b37

b12:
  %65 = load i16, ptr %8, !tbaa !2
  %66 = icmp sge i16 %65, 0
  br i1 %66, label %b13, label %b14

b13:
  %67 = load i16, ptr %15, !tbaa !2
  %68 = load i16, ptr %9, !tbaa !2
  %69 = icmp sle i16 %67, %68
  br i1 %69, label %b15, label %b16

b14:
  %70 = load i16, ptr %15, !tbaa !2
  %71 = load i16, ptr %9, !tbaa !2
  %72 = icmp sge i16 %70, %71
  br i1 %72, label %b15, label %b16

b15:
  store i32 0, ptr %18, !tbaa !2
  store i32 0, ptr %17, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 5, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b17

b16:
  store i16 0, ptr %15, !tbaa !2
  store i16 5, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b32

b17:
  %73 = load i16, ptr %5, !tbaa !2
  %74 = icmp sge i16 %73, 0
  br i1 %74, label %b18, label %b19

b18:
  %75 = load i16, ptr %7, !tbaa !2
  %76 = load i16, ptr %6, !tbaa !2
  %77 = icmp sle i16 %75, %76
  br i1 %77, label %b20, label %b21

b19:
  %78 = load i16, ptr %7, !tbaa !2
  %79 = load i16, ptr %6, !tbaa !2
  %80 = icmp sge i16 %78, %79
  br i1 %80, label %b20, label %b21

b20:
  %81 = load i16, ptr %7, !tbaa !2
  %82 = load i16, ptr %15, !tbaa !2
  %83 = icmp ne i16 %81, %82
  br i1 %83, label %b22, label %b23

b21:
  %84 = load i16, ptr %15, !tbaa !2
  %85 = load i16, ptr %15, !tbaa !2
  %86 = sub i16 %85, 0
  %87 = getelementptr inbounds i32, ptr @"VELX&", i16 %86
  %88 = load i32, ptr %87, !tbaa !2
  %89 = load i32, ptr %18, !tbaa !2
  %90 = add i32 %88, %89
  %91 = sub i16 %84, 0
  %92 = getelementptr inbounds i32, ptr @"VELX&", i16 %91
  store i32 %90, ptr %92, !tbaa !2
  %93 = load i16, ptr %15, !tbaa !2
  %94 = load i16, ptr %15, !tbaa !2
  %95 = sub i16 %94, 0
  %96 = getelementptr inbounds i32, ptr @"VELY&", i16 %95
  %97 = load i32, ptr %96, !tbaa !2
  %98 = load i32, ptr %17, !tbaa !2
  %99 = add i32 %97, %98
  %100 = sub i16 %93, 0
  %101 = getelementptr inbounds i32, ptr @"VELY&", i16 %100
  store i32 %99, ptr %101, !tbaa !2
  %102 = load i16, ptr %15, !tbaa !2
  %103 = load i16, ptr %15, !tbaa !2
  %104 = sub i16 %103, 0
  %105 = getelementptr inbounds i32, ptr @"VELX&", i16 %104
  %106 = load i32, ptr %105, !tbaa !2
  %107 = load i16, ptr %15, !tbaa !2
  %108 = sub i16 %107, 0
  %109 = getelementptr inbounds i32, ptr @"VELX&", i16 %108
  %110 = load i32, ptr %109, !tbaa !2
  %111 = sdiv i32 %110, 16
  %112 = sub i32 %106, %111
  %113 = sub i16 %102, 0
  %114 = getelementptr inbounds i32, ptr @"VELX&", i16 %113
  store i32 %112, ptr %114, !tbaa !2
  %115 = load i16, ptr %15, !tbaa !2
  %116 = load i16, ptr %15, !tbaa !2
  %117 = sub i16 %116, 0
  %118 = getelementptr inbounds i32, ptr @"VELY&", i16 %117
  %119 = load i32, ptr %118, !tbaa !2
  %120 = load i16, ptr %15, !tbaa !2
  %121 = sub i16 %120, 0
  %122 = getelementptr inbounds i32, ptr @"VELY&", i16 %121
  %123 = load i32, ptr %122, !tbaa !2
  %124 = sdiv i32 %123, 16
  %125 = sub i32 %119, %124
  %126 = sub i16 %115, 0
  %127 = getelementptr inbounds i32, ptr @"VELY&", i16 %126
  store i32 %125, ptr %127, !tbaa !2
  %128 = load i16, ptr %15, !tbaa !2
  %129 = load i16, ptr %8, !tbaa !2
  %130 = add nsw i16 %128, %129
  store i16 %130, ptr %15, !tbaa !2
  br label %b12

b22:
  %131 = load i16, ptr %7, !tbaa !2
  %132 = sub i16 %131, 0
  %133 = getelementptr inbounds i32, ptr @"POSX&", i16 %132
  %134 = load i32, ptr %133, !tbaa !2
  %135 = load i16, ptr %15, !tbaa !2
  %136 = sub i16 %135, 0
  %137 = getelementptr inbounds i32, ptr @"POSX&", i16 %136
  %138 = load i32, ptr %137, !tbaa !2
  %139 = sub i32 %134, %138
  store i32 %139, ptr %22, !tbaa !2
  %140 = load i16, ptr %7, !tbaa !2
  %141 = sub i16 %140, 0
  %142 = getelementptr inbounds i32, ptr @"POSY&", i16 %141
  %143 = load i32, ptr %142, !tbaa !2
  %144 = load i16, ptr %15, !tbaa !2
  %145 = sub i16 %144, 0
  %146 = getelementptr inbounds i32, ptr @"POSY&", i16 %145
  %147 = load i32, ptr %146, !tbaa !2
  %148 = sub i32 %143, %147
  store i32 %148, ptr %21, !tbaa !2
  %149 = load i32, ptr %22, !tbaa !2
  %150 = load i32, ptr %22, !tbaa !2
  %151 = mul i32 %149, %150
  %152 = load i32, ptr %21, !tbaa !2
  %153 = load i32, ptr %21, !tbaa !2
  %154 = mul i32 %152, %153
  %155 = add i32 %151, %154
  %156 = add i32 %155, 262144
  store i32 %156, ptr %20, !tbaa !2
  %157 = load i32, ptr %20, !tbaa !2
  br i1 false, label %b25, label %b26

b23:
  br label %b24

b24:
  %158 = load i16, ptr %7, !tbaa !2
  %159 = load i16, ptr %5, !tbaa !2
  %160 = add nsw i16 %158, %159
  store i16 %160, ptr %7, !tbaa !2
  br label %b17

b25:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b26:
  br i1 false, label %b27, label %b28

b27:
  %161 = sub i32 0, %157
  store i32 %161, ptr %4, !tbaa !2
  br label %b29

b28:
  %162 = sdiv i32 %157, 262144
  store i32 %162, ptr %4, !tbaa !2
  br label %b29

b29:
  %163 = load i32, ptr %4, !tbaa !2
  %164 = add i32 %163, 1
  %165 = icmp eq i32 %164, 0
  br i1 %165, label %b30, label %b31

b30:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b31:
  %166 = sdiv i32 512, %164
  store i32 %166, ptr %19, !tbaa !2
  %167 = load i32, ptr %18, !tbaa !2
  %168 = load i32, ptr %22, !tbaa !2
  %169 = load i32, ptr %19, !tbaa !2
  %170 = mul i32 %168, %169
  %171 = sdiv i32 %170, 512
  %172 = add i32 %167, %171
  store i32 %172, ptr %18, !tbaa !2
  %173 = load i32, ptr %17, !tbaa !2
  %174 = load i32, ptr %21, !tbaa !2
  %175 = load i32, ptr %19, !tbaa !2
  %176 = mul i32 %174, %175
  %177 = sdiv i32 %176, 512
  %178 = add i32 %173, %177
  store i32 %178, ptr %17, !tbaa !2
  br label %b24

b32:
  %179 = load i16, ptr %2, !tbaa !2
  %180 = icmp sge i16 %179, 0
  br i1 %180, label %b33, label %b34

b33:
  %181 = load i16, ptr %15, !tbaa !2
  %182 = load i16, ptr %3, !tbaa !2
  %183 = icmp sle i16 %181, %182
  br i1 %183, label %b35, label %b36

b34:
  %184 = load i16, ptr %15, !tbaa !2
  %185 = load i16, ptr %3, !tbaa !2
  %186 = icmp sge i16 %184, %185
  br i1 %186, label %b35, label %b36

b35:
  %187 = load i16, ptr %15, !tbaa !2
  %188 = load i16, ptr %15, !tbaa !2
  %189 = sub i16 %188, 0
  %190 = getelementptr inbounds i32, ptr @"POSX&", i16 %189
  %191 = load i32, ptr %190, !tbaa !2
  %192 = load i16, ptr %15, !tbaa !2
  %193 = sub i16 %192, 0
  %194 = getelementptr inbounds i32, ptr @"VELX&", i16 %193
  %195 = load i32, ptr %194, !tbaa !2
  %196 = add i32 %191, %195
  %197 = sub i16 %187, 0
  %198 = getelementptr inbounds i32, ptr @"POSX&", i16 %197
  store i32 %196, ptr %198, !tbaa !2
  %199 = load i16, ptr %15, !tbaa !2
  %200 = load i16, ptr %15, !tbaa !2
  %201 = sub i16 %200, 0
  %202 = getelementptr inbounds i32, ptr @"POSY&", i16 %201
  %203 = load i32, ptr %202, !tbaa !2
  %204 = load i16, ptr %15, !tbaa !2
  %205 = sub i16 %204, 0
  %206 = getelementptr inbounds i32, ptr @"VELY&", i16 %205
  %207 = load i32, ptr %206, !tbaa !2
  %208 = add i32 %203, %207
  %209 = sub i16 %199, 0
  %210 = getelementptr inbounds i32, ptr @"POSY&", i16 %209
  store i32 %208, ptr %210, !tbaa !2
  %211 = load i16, ptr %15, !tbaa !2
  %212 = load i16, ptr %2, !tbaa !2
  %213 = add nsw i16 %211, %212
  store i16 %213, ptr %15, !tbaa !2
  br label %b32

b36:
  %214 = load i16, ptr %12, !tbaa !2
  %215 = load i16, ptr %10, !tbaa !2
  %216 = add nsw i16 %214, %215
  store i16 %216, ptr %12, !tbaa !2
  br label %b7

b37:
  %217 = load i16, ptr %0, !tbaa !2
  %218 = icmp sge i16 %217, 0
  br i1 %218, label %b38, label %b39

b38:
  %219 = load i16, ptr %15, !tbaa !2
  %220 = load i16, ptr %1, !tbaa !2
  %221 = icmp sle i16 %219, %220
  br i1 %221, label %b40, label %b41

b39:
  %222 = load i16, ptr %15, !tbaa !2
  %223 = load i16, ptr %1, !tbaa !2
  %224 = icmp sge i16 %222, %223
  br i1 %224, label %b40, label %b41

b40:
  %225 = load i32, ptr %16, !tbaa !2
  %226 = load i16, ptr %15, !tbaa !2
  %227 = add i16 %226, 1
  %228 = load i16, ptr %15, !tbaa !2
  %229 = sub i16 %228, 0
  %230 = getelementptr inbounds i32, ptr @"POSX&", i16 %229
  %231 = load i32, ptr %230, !tbaa !2
  %232 = load i16, ptr %15, !tbaa !2
  %233 = sub i16 %232, 0
  %234 = getelementptr inbounds i32, ptr @"POSY&", i16 %233
  %235 = load i32, ptr %234, !tbaa !2
  %236 = mul i32 3, %235
  %237 = add i32 %231, %236
  %238 = load i16, ptr %15, !tbaa !2
  %239 = sub i16 %238, 0
  %240 = getelementptr inbounds i32, ptr @"VELX&", i16 %239
  %241 = load i32, ptr %240, !tbaa !2
  %242 = mul i32 5, %241
  %243 = add i32 %237, %242
  %244 = load i16, ptr %15, !tbaa !2
  %245 = sub i16 %244, 0
  %246 = getelementptr inbounds i32, ptr @"VELY&", i16 %245
  %247 = load i32, ptr %246, !tbaa !2
  %248 = mul i32 7, %247
  %249 = add i32 %243, %248
  %250 = sext i16 %227 to i32
  %251 = mul i32 %250, %249
  %252 = add i32 %225, %251
  store i32 %252, ptr %16, !tbaa !2
  %253 = load i16, ptr %15, !tbaa !2
  %254 = load i16, ptr %0, !tbaa !2
  %255 = add nsw i16 %253, %254
  store i16 %255, ptr %15, !tbaa !2
  br label %b37

b41:
  %256 = load i32, ptr %16, !tbaa !2
  store i32 %256, ptr %23, !tbaa !2
  br label %b42

b42:
  %257 = load i32, ptr %23, !tbaa !2
  ret i32 %257
}

declare cc 1000 ptr @llrm.qb.B$STI4(i32) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
