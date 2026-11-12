target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@sum = internal global i32 0, align 2
@state = internal global i32 -1831433054, align 2

; Function Attrs: noinline nounwind optnone
define dso_local i32 @k() #0 {
entry:
  %call = call i32 @c_ushort_3()
  ret i32 %call
}

; Function Attrs: noinline nounwind optnone
define internal i32 @c_ushort_3() #0 {
entry:
  %dv = alloca i16, align 2
  %x = alloca i16, align 2
  %q = alloca i16, align 2
  %r = alloca i16, align 2
  %bad = alloca i32, align 2
  %i = alloca i32, align 2
  %w = alloca i32, align 2
  %shape = alloca i32, align 2
  %hi = alloca i16, align 2
  store volatile i16 3, ptr %dv, align 2
  store i32 0, ptr %bad, align 2
  store i32 0, ptr %i, align 2
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i32, ptr %i, align 2
  %cmp = icmp slt i32 %0, 1500
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %call = call i32 @next()
  store i32 %call, ptr %w, align 2
  %call1 = call i32 @next()
  %and = and i32 %call1, 7
  store i32 %and, ptr %shape, align 2
  %1 = load i32, ptr %shape, align 2
  %cmp2 = icmp eq i32 %1, 0
  br i1 %cmp2, label %cond.true, label %cond.false

cond.true:                                        ; preds = %for.body
  %2 = load i32, ptr %w, align 2
  br label %cond.end18

cond.false:                                       ; preds = %for.body
  %3 = load i32, ptr %shape, align 2
  %cmp3 = icmp eq i32 %3, 1
  br i1 %cmp3, label %cond.true4, label %cond.false5

cond.true4:                                       ; preds = %cond.false
  %4 = load i32, ptr %w, align 2
  %shr = lshr i32 %4, 16
  br label %cond.end16

cond.false5:                                      ; preds = %cond.false
  %5 = load i32, ptr %shape, align 2
  %cmp6 = icmp eq i32 %5, 2
  br i1 %cmp6, label %cond.true7, label %cond.false9

cond.true7:                                       ; preds = %cond.false5
  %6 = load i32, ptr %w, align 2
  %shr8 = lshr i32 %6, 24
  br label %cond.end14

cond.false9:                                      ; preds = %cond.false5
  %7 = load i32, ptr %shape, align 2
  %cmp10 = icmp eq i32 %7, 3
  br i1 %cmp10, label %cond.true11, label %cond.false13

cond.true11:                                      ; preds = %cond.false9
  %8 = load i32, ptr %w, align 2
  %shr12 = lshr i32 %8, 29
  br label %cond.end

cond.false13:                                     ; preds = %cond.false9
  %9 = load i32, ptr %w, align 2
  br label %cond.end

cond.end:                                         ; preds = %cond.false13, %cond.true11
  %cond = phi i32 [ %shr12, %cond.true11 ], [ %9, %cond.false13 ]
  br label %cond.end14

cond.end14:                                       ; preds = %cond.end, %cond.true7
  %cond15 = phi i32 [ %shr8, %cond.true7 ], [ %cond, %cond.end ]
  br label %cond.end16

cond.end16:                                       ; preds = %cond.end14, %cond.true4
  %cond17 = phi i32 [ %shr, %cond.true4 ], [ %cond15, %cond.end14 ]
  br label %cond.end18

cond.end18:                                       ; preds = %cond.end16, %cond.true
  %cond19 = phi i32 [ %2, %cond.true ], [ %cond17, %cond.end16 ]
  %conv = trunc i32 %cond19 to i16
  store i16 %conv, ptr %x, align 2
  %10 = load i16, ptr %x, align 2
  %conv20 = zext i16 %10 to i32
  %div = sdiv i32 %conv20, 3
  %conv21 = trunc i32 %div to i16
  store i16 %conv21, ptr %q, align 2
  %11 = load i16, ptr %x, align 2
  %conv22 = zext i16 %11 to i32
  %rem = srem i32 %conv22, 3
  %conv23 = trunc i32 %rem to i16
  store i16 %conv23, ptr %r, align 2
  %12 = load i16, ptr %q, align 2
  %conv24 = zext i16 %12 to i32
  %13 = load i16, ptr %x, align 2
  %conv25 = zext i16 %13 to i32
  %14 = load volatile i16, ptr %dv, align 2
  %conv26 = zext i16 %14 to i32
  %div27 = sdiv i32 %conv25, %conv26
  %cmp28 = icmp ne i32 %conv24, %div27
  br i1 %cmp28, label %if.then, label %lor.lhs.false

lor.lhs.false:                                    ; preds = %cond.end18
  %15 = load i16, ptr %r, align 2
  %conv30 = zext i16 %15 to i32
  %16 = load i16, ptr %x, align 2
  %conv31 = zext i16 %16 to i32
  %17 = load volatile i16, ptr %dv, align 2
  %conv32 = zext i16 %17 to i32
  %rem33 = srem i32 %conv31, %conv32
  %cmp34 = icmp ne i32 %conv30, %rem33
  br i1 %cmp34, label %if.then, label %if.end

