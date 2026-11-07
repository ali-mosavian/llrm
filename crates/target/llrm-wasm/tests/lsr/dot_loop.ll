target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@__main_void = hidden alias i32 (), ptr @main

define hidden i32 @bench_dot() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" nofree nosync norecurse memory(none) willreturn {
n0:
  %n1 = alloca [64 x i16], align 16
  %n2 = alloca [48 x i16], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %n1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %n2) nounwind
  br label %n3

n3:
  %n4 = phi i32 [ 0, %n0 ], [ %n24, %n3 ]
  %n5 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n4
  %n6 = trunc i32 %n4 to i16
  %n7 = mul nuw nsw i16 %n6, 3
  %n8 = add nsw i16 %n7, -90
  store i16 %n8, ptr %n5, align 8, !tbaa !8
  %n9 = or disjoint i32 %n4, 1
  %n10 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n9
  %n11 = trunc i32 %n9 to i16
  %n12 = mul nuw nsw i16 %n11, 3
  %n13 = add nsw i16 %n12, -90
  store i16 %n13, ptr %n10, align 2, !tbaa !8
  %n14 = or disjoint i32 %n4, 2
  %n15 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n14
  %n16 = trunc i32 %n14 to i16
  %n17 = mul nuw nsw i16 %n16, 3
  %n18 = add nsw i16 %n17, -90
  store i16 %n18, ptr %n15, align 4, !tbaa !8
  %n19 = or disjoint i32 %n4, 3
  %n20 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n19
  %n21 = trunc i32 %n19 to i16
  %n22 = mul nuw nsw i16 %n21, 3
  %n23 = add nsw i16 %n22, -90
  store i16 %n23, ptr %n20, align 2, !tbaa !8
  %n24 = add nuw nsw i32 %n4, 4
  %n25 = icmp eq i32 %n24, 64
  br i1 %n25, label %n26, label %n3, !llvm.loop !10

n26:
  br label %n27

n27:
  %n28 = phi i32 [ %n48, %n27 ], [ 0, %n26 ]
  %n29 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n28
  %n30 = trunc i32 %n28 to i16
  %n31 = shl nuw nsw i16 %n30, 1
  %n32 = sub nsw i16 70, %n31
  store i16 %n32, ptr %n29, align 8, !tbaa !8
  %n33 = or disjoint i32 %n28, 1
  %n34 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n33
  %n35 = trunc i32 %n33 to i16
  %n36 = shl nuw nsw i16 %n35, 1
  %n37 = sub nsw i16 70, %n36
  store i16 %n37, ptr %n34, align 2, !tbaa !8
  %n38 = or disjoint i32 %n28, 2
  %n39 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n38
  %n40 = trunc i32 %n38 to i16
  %n41 = shl nuw nsw i16 %n40, 1
  %n42 = sub nsw i16 70, %n41
  store i16 %n42, ptr %n39, align 4, !tbaa !8
  %n43 = or disjoint i32 %n28, 3
  %n44 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n43
  %n45 = trunc i32 %n43 to i16
  %n46 = shl nuw nsw i16 %n45, 1
  %n47 = sub nsw i16 70, %n46
  store i16 %n47, ptr %n44, align 2, !tbaa !8
  %n48 = add nuw nsw i32 %n28, 4
  %n49 = icmp eq i32 %n48, 48
  br i1 %n49, label %n50, label %n27, !llvm.loop !12

n50:
  br label %n51

n51:
  %n52 = phi i16 [ %n96, %n94 ], [ 0, %n50 ]
  %n53 = phi i32 [ %n95, %n94 ], [ 0, %n50 ]
  store i16 %n52, ptr %n1, align 16, !tbaa !8
  br label %n54

