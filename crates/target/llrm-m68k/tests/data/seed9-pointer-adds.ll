; ModuleID = 'p.c'
source_filename = "p.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@zero = external constant i8, align 1
@buf = external local_unnamed_addr constant [16 x i8], align 1

; Function Attrs: nofree norecurse nosync nounwind memory(inaccessiblemem: readwrite)
define dso_local zeroext i16 @test() local_unnamed_addr #0 {
  %1 = alloca [16 x i32], align 2
  %2 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %3 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %4 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %5 = load volatile i8, ptr @zero, align 1, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #4
  %6 = load volatile i8, ptr @zero, align 1, !tbaa !8
  br label %7

7:                                                ; preds = %0, %7
  %8 = phi i32 [ 0, %0 ], [ %15, %7 ]
  %9 = mul nuw nsw i32 %8, 7
  %10 = add nuw nsw i32 %9, 5
  %11 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %12 = zext i8 %11 to i32
  %13 = add nuw nsw i32 %10, %12
  %14 = getelementptr inbounds nuw [4 x i8], ptr %1, i32 %8
  store i32 %13, ptr %14, align 2, !tbaa !9
  %15 = add nuw nsw i32 %8, 1
  %16 = icmp eq i32 %15, 16
  br i1 %16, label %17, label %7, !llvm.loop !11

17:                                               ; preds = %7
  %18 = zext i8 %6 to i16
  %19 = add nuw nsw i16 %18, 728
  %20 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %21 = add i8 %20, 59
  %22 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %23 = zext i8 %22 to i16
  %24 = add nuw nsw i16 %23, -13873
  %25 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %26 = add i8 %25, -100
  %27 = zext i8 %4 to i16
  %28 = icmp ugt i8 %26, %4
  br i1 %28, label %57, label %29

29:                                               ; preds = %17
  %30 = zext i8 %3 to i32
  %31 = or disjoint i32 %30, 16384
  %32 = zext i8 %2 to i32
  %33 = add nuw i32 %32, 2147483647
  %34 = getelementptr inbounds nuw i8, ptr %1, i32 44
  %35 = load i32, ptr %34, align 2, !tbaa !9
  %36 = mul i32 %35, %31
  %37 = add i32 %36, 47476
  %38 = xor i32 %37, %31
  %39 = getelementptr inbounds nuw i8, ptr %1, i32 4
  %40 = load i32, ptr %39, align 2, !tbaa !9
  %41 = xor i32 %40, %33
  %42 = and i32 %41, 65535
  %43 = icmp ne i32 %42, 0
  %44 = sext i1 %43 to i32
  %45 = xor i32 %33, %44
  %46 = and i32 %45, 255
  %47 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 15), align 1, !tbaa !8
  %48 = zext i8 %47 to i32
  %49 = icmp samesign uge i32 %46, %48
  %50 = zext i1 %49 to i32
  %51 = getelementptr inbounds nuw i8, ptr %1, i32 60
  %52 = load i32, ptr %51, align 2, !tbaa !9
  %53 = and i32 %52, 15
  %54 = getelementptr inbounds nuw [4 x i8], ptr %1, i32 %53
  %55 = load i32, ptr %54, align 2, !tbaa !9
  %56 = icmp eq i32 %55, %50
  br i1 %56, label %76, label %57

57:                                               ; preds = %17, %29
  %58 = getelementptr inbounds nuw i8, ptr %1, i32 12
  %59 = call fastcc i32 @cpl(ptr noundef %1, ptr noundef %58)
  %60 = shl nuw nsw i16 %19, 1
  %61 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 11), align 1, !tbaa !8
  %62 = and i8 %61, 14
  %63 = zext nneg i8 %62 to i32
  %64 = getelementptr inbounds nuw i8, ptr @buf, i32 %63
  %65 = getelementptr inbounds nuw i8, ptr %64, i32 1
  %66 = load i8, ptr %65, align 1, !tbaa !8
  %67 = and i8 %66, 14
  %68 = zext nneg i8 %67 to i32
  %69 = getelementptr inbounds nuw i8, ptr @buf, i32 %68
  %70 = getelementptr inbounds nuw i8, ptr %69, i32 1
  %71 = load i8, ptr %70, align 1, !tbaa !8
  %72 = or i8 %71, %4
  %73 = zext i8 %72 to i16
  %74 = xor i16 %60, %73
  %75 = call fastcc i32 @cpl(ptr noundef %1, ptr noundef %58)
  br label %76

