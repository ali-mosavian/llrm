; ModuleID = 'clip.c'
source_filename = "clip.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

%struct.GdFill = type { ptr, ptr, ptr, ptr, i32, i32, i32, i32, i32 }
%struct.Point = type { i64, i64 }

@pattern = internal global i32 0, align 4
@phase = internal global i32 0, align 4
@gfx_x1 = external dso_local global i32, align 4
@gfx_x2 = external dso_local global i32, align 4
@gfx_y1 = external dso_local global i32, align 4
@gfx_y2 = external dso_local global i32, align 4
@gfx_view_left = external dso_local global i32, align 4
@gfx_view_right = external dso_local global i32, align 4
@gfx_view_top = external dso_local global i32, align 4
@gfx_view_bottom = external dso_local global i32, align 4

; Function Attrs: nounwind optsize uwtable
define dso_local void @gfx_line_between(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i8, align 1
  %8 = alloca i32, align 4
  %9 = alloca %struct.GdFill, align 8
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  %13 = alloca i32, align 4
  %14 = alloca i32, align 4
  %15 = alloca i32, align 4
  %16 = alloca i32, align 4
  %17 = alloca i32, align 4
  %18 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !14
  store i32 %1, ptr %5, align 4, !tbaa !14
  store i32 %2, ptr %6, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %19 = load i32, ptr %4, align 4, !tbaa !14
  %20 = call i8 @gfx_color(i32 noundef %19) #4
  store i8 %20, ptr %7, align 1, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %21 = load i32, ptr %5, align 4, !tbaa !14
  %22 = icmp eq i32 %21, -1
  %23 = zext i1 %22 to i32
  store i32 %23, ptr %8, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %9) #3
  %24 = load i32, ptr %5, align 4, !tbaa !14
  %25 = and i32 %24, 65535
  store i32 %25, ptr @pattern, align 4, !tbaa !14
  store i32 0, ptr @phase, align 4, !tbaa !14
  %26 = load i32, ptr %8, align 4, !tbaa !14
  %27 = icmp ne i32 %26, 0
  br i1 %27, label %28, label %114

28:                                               ; preds = %3
  %29 = load i32, ptr %6, align 4, !tbaa !14
  %30 = icmp eq i32 %29, 2
  br i1 %30, label %42, label %31

31:                                               ; preds = %28
  %32 = load i32, ptr %6, align 4, !tbaa !14
  %33 = icmp eq i32 %32, 0
  br i1 %33, label %34, label %114

34:                                               ; preds = %31
  %35 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %36 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %37 = icmp eq i32 %35, %36
  br i1 %37, label %42, label %38

38:                                               ; preds = %34
  %39 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %40 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %41 = icmp eq i32 %39, %40
  br i1 %41, label %42, label %114

42:                                               ; preds = %38, %34, %28
  call void @llvm.lifetime.start.p0(ptr %10) #3
  %43 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %44 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %45 = icmp slt i32 %43, %44
  br i1 %45, label %46, label %48

46:                                               ; preds = %42
  %47 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  br label %50

48:                                               ; preds = %42
  %49 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  br label %50

50:                                               ; preds = %48, %46
  %51 = phi i32 [ %47, %46 ], [ %49, %48 ]
  store i32 %51, ptr %10, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %11) #3
  %52 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %53 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %54 = icmp slt i32 %52, %53
  br i1 %54, label %55, label %57

55:                                               ; preds = %50
  %56 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  br label %59

57:                                               ; preds = %50
  %58 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  br label %59

59:                                               ; preds = %57, %55
  %60 = phi i32 [ %56, %55 ], [ %58, %57 ]
  store i32 %60, ptr %11, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %12) #3
  %61 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %62 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %63 = icmp slt i32 %61, %62
  br i1 %63, label %64, label %66

64:                                               ; preds = %59
  %65 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  br label %68

66:                                               ; preds = %59
  %67 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  br label %68

68:                                               ; preds = %66, %64
  %69 = phi i32 [ %65, %64 ], [ %67, %66 ]
  store i32 %69, ptr %12, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %13) #3
  %70 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %71 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %72 = icmp slt i32 %70, %71
  br i1 %72, label %73, label %75

73:                                               ; preds = %68
  %74 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  br label %77

75:                                               ; preds = %68
  %76 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  br label %77

77:                                               ; preds = %75, %73
  %78 = phi i32 [ %74, %73 ], [ %76, %75 ]
  store i32 %78, ptr %13, align 4, !tbaa !14
  %79 = load i32, ptr %10, align 4, !tbaa !14
  %80 = load i32, ptr @gfx_view_left, align 4, !tbaa !14
  %81 = icmp sge i32 %79, %80
  br i1 %81, label %82, label %110

82:                                               ; preds = %77
  %83 = load i32, ptr %11, align 4, !tbaa !14
  %84 = load i32, ptr @gfx_view_right, align 4, !tbaa !14
  %85 = icmp sle i32 %83, %84
  br i1 %85, label %86, label %110