n54:
  %n55 = phi i32 [ 0, %n51 ], [ %n92, %n54 ]
  %n56 = phi i32 [ 0, %n51 ], [ %n91, %n54 ]
  %n92 = add nuw nsw i32 %n55, 4
  %n57 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n55
  %n58 = load i16, ptr %n57, align 8, !tbaa !8
  %n59 = sext i16 %n58 to i32
  %n60 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n55
  %n61 = load i16, ptr %n60, align 8, !tbaa !8
  %n62 = sext i16 %n61 to i32
  %n63 = mul nsw i32 %n62, %n59
  %n64 = add nsw i32 %n63, %n56
  %n65 = or disjoint i32 %n55, 1
  %n66 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n65
  %n67 = load i16, ptr %n66, align 2, !tbaa !8
  %n68 = sext i16 %n67 to i32
  %n69 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n65
  %n70 = load i16, ptr %n69, align 2, !tbaa !8
  %n71 = sext i16 %n70 to i32
  %n72 = mul nsw i32 %n71, %n68
  %n73 = add nsw i32 %n72, %n64
  %n74 = or disjoint i32 %n55, 2
  %n75 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n74
  %n76 = load i16, ptr %n75, align 4, !tbaa !8
  %n77 = sext i16 %n76 to i32
  %n78 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n74
  %n79 = load i16, ptr %n78, align 4, !tbaa !8
  %n80 = sext i16 %n79 to i32
  %n81 = mul nsw i32 %n80, %n77
  %n82 = add nsw i32 %n81, %n73
  %n83 = or disjoint i32 %n55, 3
  %n84 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n83
  %n85 = load i16, ptr %n84, align 2, !tbaa !8
  %n86 = sext i16 %n85 to i32
  %n87 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n83
  %n88 = load i16, ptr %n87, align 2, !tbaa !8
  %n89 = sext i16 %n88 to i32
  %n90 = mul nsw i32 %n89, %n86
  %n91 = add nsw i32 %n90, %n82
  %n93 = icmp eq i32 %n92, 48
  br i1 %n93, label %n94, label %n54, !llvm.loop !17

n94:
  %n95 = add nsw i32 %n91, %n53
  %n96 = add nuw nsw i16 %n52, 1
  %n97 = icmp samesign ult i16 %n52, 39
  br i1 %n97, label %n51, label %n98, !llvm.loop !13

n98:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %n2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %n1) nounwind
  ret i32 %n95
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define hidden i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" {
n0:
  %n1 = alloca [64 x i16], align 16
  %n2 = alloca [48 x i16], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %n1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %n2) nounwind
  br label %n3

n3:
  %n4 = phi i32 [ 0, %n0 ], [ %n24, %n3 ]
  %n5 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n4
  %n6 = trunc i32 %n4 to i16
  %n7 = mul nuw nsw i16 %n6, 3
  %n8 = add nsw i16 %n7, -90
  store i16 %n8, ptr %n5, align 8, !tbaa !8
  %n9 = or disjoint i32 %n4, 1
  %n10 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n9
  %n11 = trunc i32 %n9 to i16
  %n12 = mul nuw nsw i16 %n11, 3
  %n13 = add nsw i16 %n12, -90
  store i16 %n13, ptr %n10, align 2, !tbaa !8
  %n14 = or disjoint i32 %n4, 2
  %n15 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n14
  %n16 = trunc i32 %n14 to i16
  %n17 = mul nuw nsw i16 %n16, 3
  %n18 = add nsw i16 %n17, -90
  store i16 %n18, ptr %n15, align 4, !tbaa !8
  %n19 = or disjoint i32 %n4, 3
  %n20 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n19
  %n21 = trunc i32 %n19 to i16
  %n22 = mul nuw nsw i16 %n21, 3
  %n23 = add nsw i16 %n22, -90
  store i16 %n23, ptr %n20, align 2, !tbaa !8
  %n24 = add nuw nsw i32 %n4, 4
  %n25 = icmp eq i32 %n24, 64
  br i1 %n25, label %n26, label %n3, !llvm.loop !10

n26:
  br label %n27

