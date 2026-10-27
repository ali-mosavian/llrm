target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00Rovers\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00United\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00Athletic\00"
@$str5 = internal constant [16 x i8] c"\08\00\09\00\09\00 lead on \00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00 from \00"

define internal cc 1005 i16 @Team.points(ptr addrspace(1) nonnull dereferenceable(8) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = mul i16 %2, 3
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load i16, ptr addrspace(1) %4
  %6 = add i16 %3, %5
  ret i16 %6
}

define internal cc 1005 i16 @Team.played(ptr addrspace(1) nonnull dereferenceable(8) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load i16, ptr addrspace(1) %3
  %5 = add i16 %2, %4
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %7 = load i16, ptr addrspace(1) %6
  %8 = add i16 %5, %7
  ret i16 %8
}

define internal cc 1005 void @Team.record(ptr addrspace(1) nonnull dereferenceable(8) noalias nocapture %0, i16 %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = icmp sgt i16 %1, %2
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = add i16 %5, 1
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %6, ptr addrspace(1) %7
  br label %b4

b3:
  %8 = icmp eq i16 %1, %2
  br i1 %8, label %b5, label %b6

b4:
  ret void

b5:
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %10 = load i16, ptr addrspace(1) %9
  %11 = add i16 %10, 1
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %11, ptr addrspace(1) %12
  br label %b7

b6:
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %14 = load i16, ptr addrspace(1) %13
  %15 = add i16 %14, 1
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %15, ptr addrspace(1) %16
  br label %b7

b7:
  br label %b4
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca [8 x i8]
  %14 = alloca [4 x i8]
  %15 = alloca ptr
  %16 = alloca i16
  %17 = alloca ptr
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca ptr
  %21 = alloca ptr
  %22 = getelementptr i8, ptr @$str1, i16 6
  store ptr %22, ptr %21, !tbaa !2
  %23 = getelementptr i8, ptr @$str1, i16 6
  %24 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %23, i16 3, i16 2)
  %25 = getelementptr inbounds i8, ptr %24, i16 0
  %26 = getelementptr i8, ptr @$str2, i16 6
  store ptr %26, ptr %25
  %27 = getelementptr inbounds i8, ptr %24, i16 2
  %28 = getelementptr i8, ptr @$str3, i16 6
  store ptr %28, ptr %27
  %29 = getelementptr inbounds i8, ptr %24, i16 4
  %30 = getelementptr i8, ptr @$str4, i16 6
  store ptr %30, ptr %29
  store ptr %24, ptr %20, !tbaa !2
  %31 = load ptr, ptr %20, !tbaa !2
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  store i16 0, ptr %19, !tbaa !2
  br label %b2

b2:
  %34 = load i16, ptr %19, !tbaa !2
  %35 = icmp ult i16 %34, %33
  br i1 %35, label %b3, label %b5

b3:
  %36 = mul i16 %34, 2
  %37 = getelementptr inbounds i8, ptr %31, i16 %36
  %38 = load ptr, ptr %21, !tbaa !2
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %38, i16 1, i16 8)
  store ptr %41, ptr %21, !tbaa !2
  %42 = mul i16 %40, 8
  %43 = getelementptr inbounds i8, ptr %41, i16 %42
  %44 = load ptr, ptr %37
  %45 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %44, i16 1)
  store ptr %45, ptr %43
  %46 = getelementptr i8, ptr %43, i16 2
  store i16 0, ptr %46
  %47 = getelementptr i8, ptr %43, i16 4
  store i16 0, ptr %47
  %48 = getelementptr i8, ptr %43, i16 6
  store i16 0, ptr %48
  br label %b4

b4:
  %49 = load i16, ptr %19, !tbaa !2
  %50 = add i16 %49, 1
  store i16 %50, ptr %19, !tbaa !2
  br label %b2

b5:
  %51 = load ptr, ptr %20, !tbaa !2
  %52 = icmp ne ptr %51, null
  br i1 %52, label %b7, label %b6

b6:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %51)
  %53 = load ptr, ptr %21, !tbaa !2
  %54 = getelementptr i8, ptr %53, i16 -4
  %55 = load i16, ptr %54
  %56 = icmp ult i16 0, %55
  br i1 %56, label %b11, label %b12

b7:
  %57 = getelementptr i8, ptr %51, i16 -4
  %58 = load i16, ptr %57
  store i16 0, ptr %18, !tbaa !2
  br label %b8

b8:
  %59 = load i16, ptr %18, !tbaa !2
  %60 = icmp ult i16 %59, %58
  br i1 %60, label %b10, label %b9

b9:
  br label %b6