86:                                               ; preds = %82
  %87 = load i32, ptr %12, align 4, !tbaa !14
  %88 = load i32, ptr @gfx_view_top, align 4, !tbaa !14
  %89 = icmp sge i32 %87, %88
  br i1 %89, label %90, label %110

90:                                               ; preds = %86
  %91 = load i32, ptr %13, align 4, !tbaa !14
  %92 = load i32, ptr @gfx_view_bottom, align 4, !tbaa !14
  %93 = icmp sle i32 %91, %92
  br i1 %93, label %94, label %110

94:                                               ; preds = %90
  %95 = load i32, ptr %10, align 4, !tbaa !14
  %96 = load i32, ptr %12, align 4, !tbaa !14
  %97 = load i32, ptr %11, align 4, !tbaa !14
  %98 = load i32, ptr %10, align 4, !tbaa !14
  %99 = sub nsw i32 %97, %98
  %100 = add nsw i32 %99, 1
  %101 = load i32, ptr %13, align 4, !tbaa !14
  %102 = load i32, ptr %12, align 4, !tbaa !14
  %103 = sub nsw i32 %101, %102
  %104 = add nsw i32 %103, 1
  %105 = load i8, ptr %7, align 1, !tbaa !15
  %106 = zext i8 %105 to i32
  %107 = call i32 @gd_solid_box(i32 noundef %95, i32 noundef %96, i32 noundef %100, i32 noundef %104, i32 noundef %106) #4
  %108 = icmp ne i32 %107, 0
  br i1 %108, label %109, label %110

109:                                              ; preds = %94
  store i32 1, ptr %14, align 4
  br label %111

110:                                              ; preds = %94, %90, %86, %82, %77
  store i32 0, ptr %14, align 4
  br label %111

111:                                              ; preds = %110, %109
  call void @llvm.lifetime.end.p0(ptr %13) #3
  call void @llvm.lifetime.end.p0(ptr %12) #3
  call void @llvm.lifetime.end.p0(ptr %11) #3
  call void @llvm.lifetime.end.p0(ptr %10) #3
  %112 = load i32, ptr %14, align 4
  switch i32 %112, label %175 [
    i32 0, label %113
  ]

113:                                              ; preds = %111
  br label %114

114:                                              ; preds = %113, %38, %31, %3
  %115 = load i8, ptr %7, align 1, !tbaa !15
  %116 = zext i8 %115 to i32
  call void @gd_fill_select(ptr noundef %9, i32 noundef %116, i32 noundef 0) #4
  %117 = load i32, ptr %6, align 4, !tbaa !14
  %118 = icmp eq i32 %117, 2
  br i1 %118, label %119, label %143

119:                                              ; preds = %114
  call void @llvm.lifetime.start.p0(ptr %15) #3
  %120 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  store i32 %120, ptr %15, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %16) #3
  %121 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  store i32 %121, ptr %16, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %17) #3
  %122 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  store i32 %122, ptr %17, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %18) #3
  %123 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  store i32 %123, ptr %18, align 4, !tbaa !14
  %124 = call i32 @gfx_clip_box(ptr noundef %15, ptr noundef %16, ptr noundef %17, ptr noundef %18) #4
  %125 = icmp ne i32 %124, 0
  br i1 %125, label %127, label %126

126:                                              ; preds = %119
  store i32 1, ptr %14, align 4
  br label %140

127:                                              ; preds = %119
  %128 = getelementptr inbounds nuw %struct.GdFill, ptr %9, i32 0, i32 0
  %129 = load ptr, ptr %128, align 8, !tbaa !16
  %130 = load i32, ptr %15, align 4, !tbaa !14
  %131 = load i32, ptr %16, align 4, !tbaa !14
  %132 = load i32, ptr %17, align 4, !tbaa !14
  %133 = load i32, ptr %15, align 4, !tbaa !14
  %134 = sub nsw i32 %132, %133
  %135 = add nsw i32 %134, 1
  %136 = load i32, ptr %18, align 4, !tbaa !14
  %137 = load i32, ptr %16, align 4, !tbaa !14
  %138 = sub nsw i32 %136, %137
  %139 = add nsw i32 %138, 1
  call void %129(ptr noundef %9, i32 noundef %130, i32 noundef %131, i32 noundef %135, i32 noundef %139) #4
  store i32 0, ptr %14, align 4
  br label %140

140:                                              ; preds = %127, %126
  call void @llvm.lifetime.end.p0(ptr %18) #3
  call void @llvm.lifetime.end.p0(ptr %17) #3
  call void @llvm.lifetime.end.p0(ptr %16) #3
  call void @llvm.lifetime.end.p0(ptr %15) #3
  %141 = load i32, ptr %14, align 4
  switch i32 %141, label %175 [
    i32 0, label %142
  ]

142:                                              ; preds = %140
  br label %174

143:                                              ; preds = %114
  %144 = load i32, ptr %6, align 4, !tbaa !14
  %145 = icmp eq i32 %144, 1
  br i1 %145, label %146, label %167

