target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00rope\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00lamp\00"
@$str4 = internal constant [8 x i8] c"\08\00\01\00\01\00s\00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00torch x\00"
@$str6 = internal constant [15 x i8] c"\08\00\08\00\08\00 kinds, \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00 in stock\00"
@$str8 = internal constant [9 x i8] c"\08\00\02\00\02\00  \00"
@$str9 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00popped \00"
@$str11 = internal constant [9 x i8] c"\08\00\02\00\02\00, \00"
@$str12 = internal constant [12 x i8] c"\08\00\05\00\05\00 left\00"
@$str13 = internal constant [12 x i8] c"\08\00\05\00\05\00north\00"
@$str14 = internal constant [11 x i8] c"\08\00\04\00\04\00east\00"
@$str15 = internal constant [12 x i8] c"\08\00\05\00\05\00south\00"
@$str16 = internal constant [9 x i8] c"\08\00\02\00\02\00up\00"
@$str17 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @stocked(ptr addrspace(1) %0) addrspace(1) willreturn {
b1:
  %1 = load ptr, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %11, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %12, %b3 ]
  %6 = icmp ult i16 %5, %3
  br i1 %6, label %b3, label %b5

b3:
  %7 = shl i16 %5, 2
  %8 = getelementptr i8, ptr %1, i16 %7
  %9 = getelementptr i8, ptr %8, i16 2
  %10 = load i16, ptr %9
  %11 = add i16 %4, %10
  %12 = add i16 %5, 1
  br label %b2

b5:
  ret i16 %4
}

define internal void @restock(ptr addrspace(1) %0, ptr %1, i16 %2) addrspace(1) {
b1:
  %3 = load ptr, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = call addrspace(1) ptr @N$BGRW(ptr %3, i16 1, i16 4)
  store ptr %6, ptr addrspace(1) %0
  %7 = shl i16 %5, 2
  %8 = getelementptr i8, ptr %6, i16 %7
  store ptr %1, ptr %8
  %9 = getelementptr i8, ptr %8, i16 2
  store i16 %2, ptr %9
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca ptr
  store ptr null, ptr %0
  %1 = getelementptr i8, ptr @$str1, i16 6
  store ptr %1, ptr %0, !tbaa !2
  %2 = addrspacecast ptr %0 to ptr addrspace(1)
  %3 = getelementptr i8, ptr @$str2, i16 6
  %4 = getelementptr i8, ptr %1, i16 -4
  %5 = load i16, ptr %4
  %6 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 1, i16 4)
  store ptr %6, ptr addrspace(1) %2
  %7 = shl i16 %5, 2
  %8 = getelementptr i8, ptr %6, i16 %7
  store ptr %3, ptr %8
  %9 = getelementptr i8, ptr %8, i16 2
  store i16 3, ptr %9
  call addrspace(1) void @N$BDRP(ptr null)
  %10 = getelementptr i8, ptr @$str3, i16 6
  %11 = getelementptr i8, ptr @$str4, i16 6
  %12 = call addrspace(1) ptr @N$TCAT(ptr %10, ptr %11)
  %13 = getelementptr i8, ptr %6, i16 -4
  %14 = load i16, ptr %13
  %15 = call addrspace(1) ptr @N$BGRW(ptr %6, i16 1, i16 4)
  store ptr %15, ptr addrspace(1) %2
  %16 = shl i16 %14, 2
  %17 = getelementptr i8, ptr %15, i16 %16
  store ptr %12, ptr %17
  %18 = getelementptr i8, ptr %17, i16 2
  store i16 2, ptr %18
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$PBEG()
  %19 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %19)
  call addrspace(1) void @N$PI2(i16 4)
  %20 = call addrspace(1) ptr @N$PEND()
  %21 = getelementptr i8, ptr %15, i16 -4
  %22 = load i16, ptr %21
  %23 = call addrspace(1) ptr @N$BGRW(ptr %15, i16 1, i16 4)
  store ptr %23, ptr addrspace(1) %2
  %24 = shl i16 %22, 2
  %25 = getelementptr i8, ptr %23, i16 %24
  store ptr %20, ptr %25
  %26 = getelementptr i8, ptr %25, i16 2
  store i16 1, ptr %26
  call addrspace(1) void @N$BDRP(ptr null)
  %27 = getelementptr i8, ptr %23, i16 -4
  %28 = load i16, ptr %27
  br label %29

