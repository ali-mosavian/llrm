target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00closed \00"
@$str2 = internal constant [35 x i8] c"\08\00\1C\00\1C\0012 15 x 11 30 28 err 27 9 10\00"
@$str3 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00skipped \00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00feed\00"
@$str6 = internal constant [12 x i8] c"\08\00\05\00\05\00mean \00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00alarm\00"
@$str8 = internal constant [16 x i8] c"\08\00\09\00\09\00alarm at \00"
@$str9 = internal constant [11 x i8] c"\08\00\04\00\04\00done\00"

define internal cc 1005 void @Source.drop(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %1)
  %2 = load ptr, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PS(ptr %2)
  call cc 1005 addrspace(1) void @N$PN()
  ret void
}

define internal cc 1005 i32 @number(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  store i16 0, ptr %2, !tbaa !2
  %4 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %1, !tbaa !2
  %6 = icmp ult i16 %5, %4
  br i1 %6, label %b3, label %b5

b3:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  %9 = getelementptr inbounds i8, ptr addrspace(1) %8, i16 %5
  %10 = load i8, ptr addrspace(1) %9
  %11 = icmp ult i8 %10, 48
  br i1 %11, label %b6, label %b9

b4:
  %12 = load i16, ptr %1, !tbaa !2
  %13 = add i16 %12, 1
  store i16 %13, ptr %1, !tbaa !2
  br label %b2

b5:
  %14 = load i16, ptr %2, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %14, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %3 to ptr addrspace(1)
  %17 = load i32, ptr addrspace(1) %16, !tbaa !2
  ret i32 %17

b6:
  store i32 1, ptr %3, !tbaa !2
  %18 = addrspacecast ptr %3 to ptr addrspace(1)
  %19 = load i32, ptr addrspace(1) %18, !tbaa !2
  ret i32 %19

b7:
  br label %b8

b8:
  %20 = load i16, ptr %2, !tbaa !2
  %21 = mul i16 %20, 10
  %22 = load i8, ptr addrspace(1) %9
  %23 = zext i8 %22 to i16
  %24 = add i16 %21, %23
  %25 = sub i16 %24, 48
  store i16 %25, ptr %2, !tbaa !2
  br label %b4

b9:
  %26 = load i8, ptr addrspace(1) %9
  %27 = icmp ugt i8 %26, 57
  br i1 %27, label %b6, label %b7
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [4 x i8]
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [54 x i8]
  %8 = alloca i8
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [8 x i8]
  %12 = alloca [54 x i8]
  %13 = alloca i16
  %14 = alloca [4 x i8]
  %15 = alloca i8
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca [54 x i8]
  %21 = alloca i8
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca [8 x i8]
  %25 = alloca [54 x i8]
  %26 = alloca ptr
  %27 = getelementptr i8, ptr @$str2, i16 6
  store ptr %27, ptr %26, !tbaa !2
  %28 = getelementptr i8, ptr @$str5, i16 6
  %29 = load ptr, ptr %26, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 %31, ptr %24, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %24, i16 2
  store i16 %31, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %24, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %24 to ptr addrspace(1)
  %36 = getelementptr inbounds i8, ptr %25, i16 8
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  %38 = load i16, ptr addrspace(1) %35, !tbaa !2
  store i16 %38, ptr addrspace(1) %37
  %39 = getelementptr i8, ptr addrspace(1) %35, i16 2
  %40 = load i16, ptr addrspace(1) %39, !tbaa !2
  %41 = getelementptr i8, ptr addrspace(1) %37, i16 2
  store i16 %40, ptr addrspace(1) %41
  %42 = getelementptr i8, ptr addrspace(1) %35, i16 4
  %43 = load ptr addrspace(1), ptr addrspace(1) %42, !tbaa !2
  %44 = getelementptr i8, ptr addrspace(1) %37, i16 4
  store ptr addrspace(1) %43, ptr addrspace(1) %44
  store i16 0, ptr %25, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %25, i16 2
  store i16 0, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %25, i16 4
  store i16 0, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %25, i16 6
  store ptr %28, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %25, i16 16
  store ptr null, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %25, i16 18
  store i16 0, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %25, i16 20
  store i16 0, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %25, i16 22
  store i16 0, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %25, i16 24
  store ptr addrspace(1) null, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %25, i16 28
  store ptr null, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %25, i16 30
  store i16 0, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %25, i16 32
  store ptr null, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %25, i16 34
  store i8 0, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %25, i16 36
  store ptr null, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %25, i16 38
  store i16 0, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %25, i16 40
  store i8 1, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %25, i16 42
  %61 = addrspacecast ptr %60 to ptr addrspace(1)
  store i16 0, ptr %23, !tbaa !2
  store i16 3, ptr %22, !tbaa !2
  br label %b2

b2:
  %62 = load i16, ptr %23, !tbaa !2
  %63 = load i16, ptr %22, !tbaa !2
  %64 = icmp slt i16 %62, %63
  br i1 %64, label %b3, label %b5

b3:
  %65 = load i16, ptr %23, !tbaa !2
  %66 = mul i16 %65, 2
  %67 = getelementptr inbounds i8, ptr addrspace(1) %61, i16 %66
  store i16 0, ptr addrspace(1) %67
  br label %b4

b4:
  %68 = load i16, ptr %23, !tbaa !2
  %69 = add nsw i16 %68, 1
  store i16 %69, ptr %23, !tbaa !2
  br label %b2

b5:
  %70 = getelementptr inbounds i8, ptr %25, i16 48
  store i16 0, ptr %70, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %25, i16 50
  store i16 0, ptr %71, !tbaa !2
  %72 = getelementptr inbounds i8, ptr %25, i16 52
  store i8 1, ptr %72, !tbaa !2
  store i8 1, ptr %21, !tbaa !2
  %73 = load i16, ptr %25, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %25, i16 2
  %75 = load i16, ptr %74, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %25, i16 4
  %77 = load i16, ptr %76, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %25, i16 6
  %79 = load ptr, ptr %78, !tbaa !2
  %80 = getelementptr inbounds i8, ptr %25, i16 8
  %81 = load i16, ptr %80, !tbaa !2
  %82 = getelementptr inbounds i8, ptr %25, i16 10
  %83 = load i16, ptr %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %25, i16 12
  %85 = load ptr addrspace(1), ptr %84, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %25, i16 16
  %87 = load ptr, ptr %86, !tbaa !2
  %88 = getelementptr inbounds i8, ptr %25, i16 18
  %89 = load i16, ptr %88, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %25, i16 20
  %91 = load i16, ptr %90, !tbaa !2
  %92 = getelementptr inbounds i8, ptr %25, i16 22
  %93 = load i16, ptr %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %25, i16 24
  %95 = load ptr addrspace(1), ptr %94, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %25, i16 28
  %97 = load ptr, ptr %96, !tbaa !2
  %98 = getelementptr inbounds i8, ptr %25, i16 30
  %99 = load i16, ptr %98, !tbaa !2
  %100 = getelementptr inbounds i8, ptr %25, i16 32
  %101 = load ptr, ptr %100, !tbaa !2
  %102 = getelementptr inbounds i8, ptr %25, i16 34
  %103 = load i8, ptr %102, !tbaa !2, !range !5
  %104 = getelementptr inbounds i8, ptr %25, i16 36
  %105 = load ptr, ptr %104, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %25, i16 38
  %107 = load i16, ptr %106, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %25, i16 40
  %109 = load i8, ptr %108, !tbaa !2, !range !6
  %110 = getelementptr inbounds i8, ptr %25, i16 48
  %111 = load i16, ptr %110, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %25, i16 50
  %113 = load i16, ptr %112, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %25, i16 52
  %115 = load i8, ptr %114, !tbaa !2, !range !7
  store i8 0, ptr %21, !tbaa !2
  store i16 %73, ptr %20, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %20, i16 2
  store i16 %75, ptr %116, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %20, i16 4
  store i16 %77, ptr %117, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %20, i16 6
  store ptr %79, ptr %118, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %20, i16 8
  store i16 %81, ptr %119, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %20, i16 10
  store i16 %83, ptr %120, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %20, i16 12
  store ptr addrspace(1) %85, ptr %121, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %20, i16 16
  store ptr %87, ptr %122, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %20, i16 18
  store i16 %89, ptr %123, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %20, i16 20
  store i16 %91, ptr %124, !tbaa !2
  %125 = getelementptr inbounds i8, ptr %20, i16 22
  store i16 %93, ptr %125, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %20, i16 24
  store ptr addrspace(1) %95, ptr %126, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %20, i16 28
  store ptr %97, ptr %127, !tbaa !2
  %128 = getelementptr inbounds i8, ptr %20, i16 30
  store i16 %99, ptr %128, !tbaa !2
  %129 = getelementptr inbounds i8, ptr %20, i16 32
  store ptr %101, ptr %129, !tbaa !2
  %130 = getelementptr inbounds i8, ptr %20, i16 34
  store i8 %103, ptr %130, !tbaa !2
  %131 = getelementptr inbounds i8, ptr %20, i16 36
  store ptr %105, ptr %131, !tbaa !2
  %132 = getelementptr inbounds i8, ptr %20, i16 38
  store i16 %107, ptr %132, !tbaa !2
  %133 = getelementptr inbounds i8, ptr %20, i16 40
  store i8 %109, ptr %133, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %20, i16 42
  %135 = addrspacecast ptr %134 to ptr addrspace(1)
  %136 = getelementptr inbounds i8, ptr %25, i16 42
  %137 = addrspacecast ptr %136 to ptr addrspace(1)
  store i16 0, ptr %19, !tbaa !2
  store i16 3, ptr %18, !tbaa !2
  br label %b6

b6:
  %138 = load i16, ptr %19, !tbaa !2
  %139 = load i16, ptr %18, !tbaa !2
  %140 = icmp slt i16 %138, %139
  br i1 %140, label %b7, label %b9

b7:
  %141 = load i16, ptr %19, !tbaa !2
  %142 = mul i16 %141, 2
  %143 = getelementptr inbounds i8, ptr addrspace(1) %135, i16 %142
  %144 = load i16, ptr %19, !tbaa !2
  %145 = mul i16 %144, 2
  %146 = getelementptr inbounds i8, ptr addrspace(1) %137, i16 %145
  %147 = load i16, ptr addrspace(1) %146
  store i16 %147, ptr addrspace(1) %143
  br label %b8

b8:
  %148 = load i16, ptr %19, !tbaa !2
  %149 = add nsw i16 %148, 1
  store i16 %149, ptr %19, !tbaa !2
  br label %b6

b9:
  %150 = getelementptr inbounds i8, ptr %20, i16 48
  store i16 %111, ptr %150, !tbaa !2
  %151 = getelementptr inbounds i8, ptr %20, i16 50
  store i16 %113, ptr %151, !tbaa !2
  %152 = getelementptr inbounds i8, ptr %20, i16 52
  store i8 %115, ptr %152, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  %153 = getelementptr inbounds i8, ptr %25, i16 2
  store i16 0, ptr %153, !tbaa !2
  %154 = getelementptr inbounds i8, ptr %25, i16 4
  store i16 0, ptr %154, !tbaa !2
  %155 = getelementptr inbounds i8, ptr %25, i16 6
  store ptr null, ptr %155, !tbaa !2
  %156 = getelementptr inbounds i8, ptr %25, i16 8
  store i16 0, ptr %156, !tbaa !2
  %157 = getelementptr inbounds i8, ptr %25, i16 10
  store i16 0, ptr %157, !tbaa !2
  %158 = getelementptr inbounds i8, ptr %25, i16 12
  store ptr addrspace(1) null, ptr %158, !tbaa !2
  %159 = getelementptr inbounds i8, ptr %25, i16 16
  store ptr null, ptr %159, !tbaa !2
  %160 = getelementptr inbounds i8, ptr %25, i16 18
  store i16 0, ptr %160, !tbaa !2
  %161 = getelementptr inbounds i8, ptr %25, i16 20
  store i16 0, ptr %161, !tbaa !2
  %162 = getelementptr inbounds i8, ptr %25, i16 22
  store i16 0, ptr %162, !tbaa !2
  %163 = getelementptr inbounds i8, ptr %25, i16 24
  store ptr addrspace(1) null, ptr %163, !tbaa !2
  %164 = getelementptr inbounds i8, ptr %25, i16 28
  store ptr null, ptr %164, !tbaa !2
  %165 = getelementptr inbounds i8, ptr %25, i16 30
  store i16 0, ptr %165, !tbaa !2
  %166 = getelementptr inbounds i8, ptr %25, i16 32
  store ptr null, ptr %166, !tbaa !2
  %167 = getelementptr inbounds i8, ptr %25, i16 34
  store i8 0, ptr %167, !tbaa !2
  %168 = getelementptr inbounds i8, ptr %25, i16 36
  store ptr null, ptr %168, !tbaa !2
  %169 = getelementptr inbounds i8, ptr %25, i16 38
  store i16 0, ptr %169, !tbaa !2
  %170 = getelementptr inbounds i8, ptr %25, i16 40
  store i8 0, ptr %170, !tbaa !2
  %171 = getelementptr inbounds i8, ptr %25, i16 42
  %172 = addrspacecast ptr %171 to ptr addrspace(1)
  store i16 0, ptr %17, !tbaa !2
  store i16 3, ptr %16, !tbaa !2
  br label %b10

b10:
  %173 = load i16, ptr %17, !tbaa !2
  %174 = load i16, ptr %16, !tbaa !2
  %175 = icmp slt i16 %173, %174
  br i1 %175, label %b11, label %b13

b11:
  %176 = load i16, ptr %17, !tbaa !2
  %177 = mul i16 %176, 2
  %178 = getelementptr inbounds i8, ptr addrspace(1) %172, i16 %177
  store i16 0, ptr addrspace(1) %178
  br label %b12

b12:
  %179 = load i16, ptr %17, !tbaa !2
  %180 = add nsw i16 %179, 1
  store i16 %180, ptr %17, !tbaa !2
  br label %b10

b13:
  %181 = getelementptr inbounds i8, ptr %25, i16 48
  store i16 0, ptr %181, !tbaa !2
  %182 = getelementptr inbounds i8, ptr %25, i16 50
  store i16 0, ptr %182, !tbaa !2
  %183 = getelementptr inbounds i8, ptr %25, i16 52
  store i8 0, ptr %183, !tbaa !2
  store i8 1, ptr %15, !tbaa !2
  br label %b14

b14:
  br label %b15

b15:
  %184 = addrspacecast ptr %20 to ptr addrspace(1)
  %185 = call cc 1005 addrspace(1) i32 @$state3.next(ptr addrspace(1) %184)
  %186 = addrspacecast ptr %14 to ptr addrspace(1)
  store i32 %185, ptr addrspace(1) %186, !tbaa !2
  %187 = load i8, ptr %14, !tbaa !2, !range !34
  %188 = icmp eq i8 %187, 0
  br i1 %188, label %b19, label %b18

b16:
  %189 = load i8, ptr %15, !tbaa !2, !range !8
  %190 = icmp ne i8 %189, 0
  br i1 %190, label %b22, label %b21

b17:
  br label %b14

b18:
  br label %b16

b19:
  %191 = getelementptr inbounds i8, ptr %14, i16 2
  %192 = load i16, ptr %191, !tbaa !2
  %193 = getelementptr inbounds i8, ptr %14, i16 2
  %194 = load i16, ptr %193, !tbaa !2
  store i16 %194, ptr %13, !tbaa !2
  %195 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %195)
  %196 = load i16, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %196)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b17

