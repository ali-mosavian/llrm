; RUN: llrm-mir %s
; CHECK: define {{.*}} @_draw_string(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

declare fastcc i16 @_font_bit(ptr addrspace(1), i8, i16, i16)
declare cc1000 i16 @QGLSFPSET(i32, i16, i16, i16) addrspace(1)

define void @_draw_string(ptr addrspace(1) nocapture readonly %0, i32 %1, i16 %2, i16 %3, ptr nocapture readonly %4, i32 %5) addrspace(1) memory(readwrite, argmem: read) {
b1:
  %6 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 306
  %7 = trunc i32 %5 to i16
  br label %b2

b2:
  %lsr.iv2 = phi ptr [ %4, %b1 ], [ %lsr.iv.next2, %b6 ]
  %8 = phi i16 [ %2, %b1 ], [ %11, %b6 ]
  %9 = load i8, ptr %lsr.iv2, !tbaa !7
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b4, label %b3

b3:
  ret void

b4:
  br label %b7

b6:
  %11 = add nsw i16 %8, 4
  %lsr.iv.next2 = getelementptr i8, ptr %lsr.iv2, i16 1
  br label %b2

b7:
  %12 = phi i16 [ 0, %b4 ], [ %13, %b9 ]
  %lsr.iv11 = phi i16 [ %3, %b4 ], [ %lsr.iv.next1, %b9 ]
  br label %b10

b9:
  %13 = add nsw i16 %12, 1
  %lsr.iv.next1 = add i16 %lsr.iv11, 1
  %14 = icmp ne i16 %13, 8
  br i1 %14, label %b7, label %b6

b10:
  %15 = phi i16 [ 0, %b7 ], [ %18, %b11 ]
  %lsr.iv3 = phi i16 [ %8, %b7 ], [ %lsr.iv.next, %b11 ]
  %16 = call fastcc i16 @_font_bit(ptr addrspace(1) %6, i8 zeroext %9, i16 %15, i16 %12)
  %17 = icmp ne i16 %16, 0
  br i1 %17, label %b12, label %b11

b11:
  %18 = add nsw i16 %15, 1
  %lsr.iv.next = add i16 %lsr.iv3, 1
  %19 = icmp ne i16 %18, 8
  br i1 %19, label %b10, label %b9

b12:
  %20 = call cc1000 addrspace(1) i16 @QGLSFPSET(i32 %1, i16 %lsr.iv3, i16 %lsr.iv11, i16 %7)
  br label %b11
}

!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
