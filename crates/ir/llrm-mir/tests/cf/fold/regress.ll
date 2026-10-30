target datalayout = "e-p:64:64-i64:64"
@e = constant [0 x i32] []
@i1z = constant i1 zeroinitializer
@iz = constant i32 zeroinitializer
@fz = constant float zeroinitializer
@g = global i32 0
declare i16 @llvm.pext.i16(i16, i16)
declare i31 @llvm.clmul.i31(i31, i31)
declare i15 @llvm.umulh.i15(i15, i15)
declare i100 @llvm.smul.fix.i100(i100, i100, i32)
declare i127 @llvm.smul.fix.sat.i127(i127, i127, i32)
declare i8 @llvm.smul.fix.sat.i8(i8, i8, i32)

define void @r() {
  %a = load i32, ptr getelementptr inbounds (i8, ptr @e, i64 -4)
  %b = load i32, ptr @i1z
  %c = load i32, ptr @iz
  %d = load float, ptr @fz
  %e = load i8, ptr getelementptr inbounds (i8, ptr @iz, i64 1)
  %f = call i16 @llvm.pext.i16(i16 1, i16 poison)
  %h = call i31 @llvm.clmul.i31(i31 5, i31 poison)
  %i = call i15 @llvm.umulh.i15(i15 poison, i15 poison)
  %j = call i100 @llvm.smul.fix.i100(i100 -5, i100 77, i32 3)
  %k = call i127 @llvm.smul.fix.sat.i127(i127 85070591730234615865843651857942052863, i127 4, i32 2)
  %l = call i8 @llvm.smul.fix.sat.i8(i8 100, i8 100, i32 2)
  %m = getelementptr i8, ptr inttoptr (i64 4 to ptr), i64 -4
  %n = ptrtoint ptr null to i64
  %o = inttoptr i64 0 to ptr
  %p = icmp eq ptr inttoptr (i64 0 to ptr), null
  %q = and i64 ptrtoint (ptr null to i64), 7
  ret void
}