b10:
  %61 = mul i16 %59, 2
  %62 = getelementptr inbounds i8, ptr %51, i16 %61
  %63 = load ptr, ptr %62
  call cc 1005 addrspace(1) void @N$BDRP(ptr %63)
  %64 = add i16 %59, 1
  store i16 %64, ptr %18, !tbaa !2
  br label %b8

b11:
  %65 = getelementptr inbounds i8, ptr %53, i16 0
  %66 = addrspacecast ptr %65 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Team.record(ptr addrspace(1) %66, i16 2, i16 1)
  %67 = load ptr, ptr %21, !tbaa !2
  %68 = getelementptr i8, ptr %67, i16 -4
  %69 = load i16, ptr %68
  %70 = icmp ult i16 0, %69
  br i1 %70, label %b13, label %b14

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b13:
  %71 = getelementptr inbounds i8, ptr %67, i16 0
  %72 = addrspacecast ptr %71 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Team.record(ptr addrspace(1) %72, i16 0, i16 0)
  %73 = load ptr, ptr %21, !tbaa !2
  %74 = getelementptr i8, ptr %73, i16 -4
  %75 = load i16, ptr %74
  %76 = icmp ult i16 1, %75
  br i1 %76, label %b15, label %b16

b14:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b15:
  %77 = getelementptr inbounds i8, ptr %73, i16 8
  %78 = addrspacecast ptr %77 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Team.record(ptr addrspace(1) %78, i16 3, i16 0)
  %79 = load ptr, ptr %21, !tbaa !2
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  %82 = icmp ult i16 1, %81
  br i1 %82, label %b17, label %b18

b16:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b17:
  %83 = getelementptr inbounds i8, ptr %79, i16 8
  %84 = addrspacecast ptr %83 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Team.record(ptr addrspace(1) %84, i16 1, i16 2)
  %85 = load ptr, ptr %21, !tbaa !2
  %86 = getelementptr i8, ptr %85, i16 -4
  %87 = load i16, ptr %86
  %88 = icmp ult i16 2, %87
  br i1 %88, label %b19, label %b20

b18:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  %89 = getelementptr inbounds i8, ptr %85, i16 16
  %90 = addrspacecast ptr %89 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Team.record(ptr addrspace(1) %90, i16 1, i16 1)
  %91 = load ptr, ptr %21, !tbaa !2
  %92 = getelementptr i8, ptr %91, i16 -4
  %93 = load i16, ptr %92
  %94 = icmp ult i16 2, %93
  br i1 %94, label %b21, label %b22

b20:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b21:
  %95 = getelementptr inbounds i8, ptr %91, i16 16
  %96 = addrspacecast ptr %95 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Team.record(ptr addrspace(1) %96, i16 0, i16 4)
  %97 = getelementptr i8, ptr @$str1, i16 6
  store ptr %97, ptr %17, !tbaa !2
  %98 = load ptr, ptr %21, !tbaa !2
  %99 = getelementptr i8, ptr %98, i16 -4
  %100 = load i16, ptr %99
  store i16 0, ptr %16, !tbaa !2
  br label %b23

b22:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b23:
  %101 = load i16, ptr %16, !tbaa !2
  %102 = icmp ult i16 %101, %100
  br i1 %102, label %b24, label %b26

b24:
  %103 = mul i16 %101, 8
  %104 = getelementptr inbounds i8, ptr %98, i16 %103
  %105 = load ptr, ptr %17, !tbaa !2
  %106 = getelementptr i8, ptr %105, i16 -4
  %107 = load i16, ptr %106
  %108 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %105, i16 1, i16 2)
  store ptr %108, ptr %17, !tbaa !2
  %109 = mul i16 %107, 2
  %110 = getelementptr inbounds i8, ptr %108, i16 %109
  %111 = addrspacecast ptr %104 to ptr addrspace(1)
  %112 = call cc 1005 addrspace(1) i16 @Team.points(ptr addrspace(1) %111)
  store i16 %112, ptr %110
  br label %b25

b25:
  %113 = load i16, ptr %16, !tbaa !2
  %114 = add i16 %113, 1
  store i16 %114, ptr %16, !tbaa !2
  br label %b23