146:                                              ; preds = %143
  %147 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %148 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %149 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %150 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %151 = load i32, ptr %8, align 4, !tbaa !14
  call void @line(ptr noundef %9, i32 noundef %147, i32 noundef %148, i32 noundef %149, i32 noundef %150, i32 noundef %151) #4
  %152 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %153 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %154 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %155 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %156 = load i32, ptr %8, align 4, !tbaa !14
  call void @line(ptr noundef %9, i32 noundef %152, i32 noundef %153, i32 noundef %154, i32 noundef %155, i32 noundef %156) #4
  %157 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %158 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %159 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %160 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %161 = load i32, ptr %8, align 4, !tbaa !14
  call void @line(ptr noundef %9, i32 noundef %157, i32 noundef %158, i32 noundef %159, i32 noundef %160, i32 noundef %161) #4
  %162 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %163 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %164 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %165 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %166 = load i32, ptr %8, align 4, !tbaa !14
  call void @line(ptr noundef %9, i32 noundef %162, i32 noundef %163, i32 noundef %164, i32 noundef %165, i32 noundef %166) #4
  br label %173

167:                                              ; preds = %143
  %168 = load i32, ptr @gfx_x1, align 4, !tbaa !14
  %169 = load i32, ptr @gfx_y1, align 4, !tbaa !14
  %170 = load i32, ptr @gfx_x2, align 4, !tbaa !14
  %171 = load i32, ptr @gfx_y2, align 4, !tbaa !14
  %172 = load i32, ptr %8, align 4, !tbaa !14
  call void @line(ptr noundef %9, i32 noundef %168, i32 noundef %169, i32 noundef %170, i32 noundef %171, i32 noundef %172) #4
  br label %173

173:                                              ; preds = %167, %146
  br label %174

174:                                              ; preds = %173, %142
  store i32 0, ptr %14, align 4
  br label %175

175:                                              ; preds = %174, %140, %111
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  %176 = load i32, ptr %14, align 4
  switch i32 %176, label %178 [
    i32 0, label %177
    i32 1, label %177
  ]

177:                                              ; preds = %175, %175
  ret void

178:                                              ; preds = %175
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local i8 @gfx_color(i32 noundef) #2

; Function Attrs: optsize
declare dso_local i32 @gd_solid_box(i32 noundef, i32 noundef, i32 noundef, i32 noundef, i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @gd_fill_select(ptr noundef, i32 noundef, i32 noundef) #2

; Function Attrs: optsize
declare dso_local i32 @gfx_clip_box(ptr noundef, ptr noundef, ptr noundef, ptr noundef) #2

; Function Attrs: nounwind optsize uwtable
define internal void @line(ptr noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, i32 noundef %5) #0 {
  %7 = alloca ptr, align 8
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  %13 = alloca %struct.Point, align 8
  %14 = alloca %struct.Point, align 8
  %15 = alloca i32, align 4
  %16 = alloca i32, align 4
  %17 = alloca i32, align 4
  %18 = alloca i32, align 4
  %19 = alloca i32, align 4
  %20 = alloca %struct.Point, align 8
  store ptr %0, ptr %7, align 8, !tbaa !19
  store i32 %1, ptr %8, align 4, !tbaa !14
  store i32 %2, ptr %9, align 4, !tbaa !14
  store i32 %3, ptr %10, align 4, !tbaa !14
  store i32 %4, ptr %11, align 4, !tbaa !14
  store i32 %5, ptr %12, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %13) #3
  call void @llvm.lifetime.start.p0(ptr %14) #3
  call void @llvm.lifetime.start.p0(ptr %15) #3
  call void @llvm.lifetime.start.p0(ptr %16) #3
  call void @llvm.lifetime.start.p0(ptr %17) #3
  call void @llvm.lifetime.start.p0(ptr %18) #3
  %21 = load i32, ptr %8, align 4, !tbaa !14
  %22 = sext i32 %21 to i64
  %23 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 0
  store i64 %22, ptr %23, align 8, !tbaa !21
  %24 = load i32, ptr %9, align 4, !tbaa !14
  %25 = sext i32 %24 to i64
  %26 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 1
  store i64 %25, ptr %26, align 8, !tbaa !24
  %27 = load i32, ptr %10, align 4, !tbaa !14
  %28 = sext i32 %27 to i64
  %29 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 0
  store i64 %28, ptr %29, align 8, !tbaa !21
  %30 = load i32, ptr %11, align 4, !tbaa !14
  %31 = sext i32 %30 to i64
  %32 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 1
  store i64 %31, ptr %32, align 8, !tbaa !24
  %33 = call i32 @clip(ptr noundef %13, ptr noundef %14) #4
  %34 = icmp ne i32 %33, 0
  br i1 %34, label %36, label %35

35:                                               ; preds = %6
  store i32 1, ptr %19, align 4
  br label %145

