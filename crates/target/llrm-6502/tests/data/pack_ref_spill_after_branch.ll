; ModuleID = 'sso.ll'
source_filename = "tests/run/c/spill_store_order.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8"

; Function Attrs: nounwind
declare dso_local i16 @gd_read(i16 noundef, i16 noundef) #0

; Function Attrs: nounwind
define dso_local void @pack_ref(i16 noundef %0, i16 noundef %1, i16 noundef %2, i16 noundef %3, i16 noundef %4, ptr noundef %5) #0 {
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  %12 = alloca ptr, align 2
  %13 = alloca i16, align 2
  %14 = alloca i16, align 2
  %15 = alloca i16, align 2
  %16 = alloca i16, align 2
  %17 = alloca i16, align 2
  %18 = alloca i16, align 2
  %19 = alloca i16, align 2
  %20 = alloca i16, align 2
  %21 = alloca i16, align 2
  store i16 %0, ptr %7, align 2, !tbaa !7
  store i16 %1, ptr %8, align 2, !tbaa !7
  store i16 %2, ptr %9, align 2, !tbaa !7
  store i16 %3, ptr %10, align 2, !tbaa !7
  store i16 %4, ptr %11, align 2, !tbaa !7
  store ptr %5, ptr %12, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %13) #2
  %22 = load i16, ptr %7, align 2, !tbaa !7
  %23 = icmp eq i16 %22, 16
  br i1 %23, label %27, label %24

24:                                               ; preds = %6
  %25 = load i16, ptr %7, align 2, !tbaa !7
  %26 = icmp eq i16 %25, 13
  br label %27

27:                                               ; preds = %24, %6
  %28 = phi i1 [ true, %6 ], [ %26, %24 ]
  %29 = zext i1 %28 to i64
  %30 = select i1 %28, i16 4, i16 1
  store i16 %30, ptr %13, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %14) #2
  %31 = load i16, ptr %7, align 2, !tbaa !7
  %32 = icmp eq i16 %31, 19
  br i1 %32, label %33, label %34

33:                                               ; preds = %27
  br label %39

34:                                               ; preds = %27
  %35 = load i16, ptr %7, align 2, !tbaa !7
  %36 = icmp eq i16 %35, 4
  %37 = zext i1 %36 to i64
  %38 = select i1 %36, i16 2, i16 1
  br label %39

39:                                               ; preds = %34, %33
  %40 = phi i16 [ 8, %33 ], [ %38, %34 ]
  store i16 %40, ptr %14, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %15) #2
  %41 = load i16, ptr %10, align 2, !tbaa !7
  %42 = load i16, ptr %14, align 2, !tbaa !7
  %43 = mul i16 %41, %42
  %44 = add i16 %43, 7
  %45 = udiv i16 %44, 8
  store i16 %45, ptr %15, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %16) #2
  call void @llvm.lifetime.start.p0(ptr %17) #2
  call void @llvm.lifetime.start.p0(ptr %18) #2
  store i16 0, ptr %17, align 2, !tbaa !7
  br label %46

46:                                               ; preds = %137, %39
  %47 = load i16, ptr %17, align 2, !tbaa !7
  %48 = load i16, ptr %11, align 2, !tbaa !7
  %49 = icmp ult i16 %47, %48
  br i1 %49, label %50, label %145

50:                                               ; preds = %46
  store i16 0, ptr %18, align 2, !tbaa !7
  br label %51

51:                                               ; preds = %61, %50
  %52 = load i16, ptr %18, align 2, !tbaa !7
  %53 = load i16, ptr %13, align 2, !tbaa !7
  %54 = load i16, ptr %15, align 2, !tbaa !7
  %55 = mul i16 %53, %54
  %56 = icmp ult i16 %52, %55
  br i1 %56, label %57, label %64

57:                                               ; preds = %51
  %58 = load ptr, ptr %12, align 2, !tbaa !8
  %59 = load i16, ptr %18, align 2, !tbaa !7
  %60 = getelementptr inbounds nuw i8, ptr %58, i16 %59
  store i8 0, ptr %60, align 1, !tbaa !11
  br label %61

61:                                               ; preds = %57
  %62 = load i16, ptr %18, align 2, !tbaa !7
  %63 = add i16 %62, 1
  store i16 %63, ptr %18, align 2, !tbaa !7
  br label %51, !llvm.loop !12

64:                                               ; preds = %51
  store i16 0, ptr %16, align 2, !tbaa !7
  br label %65

65:                                               ; preds = %133, %64
  %66 = load i16, ptr %16, align 2, !tbaa !7
  %67 = load i16, ptr %10, align 2, !tbaa !7
  %68 = icmp ult i16 %66, %67
  br i1 %68, label %69, label %136

69:                                               ; preds = %65
  call void @llvm.lifetime.start.p0(ptr %19) #2
  %70 = load i16, ptr %8, align 2, !tbaa !7
  %71 = load i16, ptr %16, align 2, !tbaa !7
  %72 = add i16 %70, %71
  %73 = load i16, ptr %9, align 2, !tbaa !7
  %74 = load i16, ptr %17, align 2, !tbaa !7
  %75 = add i16 %73, %74
  %76 = call i16 @gd_read(i16 noundef %72, i16 noundef %75)
  store i16 %76, ptr %19, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %20) #2
  call void @llvm.lifetime.start.p0(ptr %21) #2
  %77 = load i16, ptr %16, align 2, !tbaa !7
  %78 = load i16, ptr %14, align 2, !tbaa !7
  %79 = mul i16 %77, %78
  store i16 %79, ptr %21, align 2, !tbaa !7
  %80 = load i16, ptr %13, align 2, !tbaa !7
  %81 = icmp ugt i16 %80, 1
  br i1 %81, label %82, label %114