b21:
  %197 = getelementptr i8, ptr @$str7, i16 6
  %198 = load ptr, ptr %26, !tbaa !2
  %199 = getelementptr i8, ptr %198, i16 -4
  %200 = load i16, ptr %199
  %201 = addrspacecast ptr %198 to ptr addrspace(1)
  store i16 %200, ptr %11, !tbaa !2
  %202 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %200, ptr %202, !tbaa !2
  %203 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %201, ptr %203, !tbaa !2
  %204 = addrspacecast ptr %11 to ptr addrspace(1)
  %205 = getelementptr inbounds i8, ptr %12, i16 8
  %206 = addrspacecast ptr %205 to ptr addrspace(1)
  %207 = load i16, ptr addrspace(1) %204, !tbaa !2
  store i16 %207, ptr addrspace(1) %206
  %208 = getelementptr i8, ptr addrspace(1) %204, i16 2
  %209 = load i16, ptr addrspace(1) %208, !tbaa !2
  %210 = getelementptr i8, ptr addrspace(1) %206, i16 2
  store i16 %209, ptr addrspace(1) %210
  %211 = getelementptr i8, ptr addrspace(1) %204, i16 4
  %212 = load ptr addrspace(1), ptr addrspace(1) %211, !tbaa !2
  %213 = getelementptr i8, ptr addrspace(1) %206, i16 4
  store ptr addrspace(1) %212, ptr addrspace(1) %213
  store i16 0, ptr %12, !tbaa !2
  %214 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 0, ptr %214, !tbaa !2
  %215 = getelementptr inbounds i8, ptr %12, i16 4
  store i16 0, ptr %215, !tbaa !2
  %216 = getelementptr inbounds i8, ptr %12, i16 6
  store ptr %197, ptr %216, !tbaa !2
  %217 = getelementptr inbounds i8, ptr %12, i16 16
  store ptr null, ptr %217, !tbaa !2
  %218 = getelementptr inbounds i8, ptr %12, i16 18
  store i16 0, ptr %218, !tbaa !2
  %219 = getelementptr inbounds i8, ptr %12, i16 20
  store i16 0, ptr %219, !tbaa !2
  %220 = getelementptr inbounds i8, ptr %12, i16 22
  store i16 0, ptr %220, !tbaa !2
  %221 = getelementptr inbounds i8, ptr %12, i16 24
  store ptr addrspace(1) null, ptr %221, !tbaa !2
  %222 = getelementptr inbounds i8, ptr %12, i16 28
  store ptr null, ptr %222, !tbaa !2
  %223 = getelementptr inbounds i8, ptr %12, i16 30
  store i16 0, ptr %223, !tbaa !2
  %224 = getelementptr inbounds i8, ptr %12, i16 32
  store ptr null, ptr %224, !tbaa !2
  %225 = getelementptr inbounds i8, ptr %12, i16 34
  store i8 0, ptr %225, !tbaa !2
  %226 = getelementptr inbounds i8, ptr %12, i16 36
  store ptr null, ptr %226, !tbaa !2
  %227 = getelementptr inbounds i8, ptr %12, i16 38
  store i16 0, ptr %227, !tbaa !2
  %228 = getelementptr inbounds i8, ptr %12, i16 40
  store i8 1, ptr %228, !tbaa !2
  %229 = getelementptr inbounds i8, ptr %12, i16 42
  %230 = addrspacecast ptr %229 to ptr addrspace(1)
  store i16 0, ptr %10, !tbaa !2
  store i16 3, ptr %9, !tbaa !2
  br label %b29

