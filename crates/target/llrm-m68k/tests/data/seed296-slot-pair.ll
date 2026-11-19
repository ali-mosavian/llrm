; ModuleID = 'p.c'
source_filename = "p.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@zero = external constant i8, align 1
@buf = external local_unnamed_addr constant [16 x i8], align 1

; Function Attrs: nofree norecurse nosync nounwind memory(inaccessiblemem: readwrite)
define dso_local zeroext i16 @test() local_unnamed_addr #0 {
  %1 = alloca [8 x i8], align 1
  %2 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %3 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %4 = load volatile i8, ptr @zero, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %5 = load volatile i8, ptr @zero, align 1, !tbaa !8
  br label %6

6:                                                ; preds = %0, %6
  %7 = phi i32 [ 0, %0 ], [ %13, %6 ]
  %8 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %9 = trunc i32 %7 to i8
  %10 = add i8 %9, 6
  %11 = add i8 %10, %8
  %12 = getelementptr inbounds nuw i8, ptr %1, i32 %7
  store i8 %11, ptr %12, align 1, !tbaa !8
  %13 = add nuw nsw i32 %7, 1
  %14 = icmp eq i32 %13, 8
  br i1 %14, label %15, label %6, !llvm.loop !9

15:                                               ; preds = %6
  %16 = zext i8 %3 to i32
  %17 = add nuw nsw i32 %16, 32767
  %18 = zext i8 %5 to i16
  %19 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %20 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %21 = zext i8 %20 to i32
  %22 = add nuw nsw i32 %21, 19728
  %23 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %24 = add i8 %23, -118
  %25 = tail call fastcc i32 @h0()
  %26 = xor i32 %25, %17
  %27 = shl nuw nsw i16 %18, 1
  %28 = add nuw nsw i16 %27, 732
  %29 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 10), align 1, !tbaa !8
  %30 = zext i8 %29 to i16
  %31 = add nsw i16 %30, -1
  %32 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 12), align 1, !tbaa !8
  %33 = lshr i8 %32, 5
  %34 = zext nneg i8 %33 to i32
  %35 = getelementptr inbounds nuw i8, ptr @buf, i32 %34
  %36 = load i8, ptr %35, align 1, !tbaa !8
  %37 = xor i16 %31, %28
  %38 = tail call i16 @llvm.fshl.i16(i16 %37, i16 %31, i16 1)
  %39 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 1), align 1, !tbaa !8
  %40 = zext i8 %39 to i16
  %41 = tail call fastcc i32 @h0()
  %42 = and i32 %41, 15
  %43 = getelementptr inbounds nuw i8, ptr @buf, i32 %42
  %44 = load i8, ptr %43, align 1, !tbaa !8
  %45 = zext i8 %44 to i16
  %46 = add nuw nsw i16 %45, %40
  %47 = xor i16 %46, %38
  %48 = zext i8 %24 to i32
  %49 = and i32 %48, 7
  %50 = getelementptr inbounds nuw i8, ptr %1, i32 %49
  %51 = load i8, ptr %50, align 1, !tbaa !8
  %52 = zext i8 %51 to i16
  %53 = zext i16 %47 to i32
  %54 = shl nuw nsw i32 %53, 1
  %55 = lshr i32 %53, 15
  %56 = or disjoint i32 %54, %55
  %57 = xor i32 %56, %22
  br label %58

58:                                               ; preds = %15, %58
  %59 = phi i16 [ %52, %15 ], [ %69, %58 ]
  %60 = phi i32 [ %57, %15 ], [ %83, %58 ]
  %61 = phi i8 [ 3, %15 ], [ %62, %58 ]
  %62 = add nsw i8 %61, -1
  %63 = trunc nuw i16 %59 to i8
  %64 = xor i8 %62, %63
  %65 = zext i8 %64 to i32
  %66 = tail call fastcc i32 @h0()
  %67 = and i32 %66, 255
  %68 = icmp samesign uge i32 %67, %65
  %69 = zext i1 %68 to i16
  %70 = and i32 %60, 65535
  %71 = shl nuw nsw i32 %70, 1
  %72 = lshr i32 %70, 15
  %73 = sext i1 %68 to i32
  %74 = tail call fastcc i32 @h0()
  %75 = trunc i32 %74 to i8
  %76 = add nsw i32 %26, %73
  %77 = and i32 %76, 7
  %78 = getelementptr inbounds nuw i8, ptr %1, i32 %77
  store i8 %75, ptr %78, align 1, !tbaa !8
  %79 = and i32 %71, 65534
  %80 = or disjoint i32 %79, %72
  %81 = shl nuw nsw i32 %80, 1
  %82 = lshr i32 %79, 15
  %83 = or disjoint i32 %81, %82
  %84 = icmp eq i8 %62, 0
  br i1 %84, label %85, label %58, !llvm.loop !12