36:                                               ; preds = %6
  %37 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 0
  %38 = load i64, ptr %37, align 8, !tbaa !21
  %39 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 0
  %40 = load i64, ptr %39, align 8, !tbaa !21
  %41 = icmp sgt i64 %38, %40
  br i1 %41, label %42, label %43

42:                                               ; preds = %36
  call void @llvm.lifetime.start.p0(ptr %20) #3
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %20, ptr align 8 %13, i64 16, i1 false), !tbaa.struct !25
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %13, ptr align 8 %14, i64 16, i1 false), !tbaa.struct !25
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %14, ptr align 8 %20, i64 16, i1 false), !tbaa.struct !25
  call void @llvm.lifetime.end.p0(ptr %20) #3
  br label %43

43:                                               ; preds = %42, %36
  %44 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 0
  %45 = load i64, ptr %44, align 8, !tbaa !21
  %46 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 0
  %47 = load i64, ptr %46, align 8, !tbaa !21
  %48 = sub nsw i64 %45, %47
  %49 = trunc i64 %48 to i32
  store i32 %49, ptr %15, align 4, !tbaa !14
  %50 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 1
  %51 = load i64, ptr %50, align 8, !tbaa !24
  %52 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 1
  %53 = load i64, ptr %52, align 8, !tbaa !24
  %54 = icmp sgt i64 %51, %53
  br i1 %54, label %55, label %61

55:                                               ; preds = %43
  %56 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 1
  %57 = load i64, ptr %56, align 8, !tbaa !24
  %58 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 1
  %59 = load i64, ptr %58, align 8, !tbaa !24
  %60 = sub nsw i64 %57, %59
  br label %67

61:                                               ; preds = %43
  %62 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 1
  %63 = load i64, ptr %62, align 8, !tbaa !24
  %64 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 1
  %65 = load i64, ptr %64, align 8, !tbaa !24
  %66 = sub nsw i64 %63, %65
  br label %67

67:                                               ; preds = %61, %55
  %68 = phi i64 [ %60, %55 ], [ %66, %61 ]
  %69 = trunc i64 %68 to i32
  store i32 %69, ptr %16, align 4, !tbaa !14
  %70 = getelementptr inbounds nuw %struct.Point, ptr %14, i32 0, i32 1
  %71 = load i64, ptr %70, align 8, !tbaa !24
  %72 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 1
  %73 = load i64, ptr %72, align 8, !tbaa !24
  %74 = icmp sgt i64 %71, %73
  %75 = zext i1 %74 to i64
  %76 = select i1 %74, i32 1, i32 -1
  store i32 %76, ptr %17, align 4, !tbaa !14
  %77 = load i32, ptr %12, align 4, !tbaa !14
  %78 = icmp ne i32 %77, 0
  br i1 %78, label %79, label %98

79:                                               ; preds = %67
  %80 = load i32, ptr %16, align 4, !tbaa !14
  %81 = icmp eq i32 %80, 0
  br i1 %81, label %82, label %95

82:                                               ; preds = %79
  %83 = load ptr, ptr %7, align 8, !tbaa !19
  %84 = getelementptr inbounds nuw %struct.GdFill, ptr %83, i32 0, i32 0
  %85 = load ptr, ptr %84, align 8, !tbaa !16
  %86 = load ptr, ptr %7, align 8, !tbaa !19
  %87 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 0
  %88 = load i64, ptr %87, align 8, !tbaa !21
  %89 = trunc i64 %88 to i32
  %90 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 1
  %91 = load i64, ptr %90, align 8, !tbaa !24
  %92 = trunc i64 %91 to i32
  %93 = load i32, ptr %15, align 4, !tbaa !14
  %94 = add i32 %93, 1
  call void %85(ptr noundef %86, i32 noundef %89, i32 noundef %92, i32 noundef %94, i32 noundef 1) #4
  store i32 1, ptr %19, align 4
  br label %145

95:                                               ; preds = %79
  %96 = load ptr, ptr %7, align 8, !tbaa !19
  %97 = getelementptr inbounds nuw %struct.GdFill, ptr %96, i32 0, i32 8
  store i32 65535, ptr %97, align 8, !tbaa !27
  br label %131

98:                                               ; preds = %67
  %99 = load i32, ptr @phase, align 4, !tbaa !14
  %100 = and i32 %99, 15
  store i32 %100, ptr %18, align 4, !tbaa !14
  %101 = load i32, ptr %18, align 4, !tbaa !14
  %102 = icmp ne i32 %101, 0
  br i1 %102, label %103, label %113

103:                                              ; preds = %98
  %104 = load i32, ptr @pattern, align 4, !tbaa !14
  %105 = load i32, ptr %18, align 4, !tbaa !14
  %106 = shl i32 %104, %105
  %107 = load i32, ptr @pattern, align 4, !tbaa !14
  %108 = load i32, ptr %18, align 4, !tbaa !14
  %109 = sub i32 16, %108
  %110 = lshr i32 %107, %109
  %111 = or i32 %106, %110
  %112 = and i32 %111, 65535
  br label %115

