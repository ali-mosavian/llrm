target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [40 x i8] zeroinitializer
@$qb$statementTable = internal constant [560 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\04\00\00\00\03\00\00\00\00\00\01\00\00\00\05\00\00\00\04\00\00\00\00\00\01\00\00\00\0B\00\00\00\11\00\00\00\00\00\01\00\00\00\11\00\00\00\1D\00\00\00\00\00\01\00\00\00\12\00\00\00 \00\00\00\00\00\01\00\00\00\18\00\00\00-\00\00\00\00\00\01\00\00\00\1E\00\00\009\00\00\00\00\00\01\00\00\00\1F\00\00\00<\00\00\00\00\00\01\00\00\00%\00\00\00I\00\00\00\00\00\01\00\00\00+\00\00\00U\00\00\00\00\00\01\00\00\00,\00\00\00V\00\00\00\00\00\01\00\00\002\00\00\00c\00\00\00\00\00\01\00\00\008\00\00\00o\00\00\00\00\00\01\00\00\009\00\00\00p\00\00\00\00\00\01\00\00\00:\00\00\00q\00\00\00\00\00\01\00\00\00@\00\00\00~\00\00\00\00\00\01\00\00\00F\00\00\00\8A\00\00\00\00\00\01\00\00\00G\00\00\00\8F\00\00\00\00\00\01\00\00\00H\00\00\00\92\00\00\00\00\00\01\00\00\00I\00\00\00\97\00\00\00\00\00\01\00\00\00J\00\00\00\9C\00\00\00\00\00\01\00\00\00K\00\00\00\A1\00\00\00\00\00\01\00\00\00L\00\00\00\A6\00\00\00\00\00\01\00\00\00M\00\00\00\A8\00\00\00\00\00\01\00\00\00N\00\00\00\AD\00\00\00\00\00\01\00\00\00O\00\00\00\B2\00\00\00\00\00\01\00\00\00P\00\00\00\B3\00\00\00\00\00\01\00\00\00Q\00\00\00\B8\00\00\00\00\00\01\00\00\00R\00\00\00\BD\00\00\00\00\00\01\00\00\00S\00\00\00\BE\00\00\00\00\00\01\00\00\00T\00\00\00\BF\00\00\00\00\00\01\00\00\00U\00\00\00\C0\00\00\00\00\00\01\00\00\00V\00\00\00\C4\00\00\00\00\00\01\00\00\00W\00\00\00\C8\00\00\00\00\00\01\00\00\00X\00\00\00\CA\00\00\00\00\00\01\00\00\00\02\00\00\00\CB\00\00\00\00\00\01\00\00\00Z\00\00\00\CD\00\00\00\00\00\01\00\00\00[\00\00\00\CE\00\00\00\00\00"
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@"ZERO&" = internal global [4 x i8] zeroinitializer
@"LOW&" = internal global [4 x i8] zeroinitializer
@"CAUGHT%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"DIV1=\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"MOD1=\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"DIV2=\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"MOD2=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"DIV3=\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"MOD3=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"DIV4=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"MOD4=\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string11, i16 4), [8 x i8] c"DIVBIG=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string12, i16 4), [8 x i8] c"MODBIG=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string13, i16 4), [10 x i8] c"MULSMALL=\00" }>
@$string14 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string14, i16 4), [8 x i8] c"SHDIV1=\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string15, i16 4), [8 x i8] c"SHMOD1=\00" }>
@$string16 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string16, i16 4), [8 x i8] c"SHDIV2=\00" }>
@$string17 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string17, i16 4), [8 x i8] c"SHMOD2=\00" }>
@$string18 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string18, i16 4), [8 x i8] c"SHDIV3=\00" }>
@$string19 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string19, i16 4), [8 x i8] c"SHMOD3=\00" }>
@$string20 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string20, i16 4), [8 x i8] c"SHDIV4=\00" }>
@$string21 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string21, i16 4), [8 x i8] c"SHMOD4=\00" }>
@$string22 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string22, i16 4), [8 x i8] c"MULOVF=\00" }>
@$string23 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string23, i16 4), [4 x i8] c"DONE" }>
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 1, ptr @$QB$ACTIVE
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b3

b2:
  store i16 22, ptr %site
  %0 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %62 unwind label %landing

