@L_b1 = private constant [2 x i8] c"m\00"
@L_b2 = private constant [24 x i8] c"mmnmmommommnonmmonqnmmo\00"
@L_b3 = private constant [52 x i8] c"abcdefghijklmnopqrstuvwxyzyxwvutsrqponmlkjihgfedcba\00"
@L_b4 = private constant [35 x i8] c"mmmmmaaaaammmmmaaaaaabcdefgabcdefg\00"
@L_b5 = private constant [13 x i8] c"mamamamamama\00"
@L_b6 = private constant [34 x i8] c"jklmnopqrstuvwxyzyxwvutsrqponmlkj\00"
@L_b7 = private constant [18 x i8] c"nmonqnmomnmomomno\00"
@L_b8 = private constant [29 x i8] c"mmmaaaabcdefgmmmmaaaammmaamm\00"
@L_b9 = private constant [43 x i8] c"mmmaaammmaaammmabcdefaaaammmmabcdefmmmaaaa\00"
@L_b10 = private constant [17 x i8] c"aaaaaaaazzzzzzzz\00"
@L_b11 = private constant [26 x i8] c"mmamammmmammamamaaamammma\00"
@L_b12 = private constant [37 x i8] c"abcdefghijklmnopqrrqponmlkjihgfedcba\00"
@L_b13 = private constant [2 x i8] c"a\00"
@L_b14 = private constant [3 x i8] c"az\00"
@_ls_world = internal constant <{ ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr }> <{ ptr @L_b1, ptr @L_b2, ptr @L_b3, ptr @L_b4, ptr @L_b5, ptr @L_b6, ptr @L_b7, ptr @L_b8, ptr @L_b9, ptr @L_b10, ptr @L_b11, ptr @L_b12 }>

declare cc 1005 i16 @"_ls_lchar@3"(i8) memory(none) willreturn nounwind norecurse

declare cc 1005 void @"_ls_init@3"(ptr nocapture) addrspace(1) memory(read, argmem: readwrite, inaccessiblemem: none)

declare cc 1005 void @"_ls_switch@3"(ptr nocapture writeonly, i16, i16) addrspace(1) memory(argmem: write) willreturn norecurse

declare cc 1005 void @"_ls_hold@3"(ptr nocapture writeonly) addrspace(1) memory(argmem: write) norecurse

declare cc 1005 i16 @"_ls_face_style@3"(i16, i16, i16) addrspace(1) memory(none) willreturn nounwind norecurse

declare cc 1005 i16 @"_ls_face_styles@3"(i16, i16) addrspace(1) memory(none) nounwind norecurse

declare cc 1005 i32 @"_ls_face_key@3"(ptr nocapture readonly, i16, i16) addrspace(1) memory(argmem: read) norecurse

declare cc 1005 void @"_ls_animate@3"(ptr nocapture, float) addrspace(1) memory(read, argmem: readwrite, inaccessiblemem: none) norecurse

declare cc 1005 i16 @"_ls_value@3"(ptr nocapture readonly, i16) addrspace(1) memory(argmem: read) willreturn norecurse

declare cc 1005 i16 @"_ls_scale_byte@3"(i16, i16) addrspace(1) memory(none) willreturn nounwind norecurse

declare cc 1005 i16 @"_ls_selftest@3"() addrspace(1) memory(read, argmem: none, inaccessiblemem: none)

declare i16 @_strlen(ptr nocapture) addrspace(1) memory(argmem: read)

define internal cc 1005 i16 @pipeline.body() memory(read, argmem: none, inaccessiblemem: none) {
b1:
  %0 = alloca i16
  %1 = alloca [516 x i8]
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i32
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i32
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  call cc 1005 addrspace(1) void @"_ls_init@3"(ptr %1)
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x3FA99999A0000000)
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x3FF0CCCCC0000000)
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x4000666660000000)
  %17 = getelementptr inbounds i8, ptr %1, i16 0
  %18 = getelementptr inbounds i8, ptr %17, i16 6
  %19 = load i16, ptr %18, !tbaa !9
  %20 = icmp ne i16 %19, 120
  %21 = zext i1 %20 to i16
  br i1 %20, label %b3, label %b2