if.then:                                          ; preds = %lor.lhs.false, %cond.end18
  %18 = load i32, ptr %bad, align 2
  %inc = add nsw i32 %18, 1
  store i32 %inc, ptr %bad, align 2
  br label %if.end

if.end:                                           ; preds = %if.then, %lor.lhs.false
  %19 = load i16, ptr %q, align 2
  %conv36 = zext i16 %19 to i32
  %mul = mul i32 %conv36, 31
  %20 = load i16, ptr %r, align 2
  %conv37 = zext i16 %20 to i32
  %add = add i32 %mul, %conv37
  %21 = load i32, ptr @sum, align 2
  %add38 = add i32 %21, %add
  store i32 %add38, ptr @sum, align 2
  br label %for.inc

for.inc:                                          ; preds = %if.end
  %22 = load i32, ptr %i, align 2
  %inc39 = add nsw i32 %22, 1
  store i32 %inc39, ptr %i, align 2
  br label %for.cond, !llvm.loop !4

for.end:                                          ; preds = %for.cond
  store i32 0, ptr %i, align 2
  br label %for.cond40

for.cond40:                                       ; preds = %for.inc123, %for.end
  %23 = load i32, ptr %i, align 2
  %cmp41 = icmp slt i32 %23, 12
  br i1 %cmp41, label %for.body43, label %for.end125

for.body43:                                       ; preds = %for.cond40
  store i16 -1, ptr %hi, align 2
  %24 = load i32, ptr %i, align 2
  %cmp44 = icmp slt i32 %24, 2
  br i1 %cmp44, label %cond.true46, label %cond.false47

cond.true46:                                      ; preds = %for.body43
  %25 = load i32, ptr %i, align 2
  br label %cond.end93

cond.false47:                                     ; preds = %for.body43
  %26 = load i32, ptr %i, align 2
  %cmp48 = icmp slt i32 %26, 4
  br i1 %cmp48, label %cond.true50, label %cond.false53

cond.true50:                                      ; preds = %cond.false47
  %27 = load i16, ptr %hi, align 2
  %conv51 = zext i16 %27 to i32
  %28 = load i32, ptr %i, align 2
  %sub = sub nsw i32 %28, 2
  %sub52 = sub nsw i32 %conv51, %sub
  br label %cond.end91

cond.false53:                                     ; preds = %cond.false47
  %29 = load i32, ptr %i, align 2
  %cmp54 = icmp slt i32 %29, 6
  br i1 %cmp54, label %cond.true56, label %cond.false59

cond.true56:                                      ; preds = %cond.false53
  %30 = load i32, ptr %i, align 2
  %sub57 = sub nsw i32 %30, 5
  %add58 = add nsw i32 3, %sub57
  br label %cond.end89

cond.false59:                                     ; preds = %cond.false53
  %31 = load i32, ptr %i, align 2
  %cmp60 = icmp slt i32 %31, 8
  br i1 %cmp60, label %cond.true62, label %cond.false70

cond.true62:                                      ; preds = %cond.false59
  %32 = load i16, ptr %hi, align 2
  %conv63 = zext i16 %32 to i32
  %div64 = sdiv i32 %conv63, 3
  %mul65 = mul nsw i32 %div64, 3
  %conv66 = trunc i32 %mul65 to i16
  %conv67 = zext i16 %conv66 to i32
  %33 = load i32, ptr %i, align 2
  %sub68 = sub nsw i32 %33, 6
  %sub69 = sub nsw i32 %conv67, %sub68
  br label %cond.end87

cond.false70:                                     ; preds = %cond.false59
  %34 = load i32, ptr %i, align 2
  %cmp71 = icmp slt i32 %34, 10
  br i1 %cmp71, label %cond.true73, label %cond.false80

cond.true73:                                      ; preds = %cond.false70
  %35 = load i16, ptr %hi, align 2
  %conv74 = zext i16 %35 to i32
  %sub75 = sub nsw i32 %conv74, 3
  %36 = load i32, ptr %i, align 2
  %sub76 = sub nsw i32 %36, 8
  %add77 = add nsw i32 %sub75, %sub76
  %conv78 = trunc i32 %add77 to i16
  %conv79 = zext i16 %conv78 to i32
  br label %cond.end85

cond.false80:                                     ; preds = %cond.false70
  %37 = load i32, ptr %i, align 2
  %sub81 = sub nsw i32 %37, 10
  %sub82 = sub nsw i32 6, %sub81
  %conv83 = trunc i32 %sub82 to i16
  %conv84 = zext i16 %conv83 to i32
  br label %cond.end85