b3:
  store i32 7, ptr @"A&", !tbaa !2
  br label %b4

b4:
  store i32 2, ptr @"B&", !tbaa !2
  br label %b5

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %63 unwind label %landing

b6:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %71 unwind label %landing

b7:
  %1 = icmp eq i32 %65, -1
  br i1 %1, label %b8, label %b9

b8:
  %2 = sub i32 0, %64
  store i32 %2, ptr @$data, !tbaa !2
  br label %b10

b9:
  %3 = icmp eq i32 %65, 0
  br i1 %3, label %trap, label %67

b10:
  %4 = load i32, ptr @$data, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %4)
          to label %70 unwind label %landing

b11:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %72 unwind label %landing

b12:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %81 unwind label %landing

b13:
  %5 = icmp eq i32 %74, -1
  br i1 %5, label %b14, label %b15

b14:
  %6 = getelementptr i8, ptr @$data, i16 4
  store i32 0, ptr %6, !tbaa !2
  br label %b16

b15:
  %7 = icmp eq i32 %74, 0
  br i1 %7, label %trap1, label %76

b16:
  %8 = getelementptr i8, ptr @$data, i16 4
  %9 = load i32, ptr %8, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %9)
          to label %80 unwind label %landing

b17:
  store i32 -7, ptr @"A&", !tbaa !2
  br label %b18

b18:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %82 unwind label %landing

b19:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %91 unwind label %landing

b20:
  %10 = icmp eq i32 %84, -1
  br i1 %10, label %b21, label %b22

b21:
  %11 = sub i32 0, %83
  %12 = getelementptr i8, ptr @$data, i16 8
  store i32 %11, ptr %12, !tbaa !2
  br label %b23

b22:
  %13 = icmp eq i32 %84, 0
  br i1 %13, label %trap2, label %86

b23:
  %14 = getelementptr i8, ptr @$data, i16 8
  %15 = load i32, ptr %14, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %15)
          to label %90 unwind label %landing

b24:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %92 unwind label %landing

b25:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %101 unwind label %landing

b26:
  %16 = icmp eq i32 %94, -1
  br i1 %16, label %b27, label %b28

b27:
  %17 = getelementptr i8, ptr @$data, i16 12
  store i32 0, ptr %17, !tbaa !2
  br label %b29

b28:
  %18 = icmp eq i32 %94, 0
  br i1 %18, label %trap3, label %96

b29:
  %19 = getelementptr i8, ptr @$data, i16 12
  %20 = load i32, ptr %19, !tbaa !2
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %20)
          to label %100 unwind label %landing

b30:
  store i32 -2, ptr @"B&", !tbaa !2
  br label %b31

b31:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %102 unwind label %landing

b32:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %111 unwind label %landing

b33:
  %21 = icmp eq i32 %104, -1
  br i1 %21, label %b34, label %b35

b34:
  %22 = sub i32 0, %103
  %23 = getelementptr i8, ptr @$data, i16 16
  store i32 %22, ptr %23, !tbaa !2
  br label %b36

b35:
  %24 = icmp eq i32 %104, 0
  br i1 %24, label %trap4, label %106

b36:
  %25 = getelementptr i8, ptr @$data, i16 16
  %26 = load i32, ptr %25, !tbaa !2
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %26)
          to label %110 unwind label %landing

b37:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %112 unwind label %landing

b38:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %121 unwind label %landing

b39:
  %27 = icmp eq i32 %114, -1
  br i1 %27, label %b40, label %b41

b40:
  %28 = getelementptr i8, ptr @$data, i16 20
  store i32 0, ptr %28, !tbaa !2
  br label %b42

b41:
  %29 = icmp eq i32 %114, 0
  br i1 %29, label %trap5, label %116

b42:
  %30 = getelementptr i8, ptr @$data, i16 20
  %31 = load i32, ptr %30, !tbaa !2
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %31)
          to label %120 unwind label %landing

b43:
  store i32 7, ptr @"A&", !tbaa !2
  br label %b44

b44:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %122 unwind label %landing

b45:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %131 unwind label %landing

b46:
  %32 = icmp eq i32 %124, -1
  br i1 %32, label %b47, label %b48

b47:
  %33 = sub i32 0, %123
  %34 = getelementptr i8, ptr @$data, i16 24
  store i32 %33, ptr %34, !tbaa !2
  br label %b49

