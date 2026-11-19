@fretop = external global i16
@strend = external global i16
@facmoh = external global i8
declare void @error(i8) noreturn
declare void @garba2()
declare i8 @conint()
define void @f() {
entry:
  %v1 = tail call i8 @conint()
  %v4 = load i16, ptr @fretop
  %v5 = add i16 %v4, -1
  %v6 = load i16, ptr @strend
  %v7 = icmp ult i16 %v5, %v6
  br i1 %v7, label %b8, label %b14
b8:
  tail call void @garba2()
  %v9 = load i16, ptr @fretop
  %v10 = add i16 %v9, -1
  %v11 = load i16, ptr @strend
  %v12 = icmp ult i16 %v10, %v11
  br i1 %v12, label %b13, label %b14
b13:
  ret void
b14:
  %v15 = phi i16 [ %v5, %entry ], [ %v10, %b8 ]
  %v16 = trunc i16 %v15 to i8
  %v17 = lshr i16 %v15, 8
  %v18 = trunc i16 %v17 to i8
  store i8 %v18, ptr @facmoh
  %v19 = inttoptr i16 %v15 to ptr
  store i8 %v1, ptr %v19
  ret void
}
