; ModuleID = '/tmp/mulc2.c'
source_filename = "/tmp/mulc2.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm2(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 2
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm2(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 2
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm3(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 3
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm3(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 3
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm5(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 5
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm5(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 5
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm6(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 6
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm6(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 6
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm7(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 7
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm7(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 7
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm9(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 9
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm9(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 9
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm10(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 10
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm10(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 10
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm12(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 12
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm12(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 12
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm15(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 15
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm15(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 15
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm17(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 17
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm17(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 17
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm24(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 24
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm24(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 24
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm28(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 28
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm28(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 28
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm31(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 31
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm31(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 31
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm33(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 33
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm33(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 33
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm40(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 40
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm40(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 40
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm63(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 63
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm63(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 63
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm65(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 65
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm65(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 65
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm100(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 100
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm100(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 100
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm255(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 255
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm255(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 255
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm257(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 257
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm257(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 257
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn1(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -1
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn1(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -1
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn2(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -2
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn2(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -2
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn3(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -3
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn3(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -3
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn5(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -5
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn5(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -5
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn6(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -6
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn6(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -6
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn7(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -7
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn7(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -7
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn9(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -9
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn9(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -9
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn12(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -12
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn12(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -12
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn15(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -15
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn15(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -15
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn28(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -28
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn28(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -28
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xn31(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, -31
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wn31(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, -31
  ret i32 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i64 @xm1000(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !9
  %3 = load i64, ptr %2, align 8, !tbaa !9
  %4 = mul nsw i64 %3, 1000
  ret i64 %4
}

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @wm1000(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !11
  %3 = load i32, ptr %2, align 4, !tbaa !11
  %4 = mul i32 %3, 1000
  ret i32 %4
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"uwtable", i32 1}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"long long", !7, i64 0}
!11 = !{!6, !6, i64 0}