b48:
  %35 = icmp eq i32 %124, 0
  br i1 %35, label %trap6, label %126

b49:
  %36 = getelementptr i8, ptr @$data, i16 24
  %37 = load i32, ptr %36, !tbaa !2
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %37)
          to label %130 unwind label %landing

b50:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %132 unwind label %landing

b51:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %141 unwind label %landing

b52:
  %38 = icmp eq i32 %134, -1
  br i1 %38, label %b53, label %b54

b53:
  %39 = getelementptr i8, ptr @$data, i16 28
  store i32 0, ptr %39, !tbaa !2
  br label %b55

b54:
  %40 = icmp eq i32 %134, 0
  br i1 %40, label %trap7, label %136

b55:
  %41 = getelementptr i8, ptr @$data, i16 28
  %42 = load i32, ptr %41, !tbaa !2
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %42)
          to label %140 unwind label %landing

b56:
  store i32 305419896, ptr @"A&", !tbaa !2
  br label %b57

b57:
  store i32 252645135, ptr @"B&", !tbaa !2
  br label %b58

b58:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
          to label %142 unwind label %landing

b59:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %151 unwind label %landing

b60:
  %43 = icmp eq i32 %144, -1
  br i1 %43, label %b61, label %b62

b61:
  %44 = sub i32 0, %143
  %45 = getelementptr i8, ptr @$data, i16 32
  store i32 %44, ptr %45, !tbaa !2
  br label %b63

b62:
  %46 = icmp eq i32 %144, 0
  br i1 %46, label %trap8, label %146

b63:
  %47 = getelementptr i8, ptr @$data, i16 32
  %48 = load i32, ptr %47, !tbaa !2
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %48)
          to label %150 unwind label %landing

b64:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
          to label %152 unwind label %landing

b65:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %161 unwind label %landing

b66:
  %49 = icmp eq i32 %154, -1
  br i1 %49, label %b67, label %b68

b67:
  %50 = getelementptr i8, ptr @$data, i16 36
  store i32 0, ptr %50, !tbaa !2
  br label %b69

b68:
  %51 = icmp eq i32 %154, 0
  br i1 %51, label %trap9, label %156

b69:
  %52 = getelementptr i8, ptr @$data, i16 36
  %53 = load i32, ptr %52, !tbaa !2
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %53)
          to label %160 unwind label %landing

b70:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
          to label %162 unwind label %landing

b71:
  store i32 -7, ptr @"R&", !tbaa !2
  br label %b72

b72:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14)
          to label %166 unwind label %landing

b73:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15)
          to label %170 unwind label %landing

b74:
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16)
          to label %174 unwind label %landing

b75:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17)
          to label %178 unwind label %landing

b76:
  store i32 -1000000, ptr @"R&", !tbaa !2
  br label %b77

b77:
  store i16 15, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18)
          to label %182 unwind label %landing

b78:
  store i16 16, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19)
          to label %186 unwind label %landing

b79:
  store i32 1000000, ptr @"R&", !tbaa !2
  br label %b80

b80:
  store i16 17, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20)
          to label %190 unwind label %landing

b81:
  store i16 18, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string21)
          to label %194 unwind label %landing

b82:
  store i32 305419896, ptr @"A&", !tbaa !2
  br label %b83

b83:
  store i32 252645135, ptr @"B&", !tbaa !2
  br label %b84

b84:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b85

b85:
  %54 = load i32, ptr @"A&", !tbaa !2
  %55 = load i32, ptr @"B&", !tbaa !2
  %56 = mul i32 %54, %55
  store i32 %56, ptr @"R&", !tbaa !2
  br label %b86

b86:
  store i16 19, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string22)
          to label %198 unwind label %landing

b87:
  store i16 20, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string23)
          to label %201 unwind label %landing

b88:
  store i16 21, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %202 unwind label %landing

b90:
  store i16 23, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %203 unwind label %landing