113:                                              ; preds = %98
  %114 = load i32, ptr @pattern, align 4, !tbaa !14
  br label %115

115:                                              ; preds = %113, %103
  %116 = phi i32 [ %112, %103 ], [ %114, %113 ]
  %117 = load ptr, ptr %7, align 8, !tbaa !19
  %118 = getelementptr inbounds nuw %struct.GdFill, ptr %117, i32 0, i32 8
  store i32 %116, ptr %118, align 8, !tbaa !27
  %119 = load i32, ptr %15, align 4, !tbaa !14
  %120 = load i32, ptr %16, align 4, !tbaa !14
  %121 = icmp sgt i32 %119, %120
  br i1 %121, label %122, label %124

122:                                              ; preds = %115
  %123 = load i32, ptr %15, align 4, !tbaa !14
  br label %126

124:                                              ; preds = %115
  %125 = load i32, ptr %16, align 4, !tbaa !14
  br label %126

126:                                              ; preds = %124, %122
  %127 = phi i32 [ %123, %122 ], [ %125, %124 ]
  %128 = add i32 %127, 1
  %129 = load i32, ptr @phase, align 4, !tbaa !14
  %130 = add i32 %129, %128
  store i32 %130, ptr @phase, align 4, !tbaa !14
  br label %131

131:                                              ; preds = %126, %95
  %132 = load ptr, ptr %7, align 8, !tbaa !19
  %133 = getelementptr inbounds nuw %struct.GdFill, ptr %132, i32 0, i32 2
  %134 = load ptr, ptr %133, align 8, !tbaa !28
  %135 = load ptr, ptr %7, align 8, !tbaa !19
  %136 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 0
  %137 = load i64, ptr %136, align 8, !tbaa !21
  %138 = trunc i64 %137 to i32
  %139 = getelementptr inbounds nuw %struct.Point, ptr %13, i32 0, i32 1
  %140 = load i64, ptr %139, align 8, !tbaa !24
  %141 = trunc i64 %140 to i32
  %142 = load i32, ptr %15, align 4, !tbaa !14
  %143 = load i32, ptr %16, align 4, !tbaa !14
  %144 = load i32, ptr %17, align 4, !tbaa !14
  call void %134(ptr noundef %135, i32 noundef %138, i32 noundef %141, i32 noundef %142, i32 noundef %143, i32 noundef %144) #4
  store i32 0, ptr %19, align 4
  br label %145

145:                                              ; preds = %131, %82, %35
  call void @llvm.lifetime.end.p0(ptr %18) #3
  call void @llvm.lifetime.end.p0(ptr %17) #3
  call void @llvm.lifetime.end.p0(ptr %16) #3
  call void @llvm.lifetime.end.p0(ptr %15) #3
  call void @llvm.lifetime.end.p0(ptr %14) #3
  call void @llvm.lifetime.end.p0(ptr %13) #3
  %146 = load i32, ptr %19, align 4
  switch i32 %146, label %148 [
    i32 0, label %147
    i32 1, label %147
  ]

147:                                              ; preds = %145, %145
  ret void

148:                                              ; preds = %145
  unreachable
}

; Function Attrs: nounwind optsize uwtable
define dso_local void @B_LINE(i16 noundef %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !29
  store i16 %1, ptr %5, align 2, !tbaa !29
  store i16 %2, ptr %6, align 2, !tbaa !29
  %7 = load i16, ptr %4, align 2, !tbaa !29
  %8 = sext i16 %7 to i32
  %9 = load i16, ptr %5, align 2, !tbaa !29
  %10 = sext i16 %9 to i32
  %11 = load i16, ptr %6, align 2, !tbaa !29
  %12 = sext i16 %11 to i32
  call void @gfx_line_between(i32 noundef %8, i32 noundef %10, i32 noundef %12) #4
  ret void
}

