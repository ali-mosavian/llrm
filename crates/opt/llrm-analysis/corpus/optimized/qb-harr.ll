target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"M%" = internal global [22 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"T=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 20, i16 0, i16 20, i16 2, i16 258, ptr @"M%")
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"M%", i16 2), !tbaa !2
  %1 = inttoptr i16 %0 to ptr addrspace(2)
  %2 = addrspacecast ptr addrspace(2) %1 to ptr addrspace(1)
  %3 = getelementptr i8, ptr addrspace(1) %2, i16 44
  store i16 2, ptr addrspace(1) %3, !tbaa !6
  %4 = getelementptr i8, ptr addrspace(1) %2, i16 86
  store i16 3, ptr addrspace(1) %4, !tbaa !6
  %5 = getelementptr i8, ptr addrspace(1) %2, i16 128
  store i16 4, ptr addrspace(1) %5, !tbaa !6
  %6 = getelementptr i8, ptr addrspace(1) %2, i16 170
  store i16 5, ptr addrspace(1) %6, !tbaa !6
  %7 = getelementptr i8, ptr addrspace(1) %2, i16 212
  store i16 6, ptr addrspace(1) %7, !tbaa !6
  %8 = getelementptr i8, ptr addrspace(1) %2, i16 254
  store i16 7, ptr addrspace(1) %8, !tbaa !6
  %9 = getelementptr i8, ptr addrspace(1) %2, i16 296
  store i16 8, ptr addrspace(1) %9, !tbaa !6
  %10 = getelementptr i8, ptr addrspace(1) %2, i16 338
  store i16 9, ptr addrspace(1) %10, !tbaa !6
  %11 = getelementptr i8, ptr addrspace(1) %2, i16 380
  store i16 10, ptr addrspace(1) %11, !tbaa !6
  %12 = getelementptr i8, ptr addrspace(1) %2, i16 422
  store i16 11, ptr addrspace(1) %12, !tbaa !6
  %13 = getelementptr i8, ptr addrspace(1) %2, i16 46
  store i16 3, ptr addrspace(1) %13, !tbaa !6
  %14 = getelementptr i8, ptr addrspace(1) %2, i16 88
  store i16 4, ptr addrspace(1) %14, !tbaa !6
  %15 = getelementptr i8, ptr addrspace(1) %2, i16 130
  store i16 5, ptr addrspace(1) %15, !tbaa !6
  %16 = getelementptr i8, ptr addrspace(1) %2, i16 172
  store i16 6, ptr addrspace(1) %16, !tbaa !6
  %17 = getelementptr i8, ptr addrspace(1) %2, i16 214
  store i16 7, ptr addrspace(1) %17, !tbaa !6
  %18 = getelementptr i8, ptr addrspace(1) %2, i16 256
  store i16 8, ptr addrspace(1) %18, !tbaa !6
  %19 = getelementptr i8, ptr addrspace(1) %2, i16 298
  store i16 9, ptr addrspace(1) %19, !tbaa !6
  %20 = getelementptr i8, ptr addrspace(1) %2, i16 340
  store i16 10, ptr addrspace(1) %20, !tbaa !6
  %21 = getelementptr i8, ptr addrspace(1) %2, i16 382
  store i16 11, ptr addrspace(1) %21, !tbaa !6
  %22 = getelementptr i8, ptr addrspace(1) %2, i16 424
  store i16 12, ptr addrspace(1) %22, !tbaa !6
  %23 = getelementptr i8, ptr addrspace(1) %2, i16 48
  store i16 4, ptr addrspace(1) %23, !tbaa !6
  %24 = getelementptr i8, ptr addrspace(1) %2, i16 90
  store i16 5, ptr addrspace(1) %24, !tbaa !6
  %25 = getelementptr i8, ptr addrspace(1) %2, i16 132
  store i16 6, ptr addrspace(1) %25, !tbaa !6
  %26 = getelementptr i8, ptr addrspace(1) %2, i16 174
  store i16 7, ptr addrspace(1) %26, !tbaa !6
  %27 = getelementptr i8, ptr addrspace(1) %2, i16 216
  store i16 8, ptr addrspace(1) %27, !tbaa !6
  %28 = getelementptr i8, ptr addrspace(1) %2, i16 258
  store i16 9, ptr addrspace(1) %28, !tbaa !6
  %29 = getelementptr i8, ptr addrspace(1) %2, i16 300
  store i16 10, ptr addrspace(1) %29, !tbaa !6
  %30 = getelementptr i8, ptr addrspace(1) %2, i16 342
  store i16 11, ptr addrspace(1) %30, !tbaa !6
  %31 = getelementptr i8, ptr addrspace(1) %2, i16 384
  store i16 12, ptr addrspace(1) %31, !tbaa !6
  %32 = getelementptr i8, ptr addrspace(1) %2, i16 426
  store i16 13, ptr addrspace(1) %32, !tbaa !6
  %33 = getelementptr i8, ptr addrspace(1) %2, i16 50
  store i16 5, ptr addrspace(1) %33, !tbaa !6
  %34 = getelementptr i8, ptr addrspace(1) %2, i16 92
  store i16 6, ptr addrspace(1) %34, !tbaa !6
  %35 = getelementptr i8, ptr addrspace(1) %2, i16 134
  store i16 7, ptr addrspace(1) %35, !tbaa !6
  %36 = getelementptr i8, ptr addrspace(1) %2, i16 176
  store i16 8, ptr addrspace(1) %36, !tbaa !6
  %37 = getelementptr i8, ptr addrspace(1) %2, i16 218
  store i16 9, ptr addrspace(1) %37, !tbaa !6
  %38 = getelementptr i8, ptr addrspace(1) %2, i16 260
  store i16 10, ptr addrspace(1) %38, !tbaa !6
  %39 = getelementptr i8, ptr addrspace(1) %2, i16 302
  store i16 11, ptr addrspace(1) %39, !tbaa !6
  %40 = getelementptr i8, ptr addrspace(1) %2, i16 344
  store i16 12, ptr addrspace(1) %40, !tbaa !6
  %41 = getelementptr i8, ptr addrspace(1) %2, i16 386
  store i16 13, ptr addrspace(1) %41, !tbaa !6
  %42 = getelementptr i8, ptr addrspace(1) %2, i16 428
  store i16 14, ptr addrspace(1) %42, !tbaa !6
  %43 = getelementptr i8, ptr addrspace(1) %2, i16 52
  store i16 6, ptr addrspace(1) %43, !tbaa !6
  %44 = getelementptr i8, ptr addrspace(1) %2, i16 94
  store i16 7, ptr addrspace(1) %44, !tbaa !6
  %45 = getelementptr i8, ptr addrspace(1) %2, i16 136
  store i16 8, ptr addrspace(1) %45, !tbaa !6
  %46 = getelementptr i8, ptr addrspace(1) %2, i16 178
  store i16 9, ptr addrspace(1) %46, !tbaa !6
  %47 = getelementptr i8, ptr addrspace(1) %2, i16 220
  store i16 10, ptr addrspace(1) %47, !tbaa !6
  %48 = getelementptr i8, ptr addrspace(1) %2, i16 262
  store i16 11, ptr addrspace(1) %48, !tbaa !6
  %49 = getelementptr i8, ptr addrspace(1) %2, i16 304
  store i16 12, ptr addrspace(1) %49, !tbaa !6
  %50 = getelementptr i8, ptr addrspace(1) %2, i16 346
  store i16 13, ptr addrspace(1) %50, !tbaa !6
  %51 = getelementptr i8, ptr addrspace(1) %2, i16 388
  store i16 14, ptr addrspace(1) %51, !tbaa !6
  %52 = getelementptr i8, ptr addrspace(1) %2, i16 430
  store i16 15, ptr addrspace(1) %52, !tbaa !6
  %53 = getelementptr i8, ptr addrspace(1) %2, i16 54
  store i16 7, ptr addrspace(1) %53, !tbaa !6
  %54 = getelementptr i8, ptr addrspace(1) %2, i16 96
  store i16 8, ptr addrspace(1) %54, !tbaa !6
  %55 = getelementptr i8, ptr addrspace(1) %2, i16 138
  store i16 9, ptr addrspace(1) %55, !tbaa !6
  %56 = getelementptr i8, ptr addrspace(1) %2, i16 180
  store i16 10, ptr addrspace(1) %56, !tbaa !6
  %57 = getelementptr i8, ptr addrspace(1) %2, i16 222
  store i16 11, ptr addrspace(1) %57, !tbaa !6
  %58 = getelementptr i8, ptr addrspace(1) %2, i16 264
  store i16 12, ptr addrspace(1) %58, !tbaa !6
  %59 = getelementptr i8, ptr addrspace(1) %2, i16 306
  store i16 13, ptr addrspace(1) %59, !tbaa !6
  %60 = getelementptr i8, ptr addrspace(1) %2, i16 348
  store i16 14, ptr addrspace(1) %60, !tbaa !6
  %61 = getelementptr i8, ptr addrspace(1) %2, i16 390
  store i16 15, ptr addrspace(1) %61, !tbaa !6
  %62 = getelementptr i8, ptr addrspace(1) %2, i16 432
  store i16 16, ptr addrspace(1) %62, !tbaa !6
  %63 = getelementptr i8, ptr addrspace(1) %2, i16 56
  store i16 8, ptr addrspace(1) %63, !tbaa !6
  %64 = getelementptr i8, ptr addrspace(1) %2, i16 98
  store i16 9, ptr addrspace(1) %64, !tbaa !6
  %65 = getelementptr i8, ptr addrspace(1) %2, i16 140
  store i16 10, ptr addrspace(1) %65, !tbaa !6
  %66 = getelementptr i8, ptr addrspace(1) %2, i16 182
  store i16 11, ptr addrspace(1) %66, !tbaa !6
  %67 = getelementptr i8, ptr addrspace(1) %2, i16 224
  store i16 12, ptr addrspace(1) %67, !tbaa !6
  %68 = getelementptr i8, ptr addrspace(1) %2, i16 266
  store i16 13, ptr addrspace(1) %68, !tbaa !6
  %69 = getelementptr i8, ptr addrspace(1) %2, i16 308
  store i16 14, ptr addrspace(1) %69, !tbaa !6
  %70 = getelementptr i8, ptr addrspace(1) %2, i16 350
  store i16 15, ptr addrspace(1) %70, !tbaa !6
  %71 = getelementptr i8, ptr addrspace(1) %2, i16 392
  store i16 16, ptr addrspace(1) %71, !tbaa !6
  %72 = getelementptr i8, ptr addrspace(1) %2, i16 434
  store i16 17, ptr addrspace(1) %72, !tbaa !6
  %73 = getelementptr i8, ptr addrspace(1) %2, i16 58
  store i16 9, ptr addrspace(1) %73, !tbaa !6
  %74 = getelementptr i8, ptr addrspace(1) %2, i16 100
  store i16 10, ptr addrspace(1) %74, !tbaa !6
  %75 = getelementptr i8, ptr addrspace(1) %2, i16 142
  store i16 11, ptr addrspace(1) %75, !tbaa !6
  %76 = getelementptr i8, ptr addrspace(1) %2, i16 184
  store i16 12, ptr addrspace(1) %76, !tbaa !6
  %77 = getelementptr i8, ptr addrspace(1) %2, i16 226
  store i16 13, ptr addrspace(1) %77, !tbaa !6
  %78 = getelementptr i8, ptr addrspace(1) %2, i16 268
  store i16 14, ptr addrspace(1) %78, !tbaa !6
  %79 = getelementptr i8, ptr addrspace(1) %2, i16 310
  store i16 15, ptr addrspace(1) %79, !tbaa !6
  %80 = getelementptr i8, ptr addrspace(1) %2, i16 352
  store i16 16, ptr addrspace(1) %80, !tbaa !6
  %81 = getelementptr i8, ptr addrspace(1) %2, i16 394
  store i16 17, ptr addrspace(1) %81, !tbaa !6
  %82 = getelementptr i8, ptr addrspace(1) %2, i16 436
  store i16 18, ptr addrspace(1) %82, !tbaa !6
  %83 = getelementptr i8, ptr addrspace(1) %2, i16 60
  store i16 10, ptr addrspace(1) %83, !tbaa !6
  %84 = getelementptr i8, ptr addrspace(1) %2, i16 102
  store i16 11, ptr addrspace(1) %84, !tbaa !6
  %85 = getelementptr i8, ptr addrspace(1) %2, i16 144
  store i16 12, ptr addrspace(1) %85, !tbaa !6
  %86 = getelementptr i8, ptr addrspace(1) %2, i16 186
  store i16 13, ptr addrspace(1) %86, !tbaa !6
  %87 = getelementptr i8, ptr addrspace(1) %2, i16 228
  store i16 14, ptr addrspace(1) %87, !tbaa !6
  %88 = getelementptr i8, ptr addrspace(1) %2, i16 270
  store i16 15, ptr addrspace(1) %88, !tbaa !6
  %89 = getelementptr i8, ptr addrspace(1) %2, i16 312
  store i16 16, ptr addrspace(1) %89, !tbaa !6
  %90 = getelementptr i8, ptr addrspace(1) %2, i16 354
  store i16 17, ptr addrspace(1) %90, !tbaa !6
  %91 = getelementptr i8, ptr addrspace(1) %2, i16 396
  store i16 18, ptr addrspace(1) %91, !tbaa !6
  %92 = getelementptr i8, ptr addrspace(1) %2, i16 438
  store i16 19, ptr addrspace(1) %92, !tbaa !6
  %93 = getelementptr i8, ptr addrspace(1) %2, i16 62
  store i16 11, ptr addrspace(1) %93, !tbaa !6
  %94 = getelementptr i8, ptr addrspace(1) %2, i16 104
  store i16 12, ptr addrspace(1) %94, !tbaa !6
  %95 = getelementptr i8, ptr addrspace(1) %2, i16 146
  store i16 13, ptr addrspace(1) %95, !tbaa !6
  %96 = getelementptr i8, ptr addrspace(1) %2, i16 188
  store i16 14, ptr addrspace(1) %96, !tbaa !6
  %97 = getelementptr i8, ptr addrspace(1) %2, i16 230
  store i16 15, ptr addrspace(1) %97, !tbaa !6
  %98 = getelementptr i8, ptr addrspace(1) %2, i16 272
  store i16 16, ptr addrspace(1) %98, !tbaa !6
  %99 = getelementptr i8, ptr addrspace(1) %2, i16 314
  store i16 17, ptr addrspace(1) %99, !tbaa !6
  %100 = getelementptr i8, ptr addrspace(1) %2, i16 356
  store i16 18, ptr addrspace(1) %100, !tbaa !6
  %101 = getelementptr i8, ptr addrspace(1) %2, i16 398
  store i16 19, ptr addrspace(1) %101, !tbaa !6
  %102 = getelementptr i8, ptr addrspace(1) %2, i16 440
  store i16 20, ptr addrspace(1) %102, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 1100)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
