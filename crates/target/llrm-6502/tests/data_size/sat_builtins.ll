target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@la = dso_local global [3 x i32] [i32 2000000000, i32 -2000000000, i32 70000], align 2
@ula = dso_local global [3 x i32] [i32 -294967296, i32 5, i32 70000], align 2
@qa = dso_local global [3 x i64] [i64 8070450532247928832, i64 -8070450532247928832, i64 3000000000], align 2
@uqa = dso_local global [3 x i64] [i64 -1152921504606846976, i64 5, i64 4000000000], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca ptr, align 2
  %3 = alloca ptr, align 2
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  %10 = alloca i32, align 2
  %11 = alloca i32, align 2
  %12 = alloca i64, align 2
  %13 = alloca i64, align 2
  %14 = alloca i64, align 2
  %15 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store ptr @la, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store ptr @ula, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store ptr @qa, ptr %4, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store ptr @uqa, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %6) #4
  call void @llvm.lifetime.start.p0(ptr %7) #4
  store i16 0, ptr %6, align 2, !tbaa !12
  br label %16

16:                                               ; preds = %156, %0
  %17 = load i16, ptr %6, align 2, !tbaa !12
  %18 = icmp slt i16 %17, 3
  br i1 %18, label %19, label %159

19:                                               ; preds = %16
  store i16 0, ptr %7, align 2, !tbaa !12
  br label %20

20:                                               ; preds = %152, %19
  %21 = load i16, ptr %7, align 2, !tbaa !12
  %22 = icmp slt i16 %21, 3
  br i1 %22, label %23, label %155

23:                                               ; preds = %20
  br label %24

24:                                               ; preds = %23
  call void @llvm.lifetime.start.p0(ptr %8) #4
  %25 = load ptr, ptr %2, align 2, !tbaa !7
  %26 = load i16, ptr %6, align 2, !tbaa !12
  %27 = getelementptr inbounds i32, ptr %25, i16 %26
  %28 = load volatile i32, ptr %27, align 2, !tbaa !13
  %29 = load ptr, ptr %2, align 2, !tbaa !7
  %30 = load i16, ptr %7, align 2, !tbaa !12
  %31 = getelementptr inbounds i32, ptr %29, i16 %30
  %32 = load volatile i32, ptr %31, align 2, !tbaa !13
  %33 = call i32 @llvm.sadd.sat.i32(i32 %28, i32 %32)
  store i32 %33, ptr %8, align 2, !tbaa !13
  %34 = load i32, ptr %8, align 2, !tbaa !13
  call void @report(i32 noundef %34) #5
  %35 = load i32, ptr %8, align 2, !tbaa !13
  %36 = sdiv i32 %35, 256
  call void @report(i32 noundef %36) #5
  call void @llvm.lifetime.end.p0(ptr %8) #4
  br label %37

37:                                               ; preds = %24
  br label %38

38:                                               ; preds = %37
  br label %39

39:                                               ; preds = %38
  call void @llvm.lifetime.start.p0(ptr %9) #4
  %40 = load ptr, ptr %2, align 2, !tbaa !7
  %41 = load i16, ptr %6, align 2, !tbaa !12
  %42 = getelementptr inbounds i32, ptr %40, i16 %41
  %43 = load volatile i32, ptr %42, align 2, !tbaa !13
  %44 = load ptr, ptr %2, align 2, !tbaa !7
  %45 = load i16, ptr %7, align 2, !tbaa !12
  %46 = getelementptr inbounds i32, ptr %44, i16 %45
  %47 = load volatile i32, ptr %46, align 2, !tbaa !13
  %48 = call i32 @llvm.ssub.sat.i32(i32 %43, i32 %47)
  store i32 %48, ptr %9, align 2, !tbaa !13
  %49 = load i32, ptr %9, align 2, !tbaa !13
  call void @report(i32 noundef %49) #5
  %50 = load i32, ptr %9, align 2, !tbaa !13
  %51 = sdiv i32 %50, 256
  call void @report(i32 noundef %51) #5
  call void @llvm.lifetime.end.p0(ptr %9) #4
  br label %52

52:                                               ; preds = %39
  br label %53

53:                                               ; preds = %52
  br label %54