b26:
  %115 = load ptr, ptr %17, !tbaa !2
  store ptr null, ptr %17, !tbaa !2
  store ptr %115, ptr %15, !tbaa !2
  %116 = load ptr, ptr %15, !tbaa !2
  %117 = getelementptr i8, ptr %116, i16 -4
  %118 = load i16, ptr %117
  %119 = addrspacecast ptr %116 to ptr addrspace(1)
  store i16 %118, ptr %13, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %118, ptr %120, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %119, ptr %121, !tbaa !2
  %122 = addrspacecast ptr %13 to ptr addrspace(1)
  %123 = call cc 1005 addrspace(1) i32 @"best[i16]"(ptr addrspace(1) %122)
  %124 = addrspacecast ptr %14 to ptr addrspace(1)
  store i32 %123, ptr addrspace(1) %124, !tbaa !2
  %125 = load i16, ptr %14, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %14, i16 2
  %127 = load i16, ptr %126, !tbaa !2
  store i16 %125, ptr %12, !tbaa !2
  store i16 %127, ptr %11, !tbaa !2
  %128 = load ptr, ptr %21, !tbaa !2
  %129 = load i16, ptr %12, !tbaa !2
  %130 = getelementptr i8, ptr %128, i16 -4
  %131 = load i16, ptr %130
  %132 = icmp ult i16 %129, %131
  br i1 %132, label %b27, label %b28

b27:
  %133 = mul i16 %129, 8
  %134 = getelementptr inbounds i8, ptr %128, i16 %133
  %135 = load ptr, ptr %134
  call cc 1005 addrspace(1) void @N$PS(ptr %135)
  %136 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %136)
  %137 = load i16, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %137)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 2, ptr %10, !tbaa !2
  %138 = load ptr, ptr %15, !tbaa !2
  %139 = getelementptr i8, ptr %138, i16 -4
  %140 = load i16, ptr %139
  %141 = addrspacecast ptr %138 to ptr addrspace(1)
  store i16 %140, ptr %9, !tbaa !2
  %142 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %140, ptr %142, !tbaa !2
  %143 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %141, ptr %143, !tbaa !2
  %144 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 0, ptr %8, !tbaa !2
  %145 = load i16, ptr addrspace(1) %144
  store i16 0, ptr %7, !tbaa !2
  br label %b30

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %146 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %146)
  %147 = load ptr, ptr %17, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %147)
  %148 = load ptr, ptr %21, !tbaa !2
  %149 = icmp ne ptr %148, null
  br i1 %149, label %b41, label %b40

b30:
  %150 = load i16, ptr %7, !tbaa !2
  %151 = icmp ult i16 %150, %145
  br i1 %151, label %b31, label %b33

b31:
  %152 = getelementptr i8, ptr addrspace(1) %144, i16 4
  %153 = load ptr addrspace(1), ptr addrspace(1) %152, !tbaa !2
  %154 = mul i16 %150, 2
  %155 = getelementptr inbounds i8, ptr addrspace(1) %153, i16 %154
  %156 = load i16, ptr addrspace(1) %155
  %157 = load i16, ptr %10, !tbaa !2
  %158 = icmp sge i16 %156, %157
  br i1 %158, label %b34, label %b35

b32:
  %159 = load i16, ptr %7, !tbaa !2
  %160 = add i16 %159, 1
  store i16 %160, ptr %7, !tbaa !2
  br label %b30

b33:
  br label %b29

b34:
  %161 = load i16, ptr %8, !tbaa !2
  store i16 %161, ptr %6, !tbaa !2
  %162 = load ptr, ptr %21, !tbaa !2
  %163 = load i16, ptr %6, !tbaa !2
  %164 = getelementptr i8, ptr %162, i16 -4
  %165 = load i16, ptr %164
  %166 = icmp ult i16 %163, %165
  br i1 %166, label %b38, label %b39

b35:
  br label %b36

b36:
  %167 = load i16, ptr %8, !tbaa !2
  %168 = add i16 %167, 1
  store i16 %168, ptr %8, !tbaa !2
  br label %b32

b37:
  br label %b36

