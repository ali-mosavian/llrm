target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%struct.S = type { i8, i16, i8 }

@zero = external constant i8, align 1
@buf = external constant [16 x i8], align 1

define zeroext i16 @test() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" nofree nosync norecurse memory(inaccessiblemem: readwrite) willreturn {
  %1 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %2 = zext i8 %1 to i16
  %3 = add nuw nsw i16 %2, 32
  %4 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %5 = add i8 %4, 53
  %6 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %7 = add i8 %6, 127
  %8 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %9 = zext i8 %8 to i16
  %10 = add nuw nsw i16 %9, 255
  %11 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %12 = add i8 %11, 127
  %13 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %14 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %15 = add i8 %14, 67
  %16 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %17 = zext i8 %16 to i16
  %18 = add nuw nsw i16 %17, -2053
  %19 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %20 = add i8 %19, -44
  %21 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %22 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %23 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %24 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 1), align 1, !tbaa !8
  %25 = zext i8 %22 to i16
  %26 = add nuw nsw i16 %25, 1
  %27 = zext i8 %23 to i16
  %28 = shl nuw nsw i16 %27, 1
  %29 = add nuw nsw i16 %28, 1234
  %30 = icmp ugt i8 %21, 1
  %31 = zext i1 %30 to i8
  %32 = icmp ult i8 %24, %31
  %33 = zext i1 %32 to i16
  %34 = or disjoint i16 %29, %33
  %35 = shl nuw nsw i16 %34, 3
  %36 = shl nuw nsw i16 %25, 2
  %37 = sub nuw nsw i16 32760, %36
  %38 = xor i16 %37, %35
  %39 = xor i16 %38, 13549
  %40 = tail call i16 @llvm.fshl.i16(i16 %39, i16 1, i16 1)
  %41 = lshr i16 %26, 8
  %42 = zext i8 %24 to i16
  %43 = shl i16 %42, 15
  %44 = xor i16 %41, %26
  %45 = or disjoint i16 %44, %43
  %46 = xor i16 %40, %45
  %47 = icmp ugt i16 %46, -5655
  br i1 %47, label %48, label %77

48:
  %49 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %50 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %51 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %52 = zext i8 %50 to i16
  %53 = add nuw nsw i16 %52, 1
  %54 = zext i8 %51 to i16
  %55 = shl nuw nsw i16 %54, 1
  %56 = add nuw nsw i16 %55, 1234
  %57 = icmp ugt i8 %49, 1
  %58 = zext i1 %57 to i8
  %59 = icmp ult i8 %24, %58
  %60 = zext i1 %59 to i16
  %61 = or disjoint i16 %56, %60
  %62 = shl nuw nsw i16 %61, 3
  %63 = shl nuw nsw i16 %52, 2
  %64 = sub nuw nsw i16 32760, %63
  %65 = xor i16 %64, %62
  %66 = xor i16 %65, 32531
  %67 = tail call i16 @llvm.fshl.i16(i16 %66, i16 3, i16 1)
  %68 = lshr i16 %53, 8
  %69 = xor i16 %68, %53
  %70 = or disjoint i16 %69, %43
  %71 = xor i16 %67, %70
  %72 = icmp eq i16 %71, 1
  br i1 %72, label %135, label %73

73:
  %74 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %75 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %76 = load volatile i8, ptr @zero, align 1, !tbaa !8
  br label %135

77:
  %78 = tail call zeroext i16 @h0() nobuiltin "no-builtins"
  %79 = zext i16 %78 to i32
  %80 = load i8, ptr getelementptr inbounds nuw (i8, ptr @buf, i32 14), align 1, !tbaa !8
  %81 = zext i8 %80 to i32
  %82 = xor i32 %81, 65535
  %83 = icmp samesign ugt i32 %82, %79
  br i1 %83, label %129, label %84