54:                                               ; preds = %53
  call void @llvm.lifetime.start.p0(ptr %10) #4
  %55 = load ptr, ptr %3, align 2, !tbaa !7
  %56 = load i16, ptr %6, align 2, !tbaa !12
  %57 = getelementptr inbounds i32, ptr %55, i16 %56
  %58 = load volatile i32, ptr %57, align 2, !tbaa !13
  %59 = load ptr, ptr %3, align 2, !tbaa !7
  %60 = load i16, ptr %7, align 2, !tbaa !12
  %61 = getelementptr inbounds i32, ptr %59, i16 %60
  %62 = load volatile i32, ptr %61, align 2, !tbaa !13
  %63 = call i32 @llvm.uadd.sat.i32(i32 %58, i32 %62)
  store i32 %63, ptr %10, align 2, !tbaa !13
  %64 = load i32, ptr %10, align 2, !tbaa !13
  call void @report(i32 noundef %64) #5
  %65 = load i32, ptr %10, align 2, !tbaa !13
  %66 = udiv i32 %65, 256
  call void @report(i32 noundef %66) #5
  call void @llvm.lifetime.end.p0(ptr %10) #4
  br label %67

67:                                               ; preds = %54
  br label %68

68:                                               ; preds = %67
  br label %69

69:                                               ; preds = %68
  call void @llvm.lifetime.start.p0(ptr %11) #4
  %70 = load ptr, ptr %3, align 2, !tbaa !7
  %71 = load i16, ptr %6, align 2, !tbaa !12
  %72 = getelementptr inbounds i32, ptr %70, i16 %71
  %73 = load volatile i32, ptr %72, align 2, !tbaa !13
  %74 = load ptr, ptr %3, align 2, !tbaa !7
  %75 = load i16, ptr %7, align 2, !tbaa !12
  %76 = getelementptr inbounds i32, ptr %74, i16 %75
  %77 = load volatile i32, ptr %76, align 2, !tbaa !13
  %78 = call i32 @llvm.usub.sat.i32(i32 %73, i32 %77)
  store i32 %78, ptr %11, align 2, !tbaa !13
  %79 = load i32, ptr %11, align 2, !tbaa !13
  call void @report(i32 noundef %79) #5
  %80 = load i32, ptr %11, align 2, !tbaa !13
  %81 = udiv i32 %80, 256
  call void @report(i32 noundef %81) #5
  call void @llvm.lifetime.end.p0(ptr %11) #4
  br label %82

82:                                               ; preds = %69
  br label %83

83:                                               ; preds = %82
  br label %84

84:                                               ; preds = %83
  call void @llvm.lifetime.start.p0(ptr %12) #4
  %85 = load ptr, ptr %4, align 2, !tbaa !10
  %86 = load i16, ptr %6, align 2, !tbaa !12
  %87 = getelementptr inbounds i64, ptr %85, i16 %86
  %88 = load volatile i64, ptr %87, align 2, !tbaa !15
  %89 = load ptr, ptr %4, align 2, !tbaa !10
  %90 = load i16, ptr %7, align 2, !tbaa !12
  %91 = getelementptr inbounds i64, ptr %89, i16 %90
  %92 = load volatile i64, ptr %91, align 2, !tbaa !15
  %93 = call i64 @llvm.sadd.sat.i64(i64 %88, i64 %92)
  store i64 %93, ptr %12, align 2, !tbaa !15
  %94 = load i64, ptr %12, align 2, !tbaa !15
  %95 = trunc i64 %94 to i32
  call void @report(i32 noundef %95) #5
  %96 = load i64, ptr %12, align 2, !tbaa !15
  %97 = sdiv i64 %96, 256
  %98 = trunc i64 %97 to i32
  call void @report(i32 noundef %98) #5
  call void @llvm.lifetime.end.p0(ptr %12) #4
  br label %99

99:                                               ; preds = %84
  br label %100

100:                                              ; preds = %99
  br label %101

101:                                              ; preds = %100
  call void @llvm.lifetime.start.p0(ptr %13) #4
  %102 = load ptr, ptr %4, align 2, !tbaa !10
  %103 = load i16, ptr %6, align 2, !tbaa !12
  %104 = getelementptr inbounds i64, ptr %102, i16 %103
  %105 = load volatile i64, ptr %104, align 2, !tbaa !15
  %106 = load ptr, ptr %4, align 2, !tbaa !10
  %107 = load i16, ptr %7, align 2, !tbaa !12
  %108 = getelementptr inbounds i64, ptr %106, i16 %107
  %109 = load volatile i64, ptr %108, align 2, !tbaa !15
  %110 = call i64 @llvm.ssub.sat.i64(i64 %105, i64 %109)
  store i64 %110, ptr %13, align 2, !tbaa !15
  %111 = load i64, ptr %13, align 2, !tbaa !15
  %112 = trunc i64 %111 to i32
  call void @report(i32 noundef %112) #5
  %113 = load i64, ptr %13, align 2, !tbaa !15
  %114 = sdiv i64 %113, 256
  %115 = trunc i64 %114 to i32
  call void @report(i32 noundef %115) #5
  call void @llvm.lifetime.end.p0(ptr %13) #4
  br label %116