b22:
  %231 = getelementptr inbounds i8, ptr %20, i16 52
  %232 = load i8, ptr %231, !tbaa !2, !range !9
  %233 = icmp ne i8 %232, 0
  br i1 %233, label %b24, label %b23

b23:
  br label %b21

b24:
  %234 = getelementptr inbounds i8, ptr %20, i16 36
  %235 = load ptr, ptr %234, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %235)
  %236 = getelementptr inbounds i8, ptr %20, i16 40
  %237 = load i8, ptr %236, !tbaa !2, !range !10
  %238 = icmp ne i8 %237, 0
  br i1 %238, label %b26, label %b25

b25:
  br label %b23

b26:
  %239 = getelementptr inbounds i8, ptr %20, i16 6
  %240 = load ptr, ptr %239, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %240)
  %241 = getelementptr inbounds i8, ptr %20, i16 34
  %242 = load i8, ptr %241, !tbaa !2, !range !11
  %243 = icmp ne i8 %242, 0
  br i1 %243, label %b28, label %b27

b27:
  %244 = getelementptr inbounds i8, ptr %20, i16 28
  %245 = load ptr, ptr %244, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %245)
  %246 = getelementptr inbounds i8, ptr %20, i16 32
  %247 = load ptr, ptr %246, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %247)
  br label %b25

b28:
  %248 = getelementptr inbounds i8, ptr %20, i16 16
  %249 = addrspacecast ptr %248 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %249)
  %250 = getelementptr inbounds i8, ptr %20, i16 16
  %251 = load ptr, ptr %250, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %251)
  br label %b27

b29:
  %252 = load i16, ptr %10, !tbaa !2
  %253 = load i16, ptr %9, !tbaa !2
  %254 = icmp slt i16 %252, %253
  br i1 %254, label %b30, label %b32

b30:
  %255 = load i16, ptr %10, !tbaa !2
  %256 = mul i16 %255, 2
  %257 = getelementptr inbounds i8, ptr addrspace(1) %230, i16 %256
  store i16 0, ptr addrspace(1) %257
  br label %b31

b31:
  %258 = load i16, ptr %10, !tbaa !2
  %259 = add nsw i16 %258, 1
  store i16 %259, ptr %10, !tbaa !2
  br label %b29

b32:
  %260 = getelementptr inbounds i8, ptr %12, i16 48
  store i16 0, ptr %260, !tbaa !2
  %261 = getelementptr inbounds i8, ptr %12, i16 50
  store i16 0, ptr %261, !tbaa !2
  %262 = getelementptr inbounds i8, ptr %12, i16 52
  store i8 1, ptr %262, !tbaa !2
  store i8 1, ptr %8, !tbaa !2
  %263 = load i16, ptr %12, !tbaa !2
  %264 = getelementptr inbounds i8, ptr %12, i16 2
  %265 = load i16, ptr %264, !tbaa !2
  %266 = getelementptr inbounds i8, ptr %12, i16 4
  %267 = load i16, ptr %266, !tbaa !2
  %268 = getelementptr inbounds i8, ptr %12, i16 6
  %269 = load ptr, ptr %268, !tbaa !2
  %270 = getelementptr inbounds i8, ptr %12, i16 8
  %271 = load i16, ptr %270, !tbaa !2
  %272 = getelementptr inbounds i8, ptr %12, i16 10
  %273 = load i16, ptr %272, !tbaa !2
  %274 = getelementptr inbounds i8, ptr %12, i16 12
  %275 = load ptr addrspace(1), ptr %274, !tbaa !2
  %276 = getelementptr inbounds i8, ptr %12, i16 16
  %277 = load ptr, ptr %276, !tbaa !2
  %278 = getelementptr inbounds i8, ptr %12, i16 18
  %279 = load i16, ptr %278, !tbaa !2
  %280 = getelementptr inbounds i8, ptr %12, i16 20
  %281 = load i16, ptr %280, !tbaa !2
  %282 = getelementptr inbounds i8, ptr %12, i16 22
  %283 = load i16, ptr %282, !tbaa !2
  %284 = getelementptr inbounds i8, ptr %12, i16 24
  %285 = load ptr addrspace(1), ptr %284, !tbaa !2
  %286 = getelementptr inbounds i8, ptr %12, i16 28
  %287 = load ptr, ptr %286, !tbaa !2
  %288 = getelementptr inbounds i8, ptr %12, i16 30
  %289 = load i16, ptr %288, !tbaa !2
  %290 = getelementptr inbounds i8, ptr %12, i16 32
  %291 = load ptr, ptr %290, !tbaa !2
  %292 = getelementptr inbounds i8, ptr %12, i16 34
  %293 = load i8, ptr %292, !tbaa !2, !range !12
  %294 = getelementptr inbounds i8, ptr %12, i16 36
  %295 = load ptr, ptr %294, !tbaa !2
  %296 = getelementptr inbounds i8, ptr %12, i16 38
  %297 = load i16, ptr %296, !tbaa !2
  %298 = getelementptr inbounds i8, ptr %12, i16 40
  %299 = load i8, ptr %298, !tbaa !2, !range !13
  %300 = getelementptr inbounds i8, ptr %12, i16 48
  %301 = load i16, ptr %300, !tbaa !2
  %302 = getelementptr inbounds i8, ptr %12, i16 50
  %303 = load i16, ptr %302, !tbaa !2
  %304 = getelementptr inbounds i8, ptr %12, i16 52
  %305 = load i8, ptr %304, !tbaa !2, !range !14
  store i8 0, ptr %8, !tbaa !2
  store i16 %263, ptr %7, !tbaa !2
  %306 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %265, ptr %306, !tbaa !2
  %307 = getelementptr inbounds i8, ptr %7, i16 4
  store i16 %267, ptr %307, !tbaa !2
  %308 = getelementptr inbounds i8, ptr %7, i16 6
  store ptr %269, ptr %308, !tbaa !2
  %309 = getelementptr inbounds i8, ptr %7, i16 8
  store i16 %271, ptr %309, !tbaa !2
  %310 = getelementptr inbounds i8, ptr %7, i16 10
  store i16 %273, ptr %310, !tbaa !2
  %311 = getelementptr inbounds i8, ptr %7, i16 12
  store ptr addrspace(1) %275, ptr %311, !tbaa !2
  %312 = getelementptr inbounds i8, ptr %7, i16 16
  store ptr %277, ptr %312, !tbaa !2
  %313 = getelementptr inbounds i8, ptr %7, i16 18
  store i16 %279, ptr %313, !tbaa !2
  %314 = getelementptr inbounds i8, ptr %7, i16 20
  store i16 %281, ptr %314, !tbaa !2
  %315 = getelementptr inbounds i8, ptr %7, i16 22
  store i16 %283, ptr %315, !tbaa !2
  %316 = getelementptr inbounds i8, ptr %7, i16 24
  store ptr addrspace(1) %285, ptr %316, !tbaa !2
  %317 = getelementptr inbounds i8, ptr %7, i16 28
  store ptr %287, ptr %317, !tbaa !2
  %318 = getelementptr inbounds i8, ptr %7, i16 30
  store i16 %289, ptr %318, !tbaa !2
  %319 = getelementptr inbounds i8, ptr %7, i16 32
  store ptr %291, ptr %319, !tbaa !2
  %320 = getelementptr inbounds i8, ptr %7, i16 34
  store i8 %293, ptr %320, !tbaa !2
  %321 = getelementptr inbounds i8, ptr %7, i16 36
  store ptr %295, ptr %321, !tbaa !2
  %322 = getelementptr inbounds i8, ptr %7, i16 38
  store i16 %297, ptr %322, !tbaa !2
  %323 = getelementptr inbounds i8, ptr %7, i16 40
  store i8 %299, ptr %323, !tbaa !2
  %324 = getelementptr inbounds i8, ptr %7, i16 42
  %325 = addrspacecast ptr %324 to ptr addrspace(1)
  %326 = getelementptr inbounds i8, ptr %12, i16 42
  %327 = addrspacecast ptr %326 to ptr addrspace(1)
  store i16 0, ptr %6, !tbaa !2
  store i16 3, ptr %5, !tbaa !2
  br label %b33