b91:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %57 = extractvalue { ptr, i32 } %landed, 1
  %58 = trunc i32 %57 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %58, i16 0)
  switch i16 %resumed, label %61 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %59 = load i16, ptr %site
  switch i16 %59, label %204 [
    i16 0, label %b5
    i16 1, label %b11
    i16 2, label %b18
    i16 3, label %b24
    i16 4, label %b31
    i16 5, label %b37
    i16 6, label %b44
    i16 7, label %b50
    i16 8, label %b58
    i16 9, label %b64
    i16 10, label %b70
    i16 11, label %b72
    i16 12, label %b73
    i16 13, label %b74
    i16 14, label %b75
    i16 15, label %b77
    i16 16, label %b78
    i16 17, label %b80
    i16 18, label %b81
    i16 19, label %b86
    i16 20, label %b87
    i16 21, label %b88
    i16 22, label %b2
    i16 23, label %b90
  ]

next:
  %60 = load i16, ptr %site
  switch i16 %60, label %205 [
    i16 0, label %b11
    i16 1, label %b17
    i16 2, label %b24
    i16 3, label %b30
    i16 4, label %b37
    i16 5, label %b43
    i16 6, label %b50
    i16 7, label %b56
    i16 8, label %b64
    i16 9, label %b70
    i16 10, label %b71
    i16 11, label %b73
    i16 12, label %b74
    i16 13, label %b75
    i16 14, label %b76
    i16 15, label %b78
    i16 16, label %b79
    i16 17, label %b81
    i16 18, label %b82
    i16 19, label %b87
    i16 20, label %b88
    i16 21, label %b2
    i16 22, label %b90
    i16 23, label %b91
  ]

61:
  unreachable

62:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  br label %b90

63:
  %64 = load i32, ptr @"A&", !tbaa !2
  %65 = load i32, ptr @"B&", !tbaa !2
  %66 = icmp eq i32 %65, 0
  br i1 %66, label %b6, label %b7

trap:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %69 unwind label %landing

67:
  %68 = sdiv i32 %64, %65
  store i32 %68, ptr @$data, !tbaa !2
  br label %b10

69:
  unreachable

70:
  br label %b11

71:
  unreachable

72:
  %73 = load i32, ptr @"A&", !tbaa !2
  %74 = load i32, ptr @"B&", !tbaa !2
  %75 = icmp eq i32 %74, 0
  br i1 %75, label %b12, label %b13

trap1:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %79 unwind label %landing

76:
  %77 = srem i32 %73, %74
  %78 = getelementptr i8, ptr @$data, i16 4
  store i32 %77, ptr %78, !tbaa !2
  br label %b16

79:
  unreachable

80:
  br label %b17

81:
  unreachable

82:
  %83 = load i32, ptr @"A&", !tbaa !2
  %84 = load i32, ptr @"B&", !tbaa !2
  %85 = icmp eq i32 %84, 0
  br i1 %85, label %b19, label %b20

trap2:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %89 unwind label %landing

86:
  %87 = sdiv i32 %83, %84
  %88 = getelementptr i8, ptr @$data, i16 8
  store i32 %87, ptr %88, !tbaa !2
  br label %b23

89:
  unreachable

90:
  br label %b24

91:
  unreachable

92:
  %93 = load i32, ptr @"A&", !tbaa !2
  %94 = load i32, ptr @"B&", !tbaa !2
  %95 = icmp eq i32 %94, 0
  br i1 %95, label %b25, label %b26

trap3:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %99 unwind label %landing

96:
  %97 = srem i32 %93, %94
  %98 = getelementptr i8, ptr @$data, i16 12
  store i32 %97, ptr %98, !tbaa !2
  br label %b29

99:
  unreachable

100:
  br label %b30

101:
  unreachable

102:
  %103 = load i32, ptr @"A&", !tbaa !2
  %104 = load i32, ptr @"B&", !tbaa !2
  %105 = icmp eq i32 %104, 0
  br i1 %105, label %b32, label %b33

trap4:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %109 unwind label %landing

106:
  %107 = sdiv i32 %103, %104
  %108 = getelementptr i8, ptr @$data, i16 16
  store i32 %107, ptr %108, !tbaa !2
  br label %b36

109:
  unreachable

110:
  br label %b37

111:
  unreachable

112:
  %113 = load i32, ptr @"A&", !tbaa !2
  %114 = load i32, ptr @"B&", !tbaa !2
  %115 = icmp eq i32 %114, 0
  br i1 %115, label %b38, label %b39

trap5:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %119 unwind label %landing

