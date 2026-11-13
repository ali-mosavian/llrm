target datalayout = "@m64-datalayout@"
target triple = "x86_64-unknown-linux-gnu"
define dso_local i32 @f1(i32 noundef %a, i32 noundef %b) local_unnamed_addr {
entry:
  %ea = alloca i32, align 4
  %eb = alloca i32, align 4
  %xor = xor i32 %b, %a
  %and = and i32 %xor, -2147483648
  %shl.mask = and i32 %a, 2147483647
  %cmp = icmp eq i32 %shl.mask, 0
  %shl1.mask = and i32 %b, 2147483647
  %cmp2 = icmp eq i32 %shl1.mask, 0
  %or.cond = or i1 %cmp, %cmp2
  br i1 %or.cond, label %cleanup, label %if.end

if.end:                                           ; preds = %entry
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %ea)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %eb)
  %call = call fastcc i32 @unpack(i32 noundef %a, ptr noundef %ea)
  %call3 = call fastcc i32 @unpack(i32 noundef %b, ptr noundef %eb)
  %tobool.not = icmp samesign ult i32 %call, 8388608
  br i1 %tobool.not, label %if.then5, label %if.end8

if.then5:                                         ; preds = %if.end
  %0 = tail call range(i32 9, 33) i32 @llvm.ctlz.i32(i32 %call, i1 true)
  %sub = add nsw i32 %0, -8
  %shl6 = shl i32 %call, %sub
  %1 = load i32, ptr %ea, align 4
  %sub7 = sub nsw i32 %1, %sub
  store i32 %sub7, ptr %ea, align 4
  br label %if.end8

if.end8:                                          ; preds = %if.then5, %if.end
  %ma.0 = phi i32 [ %call, %if.end ], [ %shl6, %if.then5 ]
  %tobool10.not = icmp samesign ult i32 %call3, 8388608
  br i1 %tobool10.not, label %if.then11, label %if.end16

if.then11:                                        ; preds = %if.end8
  %2 = tail call range(i32 9, 33) i32 @llvm.ctlz.i32(i32 %call3, i1 true)
  %sub13 = add nsw i32 %2, -8
  %shl14 = shl i32 %call3, %sub13
  %3 = load i32, ptr %eb, align 4
  %sub15 = sub nsw i32 %3, %sub13
  store i32 %sub15, ptr %eb, align 4
  br label %if.end16

if.end16:                                         ; preds = %if.then11, %if.end8
  %mb.0 = phi i32 [ %call3, %if.end8 ], [ %shl14, %if.then11 ]
  %4 = load i32, ptr %ea, align 4
  %5 = load i32, ptr %eb, align 4
  %call17 = tail call i32 @core_a(i32 noundef %and, i32 noundef %4, i32 noundef %5, i32 noundef %ma.0, i32 noundef %mb.0)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %eb)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %ea)
  br label %cleanup

cleanup:                                          ; preds = %entry, %if.end16
  %retval.0 = phi i32 [ %call17, %if.end16 ], [ %and, %entry ]
  ret i32 %retval.0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture)

define internal fastcc range(i32 0, 16777216) i32 @unpack(i32 noundef %a, ptr nocapture noundef nonnull writeonly initializes((0, 4)) %e) unnamed_addr {
entry:
  %and = and i32 %a, 8388607
  %shr = lshr i32 %a, 23
  %and1 = and i32 %shr, 255
  %tobool.not = icmp eq i32 %and1, 0
  %or = or disjoint i32 %and, 8388608
  %m.0 = select i1 %tobool.not, i32 %and, i32 %or
  %x.0 = tail call i32 @llvm.umax.i32(i32 %and1, i32 1)
  store i32 %x.0, ptr %e, align 4
  ret i32 %m.0
}

declare i32 @llvm.ctlz.i32(i32, i1 immarg)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture)

declare i32 @core_a(i32 noundef, i32 noundef, i32 noundef, i32 noundef, i32 noundef) local_unnamed_addr

define dso_local i32 @f2(i32 noundef %a, i32 noundef %b) local_unnamed_addr {
entry:
  %ea = alloca i32, align 4
  %eb = alloca i32, align 4
  %xor = xor i32 %b, %a
  %and = and i32 %xor, -2147483648
  %shl.mask = and i32 %a, 2147483647
  %cmp = icmp eq i32 %shl.mask, 0
  %shl1.mask = and i32 %b, 2147483647
  %cmp2 = icmp eq i32 %shl1.mask, 0
  %or.cond = or i1 %cmp, %cmp2
  br i1 %or.cond, label %cleanup, label %if.end

if.end:                                           ; preds = %entry
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %ea)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %eb)
  %call = call fastcc i32 @unpack(i32 noundef %a, ptr noundef %ea)
  %call3 = call fastcc i32 @unpack(i32 noundef %b, ptr noundef %eb)
  %tobool.not = icmp samesign ult i32 %call, 8388608
  br i1 %tobool.not, label %if.then5, label %if.end8