b38:
  %169 = mul i16 %163, 8
  %170 = getelementptr inbounds i8, ptr %162, i16 %169
  %171 = load ptr, ptr %170
  %172 = getelementptr i8, ptr %170, i16 2
  %173 = load i16, ptr %172
  %174 = getelementptr i8, ptr %170, i16 4
  %175 = load i16, ptr %174
  %176 = getelementptr i8, ptr %170, i16 6
  %177 = load i16, ptr %176
  store ptr %171, ptr %4, !tbaa !2
  %178 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %173, ptr %178, !tbaa !2
  %179 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 %175, ptr %179, !tbaa !2
  %180 = getelementptr inbounds i8, ptr %4, i16 6
  store i16 %177, ptr %180, !tbaa !2
  %181 = load ptr, ptr %4, !tbaa !2
  %182 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %181, i16 1)
  store ptr %182, ptr %4, !tbaa !2
  %183 = load ptr, ptr %4, !tbaa !2
  %184 = getelementptr inbounds i8, ptr %4, i16 2
  %185 = load i16, ptr %184, !tbaa !2
  %186 = getelementptr inbounds i8, ptr %4, i16 4
  %187 = load i16, ptr %186, !tbaa !2
  %188 = getelementptr inbounds i8, ptr %4, i16 6
  %189 = load i16, ptr %188, !tbaa !2
  store ptr %183, ptr %5, !tbaa !2
  %190 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %185, ptr %190, !tbaa !2
  %191 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 %187, ptr %191, !tbaa !2
  %192 = getelementptr inbounds i8, ptr %5, i16 6
  store i16 %189, ptr %192, !tbaa !2
  %193 = load ptr, ptr %5, !tbaa !2
  %194 = getelementptr i8, ptr %193, i16 -4
  %195 = load i16, ptr %194
  %196 = addrspacecast ptr %193 to ptr addrspace(1)
  store i16 %195, ptr %3, !tbaa !2
  %197 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %195, ptr %197, !tbaa !2
  %198 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %196, ptr %198, !tbaa !2
  %199 = addrspacecast ptr %3 to ptr addrspace(1)
  %200 = addrspacecast ptr %5 to ptr addrspace(1)
  %201 = call cc 1005 addrspace(1) i16 @Team.points(ptr addrspace(1) %200)
  store i16 %201, ptr %2, !tbaa !2
  %202 = addrspacecast ptr %5 to ptr addrspace(1)
  %203 = call cc 1005 addrspace(1) i16 @Team.played(ptr addrspace(1) %202)
  store i16 %203, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %199)
  %204 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %204)
  %205 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %205)
  %206 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %206)
  %207 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %207)
  call cc 1005 addrspace(1) void @N$PN()
  %208 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %208)
  br label %b37

b39:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b40:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %148)
  ret i16 0

b41:
  %209 = getelementptr i8, ptr %148, i16 -4
  %210 = load i16, ptr %209
  store i16 0, ptr %0, !tbaa !2
  br label %b42

b42:
  %211 = load i16, ptr %0, !tbaa !2
  %212 = icmp ult i16 %211, %210
  br i1 %212, label %b44, label %b43

b43:
  br label %b40

b44:
  %213 = mul i16 %211, 8
  %214 = getelementptr inbounds i8, ptr %148, i16 %213
  %215 = load ptr, ptr %214
  call cc 1005 addrspace(1) void @N$BDRP(ptr %215)
  %216 = add i16 %211, 1
  store i16 %216, ptr %0, !tbaa !2
  br label %b42
}

define internal cc 1005 i32 @"best[i16]"(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [4 x i8]
  store i16 0, ptr %4, !tbaa !2
  %6 = load i16, ptr addrspace(1) %0, !range !5
  %7 = icmp ult i16 0, %6
  br i1 %7, label %b2, label %b3

b2:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = getelementptr inbounds i8, ptr addrspace(1) %9, i16 0
  %11 = load i16, ptr addrspace(1) %10
  store i16 %11, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %12 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b4

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %13 = load i16, ptr %1, !tbaa !2
  %14 = icmp ult i16 %13, %12
  br i1 %14, label %b5, label %b7

b5:
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %16 = load ptr addrspace(1), ptr addrspace(1) %15
  %17 = mul i16 %13, 2
  %18 = getelementptr inbounds i8, ptr addrspace(1) %16, i16 %17
  %19 = load i16, ptr addrspace(1) %18
  %20 = load i16, ptr %3, !tbaa !2
  %21 = icmp sgt i16 %19, %20
  br i1 %21, label %b8, label %b9

b6:
  %22 = load i16, ptr %1, !tbaa !2
  %23 = add i16 %22, 1
  store i16 %23, ptr %1, !tbaa !2
  br label %b4

b7:
  %24 = load i16, ptr %4, !tbaa !2
  %25 = load i16, ptr %3, !tbaa !2
  store i16 %24, ptr %5, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %25, ptr %26, !tbaa !2
  %27 = addrspacecast ptr %5 to ptr addrspace(1)
  %28 = load i32, ptr addrspace(1) %27, !tbaa !2
  ret i32 %28

b8:
  %29 = load i16, ptr addrspace(1) %18
  store i16 %29, ptr %3, !tbaa !2
  %30 = load i16, ptr %2, !tbaa !2
  store i16 %30, ptr %4, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  %31 = load i16, ptr %2, !tbaa !2
  %32 = add i16 %31, 1
  store i16 %32, ptr %2, !tbaa !2
  br label %b6
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