116:
  %117 = srem i32 %113, %114
  %118 = getelementptr i8, ptr @$data, i16 20
  store i32 %117, ptr %118, !tbaa !2
  br label %b42

119:
  unreachable

120:
  br label %b43

121:
  unreachable

122:
  %123 = load i32, ptr @"A&", !tbaa !2
  %124 = load i32, ptr @"B&", !tbaa !2
  %125 = icmp eq i32 %124, 0
  br i1 %125, label %b45, label %b46

trap6:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %129 unwind label %landing

126:
  %127 = sdiv i32 %123, %124
  %128 = getelementptr i8, ptr @$data, i16 24
  store i32 %127, ptr %128, !tbaa !2
  br label %b49

129:
  unreachable

130:
  br label %b50

131:
  unreachable

132:
  %133 = load i32, ptr @"A&", !tbaa !2
  %134 = load i32, ptr @"B&", !tbaa !2
  %135 = icmp eq i32 %134, 0
  br i1 %135, label %b51, label %b52

trap7:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %139 unwind label %landing

136:
  %137 = srem i32 %133, %134
  %138 = getelementptr i8, ptr @$data, i16 28
  store i32 %137, ptr %138, !tbaa !2
  br label %b55

139:
  unreachable

140:
  br label %b56

141:
  unreachable

142:
  %143 = load i32, ptr @"B&", !tbaa !2
  %144 = load i32, ptr @"A&", !tbaa !2
  %145 = icmp eq i32 %144, 0
  br i1 %145, label %b59, label %b60

trap8:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %149 unwind label %landing

146:
  %147 = sdiv i32 %143, %144
  %148 = getelementptr i8, ptr @$data, i16 32
  store i32 %147, ptr %148, !tbaa !2
  br label %b63

149:
  unreachable

150:
  br label %b64

151:
  unreachable

152:
  %153 = load i32, ptr @"B&", !tbaa !2
  %154 = load i32, ptr @"A&", !tbaa !2
  %155 = icmp eq i32 %154, 0
  br i1 %155, label %b65, label %b66

trap9:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %159 unwind label %landing

156:
  %157 = srem i32 %153, %154
  %158 = getelementptr i8, ptr @$data, i16 36
  store i32 %157, ptr %158, !tbaa !2
  br label %b69

159:
  unreachable

160:
  br label %b70

161:
  unreachable

162:
  %163 = load i32, ptr @"A&", !tbaa !2
  %164 = mul i32 %163, 3
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %164)
          to label %165 unwind label %landing

165:
  br label %b71

166:
  %167 = load i32, ptr @"R&", !tbaa !2
  %168 = sdiv i32 %167, 2
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %168)
          to label %169 unwind label %landing

169:
  br label %b73

170:
  %171 = load i32, ptr @"R&", !tbaa !2
  %172 = srem i32 %171, 2
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %172)
          to label %173 unwind label %landing

173:
  br label %b74

174:
  %175 = load i32, ptr @"R&", !tbaa !2
  %176 = sdiv i32 %175, 512
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %176)
          to label %177 unwind label %landing

177:
  br label %b75

178:
  %179 = load i32, ptr @"R&", !tbaa !2
  %180 = srem i32 %179, 512
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %180)
          to label %181 unwind label %landing

181:
  br label %b76

182:
  %183 = load i32, ptr @"R&", !tbaa !2
  %184 = sdiv i32 %183, 512
  store i16 15, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %184)
          to label %185 unwind label %landing

185:
  br label %b78

186:
  %187 = load i32, ptr @"R&", !tbaa !2
  %188 = srem i32 %187, 512
  store i16 16, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %188)
          to label %189 unwind label %landing

189:
  br label %b79

190:
  %191 = load i32, ptr @"R&", !tbaa !2
  %192 = sdiv i32 %191, 512
  store i16 17, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %192)
          to label %193 unwind label %landing

193:
  br label %b81

194:
  %195 = load i32, ptr @"R&", !tbaa !2
  %196 = srem i32 %195, 512
  store i16 18, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %196)
          to label %197 unwind label %landing

197:
  br label %b82

198:
  %199 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 19, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %199)
          to label %200 unwind label %landing

200:
  br label %b87

201:
  br label %b88

202:
  unreachable

203:
  unreachable

204:
  unreachable

205:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

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
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  br label %b90

b90:
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