85:                                               ; preds = %58
  %86 = add i8 %19, 54
  %87 = icmp eq i8 %86, 127
  br i1 %87, label %88, label %91

88:                                               ; preds = %85
  %89 = tail call fastcc i32 @h0()
  %90 = trunc i32 %89 to i8
  br label %91

91:                                               ; preds = %85, %88
  %92 = phi i8 [ %90, %88 ], [ 92, %85 ]
  %93 = zext i1 %68 to i32
  %94 = getelementptr inbounds nuw i8, ptr %1, i32 %93
  store i8 %92, ptr %94, align 1, !tbaa !8
  %95 = shl i32 %26, 24
  %96 = ashr i32 %95, 28
  %97 = lshr i8 %36, 3
  %98 = zext nneg i8 %97 to i32
  %99 = sub nsw i32 %96, %98
  %100 = shl nuw nsw i32 %83, 2
  %101 = lshr i32 %60, 12
  %102 = and i32 %101, 2
  %103 = lshr i32 %60, 12
  %104 = and i32 %103, 1
  %105 = tail call fastcc i32 @h0()
  %106 = and i32 %105, 255
  %107 = icmp samesign ult i32 %106, %48
  %108 = zext i1 %107 to i32
  %109 = add nuw nsw i32 %26, %108
  %110 = and i32 %100, 65532
  %111 = or disjoint i32 %110, %102
  %112 = or disjoint i32 %111, %104
  %113 = xor i32 %112, %26
  %114 = shl nuw nsw i32 %113, 1
  %115 = lshr i32 %113, 15
  %116 = or disjoint i32 %114, %115
  %117 = lshr i32 %109, 8
  %118 = xor i32 %116, %117
  %119 = xor i32 %118, %109
  %120 = and i32 %119, 65535
  %121 = shl nuw nsw i32 %120, 1
  %122 = lshr i32 %120, 15
  %123 = or disjoint i32 %121, %122
  %124 = trunc i32 %123 to i16
  %125 = xor i16 %124, %69
  %126 = zext i16 %125 to i32
  %127 = shl nuw nsw i32 %126, 1
  %128 = lshr i32 %126, 15
  %129 = or disjoint i32 %127, %128
  %130 = and i32 %99, 65535
  %131 = lshr i32 %130, 8
  %132 = xor i32 %131, %129
  %133 = xor i32 %132, %99
  br label %134

134:                                              ; preds = %91, %134
  %135 = phi i32 [ 0, %91 ], [ %145, %134 ]
  %136 = phi i32 [ %133, %91 ], [ %144, %134 ]
  %137 = and i32 %136, 65535
  %138 = shl nuw nsw i32 %137, 1
  %139 = lshr i32 %137, 15
  %140 = or disjoint i32 %138, %139
  %141 = getelementptr inbounds nuw i8, ptr %1, i32 %135
  %142 = load i8, ptr %141, align 1, !tbaa !8
  %143 = zext i8 %142 to i32
  %144 = xor i32 %140, %143
  %145 = add nuw nsw i32 %135, 1
  %146 = icmp eq i32 %145, 8
  br i1 %146, label %147, label %134, !llvm.loop !13

147:                                              ; preds = %134
  %148 = zext i8 %86 to i32
  %149 = and i32 %144, 65535
  %150 = shl nuw nsw i32 %149, 1
  %151 = lshr i32 %149, 15
  %152 = urem i32 %130, 32769
  %153 = add nuw nsw i32 %152, 1
  %154 = and i32 %150, 65534
  %155 = or disjoint i32 %154, %151
  %156 = xor i32 %153, %148
  %157 = xor i32 %156, %155
  %158 = xor i32 %157, %22
  %159 = xor i32 %158, %48
  %160 = trunc nuw i32 %159 to i16
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i16 %160
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nofree norecurse nosync nounwind memory(inaccessiblemem: readwrite)
define internal fastcc range(i32 0, 65536) i32 @h0() unnamed_addr #0 {
  %1 = alloca [8 x i16], align 2
  %2 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %3 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %4 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %5 = load volatile i8, ptr @zero, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %6 = load volatile i8, ptr @zero, align 1, !tbaa !8
  br label %7