b2:
  %22 = getelementptr inbounds i8, ptr %1, i16 0
  %23 = getelementptr inbounds i8, ptr %22, i16 240
  store ptr @L_b14, ptr %23, !tbaa !21
  %24 = getelementptr inbounds i8, ptr %1, i16 0
  %25 = getelementptr inbounds i8, ptr %24, i16 242
  store i16 2, ptr %25, !tbaa !9
  %26 = getelementptr inbounds i8, ptr %1, i16 0
  %27 = getelementptr inbounds i8, ptr %26, i16 244
  store i16 0, ptr %27, !tbaa !9
  %28 = call cc 1005 i16 @"_ls_lchar@3"(i8 signext 97)
  %29 = getelementptr inbounds i8, ptr %1, i16 0
  %30 = getelementptr inbounds i8, ptr %29, i16 246
  store i16 %28, ptr %30, !tbaa !9
  %31 = getelementptr inbounds i8, ptr %1, i16 512
  store float 0.000000e+00, ptr %31, !tbaa !15
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x3FB99999A0000000)
  %32 = getelementptr inbounds i8, ptr %1, i16 0
  %33 = getelementptr inbounds i8, ptr %32, i16 246
  %34 = load i16, ptr %33, !tbaa !9
  %35 = call cc 1005 i16 @"_ls_lchar@3"(i8 signext 122)
  %36 = icmp ne i16 %34, %35
  %37 = zext i1 %36 to i16
  br i1 %36, label %b6, label %b5

b3:
  store i16 -1, ptr %0, !tbaa !9
  br label %b4

b4:
  %38 = load i16, ptr %0, !tbaa !9
  ret i16 %38

b5:
  %39 = getelementptr inbounds i8, ptr %1, i16 0
  %40 = getelementptr inbounds i8, ptr %39, i16 244
  %41 = load i16, ptr %40, !tbaa !9
  %42 = icmp ne i16 %41, 1
  %43 = zext i1 %42 to i16
  br i1 %42, label %b8, label %b7

b6:
  store i16 -2, ptr %0, !tbaa !9
  br label %b4

b7:
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x3FC3333340000000)
  %44 = getelementptr inbounds i8, ptr %1, i16 0
  %45 = getelementptr inbounds i8, ptr %44, i16 244
  %46 = load i16, ptr %45, !tbaa !9
  %47 = icmp ne i16 %46, 1
  %48 = zext i1 %47 to i16
  br i1 %47, label %b10, label %b9

b8:
  store i16 -3, ptr %0, !tbaa !9
  br label %b4

b9:
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x3FC99999A0000000)
  %49 = getelementptr inbounds i8, ptr %1, i16 0
  %50 = getelementptr inbounds i8, ptr %49, i16 246
  %51 = load i16, ptr %50, !tbaa !9
  %52 = icmp ne i16 %51, %28
  %53 = zext i1 %52 to i16
  br i1 %52, label %b12, label %b11

b10:
  store i16 -4, ptr %0, !tbaa !9
  br label %b4

b11:
  %54 = getelementptr inbounds i8, ptr %1, i16 0
  %55 = getelementptr inbounds i8, ptr %54, i16 244
  %56 = load i16, ptr %55, !tbaa !9
  %57 = icmp ne i16 %56, 0
  %58 = zext i1 %57 to i16
  br i1 %57, label %b14, label %b13

b12:
  store i16 -5, ptr %0, !tbaa !9
  br label %b4

b13:
  %59 = getelementptr inbounds i8, ptr %1, i16 512
  store float 0.000000e+00, ptr %59, !tbaa !15
  %60 = getelementptr inbounds i8, ptr %1, i16 0
  %61 = getelementptr inbounds i8, ptr %60, i16 244
  store i16 0, ptr %61, !tbaa !9
  %62 = getelementptr inbounds i8, ptr %1, i16 0
  %63 = getelementptr inbounds i8, ptr %62, i16 246
  store i16 %28, ptr %63, !tbaa !9
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x3FA47AE140000000)
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 0x3FBC28F5C0000000)
  %64 = getelementptr inbounds i8, ptr %1, i16 0
  %65 = getelementptr inbounds i8, ptr %64, i16 244
  %66 = load i16, ptr %65, !tbaa !9
  %67 = icmp ne i16 %66, 1
  %68 = zext i1 %67 to i16
  br i1 %67, label %b16, label %b15

b14:
  store i16 -6, ptr %0, !tbaa !9
  br label %b4