84:
  %85 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %86 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %87 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %88 = zext i8 %86 to i16
  %89 = add nuw nsw i16 %88, 1
  %90 = zext i8 %87 to i16
  %91 = shl nuw nsw i16 %90, 1
  %92 = add nuw nsw i16 %91, 1234
  %93 = icmp ugt i8 %85, 1
  %94 = zext i1 %93 to i8
  %95 = icmp ult i8 %24, %94
  %96 = zext i1 %95 to i16
  %97 = or disjoint i16 %92, %96
  %98 = shl nuw nsw i16 %97, 2
  %99 = shl nuw nsw i16 %88, 1
  %100 = sub nuw nsw i16 32764, %99
  %101 = xor i16 %98, %100
  %102 = or disjoint i16 %101, 1
  %103 = lshr i16 %3, 8
  %104 = xor i16 %103, %3
  %105 = xor i16 %104, %102
  %106 = shl nuw i16 %105, 1
  %107 = xor i16 %106, 17
  %108 = tail call i16 @llvm.fshl.i16(i16 %107, i16 %107, i16 1)
  %109 = lshr i16 %89, 8
  %110 = xor i16 %109, %89
  %111 = xor i16 %110, %108
  %112 = trunc i16 %111 to i8
  %113 = add i8 %15, %112
  %114 = shl i16 %111, 3
  %115 = and i16 %114, 2040
  %116 = xor i16 %115, %18
  %117 = and i8 %112, 1
  %118 = icmp eq i8 %117, 0
  %119 = select i1 %118, i8 0, i8 %113
  %120 = add i8 %119, %20
  %121 = zext i8 %12 to i16
  %122 = and i16 %10, %121
  %123 = add nuw nsw i16 %2, 33
  %124 = add nuw nsw i16 %123, %122
  %125 = xor i8 %12, %5
  %126 = and i8 %125, %7
  %127 = zext i8 %126 to i16
  %128 = and i16 %124, %127
  br label %135

129:
  %130 = zext i8 %7 to i16
  %131 = xor i16 %130, -1
  %132 = sub nsw i16 148, %17
  %133 = or i16 %132, %131
  %134 = xor i16 %133, %3
  br label %135

135:
  %136 = phi i16 [ %18, %48 ], [ %18, %73 ], [ %18, %129 ], [ %116, %84 ]
  %137 = phi i8 [ %20, %48 ], [ %20, %73 ], [ %20, %129 ], [ %120, %84 ]
  %138 = phi i8 [ %15, %48 ], [ %15, %73 ], [ %15, %129 ], [ %113, %84 ]
  %139 = phi i16 [ %3, %73 ], [ %3, %48 ], [ %128, %84 ], [ %134, %129 ]
  %140 = or i8 %137, 1
  %141 = zext i8 %140 to i16
  %142 = add nsw i16 %139, %141
  %143 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %144 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %145 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %146 = zext i8 %144 to i16
  %147 = add nuw nsw i16 %146, 1
  %148 = zext i8 %145 to i16
  %149 = shl nuw nsw i16 %148, 1
  %150 = add nuw nsw i16 %149, 1234
  %151 = icmp ugt i8 %143, 1
  %152 = zext i1 %151 to i8
  %153 = icmp ult i8 %24, %152
  %154 = zext i1 %153 to i16
  %155 = or disjoint i16 %150, %154
  %156 = shl nuw nsw i16 %155, 3
  %157 = shl nuw nsw i16 %146, 2
  %158 = sub nuw nsw i16 32760, %157
  %159 = xor i16 %158, %156
  %160 = xor i16 %159, 19
  %161 = tail call i16 @llvm.fshl.i16(i16 %160, i16 -32765, i16 1)
  %162 = lshr i16 %147, 8
  %163 = xor i16 %162, %147
  %164 = or disjoint i16 %163, %43
  %165 = xor i16 %161, %164
  %166 = add i16 %165, %142
  %167 = add i8 %6, 27
  %168 = icmp eq i8 %167, %4
  br i1 %168, label %169, label %175

169:
  %170 = zext i8 %7 to i16
  %171 = zext i8 %138 to i16
  %172 = or i16 %166, %171
  %173 = sub i16 %170, %172
  %174 = shl i16 %173, 10
  br label %194

175:
  %176 = and i16 %166, 15
  %177 = zext nneg i16 %176 to i32
  %178 = getelementptr inbounds nuw i8, ptr @buf, i32 %177
  %179 = load i8, ptr %178, align 1, !tbaa !8
  %180 = freeze i8 %179
  %181 = icmp sgt i8 %180, -2
  %182 = zext i1 %181 to i8
  %183 = or i8 %137, %7
  %184 = freeze i8 %183
  %185 = or i8 %184, 1
  %186 = icmp eq i8 %185, %182
  %187 = select i1 %186, i8 %185, i8 0
  %188 = sub nuw i8 %182, %187
  %189 = sub i8 -127, %6
  %190 = xor i8 %188, %189
  %191 = zext i8 %7 to i16
  %192 = zext i8 %190 to i16
  %193 = zext i8 %138 to i16
  br label %194