b33:
  %328 = load i16, ptr %6, !tbaa !2
  %329 = load i16, ptr %5, !tbaa !2
  %330 = icmp slt i16 %328, %329
  br i1 %330, label %b34, label %b36

b34:
  %331 = load i16, ptr %6, !tbaa !2
  %332 = mul i16 %331, 2
  %333 = getelementptr inbounds i8, ptr addrspace(1) %325, i16 %332
  %334 = load i16, ptr %6, !tbaa !2
  %335 = mul i16 %334, 2
  %336 = getelementptr inbounds i8, ptr addrspace(1) %327, i16 %335
  %337 = load i16, ptr addrspace(1) %336
  store i16 %337, ptr addrspace(1) %333
  br label %b35

b35:
  %338 = load i16, ptr %6, !tbaa !2
  %339 = add nsw i16 %338, 1
  store i16 %339, ptr %6, !tbaa !2
  br label %b33

b36:
  %340 = getelementptr inbounds i8, ptr %7, i16 48
  store i16 %301, ptr %340, !tbaa !2
  %341 = getelementptr inbounds i8, ptr %7, i16 50
  store i16 %303, ptr %341, !tbaa !2
  %342 = getelementptr inbounds i8, ptr %7, i16 52
  store i8 %305, ptr %342, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  %343 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 0, ptr %343, !tbaa !2
  %344 = getelementptr inbounds i8, ptr %12, i16 4
  store i16 0, ptr %344, !tbaa !2
  %345 = getelementptr inbounds i8, ptr %12, i16 6
  store ptr null, ptr %345, !tbaa !2
  %346 = getelementptr inbounds i8, ptr %12, i16 8
  store i16 0, ptr %346, !tbaa !2
  %347 = getelementptr inbounds i8, ptr %12, i16 10
  store i16 0, ptr %347, !tbaa !2
  %348 = getelementptr inbounds i8, ptr %12, i16 12
  store ptr addrspace(1) null, ptr %348, !tbaa !2
  %349 = getelementptr inbounds i8, ptr %12, i16 16
  store ptr null, ptr %349, !tbaa !2
  %350 = getelementptr inbounds i8, ptr %12, i16 18
  store i16 0, ptr %350, !tbaa !2
  %351 = getelementptr inbounds i8, ptr %12, i16 20
  store i16 0, ptr %351, !tbaa !2
  %352 = getelementptr inbounds i8, ptr %12, i16 22
  store i16 0, ptr %352, !tbaa !2
  %353 = getelementptr inbounds i8, ptr %12, i16 24
  store ptr addrspace(1) null, ptr %353, !tbaa !2
  %354 = getelementptr inbounds i8, ptr %12, i16 28
  store ptr null, ptr %354, !tbaa !2
  %355 = getelementptr inbounds i8, ptr %12, i16 30
  store i16 0, ptr %355, !tbaa !2
  %356 = getelementptr inbounds i8, ptr %12, i16 32
  store ptr null, ptr %356, !tbaa !2
  %357 = getelementptr inbounds i8, ptr %12, i16 34
  store i8 0, ptr %357, !tbaa !2
  %358 = getelementptr inbounds i8, ptr %12, i16 36
  store ptr null, ptr %358, !tbaa !2
  %359 = getelementptr inbounds i8, ptr %12, i16 38
  store i16 0, ptr %359, !tbaa !2
  %360 = getelementptr inbounds i8, ptr %12, i16 40
  store i8 0, ptr %360, !tbaa !2
  %361 = getelementptr inbounds i8, ptr %12, i16 42
  %362 = addrspacecast ptr %361 to ptr addrspace(1)
  store i16 0, ptr %4, !tbaa !2
  store i16 3, ptr %3, !tbaa !2
  br label %b37

b37:
  %363 = load i16, ptr %4, !tbaa !2
  %364 = load i16, ptr %3, !tbaa !2
  %365 = icmp slt i16 %363, %364
  br i1 %365, label %b38, label %b40

b38:
  %366 = load i16, ptr %4, !tbaa !2
  %367 = mul i16 %366, 2
  %368 = getelementptr inbounds i8, ptr addrspace(1) %362, i16 %367
  store i16 0, ptr addrspace(1) %368
  br label %b39

b39:
  %369 = load i16, ptr %4, !tbaa !2
  %370 = add nsw i16 %369, 1
  store i16 %370, ptr %4, !tbaa !2
  br label %b37

b40:
  %371 = getelementptr inbounds i8, ptr %12, i16 48
  store i16 0, ptr %371, !tbaa !2
  %372 = getelementptr inbounds i8, ptr %12, i16 50
  store i16 0, ptr %372, !tbaa !2
  %373 = getelementptr inbounds i8, ptr %12, i16 52
  store i8 0, ptr %373, !tbaa !2
  store i8 1, ptr %2, !tbaa !2
  br label %b41

b41:
  br label %b42

b42:
  %374 = addrspacecast ptr %7 to ptr addrspace(1)
  %375 = call cc 1005 addrspace(1) i32 @$state3.next(ptr addrspace(1) %374)
  %376 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %375, ptr addrspace(1) %376, !tbaa !2
  %377 = load i8, ptr %1, !tbaa !2, !range !34
  %378 = icmp eq i8 %377, 0
  br i1 %378, label %b46, label %b45

b43:
  %379 = load i8, ptr %2, !tbaa !2, !range !15
  %380 = icmp ne i8 %379, 0
  br i1 %380, label %b52, label %b51

b44:
  br label %b41

b45:
  br label %b43

b46:
  %381 = getelementptr inbounds i8, ptr %1, i16 2
  %382 = load i16, ptr %381, !tbaa !2
  %383 = getelementptr inbounds i8, ptr %1, i16 2
  %384 = load i16, ptr %383, !tbaa !2
  store i16 %384, ptr %0, !tbaa !2
  %385 = load i16, ptr %0, !tbaa !2
  %386 = icmp sgt i16 %385, 20
  br i1 %386, label %b47, label %b48

b47:
  %387 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %387)
  %388 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %388)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b43

b48:
  br label %b49

b49:
  br label %b44

b51:
  %389 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %389)
  call cc 1005 addrspace(1) void @N$PN()
  %390 = load i8, ptr %8, !tbaa !2, !range !16
  %391 = icmp ne i8 %390, 0
  br i1 %391, label %b60, label %b59