if.then5:                                         ; preds = %if.end
  %0 = tail call range(i32 9, 33) i32 @llvm.ctlz.i32(i32 %call, i1 true)
  %sub = add nsw i32 %0, -8
  %shl6 = shl i32 %call, %sub
  %1 = load i32, ptr %ea, align 4
  %sub7 = sub nsw i32 %1, %sub
  store i32 %sub7, ptr %ea, align 4
  br label %if.end8

if.end8:                                          ; preds = %if.then5, %if.end
  %ma.0 = phi i32 [ %call, %if.end ], [ %shl6, %if.then5 ]
  %tobool10.not = icmp samesign ult i32 %call3, 8388608
  br i1 %tobool10.not, label %if.then11, label %if.end16

if.then11:                                        ; preds = %if.end8
  %2 = tail call range(i32 9, 33) i32 @llvm.ctlz.i32(i32 %call3, i1 true)
  %sub13 = add nsw i32 %2, -8
  %shl14 = shl i32 %call3, %sub13
  %3 = load i32, ptr %eb, align 4
  %sub15 = sub nsw i32 %3, %sub13
  store i32 %sub15, ptr %eb, align 4
  br label %if.end16

if.end16:                                         ; preds = %if.then11, %if.end8
  %mb.0 = phi i32 [ %call3, %if.end8 ], [ %shl14, %if.then11 ]
  %4 = load i32, ptr %ea, align 4
  %5 = load i32, ptr %eb, align 4
  %call17 = tail call i32 @core_b(i32 noundef %and, i32 noundef %4, i32 noundef %5, i32 noundef %ma.0, i32 noundef %mb.0)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %eb)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %ea)
  br label %cleanup

cleanup:                                          ; preds = %entry, %if.end16
  %retval.0 = phi i32 [ %call17, %if.end16 ], [ %and, %entry ]
  ret i32 %retval.0
}

declare i32 @core_b(i32 noundef, i32 noundef, i32 noundef, i32 noundef, i32 noundef) local_unnamed_addr

define dso_local i32 @f3(i32 noundef %a, i32 noundef %b) local_unnamed_addr {
entry:
  %ea = alloca i32, align 4
  %eb = alloca i32, align 4
  %xor = xor i32 %b, %a
  %and = and i32 %xor, -2147483648
  %shl.mask = and i32 %a, 2147483647
  %cmp = icmp eq i32 %shl.mask, 0
  %shl1.mask = and i32 %b, 2147483647
  %cmp2 = icmp eq i32 %shl1.mask, 0
  %or.cond = or i1 %cmp, %cmp2
  br i1 %or.cond, label %cleanup, label %if.end

if.end:                                           ; preds = %entry
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %ea)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %eb)
  %call = call fastcc i32 @unpack(i32 noundef %a, ptr noundef %ea)
  %call3 = call fastcc i32 @unpack(i32 noundef %b, ptr noundef %eb)
  %tobool.not = icmp samesign ult i32 %call, 8388608
  br i1 %tobool.not, label %if.then5, label %if.end8

if.then5:                                         ; preds = %if.end
  %0 = tail call range(i32 9, 33) i32 @llvm.ctlz.i32(i32 %call, i1 true)
  %sub = add nsw i32 %0, -8
  %shl6 = shl i32 %call, %sub
  %1 = load i32, ptr %ea, align 4
  %sub7 = sub nsw i32 %1, %sub
  store i32 %sub7, ptr %ea, align 4
  br label %if.end8

if.end8:                                          ; preds = %if.then5, %if.end
  %ma.0 = phi i32 [ %call, %if.end ], [ %shl6, %if.then5 ]
  %tobool10.not = icmp samesign ult i32 %call3, 8388608
  br i1 %tobool10.not, label %if.then11, label %if.end16

if.then11:                                        ; preds = %if.end8
  %2 = tail call range(i32 9, 33) i32 @llvm.ctlz.i32(i32 %call3, i1 true)
  %sub13 = add nsw i32 %2, -8
  %shl14 = shl i32 %call3, %sub13
  %3 = load i32, ptr %eb, align 4
  %sub15 = sub nsw i32 %3, %sub13
  store i32 %sub15, ptr %eb, align 4
  br label %if.end16

if.end16:                                         ; preds = %if.then11, %if.end8
  %mb.0 = phi i32 [ %call3, %if.end8 ], [ %shl14, %if.then11 ]
  %4 = load i32, ptr %ea, align 4
  %5 = load i32, ptr %eb, align 4
  %call17 = tail call i32 @core_c(i32 noundef %and, i32 noundef %4, i32 noundef %5, i32 noundef %ma.0, i32 noundef %mb.0)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %eb)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %ea)
  br label %cleanup

cleanup:                                          ; preds = %entry, %if.end16
  %retval.0 = phi i32 [ %call17, %if.end16 ], [ %and, %entry ]
  ret i32 %retval.0
}

declare i32 @core_c(i32 noundef, i32 noundef, i32 noundef, i32 noundef, i32 noundef) local_unnamed_addr

declare i32 @llvm.umax.i32(i32, i32)