29:
  %30 = phi i16 [ 0, %b1 ], [ %38, %33 ]
  %31 = phi i16 [ 0, %b1 ], [ %39, %33 ]
  %32 = icmp ult i16 %31, %28
  br i1 %32, label %33, label %40

33:
  %34 = shl i16 %31, 2
  %35 = getelementptr i8, ptr %23, i16 %34
  %36 = getelementptr i8, ptr %35, i16 2
  %37 = load i16, ptr %36
  %38 = add i16 %30, %37
  %39 = add i16 %31, 1
  br label %29

40:
  call addrspace(1) void @N$PU2(i16 %28)
  %41 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %41)
  call addrspace(1) void @N$PI2(i16 %30)
  %42 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %42)
  call addrspace(1) void @N$PN()
  %43 = load ptr, ptr %0, !tbaa !2
  %44 = getelementptr i8, ptr %43, i16 -4
  %45 = load i16, ptr %44
  %46 = getelementptr i8, ptr @$str8, i16 6
  %47 = getelementptr i8, ptr @$str9, i16 6
  br label %b2

b2:
  %48 = phi i16 [ 0, %40 ], [ %55, %b3 ]
  %49 = icmp ult i16 %48, %45
  br i1 %49, label %b3, label %b5

b3:
  %50 = shl i16 %48, 2
  %51 = getelementptr i8, ptr %43, i16 %50
  call addrspace(1) void @N$PS(ptr %46)
  %52 = load ptr, ptr %51
  call addrspace(1) void @N$PS(ptr %52)
  call addrspace(1) void @N$PS(ptr %47)
  %53 = getelementptr i8, ptr %51, i16 2
  %54 = load i16, ptr %53
  call addrspace(1) void @N$PI2(i16 %54)
  call addrspace(1) void @N$PN()
  %55 = add i16 %48, 1
  br label %b2

b5:
  %56 = load ptr, ptr %0, !tbaa !2
  %57 = getelementptr i8, ptr %56, i16 -4
  %58 = load i16, ptr %57
  br label %b6

b6:
  %59 = phi ptr [ %1, %b5 ], [ %66, %b7 ]
  %60 = phi i16 [ 0, %b5 ], [ %71, %b7 ]
  %61 = icmp ult i16 %60, %58
  br i1 %61, label %b7, label %b9

b7:
  %62 = shl i16 %60, 2
  %63 = getelementptr i8, ptr %56, i16 %62
  %64 = getelementptr i8, ptr %59, i16 -4
  %65 = load i16, ptr %64
  %66 = call addrspace(1) ptr @N$BGRW(ptr %59, i16 1, i16 2)
  %67 = shl i16 %65, 1
  %68 = getelementptr i8, ptr %66, i16 %67
  %69 = getelementptr i8, ptr %63, i16 2
  %70 = load i16, ptr %69
  store i16 %70, ptr %68
  %71 = add i16 %60, 1
  br label %b6

b9:
  %72 = getelementptr i8, ptr %59, i16 -4
  %73 = load i16, ptr %72
  br label %b10

b10:
  %74 = phi ptr [ %1, %b9 ], [ %81, %b11 ]
  %75 = phi i16 [ 0, %b9 ], [ %86, %b11 ]
  %76 = icmp ult i16 %75, %73
  br i1 %76, label %b11, label %b13

