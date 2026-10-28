target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 4)
  store ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr %1
  %2 = getelementptr i8, ptr %1, i16 2
  store i16 3, ptr %2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %3 = tail call cc 1005 addrspace(1) ptr @N$TCAT(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  %4 = getelementptr i8, ptr %1, i16 -4
  %5 = load i16, ptr %4
  %6 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %1, i16 1, i16 4)
  %7 = shl i16 %5, 2
  %8 = getelementptr inbounds i8, ptr %6, i16 %7
  store ptr %3, ptr %8
  %9 = getelementptr i8, ptr %8, i16 2
  store i16 2, ptr %9
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  tail call cc 1005 addrspace(1) void @N$PBEG()
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 4)
  %10 = tail call cc 1005 addrspace(1) ptr @N$PEND()
  %11 = getelementptr i8, ptr %6, i16 -4
  %12 = load i16, ptr %11
  %13 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %6, i16 1, i16 4)
  %14 = shl i16 %12, 2
  %15 = getelementptr inbounds i8, ptr %13, i16 %14
  store ptr %10, ptr %15
  %16 = getelementptr i8, ptr %15, i16 2
  store i16 1, ptr %16
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %17 = getelementptr i8, ptr %13, i16 -4
  %18 = load i16, ptr %17
  %19 = icmp eq i16 %18, 0
  br i1 %19, label %30, label %20

20:
  %21 = getelementptr i8, ptr %13, i16 2
  br label %22

22:
  %23 = phi ptr [ %21, %20 ], [ %29, %22 ]
  %24 = phi i16 [ %18, %20 ], [ %28, %22 ]
  %25 = phi i16 [ %27, %22 ], [ 0, %20 ]
  %26 = load i16, ptr %23
  %27 = add i16 %26, %25
  %28 = add i16 %24, -1
  %29 = getelementptr i8, ptr %23, i16 4
  %exitcond = icmp eq i16 %28, 0
  br i1 %exitcond, label %30, label %22

30:
  %31 = phi i16 [ 0, %0 ], [ %27, %22 ]
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %18)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %31)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %32 = getelementptr i8, ptr %13, i16 -4
  %33 = load i16, ptr %32
  %34 = icmp eq i16 %33, 0
  br i1 %34, label %b9, label %35

35:
  %36 = getelementptr i8, ptr %13, i16 2
  br label %b3

b3:
  %37 = phi ptr [ %36, %35 ], [ %43, %b3 ]
  %38 = phi i16 [ %33, %35 ], [ %42, %b3 ]
  %39 = getelementptr i8, ptr %37, i16 -2
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  %40 = load ptr, ptr %39
  tail call cc 1005 addrspace(1) void @N$PS(ptr %40)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  %41 = load i16, ptr %37
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %41)
  tail call cc 1005 addrspace(1) void @N$PN()
  %42 = add i16 %38, -1
  %43 = getelementptr i8, ptr %37, i16 4
  %exitcond7 = icmp eq i16 %42, 0
  br i1 %exitcond7, label %b5, label %b3

b5:
  %44 = getelementptr i8, ptr %13, i16 -4
  %45 = load i16, ptr %44
  %46 = icmp eq i16 %45, 0
  br i1 %46, label %b9, label %47

47:
  %48 = getelementptr i8, ptr %13, i16 2
  br label %b7

b7:
  %49 = phi ptr [ %48, %47 ], [ %59, %b7 ]
  %50 = phi i16 [ %45, %47 ], [ %58, %b7 ]
  %51 = phi ptr [ %54, %b7 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %47 ]
  %52 = getelementptr i8, ptr %51, i16 -4
  %53 = load i16, ptr %52
  %54 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %51, i16 1, i16 2)
  %55 = shl i16 %53, 1
  %56 = getelementptr inbounds i8, ptr %54, i16 %55
  %57 = load i16, ptr %49
  store i16 %57, ptr %56
  %58 = add i16 %50, -1
  %59 = getelementptr i8, ptr %49, i16 4
  %exitcond6 = icmp eq i16 %58, 0
  br i1 %exitcond6, label %b9, label %b7

b9:
  %60 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b5 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %30 ], [ %54, %b7 ]
  %61 = getelementptr i8, ptr %60, i16 -4
  %62 = load i16, ptr %61
  %63 = icmp eq i16 %62, 0
  br i1 %63, label %b13, label %64

64:
  br label %b11

b11:
  %65 = phi ptr [ %60, %64 ], [ %76, %b11 ]
  %66 = phi i16 [ %62, %64 ], [ %75, %b11 ]
  %67 = phi ptr [ %70, %b11 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %64 ]
  %68 = getelementptr i8, ptr %67, i16 -4
  %69 = load i16, ptr %68
  %70 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %67, i16 1, i16 2)
  %71 = shl i16 %69, 1
  %72 = getelementptr inbounds i8, ptr %70, i16 %71
  %73 = load i16, ptr %65
  %74 = shl i16 %73, 1
  store i16 %74, ptr %72
  %75 = add i16 %66, -1
  %76 = getelementptr i8, ptr %65, i16 2
  %exitcond5 = icmp eq i16 %75, 0
  br i1 %exitcond5, label %b13, label %b11

b13:
  %77 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b9 ], [ %70, %b11 ]
  %78 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 2, i16 2)
  store i16 7, ptr %78
  %79 = getelementptr inbounds nuw i8, ptr %78, i16 2
  store i16 7, ptr %79
  %80 = getelementptr i8, ptr %78, i16 -4
  %81 = load i16, ptr %80
  %82 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %78, i16 1, i16 2)
  %83 = getelementptr i8, ptr %77, i16 -4
  %84 = load i16, ptr %83
  %85 = icmp eq i16 %84, 0
  br i1 %85, label %b18, label %b17