b52:
  %392 = getelementptr inbounds i8, ptr %7, i16 52
  %393 = load i8, ptr %392, !tbaa !2, !range !17
  %394 = icmp ne i8 %393, 0
  br i1 %394, label %b54, label %b53

b53:
  br label %b51

b54:
  %395 = getelementptr inbounds i8, ptr %7, i16 36
  %396 = load ptr, ptr %395, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %396)
  %397 = getelementptr inbounds i8, ptr %7, i16 40
  %398 = load i8, ptr %397, !tbaa !2, !range !18
  %399 = icmp ne i8 %398, 0
  br i1 %399, label %b56, label %b55

b55:
  br label %b53

b56:
  %400 = getelementptr inbounds i8, ptr %7, i16 6
  %401 = load ptr, ptr %400, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %401)
  %402 = getelementptr inbounds i8, ptr %7, i16 34
  %403 = load i8, ptr %402, !tbaa !2, !range !19
  %404 = icmp ne i8 %403, 0
  br i1 %404, label %b58, label %b57

b57:
  %405 = getelementptr inbounds i8, ptr %7, i16 28
  %406 = load ptr, ptr %405, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %406)
  %407 = getelementptr inbounds i8, ptr %7, i16 32
  %408 = load ptr, ptr %407, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %408)
  br label %b55

b58:
  %409 = getelementptr inbounds i8, ptr %7, i16 16
  %410 = addrspacecast ptr %409 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %410)
  %411 = getelementptr inbounds i8, ptr %7, i16 16
  %412 = load ptr, ptr %411, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %412)
  br label %b57

b59:
  %413 = load i8, ptr %21, !tbaa !2, !range !20
  %414 = icmp ne i8 %413, 0
  br i1 %414, label %b68, label %b67

b60:
  %415 = getelementptr inbounds i8, ptr %12, i16 52
  %416 = load i8, ptr %415, !tbaa !2, !range !21
  %417 = icmp ne i8 %416, 0
  br i1 %417, label %b62, label %b61

b61:
  br label %b59

b62:
  %418 = getelementptr inbounds i8, ptr %12, i16 36
  %419 = load ptr, ptr %418, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %419)
  %420 = getelementptr inbounds i8, ptr %12, i16 40
  %421 = load i8, ptr %420, !tbaa !2, !range !22
  %422 = icmp ne i8 %421, 0
  br i1 %422, label %b64, label %b63

b63:
  br label %b61

b64:
  %423 = getelementptr inbounds i8, ptr %12, i16 6
  %424 = load ptr, ptr %423, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %424)
  %425 = getelementptr inbounds i8, ptr %12, i16 34
  %426 = load i8, ptr %425, !tbaa !2, !range !23
  %427 = icmp ne i8 %426, 0
  br i1 %427, label %b66, label %b65

b65:
  %428 = getelementptr inbounds i8, ptr %12, i16 28
  %429 = load ptr, ptr %428, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %429)
  %430 = getelementptr inbounds i8, ptr %12, i16 32
  %431 = load ptr, ptr %430, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %431)
  br label %b63

b66:
  %432 = getelementptr inbounds i8, ptr %12, i16 16
  %433 = addrspacecast ptr %432 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %433)
  %434 = getelementptr inbounds i8, ptr %12, i16 16
  %435 = load ptr, ptr %434, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %435)
  br label %b65

b67:
  %436 = load ptr, ptr %26, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %436)
  ret i16 0

b68:
  %437 = getelementptr inbounds i8, ptr %25, i16 52
  %438 = load i8, ptr %437, !tbaa !2, !range !24
  %439 = icmp ne i8 %438, 0
  br i1 %439, label %b70, label %b69

b69:
  br label %b67

b70:
  %440 = getelementptr inbounds i8, ptr %25, i16 36
  %441 = load ptr, ptr %440, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %441)
  %442 = getelementptr inbounds i8, ptr %25, i16 40
  %443 = load i8, ptr %442, !tbaa !2, !range !25
  %444 = icmp ne i8 %443, 0
  br i1 %444, label %b72, label %b71

b71:
  br label %b69

b72:
  %445 = getelementptr inbounds i8, ptr %25, i16 6
  %446 = load ptr, ptr %445, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %446)
  %447 = getelementptr inbounds i8, ptr %25, i16 34
  %448 = load i8, ptr %447, !tbaa !2, !range !26
  %449 = icmp ne i8 %448, 0
  br i1 %449, label %b74, label %b73

b73:
  %450 = getelementptr inbounds i8, ptr %25, i16 28
  %451 = load ptr, ptr %450, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %451)
  %452 = getelementptr inbounds i8, ptr %25, i16 32
  %453 = load ptr, ptr %452, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %453)
  br label %b71

b74:
  %454 = getelementptr inbounds i8, ptr %25, i16 16
  %455 = addrspacecast ptr %454 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %455)
  %456 = getelementptr inbounds i8, ptr %25, i16 16
  %457 = load ptr, ptr %456, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %457)
  br label %b73
}

define internal cc 1005 i32 @$state3.next(ptr addrspace(1) nonnull dereferenceable(54) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  br label %b2

b2:
  br label %b3

b3:
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b7, label %b6

b6:
  %6 = icmp eq i16 %4, 2
  br i1 %6, label %b9, label %b8

b7:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 42
  store i16 0, ptr addrspace(1) %7
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 44
  store i16 0, ptr addrspace(1) %8
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 46
  store i16 0, ptr addrspace(1) %9
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 48
  store i16 0, ptr addrspace(1) %10
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %11 = icmp eq i16 %4, 3
  br i1 %11, label %b15, label %b14

b9:
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %13 = call cc 1005 addrspace(1) i32 @$state2.next(ptr addrspace(1) %12)
  %14 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 %13, ptr addrspace(1) %14, !tbaa !2
  %15 = load i8, ptr %2, !tbaa !2, !range !35
  %16 = icmp eq i8 %15, 0
  br i1 %16, label %b12, label %b11

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b12:
  %17 = getelementptr inbounds i8, ptr %2, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %2, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  store i16 %20, ptr %1, !tbaa !2
  %21 = load i16, ptr %1, !tbaa !2
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 50
  store i16 %21, ptr addrspace(1) %22
  store i16 3, ptr addrspace(1) %0
  br label %b2

b14:
  %23 = icmp eq i16 %4, 4
  br i1 %23, label %b22, label %b21

b15:
  %24 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 48
  %26 = load i16, ptr addrspace(1) %25
  %27 = urem i16 %26, 3
  %28 = icmp ult i16 %27, 3
  br i1 %28, label %b16, label %b17

b16:
  %29 = mul i16 %27, 2
  %30 = getelementptr inbounds i8, ptr addrspace(1) %24, i16 %29
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 50
  %32 = load i16, ptr addrspace(1) %31
  store i16 %32, ptr addrspace(1) %30
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 48
  %34 = load i16, ptr addrspace(1) %33
  %35 = add i16 %34, 1
  %36 = getelementptr i8, ptr addrspace(1) %0, i16 48
  store i16 %35, ptr addrspace(1) %36
  %37 = getelementptr i8, ptr addrspace(1) %0, i16 48
  %38 = load i16, ptr addrspace(1) %37
  %39 = icmp uge i16 %38, 3
  br i1 %39, label %b18, label %b19

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b19:
  store i16 6, ptr addrspace(1) %0
  br label %b2

b21:
  %40 = icmp eq i16 %4, 5
  br i1 %40, label %b30, label %b29

b22:
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 52
  %42 = load i8, ptr addrspace(1) %41, !range !27
  %43 = icmp ne i8 %42, 0
  br i1 %43, label %b24, label %b23

b23:
  %44 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %44
  %45 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr addrspace(1) %45
  %46 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store ptr null, ptr addrspace(1) %46
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 8
  store i16 0, ptr addrspace(1) %47
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store i16 0, ptr addrspace(1) %48
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 12
  store ptr addrspace(1) null, ptr addrspace(1) %49
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 16
  store ptr null, ptr addrspace(1) %50
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 18
  store i16 0, ptr addrspace(1) %51
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 20
  store i16 0, ptr addrspace(1) %52
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 22
  store i16 0, ptr addrspace(1) %53
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 24
  store ptr addrspace(1) null, ptr addrspace(1) %54
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 28
  store ptr null, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %0, i16 30
  store i16 0, ptr addrspace(1) %56
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 32
  store ptr null, ptr addrspace(1) %57
  %58 = getelementptr i8, ptr addrspace(1) %0, i16 34
  store i8 0, ptr addrspace(1) %58
  %59 = getelementptr i8, ptr addrspace(1) %0, i16 36
  store ptr null, ptr addrspace(1) %59
  %60 = getelementptr i8, ptr addrspace(1) %0, i16 38
  store i16 0, ptr addrspace(1) %60
  %61 = getelementptr i8, ptr addrspace(1) %0, i16 40
  store i8 0, ptr addrspace(1) %61
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 52
  store i8 0, ptr addrspace(1) %62
  store i16 1, ptr addrspace(1) %0
  br label %b2

b24:
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 36
  %64 = load ptr, ptr addrspace(1) %63
  call cc 1005 addrspace(1) void @N$BDRP(ptr %64)
  %65 = getelementptr i8, ptr addrspace(1) %0, i16 40
  %66 = load i8, ptr addrspace(1) %65, !range !28
  %67 = icmp ne i8 %66, 0
  br i1 %67, label %b26, label %b25

b25:
  br label %b23

b26:
  %68 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %69 = load ptr, ptr addrspace(1) %68
  call cc 1005 addrspace(1) void @N$BDRP(ptr %69)
  %70 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %71 = load i8, ptr addrspace(1) %70, !range !29
  %72 = icmp ne i8 %71, 0
  br i1 %72, label %b28, label %b27

b27:
  %73 = getelementptr i8, ptr addrspace(1) %0, i16 28
  %74 = load ptr, ptr addrspace(1) %73
  call cc 1005 addrspace(1) void @N$BDRP(ptr %74)
  %75 = getelementptr i8, ptr addrspace(1) %0, i16 32
  %76 = load ptr, ptr addrspace(1) %75
  call cc 1005 addrspace(1) void @N$BDRP(ptr %76)
  br label %b25

b28:
  %77 = getelementptr i8, ptr addrspace(1) %0, i16 16
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %77)
  %78 = getelementptr i8, ptr addrspace(1) %0, i16 16
  %79 = load ptr, ptr addrspace(1) %78
  call cc 1005 addrspace(1) void @N$BDRP(ptr %79)
  br label %b27