76:                                               ; preds = %57, %29
  %77 = phi i32 [ %59, %57 ], [ %38, %29 ]
  %78 = phi i32 [ %75, %57 ], [ %45, %29 ]
  %79 = phi i16 [ %74, %57 ], [ %19, %29 ]
  %80 = and i32 %78, 15
  %81 = getelementptr inbounds nuw i8, ptr @buf, i32 %80
  %82 = load i8, ptr %81, align 1, !tbaa !8
  %83 = icmp eq i8 %82, 0
  %84 = zext i1 %83 to i32
  %85 = trunc i16 %24 to i8
  %86 = and i8 %4, %85
  %87 = sext i8 %86 to i32
  %88 = icmp sgt i32 %84, %87
  %89 = zext i1 %88 to i8
  %90 = add i8 %21, %89
  %91 = select i1 %88, i8 %90, i8 0
  %92 = add i8 %91, %26
  %93 = getelementptr inbounds nuw i8, ptr %1, i32 12
  %94 = or i8 %92, 94
  %95 = lshr i8 %92, 7
  %96 = zext nneg i8 %95 to i32
  %97 = getelementptr inbounds nuw i8, ptr @buf, i32 %96
  %98 = getelementptr inbounds nuw i8, ptr %1, i32 56
  %99 = icmp ult i8 %90, %4
  br label %100

100:                                              ; preds = %76, %100
  %101 = phi i1 [ false, %76 ], [ true, %100 ]
  %102 = phi i32 [ 1, %76 ], [ 0, %100 ]
  %103 = call fastcc i32 @cpl(ptr noundef %1, ptr noundef %93)
  br i1 %101, label %104, label %100, !llvm.loop !14

104:                                              ; preds = %100
  %105 = load i8, ptr %97, align 1, !tbaa !8
  %106 = icmp ugt i8 %94, %105
  %107 = load i32, ptr %98, align 2
  %108 = shl i32 %107, 13
  %109 = select i1 %99, i32 0, i32 %108
  %110 = select i1 %106, i32 %109, i32 %102
  %111 = trunc i32 %103 to i16
  br label %112

112:                                              ; preds = %104, %112
  %113 = phi i8 [ 16, %104 ], [ %116, %112 ]
  %114 = phi ptr [ %1, %104 ], [ %117, %112 ]
  %115 = phi i16 [ %111, %104 ], [ %122, %112 ]
  %116 = add nsw i8 %113, -1
  %117 = getelementptr inbounds nuw i8, ptr %114, i32 4
  %118 = load i32, ptr %114, align 2, !tbaa !9
  %119 = zext i16 %115 to i32
  %120 = icmp uge i32 %118, %119
  %121 = zext i1 %120 to i16
  %122 = add i16 %115, %121
  %123 = icmp eq i8 %116, 0
  br i1 %123, label %124, label %112, !llvm.loop !15

