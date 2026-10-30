target datalayout = "e-p:32:32"
; RUN: opt < %s -disable-output -da-delinearize=false "-passes=print<da>" -da-enable-dependence-test=all     \
; RUN: -aa-pipeline=basic-aa 2>&1 | FileCheck %s
; RUN: opt < %s -disable-output -da-delinearize=false -passes='print<da><normalized-results>' -da-enable-dependence-test=all     \
; RUN: -aa-pipeline=basic-aa 2>&1 | FileCheck %s -check-prefix=NORMALIZE
; RUN: opt < %s -disable-output "-passes=print<da>" -aa-pipeline=basic-aa 2>&1 -da-enable-dependence-test=all \
; RUN: | FileCheck %s -check-prefix=DELIN





;;  for (long int i = 1; i <= 10; i++)
;;    for (long int j = 1; j <= 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j - 1];

define void @banerjee0(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc7
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc7 ]
  %i.03 = phi i32 [ 1, %entry ], [ %inc8, %for.inc7 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 1, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %sub = add nsw i32 %add5, -1
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %sub
  %0 = load i32, ptr %arrayidx6, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 11
  br i1 %exitcond, label %for.body3, label %for.inc7

for.inc7:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc8 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc8, 11
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end9

for.end9:                                         ; preds = %for.inc7
  ret void
}


;;  for (long int i = 1; i <= n; i++)
;;    for (long int j = 1; j <= m; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j - 1];

define void @banerjee1(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  %cmp4 = icmp sgt i32 %n, 0
  br i1 %cmp4, label %for.cond1.preheader.preheader, label %for.end9

for.cond1.preheader.preheader:                    ; preds = %entry
  %0 = add i32 %n, 1
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.cond1.preheader.preheader, %for.inc7
  %B.addr.06 = phi ptr [ %B.addr.1.lcssa, %for.inc7 ], [ %B, %for.cond1.preheader.preheader ]
  %i.05 = phi i32 [ %inc8, %for.inc7 ], [ 1, %for.cond1.preheader.preheader ]
  %1 = add i32 %m, 1
  %cmp21 = icmp sgt i32 %m, 0
  br i1 %cmp21, label %for.body3.preheader, label %for.inc7

for.body3.preheader:                              ; preds = %for.cond1.preheader
  br label %for.body3

for.body3:                                        ; preds = %for.body3.preheader, %for.body3
  %j.03 = phi i32 [ %inc, %for.body3 ], [ 1, %for.body3.preheader ]
  %B.addr.12 = phi ptr [ %incdec.ptr, %for.body3 ], [ %B.addr.06, %for.body3.preheader ]
  %mul = mul nsw i32 %i.05, 10
  %add = add nsw i32 %mul, %j.03
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.05, 10
  %add5 = add nsw i32 %mul4, %j.03
  %sub = add nsw i32 %add5, -1
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %sub
  %2 = load i32, ptr %arrayidx6, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.12, i32 1
  store i32 %2, ptr %B.addr.12, align 8
  %inc = add nsw i32 %j.03, 1
  %exitcond = icmp eq i32 %inc, %1
  br i1 %exitcond, label %for.inc7.loopexit, label %for.body3

for.inc7.loopexit:                                ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.06, i32 %m
  br label %for.inc7

for.inc7:                                         ; preds = %for.inc7.loopexit, %for.cond1.preheader
  %B.addr.1.lcssa = phi ptr [ %B.addr.06, %for.cond1.preheader ], [ %scevgep, %for.inc7.loopexit ]
  %inc8 = add nsw i32 %i.05, 1
  %exitcond7 = icmp eq i32 %inc8, %0
  br i1 %exitcond7, label %for.end9.loopexit, label %for.cond1.preheader

for.end9.loopexit:                                ; preds = %for.inc7
  br label %for.end9

for.end9:                                         ; preds = %for.end9.loopexit, %entry
  ret void
}


;;  for (long int i = 0; i < 10; i++)
;;    for (long int j = 0; j < 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j + 100];