b29:
  %80 = icmp eq i16 %4, 6
  br i1 %80, label %b32, label %b31

b30:
  store i16 8, ptr addrspace(1) %0
  %81 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %82 = getelementptr inbounds i8, ptr addrspace(1) %81, i16 0
  %83 = load i16, ptr addrspace(1) %82
  %84 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %85 = getelementptr inbounds i8, ptr addrspace(1) %84, i16 2
  %86 = load i16, ptr addrspace(1) %85
  %87 = add i16 %83, %86
  %88 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %89 = getelementptr inbounds i8, ptr addrspace(1) %88, i16 4
  %90 = load i16, ptr addrspace(1) %89
  %91 = add i16 %87, %90
  %92 = sdiv i16 %91, 3
  %93 = srem i16 %91, 3
  %94 = icmp ne i16 %93, 0
  %95 = zext i1 %94 to i8
  %96 = xor i16 %93, 3
  %97 = icmp slt i16 %96, 0
  %98 = zext i1 %97 to i8
  %99 = and i8 %95, %98
  %100 = zext i8 %99 to i16
  %101 = and i16 %100, 1
  %102 = sub i16 %92, %101
  store i16 0, ptr %3, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %102, ptr %103, !tbaa !2
  %104 = addrspacecast ptr %3 to ptr addrspace(1)
  %105 = load i32, ptr addrspace(1) %104, !tbaa !2
  ret i32 %105

b31:
  %106 = icmp eq i16 %4, 7
  br i1 %106, label %b34, label %b33

b32:
  store i16 7, ptr addrspace(1) %0
  br label %b2

b33:
  %107 = icmp eq i16 %4, 8
  br i1 %107, label %b36, label %b35

b34:
  store i16 2, ptr addrspace(1) %0
  br label %b2

b35:
  store i16 1, ptr addrspace(1) %0
  store i32 1, ptr %3, !tbaa !2
  %108 = addrspacecast ptr %3 to ptr addrspace(1)
  %109 = load i32, ptr addrspace(1) %108, !tbaa !2
  ret i32 %109

b36:
  store i16 7, ptr addrspace(1) %0
  br label %b2
}

define internal cc 1005 i32 @$state2.next(ptr addrspace(1) nonnull dereferenceable(40) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca ptr
  %5 = alloca [4 x i8]
  %6 = alloca [4 x i8]
  br label %b2

b2:
  br label %b3

b3:
  %7 = load i16, ptr addrspace(1) %0
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %b7, label %b6

b6:
  %9 = icmp eq i16 %7, 2
  br i1 %9, label %b9, label %b8

b7:
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %10 = icmp eq i16 %7, 3
  br i1 %10, label %b17, label %b16

b9:
  %11 = addrspacecast ptr %5 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call cc 1005 addrspace(1) void @$state1.next(ptr addrspace(1) %11, ptr addrspace(1) %12)
  %13 = load i8, ptr %5, !tbaa !2, !range !36
  %14 = icmp eq i8 %13, 0
  br i1 %14, label %b12, label %b11

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b12:
  %15 = getelementptr inbounds i8, ptr %5, i16 2
  %16 = load ptr, ptr %15, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %5, i16 2
  %18 = load ptr, ptr %17, !tbaa !2
  store ptr %18, ptr %4, !tbaa !2
  %19 = load ptr, ptr %4, !tbaa !2
  store ptr null, ptr %4, !tbaa !2
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %21 = load ptr, ptr addrspace(1) %20
  call cc 1005 addrspace(1) void @N$BDRP(ptr %21)
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 34
  store ptr %19, ptr addrspace(1) %22
  store i16 3, ptr addrspace(1) %0
  %23 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %23)
  br label %b2

b16:
  %24 = icmp eq i16 %7, 4
  br i1 %24, label %b23, label %b22

b17:
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %26 = load ptr, ptr addrspace(1) %25
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 %28, ptr %2, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %28, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %2 to ptr addrspace(1)
  %33 = call cc 1005 addrspace(1) i32 @number(ptr addrspace(1) %32)
  %34 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %33, ptr addrspace(1) %34, !tbaa !2
  %35 = load i8, ptr %3, !tbaa !2, !range !36
  %36 = icmp eq i8 %35, 0
  br i1 %36, label %b20, label %b19

b19:
  store i16 8, ptr addrspace(1) %0
  br label %b2

b20:
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 2
  %40 = load i16, ptr %39, !tbaa !2
  store i16 %40, ptr %1, !tbaa !2
  %41 = load i16, ptr %1, !tbaa !2
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 36
  store i16 %41, ptr addrspace(1) %42
  store i16 6, ptr addrspace(1) %0
  br label %b2

b22:
  %43 = icmp eq i16 %7, 5
  br i1 %43, label %b29, label %b28

b23:
  %44 = getelementptr i8, ptr addrspace(1) %0, i16 38
  %45 = load i8, ptr addrspace(1) %44, !range !30
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b25, label %b24

b24:
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %47
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr null, ptr addrspace(1) %48
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 0, ptr addrspace(1) %49
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 8
  store i16 0, ptr addrspace(1) %50
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store ptr addrspace(1) null, ptr addrspace(1) %51
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 14
  store ptr null, ptr addrspace(1) %52
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 16
  store i16 0, ptr addrspace(1) %53
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 18
  store i16 0, ptr addrspace(1) %54
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 20
  store i16 0, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %0, i16 22
  store ptr addrspace(1) null, ptr addrspace(1) %56
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 26
  store ptr null, ptr addrspace(1) %57
  %58 = getelementptr i8, ptr addrspace(1) %0, i16 28
  store i16 0, ptr addrspace(1) %58
  %59 = getelementptr i8, ptr addrspace(1) %0, i16 30
  store ptr null, ptr addrspace(1) %59
  %60 = getelementptr i8, ptr addrspace(1) %0, i16 32
  store i8 0, ptr addrspace(1) %60
  %61 = getelementptr i8, ptr addrspace(1) %0, i16 38
  store i8 0, ptr addrspace(1) %61
  store i16 1, ptr addrspace(1) %0
  br label %b2