; Function Attrs: nounwind optsize uwtable
define internal i32 @clip(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca %struct.Point, align 8
  %10 = alloca i64, align 8
  %11 = alloca i64, align 8
  store ptr %0, ptr %4, align 8, !tbaa !31
  store ptr %1, ptr %5, align 8, !tbaa !31
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %12 = load ptr, ptr %4, align 8, !tbaa !31
  %13 = call i32 @outside(ptr noundef %12) #4
  store i32 %13, ptr %6, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %14 = load ptr, ptr %5, align 8, !tbaa !31
  %15 = call i32 @outside(ptr noundef %14) #4
  store i32 %15, ptr %7, align 4, !tbaa !14
  br label %16

16:                                               ; preds = %101, %2
  %17 = load i32, ptr %6, align 4, !tbaa !14
  %18 = load i32, ptr %7, align 4, !tbaa !14
  %19 = or i32 %17, %18
  %20 = icmp ne i32 %19, 0
  br i1 %20, label %22, label %21

21:                                               ; preds = %16
  store i32 1, ptr %3, align 4
  store i32 1, ptr %8, align 4
  br label %104

22:                                               ; preds = %16
  %23 = load i32, ptr %6, align 4, !tbaa !14
  %24 = load i32, ptr %7, align 4, !tbaa !14
  %25 = and i32 %23, %24
  %26 = icmp ne i32 %25, 0
  br i1 %26, label %27, label %28

27:                                               ; preds = %22
  store i32 0, ptr %3, align 4
  store i32 1, ptr %8, align 4
  br label %104

28:                                               ; preds = %22
  %29 = load i32, ptr %6, align 4, !tbaa !14
  %30 = icmp ne i32 %29, 0
  br i1 %30, label %37, label %31

31:                                               ; preds = %28
  call void @llvm.lifetime.start.p0(ptr %9) #3
  %32 = load ptr, ptr %4, align 8, !tbaa !31
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %9, ptr align 8 %32, i64 16, i1 false), !tbaa.struct !25
  %33 = load ptr, ptr %4, align 8, !tbaa !31
  %34 = load ptr, ptr %5, align 8, !tbaa !31
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %33, ptr align 8 %34, i64 16, i1 false), !tbaa.struct !25
  %35 = load ptr, ptr %5, align 8, !tbaa !31
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %35, ptr align 8 %9, i64 16, i1 false), !tbaa.struct !25
  %36 = load i32, ptr %7, align 4, !tbaa !14
  store i32 %36, ptr %6, align 4, !tbaa !14
  store i32 0, ptr %7, align 4, !tbaa !14
  call void @llvm.lifetime.end.p0(ptr %9) #3
  br label %37

37:                                               ; preds = %31, %28
  %38 = load i32, ptr %6, align 4, !tbaa !14
  %39 = and i32 %38, 3
  %40 = icmp ne i32 %39, 0
  br i1 %40, label %41, label %71

41:                                               ; preds = %37
  call void @llvm.lifetime.start.p0(ptr %10) #3
  %42 = load i32, ptr %6, align 4, !tbaa !14
  %43 = and i32 %42, 1
  %44 = icmp ne i32 %43, 0
  br i1 %44, label %45, label %47

45:                                               ; preds = %41
  %46 = load i32, ptr @gfx_view_left, align 4, !tbaa !14
  br label %49

47:                                               ; preds = %41
  %48 = load i32, ptr @gfx_view_right, align 4, !tbaa !14
  br label %49

49:                                               ; preds = %47, %45
  %50 = phi i32 [ %46, %45 ], [ %48, %47 ]
  %51 = sext i32 %50 to i64
  store i64 %51, ptr %10, align 8, !tbaa !26
  %52 = load ptr, ptr %4, align 8, !tbaa !31
  %53 = getelementptr inbounds nuw %struct.Point, ptr %52, i32 0, i32 0
  %54 = load i64, ptr %53, align 8, !tbaa !21
  %55 = load ptr, ptr %4, align 8, !tbaa !31
  %56 = getelementptr inbounds nuw %struct.Point, ptr %55, i32 0, i32 1
  %57 = load i64, ptr %56, align 8, !tbaa !24
  %58 = load ptr, ptr %5, align 8, !tbaa !31
  %59 = getelementptr inbounds nuw %struct.Point, ptr %58, i32 0, i32 0
  %60 = load i64, ptr %59, align 8, !tbaa !21
  %61 = load ptr, ptr %5, align 8, !tbaa !31
  %62 = getelementptr inbounds nuw %struct.Point, ptr %61, i32 0, i32 1
  %63 = load i64, ptr %62, align 8, !tbaa !24
  %64 = load i64, ptr %10, align 8, !tbaa !26
  %65 = call i64 @cut(i64 noundef %54, i64 noundef %57, i64 noundef %60, i64 noundef %63, i64 noundef %64) #4
  %66 = load ptr, ptr %4, align 8, !tbaa !31
  %67 = getelementptr inbounds nuw %struct.Point, ptr %66, i32 0, i32 1
  store i64 %65, ptr %67, align 8, !tbaa !24
  %68 = load i64, ptr %10, align 8, !tbaa !26
  %69 = load ptr, ptr %4, align 8, !tbaa !31
  %70 = getelementptr inbounds nuw %struct.Point, ptr %69, i32 0, i32 0
  store i64 %68, ptr %70, align 8, !tbaa !21
  call void @llvm.lifetime.end.p0(ptr %10) #3
  br label %101

71:                                               ; preds = %37
  call void @llvm.lifetime.start.p0(ptr %11) #3
  %72 = load i32, ptr %6, align 4, !tbaa !14
  %73 = and i32 %72, 4
  %74 = icmp ne i32 %73, 0
  br i1 %74, label %75, label %77

75:                                               ; preds = %71
  %76 = load i32, ptr @gfx_view_top, align 4, !tbaa !14
  br label %79

77:                                               ; preds = %71
  %78 = load i32, ptr @gfx_view_bottom, align 4, !tbaa !14
  br label %79