124:                                              ; preds = %112
  %125 = add i32 %78, %77
  %126 = zext nneg i16 %79 to i32
  %127 = shl nuw nsw i32 %126, 1
  %128 = and i32 %125, 65535
  %129 = xor i32 %127, %128
  %130 = shl nuw nsw i32 %129, 1
  %131 = lshr i32 %129, 15
  %132 = and i32 %130, 65534
  %133 = or disjoint i32 %132, %131
  %134 = shl nuw nsw i32 %133, 1
  %135 = lshr i32 %132, 15
  %136 = or disjoint i32 %134, %135
  %137 = trunc i32 %136 to i16
  %138 = xor i16 %122, %137
  %139 = xor i16 %138, 2
  store i32 %110, ptr %1, align 2, !tbaa !9
  %140 = zext i16 %139 to i32
  %141 = shl nuw nsw i32 %140, 1
  %142 = lshr i32 %140, 15
  %143 = or disjoint i32 %141, %142
  %144 = getelementptr inbounds nuw i8, ptr %1, i32 28
  %145 = load i32, ptr %144, align 2, !tbaa !9
  %146 = icmp eq i32 %145, 0
  %147 = select i1 %146, i8 %4, i8 %90
  %148 = and i8 %147, 15
  %149 = zext nneg i8 %148 to i32
  %150 = getelementptr inbounds nuw [4 x i8], ptr %1, i32 %149
  %151 = load i32, ptr %150, align 2, !tbaa !9
  %152 = xor i32 %151, %143
  %153 = and i32 %152, 65535
  %154 = xor i32 %153, 65535
  %155 = shl nuw nsw i32 %154, 1
  %156 = lshr i32 %154, 15
  %157 = or disjoint i32 %155, %156
  %158 = lshr i32 %110, 8
  %159 = xor i32 %158, %157
  %160 = xor i32 %159, %110
  %161 = and i32 %160, 65535
  %162 = shl nuw nsw i32 %161, 1
  %163 = lshr i32 %161, 15
  %164 = or disjoint i32 %162, %163
  %165 = lshr i16 %122, 8
  %166 = trunc i32 %164 to i16
  %167 = xor i16 %165, %166
  %168 = xor i16 %167, %122
  %169 = call i16 @llvm.fshl.i16(i16 %168, i16 %168, i16 1)
  %170 = xor i16 %169, %27
  %171 = call i16 @llvm.fshl.i16(i16 %170, i16 %170, i16 1)
  br label %172

172:                                              ; preds = %124, %172
  %173 = phi i32 [ 0, %124 ], [ %183, %172 ]
  %174 = phi i16 [ %171, %124 ], [ %182, %172 ]
  %175 = zext i16 %174 to i32
  %176 = shl nuw nsw i32 %175, 1
  %177 = lshr i32 %175, 15
  %178 = or disjoint i32 %176, %177
  %179 = getelementptr inbounds nuw [4 x i8], ptr %1, i32 %173
  %180 = load i32, ptr %179, align 2, !tbaa !9
  %181 = xor i32 %180, %178
  %182 = trunc i32 %181 to i16
  %183 = add nuw nsw i32 %173, 1
  %184 = icmp eq i32 %183, 16
  br i1 %184, label %185, label %172, !llvm.loop !16

185:                                              ; preds = %172
  %186 = select i1 %88, i16 8, i16 0
  %187 = call i16 @llvm.fshl.i16(i16 %182, i16 %182, i16 1)
  %188 = zext i8 %90 to i16
  %189 = zext i8 %92 to i16
  %190 = xor i16 %186, %188
  %191 = xor i16 %190, %189
  %192 = xor i16 %191, %187
  %193 = xor i16 %192, %24
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #4
  ret i16 %193
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nofree noinline norecurse nosync nounwind memory(argmem: readwrite, inaccessiblemem: readwrite)
define internal fastcc i32 @cpl(ptr nofree noundef nonnull captures(address) initializes((0, 12)) %0, ptr nofree noundef nonnull readonly captures(none) %1) unnamed_addr #2 {
  tail call void @llvm.memcpy.p0.p0.i32(ptr noundef nonnull align 2 dereferenceable(12) %0, ptr noundef nonnull align 2 dereferenceable(12) %1, i32 12, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %0, i32 8
  %4 = load volatile i32, ptr %3, align 2, !tbaa !9
  %5 = getelementptr inbounds nuw i8, ptr %0, i32 4
  %6 = load volatile i32, ptr %5, align 2, !tbaa !9
  %7 = add i32 %6, %4
  ret i32 %7
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

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i32(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i32, i1 immarg) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.fshl.i16(i16, i16, i16) #3

attributes #0 = { nofree norecurse nosync nounwind memory(inaccessiblemem: readwrite) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nofree noinline norecurse nosync nounwind memory(argmem: readwrite, inaccessiblemem: readwrite) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #3 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #4 = { nounwind }

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
!10 = !{!"long", !6, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
!15 = distinct !{!15, !12, !13}
!16 = distinct !{!16, !12, !13}