b11:
  %77 = shl i16 %75, 1
  %78 = getelementptr i8, ptr %59, i16 %77
  %79 = getelementptr i8, ptr %74, i16 -4
  %80 = load i16, ptr %79
  %81 = call addrspace(1) ptr @N$BGRW(ptr %74, i16 1, i16 2)
  %82 = shl i16 %80, 1
  %83 = getelementptr i8, ptr %81, i16 %82
  %84 = load i16, ptr %78
  %85 = shl i16 %84, 1
  store i16 %85, ptr %83
  %86 = add i16 %75, 1
  br label %b10

b13:
  %87 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 2, i16 2)
  br label %b14

b14:
  %88 = phi i16 [ 0, %b13 ], [ %100, %b16 ]
  %89 = icmp ult i16 %88, 2
  br i1 %89, label %b16, label %b15

b15:
  %90 = getelementptr i8, ptr %87, i16 -4
  %91 = load i16, ptr %90
  %92 = call addrspace(1) ptr @N$BGRW(ptr %87, i16 1, i16 2)
  %93 = shl i16 %91, 1
  %94 = getelementptr i8, ptr %92, i16 %93
  %95 = getelementptr i8, ptr %74, i16 -4
  %96 = load i16, ptr %95
  %97 = icmp ugt i16 %96, 0
  br i1 %97, label %b17, label %b18

b16:
  %98 = shl i16 %88, 1
  %99 = getelementptr i8, ptr %87, i16 %98
  store i16 7, ptr %99
  %100 = add i16 %88, 1
  br label %b14

b17:
  %101 = getelementptr i8, ptr %74, i16 0
  %102 = load i16, ptr %101
  store i16 %102, ptr %94
  %103 = call addrspace(1) i16 @N$BSHR(ptr %92, i16 1)
  %104 = shl i16 %103, 1
  %105 = getelementptr i8, ptr %92, i16 %104
  %106 = load i16, ptr %105
  %107 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %107)
  call addrspace(1) void @N$PI2(i16 %106)
  %108 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %108)
  %109 = getelementptr i8, ptr %92, i16 -4
  %110 = load i16, ptr %109
  call addrspace(1) void @N$PU2(i16 %110)
  %111 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %111)
  call addrspace(1) void @N$PN()
  %112 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 2, i16 2)
  %113 = getelementptr i8, ptr %112, i16 0
  %114 = getelementptr i8, ptr @$str13, i16 6
  store ptr %114, ptr %113
  %115 = getelementptr i8, ptr %112, i16 2
  %116 = getelementptr i8, ptr @$str14, i16 6
  store ptr %116, ptr %115
  %117 = call addrspace(1) ptr @N$BCLN(ptr %112, i16 2)
  %118 = getelementptr i8, ptr %117, i16 -4
  %119 = load i16, ptr %118
  br label %b19

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %120 = phi i16 [ 0, %b17 ], [ %134, %b21 ]
  %121 = icmp ult i16 %120, %119
  br i1 %121, label %b21, label %b20

b20:
  %122 = load i16, ptr %118
  %123 = call addrspace(1) ptr @N$BGRW(ptr %117, i16 1, i16 2)
  %124 = shl i16 %122, 1
  %125 = getelementptr i8, ptr %123, i16 %124
  %126 = getelementptr i8, ptr @$str15, i16 6
  store ptr %126, ptr %125
  %127 = getelementptr i8, ptr %123, i16 -4
  %128 = load i16, ptr %127
  %129 = icmp ugt i16 %128, 0
  br i1 %129, label %b22, label %b23

b21:
  %130 = shl i16 %120, 1
  %131 = getelementptr i8, ptr %117, i16 %130
  %132 = load ptr, ptr %131
  %133 = call addrspace(1) ptr @N$BCLN(ptr %132, i16 1)
  store ptr %133, ptr %131
  %134 = add i16 %120, 1
  br label %b19