b15:
  call cc 1005 addrspace(1) void @"_ls_init@3"(ptr %1)
  %69 = call cc 1005 addrspace(1) i32 @"_ls_face_key@3"(ptr %1, i16 2560, i16 5396)
  store i32 %69, ptr %2, !tbaa !11
  %70 = load i32, ptr %2, !tbaa !11
  %71 = icmp ne i32 %70, 531048
  %72 = zext i1 %71 to i16
  br i1 %71, label %b18, label %b17

b16:
  store i16 -7, ptr %0, !tbaa !9
  br label %b4

b17:
  call cc 1005 addrspace(1) void @"_ls_switch@3"(ptr %1, i16 21, i16 0)
  %73 = call cc 1005 addrspace(1) i32 @"_ls_face_key@3"(ptr %1, i16 2560, i16 5396)
  %74 = load i32, ptr %2, !tbaa !11
  %75 = icmp eq i32 %73, %74
  %76 = zext i1 %75 to i16
  br i1 %75, label %b20, label %b19

b18:
  store i16 -16, ptr %0, !tbaa !9
  br label %b4

b19:
  %77 = call cc 1005 i16 @"_ls_lchar@3"(i8 signext 110)
  %78 = getelementptr inbounds i8, ptr %1, i16 0
  %79 = getelementptr inbounds i8, ptr %78, i16 86
  store i16 %77, ptr %79, !tbaa !9
  %80 = call cc 1005 addrspace(1) i32 @"_ls_face_key@3"(ptr %1, i16 2560, i16 -1)
  %81 = icmp ne i32 %80, 363
  %82 = zext i1 %81 to i16
  br i1 %81, label %b22, label %b21

b20:
  store i16 -17, ptr %0, !tbaa !9
  br label %b4

b21:
  store i16 -1, ptr %5, !tbaa !9
  store i16 0, ptr %6, !tbaa !9
  %83 = getelementptr inbounds i8, ptr %1, i16 0
  %84 = getelementptr inbounds i8, ptr %83, i16 86
  %85 = load i16, ptr %5, !tbaa !9
  %86 = load i16, ptr %5, !tbaa !9
  %87 = getelementptr inbounds i8, ptr %1, i16 0
  %88 = getelementptr inbounds i8, ptr %1, i16 0
  br label %b23

b22:
  store i16 -18, ptr %0, !tbaa !9
  br label %b4

b23:
  %89 = load i16, ptr %6, !tbaa !9
  %90 = icmp slt i16 %89, 3
  %91 = zext i1 %90 to i16
  br i1 %90, label %b25, label %b24

b24:
  store i16 2560, ptr %4, !tbaa !9
  %92 = load i16, ptr %4, !tbaa !9
  %93 = and i16 2560, -256
  %94 = icmp eq i16 2560, -256
  %95 = zext i1 false to i16
  %96 = icmp ne i16 0, 0
  %97 = load i16, ptr %5, !tbaa !9
  %98 = load i16, ptr %4, !tbaa !9
  %99 = call cc 1005 addrspace(1) i32 @"_ls_face_key@3"(ptr %1, i16 2560, i16 -1)
  store i32 %99, ptr %13, !tbaa !11
  %100 = load i32, ptr %13, !tbaa !11
  %101 = load i16, ptr %5, !tbaa !9
  %102 = load i16, ptr %4, !tbaa !9
  %103 = call cc 1005 addrspace(1) i32 @"_ls_face_key@3"(ptr %1, i16 2560, i16 -1)
  %104 = icmp ne i32 %100, %103
  %105 = zext i1 %104 to i16
  br i1 %104, label %b64, label %b50

b25:
  %106 = load i16, ptr %6, !tbaa !9
  %107 = icmp eq i16 %106, 0
  %108 = zext i1 %107 to i16
  br i1 %107, label %b28, label %b26

b26:
  %109 = load i16, ptr %6, !tbaa !9
  %110 = icmp eq i16 %109, 1
  %111 = zext i1 %110 to i16
  br i1 %110, label %b31, label %b29

b27:
  %112 = load i16, ptr %7, !tbaa !9
  store i16 %112, ptr %84, !tbaa !9
  store i16 0, ptr %3, !tbaa !9
  br label %b32

b28:
  store i16 %28, ptr %7, !tbaa !9
  br label %b27