82:                                               ; preds = %69
  store i16 0, ptr %20, align 2, !tbaa !7
  br label %83

83:                                               ; preds = %110, %82
  %84 = load i16, ptr %20, align 2, !tbaa !7
  %85 = load i16, ptr %13, align 2, !tbaa !7
  %86 = icmp ult i16 %84, %85
  br i1 %86, label %87, label %113

87:                                               ; preds = %83
  %88 = load i16, ptr %19, align 2, !tbaa !7
  %89 = load i16, ptr %20, align 2, !tbaa !7
  %90 = lshr i16 %88, %89
  %91 = and i16 %90, 1
  %92 = icmp ne i16 %91, 0
  br i1 %92, label %93, label %109

93:                                               ; preds = %87
  %94 = load i16, ptr %16, align 2, !tbaa !7
  %95 = and i16 %94, 7
  %96 = ashr i16 128, %95
  %97 = load ptr, ptr %12, align 2, !tbaa !8
  %98 = load i16, ptr %20, align 2, !tbaa !7
  %99 = load i16, ptr %15, align 2, !tbaa !7
  %100 = mul i16 %98, %99
  %101 = load i16, ptr %16, align 2, !tbaa !7
  %102 = lshr i16 %101, 3
  %103 = add i16 %100, %102
  %104 = getelementptr inbounds nuw i8, ptr %97, i16 %103
  %105 = load i8, ptr %104, align 1, !tbaa !11
  %106 = zext i8 %105 to i16
  %107 = or i16 %106, %96
  %108 = trunc i16 %107 to i8
  store i8 %108, ptr %104, align 1, !tbaa !11
  br label %109

109:                                              ; preds = %93, %87
  br label %110

110:                                              ; preds = %109
  %111 = load i16, ptr %20, align 2, !tbaa !7
  %112 = add i16 %111, 1
  store i16 %112, ptr %20, align 2, !tbaa !7
  br label %83, !llvm.loop !15

113:                                              ; preds = %83
  br label %132

114:                                              ; preds = %69
  %115 = load i16, ptr %19, align 2, !tbaa !7
  %116 = load i16, ptr %14, align 2, !tbaa !7
  %117 = sub i16 8, %116
  %118 = load i16, ptr %21, align 2, !tbaa !7
  %119 = and i16 %118, 7
  %120 = sub i16 %117, %119
  %121 = shl i16 %115, %120
  %122 = trunc i16 %121 to i8
  %123 = zext i8 %122 to i16
  %124 = load ptr, ptr %12, align 2, !tbaa !8
  %125 = load i16, ptr %21, align 2, !tbaa !7
  %126 = lshr i16 %125, 3
  %127 = getelementptr inbounds nuw i8, ptr %124, i16 %126
  %128 = load i8, ptr %127, align 1, !tbaa !11
  %129 = zext i8 %128 to i16
  %130 = or i16 %129, %123
  %131 = trunc i16 %130 to i8
  store i8 %131, ptr %127, align 1, !tbaa !11
  br label %132

132:                                              ; preds = %114, %113
  call void @llvm.lifetime.end.p0(ptr %21) #2
  call void @llvm.lifetime.end.p0(ptr %20) #2
  call void @llvm.lifetime.end.p0(ptr %19) #2
  br label %133

133:                                              ; preds = %132
  %134 = load i16, ptr %16, align 2, !tbaa !7
  %135 = add i16 %134, 1
  store i16 %135, ptr %16, align 2, !tbaa !7
  br label %65, !llvm.loop !16

136:                                              ; preds = %65
  br label %137

137:                                              ; preds = %136
  %138 = load i16, ptr %17, align 2, !tbaa !7
  %139 = add i16 %138, 1
  store i16 %139, ptr %17, align 2, !tbaa !7
  %140 = load i16, ptr %13, align 2, !tbaa !7
  %141 = load i16, ptr %15, align 2, !tbaa !7
  %142 = mul i16 %140, %141
  %143 = load ptr, ptr %12, align 2, !tbaa !8
  %144 = getelementptr inbounds nuw i8, ptr %143, i16 %142
  store ptr %144, ptr %12, align 2, !tbaa !8
  br label %46, !llvm.loop !17

145:                                              ; preds = %46
  call void @llvm.lifetime.end.p0(ptr %18) #2
  call void @llvm.lifetime.end.p0(ptr %17) #2
  call void @llvm.lifetime.end.p0(ptr %16) #2
  call void @llvm.lifetime.end.p0(ptr %15) #2
  call void @llvm.lifetime.end.p0(ptr %14) #2
  call void @llvm.lifetime.end.p0(ptr %13) #2
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"p1 omnipotent char", !10, i64 0}
!10 = !{!"any pointer", !5, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13, !14}
!13 = !{!"llvm.loop.mustprogress"}
!14 = !{!"llvm.loop.unroll.disable"}
!15 = distinct !{!15, !13, !14}
!16 = distinct !{!16, !13, !14}
!17 = distinct !{!17, !13, !14}