7:                                                ; preds = %0, %7
  %8 = phi i32 [ 0, %0 ], [ %15, %7 ]
  %9 = add nuw nsw i32 %8, 5
  %10 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %11 = zext i8 %10 to i32
  %12 = add nuw nsw i32 %9, %11
  %13 = trunc nuw nsw i32 %12 to i16
  %14 = getelementptr inbounds nuw [2 x i8], ptr %1, i32 %8
  store i16 %13, ptr %14, align 2, !tbaa !14
  %15 = add nuw nsw i32 %8, 1
  %16 = icmp eq i32 %15, 8
  br i1 %16, label %17, label %7, !llvm.loop !16

17:                                               ; preds = %7
  %18 = zext i8 %2 to i16
  %19 = add nuw i16 %18, 32767
  %20 = zext i8 %3 to i32
  %21 = add nsw i32 %20, -1
  %22 = zext i8 %4 to i32
  %23 = add nuw i32 %22, 2147483647
  %24 = zext i8 %6 to i16
  %25 = trunc nsw i32 %21 to i16
  %26 = lshr i16 %25, 8
  %27 = getelementptr inbounds nuw i8, ptr %1, i32 12
  store i16 %26, ptr %27, align 2, !tbaa !14
  %28 = shl nuw nsw i16 %24, 2
  %29 = add nuw nsw i16 %28, 588
  %30 = lshr i16 %19, 8
  %31 = xor i16 %30, %29
  %32 = xor i16 %31, %19
  %33 = zext i16 %32 to i32
  %34 = shl nuw nsw i32 %33, 1
  %35 = lshr i32 %33, 15
  %36 = or disjoint i32 %34, %35
  %37 = lshr i32 %21, 8
  %38 = xor i32 %37, %36
  %39 = xor i32 %38, %21
  %40 = and i32 %39, 65535
  %41 = shl nuw nsw i32 %40, 1
  %42 = lshr i32 %40, 15
  %43 = or disjoint i32 %41, %42
  %44 = lshr i32 %23, 8
  %45 = xor i32 %44, %43
  %46 = xor i32 %45, %23
  %47 = and i32 %46, 65535
  %48 = shl nuw nsw i32 %47, 1
  %49 = lshr i32 %47, 15
  %50 = or disjoint i32 %48, %49
  br label %51

51:                                               ; preds = %17, %51
  %52 = phi i32 [ 0, %17 ], [ %62, %51 ]
  %53 = phi i32 [ %50, %17 ], [ %61, %51 ]
  %54 = and i32 %53, 65535
  %55 = shl nuw nsw i32 %54, 1
  %56 = lshr i32 %54, 15
  %57 = or disjoint i32 %55, %56
  %58 = getelementptr inbounds nuw [2 x i8], ptr %1, i32 %52
  %59 = load i16, ptr %58, align 2, !tbaa !14
  %60 = zext i16 %59 to i32
  %61 = xor i32 %57, %60
  %62 = add nuw nsw i32 %52, 1
  %63 = icmp eq i32 %62, 8
  br i1 %63, label %64, label %51, !llvm.loop !17

64:                                               ; preds = %51
  %65 = and i32 %23, 255
  %66 = and i32 %61, 65535
  %67 = xor i32 %66, %65
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i32 %67
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nofree norecurse nosync nounwind memory(inaccessiblemem: readwrite)
define dso_local zeroext i8 @k() local_unnamed_addr #0 {
  %1 = tail call zeroext i16 @test()
  %2 = lshr i16 %1, 8
  %3 = xor i16 %2, %1
  %4 = trunc i16 %3 to i8
  ret i8 %4
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.fshl.i16(i16, i16, i16) #2

attributes #0 = { nofree norecurse nosync nounwind memory(inaccessiblemem: readwrite) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!6, !6, i64 0}
!9 = distinct !{!9, !10, !11}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!"llvm.loop.unroll.disable"}
!12 = distinct !{!12, !10, !11}
!13 = distinct !{!13, !10, !11}
!14 = !{!15, !15, i64 0}
!15 = !{!"short", !6, i64 0}
!16 = distinct !{!16, !10, !11}
!17 = distinct !{!17, !10, !11}