b17:
  %86 = shl i16 %81, 1
  %87 = getelementptr inbounds i8, ptr %82, i16 %86
  %88 = load i16, ptr %77
  store i16 %88, ptr %87
  %89 = tail call cc 1005 addrspace(1) i16 @N$BSHR(ptr %82, i16 1)
  %90 = shl i16 %89, 1
  %91 = getelementptr inbounds i8, ptr %82, i16 %90
  %92 = load i16, ptr %91
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %92)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  %93 = getelementptr i8, ptr %82, i16 -4
  %94 = load i16, ptr %93
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %94)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  %95 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 2, i16 2)
  store ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6), ptr %95
  %96 = getelementptr inbounds nuw i8, ptr %95, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6), ptr %96
  %97 = tail call cc 1005 addrspace(1) ptr @N$BCLN(ptr %95, i16 2)
  %98 = getelementptr i8, ptr %97, i16 -4
  %99 = load i16, ptr %98
  %100 = icmp eq i16 %99, 0
  br i1 %100, label %b20, label %101

101:
  br label %b21

b18:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

102:
  %103 = getelementptr i8, ptr %97, i16 -4
  %104 = load i16, ptr %103
  %105 = shl i16 %104, 1
  br label %b20

b20:
  %106 = phi i16 [ 0, %b17 ], [ %105, %102 ]
  %107 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %97, i16 1, i16 2)
  %108 = getelementptr inbounds i8, ptr %107, i16 %106
  store ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6), ptr %108
  %109 = getelementptr i8, ptr %107, i16 -4
  %110 = load i16, ptr %109
  %111 = icmp eq i16 %110, 0
  br i1 %111, label %b23, label %b22

b21:
  %112 = phi ptr [ %97, %101 ], [ %117, %b21 ]
  %113 = phi i16 [ %99, %101 ], [ %116, %b21 ]
  %114 = load ptr, ptr %112
  %115 = tail call cc 1005 addrspace(1) ptr @N$BCLN(ptr %114, i16 1)
  store ptr %115, ptr %112
  %116 = add i16 %113, -1
  %117 = getelementptr i8, ptr %112, i16 2
  %exitcond4 = icmp eq i16 %116, 0
  br i1 %exitcond4, label %102, label %b21

b22:
  %118 = load ptr, ptr %107
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %118)
  store ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6), ptr %107
  %119 = getelementptr i8, ptr %95, i16 -4
  %120 = load i16, ptr %119
  %121 = icmp eq i16 %120, 0
  br i1 %121, label %b25, label %b24

b23:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b24:
  %122 = load ptr, ptr %95
  tail call cc 1005 addrspace(1) void @N$PS(ptr %122)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str17, i16 6))
  %123 = getelementptr i8, ptr %107, i16 -4
  %124 = load i16, ptr %123
  %125 = icmp eq i16 %124, 0
  br i1 %125, label %b27, label %b26

b25:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b26:
  %126 = load ptr, ptr %107
  tail call cc 1005 addrspace(1) void @N$PS(ptr %126)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str17, i16 6))
  %127 = getelementptr i8, ptr %107, i16 -4
  %128 = load i16, ptr %127
  %129 = icmp ugt i16 %128, 2
  br i1 %129, label %b31, label %b29

b27:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b31:
  %130 = getelementptr inbounds nuw i8, ptr %107, i16 4
  %131 = load ptr, ptr %130
  tail call cc 1005 addrspace(1) void @N$PS(ptr %131)
  tail call cc 1005 addrspace(1) void @N$PN()
  %132 = getelementptr i8, ptr %107, i16 -4
  %133 = load i16, ptr %132
  %134 = icmp eq i16 %133, 0
  br i1 %134, label %b36, label %135

135:
  br label %b34

b34:
  %136 = phi ptr [ %107, %135 ], [ %140, %b34 ]
  %137 = phi i16 [ %133, %135 ], [ %139, %b34 ]
  %138 = load ptr, ptr %136
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %138)
  %139 = add i16 %137, -1
  %140 = getelementptr i8, ptr %136, i16 2
  %exitcond3 = icmp eq i16 %139, 0
  br i1 %exitcond3, label %b36, label %b34

b36:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %107)
  %141 = getelementptr i8, ptr %95, i16 -4
  %142 = load i16, ptr %141
  %143 = icmp eq i16 %142, 0
  br i1 %143, label %b41, label %144

144:
  br label %b39

b39:
  %145 = phi ptr [ %95, %144 ], [ %149, %b39 ]
  %146 = phi i16 [ %142, %144 ], [ %148, %b39 ]
  %147 = load ptr, ptr %145
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %147)
  %148 = add i16 %146, -1
  %149 = getelementptr i8, ptr %145, i16 2
  %exitcond2 = icmp eq i16 %148, 0
  br i1 %exitcond2, label %b41, label %b39

b40:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %13)
  ret i16 0

b41:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %95)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %82)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %77)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %60)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %150 = getelementptr i8, ptr %13, i16 -4
  %151 = load i16, ptr %150
  %152 = icmp eq i16 %151, 0
  br i1 %152, label %b40, label %153

153:
  br label %b44

b44:
  %154 = phi ptr [ %13, %153 ], [ %158, %b44 ]
  %155 = phi i16 [ %151, %153 ], [ %157, %b44 ]
  %156 = load ptr, ptr %154
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %156)
  %157 = add i16 %155, -1
  %158 = getelementptr i8, ptr %154, i16 4
  %exitcond1 = icmp eq i16 %157, 0
  br i1 %exitcond1, label %b40, label %b44
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$TCAT(ptr, ptr) addrspace(1)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i16 @N$BSHR(ptr, i16) addrspace(1)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