79:                                               ; preds = %77, %75
  %80 = phi i32 [ %76, %75 ], [ %78, %77 ]
  %81 = sext i32 %80 to i64
  store i64 %81, ptr %11, align 8, !tbaa !26
  %82 = load ptr, ptr %4, align 8, !tbaa !31
  %83 = getelementptr inbounds nuw %struct.Point, ptr %82, i32 0, i32 1
  %84 = load i64, ptr %83, align 8, !tbaa !24
  %85 = load ptr, ptr %4, align 8, !tbaa !31
  %86 = getelementptr inbounds nuw %struct.Point, ptr %85, i32 0, i32 0
  %87 = load i64, ptr %86, align 8, !tbaa !21
  %88 = load ptr, ptr %5, align 8, !tbaa !31
  %89 = getelementptr inbounds nuw %struct.Point, ptr %88, i32 0, i32 1
  %90 = load i64, ptr %89, align 8, !tbaa !24
  %91 = load ptr, ptr %5, align 8, !tbaa !31
  %92 = getelementptr inbounds nuw %struct.Point, ptr %91, i32 0, i32 0
  %93 = load i64, ptr %92, align 8, !tbaa !21
  %94 = load i64, ptr %11, align 8, !tbaa !26
  %95 = call i64 @cut(i64 noundef %84, i64 noundef %87, i64 noundef %90, i64 noundef %93, i64 noundef %94) #4
  %96 = load ptr, ptr %4, align 8, !tbaa !31
  %97 = getelementptr inbounds nuw %struct.Point, ptr %96, i32 0, i32 0
  store i64 %95, ptr %97, align 8, !tbaa !21
  %98 = load i64, ptr %11, align 8, !tbaa !26
  %99 = load ptr, ptr %4, align 8, !tbaa !31
  %100 = getelementptr inbounds nuw %struct.Point, ptr %99, i32 0, i32 1
  store i64 %98, ptr %100, align 8, !tbaa !24
  call void @llvm.lifetime.end.p0(ptr %11) #3
  br label %101

101:                                              ; preds = %79, %49
  %102 = load ptr, ptr %4, align 8, !tbaa !31
  %103 = call i32 @outside(ptr noundef %102) #4
  store i32 %103, ptr %6, align 4, !tbaa !14
  br label %16

104:                                              ; preds = %27, %21
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %105 = load i32, ptr %3, align 4
  ret i32 %105
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #1

; Function Attrs: nounwind optsize uwtable
define internal i32 @outside(ptr noundef %0) #0 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8, !tbaa !31
  %3 = load ptr, ptr %2, align 8, !tbaa !31
  %4 = getelementptr inbounds nuw %struct.Point, ptr %3, i32 0, i32 0
  %5 = load i64, ptr %4, align 8, !tbaa !21
  %6 = load i32, ptr @gfx_view_left, align 4, !tbaa !14
  %7 = sext i32 %6 to i64
  %8 = icmp slt i64 %5, %7
  %9 = zext i1 %8 to i32
  %10 = load ptr, ptr %2, align 8, !tbaa !31
  %11 = getelementptr inbounds nuw %struct.Point, ptr %10, i32 0, i32 0
  %12 = load i64, ptr %11, align 8, !tbaa !21
  %13 = load i32, ptr @gfx_view_right, align 4, !tbaa !14
  %14 = sext i32 %13 to i64
  %15 = icmp sgt i64 %12, %14
  %16 = zext i1 %15 to i32
  %17 = shl i32 %16, 1
  %18 = or i32 %9, %17
  %19 = load ptr, ptr %2, align 8, !tbaa !31
  %20 = getelementptr inbounds nuw %struct.Point, ptr %19, i32 0, i32 1
  %21 = load i64, ptr %20, align 8, !tbaa !24
  %22 = load i32, ptr @gfx_view_top, align 4, !tbaa !14
  %23 = sext i32 %22 to i64
  %24 = icmp slt i64 %21, %23
  %25 = zext i1 %24 to i32
  %26 = shl i32 %25, 2
  %27 = or i32 %18, %26
  %28 = load ptr, ptr %2, align 8, !tbaa !31
  %29 = getelementptr inbounds nuw %struct.Point, ptr %28, i32 0, i32 1
  %30 = load i64, ptr %29, align 8, !tbaa !24
  %31 = load i32, ptr @gfx_view_bottom, align 4, !tbaa !14
  %32 = sext i32 %31 to i64
  %33 = icmp sgt i64 %30, %32
  %34 = zext i1 %33 to i32
  %35 = shl i32 %34, 3
  %36 = or i32 %27, %35
  ret i32 %36
}

