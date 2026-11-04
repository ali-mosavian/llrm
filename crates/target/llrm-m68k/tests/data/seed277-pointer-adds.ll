; ModuleID = 'p.c'
source_filename = "p.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@zero = external constant i8, align 1
@buf = external local_unnamed_addr constant [16 x i8], align 1

; Function Attrs: nofree norecurse nosync nounwind memory(inaccessiblemem: readwrite)
define dso_local zeroext i16 @test() local_unnamed_addr #0 {
  %1 = alloca [8 x i16], align 2
  %2 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %3 = load volatile i8, ptr @zero, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %4 = load volatile i8, ptr @zero, align 1, !tbaa !8
  br label %5

5:                                                ; preds = %0, %5
  %6 = phi i32 [ 0, %0 ], [ %13, %5 ]
  %7 = add nuw nsw i32 %6, 5
  %8 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %9 = zext i8 %8 to i32
  %10 = add nuw nsw i32 %7, %9
  %11 = trunc nuw nsw i32 %10 to i16
  %12 = getelementptr inbounds nuw [2 x i8], ptr %1, i32 %6
  store i16 %11, ptr %12, align 2, !tbaa !9
  %13 = add nuw nsw i32 %6, 1
  %14 = icmp eq i32 %13, 8
  br i1 %14, label %15, label %5, !llvm.loop !11

15:                                               ; preds = %5
  %16 = add i8 %2, -1
  %17 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %18 = add i8 %17, -39
  %19 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %20 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %21 = getelementptr inbounds nuw i8, ptr %1, i32 10
  %22 = load i16, ptr %21, align 2, !tbaa !9
  %23 = lshr i8 %16, 7
  %24 = zext nneg i8 %23 to i32
  %25 = getelementptr inbounds nuw i8, ptr @buf, i32 %24
  %26 = load i8, ptr %25, align 1, !tbaa !8
  %27 = zext i8 %26 to i16
  %28 = icmp ugt i16 %22, %27
  %29 = load i16, ptr %1, align 2, !tbaa !9
  %30 = ashr i16 %29, 9
  %31 = and i16 %30, 255
  %32 = xor i16 %31, 128
  %33 = select i1 %28, i16 %32, i16 128
  %34 = zext nneg i16 %33 to i32
  %35 = shl nuw nsw i32 %34, 1
  %36 = icmp ne i8 %16, 0
  %37 = zext i1 %36 to i32
  %38 = icmp eq i32 %35, %37
  %39 = zext i1 %38 to i8
  %40 = add i8 %18, %39
  %41 = zext i8 %16 to i32
  %42 = add nuw nsw i32 %34, %41
  %43 = zext i8 %40 to i32
  %44 = icmp samesign uge i32 %34, %43
  %45 = zext i1 %44 to i32
  %46 = icmp eq i32 %42, %45
  %47 = sext i1 %46 to i8
  %48 = zext i1 %46 to i32
  %49 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 6), align 1, !tbaa !8
  %50 = and i8 %40, 7
  %51 = zext nneg i8 %50 to i32
  %52 = getelementptr inbounds nuw [2 x i8], ptr %1, i32 %51
  %53 = load i16, ptr %52, align 2, !tbaa !9
  %54 = trunc i16 %53 to i8
  %55 = add i8 %54, %47
  %56 = select i1 %46, i32 2, i32 0
  %57 = getelementptr inbounds nuw i8, ptr %1, i32 %56
  %58 = load i16, ptr %57, align 2, !tbaa !9
  %59 = icmp eq i16 %58, %29
  %60 = select i1 %59, i8 %55, i8 0
  %61 = xor i1 %46, true
  %62 = zext i1 %61 to i8
  %63 = or i8 %60, %62
  %64 = load i8, ptr @buf, align 1, !tbaa !8
  br label %65

65:                                               ; preds = %15, %65
  %66 = phi i8 [ 8, %15 ], [ %69, %65 ]
  %67 = phi ptr [ %1, %15 ], [ %70, %65 ]
  %68 = phi i8 [ %63, %15 ], [ %75, %65 ]
  %69 = add nsw i8 %66, -1
  %70 = getelementptr inbounds nuw i8, ptr %67, i32 2
  %71 = load i16, ptr %67, align 2, !tbaa !9
  %72 = zext i8 %68 to i16
  %73 = icmp eq i16 %71, %72
  %74 = zext i1 %73 to i8
  %75 = add i8 %68, %74
  %76 = icmp eq i8 %69, 0
  br i1 %76, label %77, label %65, !llvm.loop !14