define void @banerjee2(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc8
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc8 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc9, %for.inc8 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %add6 = add nsw i32 %add5, 100
  %arrayidx7 = getelementptr inbounds i32, ptr %A, i32 %add6
  %0 = load i32, ptr %arrayidx7, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 10
  br i1 %exitcond, label %for.body3, label %for.inc8

for.inc8:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc9 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc9, 10
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end10

for.end10:                                        ; preds = %for.inc8
  ret void
}


;;  for (long int i = 0; i < 10; i++)
;;    for (long int j = 0; j < 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j + 99];

define void @banerjee3(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc8
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc8 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc9, %for.inc8 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %add6 = add nsw i32 %add5, 99
  %arrayidx7 = getelementptr inbounds i32, ptr %A, i32 %add6
  %0 = load i32, ptr %arrayidx7, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 10
  br i1 %exitcond, label %for.body3, label %for.inc8

for.inc8:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc9 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc9, 10
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end10

for.end10:                                        ; preds = %for.inc8
  ret void
}


;;  for (long int i = 0; i < 10; i++)
;;    for (long int j = 0; j < 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j - 100];

define void @banerjee4(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc7
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc7 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc8, %for.inc7 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %sub = add nsw i32 %add5, -100
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %sub
  %0 = load i32, ptr %arrayidx6, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 10
  br i1 %exitcond, label %for.body3, label %for.inc7

for.inc7:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc8 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc8, 10
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end9

for.end9:                                         ; preds = %for.inc7
  ret void
}


;;  for (long int i = 0; i < 10; i++)
;;    for (long int j = 0; j < 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j - 99];

define void @banerjee5(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc7
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc7 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc8, %for.inc7 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %sub = add nsw i32 %add5, -99
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %sub
  %0 = load i32, ptr %arrayidx6, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 10
  br i1 %exitcond, label %for.body3, label %for.inc7

for.inc7:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc8 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc8, 10
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end9

for.end9:                                         ; preds = %for.inc7
  ret void
}


;;  for (long int i = 0; i < 10; i++)
;;    for (long int j = 0; j < 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j + 9];

define void @banerjee6(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc8
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc8 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc9, %for.inc8 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %add6 = add nsw i32 %add5, 9
  %arrayidx7 = getelementptr inbounds i32, ptr %A, i32 %add6
  %0 = load i32, ptr %arrayidx7, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 10
  br i1 %exitcond, label %for.body3, label %for.inc8

for.inc8:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc9 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc9, 10
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end10

for.end10:                                        ; preds = %for.inc8
  ret void
}


;;  for (long int i = 0; i < 10; i++)
;;    for (long int j = 0; j < 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j + 10];

define void @banerjee7(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc8
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc8 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc9, %for.inc8 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %add6 = add nsw i32 %add5, 10
  %arrayidx7 = getelementptr inbounds i32, ptr %A, i32 %add6
  %0 = load i32, ptr %arrayidx7, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 10
  br i1 %exitcond, label %for.body3, label %for.inc8

for.inc8:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc9 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc9, 10
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end10

for.end10:                                        ; preds = %for.inc8
  ret void
}