b29:
  store i16 116, ptr %8, !tbaa !9
  br label %b30

b30:
  %113 = load i16, ptr %8, !tbaa !9
  store i16 %113, ptr %7, !tbaa !9
  br label %b27

b31:
  %114 = call cc 1005 i16 @"_ls_lchar@3"(i8 signext 109)
  store i16 %114, ptr %8, !tbaa !9
  br label %b30

b32:
  %115 = load i16, ptr %3, !tbaa !9
  %116 = icmp sle i16 %115, 255
  %117 = zext i1 %116 to i16
  br i1 %116, label %b34, label %b33

b33:
  %118 = load i16, ptr %6, !tbaa !9
  %119 = add nsw i16 %118, 1
  store i16 %119, ptr %6, !tbaa !9
  br label %b23

b34:
  %120 = load i16, ptr %3, !tbaa !9
  %121 = or i16 %120, -256
  store i16 %121, ptr %4, !tbaa !9
  %122 = load i16, ptr %4, !tbaa !9
  %123 = and i16 %122, -256
  %124 = icmp eq i16 %123, -256
  %125 = zext i1 %124 to i16
  br i1 %124, label %b38, label %b36

b35:
  %126 = load i16, ptr %3, !tbaa !9
  %127 = add nsw i16 %126, 1
  store i16 %127, ptr %3, !tbaa !9
  br label %b32

b36:
  %128 = load i16, ptr %4, !tbaa !9
  %129 = call cc 1005 addrspace(1) i32 @"_ls_face_key@3"(ptr %1, i16 %128, i16 -1)
  store i32 %129, ptr %9, !tbaa !11
  br label %b37

b37:
  %130 = load i32, ptr %9, !tbaa !11
  %131 = load i16, ptr %4, !tbaa !9
  %132 = call cc 1005 addrspace(1) i32 @"_ls_face_key@3"(ptr %1, i16 %131, i16 -1)
  %133 = icmp ne i32 %130, %132
  %134 = zext i1 %133 to i16
  br i1 %133, label %b49, label %b35

b38:
  %135 = load i16, ptr %4, !tbaa !9
  %136 = and i16 %135, 255
  %137 = icmp ne i16 %136, 255
  %138 = zext i1 %137 to i16
  br i1 %137, label %b39, label %b36

b39:
  %139 = load i16, ptr %4, !tbaa !9
  %140 = and i16 %139, 255
  %141 = icmp sgt i16 %140, 63
  %142 = zext i1 %141 to i16
  br i1 %141, label %b44, label %b42

b40:
  %143 = load i16, ptr %4, !tbaa !9
  %144 = and i16 %143, 255
  %145 = icmp sgt i16 %144, 63
  %146 = zext i1 %145 to i16
  br i1 %145, label %b48, label %b46

b41:
  %147 = load i16, ptr %10, !tbaa !9
  %148 = sext i16 %147 to i32
  store i32 %148, ptr %9, !tbaa !11
  br label %b37

b42:
  %149 = load i16, ptr %4, !tbaa !9
  %150 = and i16 %149, 255
  store i16 %150, ptr %11, !tbaa !9
  br label %b43

b43:
  %151 = load i16, ptr %11, !tbaa !9
  %152 = mul i16 %151, 8
  %153 = getelementptr inbounds i8, ptr %87, i16 %152
  %154 = getelementptr inbounds i8, ptr %153, i16 6
  %155 = load i16, ptr %154, !tbaa !9
  %156 = icmp eq i16 %155, 116
  %157 = zext i1 %156 to i16
  br i1 %156, label %b45, label %b40

b44:
  store i16 0, ptr %11, !tbaa !9
  br label %b43

b45:
  store i16 26, ptr %10, !tbaa !9
  br label %b41

b46:
  %158 = load i16, ptr %4, !tbaa !9
  %159 = and i16 %158, 255
  store i16 %159, ptr %12, !tbaa !9
  br label %b47

b47:
  %160 = load i16, ptr %12, !tbaa !9
  %161 = mul i16 %160, 8
  %162 = getelementptr inbounds i8, ptr %88, i16 %161
  %163 = getelementptr inbounds i8, ptr %162, i16 6
  %164 = load i16, ptr %163, !tbaa !9
  %165 = sdiv i16 %164, 10
  store i16 %165, ptr %10, !tbaa !9
  br label %b41