b22:
  %135 = getelementptr i8, ptr %123, i16 0
  %136 = getelementptr i8, ptr @$str16, i16 6
  %137 = load ptr, ptr %135
  call addrspace(1) void @N$BDRP(ptr %137)
  store ptr %136, ptr %135
  %138 = getelementptr i8, ptr %112, i16 -4
  %139 = load i16, ptr %138
  %140 = icmp ugt i16 %139, 0
  br i1 %140, label %b24, label %b25

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %141 = load ptr, ptr %113
  call addrspace(1) void @N$PS(ptr %141)
  %142 = getelementptr i8, ptr @$str17, i16 6
  call addrspace(1) void @N$PS(ptr %142)
  %143 = load i16, ptr %127
  %144 = icmp ugt i16 %143, 0
  br i1 %144, label %b26, label %b27

b25:
  call addrspace(1) void @N$EBND()
  unreachable

b26:
  %145 = load ptr, ptr %135
  call addrspace(1) void @N$PS(ptr %145)
  call addrspace(1) void @N$PS(ptr %142)
  %146 = load i16, ptr %127
  %147 = icmp ugt i16 %146, 2
  br i1 %147, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %148 = getelementptr i8, ptr %123, i16 4
  %149 = load ptr, ptr %148
  call addrspace(1) void @N$PS(ptr %149)
  call addrspace(1) void @N$PN()
  %150 = icmp ne ptr %123, null
  br i1 %150, label %b31, label %b30

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  call addrspace(1) void @N$BDRP(ptr %123)
  %151 = icmp ne ptr %112, null
  br i1 %151, label %b36, label %b35

b31:
  %152 = load i16, ptr %127
  br label %b32

b32:
  %153 = phi i16 [ 0, %b31 ], [ %158, %b34 ]
  %154 = icmp ult i16 %153, %152
  br i1 %154, label %b34, label %b30

b34:
  %155 = shl i16 %153, 1
  %156 = getelementptr i8, ptr %123, i16 %155
  %157 = load ptr, ptr %156
  call addrspace(1) void @N$BDRP(ptr %157)
  %158 = add i16 %153, 1
  br label %b32

b35:
  call addrspace(1) void @N$BDRP(ptr %112)
  call addrspace(1) void @N$BDRP(ptr %92)
  call addrspace(1) void @N$BDRP(ptr %74)
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr %59)
  call addrspace(1) void @N$BDRP(ptr null)
  %159 = load ptr, ptr %0, !tbaa !2
  %160 = icmp ne ptr %159, null
  br i1 %160, label %b41, label %b40

b36:
  %161 = load i16, ptr %138
  br label %b37

b37:
  %162 = phi i16 [ 0, %b36 ], [ %167, %b39 ]
  %163 = icmp ult i16 %162, %161
  br i1 %163, label %b39, label %b35

b39:
  %164 = shl i16 %162, 1
  %165 = getelementptr i8, ptr %112, i16 %164
  %166 = load ptr, ptr %165
  call addrspace(1) void @N$BDRP(ptr %166)
  %167 = add i16 %162, 1
  br label %b37

b40:
  call addrspace(1) void @N$BDRP(ptr %159)
  ret i16 0

b41:
  %168 = getelementptr i8, ptr %159, i16 -4
  %169 = load i16, ptr %168
  br label %b42

b42:
  %170 = phi i16 [ 0, %b41 ], [ %175, %b44 ]
  %171 = icmp ult i16 %170, %169
  br i1 %171, label %b44, label %b40

b44:
  %172 = shl i16 %170, 2
  %173 = getelementptr i8, ptr %159, i16 %172
  %174 = load ptr, ptr %173
  call addrspace(1) void @N$BDRP(ptr %174)
  %175 = add i16 %170, 1
  br label %b42
}

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare ptr @N$TCAT(ptr, ptr) addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare i16 @N$BSHR(ptr, i16) addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