;;  for (long int i = 0; i < 10; i++)
;;    for (long int j = 0; j < 10; j++) {
;;      A[10*i + j] = 0;
;;      *B++ = A[10*i + j + 11];

define void @banerjee8(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc8
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc8 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc9, %for.inc8 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 10
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 10
  %add5 = add nsw i32 %mul4, %j.02
  %add6 = add nsw i32 %add5, 11
  %arrayidx7 = getelementptr inbounds i32, ptr %A, i32 %add6
  %0 = load i32, ptr %arrayidx7, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 10
  br i1 %exitcond, label %for.body3, label %for.inc8

for.inc8:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 10
  %inc9 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc9, 10
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end10

for.end10:                                        ; preds = %for.inc8
  ret void
}


;;  for (long int i = 0; i < 20; i++)
;;    for (long int j = 0; j < 20; j++) {
;;      A[30*i + 500*j] = 0;
;;      *B++ = A[i - 500*j + 11];

define void @banerjee9(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc8
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc8 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc9, %for.inc8 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 30
  %mul4 = mul nsw i32 %j.02, 500
  %add = add nsw i32 %mul, %mul4
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %0 = mul i32 %j.02, -500
  %sub = add i32 %i.03, %0
  %add6 = add nsw i32 %sub, 11
  %arrayidx7 = getelementptr inbounds i32, ptr %A, i32 %add6
  %1 = load i32, ptr %arrayidx7, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %1, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 20
  br i1 %exitcond, label %for.body3, label %for.inc8

for.inc8:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 20
  %inc9 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc9, 20
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end10

for.end10:                                        ; preds = %for.inc8
  ret void
}


;;  for (long int i = 0; i < 20; i++)
;;    for (long int j = 0; j < 20; j++) {
;;      A[i + 500*j] = 0;
;;      *B++ = A[i - 500*j + 11];

define void @banerjee10(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc7
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc7 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc8, %for.inc7 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %j.02, 500
  %add = add nsw i32 %i.03, %mul
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %0 = mul i32 %j.02, -500
  %sub = add i32 %i.03, %0
  %add5 = add nsw i32 %sub, 11
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %add5
  %1 = load i32, ptr %arrayidx6, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %1, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 20
  br i1 %exitcond, label %for.body3, label %for.inc7

for.inc7:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 20
  %inc8 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc8, 20
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end9

for.end9:                                         ; preds = %for.inc7
  ret void
}


;;  for (long int i = 0; i < 20; i++)
;;    for (long int j = 0; j < 20; j++) {
;;      A[300*i + j] = 0;
;;      *B++ = A[250*i - j + 11];

define void @banerjee11(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc7
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc7 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc8, %for.inc7 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 300
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 250
  %sub = sub nsw i32 %mul4, %j.02
  %add5 = add nsw i32 %sub, 11
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %add5
  %0 = load i32, ptr %arrayidx6, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 20
  br i1 %exitcond, label %for.body3, label %for.inc7

for.inc7:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 20
  %inc8 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc8, 20
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end9

for.end9:                                         ; preds = %for.inc7
  ret void
}


;;  for (long int i = 0; i < 20; i++)
;;    for (long int j = 0; j < 20; j++) {
;;      A[100*i + j] = 0;
;;      *B++ = A[100*i - j + 11];

define void @banerjee12(ptr %A, ptr %B, i32 %m, i32 %n) nounwind uwtable ssp {
;
;
;
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %entry, %for.inc7
  %B.addr.04 = phi ptr [ %B, %entry ], [ %scevgep, %for.inc7 ]
  %i.03 = phi i32 [ 0, %entry ], [ %inc8, %for.inc7 ]
  br label %for.body3

for.body3:                                        ; preds = %for.cond1.preheader, %for.body3
  %j.02 = phi i32 [ 0, %for.cond1.preheader ], [ %inc, %for.body3 ]
  %B.addr.11 = phi ptr [ %B.addr.04, %for.cond1.preheader ], [ %incdec.ptr, %for.body3 ]
  %mul = mul nsw i32 %i.03, 100
  %add = add nsw i32 %mul, %j.02
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %add
  store i32 0, ptr %arrayidx, align 8
  %mul4 = mul nsw i32 %i.03, 100
  %sub = sub nsw i32 %mul4, %j.02
  %add5 = add nsw i32 %sub, 11
  %arrayidx6 = getelementptr inbounds i32, ptr %A, i32 %add5
  %0 = load i32, ptr %arrayidx6, align 8
  %incdec.ptr = getelementptr inbounds i32, ptr %B.addr.11, i32 1
  store i32 %0, ptr %B.addr.11, align 8
  %inc = add nsw i32 %j.02, 1
  %exitcond = icmp ne i32 %inc, 20
  br i1 %exitcond, label %for.body3, label %for.inc7

for.inc7:                                         ; preds = %for.body3
  %scevgep = getelementptr i32, ptr %B.addr.04, i32 20
  %inc8 = add nsw i32 %i.03, 1
  %exitcond5 = icmp ne i32 %inc8, 20
  br i1 %exitcond5, label %for.cond1.preheader, label %for.end9

for.end9:                                         ; preds = %for.inc7
  ret void
}