194:
  %195 = phi i16 [ %193, %175 ], [ %171, %169 ]
  %196 = phi i16 [ %192, %175 ], [ %170, %169 ]
  %197 = phi i16 [ %191, %175 ], [ %170, %169 ]
  %198 = phi i16 [ %174, %169 ], [ %166, %175 ]
  %199 = zext i8 %13 to i16
  %200 = shl nuw nsw i16 %199, 1
  %201 = add nuw nsw i16 %200, 1456
  %202 = xor i16 %142, %136
  %203 = and i16 %202, 255
  %204 = icmp ne i16 %203, 0
  %205 = zext i1 %204 to i16
  %206 = or disjoint i16 %201, %205
  %207 = shl nuw nsw i16 %206, 1
  %208 = xor i16 %166, %207
  %209 = zext i8 %5 to i16
  %210 = tail call i16 @llvm.fshl.i16(i16 %208, i16 %208, i16 1)
  %211 = tail call zeroext i16 @h0() nobuiltin "no-builtins"
  %212 = xor i16 %211, %210
  %213 = tail call i16 @llvm.fshl.i16(i16 %212, i16 %212, i16 1)
  %214 = lshr i16 %198, 8
  %215 = xor i16 %214, %198
  %216 = xor i16 %215, %213
  %217 = tail call i16 @llvm.fshl.i16(i16 %216, i16 %216, i16 1)
  %218 = tail call zeroext i16 @h0() nobuiltin "no-builtins"
  %219 = xor i16 %217, %209
  %220 = shl i16 %219, 1
  %221 = lshr i16 %217, 15
  %222 = or disjoint i16 %220, %221
  %223 = xor i16 %222, %196
  %224 = shl i16 %223, 1
  %225 = lshr i16 %220, 15
  %226 = or disjoint i16 %224, %225
  %227 = xor i16 %226, %197
  %228 = shl i16 %227, 1
  %229 = lshr i16 %224, 15
  %230 = or disjoint i16 %228, %229
  %231 = trunc i16 %139 to i8
  %232 = xor i8 %12, %231
  %233 = zext i8 %232 to i16
  %234 = xor i16 %230, %233
  %235 = tail call i16 @llvm.fshl.i16(i16 %234, i16 %228, i16 1)
  %236 = or i16 %218, %197
  %237 = zext i8 %137 to i16
  %238 = xor i16 %136, %237
  %239 = xor i16 %238, %195
  %240 = xor i16 %239, %236
  %241 = xor i16 %240, %235
  ret i16 %241
}

declare void @llvm.lifetime.start.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal void @bump(ptr noundef nonnull nocapture nofree "llrm.reads"="0:1,2:5" "llrm.writes"="0:1,2:5" %0, i8 noundef zeroext %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" willreturn nofree nosync norecurse memory(argmem: readwrite) {
  unreachable
}

define internal zeroext i16 @h1(i16 noundef zeroext range(i16 0, -32768) %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" memory(inaccessiblemem: readwrite) willreturn nofree nosync norecurse {
  unreachable
}