n27:
  %n28 = phi i32 [ %n48, %n27 ], [ 0, %n26 ]
  %n29 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n28
  %n30 = trunc i32 %n28 to i16
  %n31 = shl nuw nsw i16 %n30, 1
  %n32 = sub nsw i16 70, %n31
  store i16 %n32, ptr %n29, align 8, !tbaa !8
  %n33 = or disjoint i32 %n28, 1
  %n34 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n33
  %n35 = trunc i32 %n33 to i16
  %n36 = shl nuw nsw i16 %n35, 1
  %n37 = sub nsw i16 70, %n36
  store i16 %n37, ptr %n34, align 2, !tbaa !8
  %n38 = or disjoint i32 %n28, 2
  %n39 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n38
  %n40 = trunc i32 %n38 to i16
  %n41 = shl nuw nsw i16 %n40, 1
  %n42 = sub nsw i16 70, %n41
  store i16 %n42, ptr %n39, align 4, !tbaa !8
  %n43 = or disjoint i32 %n28, 3
  %n44 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n43
  %n45 = trunc i32 %n43 to i16
  %n46 = shl nuw nsw i16 %n45, 1
  %n47 = sub nsw i16 70, %n46
  store i16 %n47, ptr %n44, align 2, !tbaa !8
  %n48 = add nuw nsw i32 %n28, 4
  %n49 = icmp eq i32 %n48, 48
  br i1 %n49, label %n50, label %n27, !llvm.loop !12

n50:
  br label %n51

n51:
  %n52 = phi i16 [ %n96, %n94 ], [ 0, %n50 ]
  %n53 = phi i32 [ %n95, %n94 ], [ 0, %n50 ]
  store i16 %n52, ptr %n1, align 16, !tbaa !8
  br label %n54

n54:
  %n55 = phi i32 [ 0, %n51 ], [ %n92, %n54 ]
  %n56 = phi i32 [ 0, %n51 ], [ %n91, %n54 ]
  %n92 = add nuw nsw i32 %n55, 4
  %n57 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n55
  %n58 = load i16, ptr %n57, align 8, !tbaa !8
  %n59 = sext i16 %n58 to i32
  %n60 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n55
  %n61 = load i16, ptr %n60, align 8, !tbaa !8
  %n62 = sext i16 %n61 to i32
  %n63 = mul nsw i32 %n62, %n59
  %n64 = add nsw i32 %n63, %n56
  %n65 = or disjoint i32 %n55, 1
  %n66 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n65
  %n67 = load i16, ptr %n66, align 2, !tbaa !8
  %n68 = sext i16 %n67 to i32
  %n69 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n65
  %n70 = load i16, ptr %n69, align 2, !tbaa !8
  %n71 = sext i16 %n70 to i32
  %n72 = mul nsw i32 %n71, %n68
  %n73 = add nsw i32 %n72, %n64
  %n74 = or disjoint i32 %n55, 2
  %n75 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n74
  %n76 = load i16, ptr %n75, align 4, !tbaa !8
  %n77 = sext i16 %n76 to i32
  %n78 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n74
  %n79 = load i16, ptr %n78, align 4, !tbaa !8
  %n80 = sext i16 %n79 to i32
  %n81 = mul nsw i32 %n80, %n77
  %n82 = add nsw i32 %n81, %n73
  %n83 = or disjoint i32 %n55, 3
  %n84 = getelementptr inbounds nuw [2 x i8], ptr %n1, i32 %n83
  %n85 = load i16, ptr %n84, align 2, !tbaa !8
  %n86 = sext i16 %n85 to i32
  %n87 = getelementptr inbounds nuw [2 x i8], ptr %n2, i32 %n83
  %n88 = load i16, ptr %n87, align 2, !tbaa !8
  %n89 = sext i16 %n88 to i32
  %n90 = mul nsw i32 %n89, %n86
  %n91 = add nsw i32 %n90, %n82
  %n93 = icmp eq i32 %n92, 48
  br i1 %n93, label %n94, label %n54, !llvm.loop !17

n94:
  %n95 = add nsw i32 %n91, %n53
  %n96 = add nuw nsw i16 %n52, 1
  %n97 = icmp samesign ult i16 %n52, 39
  br i1 %n97, label %n51, label %n98, !llvm.loop !13

n98:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %n2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %n1) nounwind
  tail call void @report(i32 noundef %n95) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext"

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"long", !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"short", !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 short", !16, i64 0}
!16 = !{!"any pointer", !4, i64 0}
!17 = distinct !{!17, !11}