77:                                               ; preds = %65
  %78 = zext i8 %4 to i16
  %79 = add nuw nsw i16 %78, 624
  %80 = zext i8 %19 to i16
  %81 = add nuw nsw i16 %80, -2666
  %82 = add i8 %20, -3
  %83 = select i1 %38, i8 %40, i8 0
  %84 = add i8 %82, %83
  %85 = tail call i16 @llvm.fshl.i16(i16 %79, i16 512, i16 1)
  %86 = tail call i16 @llvm.fshl.i16(i16 %85, i16 1024, i16 1)
  %87 = or i8 %84, 1
  %88 = udiv i8 %49, %87
  %89 = zext i8 %88 to i16
  %90 = xor i16 %86, %89
  %91 = tail call i16 @llvm.fshl.i16(i16 %90, i16 2048, i16 1)
  %92 = xor i16 %91, -79
  %93 = tail call i16 @llvm.fshl.i16(i16 %92, i16 %92, i16 1)
  %94 = zext i1 %46 to i16
  %95 = xor i16 %93, %94
  %96 = icmp ult i8 %64, %84
  %97 = zext i8 %63 to i32
  %98 = select i1 %96, i32 %97, i32 %48
  %99 = lshr i16 %81, 5
  %100 = and i16 %99, 7
  %101 = zext nneg i16 %100 to i32
  %102 = icmp ne i32 %98, %101
  %103 = icmp eq i8 %63, 0
  %104 = select i1 %102, i1 true, i1 %103
  %105 = zext i1 %104 to i32
  %106 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 9), align 1, !tbaa !8
  %107 = icmp ugt i8 %106, -126
  %108 = zext i1 %107 to i8
  %109 = or i8 %75, %108
  %110 = zext i16 %95 to i32
  %111 = shl nuw nsw i32 %110, 1
  %112 = lshr i32 %110, 15
  %113 = or disjoint i32 %111, %112
  %114 = zext i8 %84 to i32
  %115 = zext i8 %109 to i32
  %116 = sub nsw i32 %114, %115
  %117 = xor i32 %116, %113
  %118 = and i32 %117, 65535
  %119 = shl nuw nsw i32 %118, 1
  %120 = lshr i32 %118, 15
  %121 = and i32 %119, 65534
  %122 = or disjoint i32 %121, %120
  %123 = xor i32 %122, %105
  %124 = shl nuw nsw i32 %123, 1
  %125 = lshr i32 %121, 15
  %126 = or disjoint i32 %124, %125
  %127 = xor i32 %126, %115
  br label %128

128:                                              ; preds = %77, %128
  %129 = phi i32 [ 0, %77 ], [ %139, %128 ]
  %130 = phi i32 [ %127, %77 ], [ %138, %128 ]
  %131 = and i32 %130, 65535
  %132 = shl nuw nsw i32 %131, 1
  %133 = lshr i32 %131, 15
  %134 = or disjoint i32 %132, %133
  %135 = getelementptr inbounds nuw [2 x i8], ptr %1, i32 %129
  %136 = load i16, ptr %135, align 2, !tbaa !9
  %137 = zext i16 %136 to i32
  %138 = xor i32 %134, %137
  %139 = add nuw nsw i32 %129, 1
  %140 = icmp eq i32 %139, 8
  br i1 %140, label %141, label %128, !llvm.loop !15

141:                                              ; preds = %128
  %142 = select i1 %38, i16 8, i16 0
  %143 = xor i16 %142, %81
  %144 = zext i1 %104 to i16
  %145 = add i16 %29, %144
  %146 = zext i16 %145 to i32
  %147 = add nuw nsw i32 %115, %105
  %148 = icmp eq i32 %147, %146
  %149 = zext i8 %109 to i16
  br i1 %148, label %150, label %154

150:                                              ; preds = %141
  %151 = and i16 %143, 255
  %152 = icmp samesign ult i16 %151, %149
  %153 = zext i1 %152 to i16
  br label %164

154:                                              ; preds = %141
  %155 = and i16 %29, 255
  %156 = icmp samesign ule i16 %155, %149
  %157 = zext i1 %156 to i32
  %158 = getelementptr inbounds nuw i8, ptr @buf, i32 %157
  %159 = load i8, ptr %158, align 1, !tbaa !8
  %160 = zext i8 %159 to i16
  %161 = load i8, ptr @buf, align 1, !tbaa !8
  %162 = zext i8 %161 to i16
  %163 = sub nsw i16 %160, %162
  br label %164

164:                                              ; preds = %154, %150
  %165 = phi i16 [ %153, %150 ], [ %163, %154 ]
  %166 = and i32 %138, 65535
  %167 = shl nuw nsw i32 %166, 1
  %168 = lshr i32 %166, 15
  %169 = or disjoint i32 %167, %168
  %170 = xor i32 %169, %43
  %171 = trunc i32 %170 to i16
  %172 = zext i8 %84 to i16
  %173 = xor i16 %172, %171
  %174 = xor i16 %173, %165
  %175 = xor i16 %174, %143
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i16 %175
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

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
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !6, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
!15 = distinct !{!15, !12, !13}