define internal zeroext i16 @h0() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" nofree nosync norecurse memory(inaccessiblemem: readwrite) willreturn {
  %1 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %2 = zext i8 %1 to i32
  %3 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %4 = add i8 %3, -1
  %5 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %6 = add i8 %5, 1
  %7 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %8 = add i8 %7, -1
  %9 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %10 = zext i8 %9 to i32
  %11 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %12 = zext i8 %11 to i32
  %13 = add nuw nsw i32 %12, 6
  %14 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %15 = zext i8 %14 to i32
  %16 = add nuw nsw i32 %15, 11
  %17 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %18 = zext i8 %17 to i32
  %19 = add nuw nsw i32 %18, 16
  %20 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %21 = zext i8 %20 to i32
  %22 = add nuw nsw i32 %21, 21
  %23 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %24 = zext i8 %23 to i32
  %25 = add nuw nsw i32 %24, 26
  %26 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %27 = zext i8 %26 to i32
  %28 = add nuw nsw i32 %27, 31
  %29 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %30 = zext i8 %29 to i32
  %31 = add nuw nsw i32 %30, 36
  %32 = load volatile i8, ptr @zero, align 1, !tbaa !8
  %33 = zext i8 %32 to i32
  %34 = add nuw nsw i32 %33, 41
  %35 = trunc i32 %13 to i8
  %36 = or i8 %8, %35
  %37 = trunc i32 %16 to i8
  %38 = or i8 %36, %37
  %39 = trunc i32 %19 to i8
  %40 = or i8 %38, %39
  %41 = trunc i32 %22 to i8
  %42 = or i8 %40, %41
  %43 = trunc i32 %25 to i8
  %44 = or i8 %42, %43
  %45 = trunc i32 %28 to i8
  %46 = or i8 %44, %45
  %47 = trunc i32 %31 to i8
  %48 = or i8 %46, %47
  %49 = trunc i32 %34 to i8
  %50 = or i8 %48, %49
  %51 = add nuw nsw i32 %2, -2147483641
  %52 = shl nuw nsw i32 %10, 1
  %53 = add nuw nsw i32 %52, 920
  %54 = xor i32 %51, %53
  %55 = lshr i32 %51, 8
  %56 = xor i32 %54, %55
  %57 = shl i32 %56, 1
  %58 = and i32 %57, 4094
  %59 = zext i8 %4 to i32
  %60 = xor i32 %58, %59
  %61 = zext i8 %6 to i32
  %62 = shl nuw nsw i32 %60, 2
  %63 = shl nuw nsw i32 %61, 1
  %64 = xor i32 %62, %63
  %65 = zext i8 %50 to i32
  %66 = xor i32 %64, %65
  %67 = shl nuw nsw i32 %66, 2
  %68 = shl nuw nsw i32 %13, 1
  %69 = xor i32 %67, %68
  %70 = xor i32 %16, %69
  %71 = shl nuw nsw i32 %70, 1
  %72 = lshr i32 %58, 11
  %73 = and i32 %71, 65534
  %74 = or disjoint i32 %73, %72
  %75 = xor i32 %19, %74
  %76 = shl nuw nsw i32 %75, 1
  %77 = lshr i32 %73, 15
  %78 = and i32 %76, 65534
  %79 = or disjoint i32 %78, %77
  %80 = xor i32 %22, %79
  %81 = shl nuw nsw i32 %80, 1
  %82 = lshr i32 %78, 15
  %83 = and i32 %81, 65534
  %84 = or disjoint i32 %83, %82
  %85 = xor i32 %25, %84
  %86 = shl nuw nsw i32 %85, 1
  %87 = lshr i32 %83, 15
  %88 = and i32 %86, 65534
  %89 = or disjoint i32 %88, %87
  %90 = xor i32 %28, %89
  %91 = shl nuw nsw i32 %90, 1
  %92 = lshr i32 %88, 15
  %93 = and i32 %91, 65534
  %94 = or disjoint i32 %93, %92
  %95 = xor i32 %31, %94
  %96 = shl nuw nsw i32 %95, 1
  %97 = lshr i32 %93, 15
  %98 = or disjoint i32 %96, %97
  %99 = xor i32 %34, %98
  %100 = xor i32 %99, %59
  %101 = trunc i32 %100 to i16
  ret i16 %101
}

declare void @llvm.lifetime.end.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define zeroext i8 @k() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" nofree nosync norecurse memory(inaccessiblemem: readwrite) willreturn {
  %1 = tail call zeroext i16 @test() nobuiltin "no-builtins"
  %2 = lshr i16 %1, 8
  %3 = xor i16 %2, %1
  %4 = trunc i16 %3 to i8
  ret i8 %4
}

declare i16 @llvm.fshl.i16(i16, i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

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
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !6, i64 0}
!13 = !{!14, !6, i64 0}
!14 = !{!"S", !6, i64 0, !10, i64 2, !6, i64 4}
!15 = !{!14, !10, i64 2}
!16 = !{!14, !6, i64 4}
!17 = !{!18, !18, i64 0}
!18 = !{!"p1 _ZTS1S", !19, i64 0}
!19 = !{!"any pointer", !6, i64 0}
!20 = distinct !{!20, !21}
!21 = !{!"llvm.loop.mustprogress"}
!22 = !{!23, !23, i64 0}
!23 = !{!"p1 long", !19, i64 0}
!24 = distinct !{!24, !21}
!25 = distinct !{!25, !21}
!26 = distinct !{!26, !21}