; Function Attrs: nounwind optsize uwtable
define internal i64 @cut(i64 noundef %0, i64 noundef %1, i64 noundef %2, i64 noundef %3, i64 noundef %4) #0 {
  %6 = alloca i64, align 8
  %7 = alloca i64, align 8
  %8 = alloca i64, align 8
  %9 = alloca i64, align 8
  %10 = alloca i64, align 8
  %11 = alloca i64, align 8
  %12 = alloca i64, align 8
  %13 = alloca i64, align 8
  %14 = alloca i64, align 8
  store i64 %0, ptr %6, align 8, !tbaa !26
  store i64 %1, ptr %7, align 8, !tbaa !26
  store i64 %2, ptr %8, align 8, !tbaa !26
  store i64 %3, ptr %9, align 8, !tbaa !26
  store i64 %4, ptr %10, align 8, !tbaa !26
  call void @llvm.lifetime.start.p0(ptr %11) #3
  %15 = load i64, ptr %9, align 8, !tbaa !26
  %16 = load i64, ptr %7, align 8, !tbaa !26
  %17 = sub nsw i64 %15, %16
  %18 = load i64, ptr %10, align 8, !tbaa !26
  %19 = load i64, ptr %6, align 8, !tbaa !26
  %20 = sub nsw i64 %18, %19
  %21 = mul nsw i64 %17, %20
  store i64 %21, ptr %11, align 8, !tbaa !26
  call void @llvm.lifetime.start.p0(ptr %12) #3
  %22 = load i64, ptr %8, align 8, !tbaa !26
  %23 = load i64, ptr %6, align 8, !tbaa !26
  %24 = sub nsw i64 %22, %23
  store i64 %24, ptr %12, align 8, !tbaa !26
  call void @llvm.lifetime.start.p0(ptr %13) #3
  call void @llvm.lifetime.start.p0(ptr %14) #3
  store i64 1, ptr %14, align 8, !tbaa !26
  %25 = load i64, ptr %12, align 8, !tbaa !26
  %26 = icmp slt i64 %25, 0
  br i1 %26, label %27, label %32

27:                                               ; preds = %5
  %28 = load i64, ptr %12, align 8, !tbaa !26
  %29 = sub nsw i64 0, %28
  store i64 %29, ptr %12, align 8, !tbaa !26
  %30 = load i64, ptr %11, align 8, !tbaa !26
  %31 = sub nsw i64 0, %30
  store i64 %31, ptr %11, align 8, !tbaa !26
  br label %32

32:                                               ; preds = %27, %5
  %33 = load i64, ptr %11, align 8, !tbaa !26
  %34 = icmp slt i64 %33, 0
  br i1 %34, label %35, label %38

35:                                               ; preds = %32
  %36 = load i64, ptr %11, align 8, !tbaa !26
  %37 = sub nsw i64 0, %36
  store i64 %37, ptr %11, align 8, !tbaa !26
  store i64 -1, ptr %14, align 8, !tbaa !26
  br label %38

38:                                               ; preds = %35, %32
  %39 = load i64, ptr %11, align 8, !tbaa !26
  %40 = mul nsw i64 2, %39
  %41 = load i64, ptr %12, align 8, !tbaa !26
  %42 = add nsw i64 %40, %41
  %43 = load i64, ptr %12, align 8, !tbaa !26
  %44 = mul nsw i64 2, %43
  %45 = sdiv i64 %42, %44
  store i64 %45, ptr %13, align 8, !tbaa !26
  %46 = load i64, ptr %7, align 8, !tbaa !26
  %47 = load i64, ptr %14, align 8, !tbaa !26
  %48 = load i64, ptr %13, align 8, !tbaa !26
  %49 = mul nsw i64 %47, %48
  %50 = add nsw i64 %46, %49
  call void @llvm.lifetime.end.p0(ptr %14) #3
  call void @llvm.lifetime.end.p0(ptr %13) #3
  call void @llvm.lifetime.end.p0(ptr %12) #3
  call void @llvm.lifetime.end.p0(ptr %11) #3
  ret i64 %50
}

attributes #0 = { nounwind optsize uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 7, !"uwtable", i32 2}
!7 = !{i32 7, !"frame-pointer", i32 4}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = !{!11, !11, i64 0}
!15 = !{!12, !12, i64 0}
!16 = !{!17, !18, i64 0}
!17 = !{!"GdFill", !18, i64 0, !18, i64 8, !18, i64 16, !18, i64 24, !11, i64 32, !11, i64 36, !11, i64 40, !11, i64 44, !11, i64 48}
!18 = !{!"any pointer", !12, i64 0}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 _ZTS6GdFill", !18, i64 0}
!21 = !{!22, !23, i64 0}
!22 = !{!"Point", !23, i64 0, !23, i64 8}
!23 = !{!"long", !12, i64 0}
!24 = !{!22, !23, i64 8}
!25 = !{i64 0, i64 8, !26, i64 8, i64 8, !26}
!26 = !{!23, !23, i64 0}
!27 = !{!17, !11, i64 48}
!28 = !{!17, !18, i64 16}
!29 = !{!30, !30, i64 0}
!30 = !{!"short", !12, i64 0}
!31 = !{!32, !32, i64 0}
!32 = !{!"p1 _ZTS5Point", !18, i64 0}