116:                                              ; preds = %101
  br label %117

117:                                              ; preds = %116
  br label %118

118:                                              ; preds = %117
  call void @llvm.lifetime.start.p0(ptr %14) #4
  %119 = load ptr, ptr %5, align 2, !tbaa !10
  %120 = load i16, ptr %6, align 2, !tbaa !12
  %121 = getelementptr inbounds i64, ptr %119, i16 %120
  %122 = load volatile i64, ptr %121, align 2, !tbaa !15
  %123 = load ptr, ptr %5, align 2, !tbaa !10
  %124 = load i16, ptr %7, align 2, !tbaa !12
  %125 = getelementptr inbounds i64, ptr %123, i16 %124
  %126 = load volatile i64, ptr %125, align 2, !tbaa !15
  %127 = call i64 @llvm.uadd.sat.i64(i64 %122, i64 %126)
  store i64 %127, ptr %14, align 2, !tbaa !15
  %128 = load i64, ptr %14, align 2, !tbaa !15
  %129 = trunc i64 %128 to i32
  call void @report(i32 noundef %129) #5
  %130 = load i64, ptr %14, align 2, !tbaa !15
  %131 = udiv i64 %130, 256
  %132 = trunc i64 %131 to i32
  call void @report(i32 noundef %132) #5
  call void @llvm.lifetime.end.p0(ptr %14) #4
  br label %133

133:                                              ; preds = %118
  br label %134

134:                                              ; preds = %133
  br label %135

135:                                              ; preds = %134
  call void @llvm.lifetime.start.p0(ptr %15) #4
  %136 = load ptr, ptr %5, align 2, !tbaa !10
  %137 = load i16, ptr %6, align 2, !tbaa !12
  %138 = getelementptr inbounds i64, ptr %136, i16 %137
  %139 = load volatile i64, ptr %138, align 2, !tbaa !15
  %140 = load ptr, ptr %5, align 2, !tbaa !10
  %141 = load i16, ptr %7, align 2, !tbaa !12
  %142 = getelementptr inbounds i64, ptr %140, i16 %141
  %143 = load volatile i64, ptr %142, align 2, !tbaa !15
  %144 = call i64 @llvm.usub.sat.i64(i64 %139, i64 %143)
  store i64 %144, ptr %15, align 2, !tbaa !15
  %145 = load i64, ptr %15, align 2, !tbaa !15
  %146 = trunc i64 %145 to i32
  call void @report(i32 noundef %146) #5
  %147 = load i64, ptr %15, align 2, !tbaa !15
  %148 = udiv i64 %147, 256
  %149 = trunc i64 %148 to i32
  call void @report(i32 noundef %149) #5
  call void @llvm.lifetime.end.p0(ptr %15) #4
  br label %150

150:                                              ; preds = %135
  br label %151

151:                                              ; preds = %150
  br label %152

152:                                              ; preds = %151
  %153 = load i16, ptr %7, align 2, !tbaa !12
  %154 = add nsw i16 %153, 1
  store i16 %154, ptr %7, align 2, !tbaa !12
  br label %20, !llvm.loop !17

155:                                              ; preds = %20
  br label %156

156:                                              ; preds = %155
  %157 = load i16, ptr %6, align 2, !tbaa !12
  %158 = add nsw i16 %157, 1
  store i16 %158, ptr %6, align 2, !tbaa !12
  br label %16, !llvm.loop !19

159:                                              ; preds = %16
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.sadd.sat.i32(i32, i32) #2

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.ssub.sat.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.uadd.sat.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.usub.sat.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.sadd.sat.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.ssub.sat.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.uadd.sat.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.usub.sat.i64(i64, i64) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { optsize }

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
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 long", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"p1 long long", !9, i64 0}
!12 = !{!4, !4, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !5, i64 0}
!15 = !{!16, !16, i64 0}
!16 = !{!"long long", !5, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
!19 = distinct !{!19, !18}