cond.end85:                                       ; preds = %cond.false80, %cond.true73
  %cond86 = phi i32 [ %conv79, %cond.true73 ], [ %conv84, %cond.false80 ]
  br label %cond.end87

cond.end87:                                       ; preds = %cond.end85, %cond.true62
  %cond88 = phi i32 [ %sub69, %cond.true62 ], [ %cond86, %cond.end85 ]
  br label %cond.end89

cond.end89:                                       ; preds = %cond.end87, %cond.true56
  %cond90 = phi i32 [ %add58, %cond.true56 ], [ %cond88, %cond.end87 ]
  br label %cond.end91

cond.end91:                                       ; preds = %cond.end89, %cond.true50
  %cond92 = phi i32 [ %sub52, %cond.true50 ], [ %cond90, %cond.end89 ]
  br label %cond.end93

cond.end93:                                       ; preds = %cond.end91, %cond.true46
  %cond94 = phi i32 [ %25, %cond.true46 ], [ %cond92, %cond.end91 ]
  %conv95 = trunc i32 %cond94 to i16
  store i16 %conv95, ptr %x, align 2
  %38 = load i16, ptr %x, align 2
  %conv96 = zext i16 %38 to i32
  %div97 = sdiv i32 %conv96, 3
  %conv98 = trunc i32 %div97 to i16
  store i16 %conv98, ptr %q, align 2
  %39 = load i16, ptr %x, align 2
  %conv99 = zext i16 %39 to i32
  %rem100 = srem i32 %conv99, 3
  %conv101 = trunc i32 %rem100 to i16
  store i16 %conv101, ptr %r, align 2
  %40 = load i16, ptr %q, align 2
  %conv102 = zext i16 %40 to i32
  %41 = load i16, ptr %x, align 2
  %conv103 = zext i16 %41 to i32
  %42 = load volatile i16, ptr %dv, align 2
  %conv104 = zext i16 %42 to i32
  %div105 = sdiv i32 %conv103, %conv104
  %cmp106 = icmp ne i32 %conv102, %div105
  br i1 %cmp106, label %if.then115, label %lor.lhs.false108

lor.lhs.false108:                                 ; preds = %cond.end93
  %43 = load i16, ptr %r, align 2
  %conv109 = zext i16 %43 to i32
  %44 = load i16, ptr %x, align 2
  %conv110 = zext i16 %44 to i32
  %45 = load volatile i16, ptr %dv, align 2
  %conv111 = zext i16 %45 to i32
  %rem112 = srem i32 %conv110, %conv111
  %cmp113 = icmp ne i32 %conv109, %rem112
  br i1 %cmp113, label %if.then115, label %if.end117

if.then115:                                       ; preds = %lor.lhs.false108, %cond.end93
  %46 = load i32, ptr %bad, align 2
  %inc116 = add nsw i32 %46, 1
  store i32 %inc116, ptr %bad, align 2
  br label %if.end117

if.end117:                                        ; preds = %if.then115, %lor.lhs.false108
  %47 = load i16, ptr %q, align 2
  %conv118 = zext i16 %47 to i32
  %mul119 = mul i32 %conv118, 31
  %48 = load i16, ptr %r, align 2
  %conv120 = zext i16 %48 to i32
  %add121 = add i32 %mul119, %conv120
  %49 = load i32, ptr @sum, align 2
  %add122 = add i32 %49, %add121
  store i32 %add122, ptr @sum, align 2
  br label %for.inc123

for.inc123:                                       ; preds = %if.end117
  %50 = load i32, ptr %i, align 2
  %inc124 = add nsw i32 %50, 1
  store i32 %inc124, ptr %i, align 2
  br label %for.cond40, !llvm.loop !6

for.end125:                                       ; preds = %for.cond40
  %51 = load i32, ptr %bad, align 2
  ret i32 %51
}

; Function Attrs: noinline nounwind optnone
define internal i32 @next() #0 {
entry:
  %0 = load i32, ptr @state, align 2
  %shl = shl i32 %0, 13
  %1 = load i32, ptr @state, align 2
  %xor = xor i32 %1, %shl
  store i32 %xor, ptr @state, align 2
  %2 = load i32, ptr @state, align 2
  %shr = lshr i32 %2, 17
  %3 = load i32, ptr @state, align 2
  %xor1 = xor i32 %3, %shr
  store i32 %xor1, ptr @state, align 2
  %4 = load i32, ptr @state, align 2
  %shl2 = shl i32 %4, 5
  %5 = load i32, ptr @state, align 2
  %xor3 = xor i32 %5, %shl2
  store i32 %xor3, ptr @state, align 2
  %6 = load i32, ptr @state, align 2
  ret i32 %6
}

attributes #0 = { noinline nounwind optnone "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git 7e3542bb5338616657772790c7499f26077216b8)"}
!4 = distinct !{!4, !5}
!5 = !{!"llvm.loop.mustprogress"}
!6 = distinct !{!6, !5}