b25:
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %63 = load ptr, ptr addrspace(1) %62
  call cc 1005 addrspace(1) void @N$BDRP(ptr %63)
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 32
  %65 = load i8, ptr addrspace(1) %64, !range !31
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b27, label %b26

b26:
  %67 = getelementptr i8, ptr addrspace(1) %0, i16 26
  %68 = load ptr, ptr addrspace(1) %67
  call cc 1005 addrspace(1) void @N$BDRP(ptr %68)
  %69 = getelementptr i8, ptr addrspace(1) %0, i16 30
  %70 = load ptr, ptr addrspace(1) %69
  call cc 1005 addrspace(1) void @N$BDRP(ptr %70)
  br label %b24

b27:
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 14
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %71)
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %73 = load ptr, ptr addrspace(1) %72
  call cc 1005 addrspace(1) void @N$BDRP(ptr %73)
  br label %b26

b28:
  %74 = icmp eq i16 %7, 6
  br i1 %74, label %b31, label %b30

b29:
  %75 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %76 = load ptr, ptr addrspace(1) %75
  call cc 1005 addrspace(1) void @N$BDRP(ptr %76)
  %77 = getelementptr i8, ptr addrspace(1) %0, i16 34
  store ptr null, ptr addrspace(1) %77
  store i16 2, ptr addrspace(1) %0
  br label %b2

b30:
  %78 = icmp eq i16 %7, 7
  br i1 %78, label %b33, label %b32

b31:
  store i16 7, ptr addrspace(1) %0
  %79 = getelementptr i8, ptr addrspace(1) %0, i16 36
  %80 = load i16, ptr addrspace(1) %79
  store i16 0, ptr %6, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %80, ptr %81, !tbaa !2
  %82 = addrspacecast ptr %6 to ptr addrspace(1)
  %83 = load i32, ptr addrspace(1) %82, !tbaa !2
  ret i32 %83

b32:
  %84 = icmp eq i16 %7, 8
  br i1 %84, label %b35, label %b34

b33:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b34:
  store i16 1, ptr addrspace(1) %0
  store i32 1, ptr %6, !tbaa !2
  %85 = addrspacecast ptr %6 to ptr addrspace(1)
  %86 = load i32, ptr addrspace(1) %85, !tbaa !2
  ret i32 %86

b35:
  %87 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %87)
  %88 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %89 = load ptr, ptr addrspace(1) %88
  call cc 1005 addrspace(1) void @N$PS(ptr %89)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 5, ptr addrspace(1) %0
  br label %b2
}

define internal cc 1005 void @$state1.next(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(32) noalias nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  %3 = alloca [4 x i8]
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 4
  br label %b2

b2:
  br label %b3

b3:
  %5 = load i16, ptr addrspace(1) %1
  %6 = icmp eq i16 %5, 0
  br i1 %6, label %b7, label %b6

b6:
  %7 = icmp eq i16 %5, 2
  br i1 %7, label %b11, label %b10

b7:
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %9 = load ptr, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr null, ptr addrspace(1) %10
  %11 = getelementptr i8, ptr addrspace(1) %1, i16 30
  %12 = load i8, ptr addrspace(1) %11, !range !32
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b9, label %b8

b8:
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store ptr %9, ptr addrspace(1) %14
  %15 = getelementptr i8, ptr addrspace(1) %1, i16 30
  store i8 1, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %1, i16 16
  %17 = load i16, ptr addrspace(1) %4
  store i16 %17, ptr addrspace(1) %16
  %18 = getelementptr i8, ptr addrspace(1) %4, i16 2
  %19 = load i16, ptr addrspace(1) %18
  %20 = getelementptr i8, ptr addrspace(1) %16, i16 2
  store i16 %19, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %4, i16 4
  %22 = load ptr addrspace(1), ptr addrspace(1) %21
  %23 = getelementptr i8, ptr addrspace(1) %16, i16 4
  store ptr addrspace(1) %22, ptr addrspace(1) %23
  %24 = getelementptr i8, ptr addrspace(1) %1, i16 24
  %25 = load ptr, ptr addrspace(1) %24
  call cc 1005 addrspace(1) void @N$BDRP(ptr %25)
  %26 = getelementptr i8, ptr addrspace(1) %1, i16 14
  store i16 0, ptr addrspace(1) %26
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 24
  store ptr null, ptr addrspace(1) %27
  %28 = getelementptr i8, ptr addrspace(1) %1, i16 26
  store i16 0, ptr addrspace(1) %28
  store i16 2, ptr addrspace(1) %1
  br label %b2

b9:
  %29 = getelementptr i8, ptr addrspace(1) %1, i16 12
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %29)
  %30 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %31 = load ptr, ptr addrspace(1) %30
  call cc 1005 addrspace(1) void @N$BDRP(ptr %31)
  br label %b8

b10:
  %32 = icmp eq i16 %5, 3
  br i1 %32, label %b19, label %b18

b11:
  %33 = addrspacecast ptr %3 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %1, i16 14
  call cc 1005 addrspace(1) void @$state0.next(ptr addrspace(1) %33, ptr addrspace(1) %34)
  %35 = load i8, ptr %3, !tbaa !2, !range !37
  %36 = icmp eq i8 %35, 0
  br i1 %36, label %b14, label %b13

b13:
  store i16 4, ptr addrspace(1) %1
  br label %b2

b14:
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  %38 = load ptr, ptr %37, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 2
  %40 = load ptr, ptr %39, !tbaa !2
  store ptr %40, ptr %2, !tbaa !2
  %41 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  %42 = getelementptr i8, ptr addrspace(1) %1, i16 28
  %43 = load ptr, ptr addrspace(1) %42
  call cc 1005 addrspace(1) void @N$BDRP(ptr %43)
  %44 = getelementptr i8, ptr addrspace(1) %1, i16 28
  store ptr %41, ptr addrspace(1) %44
  store i16 3, ptr addrspace(1) %1
  %45 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %45)
  br label %b2

b18:
  %46 = icmp eq i16 %5, 4
  br i1 %46, label %b21, label %b20

b19:
  store i16 5, ptr addrspace(1) %1
  %47 = getelementptr i8, ptr addrspace(1) %1, i16 28
  %48 = load ptr, ptr addrspace(1) %47
  %49 = getelementptr i8, ptr addrspace(1) %1, i16 28
  store ptr null, ptr addrspace(1) %49
  store i16 0, ptr addrspace(1) %0
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %48, ptr addrspace(1) %50
  ret void

b20:
  %51 = icmp eq i16 %5, 5
  br i1 %51, label %b25, label %b24

b21:
  %52 = getelementptr i8, ptr addrspace(1) %1, i16 24
  %53 = load ptr, ptr addrspace(1) %52
  call cc 1005 addrspace(1) void @N$BDRP(ptr %53)
  %54 = getelementptr i8, ptr addrspace(1) %1, i16 14
  store i16 0, ptr addrspace(1) %54
  %55 = getelementptr i8, ptr addrspace(1) %1, i16 16
  store i16 0, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %1, i16 18
  store i16 0, ptr addrspace(1) %56
  %57 = getelementptr i8, ptr addrspace(1) %1, i16 20
  store ptr addrspace(1) null, ptr addrspace(1) %57
  %58 = getelementptr i8, ptr addrspace(1) %1, i16 24
  store ptr null, ptr addrspace(1) %58
  %59 = getelementptr i8, ptr addrspace(1) %1, i16 26
  store i16 0, ptr addrspace(1) %59
  %60 = getelementptr i8, ptr addrspace(1) %1, i16 30
  %61 = load i8, ptr addrspace(1) %60, !range !33
  %62 = icmp ne i8 %61, 0
  br i1 %62, label %b23, label %b22

b22:
  %63 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store ptr null, ptr addrspace(1) %63
  %64 = getelementptr i8, ptr addrspace(1) %1, i16 30
  store i8 0, ptr addrspace(1) %64
  %65 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %66 = load ptr, ptr addrspace(1) %65
  call cc 1005 addrspace(1) void @N$BDRP(ptr %66)
  %67 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr null, ptr addrspace(1) %67
  store i16 1, ptr addrspace(1) %1
  br label %b2

b23:
  %68 = getelementptr i8, ptr addrspace(1) %1, i16 12
  call cc 1005 addrspace(1) void @Source.drop(ptr addrspace(1) %68)
  %69 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %70 = load ptr, ptr addrspace(1) %69
  call cc 1005 addrspace(1) void @N$BDRP(ptr %70)
  br label %b22