b48:
  store i16 0, ptr %12, !tbaa !9
  br label %b47

b49:
  store i16 -19, ptr %0, !tbaa !9
  br label %b4

b50:
  %166 = call cc 1005 addrspace(1) i16 @"_ls_scale_byte@3"(i16 200, i16 120)
  %167 = icmp ne i16 %166, 200
  %168 = zext i1 %167 to i16
  br i1 %167, label %b66, label %b65

b64:
  store i16 -20, ptr %0, !tbaa !9
  br label %b4

b65:
  %169 = call cc 1005 addrspace(1) i16 @"_ls_scale_byte@3"(i16 200, i16 60)
  %170 = icmp ne i16 %169, 100
  %171 = zext i1 %170 to i16
  br i1 %170, label %b68, label %b67

b66:
  store i16 -8, ptr %0, !tbaa !9
  br label %b4

b67:
  %172 = call cc 1005 addrspace(1) i16 @"_ls_scale_byte@3"(i16 200, i16 240)
  %173 = icmp ne i16 %172, 255
  %174 = zext i1 %173 to i16
  br i1 %173, label %b70, label %b69

b68:
  store i16 -9, ptr %0, !tbaa !9
  br label %b4

b69:
  %175 = call cc 1005 addrspace(1) i16 @"_ls_scale_byte@3"(i16 200, i16 0)
  %176 = icmp ne i16 %175, 0
  %177 = zext i1 %176 to i16
  br i1 %176, label %b72, label %b71

b70:
  store i16 -10, ptr %0, !tbaa !9
  br label %b4

b71:
  call cc 1005 addrspace(1) void @"_ls_init@3"(ptr %1)
  %178 = getelementptr inbounds i8, ptr %1, i16 0
  %179 = getelementptr inbounds i8, ptr %178, i16 166
  %180 = load i16, ptr %179, !tbaa !9
  %181 = icmp ne i16 %180, 116
  %182 = zext i1 %181 to i16
  br i1 %181, label %b74, label %b73

b72:
  store i16 -11, ptr %0, !tbaa !9
  br label %b4

b73:
  %183 = getelementptr inbounds i8, ptr %1, i16 0
  %184 = getelementptr inbounds i8, ptr %183, i16 22
  %185 = load i16, ptr %184, !tbaa !9
  %186 = icmp ne i16 %185, %28
  %187 = zext i1 %186 to i16
  br i1 %186, label %b76, label %b75

b74:
  store i16 -12, ptr %0, !tbaa !9
  br label %b4

b75:
  call cc 1005 addrspace(1) void @"_ls_hold@3"(ptr %1)
  %188 = getelementptr inbounds i8, ptr %1, i16 0
  %189 = getelementptr inbounds i8, ptr %188, i16 22
  %190 = load i16, ptr %189, !tbaa !9
  %191 = icmp ne i16 %190, 120
  %192 = zext i1 %191 to i16
  br i1 %191, label %b78, label %b79

b76:
  store i16 -13, ptr %0, !tbaa !9
  br label %b4

b77:
  call cc 1005 addrspace(1) void @"_ls_animate@3"(ptr %1, float 5.000000e+00)
  %193 = getelementptr inbounds i8, ptr %1, i16 0
  %194 = getelementptr inbounds i8, ptr %193, i16 22
  %195 = load i16, ptr %194, !tbaa !9
  %196 = icmp ne i16 %195, 120
  %197 = zext i1 %196 to i16
  br i1 %196, label %b82, label %b81

b78:
  store i16 -14, ptr %0, !tbaa !9
  br label %b4

b79:
  %198 = getelementptr inbounds i8, ptr %1, i16 0
  %199 = getelementptr inbounds i8, ptr %198, i16 166
  %200 = load i16, ptr %199, !tbaa !9
  %201 = icmp ne i16 %200, 120
  %202 = zext i1 %201 to i16
  br i1 %201, label %b78, label %b77

b81:
  store i16 1, ptr %0, !tbaa !9
  br label %b4

b82:
  store i16 -15, ptr %0, !tbaa !9
  br label %b4
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"int4", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"int8", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"float4", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!"float8", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!"float10", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!"pointer2", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!"pointer4", !6, i64 0}
!23 = !{!22, !22, i64 0}