b24:
  store i16 1, ptr addrspace(1) %1
  store i32 1, ptr addrspace(1) %0
  ret void

b25:
  %71 = getelementptr i8, ptr addrspace(1) %1, i16 28
  %72 = load ptr, ptr addrspace(1) %71
  call cc 1005 addrspace(1) void @N$BDRP(ptr %72)
  %73 = getelementptr i8, ptr addrspace(1) %1, i16 28
  store ptr null, ptr addrspace(1) %73
  store i16 2, ptr addrspace(1) %1
  br label %b2
}

define internal cc 1005 void @$state0.next(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(14) noalias nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %1, i16 2
  br label %b2

b2:
  br label %b3

b3:
  %3 = load i16, ptr addrspace(1) %1
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b7, label %b6

b6:
  %5 = icmp eq i16 %3, 2
  br i1 %5, label %b9, label %b8

b7:
  %6 = getelementptr i8, ptr @$str3, i16 6
  %7 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %8 = load ptr, ptr addrspace(1) %7
  call cc 1005 addrspace(1) void @N$BDRP(ptr %8)
  %9 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr %6, ptr addrspace(1) %9
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store i16 0, ptr addrspace(1) %10
  store i16 2, ptr addrspace(1) %1
  br label %b2

b8:
  %11 = icmp eq i16 %3, 3
  br i1 %11, label %b14, label %b13

b9:
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %13 = load i16, ptr addrspace(1) %12
  %14 = load i16, ptr addrspace(1) %2
  %15 = icmp ult i16 %13, %14
  br i1 %15, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %1
  br label %b2

b11:
  store i16 5, ptr addrspace(1) %1
  br label %b2

b13:
  %16 = icmp eq i16 %3, 4
  br i1 %16, label %b21, label %b20

b14:
  %17 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %18 = load i16, ptr addrspace(1) %17
  %19 = load i16, ptr addrspace(1) %2
  %20 = icmp ult i16 %18, %19
  br i1 %20, label %b18, label %b19

b15:
  store i16 6, ptr addrspace(1) %1
  br label %b2

b16:
  store i16 7, ptr addrspace(1) %1
  br label %b2

b18:
  %21 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %22 = load ptr addrspace(1), ptr addrspace(1) %21
  %23 = getelementptr inbounds i8, ptr addrspace(1) %22, i16 %18
  %24 = load i8, ptr addrspace(1) %23
  %25 = icmp eq i8 %24, 32
  br i1 %25, label %b15, label %b16

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  %26 = icmp eq i16 %3, 5
  br i1 %26, label %b23, label %b22

b21:
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %28 = load i16, ptr addrspace(1) %27
  %29 = add i16 %28, 1
  %30 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store i16 %29, ptr addrspace(1) %30
  store i16 2, ptr addrspace(1) %1
  br label %b2

b22:
  %31 = icmp eq i16 %3, 6
  br i1 %31, label %b28, label %b27

b23:
  %32 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %33 = load ptr, ptr addrspace(1) %32
  %34 = getelementptr i8, ptr %33, i16 -4
  %35 = load i16, ptr %34
  %36 = icmp ugt i16 %35, 0
  br i1 %36, label %b24, label %b25

b24:
  store i16 13, ptr addrspace(1) %1
  br label %b2

b25:
  store i16 14, ptr addrspace(1) %1
  br label %b2

b27:
  %37 = icmp eq i16 %3, 7
  br i1 %37, label %b33, label %b32

b28:
  %38 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %39 = load ptr, ptr addrspace(1) %38
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  %42 = icmp ugt i16 %41, 0
  br i1 %42, label %b29, label %b30

b29:
  store i16 9, ptr addrspace(1) %1
  br label %b2

b30:
  store i16 10, ptr addrspace(1) %1
  br label %b2

b32:
  %43 = icmp eq i16 %3, 8
  br i1 %43, label %b37, label %b36

b33:
  %44 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %45 = load ptr, ptr addrspace(1) %44
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %45, i16 1, i16 1)
  %49 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr %48, ptr addrspace(1) %49
  %50 = getelementptr inbounds i8, ptr %48, i16 %47
  %51 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %52 = load i16, ptr addrspace(1) %51
  %53 = load i16, ptr addrspace(1) %2
  %54 = icmp ult i16 %52, %53
  br i1 %54, label %b34, label %b35

b34:
  %55 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %56 = load ptr addrspace(1), ptr addrspace(1) %55
  %57 = getelementptr inbounds i8, ptr addrspace(1) %56, i16 %52
  %58 = load i8, ptr addrspace(1) %57
  store i8 %58, ptr %50
  %59 = getelementptr i8, ptr %48, i16 -4
  %60 = load i16, ptr %59
  %61 = getelementptr inbounds i8, ptr %48, i16 %60
  store i8 0, ptr %61
  store i16 8, ptr addrspace(1) %1
  br label %b2

b35:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b36:
  %62 = icmp eq i16 %3, 9
  br i1 %62, label %b39, label %b38

b37:
  store i16 4, ptr addrspace(1) %1
  br label %b2

b38:
  %63 = icmp eq i16 %3, 10
  br i1 %63, label %b41, label %b40

b39:
  store i16 12, ptr addrspace(1) %1
  %64 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %65 = load ptr, ptr addrspace(1) %64
  %66 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr null, ptr addrspace(1) %66
  store i16 0, ptr addrspace(1) %0
  %67 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %65, ptr addrspace(1) %67
  ret void

b40:
  %68 = icmp eq i16 %3, 11
  br i1 %68, label %b43, label %b42

b41:
  store i16 11, ptr addrspace(1) %1
  br label %b2

b42:
  %69 = icmp eq i16 %3, 12
  br i1 %69, label %b45, label %b44

b43:
  store i16 8, ptr addrspace(1) %1
  br label %b2

b44:
  %70 = icmp eq i16 %3, 13
  br i1 %70, label %b47, label %b46

b45:
  %71 = getelementptr i8, ptr @$str3, i16 6
  %72 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %73 = load ptr, ptr addrspace(1) %72
  call cc 1005 addrspace(1) void @N$BDRP(ptr %73)
  %74 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr %71, ptr addrspace(1) %74
  store i16 11, ptr addrspace(1) %1
  br label %b2

b46:
  %75 = icmp eq i16 %3, 14
  br i1 %75, label %b49, label %b48

b47:
  store i16 16, ptr addrspace(1) %1
  %76 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %77 = load ptr, ptr addrspace(1) %76
  %78 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr null, ptr addrspace(1) %78
  store i16 0, ptr addrspace(1) %0
  %79 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %77, ptr addrspace(1) %79
  ret void

b48:
  %80 = icmp eq i16 %3, 15
  br i1 %80, label %b51, label %b50

b49:
  store i16 15, ptr addrspace(1) %1
  br label %b2

b50:
  %81 = icmp eq i16 %3, 16
  br i1 %81, label %b53, label %b52

b51:
  %82 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %83 = load ptr, ptr addrspace(1) %82
  call cc 1005 addrspace(1) void @N$BDRP(ptr %83)
  %84 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr null, ptr addrspace(1) %84
  store i16 1, ptr addrspace(1) %1
  br label %b2

b52:
  store i16 1, ptr addrspace(1) %1
  store i32 1, ptr addrspace(1) %0
  ret void

b53:
  store i16 15, ptr addrspace(1) %1
  br label %b2
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
!11 = !{i8 0, i8 2}
!12 = !{i8 0, i8 2}
!13 = !{i8 0, i8 2}
!14 = !{i8 0, i8 2}
!15 = !{i8 0, i8 2}
!16 = !{i8 0, i8 2}
!17 = !{i8 0, i8 2}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
!20 = !{i8 0, i8 2}
!21 = !{i8 0, i8 2}
!22 = !{i8 0, i8 2}
!23 = !{i8 0, i8 2}
!24 = !{i8 0, i8 2}
!25 = !{i8 0, i8 2}
!26 = !{i8 0, i8 2}
!27 = !{i8 0, i8 2}
!28 = !{i8 0, i8 2}
!29 = !{i8 0, i8 2}
!30 = !{i8 0, i8 2}
!31 = !{i8 0, i8 2}
!32 = !{i8 0, i8 2}
!33 = !{i8 0, i8 2}
!34 = !{i8 0, i8 2}
!35 = !{i8 0, i8 2}
!36 = !{i8 0, i8 2}
!37 = !{i8 0, i8 2}
