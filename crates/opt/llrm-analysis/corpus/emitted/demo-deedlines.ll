target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [32 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"R%" = internal global [18 x i8] zeroinitializer
@"G%" = internal global [18 x i8] zeroinitializer
@"B%" = internal global [18 x i8] zeroinitializer
@"R1%" = internal global [18 x i8] zeroinitializer
@"G1%" = internal global [18 x i8] zeroinitializer
@"B1%" = internal global [18 x i8] zeroinitializer
@"FPOS%" = internal global [18 x i8] zeroinitializer
@"FONTS%" = internal global [18 x i8] zeroinitializer
@"FSIN4%" = internal global [1844 x i8] zeroinitializer, align 2
@"FSIN4%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FSIN4%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"FSIN4%", i16 638), [6 x i8] c"\02\00\9A\03\C1\FE" }>
@"FSIN1%" = internal global [2264 x i8] zeroinitializer, align 2
@"FSIN1%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FSIN1%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"FSIN1%", i16 96), [6 x i8] c"\02\00l\04\D0\FF" }>
@"FSIN2%" = internal global [3196 x i8] zeroinitializer, align 2
@"FSIN2%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FSIN2%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"FSIN2%", i16 1280), [6 x i8] c"\02\00>\06\80\FD" }>
@"FSIN3%" = internal global [3024 x i8] zeroinitializer, align 2
@"FSIN3%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FSIN3%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"FSIN3%", i16 1280), [6 x i8] c"\02\00\E8\05\80\FD" }>
@"MOD256128%" = internal global [674 x i8] zeroinitializer, align 2
@"MOD256128%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"MOD256128%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"MOD256128%", i16 336), [6 x i8] c"\02\00Q\01X\FF" }>
@"SP%" = internal global [32770 x i8] zeroinitializer, align 2
@"SP%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"SP%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"SP%", [6 x i8] c"\02\00\01@\00\00" }>
@"DT%" = internal global [746 x i8] zeroinitializer, align 2
@"DT%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DT%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"DT%", i16 384), [6 x i8] c"\02\00u\01@\FF" }>
@"DT100%" = internal global [722 x i8] zeroinitializer, align 2
@"DT100%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DT100%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"DT100%", i16 360), [6 x i8] c"\02\00i\01L\FF" }>
@"CD%" = internal global [3586 x i8] zeroinitializer, align 2
@"CD%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"CD%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"CD%", [6 x i8] c"\02\00\01\07\00\00" }>
@"DSX%" = internal global [22 x i8] zeroinitializer, align 2
@"DSX%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DSX%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"DSX%", [6 x i8] c"\02\00\0B\00\00\00" }>
@"DSY%" = internal global [22 x i8] zeroinitializer, align 2
@"DSY%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DSY%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"DSY%", [6 x i8] c"\02\00\0B\00\00\00" }>
@"PSX%" = internal global [22 x i8] zeroinitializer, align 2
@"PSX%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"PSX%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"PSX%", [6 x i8] c"\02\00\0B\00\00\00" }>
@"PSY%" = internal global [22 x i8] zeroinitializer, align 2
@"PSY%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"PSY%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"PSY%", [6 x i8] c"\02\00\0B\00\00\00" }>
@"XP1%" = internal global [22 x i8] zeroinitializer, align 2
@"XP1%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"XP1%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"XP1%", [6 x i8] c"\02\00\0B\00\00\00" }>
@"YP1%" = internal global [22 x i8] zeroinitializer, align 2
@"YP1%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"YP1%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"YP1%", [6 x i8] c"\02\00\0B\00\00\00" }>
@"YY%" = internal global [22 x i8] zeroinitializer, align 2
@"YY%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"YY%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"YY%", [6 x i8] c"\02\00\0B\00\00\00" }>
@"EPI6%" = internal global [12 x i8] zeroinitializer, align 2
@"EPI6%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"EPI6%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"EPI6%", [6 x i8] c"\02\00\06\00\00\00" }>
@"EPI36%" = internal global [12 x i8] zeroinitializer, align 2
@"EPI36%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"EPI36%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"EPI36%", [6 x i8] c"\02\00\06\00\00\00" }>
@"CY%" = internal global [802 x i8] zeroinitializer, align 2
@"CY%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"CY%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"CY%", i16 200), [6 x i8] c"\02\00\91\01\9C\FF" }>
@"DIS%" = internal global [642 x i8] zeroinitializer, align 2
@"DIS%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DIS%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"DIS%", [6 x i8] c"\02\00A\01\00\00" }>
@"SLA%" = internal global [642 x i8] zeroinitializer, align 2
@"SLA%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"SLA%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"SLA%", [6 x i8] c"\02\00A\01\00\00" }>
@"SLB%" = internal global [642 x i8] zeroinitializer, align 2
@"SLB%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"SLB%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"SLB%", [6 x i8] c"\02\00A\01\00\00" }>
@"SLC%" = internal global [642 x i8] zeroinitializer, align 2
@"SLC%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"SLC%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"SLC%", [6 x i8] c"\02\00A\01\00\00" }>
@"SC%" = internal global [642 x i8] zeroinitializer, align 2
@"SC%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"SC%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"SC%", [6 x i8] c"\02\00A\01\00\00" }>
@"DIV2%" = internal global [512 x i8] zeroinitializer, align 2
@"DIV2%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DIV2%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"DIV2%", [6 x i8] c"\02\00\00\01\00\00" }>
@"DIV4%" = internal global [642 x i8] zeroinitializer, align 2
@"DIV4%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DIV4%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"DIV4%", [6 x i8] c"\02\00A\01\00\00" }>
@"MUL32%" = internal global [66 x i8] zeroinitializer, align 2
@"MUL32%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"MUL32%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"MUL32%", [6 x i8] c"\02\00!\00\00\00" }>
@"MUL128%" = internal global [258 x i8] zeroinitializer, align 2
@"MUL128%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"MUL128%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"MUL128%", [6 x i8] c"\02\00\81\00\00\00" }>
@"FS%" = internal global [400 x i8] zeroinitializer, align 2
@"FS%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FS%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"FS%", [6 x i8] c"\02\00\C8\00\00\00" }>
@"KON%" = internal global [256 x i8] zeroinitializer, align 2
@"KON%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"KON%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"KON%", [6 x i8] c"\02\00\80\00\00\00" }>
@"KA%" = internal global [10 x i8] zeroinitializer, align 2
@"KA%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"KA%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"KA%", [6 x i8] c"\02\00\05\00\00\00" }>
@TEXT$ = internal global [22 x i8] zeroinitializer
@"X!" = internal global [18 x i8] zeroinitializer
@"Y!" = internal global [18 x i8] zeroinitializer
@"Z!" = internal global [18 x i8] zeroinitializer
@"XO%" = internal global [18 x i8] zeroinitializer
@"YO%" = internal global [18 x i8] zeroinitializer
@"ZO%" = internal global [18 x i8] zeroinitializer
@"XS%" = internal global [22 x i8] zeroinitializer
@"YS%" = internal global [22 x i8] zeroinitializer
@"YPK&" = internal global [18 x i8] zeroinitializer
@"ZPO%" = internal global [18 x i8] zeroinitializer
@$arguments$85 = internal global [242 x i8] zeroinitializer
@$string35 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string35, i16 4), [8 x i8] c"-COPPER\00" }>
@"XIT%" = internal global [2 x i8] zeroinitializer
@"FP%" = internal global [2 x i8] zeroinitializer
@A$ = internal global [4 x i8] zeroinitializer
@$string36 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string36, i16 4), [12 x i8] c"fonts16.fnt\00" }>
@C$ = internal global [4 x i8] zeroinitializer
@$string37 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string37, i16 4), [2 x i8] c" \00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@$string38 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string38, i16 4), [20 x i8] c"   Ok! Let's get    " }>
@$string39 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string39, i16 4), [20 x i8] c"  serious now :)    " }>
@$string40 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string40, i16 4), [20 x i8] c"                    " }>
@$string41 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string41, i16 4), [20 x i8] c" Hopefully you will " }>
@$string42 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string42, i16 4), [20 x i8] c"see a nice demo from" }>
@$string43 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string43, i16 4), [20 x i8] c"my side and not old " }>
@$string44 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string44, i16 4), [20 x i8] c"  prehistoric line  " }>
@$string45 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string45, i16 4), [20 x i8] c" drawing and lame   " }>
@$string46 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string46, i16 4), [20 x i8] c"oldstyle qbasic code" }>
@$string47 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string47, i16 4), [20 x i8] c"                    " }>
@$string48 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string48, i16 4), [20 x i8] c" But let's crosfade " }>
@$string49 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string49, i16 4), [20 x i8] c" some more text now!" }>
@$string50 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string50, i16 4), [20 x i8] c"  Optimus presents  " }>
@$string51 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string51, i16 4), [20 x i8] c"  a demo done in a  " }>
@$string52 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string52, i16 4), [20 x i8] c" hurry by connecting" }>
@$string53 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string53, i16 4), [20 x i8] c"  various older or  " }>
@$string54 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string54, i16 4), [20 x i8] c" newer sources into " }>
@$string55 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string55, i16 4), [20 x i8] c" the main programm. " }>
@$string56 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string56, i16 4), [20 x i8] c"                    " }>
@$string57 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string57, i16 4), [20 x i8] c" All coded in pure  " }>
@$string58 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string58, i16 4), [20 x i8] c"  Quickbasic 4.5    " }>
@$string59 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string59, i16 4), [20 x i8] c"  Use ffix.com for  " }>
@$string60 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string60, i16 4), [20 x i8] c"  maximum pleasure  " }>
@$string61 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string61, i16 4), [20 x i8] c"                    " }>
@$string62 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string62, i16 4), [20 x i8] c"  Featuring effects " }>
@$string63 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string63, i16 4), [20 x i8] c"like Sphere mapping " }>
@$string64 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string64, i16 4), [20 x i8] c"circle blobs,rgb8bpp" }>
@$string65 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string65, i16 4), [20 x i8] c"lights, zoom distort" }>
@$string66 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string66, i16 4), [20 x i8] c"plasma inside blob, " }>
@$string67 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string67, i16 4), [20 x i8] c"hardware raster fx  " }>
@$string68 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string68, i16 4), [20 x i8] c"3d dots and a simple" }>
@$string69 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string69, i16 4), [20 x i8] c"raycaster engine.   " }>
@$string70 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string70, i16 4), [20 x i8] c"    Some are not so " }>
@$string71 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string71, i16 4), [20 x i8] c"optimized as I could" }>
@$string72 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string72, i16 4), [20 x i8] c"and few have several" }>
@$string73 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string73, i16 4), [20 x i8] c"bugs too..          " }>
@$string74 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string74, i16 4), [20 x i8] c"They still run at   " }>
@$string75 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string75, i16 4), [20 x i8] c"full frame rate in  " }>
@$string76 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string76, i16 4), [20 x i8] c"my AMDK6-2/500Mhz,  " }>
@$string77 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string77, i16 4), [20 x i8] c"though I had thought" }>
@$string78 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string78, i16 4), [20 x i8] c"ways to optimize    " }>
@$string79 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string79, i16 4), [20 x i8] c"them even more.     " }>
@$string80 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string80, i16 4), [20 x i8] c"           Anyways.." }>
@$string81 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string81, i16 4), [20 x i8] c"lean back and enjoy!" }>
@$string82 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string82, i16 4), [20 x i8] c"                    " }>
@$string83 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string83, i16 4), [20 x i8] c" Optimus/Dirty Minds" }>
@$string84 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string84, i16 4), [20 x i8] c" Thessaloniki/Greece" }>
@$string85 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string85, i16 4), [20 x i8] c" Saturday 25/05/2002" }>
@"DL%" = internal global [2 x i8] zeroinitializer
@$float86 = internal constant [4 x i8] c"\00\00\00O"
@"I&" = internal global [4 x i8] zeroinitializer
@$string87 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string87, i16 4), [12 x i8] c"wolfmap0.wad" }>
@K$ = internal global [4 x i8] zeroinitializer
@$string88 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string88, i16 4), [2 x i8] c" \00" }>
@$string89 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string89, i16 4), [10 x i8] c"3rdrec.rec" }>
@GI$ = internal global [4 x i8] zeroinitializer
@$string90 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string90, i16 4), [2 x i8] c" \00" }>
@GG$ = internal global [4 x i8] zeroinitializer
@$string91 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string91, i16 4), [2 x i8] c" \00" }>
@"G%.1" = internal global [2 x i8] zeroinitializer
@"GI%" = internal global [2 x i8] zeroinitializer
@"GG%" = internal global [2 x i8] zeroinitializer
@$string92 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string92, i16 4) }>
@"PH%" = internal global [2 x i8] zeroinitializer
@"PX%" = internal global [2 x i8] zeroinitializer
@"PY%" = internal global [2 x i8] zeroinitializer
@"PA%" = internal global [2 x i8] zeroinitializer
@"PS%" = internal global [2 x i8] zeroinitializer
@"FV%" = internal global [2 x i8] zeroinitializer
@"RA!" = internal global [4 x i8] zeroinitializer
@"DPP%" = internal global [2 x i8] zeroinitializer
@$float93 = internal constant [4 x i8] c"33eB"
@"MAP%" = internal global [2 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@"FPS%" = internal global [2 x i8] zeroinitializer
@"FILEI%" = internal global [2 x i8] zeroinitializer
@$string94 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string94, i16 4) }>
@"KB%" = internal global [2 x i8] zeroinitializer
@"NDTS%" = internal global [2 x i8] zeroinitializer
@"NDTSO%" = internal global [2 x i8] zeroinitializer
@"XC!" = internal global [4 x i8] zeroinitializer
@"YC!" = internal global [4 x i8] zeroinitializer
@"ZC!" = internal global [4 x i8] zeroinitializer
@"RXC!" = internal global [4 x i8] zeroinitializer
@"RYC!" = internal global [4 x i8] zeroinitializer
@"RZC!" = internal global [4 x i8] zeroinitializer
@"XP!" = internal global [4 x i8] zeroinitializer
@"YP!" = internal global [4 x i8] zeroinitializer
@"ZP!" = internal global [4 x i8] zeroinitializer
@"OBJ%" = internal global [2 x i8] zeroinitializer
@"FTYPE%" = internal global [2 x i8] zeroinitializer
@"MTR!" = internal global [4 x i8] zeroinitializer
@"VA%" = internal global [2 x i8] zeroinitializer
@"GGI%" = internal global [2 x i8] zeroinitializer
@"NLNS%" = internal global [2 x i8] zeroinitializer
@"NPLS%" = internal global [2 x i8] zeroinitializer
@$string95 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string95, i16 4) }>
@$float96 = internal constant [8 x i8] c"\1795\9DF\DF\91?"
@$string97 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string97, i16 4), [2 x i8] c" \00" }>
@$string98 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string98, i16 4), [2 x i8] c" \00" }>
@b$seg = external global [2 x i8]
@$string100 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string100, i16 4), [10 x i8] c"3drec.001\00" }>
@$string101 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string101, i16 4), [2 x i8] c"x\00" }>
@$string102 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string102, i16 4) }>
@$string103 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string103, i16 4), [2 x i8] c"s\00" }>
@$string104 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string104, i16 4), [2 x i8] c"S\00" }>
@$string105 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string105, i16 4) }>
@$string106 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string106, i16 4), [2 x i8] c"d\00" }>
@$string107 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string107, i16 4), [2 x i8] c"D\00" }>
@$string108 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string108, i16 4) }>
@$string109 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string109, i16 4), [2 x i8] c"f\00" }>
@$string110 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string110, i16 4), [2 x i8] c"F\00" }>
@$string111 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string111, i16 4) }>
@$string112 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string112, i16 4), [2 x i8] c"g\00" }>
@$string113 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string113, i16 4), [2 x i8] c"G\00" }>
@$string114 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string114, i16 4) }>
@$string115 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string115, i16 4), [2 x i8] c"h\00" }>
@$string116 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string116, i16 4), [2 x i8] c"H\00" }>
@$string117 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string117, i16 4), [12 x i8] c"hiteapot.3do" }>
@$string118 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string118, i16 4) }>
@$string119 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string119, i16 4), [2 x i8] c"j\00" }>
@$string120 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string120, i16 4), [2 x i8] c"J\00" }>
@$string121 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string121, i16 4), [8 x i8] c"cow.3do\00" }>
@$string122 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string122, i16 4) }>
@$string123 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string123, i16 4), [2 x i8] c",\00" }>
@$string124 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string124, i16 4), [2 x i8] c"<\00" }>
@$string125 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string125, i16 4) }>
@$string126 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string126, i16 4), [2 x i8] c".\00" }>
@$string127 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string127, i16 4), [2 x i8] c">\00" }>
@$string128 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string128, i16 4) }>
@$string129 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string129, i16 4), [2 x i8] c"k\00" }>
@$string130 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string130, i16 4), [2 x i8] c"K\00" }>
@$float131 = internal constant [4 x i8] c"\00\00\C0?"
@$string132 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string132, i16 4) }>
@$string133 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string133, i16 4), [2 x i8] c"l\00" }>
@$string134 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string134, i16 4), [2 x i8] c"L\00" }>
@$string135 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string135, i16 4) }>
@$string136 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string136, i16 4), [2 x i8] c"u\00" }>
@$string137 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string137, i16 4), [2 x i8] c"U\00" }>
@$string138 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string138, i16 4) }>
@$string139 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string139, i16 4), [2 x i8] c"y\00" }>
@$string140 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string140, i16 4), [2 x i8] c"Y\00" }>
@$string141 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string141, i16 4) }>
@$string142 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string142, i16 4), [2 x i8] c"m\00" }>
@$string143 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string143, i16 4), [2 x i8] c"M\00" }>
@$string144 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string144, i16 4) }>
@$string145 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string145, i16 4), [2 x i8] c"a\00" }>
@$string146 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string146, i16 4), [2 x i8] c"A\00" }>
@$float147 = internal constant [4 x i8] c"\CD\CC\CC="
@$string148 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string148, i16 4) }>
@$string149 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string149, i16 4), [2 x i8] c"Q\00" }>
@$string150 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string150, i16 4), [2 x i8] c"q\00" }>
@$string151 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string151, i16 4) }>
@$string152 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string152, i16 4), [2 x i8] c"o\00" }>
@$string153 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string153, i16 4), [2 x i8] c"O\00" }>
@$string154 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string154, i16 4) }>
@$string155 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string155, i16 4), [2 x i8] c"p\00" }>
@$string156 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string156, i16 4), [2 x i8] c"P\00" }>
@$string157 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string157, i16 4) }>
@$string158 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string158, i16 4), [2 x i8] c"-\00" }>
@$float159 = internal constant [4 x i8] c"\00\00\80>"
@$string160 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string160, i16 4) }>
@$string161 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string161, i16 4), [2 x i8] c"=\00" }>
@$string162 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string162, i16 4), [2 x i8] c"+\00" }>
@$string163 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string163, i16 4) }>
@$string164 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string164, i16 4), [2 x i8] c"z\00" }>
@$string165 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string165, i16 4), [2 x i8] c"Z\00" }>
@$string166 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string166, i16 4) }>
@$string167 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string167, i16 4), [2 x i8] c"x\00" }>
@$string168 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string168, i16 4), [2 x i8] c"X\00" }>
@$string169 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string169, i16 4), [2 x i8] c"4\00" }>
@$float170 = internal constant [4 x i8] c"\0A\D7\A3;"
@$string171 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string171, i16 4) }>
@$string172 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string172, i16 4), [2 x i8] c"6\00" }>
@$string173 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string173, i16 4) }>
@$string174 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string174, i16 4), [2 x i8] c"8\00" }>
@$string175 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string175, i16 4) }>
@$string176 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string176, i16 4), [2 x i8] c"2\00" }>
@$string177 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string177, i16 4) }>
@$string178 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string178, i16 4), [2 x i8] c"7\00" }>
@$string179 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string179, i16 4) }>
@$string180 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string180, i16 4), [2 x i8] c"9\00" }>
@$string181 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string181, i16 4) }>
@$string182 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string182, i16 4), [2 x i8] c"w\00" }>
@$string183 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string183, i16 4), [2 x i8] c"W\00" }>
@$string184 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string184, i16 4) }>
@$string185 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string185, i16 4), [2 x i8] c"e\00" }>
@$string186 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string186, i16 4), [2 x i8] c"E\00" }>
@$string187 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string187, i16 4) }>
@$string188 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string188, i16 4), [2 x i8] c"r\00" }>
@$string189 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string189, i16 4), [2 x i8] c"R\00" }>
@$string190 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string190, i16 4) }>
@$string191 = internal constant <{ [2 x i8], ptr, [16 x i8] }> <{ [2 x i8] c"\10\00", ptr getelementptr (i8, ptr @$string191, i16 4), [16 x i8] c"Congratulations!" }>
@$string192 = internal constant <{ [2 x i8], ptr, [78 x i8] }> <{ [2 x i8] c"M\00", ptr getelementptr (i8, ptr @$string192, i16 4), [78 x i8] c"You have just found the secret part of the demo, even if I guess it was quite\00" }>
@$string193 = internal constant <{ [2 x i8], ptr, [78 x i8] }> <{ [2 x i8] c"M\00", ptr getelementptr (i8, ptr @$string193, i16 4), [78 x i8] c"easy, since I use to give away the source of every quickbasic demo of mine ;)\00" }>
@$string194 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"P\00", ptr getelementptr (i8, ptr @$string194, i16 4), [80 x i8] c"   These are supposed to be hardware fx inspired by Amiga, C64, CPC or any other" }>
@$string195 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"P\00", ptr getelementptr (i8, ptr @$string195, i16 4), [80 x i8] c"raster display computer that happens to exist. Quite unstable if you are running" }>
@$string196 = internal constant <{ [2 x i8], ptr, [74 x i8] }> <{ [2 x i8] c"I\00", ptr getelementptr (i8, ptr @$string196, i16 4), [74 x i8] c"them under windows, so I suggest to boot up in pure DOS and retry there..\00" }>
@$string197 = internal constant <{ [2 x i8], ptr, [78 x i8] }> <{ [2 x i8] c"N\00", ptr getelementptr (i8, ptr @$string197, i16 4), [78 x i8] c"   What you are just watching are called raster or copper bars. Originally 1st" }>
@$string198 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"O\00", ptr getelementptr (i8, ptr @$string198, i16 4), [80 x i8] c"seen in Amiga demos, giving to the machine the opportunity to display more than\00" }>
@$string199 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"P\00", ptr getelementptr (i8, ptr @$string199, i16 4), [80 x i8] c"the theoritical maximum number of colors (This is text mode!) in the screen (But" }>
@$string200 = internal constant <{ [2 x i8], ptr, [76 x i8] }> <{ [2 x i8] c"L\00", ptr getelementptr (i8, ptr @$string200, i16 4), [76 x i8] c"mostly used as horizontal colorfull lines and not per pixel. Do you remember" }>
@$string201 = internal constant <{ [2 x i8], ptr, [78 x i8] }> <{ [2 x i8] c"N\00", ptr getelementptr (i8, ptr @$string201, i16 4), [78 x i8] c"Shadow of the Beast or Agony, which used to have extra colors upon a colorfull" }>
@$string202 = internal constant <{ [2 x i8], ptr, [24 x i8] }> <{ [2 x i8] c"\17\00", ptr getelementptr (i8, ptr @$string202, i16 4), [24 x i8] c"sky in the background?)\00" }>
@$string203 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"P\00", ptr getelementptr (i8, ptr @$string203, i16 4), [80 x i8] c"   They are so simple fx, just changing the RGB values of just one color several" }>
@$string204 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"O\00", ptr getelementptr (i8, ptr @$string204, i16 4), [80 x i8] c"times in a frame, synced with the raster beam of the CRT (You can use the &HDA \00" }>
@$string205 = internal constant <{ [2 x i8], ptr, [80 x i8] }> <{ [2 x i8] c"O\00", ptr getelementptr (i8, ptr @$string205, i16 4), [80 x i8] c"port address you know from Vsync to use for horizontal syncing too, by checking\00" }>
@$string206 = internal constant <{ [2 x i8], ptr, [74 x i8] }> <{ [2 x i8] c"I\00", ptr getelementptr (i8, ptr @$string206, i16 4), [74 x i8] c" the 1st bit, in the similar way you did that for the 4th bit for Vsync!)\00" }>
@$string207 = internal constant <{ [2 x i8], ptr, [78 x i8] }> <{ [2 x i8] c"N\00", ptr getelementptr (i8, ptr @$string207, i16 4), [78 x i8] c"It seems that I have 2 go, unfortunatelly haven't explained you about the next" }>
@$string208 = internal constant <{ [2 x i8], ptr, [48 x i8] }> <{ [2 x i8] c"0\00", ptr getelementptr (i8, ptr @$string208, i16 4), [48 x i8] c"effects you are gonna see if you press any key.." }>
@$string209 = internal constant <{ [2 x i8], ptr, [60 x i8] }> <{ [2 x i8] c"<\00", ptr getelementptr (i8, ptr @$string209, i16 4), [60 x i8] c"                                                     Optimus" }>
@$string210 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string210, i16 4) }>
@$string211 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string211, i16 4) }>
@$float212 = internal constant [4 x i8] c"\CD\CC\CC@"
@$string213 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string213, i16 4) }>
@$string214 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string214, i16 4) }>
@$float215 = internal constant [8 x i8] c"\AA\9F\96\22\DC\A5L@"
@$float216 = internal constant [4 x i8] c"\00\004A"
@$float217 = internal constant [4 x i8] c"\00\00\B4A"
@$float218 = internal constant [4 x i8] zeroinitializer
@$float219 = internal constant [4 x i8] c"\00\FF\FFF"
@$float220 = internal constant [4 x i8] c"\00\00\80?"
@$string221 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string221, i16 4), [4 x i8] c"SAX!" }>
@$string222 = internal constant <{ [2 x i8], ptr, [22 x i8] }> <{ [2 x i8] c"\16\00", ptr getelementptr (i8, ptr @$string222, i16 4), [22 x i8] c"Not so serious side :)" }>
@$string223 = internal constant <{ [2 x i8], ptr, [40 x i8] }> <{ [2 x i8] c"(\00", ptr getelementptr (i8, ptr @$string223, i16 4), [40 x i8] c"Just for fun or boredom to draw anything" }>
@$float224 = internal constant [4 x i8] c"\CD\CC\00B"
@$float225 = internal constant [4 x i8] c"ff\B3C"
@$string226 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string226, i16 4), [2 x i8] c" \00" }>
@$string227 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string227, i16 4), [12 x i8] c"qbrules.spr\00" }>
@$string228 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string228, i16 4), [2 x i8] c" \00" }>
@$float229 = internal constant [4 x i8] c"ff\A6?"
@$float230 = internal constant [4 x i8] c"\0A\D7\A3>"
@$float231 = internal constant [4 x i8] c"\0A\D7#<"
@$string232 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string232, i16 4), [10 x i8] c"Press the\00" }>
@$string233 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string233, i16 4), [8 x i8] c"any key\00" }>
@$string234 = internal constant <{ [2 x i8], ptr, [26 x i8] }> <{ [2 x i8] c"\19\00", ptr getelementptr (i8, ptr @$string234, i16 4), [26 x i8] c" button to continue.. ;-)\00" }>
@$string235 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string235, i16 4) }>
@$string236 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string236, i16 4), [2 x i8] c"z\00" }>
@$float237 = internal constant [4 x i8] c"\CD\CC\0C@"
@$float238 = internal constant [4 x i8] c"\9A\99IA"
@$string239 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string239, i16 4), [12 x i8] c"sphrprec.dat" }>
@$string240 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string240, i16 4) }>
@$float241 = internal constant [4 x i8] c"\0A\D7\A3<"
@$float242 = internal constant [4 x i8] c"\CD\CCL?"
@$string243 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0E\00", ptr getelementptr (i8, ptr @$string243, i16 4), [14 x i8] c"Still Sucking?" }>
@$float244 = internal constant [4 x i8] c"333@"
@$string245 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string245, i16 4) }>
@$string246 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string246, i16 4), [12 x i8] c"qbinside.spr" }>
@$string247 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string247, i16 4), [2 x i8] c" \00" }>
@$string248 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string248, i16 4), [2 x i8] c"V\00" }>
@$string249 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string249, i16 4), [4 x i8] c".BIN" }>
@$string250 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string250, i16 4), [2 x i8] c"P\00" }>
@$string251 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string251, i16 4), [4 x i8] c".BIN" }>
@$string252 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string252, i16 4), [2 x i8] c"T\00" }>
@$string253 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string253, i16 4), [4 x i8] c".BIN" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 1, ptr @$arguments$85, !tbaa !2
  %0 = load i16, ptr @$arguments$85, !tbaa !2
  %1 = sitofp i16 %0 to double
  %2 = fpround double %1 to double
  %3 = getelementptr i8, ptr @$arguments$85, i16 2
  store double %2, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$85, i16 2
  %5 = load double, ptr %4, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$RNZP(double %5)
  %6 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %7 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %6, ptr @$string35)
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %b4, label %b5

b2:
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 0, i16 0)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 320, i16 200)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 255, i16 -1, i16 2)
  %9 = getelementptr i8, ptr @$arguments$85, i16 172
  store i16 1, ptr %9, !tbaa !2
  %10 = getelementptr i8, ptr @$arguments$85, i16 172
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %10)
  call cc 1000 addrspace(1) void @SPHEREMAPLASMA()
  %11 = getelementptr i8, ptr @$arguments$85, i16 174
  store i16 2, ptr %11, !tbaa !2
  %12 = getelementptr i8, ptr @$arguments$85, i16 174
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %12)
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @ZOOMDISTORT()
  %13 = getelementptr i8, ptr @$arguments$85, i16 176
  store i16 3, ptr %13, !tbaa !2
  %14 = getelementptr i8, ptr @$arguments$85, i16 176
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %14)
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %15 = getelementptr i8, ptr @$arguments$85, i16 178
  store i16 0, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$arguments$85, i16 178
  %17 = getelementptr i8, ptr @$arguments$85, i16 180
  store i16 255, ptr %17, !tbaa !2
  %18 = getelementptr i8, ptr @$arguments$85, i16 180
  %19 = getelementptr i8, ptr @$arguments$85, i16 182
  store i16 31, ptr %19, !tbaa !2
  %20 = getelementptr i8, ptr @$arguments$85, i16 182
  %21 = getelementptr i8, ptr @$arguments$85, i16 184
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr i8, ptr @$arguments$85, i16 184
  %23 = getelementptr i8, ptr @$arguments$85, i16 186
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$arguments$85, i16 186
  %25 = getelementptr i8, ptr @$arguments$85, i16 188
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr i8, ptr @$arguments$85, i16 188
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %16, ptr %18, ptr %20, ptr %22, ptr %24, ptr %26)
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  call cc 1000 addrspace(1) void @RGBLIGHTS()
  %27 = getelementptr i8, ptr @$arguments$85, i16 190
  store i16 4, ptr %27, !tbaa !2
  %28 = getelementptr i8, ptr @$arguments$85, i16 190
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %28)
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"X!")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"Y!")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"Z!")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr @"XO%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr @"YO%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr @"ZO%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 0, i16 1, i16 2, i16 258, ptr @"XS%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 0, i16 1, i16 2, i16 258, ptr @"YS%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -100, i16 99, i16 4, i16 257, ptr @"YPK&")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 8, i16 2, i16 257, ptr @"ZPO%")
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 0, ptr @"I%", !tbaa !2
  %29 = getelementptr i8, ptr @$data, i16 16
  store i16 826, ptr %29, !tbaa !2
  %30 = getelementptr i8, ptr @$data, i16 18
  store i16 1, ptr %30, !tbaa !2
  br label %b60

b3:
  %31 = getelementptr i8, ptr @$arguments$85, i16 228
  store i16 8, ptr %31, !tbaa !2
  %32 = getelementptr i8, ptr @$arguments$85, i16 228
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %32)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b4:
  call cc 1000 addrspace(1) void @COPPER()
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b5:
  br label %b6

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  %33 = getelementptr i8, ptr @$arguments$85, i16 10
  store i16 15, ptr %33, !tbaa !2
  %34 = getelementptr i8, ptr @$arguments$85, i16 10
  call cc 1000 addrspace(1) void @DEEDLINESAX(ptr %34)
  %35 = getelementptr i8, ptr @$arguments$85, i16 12
  store i16 140, ptr %35, !tbaa !2
  %36 = getelementptr i8, ptr @$arguments$85, i16 12
  call cc 1000 addrspace(1) void @DELAY(ptr %36)
  %37 = getelementptr i8, ptr @$arguments$85, i16 14
  store i16 0, ptr %37, !tbaa !2
  %38 = getelementptr i8, ptr @$arguments$85, i16 14
  call cc 1000 addrspace(1) void @DEEDLINESAX(ptr %38)
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %39 = getelementptr i8, ptr @$arguments$85, i16 16
  store i16 0, ptr %39, !tbaa !2
  %40 = getelementptr i8, ptr @$arguments$85, i16 16
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %40)
  call cc 1000 addrspace(1) void @SUCKING()
  %41 = getelementptr i8, ptr @$arguments$85, i16 18
  store i16 140, ptr %41, !tbaa !2
  %42 = getelementptr i8, ptr @$arguments$85, i16 18
  call cc 1000 addrspace(1) void @DELAY(ptr %42)
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"R%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"G%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"B%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"R1%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"G1%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"B1%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"FPOS%")
  store i16 22016, ptr @"FP%", !tbaa !2
  %43 = load i16, ptr @"FP%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 %43, i16 2, i16 257, ptr @"FONTS%")
  call cc 1000 addrspace(1) void @SETCROSFADEPAL(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  call cc 1000 addrspace(1) void @GETPAL()
  %44 = getelementptr i8, ptr @$arguments$85, i16 20
  store i16 0, ptr %44, !tbaa !2
  %45 = getelementptr i8, ptr @$arguments$85, i16 20
  %46 = getelementptr i8, ptr @$arguments$85, i16 22
  store i16 255, ptr %46, !tbaa !2
  %47 = getelementptr i8, ptr @$arguments$85, i16 22
  %48 = getelementptr i8, ptr @$arguments$85, i16 24
  store i16 1, ptr %48, !tbaa !2
  %49 = getelementptr i8, ptr @$arguments$85, i16 24
  %50 = getelementptr i8, ptr @$arguments$85, i16 26
  store i16 0, ptr %50, !tbaa !2
  %51 = getelementptr i8, ptr @$arguments$85, i16 26
  %52 = getelementptr i8, ptr @$arguments$85, i16 28
  store i16 0, ptr %52, !tbaa !2
  %53 = getelementptr i8, ptr @$arguments$85, i16 28
  %54 = getelementptr i8, ptr @$arguments$85, i16 30
  store i16 0, ptr %54, !tbaa !2
  %55 = getelementptr i8, ptr @$arguments$85, i16 30
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %45, ptr %47, ptr %49, ptr %51, ptr %53, ptr %55)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string36, ptr @A$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string37, ptr @C$)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @A$, i16 1, i16 -1, i16 32)
  store i16 0, ptr @"I%", !tbaa !2
  store i16 255, ptr @$data, !tbaa !2
  %56 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %56, !tbaa !2
  br label %b8

b8:
  %57 = getelementptr i8, ptr @$data, i16 2
  %58 = load i16, ptr %57, !tbaa !2
  %59 = icmp sge i16 %58, 0
  br i1 %59, label %b9, label %b10

b9:
  %60 = load i16, ptr @"I%", !tbaa !2
  %61 = load i16, ptr @$data, !tbaa !2
  %62 = icmp sle i16 %60, %61
  br i1 %62, label %b11, label %b12

b10:
  %63 = load i16, ptr @"I%", !tbaa !2
  %64 = load i16, ptr @$data, !tbaa !2
  %65 = icmp sge i16 %63, %64
  br i1 %65, label %b11, label %b12

b11:
  %66 = addrspacecast ptr @C$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %66, i16 0)
  %67 = load i16, ptr @"I%", !tbaa !2
  %68 = mul i16 %67, 2
  %69 = getelementptr i8, ptr @"FPOS%", i16 2
  %70 = load i16, ptr %69, !tbaa !2
  %71 = add i16 0, %68
  %72 = inttoptr i16 %70 to ptr addrspace(2)
  %73 = addrspacecast ptr addrspace(2) %72 to ptr addrspace(1)
  %74 = getelementptr i8, ptr addrspace(1) %73, i16 %71
  %75 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @C$)
  store i16 %75, ptr addrspace(1) %74, !tbaa !6
  %76 = load i16, ptr @"I%", !tbaa !2
  %77 = getelementptr i8, ptr @$data, i16 2
  %78 = load i16, ptr %77, !tbaa !2
  %79 = add nsw i16 %76, %78
  store i16 %79, ptr @"I%", !tbaa !2
  br label %b8

b12:
  store i16 0, ptr @"I%", !tbaa !2
  %80 = getelementptr i8, ptr @$data, i16 4
  store i16 85, ptr %80, !tbaa !2
  %81 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %81, !tbaa !2
  br label %b13

b13:
  %82 = getelementptr i8, ptr @$data, i16 6
  %83 = load i16, ptr %82, !tbaa !2
  %84 = icmp sge i16 %83, 0
  br i1 %84, label %b14, label %b15

b14:
  %85 = load i16, ptr @"I%", !tbaa !2
  %86 = getelementptr i8, ptr @$data, i16 4
  %87 = load i16, ptr %86, !tbaa !2
  %88 = icmp sle i16 %85, %87
  br i1 %88, label %b16, label %b17

b15:
  %89 = load i16, ptr @"I%", !tbaa !2
  %90 = getelementptr i8, ptr @$data, i16 4
  %91 = load i16, ptr %90, !tbaa !2
  %92 = icmp sge i16 %89, %91
  br i1 %92, label %b16, label %b17

b16:
  store i16 0, ptr @"Y%", !tbaa !2
  %93 = getelementptr i8, ptr @$data, i16 8
  store i16 15, ptr %93, !tbaa !2
  %94 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %94, !tbaa !2
  br label %b18

b17:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 3, i16 0, i16 11, i16 4, i16 -32766, ptr @TEXT$)
  %95 = getelementptr i8, ptr @TEXT$, i16 10
  %96 = load ptr, ptr %95, !tbaa !2
  %97 = getelementptr i8, ptr %96, i16 0
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string38, ptr %97)
  %98 = getelementptr i8, ptr @TEXT$, i16 10
  %99 = load ptr, ptr %98, !tbaa !2
  %100 = getelementptr i8, ptr %99, i16 16
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string39, ptr %100)
  %101 = getelementptr i8, ptr @TEXT$, i16 10
  %102 = load ptr, ptr %101, !tbaa !2
  %103 = getelementptr i8, ptr %102, i16 32
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string40, ptr %103)
  %104 = getelementptr i8, ptr @TEXT$, i16 10
  %105 = load ptr, ptr %104, !tbaa !2
  %106 = getelementptr i8, ptr %105, i16 48
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string41, ptr %106)
  %107 = getelementptr i8, ptr @TEXT$, i16 10
  %108 = load ptr, ptr %107, !tbaa !2
  %109 = getelementptr i8, ptr %108, i16 64
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string42, ptr %109)
  %110 = getelementptr i8, ptr @TEXT$, i16 10
  %111 = load ptr, ptr %110, !tbaa !2
  %112 = getelementptr i8, ptr %111, i16 80
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string43, ptr %112)
  %113 = getelementptr i8, ptr @TEXT$, i16 10
  %114 = load ptr, ptr %113, !tbaa !2
  %115 = getelementptr i8, ptr %114, i16 96
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string44, ptr %115)
  %116 = getelementptr i8, ptr @TEXT$, i16 10
  %117 = load ptr, ptr %116, !tbaa !2
  %118 = getelementptr i8, ptr %117, i16 112
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string45, ptr %118)
  %119 = getelementptr i8, ptr @TEXT$, i16 10
  %120 = load ptr, ptr %119, !tbaa !2
  %121 = getelementptr i8, ptr %120, i16 128
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string46, ptr %121)
  %122 = getelementptr i8, ptr @TEXT$, i16 10
  %123 = load ptr, ptr %122, !tbaa !2
  %124 = getelementptr i8, ptr %123, i16 144
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string47, ptr %124)
  %125 = getelementptr i8, ptr @TEXT$, i16 10
  %126 = load ptr, ptr %125, !tbaa !2
  %127 = getelementptr i8, ptr %126, i16 160
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string48, ptr %127)
  %128 = getelementptr i8, ptr @TEXT$, i16 10
  %129 = load ptr, ptr %128, !tbaa !2
  %130 = getelementptr i8, ptr %129, i16 176
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string49, ptr %130)
  %131 = getelementptr i8, ptr @TEXT$, i16 10
  %132 = load ptr, ptr %131, !tbaa !2
  %133 = getelementptr i8, ptr %132, i16 4
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string50, ptr %133)
  %134 = getelementptr i8, ptr @TEXT$, i16 10
  %135 = load ptr, ptr %134, !tbaa !2
  %136 = getelementptr i8, ptr %135, i16 20
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string51, ptr %136)
  %137 = getelementptr i8, ptr @TEXT$, i16 10
  %138 = load ptr, ptr %137, !tbaa !2
  %139 = getelementptr i8, ptr %138, i16 36
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string52, ptr %139)
  %140 = getelementptr i8, ptr @TEXT$, i16 10
  %141 = load ptr, ptr %140, !tbaa !2
  %142 = getelementptr i8, ptr %141, i16 52
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string53, ptr %142)
  %143 = getelementptr i8, ptr @TEXT$, i16 10
  %144 = load ptr, ptr %143, !tbaa !2
  %145 = getelementptr i8, ptr %144, i16 68
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string54, ptr %145)
  %146 = getelementptr i8, ptr @TEXT$, i16 10
  %147 = load ptr, ptr %146, !tbaa !2
  %148 = getelementptr i8, ptr %147, i16 84
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string55, ptr %148)
  %149 = getelementptr i8, ptr @TEXT$, i16 10
  %150 = load ptr, ptr %149, !tbaa !2
  %151 = getelementptr i8, ptr %150, i16 100
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string56, ptr %151)
  %152 = getelementptr i8, ptr @TEXT$, i16 10
  %153 = load ptr, ptr %152, !tbaa !2
  %154 = getelementptr i8, ptr %153, i16 116
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string57, ptr %154)
  %155 = getelementptr i8, ptr @TEXT$, i16 10
  %156 = load ptr, ptr %155, !tbaa !2
  %157 = getelementptr i8, ptr %156, i16 132
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string58, ptr %157)
  %158 = getelementptr i8, ptr @TEXT$, i16 10
  %159 = load ptr, ptr %158, !tbaa !2
  %160 = getelementptr i8, ptr %159, i16 148
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string59, ptr %160)
  %161 = getelementptr i8, ptr @TEXT$, i16 10
  %162 = load ptr, ptr %161, !tbaa !2
  %163 = getelementptr i8, ptr %162, i16 164
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string60, ptr %163)
  %164 = getelementptr i8, ptr @TEXT$, i16 10
  %165 = load ptr, ptr %164, !tbaa !2
  %166 = getelementptr i8, ptr %165, i16 180
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string61, ptr %166)
  %167 = getelementptr i8, ptr @TEXT$, i16 10
  %168 = load ptr, ptr %167, !tbaa !2
  %169 = getelementptr i8, ptr %168, i16 8
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string62, ptr %169)
  %170 = getelementptr i8, ptr @TEXT$, i16 10
  %171 = load ptr, ptr %170, !tbaa !2
  %172 = getelementptr i8, ptr %171, i16 24
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string63, ptr %172)
  %173 = getelementptr i8, ptr @TEXT$, i16 10
  %174 = load ptr, ptr %173, !tbaa !2
  %175 = getelementptr i8, ptr %174, i16 40
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string64, ptr %175)
  %176 = getelementptr i8, ptr @TEXT$, i16 10
  %177 = load ptr, ptr %176, !tbaa !2
  %178 = getelementptr i8, ptr %177, i16 56
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string65, ptr %178)
  %179 = getelementptr i8, ptr @TEXT$, i16 10
  %180 = load ptr, ptr %179, !tbaa !2
  %181 = getelementptr i8, ptr %180, i16 72
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string66, ptr %181)
  %182 = getelementptr i8, ptr @TEXT$, i16 10
  %183 = load ptr, ptr %182, !tbaa !2
  %184 = getelementptr i8, ptr %183, i16 88
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string67, ptr %184)
  %185 = getelementptr i8, ptr @TEXT$, i16 10
  %186 = load ptr, ptr %185, !tbaa !2
  %187 = getelementptr i8, ptr %186, i16 104
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string68, ptr %187)
  %188 = getelementptr i8, ptr @TEXT$, i16 10
  %189 = load ptr, ptr %188, !tbaa !2
  %190 = getelementptr i8, ptr %189, i16 120
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string69, ptr %190)
  %191 = getelementptr i8, ptr @TEXT$, i16 10
  %192 = load ptr, ptr %191, !tbaa !2
  %193 = getelementptr i8, ptr %192, i16 136
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string70, ptr %193)
  %194 = getelementptr i8, ptr @TEXT$, i16 10
  %195 = load ptr, ptr %194, !tbaa !2
  %196 = getelementptr i8, ptr %195, i16 152
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string71, ptr %196)
  %197 = getelementptr i8, ptr @TEXT$, i16 10
  %198 = load ptr, ptr %197, !tbaa !2
  %199 = getelementptr i8, ptr %198, i16 168
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string72, ptr %199)
  %200 = getelementptr i8, ptr @TEXT$, i16 10
  %201 = load ptr, ptr %200, !tbaa !2
  %202 = getelementptr i8, ptr %201, i16 184
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string73, ptr %202)
  %203 = getelementptr i8, ptr @TEXT$, i16 10
  %204 = load ptr, ptr %203, !tbaa !2
  %205 = getelementptr i8, ptr %204, i16 12
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string74, ptr %205)
  %206 = getelementptr i8, ptr @TEXT$, i16 10
  %207 = load ptr, ptr %206, !tbaa !2
  %208 = getelementptr i8, ptr %207, i16 28
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string75, ptr %208)
  %209 = getelementptr i8, ptr @TEXT$, i16 10
  %210 = load ptr, ptr %209, !tbaa !2
  %211 = getelementptr i8, ptr %210, i16 44
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string76, ptr %211)
  %212 = getelementptr i8, ptr @TEXT$, i16 10
  %213 = load ptr, ptr %212, !tbaa !2
  %214 = getelementptr i8, ptr %213, i16 60
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string77, ptr %214)
  %215 = getelementptr i8, ptr @TEXT$, i16 10
  %216 = load ptr, ptr %215, !tbaa !2
  %217 = getelementptr i8, ptr %216, i16 76
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string78, ptr %217)
  %218 = getelementptr i8, ptr @TEXT$, i16 10
  %219 = load ptr, ptr %218, !tbaa !2
  %220 = getelementptr i8, ptr %219, i16 92
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string79, ptr %220)
  %221 = getelementptr i8, ptr @TEXT$, i16 10
  %222 = load ptr, ptr %221, !tbaa !2
  %223 = getelementptr i8, ptr %222, i16 108
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string80, ptr %223)
  %224 = getelementptr i8, ptr @TEXT$, i16 10
  %225 = load ptr, ptr %224, !tbaa !2
  %226 = getelementptr i8, ptr %225, i16 124
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string81, ptr %226)
  %227 = getelementptr i8, ptr @TEXT$, i16 10
  %228 = load ptr, ptr %227, !tbaa !2
  %229 = getelementptr i8, ptr %228, i16 140
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string82, ptr %229)
  %230 = getelementptr i8, ptr @TEXT$, i16 10
  %231 = load ptr, ptr %230, !tbaa !2
  %232 = getelementptr i8, ptr %231, i16 156
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string83, ptr %232)
  %233 = getelementptr i8, ptr @TEXT$, i16 10
  %234 = load ptr, ptr %233, !tbaa !2
  %235 = getelementptr i8, ptr %234, i16 172
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string84, ptr %235)
  %236 = getelementptr i8, ptr @TEXT$, i16 10
  %237 = load ptr, ptr %236, !tbaa !2
  %238 = getelementptr i8, ptr %237, i16 188
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string85, ptr %238)
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 840, ptr @"DL%", !tbaa !2
  %239 = getelementptr i8, ptr @$arguments$85, i16 32
  store i16 0, ptr %239, !tbaa !2
  %240 = getelementptr i8, ptr @$arguments$85, i16 32
  %241 = getelementptr i8, ptr @$arguments$85, i16 34
  store i16 1, ptr %241, !tbaa !2
  %242 = getelementptr i8, ptr @$arguments$85, i16 34
  call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr %240, ptr %242)
  %243 = getelementptr i8, ptr @$arguments$85, i16 36
  store i16 0, ptr %243, !tbaa !2
  %244 = getelementptr i8, ptr @$arguments$85, i16 36
  %245 = getelementptr i8, ptr @$arguments$85, i16 38
  store i16 255, ptr %245, !tbaa !2
  %246 = getelementptr i8, ptr @$arguments$85, i16 38
  %247 = getelementptr i8, ptr @$arguments$85, i16 40
  store i16 256, ptr %247, !tbaa !2
  %248 = getelementptr i8, ptr @$arguments$85, i16 40
  %249 = getelementptr i8, ptr @$arguments$85, i16 42
  store i16 0, ptr %249, !tbaa !2
  %250 = getelementptr i8, ptr @$arguments$85, i16 42
  %251 = getelementptr i8, ptr @$arguments$85, i16 44
  store i16 0, ptr %251, !tbaa !2
  %252 = getelementptr i8, ptr @$arguments$85, i16 44
  %253 = getelementptr i8, ptr @$arguments$85, i16 46
  store i16 0, ptr %253, !tbaa !2
  %254 = getelementptr i8, ptr @$arguments$85, i16 46
  call cc 1000 addrspace(1) void @FADEFROMCOLOR(ptr %244, ptr %246, ptr %248, ptr %250, ptr %252, ptr %254)
  call cc 1000 addrspace(1) void @DELAY(ptr @"DL%")
  %255 = load i16, ptr @"XIT%", !tbaa !2
  %256 = icmp eq i16 %255, 1
  br i1 %256, label %b28, label %b29

b18:
  %257 = getelementptr i8, ptr @$data, i16 10
  %258 = load i16, ptr %257, !tbaa !2
  %259 = icmp sge i16 %258, 0
  br i1 %259, label %b19, label %b20

b19:
  %260 = load i16, ptr @"Y%", !tbaa !2
  %261 = getelementptr i8, ptr @$data, i16 8
  %262 = load i16, ptr %261, !tbaa !2
  %263 = icmp sle i16 %260, %262
  br i1 %263, label %b21, label %b22

b20:
  %264 = load i16, ptr @"Y%", !tbaa !2
  %265 = getelementptr i8, ptr @$data, i16 8
  %266 = load i16, ptr %265, !tbaa !2
  %267 = icmp sge i16 %264, %266
  br i1 %267, label %b21, label %b22

b21:
  store i16 0, ptr @"X%", !tbaa !2
  %268 = getelementptr i8, ptr @$data, i16 12
  store i16 15, ptr %268, !tbaa !2
  %269 = getelementptr i8, ptr @$data, i16 14
  store i16 1, ptr %269, !tbaa !2
  br label %b23

b22:
  %270 = load i16, ptr @"I%", !tbaa !2
  %271 = getelementptr i8, ptr @$data, i16 6
  %272 = load i16, ptr %271, !tbaa !2
  %273 = add nsw i16 %270, %272
  store i16 %273, ptr @"I%", !tbaa !2
  br label %b13

b23:
  %274 = getelementptr i8, ptr @$data, i16 14
  %275 = load i16, ptr %274, !tbaa !2
  %276 = icmp sge i16 %275, 0
  br i1 %276, label %b24, label %b25

b24:
  %277 = load i16, ptr @"X%", !tbaa !2
  %278 = getelementptr i8, ptr @$data, i16 12
  %279 = load i16, ptr %278, !tbaa !2
  %280 = icmp sle i16 %277, %279
  br i1 %280, label %b26, label %b27

b25:
  %281 = load i16, ptr @"X%", !tbaa !2
  %282 = getelementptr i8, ptr @$data, i16 12
  %283 = load i16, ptr %282, !tbaa !2
  %284 = icmp sge i16 %281, %283
  br i1 %284, label %b26, label %b27

b26:
  %285 = addrspacecast ptr @C$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %285, i16 0)
  %286 = load i16, ptr @"I%", !tbaa !2
  %287 = mul i16 %286, 256
  %288 = load i16, ptr @"Y%", !tbaa !2
  %289 = mul i16 %288, 16
  %290 = add i16 %287, %289
  %291 = load i16, ptr @"X%", !tbaa !2
  %292 = add i16 %290, %291
  %293 = mul i16 %292, 2
  %294 = getelementptr i8, ptr @"FONTS%", i16 2
  %295 = load i16, ptr %294, !tbaa !2
  %296 = getelementptr i8, ptr @"FONTS%", i16 10
  %297 = load i16, ptr %296, !tbaa !2
  %298 = add i16 %297, %293
  %299 = inttoptr i16 %295 to ptr addrspace(2)
  %300 = addrspacecast ptr addrspace(2) %299 to ptr addrspace(1)
  %301 = getelementptr i8, ptr addrspace(1) %300, i16 %298
  %302 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @C$)
  store i16 %302, ptr addrspace(1) %301, !tbaa !10
  %303 = load i16, ptr @"X%", !tbaa !2
  %304 = getelementptr i8, ptr @$data, i16 14
  %305 = load i16, ptr %304, !tbaa !2
  %306 = add nsw i16 %303, %305
  store i16 %306, ptr @"X%", !tbaa !2
  br label %b23

b27:
  %307 = load i16, ptr @"Y%", !tbaa !2
  %308 = getelementptr i8, ptr @$data, i16 10
  %309 = load i16, ptr %308, !tbaa !2
  %310 = add nsw i16 %307, %309
  store i16 %310, ptr @"Y%", !tbaa !2
  br label %b18

b28:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %311 = getelementptr i8, ptr @$arguments$85, i16 48
  store i16 0, ptr %311, !tbaa !2
  %312 = getelementptr i8, ptr @$arguments$85, i16 48
  %313 = getelementptr i8, ptr @$arguments$85, i16 50
  store i16 255, ptr %313, !tbaa !2
  %314 = getelementptr i8, ptr @$arguments$85, i16 50
  %315 = getelementptr i8, ptr @$arguments$85, i16 52
  store i16 64, ptr %315, !tbaa !2
  %316 = getelementptr i8, ptr @$arguments$85, i16 52
  %317 = getelementptr i8, ptr @$arguments$85, i16 54
  store i16 0, ptr %317, !tbaa !2
  %318 = getelementptr i8, ptr @$arguments$85, i16 54
  %319 = getelementptr i8, ptr @$arguments$85, i16 56
  store i16 0, ptr %319, !tbaa !2
  %320 = getelementptr i8, ptr @$arguments$85, i16 56
  %321 = getelementptr i8, ptr @$arguments$85, i16 58
  store i16 0, ptr %321, !tbaa !2
  %322 = getelementptr i8, ptr @$arguments$85, i16 58
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %312, ptr %314, ptr %316, ptr %318, ptr %320, ptr %322)
  br label %b2

b29:
  br label %b30

b30:
  call cc 1000 addrspace(1) void @CROSFADE(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %323 = load i16, ptr @"XIT%", !tbaa !2
  %324 = icmp eq i16 %323, 1
  br i1 %324, label %b32, label %b33

b32:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %325 = getelementptr i8, ptr @$arguments$85, i16 60
  store i16 0, ptr %325, !tbaa !2
  %326 = getelementptr i8, ptr @$arguments$85, i16 60
  %327 = getelementptr i8, ptr @$arguments$85, i16 62
  store i16 255, ptr %327, !tbaa !2
  %328 = getelementptr i8, ptr @$arguments$85, i16 62
  %329 = getelementptr i8, ptr @$arguments$85, i16 64
  store i16 64, ptr %329, !tbaa !2
  %330 = getelementptr i8, ptr @$arguments$85, i16 64
  %331 = getelementptr i8, ptr @$arguments$85, i16 66
  store i16 0, ptr %331, !tbaa !2
  %332 = getelementptr i8, ptr @$arguments$85, i16 66
  %333 = getelementptr i8, ptr @$arguments$85, i16 68
  store i16 0, ptr %333, !tbaa !2
  %334 = getelementptr i8, ptr @$arguments$85, i16 68
  %335 = getelementptr i8, ptr @$arguments$85, i16 70
  store i16 0, ptr %335, !tbaa !2
  %336 = getelementptr i8, ptr @$arguments$85, i16 70
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %326, ptr %328, ptr %330, ptr %332, ptr %334, ptr %336)
  br label %b2

b33:
  br label %b34

b34:
  %337 = getelementptr i8, ptr @$arguments$85, i16 72
  store i16 1, ptr %337, !tbaa !2
  %338 = getelementptr i8, ptr @$arguments$85, i16 72
  %339 = getelementptr i8, ptr @$arguments$85, i16 74
  store i16 1, ptr %339, !tbaa !2
  %340 = getelementptr i8, ptr @$arguments$85, i16 74
  call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr %338, ptr %340)
  call cc 1000 addrspace(1) void @SETCROSFADEPAL(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %341 = getelementptr i8, ptr @$arguments$85, i16 76
  store i16 1, ptr %341, !tbaa !2
  %342 = getelementptr i8, ptr @$arguments$85, i16 76
  %343 = getelementptr i8, ptr @$arguments$85, i16 78
  store i16 2, ptr %343, !tbaa !2
  %344 = getelementptr i8, ptr @$arguments$85, i16 78
  call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr %342, ptr %344)
  call cc 1000 addrspace(1) void @DELAY(ptr @"DL%")
  %345 = load i16, ptr @"XIT%", !tbaa !2
  %346 = icmp eq i16 %345, 1
  br i1 %346, label %b36, label %b37

b36:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %347 = getelementptr i8, ptr @$arguments$85, i16 80
  store i16 0, ptr %347, !tbaa !2
  %348 = getelementptr i8, ptr @$arguments$85, i16 80
  %349 = getelementptr i8, ptr @$arguments$85, i16 82
  store i16 255, ptr %349, !tbaa !2
  %350 = getelementptr i8, ptr @$arguments$85, i16 82
  %351 = getelementptr i8, ptr @$arguments$85, i16 84
  store i16 64, ptr %351, !tbaa !2
  %352 = getelementptr i8, ptr @$arguments$85, i16 84
  %353 = getelementptr i8, ptr @$arguments$85, i16 86
  store i16 0, ptr %353, !tbaa !2
  %354 = getelementptr i8, ptr @$arguments$85, i16 86
  %355 = getelementptr i8, ptr @$arguments$85, i16 88
  store i16 0, ptr %355, !tbaa !2
  %356 = getelementptr i8, ptr @$arguments$85, i16 88
  %357 = getelementptr i8, ptr @$arguments$85, i16 90
  store i16 0, ptr %357, !tbaa !2
  %358 = getelementptr i8, ptr @$arguments$85, i16 90
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %348, ptr %350, ptr %352, ptr %354, ptr %356, ptr %358)
  br label %b2

b37:
  br label %b38

b38:
  call cc 1000 addrspace(1) void @CROSFADE(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %359 = load i16, ptr @"XIT%", !tbaa !2
  %360 = icmp eq i16 %359, 1
  br i1 %360, label %b40, label %b41

b40:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %361 = getelementptr i8, ptr @$arguments$85, i16 92
  store i16 0, ptr %361, !tbaa !2
  %362 = getelementptr i8, ptr @$arguments$85, i16 92
  %363 = getelementptr i8, ptr @$arguments$85, i16 94
  store i16 255, ptr %363, !tbaa !2
  %364 = getelementptr i8, ptr @$arguments$85, i16 94
  %365 = getelementptr i8, ptr @$arguments$85, i16 96
  store i16 64, ptr %365, !tbaa !2
  %366 = getelementptr i8, ptr @$arguments$85, i16 96
  %367 = getelementptr i8, ptr @$arguments$85, i16 98
  store i16 0, ptr %367, !tbaa !2
  %368 = getelementptr i8, ptr @$arguments$85, i16 98
  %369 = getelementptr i8, ptr @$arguments$85, i16 100
  store i16 0, ptr %369, !tbaa !2
  %370 = getelementptr i8, ptr @$arguments$85, i16 100
  %371 = getelementptr i8, ptr @$arguments$85, i16 102
  store i16 0, ptr %371, !tbaa !2
  %372 = getelementptr i8, ptr @$arguments$85, i16 102
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %362, ptr %364, ptr %366, ptr %368, ptr %370, ptr %372)
  br label %b2

b41:
  br label %b42

b42:
  call cc 1000 addrspace(1) void @DELAY(ptr @"DL%")
  %373 = load i16, ptr @"XIT%", !tbaa !2
  %374 = icmp eq i16 %373, 1
  br i1 %374, label %b44, label %b45

b44:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %375 = getelementptr i8, ptr @$arguments$85, i16 104
  store i16 0, ptr %375, !tbaa !2
  %376 = getelementptr i8, ptr @$arguments$85, i16 104
  %377 = getelementptr i8, ptr @$arguments$85, i16 106
  store i16 255, ptr %377, !tbaa !2
  %378 = getelementptr i8, ptr @$arguments$85, i16 106
  %379 = getelementptr i8, ptr @$arguments$85, i16 108
  store i16 64, ptr %379, !tbaa !2
  %380 = getelementptr i8, ptr @$arguments$85, i16 108
  %381 = getelementptr i8, ptr @$arguments$85, i16 110
  store i16 0, ptr %381, !tbaa !2
  %382 = getelementptr i8, ptr @$arguments$85, i16 110
  %383 = getelementptr i8, ptr @$arguments$85, i16 112
  store i16 0, ptr %383, !tbaa !2
  %384 = getelementptr i8, ptr @$arguments$85, i16 112
  %385 = getelementptr i8, ptr @$arguments$85, i16 114
  store i16 0, ptr %385, !tbaa !2
  %386 = getelementptr i8, ptr @$arguments$85, i16 114
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %376, ptr %378, ptr %380, ptr %382, ptr %384, ptr %386)
  br label %b2

b45:
  br label %b46

b46:
  %387 = getelementptr i8, ptr @$arguments$85, i16 116
  store i16 2, ptr %387, !tbaa !2
  %388 = getelementptr i8, ptr @$arguments$85, i16 116
  %389 = getelementptr i8, ptr @$arguments$85, i16 118
  store i16 2, ptr %389, !tbaa !2
  %390 = getelementptr i8, ptr @$arguments$85, i16 118
  call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr %388, ptr %390)
  call cc 1000 addrspace(1) void @SETCROSFADEPAL(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %391 = getelementptr i8, ptr @$arguments$85, i16 120
  store i16 2, ptr %391, !tbaa !2
  %392 = getelementptr i8, ptr @$arguments$85, i16 120
  %393 = getelementptr i8, ptr @$arguments$85, i16 122
  store i16 3, ptr %393, !tbaa !2
  %394 = getelementptr i8, ptr @$arguments$85, i16 122
  call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr %392, ptr %394)
  call cc 1000 addrspace(1) void @CROSFADE(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %395 = load i16, ptr @"XIT%", !tbaa !2
  %396 = icmp eq i16 %395, 1
  br i1 %396, label %b48, label %b49

b48:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %397 = getelementptr i8, ptr @$arguments$85, i16 124
  store i16 0, ptr %397, !tbaa !2
  %398 = getelementptr i8, ptr @$arguments$85, i16 124
  %399 = getelementptr i8, ptr @$arguments$85, i16 126
  store i16 255, ptr %399, !tbaa !2
  %400 = getelementptr i8, ptr @$arguments$85, i16 126
  %401 = getelementptr i8, ptr @$arguments$85, i16 128
  store i16 64, ptr %401, !tbaa !2
  %402 = getelementptr i8, ptr @$arguments$85, i16 128
  %403 = getelementptr i8, ptr @$arguments$85, i16 130
  store i16 0, ptr %403, !tbaa !2
  %404 = getelementptr i8, ptr @$arguments$85, i16 130
  %405 = getelementptr i8, ptr @$arguments$85, i16 132
  store i16 0, ptr %405, !tbaa !2
  %406 = getelementptr i8, ptr @$arguments$85, i16 132
  %407 = getelementptr i8, ptr @$arguments$85, i16 134
  store i16 0, ptr %407, !tbaa !2
  %408 = getelementptr i8, ptr @$arguments$85, i16 134
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %398, ptr %400, ptr %402, ptr %404, ptr %406, ptr %408)
  br label %b2

b49:
  br label %b50

b50:
  call cc 1000 addrspace(1) void @DELAY(ptr @"DL%")
  %409 = load i16, ptr @"XIT%", !tbaa !2
  %410 = icmp eq i16 %409, 1
  br i1 %410, label %b52, label %b53

b52:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %411 = getelementptr i8, ptr @$arguments$85, i16 136
  store i16 0, ptr %411, !tbaa !2
  %412 = getelementptr i8, ptr @$arguments$85, i16 136
  %413 = getelementptr i8, ptr @$arguments$85, i16 138
  store i16 255, ptr %413, !tbaa !2
  %414 = getelementptr i8, ptr @$arguments$85, i16 138
  %415 = getelementptr i8, ptr @$arguments$85, i16 140
  store i16 64, ptr %415, !tbaa !2
  %416 = getelementptr i8, ptr @$arguments$85, i16 140
  %417 = getelementptr i8, ptr @$arguments$85, i16 142
  store i16 0, ptr %417, !tbaa !2
  %418 = getelementptr i8, ptr @$arguments$85, i16 142
  %419 = getelementptr i8, ptr @$arguments$85, i16 144
  store i16 0, ptr %419, !tbaa !2
  %420 = getelementptr i8, ptr @$arguments$85, i16 144
  %421 = getelementptr i8, ptr @$arguments$85, i16 146
  store i16 0, ptr %421, !tbaa !2
  %422 = getelementptr i8, ptr @$arguments$85, i16 146
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %412, ptr %414, ptr %416, ptr %418, ptr %420, ptr %422)
  br label %b2

b53:
  br label %b54

b54:
  call cc 1000 addrspace(1) void @GETPAL()
  %423 = getelementptr i8, ptr @$arguments$85, i16 148
  store i16 0, ptr %423, !tbaa !2
  %424 = getelementptr i8, ptr @$arguments$85, i16 148
  %425 = getelementptr i8, ptr @$arguments$85, i16 150
  store i16 255, ptr %425, !tbaa !2
  %426 = getelementptr i8, ptr @$arguments$85, i16 150
  %427 = getelementptr i8, ptr @$arguments$85, i16 152
  store i16 256, ptr %427, !tbaa !2
  %428 = getelementptr i8, ptr @$arguments$85, i16 152
  %429 = getelementptr i8, ptr @$arguments$85, i16 154
  store i16 63, ptr %429, !tbaa !2
  %430 = getelementptr i8, ptr @$arguments$85, i16 154
  %431 = getelementptr i8, ptr @$arguments$85, i16 156
  store i16 63, ptr %431, !tbaa !2
  %432 = getelementptr i8, ptr @$arguments$85, i16 156
  %433 = getelementptr i8, ptr @$arguments$85, i16 158
  store i16 63, ptr %433, !tbaa !2
  %434 = getelementptr i8, ptr @$arguments$85, i16 158
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %424, ptr %426, ptr %428, ptr %430, ptr %432, ptr %434)
  %435 = load i16, ptr @"XIT%", !tbaa !2
  %436 = icmp eq i16 %435, 1
  br i1 %436, label %b56, label %b57

b56:
  store i16 2, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @GETPAL()
  %437 = getelementptr i8, ptr @$arguments$85, i16 160
  store i16 0, ptr %437, !tbaa !2
  %438 = getelementptr i8, ptr @$arguments$85, i16 160
  %439 = getelementptr i8, ptr @$arguments$85, i16 162
  store i16 255, ptr %439, !tbaa !2
  %440 = getelementptr i8, ptr @$arguments$85, i16 162
  %441 = getelementptr i8, ptr @$arguments$85, i16 164
  store i16 64, ptr %441, !tbaa !2
  %442 = getelementptr i8, ptr @$arguments$85, i16 164
  %443 = getelementptr i8, ptr @$arguments$85, i16 166
  store i16 0, ptr %443, !tbaa !2
  %444 = getelementptr i8, ptr @$arguments$85, i16 166
  %445 = getelementptr i8, ptr @$arguments$85, i16 168
  store i16 0, ptr %445, !tbaa !2
  %446 = getelementptr i8, ptr @$arguments$85, i16 168
  %447 = getelementptr i8, ptr @$arguments$85, i16 170
  store i16 0, ptr %447, !tbaa !2
  %448 = getelementptr i8, ptr @$arguments$85, i16 170
  call cc 1000 addrspace(1) void @FADETOCOLOR(ptr %438, ptr %440, ptr %442, ptr %444, ptr %446, ptr %448)
  br label %b2

b57:
  br label %b58

b58:
  br label %b2

b60:
  %449 = getelementptr i8, ptr @$data, i16 18
  %450 = load i16, ptr %449, !tbaa !2
  %451 = icmp sge i16 %450, 0
  br i1 %451, label %b61, label %b62

b61:
  %452 = load i16, ptr @"I%", !tbaa !2
  %453 = getelementptr i8, ptr @$data, i16 16
  %454 = load i16, ptr %453, !tbaa !2
  %455 = icmp sle i16 %452, %454
  br i1 %455, label %b63, label %b64

b62:
  %456 = load i16, ptr @"I%", !tbaa !2
  %457 = getelementptr i8, ptr @$data, i16 16
  %458 = load i16, ptr %457, !tbaa !2
  %459 = icmp sge i16 %456, %458
  br i1 %459, label %b63, label %b64

b63:
  %460 = load i16, ptr @"I%", !tbaa !2
  %461 = load i16, ptr @"I%", !tbaa !2
  %462 = getelementptr i8, ptr @$arguments$85, i16 192
  store i16 %461, ptr %462, !tbaa !2
  %463 = getelementptr i8, ptr @$arguments$85, i16 192
  %464 = load i16, ptr %463, !tbaa !2
  %465 = sitofp i16 %464 to float
  %466 = getelementptr i8, ptr @$arguments$85, i16 194
  store i16 50, ptr %466, !tbaa !2
  %467 = getelementptr i8, ptr @$arguments$85, i16 194
  %468 = load i16, ptr %467, !tbaa !2
  %469 = sitofp i16 %468 to float
  %470 = fdiv float %465, %469
  %471 = call float @llvm.fabs.f32(float %470)
  %472 = load float, ptr @$float86, !tbaa !2
  %473 = fcmp ogt float %471, %472
  br i1 %473, label %b65, label %b66

b64:
  store i32 -100, ptr @"I&", !tbaa !2
  %474 = getelementptr i8, ptr @$data, i16 20
  store i32 99, ptr %474, !tbaa !2
  %475 = getelementptr i8, ptr @$data, i16 24
  store i32 1, ptr %475, !tbaa !2
  br label %b67

b65:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b66:
  %476 = call float @llvm.sin.f32(float %470)
  %477 = getelementptr i8, ptr @$arguments$85, i16 196
  store i16 48, ptr %477, !tbaa !2
  %478 = getelementptr i8, ptr @$arguments$85, i16 196
  %479 = load i16, ptr %478, !tbaa !2
  %480 = sitofp i16 %479 to float
  %481 = fmul float %476, %480
  %482 = call i16 @llvm.lrint.i16.f32(float %481)
  %483 = sub i16 %460, -48
  %484 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %483
  store i16 %482, ptr %484, !tbaa !2
  %485 = load i16, ptr @"I%", !tbaa !2
  %486 = getelementptr i8, ptr @$data, i16 18
  %487 = load i16, ptr %486, !tbaa !2
  %488 = add nsw i16 %485, %487
  store i16 %488, ptr @"I%", !tbaa !2
  br label %b60

b67:
  %489 = getelementptr i8, ptr @$data, i16 24
  %490 = load i32, ptr %489, !tbaa !2
  %491 = icmp sge i32 %490, 0
  br i1 %491, label %b68, label %b69

b68:
  %492 = load i32, ptr @"I&", !tbaa !2
  %493 = getelementptr i8, ptr @$data, i16 20
  %494 = load i32, ptr %493, !tbaa !2
  %495 = icmp sle i32 %492, %494
  br i1 %495, label %b70, label %b71

b69:
  %496 = load i32, ptr @"I&", !tbaa !2
  %497 = getelementptr i8, ptr @$data, i16 20
  %498 = load i32, ptr %497, !tbaa !2
  %499 = icmp sge i32 %496, %498
  br i1 %499, label %b70, label %b71

b70:
  %500 = load i32, ptr @"I&", !tbaa !2
  %501 = trunc i32 %500 to i16
  %502 = mul i16 %501, 4
  %503 = getelementptr i8, ptr @"YPK&", i16 2
  %504 = load i16, ptr %503, !tbaa !2
  %505 = add i16 400, %502
  %506 = inttoptr i16 %504 to ptr addrspace(2)
  %507 = addrspacecast ptr addrspace(2) %506 to ptr addrspace(1)
  %508 = getelementptr i8, ptr addrspace(1) %507, i16 %505
  %509 = load i32, ptr @"I&", !tbaa !2
  %510 = add i32 %509, 100
  %511 = mul i32 %510, 320
  store i32 %511, ptr addrspace(1) %508, !tbaa !12
  %512 = load i32, ptr @"I&", !tbaa !2
  %513 = getelementptr i8, ptr @$data, i16 24
  %514 = load i32, ptr %513, !tbaa !2
  %515 = add nsw i32 %512, %514
  store i32 %515, ptr @"I&", !tbaa !2
  br label %b67

b71:
  call cc 1000 addrspace(1) void @ACTIONS3D()
  %516 = getelementptr i8, ptr @$arguments$85, i16 198
  store i16 5, ptr %516, !tbaa !2
  %517 = getelementptr i8, ptr @$arguments$85, i16 198
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %517)
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FSIN2%$descriptor")
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FSIN3%$descriptor")
  call cc 1000 addrspace(1) void @CYCLEBLOBS()
  %518 = getelementptr i8, ptr @$arguments$85, i16 200
  store i16 6, ptr %518, !tbaa !2
  %519 = getelementptr i8, ptr @$arguments$85, i16 200
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %519)
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  call cc 1000 addrspace(1) void @PLASMABLOBS()
  %520 = getelementptr i8, ptr @$arguments$85, i16 202
  store i16 7, ptr %520, !tbaa !2
  %521 = getelementptr i8, ptr @$arguments$85, i16 202
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %521)
  store i16 0, ptr @"XIT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 0, i16 0)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 320, i16 200)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 255, i16 -1, i16 2)
  %522 = getelementptr i8, ptr @$arguments$85, i16 204
  store i16 0, ptr %522, !tbaa !2
  %523 = getelementptr i8, ptr @$arguments$85, i16 204
  %524 = getelementptr i8, ptr @$arguments$85, i16 206
  store i16 255, ptr %524, !tbaa !2
  %525 = getelementptr i8, ptr @$arguments$85, i16 206
  %526 = getelementptr i8, ptr @$arguments$85, i16 208
  store i16 0, ptr %526, !tbaa !2
  %527 = getelementptr i8, ptr @$arguments$85, i16 208
  %528 = getelementptr i8, ptr @$arguments$85, i16 210
  store i16 0, ptr %528, !tbaa !2
  %529 = getelementptr i8, ptr @$arguments$85, i16 210
  %530 = getelementptr i8, ptr @$arguments$85, i16 212
  store i16 0, ptr %530, !tbaa !2
  %531 = getelementptr i8, ptr @$arguments$85, i16 212
  %532 = getelementptr i8, ptr @$arguments$85, i16 214
  store i16 63, ptr %532, !tbaa !2
  %533 = getelementptr i8, ptr @$arguments$85, i16 214
  %534 = getelementptr i8, ptr @$arguments$85, i16 216
  store i16 63, ptr %534, !tbaa !2
  %535 = getelementptr i8, ptr @$arguments$85, i16 216
  %536 = getelementptr i8, ptr @$arguments$85, i16 218
  store i16 63, ptr %536, !tbaa !2
  %537 = getelementptr i8, ptr @$arguments$85, i16 218
  call cc 1000 addrspace(1) void @SETPAL(ptr %523, ptr %525, ptr %527, ptr %529, ptr %531, ptr %533, ptr %535, ptr %537)
  %538 = getelementptr i8, ptr @$data, i16 28
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string87, ptr %538)
  call cc 1000 addrspace(1) void @GETMAP(ptr %538)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %538)
  call cc 1000 addrspace(1) void @PRECALCULATIONS()
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string88, ptr @K$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string89, ptr @A$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string90, ptr @GI$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string91, ptr @GG$)
  store i16 0, ptr @"G%.1", !tbaa !2
  store i16 0, ptr @"GI%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @A$, i16 2, i16 -1, i16 32)
  %539 = addrspacecast ptr @GI$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) %539, i16 0)
  %540 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @GI$)
  store i16 %540, ptr @"GI%", !tbaa !2
  %541 = addrspacecast ptr @GG$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) %541, i16 0)
  %542 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @GG$)
  store i16 %542, ptr @"GG%", !tbaa !2
  br label %b72

b72:
  %543 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %544 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %543, ptr @$string92)
  %545 = icmp ne i16 %544, 0
  br i1 %545, label %b73, label %b74

b73:
  br label %b72

b74:
  store i16 32, ptr @"PH%", !tbaa !2
  store i16 96, ptr @"PX%", !tbaa !2
  store i16 96, ptr @"PY%", !tbaa !2
  store i16 90, ptr @"PA%", !tbaa !2
  store i16 3, ptr @"PS%", !tbaa !2
  store i16 60, ptr @"FV%", !tbaa !2
  %546 = load i16, ptr @"FV%", !tbaa !2
  %547 = getelementptr i8, ptr @$arguments$85, i16 220
  store i16 %546, ptr %547, !tbaa !2
  %548 = getelementptr i8, ptr @$arguments$85, i16 220
  %549 = load i16, ptr %548, !tbaa !2
  %550 = sitofp i16 %549 to float
  %551 = getelementptr i8, ptr @$arguments$85, i16 222
  store i16 320, ptr %551, !tbaa !2
  %552 = getelementptr i8, ptr @$arguments$85, i16 222
  %553 = load i16, ptr %552, !tbaa !2
  %554 = sitofp i16 %553 to float
  %555 = fdiv float %550, %554
  %556 = fpround float %555 to float
  store float %556, ptr @"RA!", !tbaa !2
  %557 = load i16, ptr @"FV%", !tbaa !2
  %558 = sext i16 %557 to i32
  %559 = sdiv i32 %558, 2
  %560 = trunc i32 %559 to i16
  %561 = load float, ptr @$float93, !tbaa !2
  %562 = getelementptr i8, ptr @$arguments$85, i16 224
  store i16 %560, ptr %562, !tbaa !2
  %563 = getelementptr i8, ptr @$arguments$85, i16 224
  %564 = load i16, ptr %563, !tbaa !2
  %565 = sitofp i16 %564 to float
  %566 = fdiv float %565, %561
  %567 = call float @llvm.fabs.f32(float %566)
  %568 = load float, ptr @$float86, !tbaa !2
  %569 = fcmp ogt float %567, %568
  br i1 %569, label %b75, label %b76

b75:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b76:
  %570 = call float @llvm.sin.f32(float %566)
  %571 = call float @llvm.cos.f32(float %566)
  %572 = fdiv float %570, %571
  %573 = getelementptr i8, ptr @$arguments$85, i16 226
  store i16 160, ptr %573, !tbaa !2
  %574 = getelementptr i8, ptr @$arguments$85, i16 226
  %575 = load i16, ptr %574, !tbaa !2
  %576 = sitofp i16 %575 to float
  %577 = fdiv float %576, %572
  %578 = call i16 @llvm.lrint.i16.f32(float %577)
  store i16 %578, ptr @"DPP%", !tbaa !2
  store i16 0, ptr @"MAP%", !tbaa !2
  store i16 0, ptr @"K%", !tbaa !2
  store i16 0, ptr @"FPS%", !tbaa !2
  store i16 0, ptr @"FILEI%", !tbaa !2
  br label %b77

b77:
  %579 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %580 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %579, ptr @$string94)
  %581 = icmp eq i16 %580, 0
  br i1 %581, label %b78, label %b79

b78:
  %582 = load i16, ptr @"FPS%", !tbaa !2
  %583 = add i16 %582, 1
  store i16 %583, ptr @"FPS%", !tbaa !2
  %584 = load i16, ptr @"FPS%", !tbaa !2
  %585 = icmp eq i16 %584, 558
  br i1 %585, label %b80, label %b81

b79:
  br label %b3

b80:
  store i16 1, ptr @"MAP%", !tbaa !2
  br label %b82

b81:
  br label %b82

b82:
  %586 = load i16, ptr @"FPS%", !tbaa !2
  %587 = icmp eq i16 %586, 2148
  br i1 %587, label %b83, label %b84

b83:
  store i16 0, ptr @"MAP%", !tbaa !2
  br label %b85

b84:
  br label %b85

b85:
  %588 = load i16, ptr @"FPS%", !tbaa !2
  %589 = icmp eq i16 %588, 3500
  br i1 %589, label %b86, label %b87

b86:
  store i16 1, ptr @"MAP%", !tbaa !2
  br label %b88

b87:
  br label %b88

b88:
  %590 = load i16, ptr @"FPS%", !tbaa !2
  %591 = icmp eq i16 %590, 4760
  br i1 %591, label %b89, label %b90

b89:
  store i16 0, ptr @"MAP%", !tbaa !2
  br label %b91

b90:
  br label %b91

b91:
  call cc 1000 addrspace(1) void @KEYACTION()
  call cc 1000 addrspace(1) void @LOADRECORD()
  call cc 1000 addrspace(1) void @ACTIONS()
  %592 = load i16, ptr @"PY%", !tbaa !2
  %593 = sext i16 %592 to i32
  %594 = sdiv i32 %593, 16
  %595 = trunc i32 %594 to i16
  %596 = sub i16 %595, 0
  %597 = getelementptr inbounds i16, ptr @"MUL128%", i16 %596
  %598 = load i16, ptr %597, !tbaa !2
  %599 = load i16, ptr @"PX%", !tbaa !2
  %600 = sext i16 %599 to i32
  %601 = sdiv i32 %600, 16
  %602 = trunc i32 %601 to i16
  %603 = add i16 %598, %602
  %604 = sub i16 %603, 0
  %605 = getelementptr inbounds i16, ptr @"SP%", i16 %604
  store i16 64, ptr %605, !tbaa !2
  call cc 1000 addrspace(1) void @INTERSECTIONS()
  %606 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %606, ptr @"BENCHFRAME&", !tbaa !2
  %607 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %607, ptr @"BENCHFRAME&", !tbaa !2
  call cc 1000 addrspace(1) void @OUTPUT0()
  %608 = load i16, ptr @"FILEI%", !tbaa !2
  %609 = icmp eq i16 %608, 20
  br i1 %609, label %b92, label %b93

b92:
  br label %b3

b93:
  br label %b94

b94:
  br label %b77
}

define cc 1000 void @ACTIONS() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  br label %b7

b2:
  br label %b14

b3:
  br label %b21

b4:
  br label %b35

b5:
  br label %b49

b6:
  br label %b56

b7:
  %10 = getelementptr inbounds i16, ptr @"KON%", i16 75
  %11 = load i16, ptr %10, !tbaa !2
  %12 = icmp eq i16 %11, 1
  br i1 %12, label %b8, label %b9

b8:
  %13 = load i16, ptr @"PA%", !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr @"PA%", !tbaa !2
  %15 = load i16, ptr @"PA%", !tbaa !2
  %16 = icmp sgt i16 %15, 360
  br i1 %16, label %b10, label %b11

b9:
  br label %b2

b10:
  %17 = load i16, ptr @"PA%", !tbaa !2
  %18 = sub i16 %17, 360
  %19 = add i16 0, %18
  store i16 %19, ptr @"PA%", !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  br label %b2

b14:
  %20 = getelementptr inbounds i16, ptr @"KON%", i16 77
  %21 = load i16, ptr %20, !tbaa !2
  %22 = icmp eq i16 %21, 1
  br i1 %22, label %b15, label %b16

b15:
  %23 = load i16, ptr @"PA%", !tbaa !2
  %24 = sub i16 %23, 1
  store i16 %24, ptr @"PA%", !tbaa !2
  %25 = load i16, ptr @"PA%", !tbaa !2
  %26 = icmp slt i16 %25, 0
  br i1 %26, label %b17, label %b18

b16:
  br label %b3

b17:
  %27 = load i16, ptr @"PA%", !tbaa !2
  %28 = add i16 360, %27
  store i16 %28, ptr @"PA%", !tbaa !2
  br label %b19

b18:
  br label %b19

b19:
  br label %b3

b21:
  %29 = getelementptr inbounds i16, ptr @"KON%", i16 72
  %30 = load i16, ptr %29, !tbaa !2
  %31 = icmp eq i16 %30, 1
  br i1 %31, label %b22, label %b23

b22:
  %32 = load i16, ptr @"PA%", !tbaa !2
  %33 = load float, ptr @$float93, !tbaa !2
  store i16 %32, ptr %8, !tbaa !2
  %34 = load i16, ptr %8, !tbaa !2
  %35 = sitofp i16 %34 to float
  %36 = fdiv float %35, %33
  %37 = call float @llvm.fabs.f32(float %36)
  %38 = load float, ptr @$float86, !tbaa !2
  %39 = fcmp ogt float %37, %38
  br i1 %39, label %b24, label %b25

b23:
  br label %b4

b24:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b25:
  %40 = call float @llvm.cos.f32(float %36)
  %41 = load i16, ptr @"PS%", !tbaa !2
  store i16 %41, ptr %7, !tbaa !2
  %42 = load i16, ptr %7, !tbaa !2
  %43 = sitofp i16 %42 to float
  %44 = fmul float %40, %43
  %45 = call i16 @llvm.lrint.i16.f32(float %44)
  store i16 %45, ptr %9, !tbaa !2
  %46 = load i16, ptr @"PA%", !tbaa !2
  %47 = load float, ptr @$float93, !tbaa !2
  store i16 %46, ptr %5, !tbaa !2
  %48 = load i16, ptr %5, !tbaa !2
  %49 = sitofp i16 %48 to float
  %50 = fdiv float %49, %47
  %51 = call float @llvm.fabs.f32(float %50)
  %52 = load float, ptr @$float86, !tbaa !2
  %53 = fcmp ogt float %51, %52
  br i1 %53, label %b26, label %b27

b26:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b27:
  %54 = call float @llvm.sin.f32(float %50)
  %55 = load i16, ptr @"PS%", !tbaa !2
  store i16 %55, ptr %4, !tbaa !2
  %56 = load i16, ptr %4, !tbaa !2
  %57 = sitofp i16 %56 to float
  %58 = fmul float %54, %57
  %59 = call i16 @llvm.lrint.i16.f32(float %58)
  store i16 %59, ptr %6, !tbaa !2
  %60 = load i16, ptr @"PX%", !tbaa !2
  %61 = load i16, ptr %9, !tbaa !2
  %62 = add i16 %60, %61
  store i16 %62, ptr @"PX%", !tbaa !2
  %63 = load i16, ptr @"PY%", !tbaa !2
  %64 = sext i16 %63 to i32
  %65 = sdiv i32 %64, 64
  %66 = trunc i32 %65 to i16
  %67 = sub i16 %66, 0
  %68 = getelementptr inbounds i16, ptr @"MUL32%", i16 %67
  %69 = load i16, ptr %68, !tbaa !2
  %70 = load i16, ptr @"PX%", !tbaa !2
  %71 = sext i16 %70 to i32
  %72 = sdiv i32 %71, 64
  %73 = trunc i32 %72 to i16
  %74 = add i16 %69, %73
  %75 = sub i16 %74, 0
  %76 = getelementptr inbounds i16, ptr @"CD%", i16 %75
  %77 = load i16, ptr %76, !tbaa !2
  %78 = icmp eq i16 %77, 1
  br i1 %78, label %b28, label %b29

b28:
  %79 = load i16, ptr @"PX%", !tbaa !2
  %80 = load i16, ptr %9, !tbaa !2
  %81 = sub i16 %79, %80
  store i16 %81, ptr @"PX%", !tbaa !2
  br label %b30

b29:
  br label %b30

b30:
  %82 = load i16, ptr @"PY%", !tbaa !2
  %83 = load i16, ptr %6, !tbaa !2
  %84 = sub i16 %82, %83
  store i16 %84, ptr @"PY%", !tbaa !2
  %85 = load i16, ptr @"PY%", !tbaa !2
  %86 = sext i16 %85 to i32
  %87 = sdiv i32 %86, 64
  %88 = trunc i32 %87 to i16
  %89 = sub i16 %88, 0
  %90 = getelementptr inbounds i16, ptr @"MUL32%", i16 %89
  %91 = load i16, ptr %90, !tbaa !2
  %92 = load i16, ptr @"PX%", !tbaa !2
  %93 = sext i16 %92 to i32
  %94 = sdiv i32 %93, 64
  %95 = trunc i32 %94 to i16
  %96 = add i16 %91, %95
  %97 = sub i16 %96, 0
  %98 = getelementptr inbounds i16, ptr @"CD%", i16 %97
  %99 = load i16, ptr %98, !tbaa !2
  %100 = icmp eq i16 %99, 1
  br i1 %100, label %b31, label %b32

b31:
  %101 = load i16, ptr @"PY%", !tbaa !2
  %102 = load i16, ptr %6, !tbaa !2
  %103 = add i16 %101, %102
  store i16 %103, ptr @"PY%", !tbaa !2
  br label %b33

b32:
  br label %b33

b33:
  br label %b4

b35:
  %104 = getelementptr inbounds i16, ptr @"KON%", i16 80
  %105 = load i16, ptr %104, !tbaa !2
  %106 = icmp eq i16 %105, 1
  br i1 %106, label %b36, label %b37

b36:
  %107 = load i16, ptr @"PA%", !tbaa !2
  %108 = load float, ptr @$float93, !tbaa !2
  store i16 %107, ptr %3, !tbaa !2
  %109 = load i16, ptr %3, !tbaa !2
  %110 = sitofp i16 %109 to float
  %111 = fdiv float %110, %108
  %112 = call float @llvm.fabs.f32(float %111)
  %113 = load float, ptr @$float86, !tbaa !2
  %114 = fcmp ogt float %112, %113
  br i1 %114, label %b38, label %b39

b37:
  br label %b5

b38:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b39:
  %115 = call float @llvm.cos.f32(float %111)
  %116 = load i16, ptr @"PS%", !tbaa !2
  store i16 %116, ptr %2, !tbaa !2
  %117 = load i16, ptr %2, !tbaa !2
  %118 = sitofp i16 %117 to float
  %119 = fmul float %115, %118
  %120 = call i16 @llvm.lrint.i16.f32(float %119)
  store i16 %120, ptr %9, !tbaa !2
  %121 = load i16, ptr @"PA%", !tbaa !2
  %122 = load float, ptr @$float93, !tbaa !2
  store i16 %121, ptr %1, !tbaa !2
  %123 = load i16, ptr %1, !tbaa !2
  %124 = sitofp i16 %123 to float
  %125 = fdiv float %124, %122
  %126 = call float @llvm.fabs.f32(float %125)
  %127 = load float, ptr @$float86, !tbaa !2
  %128 = fcmp ogt float %126, %127
  br i1 %128, label %b40, label %b41

b40:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b41:
  %129 = call float @llvm.sin.f32(float %125)
  %130 = load i16, ptr @"PS%", !tbaa !2
  store i16 %130, ptr %0, !tbaa !2
  %131 = load i16, ptr %0, !tbaa !2
  %132 = sitofp i16 %131 to float
  %133 = fmul float %129, %132
  %134 = call i16 @llvm.lrint.i16.f32(float %133)
  store i16 %134, ptr %6, !tbaa !2
  %135 = load i16, ptr @"PX%", !tbaa !2
  %136 = load i16, ptr %9, !tbaa !2
  %137 = sub i16 %135, %136
  store i16 %137, ptr @"PX%", !tbaa !2
  %138 = load i16, ptr @"PY%", !tbaa !2
  %139 = sext i16 %138 to i32
  %140 = sdiv i32 %139, 64
  %141 = trunc i32 %140 to i16
  %142 = sub i16 %141, 0
  %143 = getelementptr inbounds i16, ptr @"MUL32%", i16 %142
  %144 = load i16, ptr %143, !tbaa !2
  %145 = load i16, ptr @"PX%", !tbaa !2
  %146 = sext i16 %145 to i32
  %147 = sdiv i32 %146, 64
  %148 = trunc i32 %147 to i16
  %149 = add i16 %144, %148
  %150 = sub i16 %149, 0
  %151 = getelementptr inbounds i16, ptr @"CD%", i16 %150
  %152 = load i16, ptr %151, !tbaa !2
  %153 = icmp eq i16 %152, 1
  br i1 %153, label %b42, label %b43

b42:
  %154 = load i16, ptr @"PX%", !tbaa !2
  %155 = load i16, ptr %9, !tbaa !2
  %156 = add i16 %154, %155
  store i16 %156, ptr @"PX%", !tbaa !2
  br label %b44

b43:
  br label %b44

b44:
  %157 = load i16, ptr @"PY%", !tbaa !2
  %158 = load i16, ptr %6, !tbaa !2
  %159 = add i16 %157, %158
  store i16 %159, ptr @"PY%", !tbaa !2
  %160 = load i16, ptr @"PY%", !tbaa !2
  %161 = sext i16 %160 to i32
  %162 = sdiv i32 %161, 64
  %163 = trunc i32 %162 to i16
  %164 = sub i16 %163, 0
  %165 = getelementptr inbounds i16, ptr @"MUL32%", i16 %164
  %166 = load i16, ptr %165, !tbaa !2
  %167 = load i16, ptr @"PX%", !tbaa !2
  %168 = sext i16 %167 to i32
  %169 = sdiv i32 %168, 64
  %170 = trunc i32 %169 to i16
  %171 = add i16 %166, %170
  %172 = sub i16 %171, 0
  %173 = getelementptr inbounds i16, ptr @"CD%", i16 %172
  %174 = load i16, ptr %173, !tbaa !2
  %175 = icmp eq i16 %174, 1
  br i1 %175, label %b45, label %b46

b45:
  %176 = load i16, ptr @"PY%", !tbaa !2
  %177 = load i16, ptr %6, !tbaa !2
  %178 = sub i16 %176, %177
  store i16 %178, ptr @"PY%", !tbaa !2
  br label %b47

b46:
  br label %b47

b47:
  br label %b5

b49:
  %179 = getelementptr inbounds i16, ptr @"KON%", i16 15
  %180 = load i16, ptr %179, !tbaa !2
  %181 = icmp eq i16 %180, 1
  br i1 %181, label %b50, label %b51

b50:
  %182 = load i16, ptr @"MAP%", !tbaa !2
  %183 = icmp eq i16 %182, 0
  br i1 %183, label %b52, label %b53

b51:
  br label %b6

b52:
  store i16 1, ptr @"MAP%", !tbaa !2
  br label %b54

b53:
  store i16 0, ptr @"MAP%", !tbaa !2
  br label %b54

b54:
  br label %b6

b56:
  %184 = getelementptr inbounds i16, ptr @"KON%", i16 1
  %185 = load i16, ptr %184, !tbaa !2
  %186 = icmp eq i16 %185, 1
  br i1 %186, label %b57, label %b58

b57:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b58:
  br label %b60

b60:
  %187 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %188 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %187, ptr @$string95)
  %189 = icmp ne i16 %188, 0
  br i1 %189, label %b61, label %b62

b61:
  br label %b60

b62:
  ret void
}

define cc 1000 void @ACTIONS3D() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca float
  %6 = alloca i16
  %7 = alloca float
  %8 = alloca i16
  %9 = alloca float
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca [4 x i8]
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca [4 x i8]
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca i16
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca i16
  %79 = alloca i16
  %80 = alloca i16
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca i16
  %84 = alloca i16
  %85 = alloca i16
  %86 = alloca i16
  %87 = alloca i16
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca i16
  %91 = alloca [4 x i8]
  %92 = alloca [4 x i8]
  %93 = alloca [4 x i8]
  %94 = alloca i16
  %95 = alloca i16
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store float 0.000000e+00, ptr %5
  store i16 0, ptr %6
  store float 0.000000e+00, ptr %7
  store i16 0, ptr %8
  store float 0.000000e+00, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store i16 0, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  store i16 0, ptr %33
  store i16 0, ptr %34
  store i16 0, ptr %35
  store i16 0, ptr %36
  store i16 0, ptr %37
  store i16 0, ptr %38
  store i16 0, ptr %39
  store i16 0, ptr %40
  call void @llvm.memset.p0.i16(ptr %41, i8 0, i16 4, i1 false)
  store i16 0, ptr %42
  store i16 0, ptr %43
  store i16 0, ptr %44
  store i16 0, ptr %45
  store i16 0, ptr %46
  store i16 0, ptr %47
  store i16 0, ptr %48
  store i16 0, ptr %49
  call void @llvm.memset.p0.i16(ptr %50, i8 0, i16 4, i1 false)
  store i16 0, ptr %51
  store i16 0, ptr %52
  store i16 0, ptr %53
  store i16 0, ptr %54
  store i16 0, ptr %55
  store i16 0, ptr %56
  store i16 0, ptr %57
  store i16 0, ptr %58
  store i16 0, ptr %59
  store i16 0, ptr %60
  store i16 0, ptr %61
  store i16 0, ptr %62
  store i16 0, ptr %63
  store i16 0, ptr %64
  store i16 0, ptr %65
  store i16 0, ptr %66
  store i16 0, ptr %67
  store i16 0, ptr %68
  store i16 0, ptr %69
  store i16 0, ptr %70
  store i16 0, ptr %71
  store i16 0, ptr %72
  store i16 0, ptr %73
  store i16 0, ptr %74
  store i16 0, ptr %75
  store i16 0, ptr %76
  store i16 0, ptr %77
  store i16 0, ptr %78
  store i16 0, ptr %79
  store i16 0, ptr %80
  store i16 0, ptr %81
  store i16 0, ptr %82
  store i16 0, ptr %83
  store i16 0, ptr %84
  store i16 0, ptr %85
  store i16 0, ptr %86
  store i16 0, ptr %87
  store i16 0, ptr %88
  store i16 0, ptr %89
  store i16 0, ptr %90
  call void @llvm.memset.p0.i16(ptr %91, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %92, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %93, i8 0, i16 4, i1 false)
  store i16 0, ptr %94
  store i16 0, ptr %95
  store i16 0, ptr %96
  store i16 0, ptr %97
  store i16 0, ptr %98
  store i16 0, ptr %99
  store i16 0, ptr %100
  store i16 0, ptr %101
  store i16 0, ptr %102
  store i16 0, ptr %93, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %93, i16 2
  store i16 0, ptr %103, !tbaa !2
  store i16 0, ptr %92, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %92, i16 2
  store i16 0, ptr %104, !tbaa !2
  store i16 0, ptr %91, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %91, i16 2
  store i16 0, ptr %105, !tbaa !2
  store i16 0, ptr %50, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %50, i16 2
  store i16 0, ptr %106, !tbaa !2
  store i16 0, ptr %41, !tbaa !2
  %107 = getelementptr inbounds i8, ptr %41, i16 2
  store i16 0, ptr %107, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 0, ptr %108, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  %109 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 0, ptr %109, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 0, ptr %110, !tbaa !2
  %111 = load double, ptr @$float96, !tbaa !2
  %112 = fptrunc double %111 to float
  %113 = fpround float %112 to float
  store float %113, ptr @"MTR!", !tbaa !2
  store i16 0, ptr @"K%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string97, ptr @GI$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string98, ptr @GG$)
  store i16 0, ptr %102, !tbaa !2
  %114 = load i16, ptr %102, !tbaa !2
  %115 = sitofp i16 %114 to float
  %116 = fpround float %115 to float
  store float %116, ptr @"XP!", !tbaa !2
  store i16 0, ptr %101, !tbaa !2
  %117 = load i16, ptr %101, !tbaa !2
  %118 = sitofp i16 %117 to float
  %119 = fpround float %118 to float
  store float %119, ptr @"YP!", !tbaa !2
  store i16 800, ptr %100, !tbaa !2
  %120 = load i16, ptr %100, !tbaa !2
  %121 = sitofp i16 %120 to float
  %122 = fpround float %121 to float
  store float %122, ptr @"ZP!", !tbaa !2
  %123 = getelementptr i8, ptr @"ZPO%", i16 2
  %124 = load i16, ptr %123, !tbaa !2
  %125 = inttoptr i16 %124 to ptr addrspace(2)
  %126 = addrspacecast ptr addrspace(2) %125 to ptr addrspace(1)
  %127 = getelementptr i8, ptr addrspace(1) %126, i16 2
  store i16 100, ptr addrspace(1) %127, !tbaa !14
  %128 = getelementptr i8, ptr @"ZPO%", i16 2
  %129 = load i16, ptr %128, !tbaa !2
  %130 = inttoptr i16 %129 to ptr addrspace(2)
  %131 = addrspacecast ptr addrspace(2) %130 to ptr addrspace(1)
  %132 = getelementptr i8, ptr addrspace(1) %131, i16 4
  store i16 200, ptr addrspace(1) %132, !tbaa !14
  %133 = getelementptr i8, ptr @"ZPO%", i16 2
  %134 = load i16, ptr %133, !tbaa !2
  %135 = inttoptr i16 %134 to ptr addrspace(2)
  %136 = addrspacecast ptr addrspace(2) %135 to ptr addrspace(1)
  %137 = getelementptr i8, ptr addrspace(1) %136, i16 6
  store i16 600, ptr addrspace(1) %137, !tbaa !14
  %138 = getelementptr i8, ptr @"ZPO%", i16 2
  %139 = load i16, ptr %138, !tbaa !2
  %140 = inttoptr i16 %139 to ptr addrspace(2)
  %141 = addrspacecast ptr addrspace(2) %140 to ptr addrspace(1)
  %142 = getelementptr i8, ptr addrspace(1) %141, i16 8
  store i16 800, ptr addrspace(1) %142, !tbaa !14
  %143 = getelementptr i8, ptr @"ZPO%", i16 2
  %144 = load i16, ptr %143, !tbaa !2
  %145 = inttoptr i16 %144 to ptr addrspace(2)
  %146 = addrspacecast ptr addrspace(2) %145 to ptr addrspace(1)
  %147 = getelementptr i8, ptr addrspace(1) %146, i16 10
  store i16 300, ptr addrspace(1) %147, !tbaa !14
  %148 = getelementptr i8, ptr @"ZPO%", i16 2
  %149 = load i16, ptr %148, !tbaa !2
  %150 = inttoptr i16 %149 to ptr addrspace(2)
  %151 = addrspacecast ptr addrspace(2) %150 to ptr addrspace(1)
  %152 = getelementptr i8, ptr addrspace(1) %151, i16 12
  store i16 300, ptr addrspace(1) %152, !tbaa !14
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 255, i16 2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 0, ptr %99, !tbaa !2
  %153 = load i16, ptr %99, !tbaa !2
  %154 = sitofp i16 %153 to float
  %155 = fpround float %154 to float
  store float %155, ptr @"XC!", !tbaa !2
  store i16 0, ptr %98, !tbaa !2
  %156 = load i16, ptr %98, !tbaa !2
  %157 = sitofp i16 %156 to float
  %158 = fpround float %157 to float
  store float %158, ptr @"YC!", !tbaa !2
  store i16 0, ptr %97, !tbaa !2
  %159 = load i16, ptr %97, !tbaa !2
  %160 = sitofp i16 %159 to float
  %161 = fpround float %160 to float
  store float %161, ptr @"ZC!", !tbaa !2
  store i16 0, ptr %96, !tbaa !2
  %162 = load i16, ptr %96, !tbaa !2
  %163 = sitofp i16 %162 to float
  %164 = fpround float %163 to float
  store float %164, ptr @"RXC!", !tbaa !2
  store i16 0, ptr %95, !tbaa !2
  %165 = load i16, ptr %95, !tbaa !2
  %166 = sitofp i16 %165 to float
  %167 = fpround float %166 to float
  store float %167, ptr @"RYC!", !tbaa !2
  store i16 0, ptr %94, !tbaa !2
  %168 = load i16, ptr %94, !tbaa !2
  %169 = sitofp i16 %168 to float
  %170 = fpround float %169 to float
  store float %170, ptr @"RZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string100, ptr %93)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr %93, i16 2, i16 -1, i16 32)
  store i16 0, ptr @"K%", !tbaa !2
  br label %b4

b2:
  store i16 0, ptr @"FILEI%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 2, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %93)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %92)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %91)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %50)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %41)
  ret void

b4:
  %171 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %171, ptr %92)
  %172 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %92, ptr @$string101)
  %173 = icmp eq i16 %172, 0
  br i1 %173, label %b6, label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b7:
  br label %b8

b8:
  call cc 1000 addrspace(1) void @LOAD3DRECORD()
  %174 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @GG$)
  %175 = icmp eq i16 %174, 0
  br i1 %175, label %b10, label %b11

b10:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string102, ptr %91)
  br label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @GG$, ptr %91)
  br label %b12

b12:
  br label %b13

b13:
  %176 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string103)
  %177 = icmp eq i16 %176, 0
  %178 = sext i1 %177 to i16
  %179 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string104)
  %180 = icmp eq i16 %179, 0
  %181 = sext i1 %180 to i16
  %182 = or i16 %178, %181
  %183 = icmp ne i16 %182, 0
  br i1 %183, label %b14, label %b15

b14:
  store i16 0, ptr %90, !tbaa !2
  store i16 255, ptr %89, !tbaa !2
  store i16 0, ptr %88, !tbaa !2
  store i16 0, ptr %87, !tbaa !2
  store i16 0, ptr %86, !tbaa !2
  store i16 63, ptr %85, !tbaa !2
  store i16 63, ptr %84, !tbaa !2
  store i16 63, ptr %83, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %90, ptr %89, ptr %88, ptr %87, ptr %86, ptr %85, ptr %84, ptr %83)
  store i16 1, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 64, ptr @"NDTSO%", !tbaa !2
  %184 = load i16, ptr @"NDTSO%", !tbaa !2
  store i16 %184, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string105, ptr %91)
  br label %b13

b15:
  br label %b16

b16:
  %185 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string106)
  %186 = icmp eq i16 %185, 0
  %187 = sext i1 %186 to i16
  %188 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string107)
  %189 = icmp eq i16 %188, 0
  %190 = sext i1 %189 to i16
  %191 = or i16 %187, %190
  %192 = icmp ne i16 %191, 0
  br i1 %192, label %b17, label %b18

b17:
  store i16 0, ptr %82, !tbaa !2
  store i16 255, ptr %81, !tbaa !2
  store i16 0, ptr %80, !tbaa !2
  store i16 0, ptr %79, !tbaa !2
  store i16 0, ptr %78, !tbaa !2
  store i16 63, ptr %77, !tbaa !2
  store i16 47, ptr %76, !tbaa !2
  store i16 15, ptr %75, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %82, ptr %81, ptr %80, ptr %79, ptr %78, ptr %77, ptr %76, ptr %75)
  store i16 2, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 256, ptr @"NDTSO%", !tbaa !2
  %193 = load i16, ptr @"NDTSO%", !tbaa !2
  store i16 %193, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string108, ptr %91)
  br label %b16

b18:
  br label %b19

b19:
  %194 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string109)
  %195 = icmp eq i16 %194, 0
  %196 = sext i1 %195 to i16
  %197 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string110)
  %198 = icmp eq i16 %197, 0
  %199 = sext i1 %198 to i16
  %200 = or i16 %196, %199
  %201 = icmp ne i16 %200, 0
  br i1 %201, label %b20, label %b21

b20:
  store i16 0, ptr %74, !tbaa !2
  store i16 255, ptr %73, !tbaa !2
  store i16 0, ptr %72, !tbaa !2
  store i16 0, ptr %71, !tbaa !2
  store i16 0, ptr %70, !tbaa !2
  store i16 31, ptr %69, !tbaa !2
  store i16 63, ptr %68, !tbaa !2
  store i16 47, ptr %67, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %74, ptr %73, ptr %72, ptr %71, ptr %70, ptr %69, ptr %68, ptr %67)
  store i16 3, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 512, ptr @"NDTSO%", !tbaa !2
  %202 = load i16, ptr @"NDTSO%", !tbaa !2
  store i16 %202, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string111, ptr %91)
  br label %b19

b21:
  br label %b22

b22:
  %203 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string112)
  %204 = icmp eq i16 %203, 0
  %205 = sext i1 %204 to i16
  %206 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string113)
  %207 = icmp eq i16 %206, 0
  %208 = sext i1 %207 to i16
  %209 = or i16 %205, %208
  %210 = icmp ne i16 %209, 0
  br i1 %210, label %b23, label %b24

b23:
  store i16 0, ptr %66, !tbaa !2
  store i16 255, ptr %65, !tbaa !2
  store i16 0, ptr %64, !tbaa !2
  store i16 0, ptr %63, !tbaa !2
  store i16 0, ptr %62, !tbaa !2
  store i16 15, ptr %61, !tbaa !2
  store i16 31, ptr %60, !tbaa !2
  store i16 63, ptr %59, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %66, ptr %65, ptr %64, ptr %63, ptr %62, ptr %61, ptr %60, ptr %59)
  store i16 4, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 64, ptr @"NDTSO%", !tbaa !2
  store i16 0, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string114, ptr %91)
  br label %b22

b24:
  br label %b25

b25:
  %211 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string115)
  %212 = icmp eq i16 %211, 0
  %213 = sext i1 %212 to i16
  %214 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string116)
  %215 = icmp eq i16 %214, 0
  %216 = sext i1 %215 to i16
  %217 = or i16 %213, %216
  %218 = icmp ne i16 %217, 0
  br i1 %218, label %b26, label %b27

b26:
  store i16 0, ptr %58, !tbaa !2
  store i16 255, ptr %57, !tbaa !2
  store i16 0, ptr %56, !tbaa !2
  store i16 0, ptr %55, !tbaa !2
  store i16 0, ptr %54, !tbaa !2
  store i16 0, ptr %53, !tbaa !2
  store i16 63, ptr %52, !tbaa !2
  store i16 63, ptr %51, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %58, ptr %57, ptr %56, ptr %55, ptr %54, ptr %53, ptr %52, ptr %51)
  store i16 5, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string117, ptr %50)
  call cc 1000 addrspace(1) void @LOADOBJECT(ptr %50)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %50)
  store i16 0, ptr @"NDTSO%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string118, ptr %91)
  br label %b25

b27:
  br label %b28

b28:
  %219 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string119)
  %220 = icmp eq i16 %219, 0
  %221 = sext i1 %220 to i16
  %222 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string120)
  %223 = icmp eq i16 %222, 0
  %224 = sext i1 %223 to i16
  %225 = or i16 %221, %224
  %226 = icmp ne i16 %225, 0
  br i1 %226, label %b29, label %b30

b29:
  store i16 0, ptr %49, !tbaa !2
  store i16 255, ptr %48, !tbaa !2
  store i16 0, ptr %47, !tbaa !2
  store i16 0, ptr %46, !tbaa !2
  store i16 0, ptr %45, !tbaa !2
  store i16 63, ptr %44, !tbaa !2
  store i16 31, ptr %43, !tbaa !2
  store i16 47, ptr %42, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %49, ptr %48, ptr %47, ptr %46, ptr %45, ptr %44, ptr %43, ptr %42)
  store i16 6, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string121, ptr %41)
  call cc 1000 addrspace(1) void @LOADOBJECT(ptr %41)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %41)
  store i16 0, ptr @"NDTSO%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string122, ptr %91)
  br label %b28

b30:
  br label %b31

b31:
  %227 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string123)
  %228 = icmp eq i16 %227, 0
  %229 = sext i1 %228 to i16
  %230 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string124)
  %231 = icmp eq i16 %230, 0
  %232 = sext i1 %231 to i16
  %233 = or i16 %229, %232
  %234 = icmp ne i16 %233, 0
  br i1 %234, label %b32, label %b33

b32:
  store i16 0, ptr %40, !tbaa !2
  call cc 1000 addrspace(1) void @CLEARSCREEN(ptr %40)
  %235 = load i16, ptr @"NDTS%", !tbaa !2
  %236 = load i16, ptr @"NDTSO%", !tbaa !2
  %237 = sub i16 %235, %236
  %238 = icmp sgt i16 %237, 0
  br i1 %238, label %b34, label %b35

b33:
  br label %b37

b34:
  %239 = load i16, ptr @"NDTS%", !tbaa !2
  %240 = load i16, ptr @"NDTSO%", !tbaa !2
  %241 = sub i16 %239, %240
  store i16 %241, ptr @"NDTS%", !tbaa !2
  br label %b36

b35:
  br label %b36

b36:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string125, ptr %91)
  br label %b31

b37:
  %242 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string126)
  %243 = icmp eq i16 %242, 0
  %244 = sext i1 %243 to i16
  %245 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string127)
  %246 = icmp eq i16 %245, 0
  %247 = sext i1 %246 to i16
  %248 = or i16 %244, %247
  %249 = icmp ne i16 %248, 0
  br i1 %249, label %b38, label %b39

b38:
  %250 = load i16, ptr @"NDTS%", !tbaa !2
  %251 = load i16, ptr @"NDTSO%", !tbaa !2
  %252 = add i16 %250, %251
  %253 = icmp slt i16 %252, 4096
  br i1 %253, label %b40, label %b41

b39:
  br label %b43

b40:
  %254 = load i16, ptr @"NDTS%", !tbaa !2
  %255 = load i16, ptr @"NDTSO%", !tbaa !2
  %256 = add i16 %254, %255
  store i16 %256, ptr @"NDTS%", !tbaa !2
  br label %b42

b41:
  br label %b42

b42:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string128, ptr %91)
  br label %b37

b43:
  %257 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string129)
  %258 = icmp eq i16 %257, 0
  %259 = sext i1 %258 to i16
  %260 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string130)
  %261 = icmp eq i16 %260, 0
  %262 = sext i1 %261 to i16
  %263 = or i16 %259, %262
  %264 = icmp ne i16 %263, 0
  br i1 %264, label %b44, label %b45

b44:
  %265 = load i16, ptr @"OBJ%", !tbaa !2
  %266 = mul i16 %265, 2
  %267 = getelementptr i8, ptr @"ZPO%", i16 2
  %268 = load i16, ptr %267, !tbaa !2
  %269 = add i16 0, %266
  %270 = inttoptr i16 %268 to ptr addrspace(2)
  %271 = addrspacecast ptr addrspace(2) %270 to ptr addrspace(1)
  %272 = getelementptr i8, ptr addrspace(1) %271, i16 %269
  %273 = load i16, ptr addrspace(1) %272, !tbaa !14
  %274 = sub i16 0, %273
  %275 = load float, ptr @$float131, !tbaa !2
  store i16 %274, ptr %39, !tbaa !2
  %276 = load i16, ptr %39, !tbaa !2
  %277 = sitofp i16 %276 to float
  %278 = fmul float %277, %275
  %279 = fpround float %278 to float
  store float %279, ptr @"XP!", !tbaa !2
  store i16 0, ptr %38, !tbaa !2
  %280 = load i16, ptr %38, !tbaa !2
  %281 = sitofp i16 %280 to float
  %282 = fpround float %281 to float
  store float %282, ptr @"YP!", !tbaa !2
  %283 = load i16, ptr @"OBJ%", !tbaa !2
  %284 = mul i16 %283, 2
  %285 = getelementptr i8, ptr @"ZPO%", i16 2
  %286 = load i16, ptr %285, !tbaa !2
  %287 = add i16 0, %284
  %288 = inttoptr i16 %286 to ptr addrspace(2)
  %289 = addrspacecast ptr addrspace(2) %288 to ptr addrspace(1)
  %290 = getelementptr i8, ptr addrspace(1) %289, i16 %287
  %291 = load i16, ptr addrspace(1) %290, !tbaa !14
  store i16 %291, ptr %37, !tbaa !2
  %292 = load i16, ptr %37, !tbaa !2
  %293 = sitofp i16 %292 to float
  %294 = fpround float %293 to float
  store float %294, ptr @"ZP!", !tbaa !2
  store i16 0, ptr %36, !tbaa !2
  %295 = load i16, ptr %36, !tbaa !2
  %296 = sitofp i16 %295 to float
  %297 = fpround float %296 to float
  store float %297, ptr @"XC!", !tbaa !2
  store i16 0, ptr %35, !tbaa !2
  %298 = load i16, ptr %35, !tbaa !2
  %299 = sitofp i16 %298 to float
  %300 = fpround float %299 to float
  store float %300, ptr @"YC!", !tbaa !2
  store i16 0, ptr %34, !tbaa !2
  %301 = load i16, ptr %34, !tbaa !2
  %302 = sitofp i16 %301 to float
  %303 = fpround float %302 to float
  store float %303, ptr @"ZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string132, ptr %91)
  br label %b43

b45:
  br label %b46

b46:
  %304 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string133)
  %305 = icmp eq i16 %304, 0
  %306 = sext i1 %305 to i16
  %307 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string134)
  %308 = icmp eq i16 %307, 0
  %309 = sext i1 %308 to i16
  %310 = or i16 %306, %309
  %311 = icmp ne i16 %310, 0
  br i1 %311, label %b47, label %b48

b47:
  %312 = load i16, ptr @"OBJ%", !tbaa !2
  %313 = mul i16 %312, 2
  %314 = getelementptr i8, ptr @"ZPO%", i16 2
  %315 = load i16, ptr %314, !tbaa !2
  %316 = add i16 0, %313
  %317 = inttoptr i16 %315 to ptr addrspace(2)
  %318 = addrspacecast ptr addrspace(2) %317 to ptr addrspace(1)
  %319 = getelementptr i8, ptr addrspace(1) %318, i16 %316
  %320 = load i16, ptr addrspace(1) %319, !tbaa !14
  %321 = load float, ptr @$float131, !tbaa !2
  store i16 %320, ptr %33, !tbaa !2
  %322 = load i16, ptr %33, !tbaa !2
  %323 = sitofp i16 %322 to float
  %324 = fmul float %323, %321
  %325 = fpround float %324 to float
  store float %325, ptr @"XP!", !tbaa !2
  store i16 0, ptr %32, !tbaa !2
  %326 = load i16, ptr %32, !tbaa !2
  %327 = sitofp i16 %326 to float
  %328 = fpround float %327 to float
  store float %328, ptr @"YP!", !tbaa !2
  %329 = load i16, ptr @"OBJ%", !tbaa !2
  %330 = mul i16 %329, 2
  %331 = getelementptr i8, ptr @"ZPO%", i16 2
  %332 = load i16, ptr %331, !tbaa !2
  %333 = add i16 0, %330
  %334 = inttoptr i16 %332 to ptr addrspace(2)
  %335 = addrspacecast ptr addrspace(2) %334 to ptr addrspace(1)
  %336 = getelementptr i8, ptr addrspace(1) %335, i16 %333
  %337 = load i16, ptr addrspace(1) %336, !tbaa !14
  store i16 %337, ptr %31, !tbaa !2
  %338 = load i16, ptr %31, !tbaa !2
  %339 = sitofp i16 %338 to float
  %340 = fpround float %339 to float
  store float %340, ptr @"ZP!", !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  %341 = load i16, ptr %30, !tbaa !2
  %342 = sitofp i16 %341 to float
  %343 = fpround float %342 to float
  store float %343, ptr @"XC!", !tbaa !2
  store i16 0, ptr %29, !tbaa !2
  %344 = load i16, ptr %29, !tbaa !2
  %345 = sitofp i16 %344 to float
  %346 = fpround float %345 to float
  store float %346, ptr @"YC!", !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  %347 = load i16, ptr %28, !tbaa !2
  %348 = sitofp i16 %347 to float
  %349 = fpround float %348 to float
  store float %349, ptr @"ZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string135, ptr %91)
  br label %b46

b48:
  br label %b49

b49:
  %350 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string136)
  %351 = icmp eq i16 %350, 0
  %352 = sext i1 %351 to i16
  %353 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string137)
  %354 = icmp eq i16 %353, 0
  %355 = sext i1 %354 to i16
  %356 = or i16 %352, %355
  %357 = icmp ne i16 %356, 0
  br i1 %357, label %b50, label %b51

b50:
  store i16 0, ptr %27, !tbaa !2
  %358 = load i16, ptr %27, !tbaa !2
  %359 = sitofp i16 %358 to float
  %360 = fpround float %359 to float
  store float %360, ptr @"XP!", !tbaa !2
  %361 = load i16, ptr @"OBJ%", !tbaa !2
  %362 = mul i16 %361, 2
  %363 = getelementptr i8, ptr @"ZPO%", i16 2
  %364 = load i16, ptr %363, !tbaa !2
  %365 = add i16 0, %362
  %366 = inttoptr i16 %364 to ptr addrspace(2)
  %367 = addrspacecast ptr addrspace(2) %366 to ptr addrspace(1)
  %368 = getelementptr i8, ptr addrspace(1) %367, i16 %365
  %369 = load i16, ptr addrspace(1) %368, !tbaa !14
  %370 = load float, ptr @$float131, !tbaa !2
  store i16 %369, ptr %26, !tbaa !2
  %371 = load i16, ptr %26, !tbaa !2
  %372 = sitofp i16 %371 to float
  %373 = fmul float %372, %370
  %374 = fpround float %373 to float
  store float %374, ptr @"YP!", !tbaa !2
  %375 = load i16, ptr @"OBJ%", !tbaa !2
  %376 = mul i16 %375, 2
  %377 = getelementptr i8, ptr @"ZPO%", i16 2
  %378 = load i16, ptr %377, !tbaa !2
  %379 = add i16 0, %376
  %380 = inttoptr i16 %378 to ptr addrspace(2)
  %381 = addrspacecast ptr addrspace(2) %380 to ptr addrspace(1)
  %382 = getelementptr i8, ptr addrspace(1) %381, i16 %379
  %383 = load i16, ptr addrspace(1) %382, !tbaa !14
  store i16 %383, ptr %25, !tbaa !2
  %384 = load i16, ptr %25, !tbaa !2
  %385 = sitofp i16 %384 to float
  %386 = fpround float %385 to float
  store float %386, ptr @"ZP!", !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  %387 = load i16, ptr %24, !tbaa !2
  %388 = sitofp i16 %387 to float
  %389 = fpround float %388 to float
  store float %389, ptr @"XC!", !tbaa !2
  store i16 0, ptr %23, !tbaa !2
  %390 = load i16, ptr %23, !tbaa !2
  %391 = sitofp i16 %390 to float
  %392 = fpround float %391 to float
  store float %392, ptr @"YC!", !tbaa !2
  store i16 0, ptr %22, !tbaa !2
  %393 = load i16, ptr %22, !tbaa !2
  %394 = sitofp i16 %393 to float
  %395 = fpround float %394 to float
  store float %395, ptr @"ZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string138, ptr %91)
  br label %b49

b51:
  br label %b52

b52:
  %396 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string139)
  %397 = icmp eq i16 %396, 0
  %398 = sext i1 %397 to i16
  %399 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string140)
  %400 = icmp eq i16 %399, 0
  %401 = sext i1 %400 to i16
  %402 = or i16 %398, %401
  %403 = icmp ne i16 %402, 0
  br i1 %403, label %b53, label %b54

b53:
  store i16 0, ptr %21, !tbaa !2
  %404 = load i16, ptr %21, !tbaa !2
  %405 = sitofp i16 %404 to float
  %406 = fpround float %405 to float
  store float %406, ptr @"XP!", !tbaa !2
  %407 = load i16, ptr @"OBJ%", !tbaa !2
  %408 = mul i16 %407, 2
  %409 = getelementptr i8, ptr @"ZPO%", i16 2
  %410 = load i16, ptr %409, !tbaa !2
  %411 = add i16 0, %408
  %412 = inttoptr i16 %410 to ptr addrspace(2)
  %413 = addrspacecast ptr addrspace(2) %412 to ptr addrspace(1)
  %414 = getelementptr i8, ptr addrspace(1) %413, i16 %411
  %415 = load i16, ptr addrspace(1) %414, !tbaa !14
  %416 = sub i16 0, %415
  %417 = load float, ptr @$float131, !tbaa !2
  store i16 %416, ptr %20, !tbaa !2
  %418 = load i16, ptr %20, !tbaa !2
  %419 = sitofp i16 %418 to float
  %420 = fmul float %419, %417
  %421 = fpround float %420 to float
  store float %421, ptr @"YP!", !tbaa !2
  %422 = load i16, ptr @"OBJ%", !tbaa !2
  %423 = mul i16 %422, 2
  %424 = getelementptr i8, ptr @"ZPO%", i16 2
  %425 = load i16, ptr %424, !tbaa !2
  %426 = add i16 0, %423
  %427 = inttoptr i16 %425 to ptr addrspace(2)
  %428 = addrspacecast ptr addrspace(2) %427 to ptr addrspace(1)
  %429 = getelementptr i8, ptr addrspace(1) %428, i16 %426
  %430 = load i16, ptr addrspace(1) %429, !tbaa !14
  store i16 %430, ptr %19, !tbaa !2
  %431 = load i16, ptr %19, !tbaa !2
  %432 = sitofp i16 %431 to float
  %433 = fpround float %432 to float
  store float %433, ptr @"ZP!", !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  %434 = load i16, ptr %18, !tbaa !2
  %435 = sitofp i16 %434 to float
  %436 = fpround float %435 to float
  store float %436, ptr @"XC!", !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  %437 = load i16, ptr %17, !tbaa !2
  %438 = sitofp i16 %437 to float
  %439 = fpround float %438 to float
  store float %439, ptr @"YC!", !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  %440 = load i16, ptr %16, !tbaa !2
  %441 = sitofp i16 %440 to float
  %442 = fpround float %441 to float
  store float %442, ptr @"ZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string141, ptr %91)
  br label %b52

b54:
  br label %b55

b55:
  %443 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string142)
  %444 = icmp eq i16 %443, 0
  %445 = sext i1 %444 to i16
  %446 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string143)
  %447 = icmp eq i16 %446, 0
  %448 = sext i1 %447 to i16
  %449 = or i16 %445, %448
  %450 = icmp ne i16 %449, 0
  br i1 %450, label %b56, label %b57

b56:
  store i16 0, ptr %15, !tbaa !2
  %451 = load i16, ptr %15, !tbaa !2
  %452 = sitofp i16 %451 to float
  %453 = fpround float %452 to float
  store float %453, ptr @"XP!", !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  %454 = load i16, ptr %14, !tbaa !2
  %455 = sitofp i16 %454 to float
  %456 = fpround float %455 to float
  store float %456, ptr @"YP!", !tbaa !2
  %457 = load i16, ptr @"OBJ%", !tbaa !2
  %458 = mul i16 %457, 2
  %459 = getelementptr i8, ptr @"ZPO%", i16 2
  %460 = load i16, ptr %459, !tbaa !2
  %461 = add i16 0, %458
  %462 = inttoptr i16 %460 to ptr addrspace(2)
  %463 = addrspacecast ptr addrspace(2) %462 to ptr addrspace(1)
  %464 = getelementptr i8, ptr addrspace(1) %463, i16 %461
  %465 = load i16, ptr addrspace(1) %464, !tbaa !14
  %466 = sub i16 0, %465
  %467 = sext i16 %466 to i32
  %468 = sdiv i32 %467, 2
  %469 = trunc i32 %468 to i16
  store i16 %469, ptr %13, !tbaa !2
  %470 = load i16, ptr %13, !tbaa !2
  %471 = sitofp i16 %470 to float
  %472 = fpround float %471 to float
  store float %472, ptr @"ZP!", !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  %473 = load i16, ptr %12, !tbaa !2
  %474 = sitofp i16 %473 to float
  %475 = fpround float %474 to float
  store float %475, ptr @"XC!", !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  %476 = load i16, ptr %11, !tbaa !2
  %477 = sitofp i16 %476 to float
  %478 = fpround float %477 to float
  store float %478, ptr @"YC!", !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %479 = load i16, ptr %10, !tbaa !2
  %480 = sitofp i16 %479 to float
  %481 = fpround float %480 to float
  store float %481, ptr @"ZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string144, ptr %91)
  br label %b55

b57:
  br label %b58

b58:
  %482 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string145)
  %483 = icmp eq i16 %482, 0
  %484 = sext i1 %483 to i16
  %485 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string146)
  %486 = icmp eq i16 %485, 0
  %487 = sext i1 %486 to i16
  %488 = or i16 %484, %487
  %489 = icmp ne i16 %488, 0
  br i1 %489, label %b59, label %b60

b59:
  %490 = load float, ptr @"YC!", !tbaa !2
  %491 = load float, ptr @$float147, !tbaa !2
  %492 = fadd float %490, %491
  %493 = fpround float %492 to float
  store float %493, ptr @"YC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string148, ptr %91)
  br label %b58

b60:
  br label %b61

b61:
  %494 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string149)
  %495 = icmp eq i16 %494, 0
  %496 = sext i1 %495 to i16
  %497 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string150)
  %498 = icmp eq i16 %497, 0
  %499 = sext i1 %498 to i16
  %500 = or i16 %496, %499
  %501 = icmp ne i16 %500, 0
  br i1 %501, label %b62, label %b63

b62:
  %502 = load float, ptr @"YC!", !tbaa !2
  %503 = load float, ptr @$float147, !tbaa !2
  %504 = fsub float %502, %503
  %505 = fpround float %504 to float
  store float %505, ptr @"YC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string151, ptr %91)
  br label %b61

b63:
  br label %b64

b64:
  %506 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string152)
  %507 = icmp eq i16 %506, 0
  %508 = sext i1 %507 to i16
  %509 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string153)
  %510 = icmp eq i16 %509, 0
  %511 = sext i1 %510 to i16
  %512 = or i16 %508, %511
  %513 = icmp ne i16 %512, 0
  br i1 %513, label %b65, label %b66

b65:
  %514 = load float, ptr @"XC!", !tbaa !2
  %515 = load float, ptr @$float147, !tbaa !2
  %516 = fsub float %514, %515
  %517 = fpround float %516 to float
  store float %517, ptr @"XC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string154, ptr %91)
  br label %b64

b66:
  br label %b67

b67:
  %518 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string155)
  %519 = icmp eq i16 %518, 0
  %520 = sext i1 %519 to i16
  %521 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string156)
  %522 = icmp eq i16 %521, 0
  %523 = sext i1 %522 to i16
  %524 = or i16 %520, %523
  %525 = icmp ne i16 %524, 0
  br i1 %525, label %b68, label %b69

b68:
  %526 = load float, ptr @"XC!", !tbaa !2
  %527 = load float, ptr @$float147, !tbaa !2
  %528 = fadd float %526, %527
  %529 = fpround float %528 to float
  store float %529, ptr @"XC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string157, ptr %91)
  br label %b67

b69:
  br label %b70

b70:
  %530 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string158)
  %531 = icmp eq i16 %530, 0
  br i1 %531, label %b71, label %b72

b71:
  %532 = load float, ptr @"ZC!", !tbaa !2
  %533 = load float, ptr @$float159, !tbaa !2
  %534 = fadd float %532, %533
  %535 = fpround float %534 to float
  store float %535, ptr @"ZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string160, ptr %91)
  br label %b70

b72:
  br label %b73

b73:
  %536 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string161)
  %537 = icmp eq i16 %536, 0
  %538 = sext i1 %537 to i16
  %539 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string162)
  %540 = icmp eq i16 %539, 0
  %541 = sext i1 %540 to i16
  %542 = or i16 %538, %541
  %543 = icmp ne i16 %542, 0
  br i1 %543, label %b74, label %b75

b74:
  %544 = load float, ptr @"ZC!", !tbaa !2
  %545 = load float, ptr @$float159, !tbaa !2
  %546 = fsub float %544, %545
  %547 = fpround float %546 to float
  store float %547, ptr @"ZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string163, ptr %91)
  br label %b73

b75:
  br label %b76

b76:
  %548 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string164)
  %549 = icmp eq i16 %548, 0
  %550 = sext i1 %549 to i16
  %551 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string165)
  %552 = icmp eq i16 %551, 0
  %553 = sext i1 %552 to i16
  %554 = or i16 %550, %553
  %555 = icmp ne i16 %554, 0
  br i1 %555, label %b77, label %b78

b77:
  store i16 0, ptr %8, !tbaa !2
  %556 = load i16, ptr %8, !tbaa !2
  %557 = sitofp i16 %556 to float
  %558 = fpround float %557 to float
  store float %558, ptr %9, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %559 = load i16, ptr %6, !tbaa !2
  %560 = sitofp i16 %559 to float
  %561 = fpround float %560 to float
  store float %561, ptr %7, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  %562 = load i16, ptr %4, !tbaa !2
  %563 = sitofp i16 %562 to float
  %564 = fpround float %563 to float
  store float %564, ptr %5, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  %565 = load i16, ptr %3, !tbaa !2
  %566 = sitofp i16 %565 to float
  %567 = fpround float %566 to float
  store float %567, ptr @"RXC!", !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %568 = load i16, ptr %2, !tbaa !2
  %569 = sitofp i16 %568 to float
  %570 = fpround float %569 to float
  store float %570, ptr @"RYC!", !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  %571 = load i16, ptr %1, !tbaa !2
  %572 = sitofp i16 %571 to float
  %573 = fpround float %572 to float
  store float %573, ptr @"RZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string166, ptr %91)
  br label %b76

b78:
  br label %b79

b79:
  %574 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string167)
  %575 = icmp eq i16 %574, 0
  %576 = sext i1 %575 to i16
  %577 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string168)
  %578 = icmp eq i16 %577, 0
  %579 = sext i1 %578 to i16
  %580 = or i16 %576, %579
  %581 = icmp ne i16 %580, 0
  br i1 %581, label %b80, label %b81

b80:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b81:
  br label %b83

b83:
  %582 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string169)
  %583 = icmp eq i16 %582, 0
  br i1 %583, label %b84, label %b85

b84:
  %584 = load float, ptr @"RYC!", !tbaa !2
  %585 = load float, ptr @$float170, !tbaa !2
  %586 = fsub float %584, %585
  %587 = fpround float %586 to float
  store float %587, ptr @"RYC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string171, ptr %91)
  br label %b83

b85:
  br label %b86

b86:
  %588 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string172)
  %589 = icmp eq i16 %588, 0
  br i1 %589, label %b87, label %b88

b87:
  %590 = load float, ptr @"RYC!", !tbaa !2
  %591 = load float, ptr @$float170, !tbaa !2
  %592 = fadd float %590, %591
  %593 = fpround float %592 to float
  store float %593, ptr @"RYC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string173, ptr %91)
  br label %b86

b88:
  br label %b89

b89:
  %594 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string174)
  %595 = icmp eq i16 %594, 0
  br i1 %595, label %b90, label %b91

b90:
  %596 = load float, ptr @"RXC!", !tbaa !2
  %597 = load float, ptr @$float170, !tbaa !2
  %598 = fsub float %596, %597
  %599 = fpround float %598 to float
  store float %599, ptr @"RXC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string175, ptr %91)
  br label %b89

b91:
  br label %b92

b92:
  %600 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string176)
  %601 = icmp eq i16 %600, 0
  br i1 %601, label %b93, label %b94

b93:
  %602 = load float, ptr @"RXC!", !tbaa !2
  %603 = load float, ptr @$float170, !tbaa !2
  %604 = fadd float %602, %603
  %605 = fpround float %604 to float
  store float %605, ptr @"RXC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string177, ptr %91)
  br label %b92

b94:
  br label %b95

b95:
  %606 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string178)
  %607 = icmp eq i16 %606, 0
  br i1 %607, label %b96, label %b97

b96:
  %608 = load float, ptr @"RZC!", !tbaa !2
  %609 = load float, ptr @$float170, !tbaa !2
  %610 = fsub float %608, %609
  %611 = fpround float %610 to float
  store float %611, ptr @"RZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string179, ptr %91)
  br label %b95

b97:
  br label %b98

b98:
  %612 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string180)
  %613 = icmp eq i16 %612, 0
  br i1 %613, label %b99, label %b100

b99:
  %614 = load float, ptr @"RZC!", !tbaa !2
  %615 = load float, ptr @$float170, !tbaa !2
  %616 = fadd float %614, %615
  %617 = fpround float %616 to float
  store float %617, ptr @"RZC!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string181, ptr %91)
  br label %b98

b100:
  br label %b101

b101:
  %618 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string182)
  %619 = icmp eq i16 %618, 0
  %620 = sext i1 %619 to i16
  %621 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string183)
  %622 = icmp eq i16 %621, 0
  %623 = sext i1 %622 to i16
  %624 = or i16 %620, %623
  %625 = icmp ne i16 %624, 0
  br i1 %625, label %b102, label %b103

b102:
  store i16 1, ptr %0, !tbaa !2
  call cc 1000 addrspace(1) void @CLEARSCREEN(ptr %0)
  store i16 0, ptr @"FTYPE%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string184, ptr %91)
  br label %b101

b103:
  br label %b104

b104:
  %626 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string185)
  %627 = icmp eq i16 %626, 0
  %628 = sext i1 %627 to i16
  %629 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string186)
  %630 = icmp eq i16 %629, 0
  %631 = sext i1 %630 to i16
  %632 = or i16 %628, %631
  %633 = icmp ne i16 %632, 0
  br i1 %633, label %b105, label %b106

b105:
  store i16 1, ptr @"FTYPE%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string187, ptr %91)
  br label %b104

b106:
  br label %b107

b107:
  %634 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string188)
  %635 = icmp eq i16 %634, 0
  %636 = sext i1 %635 to i16
  %637 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %91, ptr @$string189)
  %638 = icmp eq i16 %637, 0
  %639 = sext i1 %638 to i16
  %640 = or i16 %636, %639
  %641 = icmp ne i16 %640, 0
  br i1 %641, label %b108, label %b109

b108:
  store i16 2, ptr @"FTYPE%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string190, ptr %91)
  br label %b107

b109:
  %642 = load float, ptr @"XP!", !tbaa !2
  %643 = load float, ptr @"XC!", !tbaa !2
  %644 = fadd float %642, %643
  %645 = fpround float %644 to float
  store float %645, ptr @"XP!", !tbaa !2
  %646 = load float, ptr @"YP!", !tbaa !2
  %647 = load float, ptr @"YC!", !tbaa !2
  %648 = fadd float %646, %647
  %649 = fpround float %648 to float
  store float %649, ptr @"YP!", !tbaa !2
  %650 = load float, ptr @"ZP!", !tbaa !2
  %651 = load float, ptr @"ZC!", !tbaa !2
  %652 = fadd float %650, %651
  %653 = fpround float %652 to float
  store float %653, ptr @"ZP!", !tbaa !2
  %654 = load float, ptr %9, !tbaa !2
  %655 = load float, ptr @"RXC!", !tbaa !2
  %656 = fadd float %654, %655
  %657 = fpround float %656 to float
  store float %657, ptr %9, !tbaa !2
  %658 = load float, ptr %7, !tbaa !2
  %659 = load float, ptr @"RYC!", !tbaa !2
  %660 = fadd float %658, %659
  %661 = fpround float %660 to float
  store float %661, ptr %7, !tbaa !2
  %662 = load float, ptr %5, !tbaa !2
  %663 = load float, ptr @"RZC!", !tbaa !2
  %664 = fadd float %662, %663
  %665 = fpround float %664 to float
  store float %665, ptr %5, !tbaa !2
  %666 = load i16, ptr @"OBJ%", !tbaa !2
  %667 = icmp eq i16 %666, 4
  br i1 %667, label %b110, label %b111

b110:
  call cc 1000 addrspace(1) void @ANIMATEWAVELET(ptr @"K%")
  %668 = load i16, ptr @"K%", !tbaa !2
  %669 = add i16 %668, 1
  store i16 %669, ptr @"K%", !tbaa !2
  %670 = load i16, ptr @"K%", !tbaa !2
  %671 = icmp eq i16 %670, 314
  br i1 %671, label %b113, label %b114

b111:
  br label %b112

b112:
  call cc 1000 addrspace(1) void @ROTATE3D(ptr %9, ptr %7, ptr %5)
  call cc 1000 addrspace(1) void @MOV3DPOS(ptr @"XP!", ptr @"YP!", ptr @"ZP!")
  call cc 1000 addrspace(1) void @TRANSLATE3D()
  call cc 1000 addrspace(1) void @OUTPUT3D()
  %672 = load i16, ptr @"FILEI%", !tbaa !2
  %673 = icmp eq i16 %672, 444
  br i1 %673, label %b116, label %b117

b113:
  store i16 0, ptr @"K%", !tbaa !2
  br label %b115

b114:
  br label %b115

b115:
  br label %b112

b116:
  br label %b2

b117:
  br label %b118

b118:
  %674 = load i16, ptr @"XIT%", !tbaa !2
  %675 = icmp eq i16 %674, 0
  br i1 %675, label %b120, label %b121

b120:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b122

b121:
  br label %b122

b122:
  %676 = load i16, ptr @"XIT%", !tbaa !2
  %677 = icmp eq i16 %676, 1
  br i1 %677, label %b123, label %b124

b123:
  br label %b2

b124:
  br label %b125

b125:
  br label %b4
}

define cc 1000 void @ANIMATEWAVELET(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  store i16 0, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i16 511, ptr %5, !tbaa !2
  store i16 8, ptr %4, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %4, !tbaa !2
  %9 = icmp sge i16 %8, 0
  br i1 %9, label %b3, label %b4

b3:
  %10 = load i16, ptr %6, !tbaa !2
  %11 = load i16, ptr %5, !tbaa !2
  %12 = icmp sle i16 %10, %11
  br i1 %12, label %b5, label %b6

b4:
  %13 = load i16, ptr %6, !tbaa !2
  %14 = load i16, ptr %5, !tbaa !2
  %15 = icmp sge i16 %13, %14
  br i1 %15, label %b5, label %b6

b5:
  store i16 0, ptr %3, !tbaa !2
  store i16 511, ptr %2, !tbaa !2
  store i16 8, ptr %1, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = icmp sge i16 %16, 0
  br i1 %17, label %b8, label %b9

b8:
  %18 = load i16, ptr %3, !tbaa !2
  %19 = load i16, ptr %2, !tbaa !2
  %20 = icmp sle i16 %18, %19
  br i1 %20, label %b10, label %b11

b9:
  %21 = load i16, ptr %3, !tbaa !2
  %22 = load i16, ptr %2, !tbaa !2
  %23 = icmp sge i16 %21, %22
  br i1 %23, label %b10, label %b11

b10:
  %24 = load i16, ptr %7, !tbaa !2
  %25 = mul i16 %24, 2
  %26 = getelementptr i8, ptr @"XO%", i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = add i16 0, %25
  %29 = inttoptr i16 %27 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %28
  %32 = load i16, ptr %3, !tbaa !2
  %33 = sub i16 %32, 256
  store i16 %33, ptr addrspace(1) %31, !tbaa !16
  %34 = load i16, ptr %7, !tbaa !2
  %35 = mul i16 %34, 2
  %36 = getelementptr i8, ptr @"ZO%", i16 2
  %37 = load i16, ptr %36, !tbaa !2
  %38 = add i16 0, %35
  %39 = inttoptr i16 %37 to ptr addrspace(2)
  %40 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 %38
  %42 = load i16, ptr %6, !tbaa !2
  %43 = sub i16 %42, 256
  store i16 %43, ptr addrspace(1) %41, !tbaa !18
  %44 = load i16, ptr %7, !tbaa !2
  %45 = mul i16 %44, 2
  %46 = getelementptr i8, ptr @"YO%", i16 2
  %47 = load i16, ptr %46, !tbaa !2
  %48 = add i16 0, %45
  %49 = inttoptr i16 %47 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = getelementptr i8, ptr addrspace(1) %50, i16 %48
  %52 = load i16, ptr %3, !tbaa !2
  %53 = load i16, ptr %0
  %54 = add i16 %52, %53
  %55 = sub i16 %54, -48
  %56 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %55
  %57 = load i16, ptr %56, !tbaa !2
  %58 = load i16, ptr %6, !tbaa !2
  %59 = load i16, ptr %0
  %60 = add i16 %58, %59
  %61 = sub i16 %60, -48
  %62 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %61
  %63 = load i16, ptr %62, !tbaa !2
  %64 = add i16 %57, %63
  store i16 %64, ptr addrspace(1) %51, !tbaa !20
  %65 = load i16, ptr %7, !tbaa !2
  %66 = add i16 %65, 1
  store i16 %66, ptr %7, !tbaa !2
  %67 = load i16, ptr %3, !tbaa !2
  %68 = load i16, ptr %1, !tbaa !2
  %69 = add nsw i16 %67, %68
  store i16 %69, ptr %3, !tbaa !2
  br label %b7

b11:
  %70 = load i16, ptr %6, !tbaa !2
  %71 = load i16, ptr %4, !tbaa !2
  %72 = add nsw i16 %70, %71
  store i16 %72, ptr %6, !tbaa !2
  br label %b2
}

define cc 1000 void @CLEARSCREEN(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  store i16 199, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %6, !tbaa !2
  %10 = load i16, ptr %5, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %6, !tbaa !2
  %13 = load i16, ptr %5, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i16, ptr %6, !tbaa !2
  %16 = mul i16 %15, 20
  %17 = add i16 -24576, %16
  store i16 %17, ptr @b$seg, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 319, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  ret void

b7:
  %18 = load i16, ptr %1, !tbaa !2
  %19 = icmp sge i16 %18, 0
  br i1 %19, label %b8, label %b9

b8:
  %20 = load i16, ptr %3, !tbaa !2
  %21 = load i16, ptr %2, !tbaa !2
  %22 = icmp sle i16 %20, %21
  br i1 %22, label %b10, label %b11

b9:
  %23 = load i16, ptr %3, !tbaa !2
  %24 = load i16, ptr %2, !tbaa !2
  %25 = icmp sge i16 %23, %24
  br i1 %25, label %b10, label %b11

b10:
  %26 = load i16, ptr %3, !tbaa !2
  %27 = load i16, ptr %0
  %28 = trunc i16 %27 to i8
  %29 = load i16, ptr @b$seg, !tbaa !2
  %30 = inttoptr i16 %29 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %26
  store volatile i8 %28, ptr addrspace(1) %32
  %33 = load i16, ptr %3, !tbaa !2
  %34 = load i16, ptr %1, !tbaa !2
  %35 = add nsw i16 %33, %34
  store i16 %35, ptr %3, !tbaa !2
  br label %b7

b11:
  %36 = load i16, ptr %6, !tbaa !2
  %37 = load i16, ptr %4, !tbaa !2
  %38 = add nsw i16 %36, %37
  store i16 %38, ptr %6, !tbaa !2
  br label %b2
}

define cc 1000 void @COPPER() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca i16
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca i16
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca i16
  %79 = alloca i16
  %80 = alloca i16
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca i16
  %84 = alloca i16
  %85 = alloca i16
  %86 = alloca i16
  %87 = alloca i16
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca i16
  %91 = alloca i16
  %92 = alloca i16
  %93 = alloca i16
  %94 = alloca i16
  %95 = alloca i16
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  %103 = alloca i16
  %104 = alloca i16
  %105 = alloca i16
  %106 = alloca i16
  %107 = alloca i16
  %108 = alloca i16
  %109 = alloca i16
  %110 = alloca i16
  %111 = alloca i16
  %112 = alloca i16
  %113 = alloca i16
  %114 = alloca i16
  %115 = alloca i16
  %116 = alloca i16
  %117 = alloca i16
  %118 = alloca i16
  %119 = alloca i16
  %120 = alloca i16
  %121 = alloca i16
  %122 = alloca i16
  %123 = alloca i16
  %124 = alloca i16
  %125 = alloca i16
  %126 = alloca i16
  %127 = alloca i16
  %128 = alloca i16
  %129 = alloca i16
  %130 = alloca i16
  %131 = alloca i16
  %132 = alloca i16
  %133 = alloca i16
  %134 = alloca i16
  %135 = alloca i16
  %136 = alloca [18 x i8]
  %137 = alloca [18 x i8]
  %138 = alloca [18 x i8]
  %139 = alloca [18 x i8]
  %140 = alloca [18 x i8]
  %141 = alloca [18 x i8]
  %142 = alloca [18 x i8]
  %143 = alloca [18 x i8]
  %144 = alloca [18 x i8]
  %145 = alloca [18 x i8]
  %146 = alloca [22 x i8]
  %147 = alloca [18 x i8]
  %148 = alloca [18 x i8]
  %149 = alloca [18 x i8]
  %150 = alloca [18 x i8]
  %151 = alloca [18 x i8]
  store i16 0, ptr %151, !tbaa !2
  %152 = getelementptr inbounds i8, ptr %151, i16 2
  store i16 0, ptr %152, !tbaa !2
  %153 = getelementptr inbounds i8, ptr %151, i16 4
  store i16 0, ptr %153, !tbaa !2
  %154 = getelementptr inbounds i8, ptr %151, i16 6
  store i16 0, ptr %154, !tbaa !2
  %155 = getelementptr inbounds i8, ptr %151, i16 8
  store i16 0, ptr %155, !tbaa !2
  %156 = getelementptr inbounds i8, ptr %151, i16 10
  store i16 0, ptr %156, !tbaa !2
  %157 = getelementptr inbounds i8, ptr %151, i16 12
  store i16 0, ptr %157, !tbaa !2
  %158 = getelementptr inbounds i8, ptr %151, i16 14
  store i16 0, ptr %158, !tbaa !2
  %159 = getelementptr inbounds i8, ptr %151, i16 16
  store i16 0, ptr %159, !tbaa !2
  store i16 0, ptr %150, !tbaa !2
  %160 = getelementptr inbounds i8, ptr %150, i16 2
  store i16 0, ptr %160, !tbaa !2
  %161 = getelementptr inbounds i8, ptr %150, i16 4
  store i16 0, ptr %161, !tbaa !2
  %162 = getelementptr inbounds i8, ptr %150, i16 6
  store i16 0, ptr %162, !tbaa !2
  %163 = getelementptr inbounds i8, ptr %150, i16 8
  store i16 0, ptr %163, !tbaa !2
  %164 = getelementptr inbounds i8, ptr %150, i16 10
  store i16 0, ptr %164, !tbaa !2
  %165 = getelementptr inbounds i8, ptr %150, i16 12
  store i16 0, ptr %165, !tbaa !2
  %166 = getelementptr inbounds i8, ptr %150, i16 14
  store i16 0, ptr %166, !tbaa !2
  %167 = getelementptr inbounds i8, ptr %150, i16 16
  store i16 0, ptr %167, !tbaa !2
  store i16 0, ptr %149, !tbaa !2
  %168 = getelementptr inbounds i8, ptr %149, i16 2
  store i16 0, ptr %168, !tbaa !2
  %169 = getelementptr inbounds i8, ptr %149, i16 4
  store i16 0, ptr %169, !tbaa !2
  %170 = getelementptr inbounds i8, ptr %149, i16 6
  store i16 0, ptr %170, !tbaa !2
  %171 = getelementptr inbounds i8, ptr %149, i16 8
  store i16 0, ptr %171, !tbaa !2
  %172 = getelementptr inbounds i8, ptr %149, i16 10
  store i16 0, ptr %172, !tbaa !2
  %173 = getelementptr inbounds i8, ptr %149, i16 12
  store i16 0, ptr %173, !tbaa !2
  %174 = getelementptr inbounds i8, ptr %149, i16 14
  store i16 0, ptr %174, !tbaa !2
  %175 = getelementptr inbounds i8, ptr %149, i16 16
  store i16 0, ptr %175, !tbaa !2
  store i16 0, ptr %148, !tbaa !2
  %176 = getelementptr inbounds i8, ptr %148, i16 2
  store i16 0, ptr %176, !tbaa !2
  %177 = getelementptr inbounds i8, ptr %148, i16 4
  store i16 0, ptr %177, !tbaa !2
  %178 = getelementptr inbounds i8, ptr %148, i16 6
  store i16 0, ptr %178, !tbaa !2
  %179 = getelementptr inbounds i8, ptr %148, i16 8
  store i16 0, ptr %179, !tbaa !2
  %180 = getelementptr inbounds i8, ptr %148, i16 10
  store i16 0, ptr %180, !tbaa !2
  %181 = getelementptr inbounds i8, ptr %148, i16 12
  store i16 0, ptr %181, !tbaa !2
  %182 = getelementptr inbounds i8, ptr %148, i16 14
  store i16 0, ptr %182, !tbaa !2
  %183 = getelementptr inbounds i8, ptr %148, i16 16
  store i16 0, ptr %183, !tbaa !2
  store i16 0, ptr %147, !tbaa !2
  %184 = getelementptr inbounds i8, ptr %147, i16 2
  store i16 0, ptr %184, !tbaa !2
  %185 = getelementptr inbounds i8, ptr %147, i16 4
  store i16 0, ptr %185, !tbaa !2
  %186 = getelementptr inbounds i8, ptr %147, i16 6
  store i16 0, ptr %186, !tbaa !2
  %187 = getelementptr inbounds i8, ptr %147, i16 8
  store i16 0, ptr %187, !tbaa !2
  %188 = getelementptr inbounds i8, ptr %147, i16 10
  store i16 0, ptr %188, !tbaa !2
  %189 = getelementptr inbounds i8, ptr %147, i16 12
  store i16 0, ptr %189, !tbaa !2
  %190 = getelementptr inbounds i8, ptr %147, i16 14
  store i16 0, ptr %190, !tbaa !2
  %191 = getelementptr inbounds i8, ptr %147, i16 16
  store i16 0, ptr %191, !tbaa !2
  store i16 0, ptr %146, !tbaa !2
  %192 = getelementptr inbounds i8, ptr %146, i16 2
  store i16 0, ptr %192, !tbaa !2
  %193 = getelementptr inbounds i8, ptr %146, i16 4
  store i16 0, ptr %193, !tbaa !2
  %194 = getelementptr inbounds i8, ptr %146, i16 6
  store i16 0, ptr %194, !tbaa !2
  %195 = getelementptr inbounds i8, ptr %146, i16 8
  store i16 0, ptr %195, !tbaa !2
  %196 = getelementptr inbounds i8, ptr %146, i16 10
  store i16 0, ptr %196, !tbaa !2
  %197 = getelementptr inbounds i8, ptr %146, i16 12
  store i16 0, ptr %197, !tbaa !2
  %198 = getelementptr inbounds i8, ptr %146, i16 14
  store i16 0, ptr %198, !tbaa !2
  %199 = getelementptr inbounds i8, ptr %146, i16 16
  store i16 0, ptr %199, !tbaa !2
  %200 = getelementptr inbounds i8, ptr %146, i16 18
  store i16 0, ptr %200, !tbaa !2
  %201 = getelementptr inbounds i8, ptr %146, i16 20
  store i16 0, ptr %201, !tbaa !2
  store i16 0, ptr %145, !tbaa !2
  %202 = getelementptr inbounds i8, ptr %145, i16 2
  store i16 0, ptr %202, !tbaa !2
  %203 = getelementptr inbounds i8, ptr %145, i16 4
  store i16 0, ptr %203, !tbaa !2
  %204 = getelementptr inbounds i8, ptr %145, i16 6
  store i16 0, ptr %204, !tbaa !2
  %205 = getelementptr inbounds i8, ptr %145, i16 8
  store i16 0, ptr %205, !tbaa !2
  %206 = getelementptr inbounds i8, ptr %145, i16 10
  store i16 0, ptr %206, !tbaa !2
  %207 = getelementptr inbounds i8, ptr %145, i16 12
  store i16 0, ptr %207, !tbaa !2
  %208 = getelementptr inbounds i8, ptr %145, i16 14
  store i16 0, ptr %208, !tbaa !2
  %209 = getelementptr inbounds i8, ptr %145, i16 16
  store i16 0, ptr %209, !tbaa !2
  store i16 0, ptr %144, !tbaa !2
  %210 = getelementptr inbounds i8, ptr %144, i16 2
  store i16 0, ptr %210, !tbaa !2
  %211 = getelementptr inbounds i8, ptr %144, i16 4
  store i16 0, ptr %211, !tbaa !2
  %212 = getelementptr inbounds i8, ptr %144, i16 6
  store i16 0, ptr %212, !tbaa !2
  %213 = getelementptr inbounds i8, ptr %144, i16 8
  store i16 0, ptr %213, !tbaa !2
  %214 = getelementptr inbounds i8, ptr %144, i16 10
  store i16 0, ptr %214, !tbaa !2
  %215 = getelementptr inbounds i8, ptr %144, i16 12
  store i16 0, ptr %215, !tbaa !2
  %216 = getelementptr inbounds i8, ptr %144, i16 14
  store i16 0, ptr %216, !tbaa !2
  %217 = getelementptr inbounds i8, ptr %144, i16 16
  store i16 0, ptr %217, !tbaa !2
  store i16 0, ptr %143, !tbaa !2
  %218 = getelementptr inbounds i8, ptr %143, i16 2
  store i16 0, ptr %218, !tbaa !2
  %219 = getelementptr inbounds i8, ptr %143, i16 4
  store i16 0, ptr %219, !tbaa !2
  %220 = getelementptr inbounds i8, ptr %143, i16 6
  store i16 0, ptr %220, !tbaa !2
  %221 = getelementptr inbounds i8, ptr %143, i16 8
  store i16 0, ptr %221, !tbaa !2
  %222 = getelementptr inbounds i8, ptr %143, i16 10
  store i16 0, ptr %222, !tbaa !2
  %223 = getelementptr inbounds i8, ptr %143, i16 12
  store i16 0, ptr %223, !tbaa !2
  %224 = getelementptr inbounds i8, ptr %143, i16 14
  store i16 0, ptr %224, !tbaa !2
  %225 = getelementptr inbounds i8, ptr %143, i16 16
  store i16 0, ptr %225, !tbaa !2
  store i16 0, ptr %142, !tbaa !2
  %226 = getelementptr inbounds i8, ptr %142, i16 2
  store i16 0, ptr %226, !tbaa !2
  %227 = getelementptr inbounds i8, ptr %142, i16 4
  store i16 0, ptr %227, !tbaa !2
  %228 = getelementptr inbounds i8, ptr %142, i16 6
  store i16 0, ptr %228, !tbaa !2
  %229 = getelementptr inbounds i8, ptr %142, i16 8
  store i16 0, ptr %229, !tbaa !2
  %230 = getelementptr inbounds i8, ptr %142, i16 10
  store i16 0, ptr %230, !tbaa !2
  %231 = getelementptr inbounds i8, ptr %142, i16 12
  store i16 0, ptr %231, !tbaa !2
  %232 = getelementptr inbounds i8, ptr %142, i16 14
  store i16 0, ptr %232, !tbaa !2
  %233 = getelementptr inbounds i8, ptr %142, i16 16
  store i16 0, ptr %233, !tbaa !2
  store i16 0, ptr %141, !tbaa !2
  %234 = getelementptr inbounds i8, ptr %141, i16 2
  store i16 0, ptr %234, !tbaa !2
  %235 = getelementptr inbounds i8, ptr %141, i16 4
  store i16 0, ptr %235, !tbaa !2
  %236 = getelementptr inbounds i8, ptr %141, i16 6
  store i16 0, ptr %236, !tbaa !2
  %237 = getelementptr inbounds i8, ptr %141, i16 8
  store i16 0, ptr %237, !tbaa !2
  %238 = getelementptr inbounds i8, ptr %141, i16 10
  store i16 0, ptr %238, !tbaa !2
  %239 = getelementptr inbounds i8, ptr %141, i16 12
  store i16 0, ptr %239, !tbaa !2
  %240 = getelementptr inbounds i8, ptr %141, i16 14
  store i16 0, ptr %240, !tbaa !2
  %241 = getelementptr inbounds i8, ptr %141, i16 16
  store i16 0, ptr %241, !tbaa !2
  store i16 0, ptr %140, !tbaa !2
  %242 = getelementptr inbounds i8, ptr %140, i16 2
  store i16 0, ptr %242, !tbaa !2
  %243 = getelementptr inbounds i8, ptr %140, i16 4
  store i16 0, ptr %243, !tbaa !2
  %244 = getelementptr inbounds i8, ptr %140, i16 6
  store i16 0, ptr %244, !tbaa !2
  %245 = getelementptr inbounds i8, ptr %140, i16 8
  store i16 0, ptr %245, !tbaa !2
  %246 = getelementptr inbounds i8, ptr %140, i16 10
  store i16 0, ptr %246, !tbaa !2
  %247 = getelementptr inbounds i8, ptr %140, i16 12
  store i16 0, ptr %247, !tbaa !2
  %248 = getelementptr inbounds i8, ptr %140, i16 14
  store i16 0, ptr %248, !tbaa !2
  %249 = getelementptr inbounds i8, ptr %140, i16 16
  store i16 0, ptr %249, !tbaa !2
  store i16 0, ptr %139, !tbaa !2
  %250 = getelementptr inbounds i8, ptr %139, i16 2
  store i16 0, ptr %250, !tbaa !2
  %251 = getelementptr inbounds i8, ptr %139, i16 4
  store i16 0, ptr %251, !tbaa !2
  %252 = getelementptr inbounds i8, ptr %139, i16 6
  store i16 0, ptr %252, !tbaa !2
  %253 = getelementptr inbounds i8, ptr %139, i16 8
  store i16 0, ptr %253, !tbaa !2
  %254 = getelementptr inbounds i8, ptr %139, i16 10
  store i16 0, ptr %254, !tbaa !2
  %255 = getelementptr inbounds i8, ptr %139, i16 12
  store i16 0, ptr %255, !tbaa !2
  %256 = getelementptr inbounds i8, ptr %139, i16 14
  store i16 0, ptr %256, !tbaa !2
  %257 = getelementptr inbounds i8, ptr %139, i16 16
  store i16 0, ptr %257, !tbaa !2
  store i16 0, ptr %138, !tbaa !2
  %258 = getelementptr inbounds i8, ptr %138, i16 2
  store i16 0, ptr %258, !tbaa !2
  %259 = getelementptr inbounds i8, ptr %138, i16 4
  store i16 0, ptr %259, !tbaa !2
  %260 = getelementptr inbounds i8, ptr %138, i16 6
  store i16 0, ptr %260, !tbaa !2
  %261 = getelementptr inbounds i8, ptr %138, i16 8
  store i16 0, ptr %261, !tbaa !2
  %262 = getelementptr inbounds i8, ptr %138, i16 10
  store i16 0, ptr %262, !tbaa !2
  %263 = getelementptr inbounds i8, ptr %138, i16 12
  store i16 0, ptr %263, !tbaa !2
  %264 = getelementptr inbounds i8, ptr %138, i16 14
  store i16 0, ptr %264, !tbaa !2
  %265 = getelementptr inbounds i8, ptr %138, i16 16
  store i16 0, ptr %265, !tbaa !2
  store i16 0, ptr %137, !tbaa !2
  %266 = getelementptr inbounds i8, ptr %137, i16 2
  store i16 0, ptr %266, !tbaa !2
  %267 = getelementptr inbounds i8, ptr %137, i16 4
  store i16 0, ptr %267, !tbaa !2
  %268 = getelementptr inbounds i8, ptr %137, i16 6
  store i16 0, ptr %268, !tbaa !2
  %269 = getelementptr inbounds i8, ptr %137, i16 8
  store i16 0, ptr %269, !tbaa !2
  %270 = getelementptr inbounds i8, ptr %137, i16 10
  store i16 0, ptr %270, !tbaa !2
  %271 = getelementptr inbounds i8, ptr %137, i16 12
  store i16 0, ptr %271, !tbaa !2
  %272 = getelementptr inbounds i8, ptr %137, i16 14
  store i16 0, ptr %272, !tbaa !2
  %273 = getelementptr inbounds i8, ptr %137, i16 16
  store i16 0, ptr %273, !tbaa !2
  store i16 0, ptr %136, !tbaa !2
  %274 = getelementptr inbounds i8, ptr %136, i16 2
  store i16 0, ptr %274, !tbaa !2
  %275 = getelementptr inbounds i8, ptr %136, i16 4
  store i16 0, ptr %275, !tbaa !2
  %276 = getelementptr inbounds i8, ptr %136, i16 6
  store i16 0, ptr %276, !tbaa !2
  %277 = getelementptr inbounds i8, ptr %136, i16 8
  store i16 0, ptr %277, !tbaa !2
  %278 = getelementptr inbounds i8, ptr %136, i16 10
  store i16 0, ptr %278, !tbaa !2
  %279 = getelementptr inbounds i8, ptr %136, i16 12
  store i16 0, ptr %279, !tbaa !2
  %280 = getelementptr inbounds i8, ptr %136, i16 14
  store i16 0, ptr %280, !tbaa !2
  %281 = getelementptr inbounds i8, ptr %136, i16 16
  store i16 0, ptr %281, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 2, i16 1, i16 32, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string191)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 4, i16 1, i16 2, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string192)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 5, i16 1, i16 2, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string193)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(i16 0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string194)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string195)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string196)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(i16 0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string197)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string198)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string199)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string200)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string201)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string202)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string203)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string204)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string205)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string206)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(i16 0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string207)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string208)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string209)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 32, i16 2, i16 257, ptr %151)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 319, i16 2, i16 257, ptr %150)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 32, i16 2, i16 257, ptr %149)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 32, i16 2, i16 257, ptr %148)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32, i16 2, i16 257, ptr %147)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 15, i16 0, i16 15, i16 2, i16 258, ptr %146)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -640, i16 640, i16 2, i16 257, ptr %145)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -640, i16 640, i16 2, i16 257, ptr %144)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 399, i16 2, i16 257, ptr %143)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 399, i16 2, i16 257, ptr %142)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 3, i16 2, i16 257, ptr %141)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -200, i16 500, i16 2, i16 257, ptr %140)
  store i16 0, ptr %135, !tbaa !2
  store i16 168, ptr %134, !tbaa !2
  store i16 200, ptr %133, !tbaa !2
  store i16 1, ptr %132, !tbaa !2
  br label %b5

b2:
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  %282 = load i16, ptr %30, !tbaa !2
  store i16 %282, ptr %25, !tbaa !2
  store i16 1, ptr %24, !tbaa !2
  br label %b204

b3:
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 0, ptr %134, !tbaa !2
  store i16 7, ptr %13, !tbaa !2
  store i16 1, ptr %12, !tbaa !2
  br label %b224

b4:
  call void @llrm.ia16.out.i8(i16 980, i8 40)
  call void @llrm.ia16.out.i8(i16 981, i8 19)
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 1, i16 2)
  call void @llrm.ia16.out.i8(i16 980, i8 40)
  call void @llrm.ia16.out.i8(i16 981, i8 19)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %145)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %144)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %140)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %151)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %138)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %137)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %136)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %149)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %141)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %142)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %139)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %148)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %146)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %150)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %147)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %143)
  ret void

b5:
  %283 = load i16, ptr %132, !tbaa !2
  %284 = icmp sge i16 %283, 0
  br i1 %284, label %b6, label %b7

b6:
  %285 = load i16, ptr %134, !tbaa !2
  %286 = load i16, ptr %133, !tbaa !2
  %287 = icmp sle i16 %285, %286
  br i1 %287, label %b8, label %b9

b7:
  %288 = load i16, ptr %134, !tbaa !2
  %289 = load i16, ptr %133, !tbaa !2
  %290 = icmp sge i16 %288, %289
  br i1 %290, label %b8, label %b9

b8:
  %291 = load i16, ptr %135, !tbaa !2
  %292 = icmp sgt i16 %291, 63
  br i1 %292, label %b10, label %b11

b9:
  store i16 201, ptr %134, !tbaa !2
  store i16 232, ptr %131, !tbaa !2
  store i16 1, ptr %130, !tbaa !2
  br label %b13

b10:
  store i16 63, ptr %135, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  %293 = load i16, ptr %134, !tbaa !2
  %294 = mul i16 %293, 2
  %295 = getelementptr i8, ptr %140, i16 2
  %296 = load i16, ptr %295, !tbaa !2
  %297 = add i16 400, %294
  %298 = inttoptr i16 %296 to ptr addrspace(2)
  %299 = addrspacecast ptr addrspace(2) %298 to ptr addrspace(1)
  %300 = getelementptr i8, ptr addrspace(1) %299, i16 %297
  %301 = load i16, ptr %135, !tbaa !2
  store i16 %301, ptr addrspace(1) %300, !tbaa !22
  %302 = load i16, ptr %135, !tbaa !2
  %303 = add i16 %302, 2
  store i16 %303, ptr %135, !tbaa !2
  %304 = load i16, ptr %134, !tbaa !2
  %305 = load i16, ptr %132, !tbaa !2
  %306 = add nsw i16 %304, %305
  store i16 %306, ptr %134, !tbaa !2
  br label %b5

b13:
  %307 = load i16, ptr %130, !tbaa !2
  %308 = icmp sge i16 %307, 0
  br i1 %308, label %b14, label %b15

b14:
  %309 = load i16, ptr %134, !tbaa !2
  %310 = load i16, ptr %131, !tbaa !2
  %311 = icmp sle i16 %309, %310
  br i1 %311, label %b16, label %b17

b15:
  %312 = load i16, ptr %134, !tbaa !2
  %313 = load i16, ptr %131, !tbaa !2
  %314 = icmp sge i16 %312, %313
  br i1 %314, label %b16, label %b17

b16:
  %315 = load i16, ptr %135, !tbaa !2
  %316 = sub i16 %315, 2
  store i16 %316, ptr %135, !tbaa !2
  %317 = load i16, ptr %135, !tbaa !2
  %318 = icmp slt i16 %317, 0
  br i1 %318, label %b18, label %b19

b17:
  store i16 0, ptr %135, !tbaa !2
  br label %b21

b18:
  store i16 0, ptr %135, !tbaa !2
  br label %b20

b19:
  br label %b20

b20:
  %319 = load i16, ptr %134, !tbaa !2
  %320 = mul i16 %319, 2
  %321 = getelementptr i8, ptr %140, i16 2
  %322 = load i16, ptr %321, !tbaa !2
  %323 = add i16 400, %320
  %324 = inttoptr i16 %322 to ptr addrspace(2)
  %325 = addrspacecast ptr addrspace(2) %324 to ptr addrspace(1)
  %326 = getelementptr i8, ptr addrspace(1) %325, i16 %323
  %327 = load i16, ptr %135, !tbaa !2
  store i16 %327, ptr addrspace(1) %326, !tbaa !22
  %328 = load i16, ptr %134, !tbaa !2
  %329 = load i16, ptr %130, !tbaa !2
  %330 = add nsw i16 %328, %329
  store i16 %330, ptr %134, !tbaa !2
  br label %b13

b21:
  %331 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %332 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %331, ptr @$string210)
  %333 = icmp eq i16 %332, 0
  br i1 %333, label %b22, label %b23

b22:
  %334 = load i16, ptr %135, !tbaa !2
  %335 = add i16 %334, 1
  store i16 %335, ptr %135, !tbaa !2
  %336 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %336, ptr @"BENCHFRAME&", !tbaa !2
  %337 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %337, ptr @"BENCHFRAME&", !tbaa !2
  %338 = getelementptr i8, ptr %141, i16 2
  %339 = load i16, ptr %338, !tbaa !2
  %340 = inttoptr i16 %339 to ptr addrspace(2)
  %341 = addrspacecast ptr addrspace(2) %340 to ptr addrspace(1)
  %342 = getelementptr i8, ptr addrspace(1) %341, i16 2
  %343 = load i16, ptr %135, !tbaa !2
  store i16 %343, ptr %129, !tbaa !2
  %344 = load i16, ptr %129, !tbaa !2
  %345 = sitofp i16 %344 to float
  store i16 25, ptr %128, !tbaa !2
  %346 = load i16, ptr %128, !tbaa !2
  %347 = sitofp i16 %346 to float
  %348 = fdiv float %345, %347
  %349 = call float @llvm.fabs.f32(float %348)
  %350 = load float, ptr @$float86, !tbaa !2
  %351 = fcmp ogt float %349, %350
  br i1 %351, label %b24, label %b25

b23:
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call void @llrm.ia16.out.i8(i16 980, i8 19)
  call void @llrm.ia16.out.i8(i16 981, i8 0)
  store i16 1, ptr %134, !tbaa !2
  store i16 16, ptr %118, !tbaa !2
  store i16 1, ptr %117, !tbaa !2
  br label %b35

b24:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b25:
  %352 = call float @llvm.sin.f32(float %348)
  store i16 120, ptr %127, !tbaa !2
  %353 = load i16, ptr %127, !tbaa !2
  %354 = sitofp i16 %353 to float
  %355 = fmul float %352, %354
  %356 = call i16 @llvm.lrint.i16.f32(float %355)
  store i16 %356, ptr addrspace(1) %342, !tbaa !24
  %357 = getelementptr i8, ptr %141, i16 2
  %358 = load i16, ptr %357, !tbaa !2
  %359 = inttoptr i16 %358 to ptr addrspace(2)
  %360 = addrspacecast ptr addrspace(2) %359 to ptr addrspace(1)
  %361 = getelementptr i8, ptr addrspace(1) %360, i16 4
  %362 = load i16, ptr %135, !tbaa !2
  store i16 %362, ptr %126, !tbaa !2
  %363 = load i16, ptr %126, !tbaa !2
  %364 = sitofp i16 %363 to float
  store i16 45, ptr %125, !tbaa !2
  %365 = load i16, ptr %125, !tbaa !2
  %366 = sitofp i16 %365 to float
  %367 = fdiv float %364, %366
  %368 = call float @llvm.fabs.f32(float %367)
  %369 = load float, ptr @$float86, !tbaa !2
  %370 = fcmp ogt float %368, %369
  br i1 %370, label %b26, label %b27

b26:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b27:
  %371 = call float @llvm.sin.f32(float %367)
  store i16 110, ptr %124, !tbaa !2
  %372 = load i16, ptr %124, !tbaa !2
  %373 = sitofp i16 %372 to float
  %374 = fmul float %371, %373
  %375 = call i16 @llvm.lrint.i16.f32(float %374)
  store i16 %375, ptr addrspace(1) %361, !tbaa !24
  %376 = getelementptr i8, ptr %141, i16 2
  %377 = load i16, ptr %376, !tbaa !2
  %378 = inttoptr i16 %377 to ptr addrspace(2)
  %379 = addrspacecast ptr addrspace(2) %378 to ptr addrspace(1)
  %380 = getelementptr i8, ptr addrspace(1) %379, i16 6
  %381 = load i16, ptr %135, !tbaa !2
  store i16 %381, ptr %123, !tbaa !2
  %382 = load i16, ptr %123, !tbaa !2
  %383 = sitofp i16 %382 to float
  store i16 35, ptr %122, !tbaa !2
  %384 = load i16, ptr %122, !tbaa !2
  %385 = sitofp i16 %384 to float
  %386 = fdiv float %383, %385
  %387 = call float @llvm.fabs.f32(float %386)
  %388 = load float, ptr @$float86, !tbaa !2
  %389 = fcmp ogt float %387, %388
  br i1 %389, label %b28, label %b29

b28:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b29:
  %390 = call float @llvm.sin.f32(float %386)
  store i16 100, ptr %121, !tbaa !2
  %391 = load i16, ptr %121, !tbaa !2
  %392 = sitofp i16 %391 to float
  %393 = fmul float %390, %392
  %394 = call i16 @llvm.lrint.i16.f32(float %393)
  store i16 %394, ptr addrspace(1) %380, !tbaa !24
  store i16 0, ptr %134, !tbaa !2
  store i16 380, ptr %120, !tbaa !2
  store i16 1, ptr %119, !tbaa !2
  br label %b30

b30:
  %395 = load i16, ptr %119, !tbaa !2
  %396 = icmp sge i16 %395, 0
  br i1 %396, label %b31, label %b32

b31:
  %397 = load i16, ptr %134, !tbaa !2
  %398 = load i16, ptr %120, !tbaa !2
  %399 = icmp sle i16 %397, %398
  br i1 %399, label %b33, label %b34

b32:
  %400 = load i16, ptr %134, !tbaa !2
  %401 = load i16, ptr %120, !tbaa !2
  %402 = icmp sge i16 %400, %401
  br i1 %402, label %b33, label %b34

b33:
  %403 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %403, ptr @"BENCHFRAME&", !tbaa !2
  %404 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %404, ptr @"BENCHFRAME&", !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  %405 = load i16, ptr %134, !tbaa !2
  %406 = getelementptr i8, ptr %141, i16 2
  %407 = load i16, ptr %406, !tbaa !2
  %408 = inttoptr i16 %407 to ptr addrspace(2)
  %409 = addrspacecast ptr addrspace(2) %408 to ptr addrspace(1)
  %410 = getelementptr i8, ptr addrspace(1) %409, i16 2
  %411 = load i16, ptr addrspace(1) %410, !tbaa !24
  %412 = add i16 %405, %411
  %413 = mul i16 %412, 2
  %414 = getelementptr i8, ptr %140, i16 2
  %415 = load i16, ptr %414, !tbaa !2
  %416 = add i16 400, %413
  %417 = inttoptr i16 %415 to ptr addrspace(2)
  %418 = addrspacecast ptr addrspace(2) %417 to ptr addrspace(1)
  %419 = getelementptr i8, ptr addrspace(1) %418, i16 %416
  %420 = load i16, ptr addrspace(1) %419, !tbaa !22
  %421 = trunc i16 %420 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %421)
  %422 = load i16, ptr %134, !tbaa !2
  %423 = getelementptr i8, ptr %141, i16 2
  %424 = load i16, ptr %423, !tbaa !2
  %425 = inttoptr i16 %424 to ptr addrspace(2)
  %426 = addrspacecast ptr addrspace(2) %425 to ptr addrspace(1)
  %427 = getelementptr i8, ptr addrspace(1) %426, i16 4
  %428 = load i16, ptr addrspace(1) %427, !tbaa !24
  %429 = add i16 %422, %428
  %430 = mul i16 %429, 2
  %431 = getelementptr i8, ptr %140, i16 2
  %432 = load i16, ptr %431, !tbaa !2
  %433 = add i16 400, %430
  %434 = inttoptr i16 %432 to ptr addrspace(2)
  %435 = addrspacecast ptr addrspace(2) %434 to ptr addrspace(1)
  %436 = getelementptr i8, ptr addrspace(1) %435, i16 %433
  %437 = load i16, ptr addrspace(1) %436, !tbaa !22
  %438 = trunc i16 %437 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %438)
  %439 = load i16, ptr %134, !tbaa !2
  %440 = getelementptr i8, ptr %141, i16 2
  %441 = load i16, ptr %440, !tbaa !2
  %442 = inttoptr i16 %441 to ptr addrspace(2)
  %443 = addrspacecast ptr addrspace(2) %442 to ptr addrspace(1)
  %444 = getelementptr i8, ptr addrspace(1) %443, i16 6
  %445 = load i16, ptr addrspace(1) %444, !tbaa !24
  %446 = add i16 %439, %445
  %447 = mul i16 %446, 2
  %448 = getelementptr i8, ptr %140, i16 2
  %449 = load i16, ptr %448, !tbaa !2
  %450 = add i16 400, %447
  %451 = inttoptr i16 %449 to ptr addrspace(2)
  %452 = addrspacecast ptr addrspace(2) %451 to ptr addrspace(1)
  %453 = getelementptr i8, ptr addrspace(1) %452, i16 %450
  %454 = load i16, ptr addrspace(1) %453, !tbaa !22
  %455 = trunc i16 %454 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %455)
  %456 = load i16, ptr %134, !tbaa !2
  %457 = load i16, ptr %119, !tbaa !2
  %458 = add nsw i16 %456, %457
  store i16 %458, ptr %134, !tbaa !2
  br label %b30

b34:
  br label %b21

b35:
  %459 = load i16, ptr %117, !tbaa !2
  %460 = icmp sge i16 %459, 0
  br i1 %460, label %b36, label %b37

b36:
  %461 = load i16, ptr %134, !tbaa !2
  %462 = load i16, ptr %118, !tbaa !2
  %463 = icmp sle i16 %461, %462
  br i1 %463, label %b38, label %b39

b37:
  %464 = load i16, ptr %134, !tbaa !2
  %465 = load i16, ptr %118, !tbaa !2
  %466 = icmp sge i16 %464, %465
  br i1 %466, label %b38, label %b39

b38:
  %467 = load i16, ptr %134, !tbaa !2
  %468 = trunc i16 %467 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %468)
  %469 = load i16, ptr %134, !tbaa !2
  %470 = sub i16 %469, 1
  %471 = mul i16 %470, 4
  %472 = trunc i16 %471 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %472)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %473 = load i16, ptr %134, !tbaa !2
  %474 = load i16, ptr %117, !tbaa !2
  %475 = add nsw i16 %473, %474
  store i16 %475, ptr %134, !tbaa !2
  br label %b35

b39:
  store i16 17, ptr %134, !tbaa !2
  store i16 32, ptr %116, !tbaa !2
  store i16 1, ptr %115, !tbaa !2
  br label %b40

b40:
  %476 = load i16, ptr %115, !tbaa !2
  %477 = icmp sge i16 %476, 0
  br i1 %477, label %b41, label %b42

b41:
  %478 = load i16, ptr %134, !tbaa !2
  %479 = load i16, ptr %116, !tbaa !2
  %480 = icmp sle i16 %478, %479
  br i1 %480, label %b43, label %b44

b42:
  %481 = load i16, ptr %134, !tbaa !2
  %482 = load i16, ptr %116, !tbaa !2
  %483 = icmp sge i16 %481, %482
  br i1 %483, label %b43, label %b44

b43:
  %484 = load i16, ptr %134, !tbaa !2
  %485 = trunc i16 %484 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %485)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %486 = load i16, ptr %134, !tbaa !2
  %487 = sub i16 %486, 17
  %488 = mul i16 %487, 4
  %489 = trunc i16 %488 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %489)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %490 = load i16, ptr %134, !tbaa !2
  %491 = load i16, ptr %115, !tbaa !2
  %492 = add nsw i16 %490, %491
  store i16 %492, ptr %134, !tbaa !2
  br label %b40

b44:
  store i16 33, ptr %134, !tbaa !2
  store i16 48, ptr %114, !tbaa !2
  store i16 1, ptr %113, !tbaa !2
  br label %b45

b45:
  %493 = load i16, ptr %113, !tbaa !2
  %494 = icmp sge i16 %493, 0
  br i1 %494, label %b46, label %b47

b46:
  %495 = load i16, ptr %134, !tbaa !2
  %496 = load i16, ptr %114, !tbaa !2
  %497 = icmp sle i16 %495, %496
  br i1 %497, label %b48, label %b49

b47:
  %498 = load i16, ptr %134, !tbaa !2
  %499 = load i16, ptr %114, !tbaa !2
  %500 = icmp sge i16 %498, %499
  br i1 %500, label %b48, label %b49

b48:
  %501 = load i16, ptr %134, !tbaa !2
  %502 = trunc i16 %501 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %502)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %503 = load i16, ptr %134, !tbaa !2
  %504 = sub i16 %503, 33
  %505 = mul i16 %504, 4
  %506 = trunc i16 %505 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %506)
  %507 = load i16, ptr %134, !tbaa !2
  %508 = load i16, ptr %113, !tbaa !2
  %509 = add nsw i16 %507, %508
  store i16 %509, ptr %134, !tbaa !2
  br label %b45

b49:
  store i16 49, ptr %134, !tbaa !2
  store i16 64, ptr %112, !tbaa !2
  store i16 1, ptr %111, !tbaa !2
  br label %b50

b50:
  %510 = load i16, ptr %111, !tbaa !2
  %511 = icmp sge i16 %510, 0
  br i1 %511, label %b51, label %b52

b51:
  %512 = load i16, ptr %134, !tbaa !2
  %513 = load i16, ptr %112, !tbaa !2
  %514 = icmp sle i16 %512, %513
  br i1 %514, label %b53, label %b54

b52:
  %515 = load i16, ptr %134, !tbaa !2
  %516 = load i16, ptr %112, !tbaa !2
  %517 = icmp sge i16 %515, %516
  br i1 %517, label %b53, label %b54

b53:
  %518 = load i16, ptr %134, !tbaa !2
  %519 = trunc i16 %518 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %519)
  %520 = load i16, ptr %134, !tbaa !2
  %521 = sub i16 %520, 49
  %522 = mul i16 %521, 4
  %523 = trunc i16 %522 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %523)
  %524 = load i16, ptr %134, !tbaa !2
  %525 = sub i16 %524, 49
  %526 = mul i16 %525, 4
  %527 = trunc i16 %526 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %527)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %528 = load i16, ptr %134, !tbaa !2
  %529 = load i16, ptr %111, !tbaa !2
  %530 = add nsw i16 %528, %529
  store i16 %530, ptr %134, !tbaa !2
  br label %b50

b54:
  store i16 65, ptr %134, !tbaa !2
  store i16 80, ptr %110, !tbaa !2
  store i16 1, ptr %109, !tbaa !2
  br label %b55

b55:
  %531 = load i16, ptr %109, !tbaa !2
  %532 = icmp sge i16 %531, 0
  br i1 %532, label %b56, label %b57

b56:
  %533 = load i16, ptr %134, !tbaa !2
  %534 = load i16, ptr %110, !tbaa !2
  %535 = icmp sle i16 %533, %534
  br i1 %535, label %b58, label %b59

b57:
  %536 = load i16, ptr %134, !tbaa !2
  %537 = load i16, ptr %110, !tbaa !2
  %538 = icmp sge i16 %536, %537
  br i1 %538, label %b58, label %b59

b58:
  %539 = load i16, ptr %134, !tbaa !2
  %540 = trunc i16 %539 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %540)
  %541 = load i16, ptr %134, !tbaa !2
  %542 = sub i16 %541, 65
  %543 = mul i16 %542, 4
  %544 = trunc i16 %543 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %544)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %545 = load i16, ptr %134, !tbaa !2
  %546 = sub i16 %545, 65
  %547 = mul i16 %546, 4
  %548 = trunc i16 %547 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %548)
  %549 = load i16, ptr %134, !tbaa !2
  %550 = load i16, ptr %109, !tbaa !2
  %551 = add nsw i16 %549, %550
  store i16 %551, ptr %134, !tbaa !2
  br label %b55

b59:
  store i16 81, ptr %134, !tbaa !2
  store i16 96, ptr %108, !tbaa !2
  store i16 1, ptr %107, !tbaa !2
  br label %b60

b60:
  %552 = load i16, ptr %107, !tbaa !2
  %553 = icmp sge i16 %552, 0
  br i1 %553, label %b61, label %b62

b61:
  %554 = load i16, ptr %134, !tbaa !2
  %555 = load i16, ptr %108, !tbaa !2
  %556 = icmp sle i16 %554, %555
  br i1 %556, label %b63, label %b64

b62:
  %557 = load i16, ptr %134, !tbaa !2
  %558 = load i16, ptr %108, !tbaa !2
  %559 = icmp sge i16 %557, %558
  br i1 %559, label %b63, label %b64

b63:
  %560 = load i16, ptr %134, !tbaa !2
  %561 = trunc i16 %560 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %561)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %562 = load i16, ptr %134, !tbaa !2
  %563 = sub i16 %562, 81
  %564 = mul i16 %563, 4
  %565 = trunc i16 %564 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %565)
  %566 = load i16, ptr %134, !tbaa !2
  %567 = sub i16 %566, 81
  %568 = mul i16 %567, 4
  %569 = trunc i16 %568 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %569)
  %570 = load i16, ptr %134, !tbaa !2
  %571 = load i16, ptr %107, !tbaa !2
  %572 = add nsw i16 %570, %571
  store i16 %572, ptr %134, !tbaa !2
  br label %b60

b64:
  store i16 97, ptr %134, !tbaa !2
  store i16 112, ptr %106, !tbaa !2
  store i16 1, ptr %105, !tbaa !2
  br label %b65

b65:
  %573 = load i16, ptr %105, !tbaa !2
  %574 = icmp sge i16 %573, 0
  br i1 %574, label %b66, label %b67

b66:
  %575 = load i16, ptr %134, !tbaa !2
  %576 = load i16, ptr %106, !tbaa !2
  %577 = icmp sle i16 %575, %576
  br i1 %577, label %b68, label %b69

b67:
  %578 = load i16, ptr %134, !tbaa !2
  %579 = load i16, ptr %106, !tbaa !2
  %580 = icmp sge i16 %578, %579
  br i1 %580, label %b68, label %b69

b68:
  %581 = load i16, ptr %134, !tbaa !2
  %582 = trunc i16 %581 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %582)
  %583 = load i16, ptr %134, !tbaa !2
  %584 = sub i16 %583, 97
  %585 = mul i16 %584, 4
  %586 = trunc i16 %585 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %586)
  %587 = load i16, ptr %134, !tbaa !2
  %588 = sub i16 %587, 97
  %589 = mul i16 %588, 2
  %590 = trunc i16 %589 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %590)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %591 = load i16, ptr %134, !tbaa !2
  %592 = load i16, ptr %105, !tbaa !2
  %593 = add nsw i16 %591, %592
  store i16 %593, ptr %134, !tbaa !2
  br label %b65

b69:
  store i16 113, ptr %134, !tbaa !2
  store i16 128, ptr %104, !tbaa !2
  store i16 1, ptr %103, !tbaa !2
  br label %b70

b70:
  %594 = load i16, ptr %103, !tbaa !2
  %595 = icmp sge i16 %594, 0
  br i1 %595, label %b71, label %b72

b71:
  %596 = load i16, ptr %134, !tbaa !2
  %597 = load i16, ptr %104, !tbaa !2
  %598 = icmp sle i16 %596, %597
  br i1 %598, label %b73, label %b74

b72:
  %599 = load i16, ptr %134, !tbaa !2
  %600 = load i16, ptr %104, !tbaa !2
  %601 = icmp sge i16 %599, %600
  br i1 %601, label %b73, label %b74

b73:
  %602 = load i16, ptr %134, !tbaa !2
  %603 = trunc i16 %602 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %603)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %604 = load i16, ptr %134, !tbaa !2
  %605 = sub i16 %604, 113
  %606 = mul i16 %605, 2
  %607 = trunc i16 %606 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %607)
  %608 = load i16, ptr %134, !tbaa !2
  %609 = sub i16 %608, 113
  %610 = mul i16 %609, 4
  %611 = trunc i16 %610 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %611)
  %612 = load i16, ptr %134, !tbaa !2
  %613 = load i16, ptr %103, !tbaa !2
  %614 = add nsw i16 %612, %613
  store i16 %614, ptr %134, !tbaa !2
  br label %b70

b74:
  store i16 1, ptr %134, !tbaa !2
  store i16 16, ptr %102, !tbaa !2
  store i16 1, ptr %101, !tbaa !2
  br label %b75

b75:
  %615 = load i16, ptr %101, !tbaa !2
  %616 = icmp sge i16 %615, 0
  br i1 %616, label %b76, label %b77

b76:
  %617 = load i16, ptr %134, !tbaa !2
  %618 = load i16, ptr %102, !tbaa !2
  %619 = icmp sle i16 %617, %618
  br i1 %619, label %b78, label %b79

b77:
  %620 = load i16, ptr %134, !tbaa !2
  %621 = load i16, ptr %102, !tbaa !2
  %622 = icmp sge i16 %620, %621
  br i1 %622, label %b78, label %b79

b78:
  %623 = load i16, ptr %134, !tbaa !2
  %624 = mul i16 %623, 2
  %625 = getelementptr i8, ptr %151, i16 2
  %626 = load i16, ptr %625, !tbaa !2
  %627 = add i16 -2, %624
  %628 = inttoptr i16 %626 to ptr addrspace(2)
  %629 = addrspacecast ptr addrspace(2) %628 to ptr addrspace(1)
  %630 = getelementptr i8, ptr addrspace(1) %629, i16 %627
  %631 = load i16, ptr %134, !tbaa !2
  store i16 %631, ptr addrspace(1) %630, !tbaa !26
  %632 = load i16, ptr %134, !tbaa !2
  %633 = load i16, ptr %101, !tbaa !2
  %634 = add nsw i16 %632, %633
  store i16 %634, ptr %134, !tbaa !2
  br label %b75

b79:
  store i16 17, ptr %134, !tbaa !2
  store i16 32, ptr %100, !tbaa !2
  store i16 1, ptr %99, !tbaa !2
  br label %b80

b80:
  %635 = load i16, ptr %99, !tbaa !2
  %636 = icmp sge i16 %635, 0
  br i1 %636, label %b81, label %b82

b81:
  %637 = load i16, ptr %134, !tbaa !2
  %638 = load i16, ptr %100, !tbaa !2
  %639 = icmp sle i16 %637, %638
  br i1 %639, label %b83, label %b84

b82:
  %640 = load i16, ptr %134, !tbaa !2
  %641 = load i16, ptr %100, !tbaa !2
  %642 = icmp sge i16 %640, %641
  br i1 %642, label %b83, label %b84

b83:
  %643 = load i16, ptr %134, !tbaa !2
  %644 = mul i16 %643, 2
  %645 = getelementptr i8, ptr %151, i16 2
  %646 = load i16, ptr %645, !tbaa !2
  %647 = add i16 -2, %644
  %648 = inttoptr i16 %646 to ptr addrspace(2)
  %649 = addrspacecast ptr addrspace(2) %648 to ptr addrspace(1)
  %650 = getelementptr i8, ptr addrspace(1) %649, i16 %647
  %651 = load i16, ptr %134, !tbaa !2
  %652 = sub i16 33, %651
  store i16 %652, ptr addrspace(1) %650, !tbaa !26
  %653 = load i16, ptr %134, !tbaa !2
  %654 = load i16, ptr %99, !tbaa !2
  %655 = add nsw i16 %653, %654
  store i16 %655, ptr %134, !tbaa !2
  br label %b80

b84:
  store i16 1, ptr %134, !tbaa !2
  store i16 32, ptr %98, !tbaa !2
  store i16 1, ptr %97, !tbaa !2
  br label %b85

b85:
  %656 = load i16, ptr %97, !tbaa !2
  %657 = icmp sge i16 %656, 0
  br i1 %657, label %b86, label %b87

b86:
  %658 = load i16, ptr %134, !tbaa !2
  %659 = load i16, ptr %98, !tbaa !2
  %660 = icmp sle i16 %658, %659
  br i1 %660, label %b88, label %b89

b87:
  %661 = load i16, ptr %134, !tbaa !2
  %662 = load i16, ptr %98, !tbaa !2
  %663 = icmp sge i16 %661, %662
  br i1 %663, label %b88, label %b89

b88:
  %664 = load i16, ptr %134, !tbaa !2
  %665 = mul i16 %664, 2
  %666 = getelementptr i8, ptr %149, i16 2
  %667 = load i16, ptr %666, !tbaa !2
  %668 = add i16 -2, %665
  %669 = inttoptr i16 %667 to ptr addrspace(2)
  %670 = addrspacecast ptr addrspace(2) %669 to ptr addrspace(1)
  %671 = getelementptr i8, ptr addrspace(1) %670, i16 %668
  %672 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %673 = load float, ptr %672
  store i16 63, ptr %96, !tbaa !2
  %674 = load i16, ptr %96, !tbaa !2
  %675 = sitofp i16 %674 to float
  %676 = fmul float %673, %675
  store i16 32, ptr %95, !tbaa !2
  %677 = load i16, ptr %95, !tbaa !2
  %678 = sitofp i16 %677 to float
  %679 = fadd float %676, %678
  %680 = call float @llvm.rint.f32(float %679)
  %681 = fcmp olt float %679, %680
  %682 = sext i1 %681 to i16
  store i16 %682, ptr %94, !tbaa !2
  %683 = load i16, ptr %94, !tbaa !2
  %684 = sitofp i16 %683 to float
  %685 = fadd float %680, %684
  %686 = call i16 @llvm.lrint.i16.f32(float %685)
  store i16 %686, ptr addrspace(1) %671, !tbaa !28
  %687 = load i16, ptr %134, !tbaa !2
  %688 = mul i16 %687, 2
  %689 = getelementptr i8, ptr %148, i16 2
  %690 = load i16, ptr %689, !tbaa !2
  %691 = add i16 -2, %688
  %692 = inttoptr i16 %690 to ptr addrspace(2)
  %693 = addrspacecast ptr addrspace(2) %692 to ptr addrspace(1)
  %694 = getelementptr i8, ptr addrspace(1) %693, i16 %691
  %695 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %696 = load float, ptr %695
  store i16 127, ptr %93, !tbaa !2
  %697 = load i16, ptr %93, !tbaa !2
  %698 = sitofp i16 %697 to float
  %699 = fmul float %696, %698
  %700 = call float @llvm.rint.f32(float %699)
  %701 = fcmp olt float %699, %700
  %702 = sext i1 %701 to i16
  store i16 %702, ptr %92, !tbaa !2
  %703 = load i16, ptr %92, !tbaa !2
  %704 = sitofp i16 %703 to float
  %705 = fadd float %700, %704
  %706 = call i16 @llvm.lrint.i16.f32(float %705)
  store i16 %706, ptr addrspace(1) %694, !tbaa !30
  %707 = load i16, ptr %134, !tbaa !2
  %708 = load i16, ptr %97, !tbaa !2
  %709 = add nsw i16 %707, %708
  store i16 %709, ptr %134, !tbaa !2
  br label %b85

b89:
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 0, ptr %135, !tbaa !2
  br label %b90

b90:
  %710 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %711 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %710, ptr @$string211)
  %712 = icmp eq i16 %711, 0
  br i1 %712, label %b91, label %b92

b91:
  %713 = load i16, ptr %135, !tbaa !2
  %714 = add i16 %713, 1
  store i16 %714, ptr %135, !tbaa !2
  store i16 1, ptr %134, !tbaa !2
  store i16 8, ptr %91, !tbaa !2
  store i16 1, ptr %90, !tbaa !2
  br label %b93

b92:
  store i16 0, ptr %64, !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %63, !tbaa !2
  store i16 15, ptr %62, !tbaa !2
  store i16 1, ptr %61, !tbaa !2
  br label %b131

b93:
  %715 = load i16, ptr %90, !tbaa !2
  %716 = icmp sge i16 %715, 0
  br i1 %716, label %b94, label %b95

b94:
  %717 = load i16, ptr %134, !tbaa !2
  %718 = load i16, ptr %91, !tbaa !2
  %719 = icmp sle i16 %717, %718
  br i1 %719, label %b96, label %b97

b95:
  %720 = load i16, ptr %134, !tbaa !2
  %721 = load i16, ptr %91, !tbaa !2
  %722 = icmp sge i16 %720, %721
  br i1 %722, label %b96, label %b97

b96:
  %723 = load i16, ptr %134, !tbaa !2
  %724 = mul i16 %723, 2
  %725 = getelementptr i8, ptr %147, i16 2
  %726 = load i16, ptr %725, !tbaa !2
  %727 = add i16 0, %724
  %728 = inttoptr i16 %726 to ptr addrspace(2)
  %729 = addrspacecast ptr addrspace(2) %728 to ptr addrspace(1)
  %730 = getelementptr i8, ptr addrspace(1) %729, i16 %727
  %731 = load i16, ptr %135, !tbaa !2
  %732 = load i16, ptr %134, !tbaa !2
  %733 = mul i16 %732, 2
  %734 = getelementptr i8, ptr %149, i16 2
  %735 = load i16, ptr %734, !tbaa !2
  %736 = add i16 -2, %733
  %737 = inttoptr i16 %735 to ptr addrspace(2)
  %738 = addrspacecast ptr addrspace(2) %737 to ptr addrspace(1)
  %739 = getelementptr i8, ptr addrspace(1) %738, i16 %736
  %740 = load i16, ptr addrspace(1) %739, !tbaa !28
  store i16 %731, ptr %89, !tbaa !2
  %741 = load i16, ptr %89, !tbaa !2
  %742 = sitofp i16 %741 to float
  store i16 %740, ptr %88, !tbaa !2
  %743 = load i16, ptr %88, !tbaa !2
  %744 = sitofp i16 %743 to float
  %745 = fdiv float %742, %744
  %746 = call float @llvm.fabs.f32(float %745)
  %747 = load float, ptr @$float86, !tbaa !2
  %748 = fcmp ogt float %746, %747
  br i1 %748, label %b98, label %b99

b97:
  store i16 1, ptr %84, !tbaa !2
  store i16 8, ptr %83, !tbaa !2
  store i16 1, ptr %82, !tbaa !2
  br label %b100

b98:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b99:
  %749 = call float @llvm.sin.f32(float %745)
  %750 = load i16, ptr %134, !tbaa !2
  %751 = mul i16 %750, 2
  %752 = getelementptr i8, ptr %148, i16 2
  %753 = load i16, ptr %752, !tbaa !2
  %754 = add i16 -2, %751
  %755 = inttoptr i16 %753 to ptr addrspace(2)
  %756 = addrspacecast ptr addrspace(2) %755 to ptr addrspace(1)
  %757 = getelementptr i8, ptr addrspace(1) %756, i16 %754
  %758 = load i16, ptr addrspace(1) %757, !tbaa !30
  store i16 %758, ptr %87, !tbaa !2
  %759 = load i16, ptr %87, !tbaa !2
  %760 = sitofp i16 %759 to float
  %761 = fmul float %749, %760
  %762 = load i16, ptr %134, !tbaa !2
  %763 = mul i16 %762, 2
  %764 = getelementptr i8, ptr %148, i16 2
  %765 = load i16, ptr %764, !tbaa !2
  %766 = add i16 -2, %763
  %767 = inttoptr i16 %765 to ptr addrspace(2)
  %768 = addrspacecast ptr addrspace(2) %767 to ptr addrspace(1)
  %769 = getelementptr i8, ptr addrspace(1) %768, i16 %766
  %770 = load i16, ptr addrspace(1) %769, !tbaa !30
  store i16 %770, ptr %86, !tbaa !2
  %771 = load i16, ptr %86, !tbaa !2
  %772 = sitofp i16 %771 to float
  %773 = fadd float %761, %772
  %774 = load i16, ptr %134, !tbaa !2
  %775 = mul i16 %774, 2
  %776 = getelementptr i8, ptr %148, i16 2
  %777 = load i16, ptr %776, !tbaa !2
  %778 = add i16 -2, %775
  %779 = inttoptr i16 %777 to ptr addrspace(2)
  %780 = addrspacecast ptr addrspace(2) %779 to ptr addrspace(1)
  %781 = getelementptr i8, ptr addrspace(1) %780, i16 %778
  %782 = load i16, ptr addrspace(1) %781, !tbaa !30
  %783 = mul i16 2, %782
  %784 = sub i16 280, %783
  %785 = sext i16 %784 to i32
  %786 = sdiv i32 %785, 2
  %787 = trunc i32 %786 to i16
  store i16 %787, ptr %85, !tbaa !2
  %788 = load i16, ptr %85, !tbaa !2
  %789 = sitofp i16 %788 to float
  %790 = fadd float %773, %789
  %791 = call i16 @llvm.lrint.i16.f32(float %790)
  store i16 %791, ptr addrspace(1) %730, !tbaa !32
  %792 = load i16, ptr %134, !tbaa !2
  %793 = load i16, ptr %90, !tbaa !2
  %794 = add nsw i16 %792, %793
  store i16 %794, ptr %134, !tbaa !2
  br label %b93

b100:
  %795 = load i16, ptr %82, !tbaa !2
  %796 = icmp sge i16 %795, 0
  br i1 %796, label %b101, label %b102

b101:
  %797 = load i16, ptr %84, !tbaa !2
  %798 = load i16, ptr %83, !tbaa !2
  %799 = icmp sle i16 %797, %798
  br i1 %799, label %b103, label %b104

b102:
  %800 = load i16, ptr %84, !tbaa !2
  %801 = load i16, ptr %83, !tbaa !2
  %802 = icmp sge i16 %800, %801
  br i1 %802, label %b103, label %b104

b103:
  store i16 1, ptr %134, !tbaa !2
  store i16 32, ptr %81, !tbaa !2
  store i16 1, ptr %80, !tbaa !2
  br label %b105

b104:
  %803 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %803, ptr @"BENCHFRAME&", !tbaa !2
  store i16 0, ptr %134, !tbaa !2
  store i16 319, ptr %79, !tbaa !2
  store i16 1, ptr %78, !tbaa !2
  br label %b110

b105:
  %804 = load i16, ptr %80, !tbaa !2
  %805 = icmp sge i16 %804, 0
  br i1 %805, label %b106, label %b107

b106:
  %806 = load i16, ptr %134, !tbaa !2
  %807 = load i16, ptr %81, !tbaa !2
  %808 = icmp sle i16 %806, %807
  br i1 %808, label %b108, label %b109

b107:
  %809 = load i16, ptr %134, !tbaa !2
  %810 = load i16, ptr %81, !tbaa !2
  %811 = icmp sge i16 %809, %810
  br i1 %811, label %b108, label %b109

b108:
  %812 = load i16, ptr %134, !tbaa !2
  %813 = load i16, ptr %84, !tbaa !2
  %814 = mul i16 %813, 2
  %815 = getelementptr i8, ptr %147, i16 2
  %816 = load i16, ptr %815, !tbaa !2
  %817 = add i16 0, %814
  %818 = inttoptr i16 %816 to ptr addrspace(2)
  %819 = addrspacecast ptr addrspace(2) %818 to ptr addrspace(1)
  %820 = getelementptr i8, ptr addrspace(1) %819, i16 %817
  %821 = load i16, ptr addrspace(1) %820, !tbaa !32
  %822 = add i16 %812, %821
  %823 = mul i16 %822, 2
  %824 = getelementptr i8, ptr %150, i16 2
  %825 = load i16, ptr %824, !tbaa !2
  %826 = add i16 0, %823
  %827 = inttoptr i16 %825 to ptr addrspace(2)
  %828 = addrspacecast ptr addrspace(2) %827 to ptr addrspace(1)
  %829 = getelementptr i8, ptr addrspace(1) %828, i16 %826
  %830 = load i16, ptr %134, !tbaa !2
  %831 = mul i16 %830, 2
  %832 = getelementptr i8, ptr %151, i16 2
  %833 = load i16, ptr %832, !tbaa !2
  %834 = add i16 -2, %831
  %835 = inttoptr i16 %833 to ptr addrspace(2)
  %836 = addrspacecast ptr addrspace(2) %835 to ptr addrspace(1)
  %837 = getelementptr i8, ptr addrspace(1) %836, i16 %834
  %838 = load i16, ptr addrspace(1) %837, !tbaa !26
  %839 = load i16, ptr %84, !tbaa !2
  %840 = sub i16 %839, 1
  %841 = mul i16 16, %840
  %842 = add i16 %838, %841
  store i16 %842, ptr addrspace(1) %829, !tbaa !34
  %843 = load i16, ptr %134, !tbaa !2
  %844 = load i16, ptr %80, !tbaa !2
  %845 = add nsw i16 %843, %844
  store i16 %845, ptr %134, !tbaa !2
  br label %b105

b109:
  %846 = load i16, ptr %84, !tbaa !2
  %847 = load i16, ptr %82, !tbaa !2
  %848 = add nsw i16 %846, %847
  store i16 %848, ptr %84, !tbaa !2
  br label %b100

b110:
  %849 = load i16, ptr %78, !tbaa !2
  %850 = icmp sge i16 %849, 0
  br i1 %850, label %b111, label %b112

b111:
  %851 = load i16, ptr %134, !tbaa !2
  %852 = load i16, ptr %79, !tbaa !2
  %853 = icmp sle i16 %851, %852
  br i1 %853, label %b113, label %b114

b112:
  %854 = load i16, ptr %134, !tbaa !2
  %855 = load i16, ptr %79, !tbaa !2
  %856 = icmp sge i16 %854, %855
  br i1 %856, label %b113, label %b114

b113:
  %857 = load i16, ptr %134, !tbaa !2
  %858 = load i16, ptr %134, !tbaa !2
  %859 = mul i16 %858, 2
  %860 = getelementptr i8, ptr %150, i16 2
  %861 = load i16, ptr %860, !tbaa !2
  %862 = add i16 0, %859
  %863 = inttoptr i16 %861 to ptr addrspace(2)
  %864 = addrspacecast ptr addrspace(2) %863 to ptr addrspace(1)
  %865 = getelementptr i8, ptr addrspace(1) %864, i16 %862
  %866 = load i16, ptr addrspace(1) %865, !tbaa !34
  %867 = trunc i16 %866 to i8
  %868 = load i16, ptr @b$seg, !tbaa !2
  %869 = inttoptr i16 %868 to ptr addrspace(2)
  %870 = addrspacecast ptr addrspace(2) %869 to ptr addrspace(1)
  %871 = getelementptr i8, ptr addrspace(1) %870, i16 %857
  store volatile i8 %867, ptr addrspace(1) %871, !tbaa !4
  %872 = load i16, ptr %134, !tbaa !2
  %873 = add i16 %872, 80
  %874 = load i16, ptr %134, !tbaa !2
  %875 = mul i16 %874, 2
  %876 = getelementptr i8, ptr %150, i16 2
  %877 = load i16, ptr %876, !tbaa !2
  %878 = add i16 0, %875
  %879 = inttoptr i16 %877 to ptr addrspace(2)
  %880 = addrspacecast ptr addrspace(2) %879 to ptr addrspace(1)
  %881 = getelementptr i8, ptr addrspace(1) %880, i16 %878
  %882 = load i16, ptr addrspace(1) %881, !tbaa !34
  %883 = trunc i16 %882 to i8
  %884 = load i16, ptr @b$seg, !tbaa !2
  %885 = inttoptr i16 %884 to ptr addrspace(2)
  %886 = addrspacecast ptr addrspace(2) %885 to ptr addrspace(1)
  %887 = getelementptr i8, ptr addrspace(1) %886, i16 %873
  store volatile i8 %883, ptr addrspace(1) %887, !tbaa !4
  %888 = load i16, ptr %134, !tbaa !2
  %889 = load i16, ptr %78, !tbaa !2
  %890 = add nsw i16 %888, %889
  store i16 %890, ptr %134, !tbaa !2
  br label %b110

b114:
  %891 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %891, ptr @"BENCHFRAME&", !tbaa !2
  %892 = getelementptr i8, ptr %141, i16 2
  %893 = load i16, ptr %892, !tbaa !2
  %894 = inttoptr i16 %893 to ptr addrspace(2)
  %895 = addrspacecast ptr addrspace(2) %894 to ptr addrspace(1)
  %896 = getelementptr i8, ptr addrspace(1) %895, i16 2
  %897 = load i16, ptr %135, !tbaa !2
  store i16 %897, ptr %77, !tbaa !2
  %898 = load i16, ptr %77, !tbaa !2
  %899 = sitofp i16 %898 to float
  store i16 25, ptr %76, !tbaa !2
  %900 = load i16, ptr %76, !tbaa !2
  %901 = sitofp i16 %900 to float
  %902 = fdiv float %899, %901
  %903 = call float @llvm.fabs.f32(float %902)
  %904 = load float, ptr @$float86, !tbaa !2
  %905 = fcmp ogt float %903, %904
  br i1 %905, label %b115, label %b116

b115:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b116:
  %906 = call float @llvm.sin.f32(float %902)
  store i16 120, ptr %75, !tbaa !2
  %907 = load i16, ptr %75, !tbaa !2
  %908 = sitofp i16 %907 to float
  %909 = fmul float %906, %908
  %910 = call i16 @llvm.lrint.i16.f32(float %909)
  store i16 %910, ptr addrspace(1) %896, !tbaa !24
  %911 = getelementptr i8, ptr %141, i16 2
  %912 = load i16, ptr %911, !tbaa !2
  %913 = inttoptr i16 %912 to ptr addrspace(2)
  %914 = addrspacecast ptr addrspace(2) %913 to ptr addrspace(1)
  %915 = getelementptr i8, ptr addrspace(1) %914, i16 4
  %916 = load i16, ptr %135, !tbaa !2
  store i16 %916, ptr %74, !tbaa !2
  %917 = load i16, ptr %74, !tbaa !2
  %918 = sitofp i16 %917 to float
  store i16 45, ptr %73, !tbaa !2
  %919 = load i16, ptr %73, !tbaa !2
  %920 = sitofp i16 %919 to float
  %921 = fdiv float %918, %920
  %922 = call float @llvm.fabs.f32(float %921)
  %923 = load float, ptr @$float86, !tbaa !2
  %924 = fcmp ogt float %922, %923
  br i1 %924, label %b117, label %b118

b117:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b118:
  %925 = call float @llvm.sin.f32(float %921)
  store i16 110, ptr %72, !tbaa !2
  %926 = load i16, ptr %72, !tbaa !2
  %927 = sitofp i16 %926 to float
  %928 = fmul float %925, %927
  %929 = call i16 @llvm.lrint.i16.f32(float %928)
  store i16 %929, ptr addrspace(1) %915, !tbaa !24
  %930 = getelementptr i8, ptr %141, i16 2
  %931 = load i16, ptr %930, !tbaa !2
  %932 = inttoptr i16 %931 to ptr addrspace(2)
  %933 = addrspacecast ptr addrspace(2) %932 to ptr addrspace(1)
  %934 = getelementptr i8, ptr addrspace(1) %933, i16 6
  %935 = load i16, ptr %135, !tbaa !2
  store i16 %935, ptr %71, !tbaa !2
  %936 = load i16, ptr %71, !tbaa !2
  %937 = sitofp i16 %936 to float
  store i16 35, ptr %70, !tbaa !2
  %938 = load i16, ptr %70, !tbaa !2
  %939 = sitofp i16 %938 to float
  %940 = fdiv float %937, %939
  %941 = call float @llvm.fabs.f32(float %940)
  %942 = load float, ptr @$float86, !tbaa !2
  %943 = fcmp ogt float %941, %942
  br i1 %943, label %b119, label %b120

b119:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b120:
  %944 = call float @llvm.sin.f32(float %940)
  store i16 100, ptr %69, !tbaa !2
  %945 = load i16, ptr %69, !tbaa !2
  %946 = sitofp i16 %945 to float
  %947 = fmul float %944, %946
  %948 = call i16 @llvm.lrint.i16.f32(float %947)
  store i16 %948, ptr addrspace(1) %934, !tbaa !24
  store i16 0, ptr %134, !tbaa !2
  store i16 380, ptr %68, !tbaa !2
  store i16 1, ptr %67, !tbaa !2
  br label %b121

b121:
  %949 = load i16, ptr %67, !tbaa !2
  %950 = icmp sge i16 %949, 0
  br i1 %950, label %b122, label %b123

b122:
  %951 = load i16, ptr %134, !tbaa !2
  %952 = load i16, ptr %68, !tbaa !2
  %953 = icmp sle i16 %951, %952
  br i1 %953, label %b124, label %b125

b123:
  %954 = load i16, ptr %134, !tbaa !2
  %955 = load i16, ptr %68, !tbaa !2
  %956 = icmp sge i16 %954, %955
  br i1 %956, label %b124, label %b125

b124:
  %957 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %957, ptr @"BENCHFRAME&", !tbaa !2
  %958 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %958, ptr @"BENCHFRAME&", !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  %959 = load i16, ptr %134, !tbaa !2
  %960 = getelementptr i8, ptr %141, i16 2
  %961 = load i16, ptr %960, !tbaa !2
  %962 = inttoptr i16 %961 to ptr addrspace(2)
  %963 = addrspacecast ptr addrspace(2) %962 to ptr addrspace(1)
  %964 = getelementptr i8, ptr addrspace(1) %963, i16 2
  %965 = load i16, ptr addrspace(1) %964, !tbaa !24
  %966 = add i16 %959, %965
  %967 = mul i16 %966, 2
  %968 = getelementptr i8, ptr %140, i16 2
  %969 = load i16, ptr %968, !tbaa !2
  %970 = add i16 400, %967
  %971 = inttoptr i16 %969 to ptr addrspace(2)
  %972 = addrspacecast ptr addrspace(2) %971 to ptr addrspace(1)
  %973 = getelementptr i8, ptr addrspace(1) %972, i16 %970
  %974 = load i16, ptr addrspace(1) %973, !tbaa !22
  %975 = trunc i16 %974 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %975)
  %976 = load i16, ptr %134, !tbaa !2
  %977 = getelementptr i8, ptr %141, i16 2
  %978 = load i16, ptr %977, !tbaa !2
  %979 = inttoptr i16 %978 to ptr addrspace(2)
  %980 = addrspacecast ptr addrspace(2) %979 to ptr addrspace(1)
  %981 = getelementptr i8, ptr addrspace(1) %980, i16 4
  %982 = load i16, ptr addrspace(1) %981, !tbaa !24
  %983 = add i16 %976, %982
  %984 = mul i16 %983, 2
  %985 = getelementptr i8, ptr %140, i16 2
  %986 = load i16, ptr %985, !tbaa !2
  %987 = add i16 400, %984
  %988 = inttoptr i16 %986 to ptr addrspace(2)
  %989 = addrspacecast ptr addrspace(2) %988 to ptr addrspace(1)
  %990 = getelementptr i8, ptr addrspace(1) %989, i16 %987
  %991 = load i16, ptr addrspace(1) %990, !tbaa !22
  %992 = trunc i16 %991 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %992)
  %993 = load i16, ptr %134, !tbaa !2
  %994 = getelementptr i8, ptr %141, i16 2
  %995 = load i16, ptr %994, !tbaa !2
  %996 = inttoptr i16 %995 to ptr addrspace(2)
  %997 = addrspacecast ptr addrspace(2) %996 to ptr addrspace(1)
  %998 = getelementptr i8, ptr addrspace(1) %997, i16 6
  %999 = load i16, ptr addrspace(1) %998, !tbaa !24
  %1000 = add i16 %993, %999
  %1001 = mul i16 %1000, 2
  %1002 = getelementptr i8, ptr %140, i16 2
  %1003 = load i16, ptr %1002, !tbaa !2
  %1004 = add i16 400, %1001
  %1005 = inttoptr i16 %1003 to ptr addrspace(2)
  %1006 = addrspacecast ptr addrspace(2) %1005 to ptr addrspace(1)
  %1007 = getelementptr i8, ptr addrspace(1) %1006, i16 %1004
  %1008 = load i16, ptr addrspace(1) %1007, !tbaa !22
  %1009 = trunc i16 %1008 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1009)
  %1010 = load i16, ptr %134, !tbaa !2
  %1011 = load i16, ptr %67, !tbaa !2
  %1012 = add nsw i16 %1010, %1011
  store i16 %1012, ptr %134, !tbaa !2
  br label %b121

b125:
  store i16 0, ptr %134, !tbaa !2
  store i16 319, ptr %66, !tbaa !2
  store i16 1, ptr %65, !tbaa !2
  br label %b126

b126:
  %1013 = load i16, ptr %65, !tbaa !2
  %1014 = icmp sge i16 %1013, 0
  br i1 %1014, label %b127, label %b128

b127:
  %1015 = load i16, ptr %134, !tbaa !2
  %1016 = load i16, ptr %66, !tbaa !2
  %1017 = icmp sle i16 %1015, %1016
  br i1 %1017, label %b129, label %b130

b128:
  %1018 = load i16, ptr %134, !tbaa !2
  %1019 = load i16, ptr %66, !tbaa !2
  %1020 = icmp sge i16 %1018, %1019
  br i1 %1020, label %b129, label %b130

b129:
  %1021 = load i16, ptr %134, !tbaa !2
  %1022 = mul i16 %1021, 2
  %1023 = getelementptr i8, ptr %150, i16 2
  %1024 = load i16, ptr %1023, !tbaa !2
  %1025 = add i16 0, %1022
  %1026 = inttoptr i16 %1024 to ptr addrspace(2)
  %1027 = addrspacecast ptr addrspace(2) %1026 to ptr addrspace(1)
  %1028 = getelementptr i8, ptr addrspace(1) %1027, i16 %1025
  store i16 0, ptr addrspace(1) %1028, !tbaa !34
  %1029 = load i16, ptr %134, !tbaa !2
  %1030 = load i16, ptr %65, !tbaa !2
  %1031 = add nsw i16 %1029, %1030
  store i16 %1031, ptr %134, !tbaa !2
  br label %b126

b130:
  br label %b90

b131:
  %1032 = load i16, ptr %61, !tbaa !2
  %1033 = icmp sge i16 %1032, 0
  br i1 %1033, label %b132, label %b133

b132:
  %1034 = load i16, ptr %63, !tbaa !2
  %1035 = load i16, ptr %62, !tbaa !2
  %1036 = icmp sle i16 %1034, %1035
  br i1 %1036, label %b134, label %b135

b133:
  %1037 = load i16, ptr %63, !tbaa !2
  %1038 = load i16, ptr %62, !tbaa !2
  %1039 = icmp sge i16 %1037, %1038
  br i1 %1039, label %b134, label %b135

b134:
  store i16 0, ptr %60, !tbaa !2
  store i16 15, ptr %59, !tbaa !2
  store i16 1, ptr %58, !tbaa !2
  br label %b136

b135:
  call void @llrm.ia16.out.i8(i16 980, i8 19)
  call void @llrm.ia16.out.i8(i16 981, i8 0)
  store i16 0, ptr %134, !tbaa !2
  store i16 1083, ptr %57, !tbaa !2
  store i16 1, ptr %56, !tbaa !2
  br label %b141

b136:
  %1040 = load i16, ptr %58, !tbaa !2
  %1041 = icmp sge i16 %1040, 0
  br i1 %1041, label %b137, label %b138

b137:
  %1042 = load i16, ptr %60, !tbaa !2
  %1043 = load i16, ptr %59, !tbaa !2
  %1044 = icmp sle i16 %1042, %1043
  br i1 %1044, label %b139, label %b140

b138:
  %1045 = load i16, ptr %60, !tbaa !2
  %1046 = load i16, ptr %59, !tbaa !2
  %1047 = icmp sge i16 %1045, %1046
  br i1 %1047, label %b139, label %b140

b139:
  %1048 = load i16, ptr %63, !tbaa !2
  %1049 = mul i16 %1048, 2
  %1050 = add i16 %1049, 33
  %1051 = trunc i16 %1050 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1051)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %1052 = load i16, ptr %60, !tbaa !2
  %1053 = mul i16 %1052, 2
  %1054 = add i16 %1053, 33
  %1055 = trunc i16 %1054 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1055)
  %1056 = load i16, ptr %63, !tbaa !2
  %1057 = load i16, ptr %60, !tbaa !2
  %1058 = mul i16 %1057, 16
  %1059 = add i16 %1058, %1056
  %1060 = mul i16 %1059, 2
  %1061 = getelementptr i8, ptr %146, i16 2
  %1062 = load i16, ptr %1061, !tbaa !2
  %1063 = add i16 0, %1060
  %1064 = inttoptr i16 %1062 to ptr addrspace(2)
  %1065 = addrspacecast ptr addrspace(2) %1064 to ptr addrspace(1)
  %1066 = getelementptr i8, ptr addrspace(1) %1065, i16 %1063
  %1067 = load i16, ptr %64, !tbaa !2
  store i16 %1067, ptr addrspace(1) %1066, !tbaa !36
  %1068 = load i16, ptr %64, !tbaa !2
  %1069 = add i16 %1068, 1
  store i16 %1069, ptr %64, !tbaa !2
  %1070 = load i16, ptr %60, !tbaa !2
  %1071 = load i16, ptr %58, !tbaa !2
  %1072 = add nsw i16 %1070, %1071
  store i16 %1072, ptr %60, !tbaa !2
  br label %b136

b140:
  %1073 = load i16, ptr %63, !tbaa !2
  %1074 = load i16, ptr %61, !tbaa !2
  %1075 = add nsw i16 %1073, %1074
  store i16 %1075, ptr %63, !tbaa !2
  br label %b131

b141:
  %1076 = load i16, ptr %56, !tbaa !2
  %1077 = icmp sge i16 %1076, 0
  br i1 %1077, label %b142, label %b143

b142:
  %1078 = load i16, ptr %134, !tbaa !2
  %1079 = load i16, ptr %57, !tbaa !2
  %1080 = icmp sle i16 %1078, %1079
  br i1 %1080, label %b144, label %b145

b143:
  %1081 = load i16, ptr %134, !tbaa !2
  %1082 = load i16, ptr %57, !tbaa !2
  %1083 = icmp sge i16 %1081, %1082
  br i1 %1083, label %b144, label %b145

b144:
  %1084 = load i16, ptr %134, !tbaa !2
  %1085 = load i16, ptr %134, !tbaa !2
  store i16 %1085, ptr %55, !tbaa !2
  %1086 = load i16, ptr %55, !tbaa !2
  %1087 = sitofp i16 %1086 to float
  store i16 45, ptr %54, !tbaa !2
  %1088 = load i16, ptr %54, !tbaa !2
  %1089 = sitofp i16 %1088 to float
  %1090 = fdiv float %1087, %1089
  %1091 = call float @llvm.fabs.f32(float %1090)
  %1092 = load float, ptr @$float86, !tbaa !2
  %1093 = fcmp ogt float %1091, %1092
  br i1 %1093, label %b146, label %b147

b145:
  store i16 0, ptr %134, !tbaa !2
  store i16 957, ptr %51, !tbaa !2
  store i16 1, ptr %50, !tbaa !2
  br label %b148

b146:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b147:
  %1094 = call float @llvm.sin.f32(float %1090)
  store i16 63, ptr %53, !tbaa !2
  %1095 = load i16, ptr %53, !tbaa !2
  %1096 = sitofp i16 %1095 to float
  %1097 = fmul float %1094, %1096
  store i16 92, ptr %52, !tbaa !2
  %1098 = load i16, ptr %52, !tbaa !2
  %1099 = sitofp i16 %1098 to float
  %1100 = fadd float %1097, %1099
  %1101 = call i16 @llvm.lrint.i16.f32(float %1100)
  %1102 = sub i16 %1084, -48
  %1103 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %1102
  store i16 %1101, ptr %1103, !tbaa !2
  %1104 = load i16, ptr %134, !tbaa !2
  %1105 = load i16, ptr %56, !tbaa !2
  %1106 = add nsw i16 %1104, %1105
  store i16 %1106, ptr %134, !tbaa !2
  br label %b141

b148:
  %1107 = load i16, ptr %50, !tbaa !2
  %1108 = icmp sge i16 %1107, 0
  br i1 %1108, label %b149, label %b150

b149:
  %1109 = load i16, ptr %134, !tbaa !2
  %1110 = load i16, ptr %51, !tbaa !2
  %1111 = icmp sle i16 %1109, %1110
  br i1 %1111, label %b151, label %b152

b150:
  %1112 = load i16, ptr %134, !tbaa !2
  %1113 = load i16, ptr %51, !tbaa !2
  %1114 = icmp sge i16 %1112, %1113
  br i1 %1114, label %b151, label %b152

b151:
  %1115 = load i16, ptr %134, !tbaa !2
  %1116 = load i16, ptr %134, !tbaa !2
  store i16 %1116, ptr %49, !tbaa !2
  %1117 = load i16, ptr %49, !tbaa !2
  %1118 = sitofp i16 %1117 to float
  store i16 25, ptr %48, !tbaa !2
  %1119 = load i16, ptr %48, !tbaa !2
  %1120 = sitofp i16 %1119 to float
  %1121 = fdiv float %1118, %1120
  %1122 = call float @llvm.fabs.f32(float %1121)
  %1123 = load float, ptr @$float86, !tbaa !2
  %1124 = fcmp ogt float %1122, %1123
  br i1 %1124, label %b153, label %b154

b152:
  store i16 -471, ptr %134, !tbaa !2
  store i16 871, ptr %45, !tbaa !2
  store i16 1, ptr %44, !tbaa !2
  br label %b155

b153:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b154:
  %1125 = call float @llvm.sin.f32(float %1121)
  store i16 31, ptr %47, !tbaa !2
  %1126 = load i16, ptr %47, !tbaa !2
  %1127 = sitofp i16 %1126 to float
  %1128 = fmul float %1125, %1127
  store i16 31, ptr %46, !tbaa !2
  %1129 = load i16, ptr %46, !tbaa !2
  %1130 = sitofp i16 %1129 to float
  %1131 = fadd float %1128, %1130
  %1132 = call i16 @llvm.lrint.i16.f32(float %1131)
  %1133 = sub i16 %1115, -640
  %1134 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1133
  store i16 %1132, ptr %1134, !tbaa !2
  %1135 = load i16, ptr %134, !tbaa !2
  %1136 = load i16, ptr %50, !tbaa !2
  %1137 = add nsw i16 %1135, %1136
  store i16 %1137, ptr %134, !tbaa !2
  br label %b148

b155:
  %1138 = load i16, ptr %44, !tbaa !2
  %1139 = icmp sge i16 %1138, 0
  br i1 %1139, label %b156, label %b157

b156:
  %1140 = load i16, ptr %134, !tbaa !2
  %1141 = load i16, ptr %45, !tbaa !2
  %1142 = icmp sle i16 %1140, %1141
  br i1 %1142, label %b158, label %b159

b157:
  %1143 = load i16, ptr %134, !tbaa !2
  %1144 = load i16, ptr %45, !tbaa !2
  %1145 = icmp sge i16 %1143, %1144
  br i1 %1145, label %b158, label %b159

b158:
  %1146 = load i16, ptr %134, !tbaa !2
  %1147 = load i16, ptr %134, !tbaa !2
  store i16 %1147, ptr %43, !tbaa !2
  %1148 = load i16, ptr %43, !tbaa !2
  %1149 = sitofp i16 %1148 to float
  store i16 75, ptr %42, !tbaa !2
  %1150 = load i16, ptr %42, !tbaa !2
  %1151 = sitofp i16 %1150 to float
  %1152 = fdiv float %1149, %1151
  %1153 = call float @llvm.fabs.f32(float %1152)
  %1154 = load float, ptr @$float86, !tbaa !2
  %1155 = fcmp ogt float %1153, %1154
  br i1 %1155, label %b160, label %b161

b159:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 15, i16 2, i16 257, ptr %139)
  store i16 0, ptr %134, !tbaa !2
  store i16 15, ptr %39, !tbaa !2
  store i16 1, ptr %38, !tbaa !2
  br label %b162

b160:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b161:
  %1156 = call float @llvm.sin.f32(float %1152)
  store i16 91, ptr %41, !tbaa !2
  %1157 = load i16, ptr %41, !tbaa !2
  %1158 = sitofp i16 %1157 to float
  %1159 = fmul float %1156, %1158
  store i16 127, ptr %40, !tbaa !2
  %1160 = load i16, ptr %40, !tbaa !2
  %1161 = sitofp i16 %1160 to float
  %1162 = fadd float %1159, %1161
  %1163 = call i16 @llvm.lrint.i16.f32(float %1162)
  %1164 = sub i16 %1146, -640
  %1165 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1164
  store i16 %1163, ptr %1165, !tbaa !2
  %1166 = load i16, ptr %134, !tbaa !2
  %1167 = load i16, ptr %44, !tbaa !2
  %1168 = add nsw i16 %1166, %1167
  store i16 %1168, ptr %134, !tbaa !2
  br label %b155

b162:
  %1169 = load i16, ptr %38, !tbaa !2
  %1170 = icmp sge i16 %1169, 0
  br i1 %1170, label %b163, label %b164

b163:
  %1171 = load i16, ptr %134, !tbaa !2
  %1172 = load i16, ptr %39, !tbaa !2
  %1173 = icmp sle i16 %1171, %1172
  br i1 %1173, label %b165, label %b166

b164:
  %1174 = load i16, ptr %134, !tbaa !2
  %1175 = load i16, ptr %39, !tbaa !2
  %1176 = icmp sge i16 %1174, %1175
  br i1 %1176, label %b165, label %b166

b165:
  %1177 = load i16, ptr %134, !tbaa !2
  %1178 = mul i16 %1177, 2
  %1179 = getelementptr i8, ptr %139, i16 2
  %1180 = load i16, ptr %1179, !tbaa !2
  %1181 = add i16 0, %1178
  %1182 = inttoptr i16 %1180 to ptr addrspace(2)
  %1183 = addrspacecast ptr addrspace(2) %1182 to ptr addrspace(1)
  %1184 = getelementptr i8, ptr addrspace(1) %1183, i16 %1181
  %1185 = load i16, ptr %134, !tbaa !2
  %1186 = mul i16 %1185, 16
  store i16 %1186, ptr addrspace(1) %1184, !tbaa !38
  %1187 = load i16, ptr %134, !tbaa !2
  %1188 = load i16, ptr %38, !tbaa !2
  %1189 = add nsw i16 %1187, %1188
  store i16 %1189, ptr %134, !tbaa !2
  br label %b162

b166:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 7, i16 2, i16 257, ptr %138)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 7, i16 2, i16 257, ptr %137)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 7, i16 2, i16 257, ptr %136)
  store i16 0, ptr %134, !tbaa !2
  store i16 5, ptr %37, !tbaa !2
  store i16 1, ptr %36, !tbaa !2
  br label %b167

b167:
  %1190 = load i16, ptr %36, !tbaa !2
  %1191 = icmp sge i16 %1190, 0
  br i1 %1191, label %b168, label %b169

b168:
  %1192 = load i16, ptr %134, !tbaa !2
  %1193 = load i16, ptr %37, !tbaa !2
  %1194 = icmp sle i16 %1192, %1193
  br i1 %1194, label %b170, label %b171

b169:
  %1195 = load i16, ptr %134, !tbaa !2
  %1196 = load i16, ptr %37, !tbaa !2
  %1197 = icmp sge i16 %1195, %1196
  br i1 %1197, label %b170, label %b171

b170:
  %1198 = load i16, ptr %134, !tbaa !2
  %1199 = mul i16 %1198, 2
  %1200 = getelementptr i8, ptr %138, i16 2
  %1201 = load i16, ptr %1200, !tbaa !2
  %1202 = add i16 0, %1199
  %1203 = inttoptr i16 %1201 to ptr addrspace(2)
  %1204 = addrspacecast ptr addrspace(2) %1203 to ptr addrspace(1)
  %1205 = getelementptr i8, ptr addrspace(1) %1204, i16 %1202
  %1206 = load i16, ptr %134, !tbaa !2
  %1207 = add i16 %1206, 1
  store i16 %1207, ptr addrspace(1) %1205, !tbaa !40
  %1208 = load i16, ptr %134, !tbaa !2
  %1209 = mul i16 %1208, 2
  %1210 = getelementptr i8, ptr %137, i16 2
  %1211 = load i16, ptr %1210, !tbaa !2
  %1212 = add i16 0, %1209
  %1213 = inttoptr i16 %1211 to ptr addrspace(2)
  %1214 = addrspacecast ptr addrspace(2) %1213 to ptr addrspace(1)
  %1215 = getelementptr i8, ptr addrspace(1) %1214, i16 %1212
  %1216 = load i16, ptr %134, !tbaa !2
  %1217 = add i16 %1216, 7
  store i16 %1217, ptr addrspace(1) %1215, !tbaa !42
  %1218 = load i16, ptr %134, !tbaa !2
  %1219 = mul i16 %1218, 2
  %1220 = getelementptr i8, ptr %136, i16 2
  %1221 = load i16, ptr %1220, !tbaa !2
  %1222 = add i16 0, %1219
  %1223 = inttoptr i16 %1221 to ptr addrspace(2)
  %1224 = addrspacecast ptr addrspace(2) %1223 to ptr addrspace(1)
  %1225 = getelementptr i8, ptr addrspace(1) %1224, i16 %1222
  %1226 = load i16, ptr %134, !tbaa !2
  %1227 = add i16 %1226, 13
  store i16 %1227, ptr addrspace(1) %1225, !tbaa !44
  %1228 = load i16, ptr %134, !tbaa !2
  %1229 = load i16, ptr %36, !tbaa !2
  %1230 = add nsw i16 %1228, %1229
  store i16 %1230, ptr %134, !tbaa !2
  br label %b167

b171:
  store i16 0, ptr %134, !tbaa !2
  store i16 399, ptr %35, !tbaa !2
  store i16 1, ptr %34, !tbaa !2
  br label %b172

b172:
  %1231 = load i16, ptr %34, !tbaa !2
  %1232 = icmp sge i16 %1231, 0
  br i1 %1232, label %b173, label %b174

b173:
  %1233 = load i16, ptr %134, !tbaa !2
  %1234 = load i16, ptr %35, !tbaa !2
  %1235 = icmp sle i16 %1233, %1234
  br i1 %1235, label %b175, label %b176

b174:
  %1236 = load i16, ptr %134, !tbaa !2
  %1237 = load i16, ptr %35, !tbaa !2
  %1238 = icmp sge i16 %1236, %1237
  br i1 %1238, label %b175, label %b176

b175:
  %1239 = load i16, ptr %134, !tbaa !2
  %1240 = mul i16 %1239, 2
  %1241 = getelementptr i8, ptr %143, i16 2
  %1242 = load i16, ptr %1241, !tbaa !2
  %1243 = add i16 0, %1240
  %1244 = inttoptr i16 %1242 to ptr addrspace(2)
  %1245 = addrspacecast ptr addrspace(2) %1244 to ptr addrspace(1)
  %1246 = getelementptr i8, ptr addrspace(1) %1245, i16 %1243
  %1247 = load i16, ptr %134, !tbaa !2
  %1248 = mul i16 %1247, 2
  store i16 %1248, ptr addrspace(1) %1246, !tbaa !46
  %1249 = load i16, ptr %134, !tbaa !2
  %1250 = load i16, ptr %34, !tbaa !2
  %1251 = add nsw i16 %1249, %1250
  store i16 %1251, ptr %134, !tbaa !2
  br label %b172

b176:
  store i16 0, ptr %134, !tbaa !2
  store i16 399, ptr %33, !tbaa !2
  store i16 1, ptr %32, !tbaa !2
  br label %b177

b177:
  %1252 = load i16, ptr %32, !tbaa !2
  %1253 = icmp sge i16 %1252, 0
  br i1 %1253, label %b178, label %b179

b178:
  %1254 = load i16, ptr %134, !tbaa !2
  %1255 = load i16, ptr %33, !tbaa !2
  %1256 = icmp sle i16 %1254, %1255
  br i1 %1256, label %b180, label %b181

b179:
  %1257 = load i16, ptr %134, !tbaa !2
  %1258 = load i16, ptr %33, !tbaa !2
  %1259 = icmp sge i16 %1257, %1258
  br i1 %1259, label %b180, label %b181

b180:
  %1260 = load i16, ptr %134, !tbaa !2
  %1261 = mul i16 %1260, 2
  %1262 = getelementptr i8, ptr %142, i16 2
  %1263 = load i16, ptr %1262, !tbaa !2
  %1264 = add i16 0, %1261
  %1265 = inttoptr i16 %1263 to ptr addrspace(2)
  %1266 = addrspacecast ptr addrspace(2) %1265 to ptr addrspace(1)
  %1267 = getelementptr i8, ptr addrspace(1) %1266, i16 %1264
  %1268 = load i16, ptr %134, !tbaa !2
  %1269 = load float, ptr @$float212, !tbaa !2
  store i16 %1268, ptr %31, !tbaa !2
  %1270 = load i16, ptr %31, !tbaa !2
  %1271 = sitofp i16 %1270 to float
  %1272 = fdiv float %1271, %1269
  %1273 = call i16 @llvm.lrint.i16.f32(float %1272)
  store i16 %1273, ptr addrspace(1) %1267, !tbaa !48
  %1274 = load i16, ptr %134, !tbaa !2
  %1275 = load i16, ptr %32, !tbaa !2
  %1276 = add nsw i16 %1274, %1275
  store i16 %1276, ptr %134, !tbaa !2
  br label %b177

b181:
  store i16 0, ptr %30, !tbaa !2
  store i16 0, ptr %29, !tbaa !2
  br label %b182

b182:
  %1277 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %1278 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %1277, ptr @$string213)
  %1279 = icmp eq i16 %1278, 0
  br i1 %1279, label %b183, label %b184

b183:
  %1280 = load i16, ptr %135, !tbaa !2
  %1281 = add i16 %1280, 3
  store i16 %1281, ptr %135, !tbaa !2
  %1282 = load i16, ptr %135, !tbaa !2
  %1283 = icmp sge i16 %1282, 283
  br i1 %1283, label %b185, label %b186

b184:
  br label %b3

b185:
  store i16 0, ptr %135, !tbaa !2
  br label %b187

b186:
  br label %b187

b187:
  %1284 = load i16, ptr %28, !tbaa !2
  %1285 = add i16 %1284, 2
  store i16 %1285, ptr %28, !tbaa !2
  %1286 = load i16, ptr %28, !tbaa !2
  %1287 = icmp sge i16 %1286, 157
  br i1 %1287, label %b188, label %b189

b188:
  store i16 0, ptr %28, !tbaa !2
  br label %b190

b189:
  br label %b190

b190:
  %1288 = load i16, ptr %27, !tbaa !2
  %1289 = add i16 %1288, 1
  store i16 %1289, ptr %27, !tbaa !2
  %1290 = load i16, ptr %27, !tbaa !2
  %1291 = icmp sge i16 %1290, 471
  br i1 %1291, label %b191, label %b192

b191:
  store i16 0, ptr %27, !tbaa !2
  br label %b193

b192:
  br label %b193

b193:
  %1292 = load i16, ptr %29, !tbaa !2
  %1293 = add i16 %1292, 1
  store i16 %1293, ptr %29, !tbaa !2
  %1294 = load i16, ptr %29, !tbaa !2
  %1295 = icmp slt i16 %1294, 400
  %1296 = sext i1 %1295 to i16
  %1297 = load i16, ptr %30, !tbaa !2
  %1298 = icmp slt i16 %1297, 395
  %1299 = sext i1 %1298 to i16
  %1300 = and i16 %1296, %1299
  %1301 = icmp ne i16 %1300, 0
  br i1 %1301, label %b194, label %b195

b194:
  %1302 = load i16, ptr %30, !tbaa !2
  %1303 = add i16 %1302, 1
  store i16 %1303, ptr %30, !tbaa !2
  br label %b196

b195:
  br label %b196

b196:
  %1304 = load i16, ptr %29, !tbaa !2
  %1305 = icmp sgt i16 %1304, 1200
  %1306 = sext i1 %1305 to i16
  %1307 = load i16, ptr %30, !tbaa !2
  %1308 = icmp ne i16 %1307, 0
  %1309 = sext i1 %1308 to i16
  %1310 = and i16 %1306, %1309
  %1311 = icmp ne i16 %1310, 0
  br i1 %1311, label %b197, label %b198

b197:
  %1312 = load i16, ptr %30, !tbaa !2
  %1313 = sub i16 %1312, 1
  store i16 %1313, ptr %30, !tbaa !2
  br label %b199

b198:
  br label %b199

b199:
  %1314 = load i16, ptr %30, !tbaa !2
  %1315 = icmp eq i16 %1314, 0
  br i1 %1315, label %b200, label %b201

b200:
  br label %b3

b201:
  br label %b202

b202:
  %1316 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1316, ptr @"BENCHFRAME&", !tbaa !2
  %1317 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1317, ptr @"BENCHFRAME&", !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  br label %b2

b204:
  %1318 = load i16, ptr %24, !tbaa !2
  %1319 = icmp sge i16 %1318, 0
  br i1 %1319, label %b205, label %b206

b205:
  %1320 = load i16, ptr %26, !tbaa !2
  %1321 = load i16, ptr %25, !tbaa !2
  %1322 = icmp sle i16 %1320, %1321
  br i1 %1322, label %b207, label %b208

b206:
  %1323 = load i16, ptr %26, !tbaa !2
  %1324 = load i16, ptr %25, !tbaa !2
  %1325 = icmp sge i16 %1323, %1324
  br i1 %1325, label %b207, label %b208

b207:
  %1326 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1326, ptr @"BENCHFRAME&", !tbaa !2
  %1327 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1327, ptr @"BENCHFRAME&", !tbaa !2
  %1328 = load i16, ptr %26, !tbaa !2
  %1329 = mul i16 %1328, 2
  %1330 = getelementptr i8, ptr %143, i16 2
  %1331 = load i16, ptr %1330, !tbaa !2
  %1332 = add i16 0, %1329
  %1333 = inttoptr i16 %1331 to ptr addrspace(2)
  %1334 = addrspacecast ptr addrspace(2) %1333 to ptr addrspace(1)
  %1335 = getelementptr i8, ptr addrspace(1) %1334, i16 %1332
  %1336 = load i16, ptr addrspace(1) %1335, !tbaa !46
  %1337 = load i16, ptr %28, !tbaa !2
  %1338 = add i16 %1336, %1337
  %1339 = sub i16 %1338, -640
  %1340 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1339
  %1341 = load i16, ptr %1340, !tbaa !2
  %1342 = load i16, ptr %26, !tbaa !2
  %1343 = load i16, ptr %27, !tbaa !2
  %1344 = sub i16 %1342, %1343
  %1345 = sub i16 %1344, -640
  %1346 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1345
  %1347 = load i16, ptr %1346, !tbaa !2
  %1348 = add i16 %1341, %1347
  store i16 %1348, ptr %23, !tbaa !2
  %1349 = load i16, ptr %26, !tbaa !2
  %1350 = load i16, ptr %28, !tbaa !2
  %1351 = add i16 %1349, %1350
  %1352 = sub i16 %1351, -640
  %1353 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1352
  %1354 = load i16, ptr %1353, !tbaa !2
  %1355 = load i16, ptr %26, !tbaa !2
  %1356 = mul i16 %1355, 2
  %1357 = getelementptr i8, ptr %143, i16 2
  %1358 = load i16, ptr %1357, !tbaa !2
  %1359 = add i16 0, %1356
  %1360 = inttoptr i16 %1358 to ptr addrspace(2)
  %1361 = addrspacecast ptr addrspace(2) %1360 to ptr addrspace(1)
  %1362 = getelementptr i8, ptr addrspace(1) %1361, i16 %1359
  %1363 = load i16, ptr addrspace(1) %1362, !tbaa !46
  %1364 = load i16, ptr %135, !tbaa !2
  %1365 = add i16 %1363, %1364
  %1366 = sub i16 %1365, -48
  %1367 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %1366
  %1368 = load i16, ptr %1367, !tbaa !2
  %1369 = add i16 %1354, %1368
  store i16 %1369, ptr %22, !tbaa !2
  store i16 1, ptr %134, !tbaa !2
  store i16 15, ptr %21, !tbaa !2
  store i16 1, ptr %20, !tbaa !2
  br label %b209

b208:
  store i16 0, ptr %134, !tbaa !2
  store i16 319, ptr %17, !tbaa !2
  store i16 1, ptr %16, !tbaa !2
  br label %b214

b209:
  %1370 = load i16, ptr %20, !tbaa !2
  %1371 = icmp sge i16 %1370, 0
  br i1 %1371, label %b210, label %b211

b210:
  %1372 = load i16, ptr %134, !tbaa !2
  %1373 = load i16, ptr %21, !tbaa !2
  %1374 = icmp sle i16 %1372, %1373
  br i1 %1374, label %b212, label %b213

b211:
  %1375 = load i16, ptr %134, !tbaa !2
  %1376 = load i16, ptr %21, !tbaa !2
  %1377 = icmp sge i16 %1375, %1376
  br i1 %1377, label %b212, label %b213

b212:
  %1378 = load i16, ptr %23, !tbaa !2
  %1379 = load i16, ptr %134, !tbaa !2
  %1380 = add i16 %1378, %1379
  %1381 = mul i16 %1380, 2
  %1382 = getelementptr i8, ptr %145, i16 2
  %1383 = load i16, ptr %1382, !tbaa !2
  %1384 = add i16 1280, %1381
  %1385 = inttoptr i16 %1383 to ptr addrspace(2)
  %1386 = addrspacecast ptr addrspace(2) %1385 to ptr addrspace(1)
  %1387 = getelementptr i8, ptr addrspace(1) %1386, i16 %1384
  %1388 = load i16, ptr %134, !tbaa !2
  store i16 %1388, ptr addrspace(1) %1387, !tbaa !50
  %1389 = load i16, ptr %22, !tbaa !2
  %1390 = load i16, ptr %134, !tbaa !2
  %1391 = add i16 %1389, %1390
  %1392 = mul i16 %1391, 2
  %1393 = getelementptr i8, ptr %144, i16 2
  %1394 = load i16, ptr %1393, !tbaa !2
  %1395 = add i16 1280, %1392
  %1396 = inttoptr i16 %1394 to ptr addrspace(2)
  %1397 = addrspacecast ptr addrspace(2) %1396 to ptr addrspace(1)
  %1398 = getelementptr i8, ptr addrspace(1) %1397, i16 %1395
  %1399 = load i16, ptr %134, !tbaa !2
  %1400 = mul i16 %1399, 2
  %1401 = getelementptr i8, ptr %139, i16 2
  %1402 = load i16, ptr %1401, !tbaa !2
  %1403 = add i16 0, %1400
  %1404 = inttoptr i16 %1402 to ptr addrspace(2)
  %1405 = addrspacecast ptr addrspace(2) %1404 to ptr addrspace(1)
  %1406 = getelementptr i8, ptr addrspace(1) %1405, i16 %1403
  %1407 = load i16, ptr addrspace(1) %1406, !tbaa !38
  store i16 %1407, ptr addrspace(1) %1398, !tbaa !52
  %1408 = load i16, ptr %23, !tbaa !2
  %1409 = load i16, ptr %134, !tbaa !2
  %1410 = add i16 %1408, %1409
  %1411 = load i16, ptr %134, !tbaa !2
  %1412 = load i16, ptr %23, !tbaa !2
  %1413 = load i16, ptr %134, !tbaa !2
  %1414 = add i16 %1412, %1413
  %1415 = mul i16 %1414, 2
  %1416 = getelementptr i8, ptr %144, i16 2
  %1417 = load i16, ptr %1416, !tbaa !2
  %1418 = add i16 1280, %1415
  %1419 = inttoptr i16 %1417 to ptr addrspace(2)
  %1420 = addrspacecast ptr addrspace(2) %1419 to ptr addrspace(1)
  %1421 = getelementptr i8, ptr addrspace(1) %1420, i16 %1418
  %1422 = load i16, ptr addrspace(1) %1421, !tbaa !52
  %1423 = or i16 %1411, %1422
  %1424 = trunc i16 %1423 to i8
  %1425 = load i16, ptr @b$seg, !tbaa !2
  %1426 = inttoptr i16 %1425 to ptr addrspace(2)
  %1427 = addrspacecast ptr addrspace(2) %1426 to ptr addrspace(1)
  %1428 = getelementptr i8, ptr addrspace(1) %1427, i16 %1410
  store volatile i8 %1424, ptr addrspace(1) %1428, !tbaa !4
  %1429 = load i16, ptr %22, !tbaa !2
  %1430 = load i16, ptr %134, !tbaa !2
  %1431 = add i16 %1429, %1430
  %1432 = load i16, ptr %134, !tbaa !2
  %1433 = mul i16 %1432, 2
  %1434 = getelementptr i8, ptr %139, i16 2
  %1435 = load i16, ptr %1434, !tbaa !2
  %1436 = add i16 0, %1433
  %1437 = inttoptr i16 %1435 to ptr addrspace(2)
  %1438 = addrspacecast ptr addrspace(2) %1437 to ptr addrspace(1)
  %1439 = getelementptr i8, ptr addrspace(1) %1438, i16 %1436
  %1440 = load i16, ptr addrspace(1) %1439, !tbaa !38
  %1441 = load i16, ptr %22, !tbaa !2
  %1442 = load i16, ptr %134, !tbaa !2
  %1443 = add i16 %1441, %1442
  %1444 = mul i16 %1443, 2
  %1445 = getelementptr i8, ptr %145, i16 2
  %1446 = load i16, ptr %1445, !tbaa !2
  %1447 = add i16 1280, %1444
  %1448 = inttoptr i16 %1446 to ptr addrspace(2)
  %1449 = addrspacecast ptr addrspace(2) %1448 to ptr addrspace(1)
  %1450 = getelementptr i8, ptr addrspace(1) %1449, i16 %1447
  %1451 = load i16, ptr addrspace(1) %1450, !tbaa !50
  %1452 = or i16 %1440, %1451
  %1453 = trunc i16 %1452 to i8
  %1454 = load i16, ptr @b$seg, !tbaa !2
  %1455 = inttoptr i16 %1454 to ptr addrspace(2)
  %1456 = addrspacecast ptr addrspace(2) %1455 to ptr addrspace(1)
  %1457 = getelementptr i8, ptr addrspace(1) %1456, i16 %1431
  store volatile i8 %1453, ptr addrspace(1) %1457, !tbaa !4
  %1458 = load i16, ptr %134, !tbaa !2
  %1459 = load i16, ptr %20, !tbaa !2
  %1460 = add nsw i16 %1458, %1459
  store i16 %1460, ptr %134, !tbaa !2
  br label %b209

b213:
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  %1461 = load i16, ptr %26, !tbaa !2
  %1462 = mul i16 %1461, 2
  %1463 = getelementptr i8, ptr %142, i16 2
  %1464 = load i16, ptr %1463, !tbaa !2
  %1465 = add i16 0, %1462
  %1466 = inttoptr i16 %1464 to ptr addrspace(2)
  %1467 = addrspacecast ptr addrspace(2) %1466 to ptr addrspace(1)
  %1468 = getelementptr i8, ptr addrspace(1) %1467, i16 %1465
  %1469 = load i16, ptr addrspace(1) %1468, !tbaa !48
  %1470 = sub i16 63, %1469
  %1471 = trunc i16 %1470 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1471)
  %1472 = load i16, ptr %26, !tbaa !2
  %1473 = mul i16 %1472, 2
  %1474 = getelementptr i8, ptr %142, i16 2
  %1475 = load i16, ptr %1474, !tbaa !2
  %1476 = add i16 0, %1473
  %1477 = inttoptr i16 %1475 to ptr addrspace(2)
  %1478 = addrspacecast ptr addrspace(2) %1477 to ptr addrspace(1)
  %1479 = getelementptr i8, ptr addrspace(1) %1478, i16 %1476
  %1480 = load i16, ptr addrspace(1) %1479, !tbaa !48
  %1481 = sub i16 63, %1480
  store i16 %1481, ptr %19, !tbaa !2
  %1482 = load i16, ptr %19, !tbaa !2
  %1483 = sitofp i16 %1482 to float
  store i16 2, ptr %18, !tbaa !2
  %1484 = load i16, ptr %18, !tbaa !2
  %1485 = sitofp i16 %1484 to float
  %1486 = fdiv float %1483, %1485
  %1487 = call i16 @llvm.lrint.i16.f32(float %1486)
  %1488 = trunc i16 %1487 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1488)
  %1489 = load i16, ptr %26, !tbaa !2
  %1490 = mul i16 %1489, 2
  %1491 = getelementptr i8, ptr %142, i16 2
  %1492 = load i16, ptr %1491, !tbaa !2
  %1493 = add i16 0, %1490
  %1494 = inttoptr i16 %1492 to ptr addrspace(2)
  %1495 = addrspacecast ptr addrspace(2) %1494 to ptr addrspace(1)
  %1496 = getelementptr i8, ptr addrspace(1) %1495, i16 %1493
  %1497 = load i16, ptr addrspace(1) %1496, !tbaa !48
  %1498 = trunc i16 %1497 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1498)
  %1499 = load i16, ptr %26, !tbaa !2
  %1500 = load i16, ptr %24, !tbaa !2
  %1501 = add nsw i16 %1499, %1500
  store i16 %1501, ptr %26, !tbaa !2
  br label %b204

b214:
  %1502 = load i16, ptr %16, !tbaa !2
  %1503 = icmp sge i16 %1502, 0
  br i1 %1503, label %b215, label %b216

b215:
  %1504 = load i16, ptr %134, !tbaa !2
  %1505 = load i16, ptr %17, !tbaa !2
  %1506 = icmp sle i16 %1504, %1505
  br i1 %1506, label %b217, label %b218

b216:
  %1507 = load i16, ptr %134, !tbaa !2
  %1508 = load i16, ptr %17, !tbaa !2
  %1509 = icmp sge i16 %1507, %1508
  br i1 %1509, label %b217, label %b218

b217:
  %1510 = load i16, ptr %134, !tbaa !2
  %1511 = load i16, ptr @b$seg, !tbaa !2
  %1512 = inttoptr i16 %1511 to ptr addrspace(2)
  %1513 = addrspacecast ptr addrspace(2) %1512 to ptr addrspace(1)
  %1514 = getelementptr i8, ptr addrspace(1) %1513, i16 %1510
  store volatile i8 0, ptr addrspace(1) %1514, !tbaa !4
  %1515 = load i16, ptr %134, !tbaa !2
  %1516 = load i16, ptr %16, !tbaa !2
  %1517 = add nsw i16 %1515, %1516
  store i16 %1517, ptr %134, !tbaa !2
  br label %b214

b218:
  store i16 0, ptr %134, !tbaa !2
  store i16 319, ptr %15, !tbaa !2
  store i16 1, ptr %14, !tbaa !2
  br label %b219

b219:
  %1518 = load i16, ptr %14, !tbaa !2
  %1519 = icmp sge i16 %1518, 0
  br i1 %1519, label %b220, label %b221

b220:
  %1520 = load i16, ptr %134, !tbaa !2
  %1521 = load i16, ptr %15, !tbaa !2
  %1522 = icmp sle i16 %1520, %1521
  br i1 %1522, label %b222, label %b223

b221:
  %1523 = load i16, ptr %134, !tbaa !2
  %1524 = load i16, ptr %15, !tbaa !2
  %1525 = icmp sge i16 %1523, %1524
  br i1 %1525, label %b222, label %b223

b222:
  %1526 = load i16, ptr %134, !tbaa !2
  %1527 = mul i16 %1526, 2
  %1528 = getelementptr i8, ptr %145, i16 2
  %1529 = load i16, ptr %1528, !tbaa !2
  %1530 = add i16 1280, %1527
  %1531 = inttoptr i16 %1529 to ptr addrspace(2)
  %1532 = addrspacecast ptr addrspace(2) %1531 to ptr addrspace(1)
  %1533 = getelementptr i8, ptr addrspace(1) %1532, i16 %1530
  store i16 0, ptr addrspace(1) %1533, !tbaa !50
  %1534 = load i16, ptr %134, !tbaa !2
  %1535 = mul i16 %1534, 2
  %1536 = getelementptr i8, ptr %144, i16 2
  %1537 = load i16, ptr %1536, !tbaa !2
  %1538 = add i16 1280, %1535
  %1539 = inttoptr i16 %1537 to ptr addrspace(2)
  %1540 = addrspacecast ptr addrspace(2) %1539 to ptr addrspace(1)
  %1541 = getelementptr i8, ptr addrspace(1) %1540, i16 %1538
  store i16 0, ptr addrspace(1) %1541, !tbaa !52
  %1542 = load i16, ptr %134, !tbaa !2
  %1543 = load i16, ptr %14, !tbaa !2
  %1544 = add nsw i16 %1542, %1543
  store i16 %1544, ptr %134, !tbaa !2
  br label %b219

b223:
  br label %b182

b224:
  %1545 = load i16, ptr %12, !tbaa !2
  %1546 = icmp sge i16 %1545, 0
  br i1 %1546, label %b225, label %b226

b225:
  %1547 = load i16, ptr %134, !tbaa !2
  %1548 = load i16, ptr %13, !tbaa !2
  %1549 = icmp sle i16 %1547, %1548
  br i1 %1549, label %b227, label %b228

b226:
  %1550 = load i16, ptr %134, !tbaa !2
  %1551 = load i16, ptr %13, !tbaa !2
  %1552 = icmp sge i16 %1550, %1551
  br i1 %1552, label %b227, label %b228

b227:
  %1553 = load i16, ptr %134, !tbaa !2
  %1554 = trunc i16 %1553 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %1554)
  %1555 = load i16, ptr %134, !tbaa !2
  %1556 = mul i16 %1555, 5
  %1557 = add i16 %1556, 28
  %1558 = trunc i16 %1557 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1558)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %1559 = load i16, ptr %134, !tbaa !2
  %1560 = load i16, ptr %12, !tbaa !2
  %1561 = add nsw i16 %1559, %1560
  store i16 %1561, ptr %134, !tbaa !2
  br label %b224

b228:
  store i16 8, ptr %134, !tbaa !2
  store i16 15, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b229

b229:
  %1562 = load i16, ptr %10, !tbaa !2
  %1563 = icmp sge i16 %1562, 0
  br i1 %1563, label %b230, label %b231

b230:
  %1564 = load i16, ptr %134, !tbaa !2
  %1565 = load i16, ptr %11, !tbaa !2
  %1566 = icmp sle i16 %1564, %1565
  br i1 %1566, label %b232, label %b233

b231:
  %1567 = load i16, ptr %134, !tbaa !2
  %1568 = load i16, ptr %11, !tbaa !2
  %1569 = icmp sge i16 %1567, %1568
  br i1 %1569, label %b232, label %b233

b232:
  %1570 = load i16, ptr %134, !tbaa !2
  %1571 = trunc i16 %1570 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %1571)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %1572 = load i16, ptr %134, !tbaa !2
  %1573 = sub i16 %1572, 8
  %1574 = mul i16 %1573, 5
  %1575 = add i16 %1574, 28
  %1576 = trunc i16 %1575 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1576)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %1577 = load i16, ptr %134, !tbaa !2
  %1578 = load i16, ptr %10, !tbaa !2
  %1579 = add nsw i16 %1577, %1578
  store i16 %1579, ptr %134, !tbaa !2
  br label %b229

b233:
  store i16 16, ptr %134, !tbaa !2
  store i16 23, ptr %9, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  br label %b234

b234:
  %1580 = load i16, ptr %8, !tbaa !2
  %1581 = icmp sge i16 %1580, 0
  br i1 %1581, label %b235, label %b236

b235:
  %1582 = load i16, ptr %134, !tbaa !2
  %1583 = load i16, ptr %9, !tbaa !2
  %1584 = icmp sle i16 %1582, %1583
  br i1 %1584, label %b237, label %b238

b236:
  %1585 = load i16, ptr %134, !tbaa !2
  %1586 = load i16, ptr %9, !tbaa !2
  %1587 = icmp sge i16 %1585, %1586
  br i1 %1587, label %b237, label %b238

b237:
  %1588 = load i16, ptr %134, !tbaa !2
  %1589 = trunc i16 %1588 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %1589)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %1590 = load i16, ptr %134, !tbaa !2
  %1591 = sub i16 %1590, 16
  %1592 = mul i16 %1591, 5
  %1593 = add i16 %1592, 28
  %1594 = trunc i16 %1593 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1594)
  %1595 = load i16, ptr %134, !tbaa !2
  %1596 = load i16, ptr %8, !tbaa !2
  %1597 = add nsw i16 %1595, %1596
  store i16 %1597, ptr %134, !tbaa !2
  br label %b234

b238:
  call void @llrm.ia16.out.i8(i16 980, i8 19)
  call void @llrm.ia16.out.i8(i16 981, i8 0)
  store i16 0, ptr %134, !tbaa !2
  store i16 7, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b239

b239:
  %1598 = load i16, ptr %6, !tbaa !2
  %1599 = icmp sge i16 %1598, 0
  br i1 %1599, label %b240, label %b241

b240:
  %1600 = load i16, ptr %134, !tbaa !2
  %1601 = load i16, ptr %7, !tbaa !2
  %1602 = icmp sle i16 %1600, %1601
  br i1 %1602, label %b242, label %b243

b241:
  %1603 = load i16, ptr %134, !tbaa !2
  %1604 = load i16, ptr %7, !tbaa !2
  %1605 = icmp sge i16 %1603, %1604
  br i1 %1605, label %b242, label %b243

b242:
  %1606 = load i16, ptr %134, !tbaa !2
  %1607 = mul i16 %1606, 2
  %1608 = getelementptr i8, ptr %138, i16 2
  %1609 = load i16, ptr %1608, !tbaa !2
  %1610 = add i16 0, %1607
  %1611 = inttoptr i16 %1609 to ptr addrspace(2)
  %1612 = addrspacecast ptr addrspace(2) %1611 to ptr addrspace(1)
  %1613 = getelementptr i8, ptr addrspace(1) %1612, i16 %1610
  %1614 = load i16, ptr %134, !tbaa !2
  store i16 %1614, ptr addrspace(1) %1613, !tbaa !40
  %1615 = load i16, ptr %134, !tbaa !2
  %1616 = mul i16 %1615, 2
  %1617 = getelementptr i8, ptr %137, i16 2
  %1618 = load i16, ptr %1617, !tbaa !2
  %1619 = add i16 0, %1616
  %1620 = inttoptr i16 %1618 to ptr addrspace(2)
  %1621 = addrspacecast ptr addrspace(2) %1620 to ptr addrspace(1)
  %1622 = getelementptr i8, ptr addrspace(1) %1621, i16 %1619
  %1623 = load i16, ptr %134, !tbaa !2
  %1624 = add i16 %1623, 8
  store i16 %1624, ptr addrspace(1) %1622, !tbaa !42
  %1625 = load i16, ptr %134, !tbaa !2
  %1626 = mul i16 %1625, 2
  %1627 = getelementptr i8, ptr %136, i16 2
  %1628 = load i16, ptr %1627, !tbaa !2
  %1629 = add i16 0, %1626
  %1630 = inttoptr i16 %1628 to ptr addrspace(2)
  %1631 = addrspacecast ptr addrspace(2) %1630 to ptr addrspace(1)
  %1632 = getelementptr i8, ptr addrspace(1) %1631, i16 %1629
  %1633 = load i16, ptr %134, !tbaa !2
  %1634 = add i16 %1633, 16
  store i16 %1634, ptr addrspace(1) %1632, !tbaa !44
  %1635 = load i16, ptr %134, !tbaa !2
  %1636 = load i16, ptr %6, !tbaa !2
  %1637 = add nsw i16 %1635, %1636
  store i16 %1637, ptr %134, !tbaa !2
  br label %b239

b243:
  %1638 = getelementptr i8, ptr %138, i16 2
  %1639 = load i16, ptr %1638, !tbaa !2
  %1640 = inttoptr i16 %1639 to ptr addrspace(2)
  %1641 = addrspacecast ptr addrspace(2) %1640 to ptr addrspace(1)
  %1642 = getelementptr i8, ptr addrspace(1) %1641, i16 0
  store i16 1, ptr addrspace(1) %1642, !tbaa !40
  store i16 0, ptr %135, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  store i16 0, ptr %29, !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  br label %b244

b244:
  %1643 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %1644 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %1643, ptr @$string214)
  %1645 = icmp eq i16 %1644, 0
  br i1 %1645, label %b245, label %b246

b245:
  %1646 = load i16, ptr %135, !tbaa !2
  %1647 = add i16 %1646, 3
  store i16 %1647, ptr %135, !tbaa !2
  %1648 = load i16, ptr %135, !tbaa !2
  %1649 = icmp sge i16 %1648, 283
  br i1 %1649, label %b247, label %b248

b246:
  br label %b4

b247:
  store i16 0, ptr %135, !tbaa !2
  br label %b249

b248:
  br label %b249

b249:
  %1650 = load i16, ptr %28, !tbaa !2
  %1651 = add i16 %1650, 2
  store i16 %1651, ptr %28, !tbaa !2
  %1652 = load i16, ptr %28, !tbaa !2
  %1653 = icmp sge i16 %1652, 157
  br i1 %1653, label %b250, label %b251

b250:
  store i16 0, ptr %28, !tbaa !2
  br label %b252

b251:
  br label %b252

b252:
  %1654 = load i16, ptr %27, !tbaa !2
  %1655 = add i16 %1654, 1
  store i16 %1655, ptr %27, !tbaa !2
  %1656 = load i16, ptr %27, !tbaa !2
  %1657 = icmp sge i16 %1656, 471
  br i1 %1657, label %b253, label %b254

b253:
  store i16 0, ptr %27, !tbaa !2
  br label %b255

b254:
  br label %b255

b255:
  %1658 = load i16, ptr %29, !tbaa !2
  %1659 = add i16 %1658, 1
  store i16 %1659, ptr %29, !tbaa !2
  %1660 = load i16, ptr %29, !tbaa !2
  %1661 = icmp slt i16 %1660, 400
  %1662 = sext i1 %1661 to i16
  %1663 = load i16, ptr %30, !tbaa !2
  %1664 = icmp slt i16 %1663, 395
  %1665 = sext i1 %1664 to i16
  %1666 = and i16 %1662, %1665
  %1667 = icmp ne i16 %1666, 0
  br i1 %1667, label %b256, label %b257

b256:
  %1668 = load i16, ptr %30, !tbaa !2
  %1669 = add i16 %1668, 1
  store i16 %1669, ptr %30, !tbaa !2
  br label %b258

b257:
  br label %b258

b258:
  %1670 = load i16, ptr %29, !tbaa !2
  %1671 = icmp sgt i16 %1670, 1200
  %1672 = sext i1 %1671 to i16
  %1673 = load i16, ptr %30, !tbaa !2
  %1674 = icmp ne i16 %1673, 0
  %1675 = sext i1 %1674 to i16
  %1676 = and i16 %1672, %1675
  %1677 = icmp ne i16 %1676, 0
  br i1 %1677, label %b259, label %b260

b259:
  %1678 = load i16, ptr %30, !tbaa !2
  %1679 = sub i16 %1678, 1
  store i16 %1679, ptr %30, !tbaa !2
  br label %b261

b260:
  br label %b261

b261:
  %1680 = load i16, ptr %30, !tbaa !2
  %1681 = icmp eq i16 %1680, 0
  br i1 %1681, label %b262, label %b263

b262:
  br label %b4

b263:
  br label %b264

b264:
  %1682 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1682, ptr @"BENCHFRAME&", !tbaa !2
  %1683 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1683, ptr @"BENCHFRAME&", !tbaa !2
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  %1684 = load i16, ptr %30, !tbaa !2
  store i16 %1684, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b266

b266:
  %1685 = load i16, ptr %4, !tbaa !2
  %1686 = icmp sge i16 %1685, 0
  br i1 %1686, label %b267, label %b268

b267:
  %1687 = load i16, ptr %26, !tbaa !2
  %1688 = load i16, ptr %5, !tbaa !2
  %1689 = icmp sle i16 %1687, %1688
  br i1 %1689, label %b269, label %b270

b268:
  %1690 = load i16, ptr %26, !tbaa !2
  %1691 = load i16, ptr %5, !tbaa !2
  %1692 = icmp sge i16 %1690, %1691
  br i1 %1692, label %b269, label %b270

b269:
  %1693 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1693, ptr @"BENCHFRAME&", !tbaa !2
  store i16 0, ptr %134, !tbaa !2
  store i16 7, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b271

b270:
  store i16 0, ptr %134, !tbaa !2
  store i16 319, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b276

b271:
  %1694 = load i16, ptr %2, !tbaa !2
  %1695 = icmp sge i16 %1694, 0
  br i1 %1695, label %b272, label %b273

b272:
  %1696 = load i16, ptr %134, !tbaa !2
  %1697 = load i16, ptr %3, !tbaa !2
  %1698 = icmp sle i16 %1696, %1697
  br i1 %1698, label %b274, label %b275

b273:
  %1699 = load i16, ptr %134, !tbaa !2
  %1700 = load i16, ptr %3, !tbaa !2
  %1701 = icmp sge i16 %1699, %1700
  br i1 %1701, label %b274, label %b275

b274:
  %1702 = load i16, ptr %26, !tbaa !2
  %1703 = mul i16 %1702, 2
  %1704 = getelementptr i8, ptr %143, i16 2
  %1705 = load i16, ptr %1704, !tbaa !2
  %1706 = add i16 0, %1703
  %1707 = inttoptr i16 %1705 to ptr addrspace(2)
  %1708 = addrspacecast ptr addrspace(2) %1707 to ptr addrspace(1)
  %1709 = getelementptr i8, ptr addrspace(1) %1708, i16 %1706
  %1710 = load i16, ptr addrspace(1) %1709, !tbaa !46
  %1711 = load i16, ptr %28, !tbaa !2
  %1712 = add i16 %1710, %1711
  %1713 = sub i16 %1712, -640
  %1714 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1713
  %1715 = load i16, ptr %1714, !tbaa !2
  %1716 = load i16, ptr %26, !tbaa !2
  %1717 = load i16, ptr %27, !tbaa !2
  %1718 = sub i16 %1716, %1717
  %1719 = sub i16 %1718, -640
  %1720 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1719
  %1721 = load i16, ptr %1720, !tbaa !2
  %1722 = add i16 %1715, %1721
  %1723 = load i16, ptr %134, !tbaa !2
  %1724 = add i16 %1722, %1723
  %1725 = load i16, ptr %134, !tbaa !2
  %1726 = mul i16 %1725, 2
  %1727 = getelementptr i8, ptr %138, i16 2
  %1728 = load i16, ptr %1727, !tbaa !2
  %1729 = add i16 0, %1726
  %1730 = inttoptr i16 %1728 to ptr addrspace(2)
  %1731 = addrspacecast ptr addrspace(2) %1730 to ptr addrspace(1)
  %1732 = getelementptr i8, ptr addrspace(1) %1731, i16 %1729
  %1733 = load i16, ptr addrspace(1) %1732, !tbaa !40
  %1734 = trunc i16 %1733 to i8
  %1735 = load i16, ptr @b$seg, !tbaa !2
  %1736 = inttoptr i16 %1735 to ptr addrspace(2)
  %1737 = addrspacecast ptr addrspace(2) %1736 to ptr addrspace(1)
  %1738 = getelementptr i8, ptr addrspace(1) %1737, i16 %1724
  store volatile i8 %1734, ptr addrspace(1) %1738, !tbaa !4
  %1739 = load i16, ptr %26, !tbaa !2
  %1740 = load i16, ptr %28, !tbaa !2
  %1741 = add i16 %1739, %1740
  %1742 = sub i16 %1741, -640
  %1743 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1742
  %1744 = load i16, ptr %1743, !tbaa !2
  %1745 = load i16, ptr %26, !tbaa !2
  %1746 = mul i16 %1745, 2
  %1747 = getelementptr i8, ptr %143, i16 2
  %1748 = load i16, ptr %1747, !tbaa !2
  %1749 = add i16 0, %1746
  %1750 = inttoptr i16 %1748 to ptr addrspace(2)
  %1751 = addrspacecast ptr addrspace(2) %1750 to ptr addrspace(1)
  %1752 = getelementptr i8, ptr addrspace(1) %1751, i16 %1749
  %1753 = load i16, ptr addrspace(1) %1752, !tbaa !46
  %1754 = load i16, ptr %135, !tbaa !2
  %1755 = add i16 %1753, %1754
  %1756 = sub i16 %1755, -48
  %1757 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %1756
  %1758 = load i16, ptr %1757, !tbaa !2
  %1759 = add i16 %1744, %1758
  %1760 = load i16, ptr %134, !tbaa !2
  %1761 = add i16 %1759, %1760
  %1762 = load i16, ptr %134, !tbaa !2
  %1763 = mul i16 %1762, 2
  %1764 = getelementptr i8, ptr %137, i16 2
  %1765 = load i16, ptr %1764, !tbaa !2
  %1766 = add i16 0, %1763
  %1767 = inttoptr i16 %1765 to ptr addrspace(2)
  %1768 = addrspacecast ptr addrspace(2) %1767 to ptr addrspace(1)
  %1769 = getelementptr i8, ptr addrspace(1) %1768, i16 %1766
  %1770 = load i16, ptr addrspace(1) %1769, !tbaa !42
  %1771 = trunc i16 %1770 to i8
  %1772 = load i16, ptr @b$seg, !tbaa !2
  %1773 = inttoptr i16 %1772 to ptr addrspace(2)
  %1774 = addrspacecast ptr addrspace(2) %1773 to ptr addrspace(1)
  %1775 = getelementptr i8, ptr addrspace(1) %1774, i16 %1761
  store volatile i8 %1771, ptr addrspace(1) %1775, !tbaa !4
  %1776 = load i16, ptr %26, !tbaa !2
  %1777 = load i16, ptr %135, !tbaa !2
  %1778 = add i16 %1776, %1777
  %1779 = sub i16 %1778, -48
  %1780 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %1779
  %1781 = load i16, ptr %1780, !tbaa !2
  %1782 = load i16, ptr %26, !tbaa !2
  %1783 = load i16, ptr %28, !tbaa !2
  %1784 = add i16 %1782, %1783
  %1785 = sub i16 %1784, -640
  %1786 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1785
  %1787 = load i16, ptr %1786, !tbaa !2
  %1788 = add i16 %1781, %1787
  %1789 = load i16, ptr %26, !tbaa !2
  %1790 = load i16, ptr %27, !tbaa !2
  %1791 = add i16 %1789, %1790
  %1792 = sub i16 %1791, -640
  %1793 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1792
  %1794 = load i16, ptr %1793, !tbaa !2
  %1795 = add i16 %1788, %1794
  %1796 = load i16, ptr %134, !tbaa !2
  %1797 = add i16 %1795, %1796
  %1798 = sub i16 %1797, 99
  %1799 = load i16, ptr %134, !tbaa !2
  %1800 = mul i16 %1799, 2
  %1801 = getelementptr i8, ptr %136, i16 2
  %1802 = load i16, ptr %1801, !tbaa !2
  %1803 = add i16 0, %1800
  %1804 = inttoptr i16 %1802 to ptr addrspace(2)
  %1805 = addrspacecast ptr addrspace(2) %1804 to ptr addrspace(1)
  %1806 = getelementptr i8, ptr addrspace(1) %1805, i16 %1803
  %1807 = load i16, ptr addrspace(1) %1806, !tbaa !44
  %1808 = trunc i16 %1807 to i8
  %1809 = load i16, ptr @b$seg, !tbaa !2
  %1810 = inttoptr i16 %1809 to ptr addrspace(2)
  %1811 = addrspacecast ptr addrspace(2) %1810 to ptr addrspace(1)
  %1812 = getelementptr i8, ptr addrspace(1) %1811, i16 %1798
  store volatile i8 %1808, ptr addrspace(1) %1812, !tbaa !4
  %1813 = load i16, ptr %134, !tbaa !2
  %1814 = load i16, ptr %2, !tbaa !2
  %1815 = add nsw i16 %1813, %1814
  store i16 %1815, ptr %134, !tbaa !2
  br label %b271

b275:
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  %1816 = load i16, ptr %26, !tbaa !2
  %1817 = mul i16 %1816, 2
  %1818 = getelementptr i8, ptr %142, i16 2
  %1819 = load i16, ptr %1818, !tbaa !2
  %1820 = add i16 0, %1817
  %1821 = inttoptr i16 %1819 to ptr addrspace(2)
  %1822 = addrspacecast ptr addrspace(2) %1821 to ptr addrspace(1)
  %1823 = getelementptr i8, ptr addrspace(1) %1822, i16 %1820
  %1824 = load i16, ptr addrspace(1) %1823, !tbaa !48
  %1825 = trunc i16 %1824 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1825)
  %1826 = load i16, ptr %26, !tbaa !2
  %1827 = mul i16 %1826, 2
  %1828 = getelementptr i8, ptr %142, i16 2
  %1829 = load i16, ptr %1828, !tbaa !2
  %1830 = add i16 0, %1827
  %1831 = inttoptr i16 %1829 to ptr addrspace(2)
  %1832 = addrspacecast ptr addrspace(2) %1831 to ptr addrspace(1)
  %1833 = getelementptr i8, ptr addrspace(1) %1832, i16 %1830
  %1834 = load i16, ptr addrspace(1) %1833, !tbaa !48
  %1835 = sub i16 63, %1834
  %1836 = trunc i16 %1835 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1836)
  %1837 = load i16, ptr %26, !tbaa !2
  %1838 = mul i16 %1837, 2
  %1839 = getelementptr i8, ptr %142, i16 2
  %1840 = load i16, ptr %1839, !tbaa !2
  %1841 = add i16 0, %1838
  %1842 = inttoptr i16 %1840 to ptr addrspace(2)
  %1843 = addrspacecast ptr addrspace(2) %1842 to ptr addrspace(1)
  %1844 = getelementptr i8, ptr addrspace(1) %1843, i16 %1841
  %1845 = load i16, ptr addrspace(1) %1844, !tbaa !48
  %1846 = sub i16 63, %1845
  %1847 = trunc i16 %1846 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1847)
  %1848 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1848, ptr @"BENCHFRAME&", !tbaa !2
  %1849 = load i16, ptr %26, !tbaa !2
  %1850 = load i16, ptr %4, !tbaa !2
  %1851 = add nsw i16 %1849, %1850
  store i16 %1851, ptr %26, !tbaa !2
  br label %b266

b276:
  %1852 = load i16, ptr %0, !tbaa !2
  %1853 = icmp sge i16 %1852, 0
  br i1 %1853, label %b277, label %b278

b277:
  %1854 = load i16, ptr %134, !tbaa !2
  %1855 = load i16, ptr %1, !tbaa !2
  %1856 = icmp sle i16 %1854, %1855
  br i1 %1856, label %b279, label %b280

b278:
  %1857 = load i16, ptr %134, !tbaa !2
  %1858 = load i16, ptr %1, !tbaa !2
  %1859 = icmp sge i16 %1857, %1858
  br i1 %1859, label %b279, label %b280

b279:
  %1860 = load i16, ptr %134, !tbaa !2
  %1861 = load i16, ptr @b$seg, !tbaa !2
  %1862 = inttoptr i16 %1861 to ptr addrspace(2)
  %1863 = addrspacecast ptr addrspace(2) %1862 to ptr addrspace(1)
  %1864 = getelementptr i8, ptr addrspace(1) %1863, i16 %1860
  store volatile i8 0, ptr addrspace(1) %1864, !tbaa !4
  %1865 = load i16, ptr %134, !tbaa !2
  %1866 = load i16, ptr %0, !tbaa !2
  %1867 = add nsw i16 %1865, %1866
  store i16 %1867, ptr %134, !tbaa !2
  br label %b276

b280:
  br label %b244
}

define cc 1000 void @CREATEOBJECT(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca float
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca float
  %23 = alloca float
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca float
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca float
  %43 = alloca float
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca float
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca i16
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca i16
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca float
  %79 = alloca float
  %80 = alloca i16
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca float
  %84 = alloca i16
  %85 = alloca i16
  %86 = alloca i16
  %87 = alloca float
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca i16
  %91 = alloca i16
  %92 = alloca i16
  %93 = alloca i16
  %94 = alloca i16
  %95 = alloca i16
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  %103 = alloca i16
  %104 = alloca i16
  %105 = alloca i16
  %106 = alloca i16
  %107 = alloca i16
  %108 = alloca i16
  %109 = alloca i16
  %110 = alloca i16
  %111 = alloca float
  %112 = load double, ptr @$float215, !tbaa !2
  %113 = fptrunc double %112 to float
  %114 = fpround float %113 to float
  store float %114, ptr %111, !tbaa !2
  store i16 0, ptr %110, !tbaa !2
  call cc 1000 addrspace(1) void @CLEARSCREEN(ptr %110)
  %115 = load i16, ptr %0
  %116 = icmp eq i16 %115, 1
  br i1 %116, label %b3, label %b4

b2:
  ret void

b3:
  store i16 0, ptr %109, !tbaa !2
  store i16 4, ptr %108, !tbaa !2
  store i16 1, ptr %107, !tbaa !2
  br label %b5

b4:
  %117 = icmp eq i16 %115, 2
  br i1 %117, label %b20, label %b21

b5:
  %118 = load i16, ptr %107, !tbaa !2
  %119 = icmp sge i16 %118, 0
  br i1 %119, label %b6, label %b7

b6:
  %120 = load i16, ptr %109, !tbaa !2
  %121 = load i16, ptr %108, !tbaa !2
  %122 = icmp sle i16 %120, %121
  br i1 %122, label %b8, label %b9

b7:
  %123 = load i16, ptr %109, !tbaa !2
  %124 = load i16, ptr %108, !tbaa !2
  %125 = icmp sge i16 %123, %124
  br i1 %125, label %b8, label %b9

b8:
  %126 = load i16, ptr %109, !tbaa !2
  %127 = mul i16 %126, 2
  %128 = getelementptr i8, ptr @"XO%", i16 2
  %129 = load i16, ptr %128, !tbaa !2
  %130 = add i16 0, %127
  %131 = inttoptr i16 %129 to ptr addrspace(2)
  %132 = addrspacecast ptr addrspace(2) %131 to ptr addrspace(1)
  %133 = getelementptr i8, ptr addrspace(1) %132, i16 %130
  %134 = load i16, ptr %109, !tbaa !2
  %135 = mul i16 2, %134
  %136 = sub i16 %135, 5
  store i16 %136, ptr addrspace(1) %133, !tbaa !16
  %137 = load i16, ptr %109, !tbaa !2
  %138 = mul i16 %137, 2
  %139 = getelementptr i8, ptr @"YO%", i16 2
  %140 = load i16, ptr %139, !tbaa !2
  %141 = add i16 0, %138
  %142 = inttoptr i16 %140 to ptr addrspace(2)
  %143 = addrspacecast ptr addrspace(2) %142 to ptr addrspace(1)
  %144 = getelementptr i8, ptr addrspace(1) %143, i16 %141
  store i16 5, ptr addrspace(1) %144, !tbaa !20
  %145 = load i16, ptr %109, !tbaa !2
  %146 = mul i16 %145, 2
  %147 = getelementptr i8, ptr @"ZO%", i16 2
  %148 = load i16, ptr %147, !tbaa !2
  %149 = add i16 0, %146
  %150 = inttoptr i16 %148 to ptr addrspace(2)
  %151 = addrspacecast ptr addrspace(2) %150 to ptr addrspace(1)
  %152 = getelementptr i8, ptr addrspace(1) %151, i16 %149
  store i16 5, ptr addrspace(1) %152, !tbaa !18
  %153 = load i16, ptr %109, !tbaa !2
  %154 = add i16 %153, 5
  %155 = mul i16 %154, 2
  %156 = getelementptr i8, ptr @"XO%", i16 2
  %157 = load i16, ptr %156, !tbaa !2
  %158 = add i16 0, %155
  %159 = inttoptr i16 %157 to ptr addrspace(2)
  %160 = addrspacecast ptr addrspace(2) %159 to ptr addrspace(1)
  %161 = getelementptr i8, ptr addrspace(1) %160, i16 %158
  %162 = load i16, ptr %109, !tbaa !2
  %163 = mul i16 2, %162
  %164 = sub i16 %163, 5
  store i16 %164, ptr addrspace(1) %161, !tbaa !16
  %165 = load i16, ptr %109, !tbaa !2
  %166 = add i16 %165, 5
  %167 = mul i16 %166, 2
  %168 = getelementptr i8, ptr @"YO%", i16 2
  %169 = load i16, ptr %168, !tbaa !2
  %170 = add i16 0, %167
  %171 = inttoptr i16 %169 to ptr addrspace(2)
  %172 = addrspacecast ptr addrspace(2) %171 to ptr addrspace(1)
  %173 = getelementptr i8, ptr addrspace(1) %172, i16 %170
  store i16 5, ptr addrspace(1) %173, !tbaa !20
  %174 = load i16, ptr %109, !tbaa !2
  %175 = add i16 %174, 5
  %176 = mul i16 %175, 2
  %177 = getelementptr i8, ptr @"ZO%", i16 2
  %178 = load i16, ptr %177, !tbaa !2
  %179 = add i16 0, %176
  %180 = inttoptr i16 %178 to ptr addrspace(2)
  %181 = addrspacecast ptr addrspace(2) %180 to ptr addrspace(1)
  %182 = getelementptr i8, ptr addrspace(1) %181, i16 %179
  store i16 -5, ptr addrspace(1) %182, !tbaa !18
  %183 = load i16, ptr %109, !tbaa !2
  %184 = add i16 %183, 10
  %185 = mul i16 %184, 2
  %186 = getelementptr i8, ptr @"XO%", i16 2
  %187 = load i16, ptr %186, !tbaa !2
  %188 = add i16 0, %185
  %189 = inttoptr i16 %187 to ptr addrspace(2)
  %190 = addrspacecast ptr addrspace(2) %189 to ptr addrspace(1)
  %191 = getelementptr i8, ptr addrspace(1) %190, i16 %188
  %192 = load i16, ptr %109, !tbaa !2
  %193 = mul i16 2, %192
  %194 = sub i16 %193, 5
  store i16 %194, ptr addrspace(1) %191, !tbaa !16
  %195 = load i16, ptr %109, !tbaa !2
  %196 = add i16 %195, 10
  %197 = mul i16 %196, 2
  %198 = getelementptr i8, ptr @"YO%", i16 2
  %199 = load i16, ptr %198, !tbaa !2
  %200 = add i16 0, %197
  %201 = inttoptr i16 %199 to ptr addrspace(2)
  %202 = addrspacecast ptr addrspace(2) %201 to ptr addrspace(1)
  %203 = getelementptr i8, ptr addrspace(1) %202, i16 %200
  store i16 -5, ptr addrspace(1) %203, !tbaa !20
  %204 = load i16, ptr %109, !tbaa !2
  %205 = add i16 %204, 10
  %206 = mul i16 %205, 2
  %207 = getelementptr i8, ptr @"ZO%", i16 2
  %208 = load i16, ptr %207, !tbaa !2
  %209 = add i16 0, %206
  %210 = inttoptr i16 %208 to ptr addrspace(2)
  %211 = addrspacecast ptr addrspace(2) %210 to ptr addrspace(1)
  %212 = getelementptr i8, ptr addrspace(1) %211, i16 %209
  store i16 5, ptr addrspace(1) %212, !tbaa !18
  %213 = load i16, ptr %109, !tbaa !2
  %214 = add i16 %213, 15
  %215 = mul i16 %214, 2
  %216 = getelementptr i8, ptr @"XO%", i16 2
  %217 = load i16, ptr %216, !tbaa !2
  %218 = add i16 0, %215
  %219 = inttoptr i16 %217 to ptr addrspace(2)
  %220 = addrspacecast ptr addrspace(2) %219 to ptr addrspace(1)
  %221 = getelementptr i8, ptr addrspace(1) %220, i16 %218
  %222 = load i16, ptr %109, !tbaa !2
  %223 = mul i16 2, %222
  %224 = sub i16 %223, 5
  store i16 %224, ptr addrspace(1) %221, !tbaa !16
  %225 = load i16, ptr %109, !tbaa !2
  %226 = add i16 %225, 15
  %227 = mul i16 %226, 2
  %228 = getelementptr i8, ptr @"YO%", i16 2
  %229 = load i16, ptr %228, !tbaa !2
  %230 = add i16 0, %227
  %231 = inttoptr i16 %229 to ptr addrspace(2)
  %232 = addrspacecast ptr addrspace(2) %231 to ptr addrspace(1)
  %233 = getelementptr i8, ptr addrspace(1) %232, i16 %230
  store i16 -5, ptr addrspace(1) %233, !tbaa !20
  %234 = load i16, ptr %109, !tbaa !2
  %235 = add i16 %234, 15
  %236 = mul i16 %235, 2
  %237 = getelementptr i8, ptr @"ZO%", i16 2
  %238 = load i16, ptr %237, !tbaa !2
  %239 = add i16 0, %236
  %240 = inttoptr i16 %238 to ptr addrspace(2)
  %241 = addrspacecast ptr addrspace(2) %240 to ptr addrspace(1)
  %242 = getelementptr i8, ptr addrspace(1) %241, i16 %239
  store i16 -5, ptr addrspace(1) %242, !tbaa !18
  %243 = load i16, ptr %109, !tbaa !2
  %244 = add i16 %243, 20
  %245 = mul i16 %244, 2
  %246 = getelementptr i8, ptr @"XO%", i16 2
  %247 = load i16, ptr %246, !tbaa !2
  %248 = add i16 0, %245
  %249 = inttoptr i16 %247 to ptr addrspace(2)
  %250 = addrspacecast ptr addrspace(2) %249 to ptr addrspace(1)
  %251 = getelementptr i8, ptr addrspace(1) %250, i16 %248
  store i16 -5, ptr addrspace(1) %251, !tbaa !16
  %252 = load i16, ptr %109, !tbaa !2
  %253 = add i16 %252, 20
  %254 = mul i16 %253, 2
  %255 = getelementptr i8, ptr @"YO%", i16 2
  %256 = load i16, ptr %255, !tbaa !2
  %257 = add i16 0, %254
  %258 = inttoptr i16 %256 to ptr addrspace(2)
  %259 = addrspacecast ptr addrspace(2) %258 to ptr addrspace(1)
  %260 = getelementptr i8, ptr addrspace(1) %259, i16 %257
  store i16 5, ptr addrspace(1) %260, !tbaa !20
  %261 = load i16, ptr %109, !tbaa !2
  %262 = add i16 %261, 20
  %263 = mul i16 %262, 2
  %264 = getelementptr i8, ptr @"ZO%", i16 2
  %265 = load i16, ptr %264, !tbaa !2
  %266 = add i16 0, %263
  %267 = inttoptr i16 %265 to ptr addrspace(2)
  %268 = addrspacecast ptr addrspace(2) %267 to ptr addrspace(1)
  %269 = getelementptr i8, ptr addrspace(1) %268, i16 %266
  %270 = load i16, ptr %109, !tbaa !2
  %271 = mul i16 2, %270
  %272 = sub i16 %271, 5
  store i16 %272, ptr addrspace(1) %269, !tbaa !18
  %273 = load i16, ptr %109, !tbaa !2
  %274 = add i16 %273, 25
  %275 = mul i16 %274, 2
  %276 = getelementptr i8, ptr @"XO%", i16 2
  %277 = load i16, ptr %276, !tbaa !2
  %278 = add i16 0, %275
  %279 = inttoptr i16 %277 to ptr addrspace(2)
  %280 = addrspacecast ptr addrspace(2) %279 to ptr addrspace(1)
  %281 = getelementptr i8, ptr addrspace(1) %280, i16 %278
  store i16 5, ptr addrspace(1) %281, !tbaa !16
  %282 = load i16, ptr %109, !tbaa !2
  %283 = add i16 %282, 25
  %284 = mul i16 %283, 2
  %285 = getelementptr i8, ptr @"YO%", i16 2
  %286 = load i16, ptr %285, !tbaa !2
  %287 = add i16 0, %284
  %288 = inttoptr i16 %286 to ptr addrspace(2)
  %289 = addrspacecast ptr addrspace(2) %288 to ptr addrspace(1)
  %290 = getelementptr i8, ptr addrspace(1) %289, i16 %287
  store i16 5, ptr addrspace(1) %290, !tbaa !20
  %291 = load i16, ptr %109, !tbaa !2
  %292 = add i16 %291, 25
  %293 = mul i16 %292, 2
  %294 = getelementptr i8, ptr @"ZO%", i16 2
  %295 = load i16, ptr %294, !tbaa !2
  %296 = add i16 0, %293
  %297 = inttoptr i16 %295 to ptr addrspace(2)
  %298 = addrspacecast ptr addrspace(2) %297 to ptr addrspace(1)
  %299 = getelementptr i8, ptr addrspace(1) %298, i16 %296
  %300 = load i16, ptr %109, !tbaa !2
  %301 = mul i16 2, %300
  %302 = sub i16 %301, 5
  store i16 %302, ptr addrspace(1) %299, !tbaa !18
  %303 = load i16, ptr %109, !tbaa !2
  %304 = add i16 %303, 30
  %305 = mul i16 %304, 2
  %306 = getelementptr i8, ptr @"XO%", i16 2
  %307 = load i16, ptr %306, !tbaa !2
  %308 = add i16 0, %305
  %309 = inttoptr i16 %307 to ptr addrspace(2)
  %310 = addrspacecast ptr addrspace(2) %309 to ptr addrspace(1)
  %311 = getelementptr i8, ptr addrspace(1) %310, i16 %308
  store i16 -5, ptr addrspace(1) %311, !tbaa !16
  %312 = load i16, ptr %109, !tbaa !2
  %313 = add i16 %312, 30
  %314 = mul i16 %313, 2
  %315 = getelementptr i8, ptr @"YO%", i16 2
  %316 = load i16, ptr %315, !tbaa !2
  %317 = add i16 0, %314
  %318 = inttoptr i16 %316 to ptr addrspace(2)
  %319 = addrspacecast ptr addrspace(2) %318 to ptr addrspace(1)
  %320 = getelementptr i8, ptr addrspace(1) %319, i16 %317
  store i16 -5, ptr addrspace(1) %320, !tbaa !20
  %321 = load i16, ptr %109, !tbaa !2
  %322 = add i16 %321, 30
  %323 = mul i16 %322, 2
  %324 = getelementptr i8, ptr @"ZO%", i16 2
  %325 = load i16, ptr %324, !tbaa !2
  %326 = add i16 0, %323
  %327 = inttoptr i16 %325 to ptr addrspace(2)
  %328 = addrspacecast ptr addrspace(2) %327 to ptr addrspace(1)
  %329 = getelementptr i8, ptr addrspace(1) %328, i16 %326
  %330 = load i16, ptr %109, !tbaa !2
  %331 = mul i16 2, %330
  %332 = sub i16 %331, 5
  store i16 %332, ptr addrspace(1) %329, !tbaa !18
  %333 = load i16, ptr %109, !tbaa !2
  %334 = add i16 %333, 35
  %335 = mul i16 %334, 2
  %336 = getelementptr i8, ptr @"XO%", i16 2
  %337 = load i16, ptr %336, !tbaa !2
  %338 = add i16 0, %335
  %339 = inttoptr i16 %337 to ptr addrspace(2)
  %340 = addrspacecast ptr addrspace(2) %339 to ptr addrspace(1)
  %341 = getelementptr i8, ptr addrspace(1) %340, i16 %338
  store i16 5, ptr addrspace(1) %341, !tbaa !16
  %342 = load i16, ptr %109, !tbaa !2
  %343 = add i16 %342, 35
  %344 = mul i16 %343, 2
  %345 = getelementptr i8, ptr @"YO%", i16 2
  %346 = load i16, ptr %345, !tbaa !2
  %347 = add i16 0, %344
  %348 = inttoptr i16 %346 to ptr addrspace(2)
  %349 = addrspacecast ptr addrspace(2) %348 to ptr addrspace(1)
  %350 = getelementptr i8, ptr addrspace(1) %349, i16 %347
  store i16 -5, ptr addrspace(1) %350, !tbaa !20
  %351 = load i16, ptr %109, !tbaa !2
  %352 = add i16 %351, 35
  %353 = mul i16 %352, 2
  %354 = getelementptr i8, ptr @"ZO%", i16 2
  %355 = load i16, ptr %354, !tbaa !2
  %356 = add i16 0, %353
  %357 = inttoptr i16 %355 to ptr addrspace(2)
  %358 = addrspacecast ptr addrspace(2) %357 to ptr addrspace(1)
  %359 = getelementptr i8, ptr addrspace(1) %358, i16 %356
  %360 = load i16, ptr %109, !tbaa !2
  %361 = mul i16 2, %360
  %362 = sub i16 %361, 5
  store i16 %362, ptr addrspace(1) %359, !tbaa !18
  %363 = load i16, ptr %109, !tbaa !2
  %364 = add i16 %363, 40
  %365 = mul i16 %364, 2
  %366 = getelementptr i8, ptr @"XO%", i16 2
  %367 = load i16, ptr %366, !tbaa !2
  %368 = add i16 0, %365
  %369 = inttoptr i16 %367 to ptr addrspace(2)
  %370 = addrspacecast ptr addrspace(2) %369 to ptr addrspace(1)
  %371 = getelementptr i8, ptr addrspace(1) %370, i16 %368
  store i16 5, ptr addrspace(1) %371, !tbaa !16
  %372 = load i16, ptr %109, !tbaa !2
  %373 = add i16 %372, 40
  %374 = mul i16 %373, 2
  %375 = getelementptr i8, ptr @"YO%", i16 2
  %376 = load i16, ptr %375, !tbaa !2
  %377 = add i16 0, %374
  %378 = inttoptr i16 %376 to ptr addrspace(2)
  %379 = addrspacecast ptr addrspace(2) %378 to ptr addrspace(1)
  %380 = getelementptr i8, ptr addrspace(1) %379, i16 %377
  %381 = load i16, ptr %109, !tbaa !2
  %382 = mul i16 2, %381
  %383 = sub i16 %382, 5
  store i16 %383, ptr addrspace(1) %380, !tbaa !20
  %384 = load i16, ptr %109, !tbaa !2
  %385 = add i16 %384, 40
  %386 = mul i16 %385, 2
  %387 = getelementptr i8, ptr @"ZO%", i16 2
  %388 = load i16, ptr %387, !tbaa !2
  %389 = add i16 0, %386
  %390 = inttoptr i16 %388 to ptr addrspace(2)
  %391 = addrspacecast ptr addrspace(2) %390 to ptr addrspace(1)
  %392 = getelementptr i8, ptr addrspace(1) %391, i16 %389
  store i16 5, ptr addrspace(1) %392, !tbaa !18
  %393 = load i16, ptr %109, !tbaa !2
  %394 = add i16 %393, 45
  %395 = mul i16 %394, 2
  %396 = getelementptr i8, ptr @"XO%", i16 2
  %397 = load i16, ptr %396, !tbaa !2
  %398 = add i16 0, %395
  %399 = inttoptr i16 %397 to ptr addrspace(2)
  %400 = addrspacecast ptr addrspace(2) %399 to ptr addrspace(1)
  %401 = getelementptr i8, ptr addrspace(1) %400, i16 %398
  store i16 5, ptr addrspace(1) %401, !tbaa !16
  %402 = load i16, ptr %109, !tbaa !2
  %403 = add i16 %402, 45
  %404 = mul i16 %403, 2
  %405 = getelementptr i8, ptr @"YO%", i16 2
  %406 = load i16, ptr %405, !tbaa !2
  %407 = add i16 0, %404
  %408 = inttoptr i16 %406 to ptr addrspace(2)
  %409 = addrspacecast ptr addrspace(2) %408 to ptr addrspace(1)
  %410 = getelementptr i8, ptr addrspace(1) %409, i16 %407
  %411 = load i16, ptr %109, !tbaa !2
  %412 = mul i16 2, %411
  %413 = sub i16 %412, 5
  store i16 %413, ptr addrspace(1) %410, !tbaa !20
  %414 = load i16, ptr %109, !tbaa !2
  %415 = add i16 %414, 45
  %416 = mul i16 %415, 2
  %417 = getelementptr i8, ptr @"ZO%", i16 2
  %418 = load i16, ptr %417, !tbaa !2
  %419 = add i16 0, %416
  %420 = inttoptr i16 %418 to ptr addrspace(2)
  %421 = addrspacecast ptr addrspace(2) %420 to ptr addrspace(1)
  %422 = getelementptr i8, ptr addrspace(1) %421, i16 %419
  store i16 -5, ptr addrspace(1) %422, !tbaa !18
  %423 = load i16, ptr %109, !tbaa !2
  %424 = add i16 %423, 50
  %425 = mul i16 %424, 2
  %426 = getelementptr i8, ptr @"XO%", i16 2
  %427 = load i16, ptr %426, !tbaa !2
  %428 = add i16 0, %425
  %429 = inttoptr i16 %427 to ptr addrspace(2)
  %430 = addrspacecast ptr addrspace(2) %429 to ptr addrspace(1)
  %431 = getelementptr i8, ptr addrspace(1) %430, i16 %428
  store i16 -5, ptr addrspace(1) %431, !tbaa !16
  %432 = load i16, ptr %109, !tbaa !2
  %433 = add i16 %432, 50
  %434 = mul i16 %433, 2
  %435 = getelementptr i8, ptr @"YO%", i16 2
  %436 = load i16, ptr %435, !tbaa !2
  %437 = add i16 0, %434
  %438 = inttoptr i16 %436 to ptr addrspace(2)
  %439 = addrspacecast ptr addrspace(2) %438 to ptr addrspace(1)
  %440 = getelementptr i8, ptr addrspace(1) %439, i16 %437
  %441 = load i16, ptr %109, !tbaa !2
  %442 = mul i16 2, %441
  %443 = sub i16 %442, 5
  store i16 %443, ptr addrspace(1) %440, !tbaa !20
  %444 = load i16, ptr %109, !tbaa !2
  %445 = add i16 %444, 50
  %446 = mul i16 %445, 2
  %447 = getelementptr i8, ptr @"ZO%", i16 2
  %448 = load i16, ptr %447, !tbaa !2
  %449 = add i16 0, %446
  %450 = inttoptr i16 %448 to ptr addrspace(2)
  %451 = addrspacecast ptr addrspace(2) %450 to ptr addrspace(1)
  %452 = getelementptr i8, ptr addrspace(1) %451, i16 %449
  store i16 5, ptr addrspace(1) %452, !tbaa !18
  %453 = load i16, ptr %109, !tbaa !2
  %454 = add i16 %453, 55
  %455 = mul i16 %454, 2
  %456 = getelementptr i8, ptr @"XO%", i16 2
  %457 = load i16, ptr %456, !tbaa !2
  %458 = add i16 0, %455
  %459 = inttoptr i16 %457 to ptr addrspace(2)
  %460 = addrspacecast ptr addrspace(2) %459 to ptr addrspace(1)
  %461 = getelementptr i8, ptr addrspace(1) %460, i16 %458
  store i16 -5, ptr addrspace(1) %461, !tbaa !16
  %462 = load i16, ptr %109, !tbaa !2
  %463 = add i16 %462, 55
  %464 = mul i16 %463, 2
  %465 = getelementptr i8, ptr @"YO%", i16 2
  %466 = load i16, ptr %465, !tbaa !2
  %467 = add i16 0, %464
  %468 = inttoptr i16 %466 to ptr addrspace(2)
  %469 = addrspacecast ptr addrspace(2) %468 to ptr addrspace(1)
  %470 = getelementptr i8, ptr addrspace(1) %469, i16 %467
  %471 = load i16, ptr %109, !tbaa !2
  %472 = mul i16 2, %471
  %473 = sub i16 %472, 5
  store i16 %473, ptr addrspace(1) %470, !tbaa !20
  %474 = load i16, ptr %109, !tbaa !2
  %475 = add i16 %474, 55
  %476 = mul i16 %475, 2
  %477 = getelementptr i8, ptr @"ZO%", i16 2
  %478 = load i16, ptr %477, !tbaa !2
  %479 = add i16 0, %476
  %480 = inttoptr i16 %478 to ptr addrspace(2)
  %481 = addrspacecast ptr addrspace(2) %480 to ptr addrspace(1)
  %482 = getelementptr i8, ptr addrspace(1) %481, i16 %479
  store i16 -5, ptr addrspace(1) %482, !tbaa !18
  %483 = load i16, ptr %109, !tbaa !2
  %484 = load i16, ptr %107, !tbaa !2
  %485 = add nsw i16 %483, %484
  store i16 %485, ptr %109, !tbaa !2
  br label %b5

b9:
  %486 = getelementptr i8, ptr @"XO%", i16 2
  %487 = load i16, ptr %486, !tbaa !2
  %488 = inttoptr i16 %487 to ptr addrspace(2)
  %489 = addrspacecast ptr addrspace(2) %488 to ptr addrspace(1)
  %490 = getelementptr i8, ptr addrspace(1) %489, i16 120
  store i16 5, ptr addrspace(1) %490, !tbaa !16
  %491 = getelementptr i8, ptr @"YO%", i16 2
  %492 = load i16, ptr %491, !tbaa !2
  %493 = inttoptr i16 %492 to ptr addrspace(2)
  %494 = addrspacecast ptr addrspace(2) %493 to ptr addrspace(1)
  %495 = getelementptr i8, ptr addrspace(1) %494, i16 120
  store i16 5, ptr addrspace(1) %495, !tbaa !20
  %496 = getelementptr i8, ptr @"ZO%", i16 2
  %497 = load i16, ptr %496, !tbaa !2
  %498 = inttoptr i16 %497 to ptr addrspace(2)
  %499 = addrspacecast ptr addrspace(2) %498 to ptr addrspace(1)
  %500 = getelementptr i8, ptr addrspace(1) %499, i16 120
  store i16 5, ptr addrspace(1) %500, !tbaa !18
  %501 = getelementptr i8, ptr @"XO%", i16 2
  %502 = load i16, ptr %501, !tbaa !2
  %503 = inttoptr i16 %502 to ptr addrspace(2)
  %504 = addrspacecast ptr addrspace(2) %503 to ptr addrspace(1)
  %505 = getelementptr i8, ptr addrspace(1) %504, i16 122
  store i16 5, ptr addrspace(1) %505, !tbaa !16
  %506 = getelementptr i8, ptr @"YO%", i16 2
  %507 = load i16, ptr %506, !tbaa !2
  %508 = inttoptr i16 %507 to ptr addrspace(2)
  %509 = addrspacecast ptr addrspace(2) %508 to ptr addrspace(1)
  %510 = getelementptr i8, ptr addrspace(1) %509, i16 122
  store i16 5, ptr addrspace(1) %510, !tbaa !20
  %511 = getelementptr i8, ptr @"ZO%", i16 2
  %512 = load i16, ptr %511, !tbaa !2
  %513 = inttoptr i16 %512 to ptr addrspace(2)
  %514 = addrspacecast ptr addrspace(2) %513 to ptr addrspace(1)
  %515 = getelementptr i8, ptr addrspace(1) %514, i16 122
  store i16 5, ptr addrspace(1) %515, !tbaa !18
  %516 = getelementptr i8, ptr @"XO%", i16 2
  %517 = load i16, ptr %516, !tbaa !2
  %518 = inttoptr i16 %517 to ptr addrspace(2)
  %519 = addrspacecast ptr addrspace(2) %518 to ptr addrspace(1)
  %520 = getelementptr i8, ptr addrspace(1) %519, i16 124
  store i16 5, ptr addrspace(1) %520, !tbaa !16
  %521 = getelementptr i8, ptr @"YO%", i16 2
  %522 = load i16, ptr %521, !tbaa !2
  %523 = inttoptr i16 %522 to ptr addrspace(2)
  %524 = addrspacecast ptr addrspace(2) %523 to ptr addrspace(1)
  %525 = getelementptr i8, ptr addrspace(1) %524, i16 124
  store i16 5, ptr addrspace(1) %525, !tbaa !20
  %526 = getelementptr i8, ptr @"ZO%", i16 2
  %527 = load i16, ptr %526, !tbaa !2
  %528 = inttoptr i16 %527 to ptr addrspace(2)
  %529 = addrspacecast ptr addrspace(2) %528 to ptr addrspace(1)
  %530 = getelementptr i8, ptr addrspace(1) %529, i16 124
  store i16 5, ptr addrspace(1) %530, !tbaa !18
  %531 = getelementptr i8, ptr @"XO%", i16 2
  %532 = load i16, ptr %531, !tbaa !2
  %533 = inttoptr i16 %532 to ptr addrspace(2)
  %534 = addrspacecast ptr addrspace(2) %533 to ptr addrspace(1)
  %535 = getelementptr i8, ptr addrspace(1) %534, i16 126
  store i16 5, ptr addrspace(1) %535, !tbaa !16
  %536 = getelementptr i8, ptr @"YO%", i16 2
  %537 = load i16, ptr %536, !tbaa !2
  %538 = inttoptr i16 %537 to ptr addrspace(2)
  %539 = addrspacecast ptr addrspace(2) %538 to ptr addrspace(1)
  %540 = getelementptr i8, ptr addrspace(1) %539, i16 126
  store i16 5, ptr addrspace(1) %540, !tbaa !20
  %541 = getelementptr i8, ptr @"ZO%", i16 2
  %542 = load i16, ptr %541, !tbaa !2
  %543 = inttoptr i16 %542 to ptr addrspace(2)
  %544 = addrspacecast ptr addrspace(2) %543 to ptr addrspace(1)
  %545 = getelementptr i8, ptr addrspace(1) %544, i16 126
  store i16 5, ptr addrspace(1) %545, !tbaa !18
  store i16 1, ptr %106, !tbaa !2
  store i16 63, ptr %105, !tbaa !2
  store i16 1, ptr %104, !tbaa !2
  br label %b10

b10:
  %546 = load i16, ptr %104, !tbaa !2
  %547 = icmp sge i16 %546, 0
  br i1 %547, label %b11, label %b12

b11:
  %548 = load i16, ptr %106, !tbaa !2
  %549 = load i16, ptr %105, !tbaa !2
  %550 = icmp sle i16 %548, %549
  br i1 %550, label %b13, label %b14

b12:
  %551 = load i16, ptr %106, !tbaa !2
  %552 = load i16, ptr %105, !tbaa !2
  %553 = icmp sge i16 %551, %552
  br i1 %553, label %b13, label %b14

b13:
  %554 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %555 = load float, ptr %554
  store i16 64, ptr %102, !tbaa !2
  %556 = load i16, ptr %102, !tbaa !2
  %557 = sitofp i16 %556 to float
  %558 = fmul float %555, %557
  store i16 32, ptr %101, !tbaa !2
  %559 = load i16, ptr %101, !tbaa !2
  %560 = sitofp i16 %559 to float
  %561 = fsub float %558, %560
  %562 = call float @llvm.rint.f32(float %561)
  %563 = fcmp olt float %561, %562
  %564 = sext i1 %563 to i16
  store i16 %564, ptr %100, !tbaa !2
  %565 = load i16, ptr %100, !tbaa !2
  %566 = sitofp i16 %565 to float
  %567 = fadd float %562, %566
  %568 = call i16 @llvm.lrint.i16.f32(float %567)
  store i16 %568, ptr %103, !tbaa !2
  %569 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %570 = load float, ptr %569
  store i16 64, ptr %98, !tbaa !2
  %571 = load i16, ptr %98, !tbaa !2
  %572 = sitofp i16 %571 to float
  %573 = fmul float %570, %572
  store i16 32, ptr %97, !tbaa !2
  %574 = load i16, ptr %97, !tbaa !2
  %575 = sitofp i16 %574 to float
  %576 = fsub float %573, %575
  %577 = call float @llvm.rint.f32(float %576)
  %578 = fcmp olt float %576, %577
  %579 = sext i1 %578 to i16
  store i16 %579, ptr %96, !tbaa !2
  %580 = load i16, ptr %96, !tbaa !2
  %581 = sitofp i16 %580 to float
  %582 = fadd float %577, %581
  %583 = call i16 @llvm.lrint.i16.f32(float %582)
  store i16 %583, ptr %99, !tbaa !2
  %584 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %585 = load float, ptr %584
  store i16 64, ptr %94, !tbaa !2
  %586 = load i16, ptr %94, !tbaa !2
  %587 = sitofp i16 %586 to float
  %588 = fmul float %585, %587
  store i16 32, ptr %93, !tbaa !2
  %589 = load i16, ptr %93, !tbaa !2
  %590 = sitofp i16 %589 to float
  %591 = fsub float %588, %590
  %592 = call float @llvm.rint.f32(float %591)
  %593 = fcmp olt float %591, %592
  %594 = sext i1 %593 to i16
  store i16 %594, ptr %92, !tbaa !2
  %595 = load i16, ptr %92, !tbaa !2
  %596 = sitofp i16 %595 to float
  %597 = fadd float %592, %596
  %598 = call i16 @llvm.lrint.i16.f32(float %597)
  store i16 %598, ptr %95, !tbaa !2
  store i16 0, ptr %109, !tbaa !2
  store i16 63, ptr %91, !tbaa !2
  store i16 1, ptr %90, !tbaa !2
  br label %b15

b14:
  br label %b2

b15:
  %599 = load i16, ptr %90, !tbaa !2
  %600 = icmp sge i16 %599, 0
  br i1 %600, label %b16, label %b17

b16:
  %601 = load i16, ptr %109, !tbaa !2
  %602 = load i16, ptr %91, !tbaa !2
  %603 = icmp sle i16 %601, %602
  br i1 %603, label %b18, label %b19

b17:
  %604 = load i16, ptr %109, !tbaa !2
  %605 = load i16, ptr %91, !tbaa !2
  %606 = icmp sge i16 %604, %605
  br i1 %606, label %b18, label %b19

b18:
  %607 = load i16, ptr %109, !tbaa !2
  %608 = load i16, ptr %106, !tbaa !2
  %609 = mul i16 %608, 64
  %610 = add i16 %607, %609
  %611 = mul i16 %610, 2
  %612 = getelementptr i8, ptr @"XO%", i16 2
  %613 = load i16, ptr %612, !tbaa !2
  %614 = add i16 0, %611
  %615 = inttoptr i16 %613 to ptr addrspace(2)
  %616 = addrspacecast ptr addrspace(2) %615 to ptr addrspace(1)
  %617 = getelementptr i8, ptr addrspace(1) %616, i16 %614
  %618 = load i16, ptr %109, !tbaa !2
  %619 = mul i16 %618, 2
  %620 = getelementptr i8, ptr @"XO%", i16 2
  %621 = load i16, ptr %620, !tbaa !2
  %622 = add i16 0, %619
  %623 = inttoptr i16 %621 to ptr addrspace(2)
  %624 = addrspacecast ptr addrspace(2) %623 to ptr addrspace(1)
  %625 = getelementptr i8, ptr addrspace(1) %624, i16 %622
  %626 = load i16, ptr addrspace(1) %625, !tbaa !16
  %627 = load i16, ptr %103, !tbaa !2
  %628 = add i16 %626, %627
  store i16 %628, ptr addrspace(1) %617, !tbaa !16
  %629 = load i16, ptr %109, !tbaa !2
  %630 = load i16, ptr %106, !tbaa !2
  %631 = mul i16 %630, 64
  %632 = add i16 %629, %631
  %633 = mul i16 %632, 2
  %634 = getelementptr i8, ptr @"YO%", i16 2
  %635 = load i16, ptr %634, !tbaa !2
  %636 = add i16 0, %633
  %637 = inttoptr i16 %635 to ptr addrspace(2)
  %638 = addrspacecast ptr addrspace(2) %637 to ptr addrspace(1)
  %639 = getelementptr i8, ptr addrspace(1) %638, i16 %636
  %640 = load i16, ptr %109, !tbaa !2
  %641 = mul i16 %640, 2
  %642 = getelementptr i8, ptr @"YO%", i16 2
  %643 = load i16, ptr %642, !tbaa !2
  %644 = add i16 0, %641
  %645 = inttoptr i16 %643 to ptr addrspace(2)
  %646 = addrspacecast ptr addrspace(2) %645 to ptr addrspace(1)
  %647 = getelementptr i8, ptr addrspace(1) %646, i16 %644
  %648 = load i16, ptr addrspace(1) %647, !tbaa !20
  %649 = load i16, ptr %99, !tbaa !2
  %650 = add i16 %648, %649
  store i16 %650, ptr addrspace(1) %639, !tbaa !20
  %651 = load i16, ptr %109, !tbaa !2
  %652 = load i16, ptr %106, !tbaa !2
  %653 = mul i16 %652, 64
  %654 = add i16 %651, %653
  %655 = mul i16 %654, 2
  %656 = getelementptr i8, ptr @"ZO%", i16 2
  %657 = load i16, ptr %656, !tbaa !2
  %658 = add i16 0, %655
  %659 = inttoptr i16 %657 to ptr addrspace(2)
  %660 = addrspacecast ptr addrspace(2) %659 to ptr addrspace(1)
  %661 = getelementptr i8, ptr addrspace(1) %660, i16 %658
  %662 = load i16, ptr %109, !tbaa !2
  %663 = mul i16 %662, 2
  %664 = getelementptr i8, ptr @"ZO%", i16 2
  %665 = load i16, ptr %664, !tbaa !2
  %666 = add i16 0, %663
  %667 = inttoptr i16 %665 to ptr addrspace(2)
  %668 = addrspacecast ptr addrspace(2) %667 to ptr addrspace(1)
  %669 = getelementptr i8, ptr addrspace(1) %668, i16 %666
  %670 = load i16, ptr addrspace(1) %669, !tbaa !18
  %671 = load i16, ptr %95, !tbaa !2
  %672 = add i16 %670, %671
  store i16 %672, ptr addrspace(1) %661, !tbaa !18
  %673 = load i16, ptr %109, !tbaa !2
  %674 = load i16, ptr %90, !tbaa !2
  %675 = add nsw i16 %673, %674
  store i16 %675, ptr %109, !tbaa !2
  br label %b15

b19:
  %676 = load i16, ptr %106, !tbaa !2
  %677 = load i16, ptr %104, !tbaa !2
  %678 = add nsw i16 %676, %677
  store i16 %678, ptr %106, !tbaa !2
  br label %b10

b20:
  store i16 0, ptr %106, !tbaa !2
  store i16 90, ptr %109, !tbaa !2
  store i16 270, ptr %89, !tbaa !2
  %679 = load float, ptr @$float216, !tbaa !2
  %680 = call i16 @llvm.lrint.i16.f32(float %679)
  store i16 %680, ptr %88, !tbaa !2
  br label %b22

b21:
  %681 = icmp eq i16 %115, 3
  br i1 %681, label %b55, label %b56

b22:
  %682 = load i16, ptr %88, !tbaa !2
  %683 = icmp sge i16 %682, 0
  br i1 %683, label %b23, label %b24

b23:
  %684 = load i16, ptr %109, !tbaa !2
  %685 = load i16, ptr %89, !tbaa !2
  %686 = icmp sle i16 %684, %685
  br i1 %686, label %b25, label %b26

b24:
  %687 = load i16, ptr %109, !tbaa !2
  %688 = load i16, ptr %89, !tbaa !2
  %689 = icmp sge i16 %687, %688
  br i1 %689, label %b25, label %b26

b25:
  %690 = load i16, ptr %109, !tbaa !2
  %691 = load float, ptr %111, !tbaa !2
  store i16 %690, ptr %86, !tbaa !2
  %692 = load i16, ptr %86, !tbaa !2
  %693 = sitofp i16 %692 to float
  %694 = fdiv float %693, %691
  %695 = fpround float %694 to float
  store float %695, ptr %87, !tbaa !2
  %696 = load i16, ptr %106, !tbaa !2
  %697 = mul i16 %696, 2
  %698 = getelementptr i8, ptr @"XO%", i16 2
  %699 = load i16, ptr %698, !tbaa !2
  %700 = add i16 0, %697
  %701 = inttoptr i16 %699 to ptr addrspace(2)
  %702 = addrspacecast ptr addrspace(2) %701 to ptr addrspace(1)
  %703 = getelementptr i8, ptr addrspace(1) %702, i16 %700
  %704 = load float, ptr %87, !tbaa !2
  %705 = call float @llvm.fabs.f32(float %704)
  %706 = load float, ptr @$float86, !tbaa !2
  %707 = fcmp ogt float %705, %706
  br i1 %707, label %b27, label %b28

b26:
  store i16 0, ptr %82, !tbaa !2
  %708 = load i16, ptr %82, !tbaa !2
  %709 = sitofp i16 %708 to float
  %710 = fpround float %709 to float
  store float %710, ptr %83, !tbaa !2
  store i16 1, ptr %109, !tbaa !2
  store i16 15, ptr %81, !tbaa !2
  store i16 1, ptr %80, !tbaa !2
  br label %b31

b27:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b28:
  %711 = call float @llvm.cos.f32(float %704)
  store i16 63, ptr %85, !tbaa !2
  %712 = load i16, ptr %85, !tbaa !2
  %713 = sitofp i16 %712 to float
  %714 = fmul float %711, %713
  %715 = call i16 @llvm.lrint.i16.f32(float %714)
  store i16 %715, ptr addrspace(1) %703, !tbaa !16
  %716 = load i16, ptr %106, !tbaa !2
  %717 = mul i16 %716, 2
  %718 = getelementptr i8, ptr @"YO%", i16 2
  %719 = load i16, ptr %718, !tbaa !2
  %720 = add i16 0, %717
  %721 = inttoptr i16 %719 to ptr addrspace(2)
  %722 = addrspacecast ptr addrspace(2) %721 to ptr addrspace(1)
  %723 = getelementptr i8, ptr addrspace(1) %722, i16 %720
  %724 = load float, ptr %87, !tbaa !2
  %725 = call float @llvm.fabs.f32(float %724)
  %726 = load float, ptr @$float86, !tbaa !2
  %727 = fcmp ogt float %725, %726
  br i1 %727, label %b29, label %b30

b29:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b30:
  %728 = call float @llvm.sin.f32(float %724)
  store i16 63, ptr %84, !tbaa !2
  %729 = load i16, ptr %84, !tbaa !2
  %730 = sitofp i16 %729 to float
  %731 = fmul float %728, %730
  %732 = call i16 @llvm.lrint.i16.f32(float %731)
  store i16 %732, ptr addrspace(1) %723, !tbaa !20
  %733 = load i16, ptr %106, !tbaa !2
  %734 = mul i16 %733, 2
  %735 = getelementptr i8, ptr @"ZO%", i16 2
  %736 = load i16, ptr %735, !tbaa !2
  %737 = add i16 0, %734
  %738 = inttoptr i16 %736 to ptr addrspace(2)
  %739 = addrspacecast ptr addrspace(2) %738 to ptr addrspace(1)
  %740 = getelementptr i8, ptr addrspace(1) %739, i16 %737
  store i16 0, ptr addrspace(1) %740, !tbaa !18
  %741 = load i16, ptr %106, !tbaa !2
  %742 = add i16 %741, 1
  store i16 %742, ptr %106, !tbaa !2
  %743 = load i16, ptr %109, !tbaa !2
  %744 = load i16, ptr %88, !tbaa !2
  %745 = add nsw i16 %743, %744
  store i16 %745, ptr %109, !tbaa !2
  br label %b22

b31:
  %746 = load i16, ptr %80, !tbaa !2
  %747 = icmp sge i16 %746, 0
  br i1 %747, label %b32, label %b33

b32:
  %748 = load i16, ptr %109, !tbaa !2
  %749 = load i16, ptr %81, !tbaa !2
  %750 = icmp sle i16 %748, %749
  br i1 %750, label %b34, label %b35

b33:
  %751 = load i16, ptr %109, !tbaa !2
  %752 = load i16, ptr %81, !tbaa !2
  %753 = icmp sge i16 %751, %752
  br i1 %753, label %b34, label %b35

b34:
  %754 = load float, ptr %83, !tbaa !2
  %755 = load float, ptr @$float217, !tbaa !2
  %756 = fadd float %754, %755
  %757 = fpround float %756 to float
  store float %757, ptr %83, !tbaa !2
  %758 = load float, ptr %83, !tbaa !2
  %759 = load float, ptr %111, !tbaa !2
  %760 = fdiv float %758, %759
  %761 = fpround float %760 to float
  store float %761, ptr %87, !tbaa !2
  %762 = load float, ptr %87, !tbaa !2
  %763 = call float @llvm.fabs.f32(float %762)
  %764 = load float, ptr @$float86, !tbaa !2
  %765 = fcmp ogt float %763, %764
  br i1 %765, label %b36, label %b37

b35:
  store i16 1, ptr %106, !tbaa !2
  store i16 15, ptr %71, !tbaa !2
  store i16 1, ptr %70, !tbaa !2
  br label %b45

b36:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b37:
  %766 = call float @llvm.cos.f32(float %762)
  %767 = fpround float %766 to float
  store float %767, ptr %79, !tbaa !2
  %768 = load float, ptr %87, !tbaa !2
  %769 = call float @llvm.fabs.f32(float %768)
  %770 = load float, ptr @$float86, !tbaa !2
  %771 = fcmp ogt float %769, %770
  br i1 %771, label %b38, label %b39

b38:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b39:
  %772 = call float @llvm.sin.f32(float %768)
  %773 = fpround float %772 to float
  store float %773, ptr %78, !tbaa !2
  store i16 0, ptr %106, !tbaa !2
  store i16 15, ptr %77, !tbaa !2
  store i16 1, ptr %76, !tbaa !2
  br label %b40

b40:
  %774 = load i16, ptr %76, !tbaa !2
  %775 = icmp sge i16 %774, 0
  br i1 %775, label %b41, label %b42

b41:
  %776 = load i16, ptr %106, !tbaa !2
  %777 = load i16, ptr %77, !tbaa !2
  %778 = icmp sle i16 %776, %777
  br i1 %778, label %b43, label %b44

b42:
  %779 = load i16, ptr %106, !tbaa !2
  %780 = load i16, ptr %77, !tbaa !2
  %781 = icmp sge i16 %779, %780
  br i1 %781, label %b43, label %b44

b43:
  %782 = load i16, ptr %106, !tbaa !2
  %783 = load i16, ptr %109, !tbaa !2
  %784 = mul i16 %783, 16
  %785 = add i16 %782, %784
  %786 = mul i16 %785, 2
  %787 = getelementptr i8, ptr @"XO%", i16 2
  %788 = load i16, ptr %787, !tbaa !2
  %789 = add i16 0, %786
  %790 = inttoptr i16 %788 to ptr addrspace(2)
  %791 = addrspacecast ptr addrspace(2) %790 to ptr addrspace(1)
  %792 = getelementptr i8, ptr addrspace(1) %791, i16 %789
  %793 = load float, ptr %79, !tbaa !2
  %794 = load i16, ptr %106, !tbaa !2
  %795 = mul i16 %794, 2
  %796 = getelementptr i8, ptr @"XO%", i16 2
  %797 = load i16, ptr %796, !tbaa !2
  %798 = add i16 0, %795
  %799 = inttoptr i16 %797 to ptr addrspace(2)
  %800 = addrspacecast ptr addrspace(2) %799 to ptr addrspace(1)
  %801 = getelementptr i8, ptr addrspace(1) %800, i16 %798
  %802 = load i16, ptr addrspace(1) %801, !tbaa !16
  store i16 %802, ptr %75, !tbaa !2
  %803 = load i16, ptr %75, !tbaa !2
  %804 = sitofp i16 %803 to float
  %805 = fmul float %793, %804
  %806 = load float, ptr %78, !tbaa !2
  %807 = load i16, ptr %106, !tbaa !2
  %808 = mul i16 %807, 2
  %809 = getelementptr i8, ptr @"ZO%", i16 2
  %810 = load i16, ptr %809, !tbaa !2
  %811 = add i16 0, %808
  %812 = inttoptr i16 %810 to ptr addrspace(2)
  %813 = addrspacecast ptr addrspace(2) %812 to ptr addrspace(1)
  %814 = getelementptr i8, ptr addrspace(1) %813, i16 %811
  %815 = load i16, ptr addrspace(1) %814, !tbaa !18
  store i16 %815, ptr %74, !tbaa !2
  %816 = load i16, ptr %74, !tbaa !2
  %817 = sitofp i16 %816 to float
  %818 = fmul float %806, %817
  %819 = fsub float %805, %818
  %820 = call i16 @llvm.lrint.i16.f32(float %819)
  store i16 %820, ptr addrspace(1) %792, !tbaa !16
  %821 = load i16, ptr %106, !tbaa !2
  %822 = load i16, ptr %109, !tbaa !2
  %823 = mul i16 %822, 16
  %824 = add i16 %821, %823
  %825 = mul i16 %824, 2
  %826 = getelementptr i8, ptr @"ZO%", i16 2
  %827 = load i16, ptr %826, !tbaa !2
  %828 = add i16 0, %825
  %829 = inttoptr i16 %827 to ptr addrspace(2)
  %830 = addrspacecast ptr addrspace(2) %829 to ptr addrspace(1)
  %831 = getelementptr i8, ptr addrspace(1) %830, i16 %828
  %832 = load float, ptr %78, !tbaa !2
  %833 = load i16, ptr %106, !tbaa !2
  %834 = mul i16 %833, 2
  %835 = getelementptr i8, ptr @"XO%", i16 2
  %836 = load i16, ptr %835, !tbaa !2
  %837 = add i16 0, %834
  %838 = inttoptr i16 %836 to ptr addrspace(2)
  %839 = addrspacecast ptr addrspace(2) %838 to ptr addrspace(1)
  %840 = getelementptr i8, ptr addrspace(1) %839, i16 %837
  %841 = load i16, ptr addrspace(1) %840, !tbaa !16
  store i16 %841, ptr %73, !tbaa !2
  %842 = load i16, ptr %73, !tbaa !2
  %843 = sitofp i16 %842 to float
  %844 = fmul float %832, %843
  %845 = load float, ptr %79, !tbaa !2
  %846 = load i16, ptr %106, !tbaa !2
  %847 = mul i16 %846, 2
  %848 = getelementptr i8, ptr @"ZO%", i16 2
  %849 = load i16, ptr %848, !tbaa !2
  %850 = add i16 0, %847
  %851 = inttoptr i16 %849 to ptr addrspace(2)
  %852 = addrspacecast ptr addrspace(2) %851 to ptr addrspace(1)
  %853 = getelementptr i8, ptr addrspace(1) %852, i16 %850
  %854 = load i16, ptr addrspace(1) %853, !tbaa !18
  store i16 %854, ptr %72, !tbaa !2
  %855 = load i16, ptr %72, !tbaa !2
  %856 = sitofp i16 %855 to float
  %857 = fmul float %845, %856
  %858 = fadd float %844, %857
  %859 = call i16 @llvm.lrint.i16.f32(float %858)
  store i16 %859, ptr addrspace(1) %831, !tbaa !18
  %860 = load i16, ptr %106, !tbaa !2
  %861 = load i16, ptr %109, !tbaa !2
  %862 = mul i16 %861, 16
  %863 = add i16 %860, %862
  %864 = mul i16 %863, 2
  %865 = getelementptr i8, ptr @"YO%", i16 2
  %866 = load i16, ptr %865, !tbaa !2
  %867 = add i16 0, %864
  %868 = inttoptr i16 %866 to ptr addrspace(2)
  %869 = addrspacecast ptr addrspace(2) %868 to ptr addrspace(1)
  %870 = getelementptr i8, ptr addrspace(1) %869, i16 %867
  %871 = load i16, ptr %106, !tbaa !2
  %872 = mul i16 %871, 2
  %873 = getelementptr i8, ptr @"YO%", i16 2
  %874 = load i16, ptr %873, !tbaa !2
  %875 = add i16 0, %872
  %876 = inttoptr i16 %874 to ptr addrspace(2)
  %877 = addrspacecast ptr addrspace(2) %876 to ptr addrspace(1)
  %878 = getelementptr i8, ptr addrspace(1) %877, i16 %875
  %879 = load i16, ptr addrspace(1) %878, !tbaa !20
  store i16 %879, ptr addrspace(1) %870, !tbaa !20
  %880 = load i16, ptr %106, !tbaa !2
  %881 = load i16, ptr %76, !tbaa !2
  %882 = add nsw i16 %880, %881
  store i16 %882, ptr %106, !tbaa !2
  br label %b40

b44:
  %883 = load i16, ptr %109, !tbaa !2
  %884 = load i16, ptr %80, !tbaa !2
  %885 = add nsw i16 %883, %884
  store i16 %885, ptr %109, !tbaa !2
  br label %b31

b45:
  %886 = load i16, ptr %70, !tbaa !2
  %887 = icmp sge i16 %886, 0
  br i1 %887, label %b46, label %b47

b46:
  %888 = load i16, ptr %106, !tbaa !2
  %889 = load i16, ptr %71, !tbaa !2
  %890 = icmp sle i16 %888, %889
  br i1 %890, label %b48, label %b49

b47:
  %891 = load i16, ptr %106, !tbaa !2
  %892 = load i16, ptr %71, !tbaa !2
  %893 = icmp sge i16 %891, %892
  br i1 %893, label %b48, label %b49

b48:
  %894 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %895 = load float, ptr %894
  store i16 512, ptr %69, !tbaa !2
  %896 = load i16, ptr %69, !tbaa !2
  %897 = sitofp i16 %896 to float
  %898 = fmul float %895, %897
  store i16 256, ptr %68, !tbaa !2
  %899 = load i16, ptr %68, !tbaa !2
  %900 = sitofp i16 %899 to float
  %901 = fsub float %898, %900
  %902 = call float @llvm.rint.f32(float %901)
  %903 = fcmp olt float %901, %902
  %904 = sext i1 %903 to i16
  store i16 %904, ptr %67, !tbaa !2
  %905 = load i16, ptr %67, !tbaa !2
  %906 = sitofp i16 %905 to float
  %907 = fadd float %902, %906
  %908 = call i16 @llvm.lrint.i16.f32(float %907)
  store i16 %908, ptr %103, !tbaa !2
  %909 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %910 = load float, ptr %909
  store i16 512, ptr %66, !tbaa !2
  %911 = load i16, ptr %66, !tbaa !2
  %912 = sitofp i16 %911 to float
  %913 = fmul float %910, %912
  store i16 256, ptr %65, !tbaa !2
  %914 = load i16, ptr %65, !tbaa !2
  %915 = sitofp i16 %914 to float
  %916 = fsub float %913, %915
  %917 = call float @llvm.rint.f32(float %916)
  %918 = fcmp olt float %916, %917
  %919 = sext i1 %918 to i16
  store i16 %919, ptr %64, !tbaa !2
  %920 = load i16, ptr %64, !tbaa !2
  %921 = sitofp i16 %920 to float
  %922 = fadd float %917, %921
  %923 = call i16 @llvm.lrint.i16.f32(float %922)
  store i16 %923, ptr %99, !tbaa !2
  %924 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %925 = load float, ptr %924
  store i16 512, ptr %63, !tbaa !2
  %926 = load i16, ptr %63, !tbaa !2
  %927 = sitofp i16 %926 to float
  %928 = fmul float %925, %927
  store i16 256, ptr %62, !tbaa !2
  %929 = load i16, ptr %62, !tbaa !2
  %930 = sitofp i16 %929 to float
  %931 = fsub float %928, %930
  %932 = call float @llvm.rint.f32(float %931)
  %933 = fcmp olt float %931, %932
  %934 = sext i1 %933 to i16
  store i16 %934, ptr %61, !tbaa !2
  %935 = load i16, ptr %61, !tbaa !2
  %936 = sitofp i16 %935 to float
  %937 = fadd float %932, %936
  %938 = call i16 @llvm.lrint.i16.f32(float %937)
  store i16 %938, ptr %95, !tbaa !2
  store i16 0, ptr %109, !tbaa !2
  store i16 255, ptr %60, !tbaa !2
  store i16 1, ptr %59, !tbaa !2
  br label %b50

b49:
  br label %b2

b50:
  %939 = load i16, ptr %59, !tbaa !2
  %940 = icmp sge i16 %939, 0
  br i1 %940, label %b51, label %b52

b51:
  %941 = load i16, ptr %109, !tbaa !2
  %942 = load i16, ptr %60, !tbaa !2
  %943 = icmp sle i16 %941, %942
  br i1 %943, label %b53, label %b54

b52:
  %944 = load i16, ptr %109, !tbaa !2
  %945 = load i16, ptr %60, !tbaa !2
  %946 = icmp sge i16 %944, %945
  br i1 %946, label %b53, label %b54

b53:
  %947 = load i16, ptr %109, !tbaa !2
  %948 = load i16, ptr %106, !tbaa !2
  %949 = mul i16 %948, 256
  %950 = add i16 %947, %949
  %951 = mul i16 %950, 2
  %952 = getelementptr i8, ptr @"XO%", i16 2
  %953 = load i16, ptr %952, !tbaa !2
  %954 = add i16 0, %951
  %955 = inttoptr i16 %953 to ptr addrspace(2)
  %956 = addrspacecast ptr addrspace(2) %955 to ptr addrspace(1)
  %957 = getelementptr i8, ptr addrspace(1) %956, i16 %954
  %958 = load i16, ptr %109, !tbaa !2
  %959 = mul i16 %958, 2
  %960 = getelementptr i8, ptr @"XO%", i16 2
  %961 = load i16, ptr %960, !tbaa !2
  %962 = add i16 0, %959
  %963 = inttoptr i16 %961 to ptr addrspace(2)
  %964 = addrspacecast ptr addrspace(2) %963 to ptr addrspace(1)
  %965 = getelementptr i8, ptr addrspace(1) %964, i16 %962
  %966 = load i16, ptr addrspace(1) %965, !tbaa !16
  %967 = load i16, ptr %103, !tbaa !2
  %968 = add i16 %966, %967
  store i16 %968, ptr addrspace(1) %957, !tbaa !16
  %969 = load i16, ptr %109, !tbaa !2
  %970 = load i16, ptr %106, !tbaa !2
  %971 = mul i16 %970, 256
  %972 = add i16 %969, %971
  %973 = mul i16 %972, 2
  %974 = getelementptr i8, ptr @"YO%", i16 2
  %975 = load i16, ptr %974, !tbaa !2
  %976 = add i16 0, %973
  %977 = inttoptr i16 %975 to ptr addrspace(2)
  %978 = addrspacecast ptr addrspace(2) %977 to ptr addrspace(1)
  %979 = getelementptr i8, ptr addrspace(1) %978, i16 %976
  %980 = load i16, ptr %109, !tbaa !2
  %981 = mul i16 %980, 2
  %982 = getelementptr i8, ptr @"YO%", i16 2
  %983 = load i16, ptr %982, !tbaa !2
  %984 = add i16 0, %981
  %985 = inttoptr i16 %983 to ptr addrspace(2)
  %986 = addrspacecast ptr addrspace(2) %985 to ptr addrspace(1)
  %987 = getelementptr i8, ptr addrspace(1) %986, i16 %984
  %988 = load i16, ptr addrspace(1) %987, !tbaa !20
  %989 = load i16, ptr %99, !tbaa !2
  %990 = add i16 %988, %989
  store i16 %990, ptr addrspace(1) %979, !tbaa !20
  %991 = load i16, ptr %109, !tbaa !2
  %992 = load i16, ptr %106, !tbaa !2
  %993 = mul i16 %992, 256
  %994 = add i16 %991, %993
  %995 = mul i16 %994, 2
  %996 = getelementptr i8, ptr @"ZO%", i16 2
  %997 = load i16, ptr %996, !tbaa !2
  %998 = add i16 0, %995
  %999 = inttoptr i16 %997 to ptr addrspace(2)
  %1000 = addrspacecast ptr addrspace(2) %999 to ptr addrspace(1)
  %1001 = getelementptr i8, ptr addrspace(1) %1000, i16 %998
  %1002 = load i16, ptr %109, !tbaa !2
  %1003 = mul i16 %1002, 2
  %1004 = getelementptr i8, ptr @"ZO%", i16 2
  %1005 = load i16, ptr %1004, !tbaa !2
  %1006 = add i16 0, %1003
  %1007 = inttoptr i16 %1005 to ptr addrspace(2)
  %1008 = addrspacecast ptr addrspace(2) %1007 to ptr addrspace(1)
  %1009 = getelementptr i8, ptr addrspace(1) %1008, i16 %1006
  %1010 = load i16, ptr addrspace(1) %1009, !tbaa !18
  %1011 = load i16, ptr %95, !tbaa !2
  %1012 = add i16 %1010, %1011
  store i16 %1012, ptr addrspace(1) %1001, !tbaa !18
  %1013 = load i16, ptr %109, !tbaa !2
  %1014 = load i16, ptr %59, !tbaa !2
  %1015 = add nsw i16 %1013, %1014
  store i16 %1015, ptr %109, !tbaa !2
  br label %b50

b54:
  %1016 = load i16, ptr %106, !tbaa !2
  %1017 = load i16, ptr %70, !tbaa !2
  %1018 = add nsw i16 %1016, %1017
  store i16 %1018, ptr %106, !tbaa !2
  br label %b45

b55:
  store i16 0, ptr %106, !tbaa !2
  store i16 0, ptr %109, !tbaa !2
  store i16 359, ptr %58, !tbaa !2
  %1019 = load float, ptr @$float217, !tbaa !2
  %1020 = call i16 @llvm.lrint.i16.f32(float %1019)
  store i16 %1020, ptr %57, !tbaa !2
  br label %b57

b56:
  br label %b2

b57:
  %1021 = load i16, ptr %57, !tbaa !2
  %1022 = icmp sge i16 %1021, 0
  br i1 %1022, label %b58, label %b59

b58:
  %1023 = load i16, ptr %109, !tbaa !2
  %1024 = load i16, ptr %58, !tbaa !2
  %1025 = icmp sle i16 %1023, %1024
  br i1 %1025, label %b60, label %b61

b59:
  %1026 = load i16, ptr %109, !tbaa !2
  %1027 = load i16, ptr %58, !tbaa !2
  %1028 = icmp sge i16 %1026, %1027
  br i1 %1028, label %b60, label %b61

b60:
  %1029 = load i16, ptr %109, !tbaa !2
  %1030 = load float, ptr %111, !tbaa !2
  store i16 %1029, ptr %56, !tbaa !2
  %1031 = load i16, ptr %56, !tbaa !2
  %1032 = sitofp i16 %1031 to float
  %1033 = fdiv float %1032, %1030
  %1034 = fpround float %1033 to float
  store float %1034, ptr %87, !tbaa !2
  %1035 = load i16, ptr %106, !tbaa !2
  %1036 = mul i16 %1035, 2
  %1037 = getelementptr i8, ptr @"XO%", i16 2
  %1038 = load i16, ptr %1037, !tbaa !2
  %1039 = add i16 0, %1036
  %1040 = inttoptr i16 %1038 to ptr addrspace(2)
  %1041 = addrspacecast ptr addrspace(2) %1040 to ptr addrspace(1)
  %1042 = getelementptr i8, ptr addrspace(1) %1041, i16 %1039
  %1043 = load float, ptr %87, !tbaa !2
  %1044 = call float @llvm.fabs.f32(float %1043)
  %1045 = load float, ptr @$float86, !tbaa !2
  %1046 = fcmp ogt float %1044, %1045
  br i1 %1046, label %b62, label %b63

b61:
  store i16 90, ptr %109, !tbaa !2
  %1047 = load i16, ptr %109, !tbaa !2
  %1048 = load float, ptr %111, !tbaa !2
  store i16 %1047, ptr %53, !tbaa !2
  %1049 = load i16, ptr %53, !tbaa !2
  %1050 = sitofp i16 %1049 to float
  %1051 = fdiv float %1050, %1048
  %1052 = fpround float %1051 to float
  store float %1052, ptr %87, !tbaa !2
  %1053 = load float, ptr %87, !tbaa !2
  %1054 = call float @llvm.fabs.f32(float %1053)
  %1055 = load float, ptr @$float86, !tbaa !2
  %1056 = fcmp ogt float %1054, %1055
  br i1 %1056, label %b66, label %b67

b62:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b63:
  %1057 = call float @llvm.cos.f32(float %1043)
  store i16 64, ptr %55, !tbaa !2
  %1058 = load i16, ptr %55, !tbaa !2
  %1059 = sitofp i16 %1058 to float
  %1060 = fmul float %1057, %1059
  %1061 = call i16 @llvm.lrint.i16.f32(float %1060)
  store i16 %1061, ptr addrspace(1) %1042, !tbaa !16
  %1062 = load i16, ptr %106, !tbaa !2
  %1063 = mul i16 %1062, 2
  %1064 = getelementptr i8, ptr @"YO%", i16 2
  %1065 = load i16, ptr %1064, !tbaa !2
  %1066 = add i16 0, %1063
  %1067 = inttoptr i16 %1065 to ptr addrspace(2)
  %1068 = addrspacecast ptr addrspace(2) %1067 to ptr addrspace(1)
  %1069 = getelementptr i8, ptr addrspace(1) %1068, i16 %1066
  %1070 = load float, ptr %87, !tbaa !2
  %1071 = call float @llvm.fabs.f32(float %1070)
  %1072 = load float, ptr @$float86, !tbaa !2
  %1073 = fcmp ogt float %1071, %1072
  br i1 %1073, label %b64, label %b65

b64:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b65:
  %1074 = call float @llvm.sin.f32(float %1070)
  store i16 64, ptr %54, !tbaa !2
  %1075 = load i16, ptr %54, !tbaa !2
  %1076 = sitofp i16 %1075 to float
  %1077 = fmul float %1074, %1076
  %1078 = call i16 @llvm.lrint.i16.f32(float %1077)
  store i16 %1078, ptr addrspace(1) %1069, !tbaa !20
  %1079 = load i16, ptr %106, !tbaa !2
  %1080 = mul i16 %1079, 2
  %1081 = getelementptr i8, ptr @"ZO%", i16 2
  %1082 = load i16, ptr %1081, !tbaa !2
  %1083 = add i16 0, %1080
  %1084 = inttoptr i16 %1082 to ptr addrspace(2)
  %1085 = addrspacecast ptr addrspace(2) %1084 to ptr addrspace(1)
  %1086 = getelementptr i8, ptr addrspace(1) %1085, i16 %1083
  store i16 0, ptr addrspace(1) %1086, !tbaa !18
  %1087 = load i16, ptr %106, !tbaa !2
  %1088 = add i16 %1087, 1
  store i16 %1088, ptr %106, !tbaa !2
  %1089 = load i16, ptr %109, !tbaa !2
  %1090 = load i16, ptr %57, !tbaa !2
  %1091 = add nsw i16 %1089, %1090
  store i16 %1091, ptr %109, !tbaa !2
  br label %b57

b66:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b67:
  %1092 = call float @llvm.cos.f32(float %1053)
  %1093 = fpround float %1092 to float
  store float %1093, ptr %79, !tbaa !2
  %1094 = load float, ptr %87, !tbaa !2
  %1095 = call float @llvm.fabs.f32(float %1094)
  %1096 = load float, ptr @$float86, !tbaa !2
  %1097 = fcmp ogt float %1095, %1096
  br i1 %1097, label %b68, label %b69

b68:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b69:
  %1098 = call float @llvm.sin.f32(float %1094)
  %1099 = fpround float %1098 to float
  store float %1099, ptr %78, !tbaa !2
  store i16 0, ptr %109, !tbaa !2
  store i16 15, ptr %52, !tbaa !2
  store i16 1, ptr %51, !tbaa !2
  br label %b70

b70:
  %1100 = load i16, ptr %51, !tbaa !2
  %1101 = icmp sge i16 %1100, 0
  br i1 %1101, label %b71, label %b72

b71:
  %1102 = load i16, ptr %109, !tbaa !2
  %1103 = load i16, ptr %52, !tbaa !2
  %1104 = icmp sle i16 %1102, %1103
  br i1 %1104, label %b73, label %b74

b72:
  %1105 = load i16, ptr %109, !tbaa !2
  %1106 = load i16, ptr %52, !tbaa !2
  %1107 = icmp sge i16 %1105, %1106
  br i1 %1107, label %b73, label %b74

b73:
  %1108 = load i16, ptr %109, !tbaa !2
  %1109 = mul i16 %1108, 2
  %1110 = getelementptr i8, ptr @"XO%", i16 2
  %1111 = load i16, ptr %1110, !tbaa !2
  %1112 = add i16 0, %1109
  %1113 = inttoptr i16 %1111 to ptr addrspace(2)
  %1114 = addrspacecast ptr addrspace(2) %1113 to ptr addrspace(1)
  %1115 = getelementptr i8, ptr addrspace(1) %1114, i16 %1112
  %1116 = load i16, ptr addrspace(1) %1115, !tbaa !16
  store i16 %1116, ptr %50, !tbaa !2
  %1117 = load i16, ptr %50, !tbaa !2
  %1118 = sitofp i16 %1117 to float
  %1119 = fpround float %1118 to float
  store float %1119, ptr @"XP!", !tbaa !2
  %1120 = load i16, ptr %109, !tbaa !2
  %1121 = mul i16 %1120, 2
  %1122 = getelementptr i8, ptr @"XO%", i16 2
  %1123 = load i16, ptr %1122, !tbaa !2
  %1124 = add i16 0, %1121
  %1125 = inttoptr i16 %1123 to ptr addrspace(2)
  %1126 = addrspacecast ptr addrspace(2) %1125 to ptr addrspace(1)
  %1127 = getelementptr i8, ptr addrspace(1) %1126, i16 %1124
  %1128 = load float, ptr %79, !tbaa !2
  %1129 = load float, ptr @"XP!", !tbaa !2
  %1130 = fmul float %1128, %1129
  %1131 = load float, ptr %78, !tbaa !2
  %1132 = load i16, ptr %109, !tbaa !2
  %1133 = mul i16 %1132, 2
  %1134 = getelementptr i8, ptr @"ZO%", i16 2
  %1135 = load i16, ptr %1134, !tbaa !2
  %1136 = add i16 0, %1133
  %1137 = inttoptr i16 %1135 to ptr addrspace(2)
  %1138 = addrspacecast ptr addrspace(2) %1137 to ptr addrspace(1)
  %1139 = getelementptr i8, ptr addrspace(1) %1138, i16 %1136
  %1140 = load i16, ptr addrspace(1) %1139, !tbaa !18
  store i16 %1140, ptr %49, !tbaa !2
  %1141 = load i16, ptr %49, !tbaa !2
  %1142 = sitofp i16 %1141 to float
  %1143 = fmul float %1131, %1142
  %1144 = fsub float %1130, %1143
  %1145 = call i16 @llvm.lrint.i16.f32(float %1144)
  store i16 %1145, ptr addrspace(1) %1127, !tbaa !16
  %1146 = load i16, ptr %109, !tbaa !2
  %1147 = mul i16 %1146, 2
  %1148 = getelementptr i8, ptr @"ZO%", i16 2
  %1149 = load i16, ptr %1148, !tbaa !2
  %1150 = add i16 0, %1147
  %1151 = inttoptr i16 %1149 to ptr addrspace(2)
  %1152 = addrspacecast ptr addrspace(2) %1151 to ptr addrspace(1)
  %1153 = getelementptr i8, ptr addrspace(1) %1152, i16 %1150
  %1154 = load float, ptr %78, !tbaa !2
  %1155 = load float, ptr @"XP!", !tbaa !2
  %1156 = fmul float %1154, %1155
  %1157 = load float, ptr %79, !tbaa !2
  %1158 = load i16, ptr %109, !tbaa !2
  %1159 = mul i16 %1158, 2
  %1160 = getelementptr i8, ptr @"ZO%", i16 2
  %1161 = load i16, ptr %1160, !tbaa !2
  %1162 = add i16 0, %1159
  %1163 = inttoptr i16 %1161 to ptr addrspace(2)
  %1164 = addrspacecast ptr addrspace(2) %1163 to ptr addrspace(1)
  %1165 = getelementptr i8, ptr addrspace(1) %1164, i16 %1162
  %1166 = load i16, ptr addrspace(1) %1165, !tbaa !18
  store i16 %1166, ptr %48, !tbaa !2
  %1167 = load i16, ptr %48, !tbaa !2
  %1168 = sitofp i16 %1167 to float
  %1169 = fmul float %1157, %1168
  %1170 = fadd float %1156, %1169
  %1171 = call i16 @llvm.lrint.i16.f32(float %1170)
  store i16 %1171, ptr addrspace(1) %1153, !tbaa !18
  %1172 = load i16, ptr %109, !tbaa !2
  %1173 = mul i16 %1172, 2
  %1174 = getelementptr i8, ptr @"YO%", i16 2
  %1175 = load i16, ptr %1174, !tbaa !2
  %1176 = add i16 0, %1173
  %1177 = inttoptr i16 %1175 to ptr addrspace(2)
  %1178 = addrspacecast ptr addrspace(2) %1177 to ptr addrspace(1)
  %1179 = getelementptr i8, ptr addrspace(1) %1178, i16 %1176
  %1180 = load i16, ptr %109, !tbaa !2
  %1181 = mul i16 %1180, 2
  %1182 = getelementptr i8, ptr @"YO%", i16 2
  %1183 = load i16, ptr %1182, !tbaa !2
  %1184 = add i16 0, %1181
  %1185 = inttoptr i16 %1183 to ptr addrspace(2)
  %1186 = addrspacecast ptr addrspace(2) %1185 to ptr addrspace(1)
  %1187 = getelementptr i8, ptr addrspace(1) %1186, i16 %1184
  %1188 = load i16, ptr addrspace(1) %1187, !tbaa !20
  %1189 = sub i16 %1188, 192
  store i16 %1189, ptr addrspace(1) %1179, !tbaa !20
  %1190 = load i16, ptr %109, !tbaa !2
  %1191 = load i16, ptr %51, !tbaa !2
  %1192 = add nsw i16 %1190, %1191
  store i16 %1192, ptr %109, !tbaa !2
  br label %b70

b74:
  store i16 0, ptr %46, !tbaa !2
  %1193 = load i16, ptr %46, !tbaa !2
  %1194 = sitofp i16 %1193 to float
  %1195 = fpround float %1194 to float
  store float %1195, ptr %47, !tbaa !2
  store i16 1, ptr %106, !tbaa !2
  store i16 31, ptr %45, !tbaa !2
  store i16 1, ptr %44, !tbaa !2
  br label %b75

b75:
  %1196 = load i16, ptr %44, !tbaa !2
  %1197 = icmp sge i16 %1196, 0
  br i1 %1197, label %b76, label %b77

b76:
  %1198 = load i16, ptr %106, !tbaa !2
  %1199 = load i16, ptr %45, !tbaa !2
  %1200 = icmp sle i16 %1198, %1199
  br i1 %1200, label %b78, label %b79

b77:
  %1201 = load i16, ptr %106, !tbaa !2
  %1202 = load i16, ptr %45, !tbaa !2
  %1203 = icmp sge i16 %1201, %1202
  br i1 %1203, label %b78, label %b79

b78:
  %1204 = load float, ptr %47, !tbaa !2
  %1205 = load float, ptr @$float216, !tbaa !2
  %1206 = fadd float %1204, %1205
  %1207 = fpround float %1206 to float
  store float %1207, ptr %47, !tbaa !2
  %1208 = load float, ptr %47, !tbaa !2
  %1209 = load float, ptr %111, !tbaa !2
  %1210 = fdiv float %1208, %1209
  %1211 = fpround float %1210 to float
  store float %1211, ptr %87, !tbaa !2
  %1212 = load float, ptr %87, !tbaa !2
  %1213 = call float @llvm.fabs.f32(float %1212)
  %1214 = load float, ptr @$float86, !tbaa !2
  %1215 = fcmp ogt float %1213, %1214
  br i1 %1215, label %b80, label %b81

b79:
  store i16 1, ptr %106, !tbaa !2
  store i16 7, ptr %35, !tbaa !2
  store i16 1, ptr %34, !tbaa !2
  br label %b89

b80:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b81:
  %1216 = call float @llvm.cos.f32(float %1212)
  %1217 = fpround float %1216 to float
  store float %1217, ptr %43, !tbaa !2
  %1218 = load float, ptr %87, !tbaa !2
  %1219 = call float @llvm.fabs.f32(float %1218)
  %1220 = load float, ptr @$float86, !tbaa !2
  %1221 = fcmp ogt float %1219, %1220
  br i1 %1221, label %b82, label %b83

b82:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b83:
  %1222 = call float @llvm.sin.f32(float %1218)
  %1223 = fpround float %1222 to float
  store float %1223, ptr %42, !tbaa !2
  store i16 0, ptr %109, !tbaa !2
  store i16 15, ptr %41, !tbaa !2
  store i16 1, ptr %40, !tbaa !2
  br label %b84

b84:
  %1224 = load i16, ptr %40, !tbaa !2
  %1225 = icmp sge i16 %1224, 0
  br i1 %1225, label %b85, label %b86

b85:
  %1226 = load i16, ptr %109, !tbaa !2
  %1227 = load i16, ptr %41, !tbaa !2
  %1228 = icmp sle i16 %1226, %1227
  br i1 %1228, label %b87, label %b88

b86:
  %1229 = load i16, ptr %109, !tbaa !2
  %1230 = load i16, ptr %41, !tbaa !2
  %1231 = icmp sge i16 %1229, %1230
  br i1 %1231, label %b87, label %b88

b87:
  %1232 = load i16, ptr %109, !tbaa !2
  %1233 = load i16, ptr %106, !tbaa !2
  %1234 = mul i16 %1233, 16
  %1235 = add i16 %1232, %1234
  %1236 = mul i16 %1235, 2
  %1237 = getelementptr i8, ptr @"XO%", i16 2
  %1238 = load i16, ptr %1237, !tbaa !2
  %1239 = add i16 0, %1236
  %1240 = inttoptr i16 %1238 to ptr addrspace(2)
  %1241 = addrspacecast ptr addrspace(2) %1240 to ptr addrspace(1)
  %1242 = getelementptr i8, ptr addrspace(1) %1241, i16 %1239
  %1243 = load float, ptr %43, !tbaa !2
  %1244 = load i16, ptr %109, !tbaa !2
  %1245 = mul i16 %1244, 2
  %1246 = getelementptr i8, ptr @"XO%", i16 2
  %1247 = load i16, ptr %1246, !tbaa !2
  %1248 = add i16 0, %1245
  %1249 = inttoptr i16 %1247 to ptr addrspace(2)
  %1250 = addrspacecast ptr addrspace(2) %1249 to ptr addrspace(1)
  %1251 = getelementptr i8, ptr addrspace(1) %1250, i16 %1248
  %1252 = load i16, ptr addrspace(1) %1251, !tbaa !16
  store i16 %1252, ptr %39, !tbaa !2
  %1253 = load i16, ptr %39, !tbaa !2
  %1254 = sitofp i16 %1253 to float
  %1255 = fmul float %1243, %1254
  %1256 = load float, ptr %42, !tbaa !2
  %1257 = load i16, ptr %109, !tbaa !2
  %1258 = mul i16 %1257, 2
  %1259 = getelementptr i8, ptr @"YO%", i16 2
  %1260 = load i16, ptr %1259, !tbaa !2
  %1261 = add i16 0, %1258
  %1262 = inttoptr i16 %1260 to ptr addrspace(2)
  %1263 = addrspacecast ptr addrspace(2) %1262 to ptr addrspace(1)
  %1264 = getelementptr i8, ptr addrspace(1) %1263, i16 %1261
  %1265 = load i16, ptr addrspace(1) %1264, !tbaa !20
  store i16 %1265, ptr %38, !tbaa !2
  %1266 = load i16, ptr %38, !tbaa !2
  %1267 = sitofp i16 %1266 to float
  %1268 = fmul float %1256, %1267
  %1269 = fsub float %1255, %1268
  %1270 = call i16 @llvm.lrint.i16.f32(float %1269)
  store i16 %1270, ptr addrspace(1) %1242, !tbaa !16
  %1271 = load i16, ptr %109, !tbaa !2
  %1272 = load i16, ptr %106, !tbaa !2
  %1273 = mul i16 %1272, 16
  %1274 = add i16 %1271, %1273
  %1275 = mul i16 %1274, 2
  %1276 = getelementptr i8, ptr @"YO%", i16 2
  %1277 = load i16, ptr %1276, !tbaa !2
  %1278 = add i16 0, %1275
  %1279 = inttoptr i16 %1277 to ptr addrspace(2)
  %1280 = addrspacecast ptr addrspace(2) %1279 to ptr addrspace(1)
  %1281 = getelementptr i8, ptr addrspace(1) %1280, i16 %1278
  %1282 = load float, ptr %42, !tbaa !2
  %1283 = load i16, ptr %109, !tbaa !2
  %1284 = mul i16 %1283, 2
  %1285 = getelementptr i8, ptr @"XO%", i16 2
  %1286 = load i16, ptr %1285, !tbaa !2
  %1287 = add i16 0, %1284
  %1288 = inttoptr i16 %1286 to ptr addrspace(2)
  %1289 = addrspacecast ptr addrspace(2) %1288 to ptr addrspace(1)
  %1290 = getelementptr i8, ptr addrspace(1) %1289, i16 %1287
  %1291 = load i16, ptr addrspace(1) %1290, !tbaa !16
  store i16 %1291, ptr %37, !tbaa !2
  %1292 = load i16, ptr %37, !tbaa !2
  %1293 = sitofp i16 %1292 to float
  %1294 = fmul float %1282, %1293
  %1295 = load float, ptr %43, !tbaa !2
  %1296 = load i16, ptr %109, !tbaa !2
  %1297 = mul i16 %1296, 2
  %1298 = getelementptr i8, ptr @"YO%", i16 2
  %1299 = load i16, ptr %1298, !tbaa !2
  %1300 = add i16 0, %1297
  %1301 = inttoptr i16 %1299 to ptr addrspace(2)
  %1302 = addrspacecast ptr addrspace(2) %1301 to ptr addrspace(1)
  %1303 = getelementptr i8, ptr addrspace(1) %1302, i16 %1300
  %1304 = load i16, ptr addrspace(1) %1303, !tbaa !20
  store i16 %1304, ptr %36, !tbaa !2
  %1305 = load i16, ptr %36, !tbaa !2
  %1306 = sitofp i16 %1305 to float
  %1307 = fmul float %1295, %1306
  %1308 = fadd float %1294, %1307
  %1309 = call i16 @llvm.lrint.i16.f32(float %1308)
  store i16 %1309, ptr addrspace(1) %1281, !tbaa !20
  %1310 = load i16, ptr %109, !tbaa !2
  %1311 = load i16, ptr %106, !tbaa !2
  %1312 = mul i16 %1311, 16
  %1313 = add i16 %1310, %1312
  %1314 = mul i16 %1313, 2
  %1315 = getelementptr i8, ptr @"ZO%", i16 2
  %1316 = load i16, ptr %1315, !tbaa !2
  %1317 = add i16 0, %1314
  %1318 = inttoptr i16 %1316 to ptr addrspace(2)
  %1319 = addrspacecast ptr addrspace(2) %1318 to ptr addrspace(1)
  %1320 = getelementptr i8, ptr addrspace(1) %1319, i16 %1317
  %1321 = load i16, ptr %109, !tbaa !2
  %1322 = mul i16 %1321, 2
  %1323 = getelementptr i8, ptr @"ZO%", i16 2
  %1324 = load i16, ptr %1323, !tbaa !2
  %1325 = add i16 0, %1322
  %1326 = inttoptr i16 %1324 to ptr addrspace(2)
  %1327 = addrspacecast ptr addrspace(2) %1326 to ptr addrspace(1)
  %1328 = getelementptr i8, ptr addrspace(1) %1327, i16 %1325
  %1329 = load i16, ptr addrspace(1) %1328, !tbaa !18
  store i16 %1329, ptr addrspace(1) %1320, !tbaa !18
  %1330 = load i16, ptr %109, !tbaa !2
  %1331 = load i16, ptr %40, !tbaa !2
  %1332 = add nsw i16 %1330, %1331
  store i16 %1332, ptr %109, !tbaa !2
  br label %b84

b88:
  %1333 = load i16, ptr %106, !tbaa !2
  %1334 = load i16, ptr %44, !tbaa !2
  %1335 = add nsw i16 %1333, %1334
  store i16 %1335, ptr %106, !tbaa !2
  br label %b75

b89:
  %1336 = load i16, ptr %34, !tbaa !2
  %1337 = icmp sge i16 %1336, 0
  br i1 %1337, label %b90, label %b91

b90:
  %1338 = load i16, ptr %106, !tbaa !2
  %1339 = load i16, ptr %35, !tbaa !2
  %1340 = icmp sle i16 %1338, %1339
  br i1 %1340, label %b92, label %b93

b91:
  %1341 = load i16, ptr %106, !tbaa !2
  %1342 = load i16, ptr %35, !tbaa !2
  %1343 = icmp sge i16 %1341, %1342
  br i1 %1343, label %b92, label %b93

b92:
  %1344 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %1345 = load float, ptr %1344
  store i16 180, ptr %32, !tbaa !2
  %1346 = load i16, ptr %32, !tbaa !2
  %1347 = sitofp i16 %1346 to float
  %1348 = fmul float %1345, %1347
  %1349 = call float @llvm.rint.f32(float %1348)
  %1350 = fcmp olt float %1348, %1349
  %1351 = sext i1 %1350 to i16
  store i16 %1351, ptr %31, !tbaa !2
  %1352 = load i16, ptr %31, !tbaa !2
  %1353 = sitofp i16 %1352 to float
  %1354 = fadd float %1349, %1353
  store i16 90, ptr %30, !tbaa !2
  %1355 = load i16, ptr %30, !tbaa !2
  %1356 = sitofp i16 %1355 to float
  %1357 = fsub float %1354, %1356
  %1358 = load float, ptr %111, !tbaa !2
  %1359 = fdiv float %1357, %1358
  %1360 = fpround float %1359 to float
  store float %1360, ptr %33, !tbaa !2
  %1361 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %1362 = load float, ptr %1361
  store i16 180, ptr %29, !tbaa !2
  %1363 = load i16, ptr %29, !tbaa !2
  %1364 = sitofp i16 %1363 to float
  %1365 = fmul float %1362, %1364
  %1366 = call float @llvm.rint.f32(float %1365)
  %1367 = fcmp olt float %1365, %1366
  %1368 = sext i1 %1367 to i16
  store i16 %1368, ptr %28, !tbaa !2
  %1369 = load i16, ptr %28, !tbaa !2
  %1370 = sitofp i16 %1369 to float
  %1371 = fadd float %1366, %1370
  store i16 90, ptr %27, !tbaa !2
  %1372 = load i16, ptr %27, !tbaa !2
  %1373 = sitofp i16 %1372 to float
  %1374 = fsub float %1371, %1373
  %1375 = load float, ptr %111, !tbaa !2
  %1376 = fdiv float %1374, %1375
  %1377 = fpround float %1376 to float
  store float %1377, ptr %83, !tbaa !2
  %1378 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %1379 = load float, ptr %1378
  store i16 180, ptr %26, !tbaa !2
  %1380 = load i16, ptr %26, !tbaa !2
  %1381 = sitofp i16 %1380 to float
  %1382 = fmul float %1379, %1381
  %1383 = call float @llvm.rint.f32(float %1382)
  %1384 = fcmp olt float %1382, %1383
  %1385 = sext i1 %1384 to i16
  store i16 %1385, ptr %25, !tbaa !2
  %1386 = load i16, ptr %25, !tbaa !2
  %1387 = sitofp i16 %1386 to float
  %1388 = fadd float %1383, %1387
  store i16 90, ptr %24, !tbaa !2
  %1389 = load i16, ptr %24, !tbaa !2
  %1390 = sitofp i16 %1389 to float
  %1391 = fsub float %1388, %1390
  %1392 = load float, ptr %111, !tbaa !2
  %1393 = fdiv float %1391, %1392
  %1394 = fpround float %1393 to float
  store float %1394, ptr %47, !tbaa !2
  %1395 = load float, ptr %33, !tbaa !2
  %1396 = call float @llvm.fabs.f32(float %1395)
  %1397 = load float, ptr @$float86, !tbaa !2
  %1398 = fcmp ogt float %1396, %1397
  br i1 %1398, label %b94, label %b95

b93:
  br label %b2

b94:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b95:
  %1399 = call float @llvm.cos.f32(float %1395)
  %1400 = fpround float %1399 to float
  store float %1400, ptr %23, !tbaa !2
  %1401 = load float, ptr %83, !tbaa !2
  %1402 = call float @llvm.fabs.f32(float %1401)
  %1403 = load float, ptr @$float86, !tbaa !2
  %1404 = fcmp ogt float %1402, %1403
  br i1 %1404, label %b96, label %b97

b96:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b97:
  %1405 = call float @llvm.cos.f32(float %1401)
  %1406 = fpround float %1405 to float
  store float %1406, ptr %79, !tbaa !2
  %1407 = load float, ptr %47, !tbaa !2
  %1408 = call float @llvm.fabs.f32(float %1407)
  %1409 = load float, ptr @$float86, !tbaa !2
  %1410 = fcmp ogt float %1408, %1409
  br i1 %1410, label %b98, label %b99

b98:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b99:
  %1411 = call float @llvm.cos.f32(float %1407)
  %1412 = fpround float %1411 to float
  store float %1412, ptr %43, !tbaa !2
  %1413 = load float, ptr %33, !tbaa !2
  %1414 = call float @llvm.fabs.f32(float %1413)
  %1415 = load float, ptr @$float86, !tbaa !2
  %1416 = fcmp ogt float %1414, %1415
  br i1 %1416, label %b100, label %b101

b100:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b101:
  %1417 = call float @llvm.sin.f32(float %1413)
  %1418 = fpround float %1417 to float
  store float %1418, ptr %22, !tbaa !2
  %1419 = load float, ptr %83, !tbaa !2
  %1420 = call float @llvm.fabs.f32(float %1419)
  %1421 = load float, ptr @$float86, !tbaa !2
  %1422 = fcmp ogt float %1420, %1421
  br i1 %1422, label %b102, label %b103

b102:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b103:
  %1423 = call float @llvm.sin.f32(float %1419)
  %1424 = fpround float %1423 to float
  store float %1424, ptr %78, !tbaa !2
  %1425 = load float, ptr %47, !tbaa !2
  %1426 = call float @llvm.fabs.f32(float %1425)
  %1427 = load float, ptr @$float86, !tbaa !2
  %1428 = fcmp ogt float %1426, %1427
  br i1 %1428, label %b104, label %b105

b104:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b105:
  %1429 = call float @llvm.sin.f32(float %1425)
  %1430 = fpround float %1429 to float
  store float %1430, ptr %42, !tbaa !2
  %1431 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %1432 = load float, ptr %1431
  store i16 1024, ptr %21, !tbaa !2
  %1433 = load i16, ptr %21, !tbaa !2
  %1434 = sitofp i16 %1433 to float
  %1435 = fmul float %1432, %1434
  store i16 512, ptr %20, !tbaa !2
  %1436 = load i16, ptr %20, !tbaa !2
  %1437 = sitofp i16 %1436 to float
  %1438 = fsub float %1435, %1437
  %1439 = call float @llvm.rint.f32(float %1438)
  %1440 = fcmp olt float %1438, %1439
  %1441 = sext i1 %1440 to i16
  store i16 %1441, ptr %19, !tbaa !2
  %1442 = load i16, ptr %19, !tbaa !2
  %1443 = sitofp i16 %1442 to float
  %1444 = fadd float %1439, %1443
  %1445 = call i16 @llvm.lrint.i16.f32(float %1444)
  store i16 %1445, ptr %103, !tbaa !2
  %1446 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %1447 = load float, ptr %1446
  store i16 1024, ptr %18, !tbaa !2
  %1448 = load i16, ptr %18, !tbaa !2
  %1449 = sitofp i16 %1448 to float
  %1450 = fmul float %1447, %1449
  store i16 512, ptr %17, !tbaa !2
  %1451 = load i16, ptr %17, !tbaa !2
  %1452 = sitofp i16 %1451 to float
  %1453 = fsub float %1450, %1452
  %1454 = call float @llvm.rint.f32(float %1453)
  %1455 = fcmp olt float %1453, %1454
  %1456 = sext i1 %1455 to i16
  store i16 %1456, ptr %16, !tbaa !2
  %1457 = load i16, ptr %16, !tbaa !2
  %1458 = sitofp i16 %1457 to float
  %1459 = fadd float %1454, %1458
  %1460 = call i16 @llvm.lrint.i16.f32(float %1459)
  store i16 %1460, ptr %99, !tbaa !2
  %1461 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %1462 = load float, ptr %1461
  store i16 1024, ptr %15, !tbaa !2
  %1463 = load i16, ptr %15, !tbaa !2
  %1464 = sitofp i16 %1463 to float
  %1465 = fmul float %1462, %1464
  store i16 512, ptr %14, !tbaa !2
  %1466 = load i16, ptr %14, !tbaa !2
  %1467 = sitofp i16 %1466 to float
  %1468 = fsub float %1465, %1467
  %1469 = call float @llvm.rint.f32(float %1468)
  %1470 = fcmp olt float %1468, %1469
  %1471 = sext i1 %1470 to i16
  store i16 %1471, ptr %13, !tbaa !2
  %1472 = load i16, ptr %13, !tbaa !2
  %1473 = sitofp i16 %1472 to float
  %1474 = fadd float %1469, %1473
  %1475 = call i16 @llvm.lrint.i16.f32(float %1474)
  store i16 %1475, ptr %95, !tbaa !2
  store i16 0, ptr %109, !tbaa !2
  store i16 511, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b106

b106:
  %1476 = load i16, ptr %11, !tbaa !2
  %1477 = icmp sge i16 %1476, 0
  br i1 %1477, label %b107, label %b108

b107:
  %1478 = load i16, ptr %109, !tbaa !2
  %1479 = load i16, ptr %12, !tbaa !2
  %1480 = icmp sle i16 %1478, %1479
  br i1 %1480, label %b109, label %b110

b108:
  %1481 = load i16, ptr %109, !tbaa !2
  %1482 = load i16, ptr %12, !tbaa !2
  %1483 = icmp sge i16 %1481, %1482
  br i1 %1483, label %b109, label %b110

b109:
  %1484 = load i16, ptr %109, !tbaa !2
  %1485 = mul i16 %1484, 4
  %1486 = getelementptr i8, ptr @"X!", i16 2
  %1487 = load i16, ptr %1486, !tbaa !2
  %1488 = add i16 0, %1485
  %1489 = inttoptr i16 %1487 to ptr addrspace(2)
  %1490 = addrspacecast ptr addrspace(2) %1489 to ptr addrspace(1)
  %1491 = getelementptr i8, ptr addrspace(1) %1490, i16 %1488
  %1492 = load float, ptr %79, !tbaa !2
  %1493 = load i16, ptr %109, !tbaa !2
  %1494 = mul i16 %1493, 2
  %1495 = getelementptr i8, ptr @"XO%", i16 2
  %1496 = load i16, ptr %1495, !tbaa !2
  %1497 = add i16 0, %1494
  %1498 = inttoptr i16 %1496 to ptr addrspace(2)
  %1499 = addrspacecast ptr addrspace(2) %1498 to ptr addrspace(1)
  %1500 = getelementptr i8, ptr addrspace(1) %1499, i16 %1497
  %1501 = load i16, ptr addrspace(1) %1500, !tbaa !16
  store i16 %1501, ptr %10, !tbaa !2
  %1502 = load i16, ptr %10, !tbaa !2
  %1503 = sitofp i16 %1502 to float
  %1504 = fmul float %1492, %1503
  %1505 = load float, ptr %78, !tbaa !2
  %1506 = load i16, ptr %109, !tbaa !2
  %1507 = mul i16 %1506, 2
  %1508 = getelementptr i8, ptr @"ZO%", i16 2
  %1509 = load i16, ptr %1508, !tbaa !2
  %1510 = add i16 0, %1507
  %1511 = inttoptr i16 %1509 to ptr addrspace(2)
  %1512 = addrspacecast ptr addrspace(2) %1511 to ptr addrspace(1)
  %1513 = getelementptr i8, ptr addrspace(1) %1512, i16 %1510
  %1514 = load i16, ptr addrspace(1) %1513, !tbaa !18
  store i16 %1514, ptr %9, !tbaa !2
  %1515 = load i16, ptr %9, !tbaa !2
  %1516 = sitofp i16 %1515 to float
  %1517 = fmul float %1505, %1516
  %1518 = fsub float %1504, %1517
  %1519 = fpround float %1518 to float
  store float %1519, ptr addrspace(1) %1491, !tbaa !54
  %1520 = load i16, ptr %109, !tbaa !2
  %1521 = mul i16 %1520, 4
  %1522 = getelementptr i8, ptr @"Z!", i16 2
  %1523 = load i16, ptr %1522, !tbaa !2
  %1524 = add i16 0, %1521
  %1525 = inttoptr i16 %1523 to ptr addrspace(2)
  %1526 = addrspacecast ptr addrspace(2) %1525 to ptr addrspace(1)
  %1527 = getelementptr i8, ptr addrspace(1) %1526, i16 %1524
  %1528 = load float, ptr %78, !tbaa !2
  %1529 = load i16, ptr %109, !tbaa !2
  %1530 = mul i16 %1529, 2
  %1531 = getelementptr i8, ptr @"XO%", i16 2
  %1532 = load i16, ptr %1531, !tbaa !2
  %1533 = add i16 0, %1530
  %1534 = inttoptr i16 %1532 to ptr addrspace(2)
  %1535 = addrspacecast ptr addrspace(2) %1534 to ptr addrspace(1)
  %1536 = getelementptr i8, ptr addrspace(1) %1535, i16 %1533
  %1537 = load i16, ptr addrspace(1) %1536, !tbaa !16
  store i16 %1537, ptr %8, !tbaa !2
  %1538 = load i16, ptr %8, !tbaa !2
  %1539 = sitofp i16 %1538 to float
  %1540 = fmul float %1528, %1539
  %1541 = load float, ptr %79, !tbaa !2
  %1542 = load i16, ptr %109, !tbaa !2
  %1543 = mul i16 %1542, 2
  %1544 = getelementptr i8, ptr @"ZO%", i16 2
  %1545 = load i16, ptr %1544, !tbaa !2
  %1546 = add i16 0, %1543
  %1547 = inttoptr i16 %1545 to ptr addrspace(2)
  %1548 = addrspacecast ptr addrspace(2) %1547 to ptr addrspace(1)
  %1549 = getelementptr i8, ptr addrspace(1) %1548, i16 %1546
  %1550 = load i16, ptr addrspace(1) %1549, !tbaa !18
  store i16 %1550, ptr %7, !tbaa !2
  %1551 = load i16, ptr %7, !tbaa !2
  %1552 = sitofp i16 %1551 to float
  %1553 = fmul float %1541, %1552
  %1554 = fadd float %1540, %1553
  %1555 = fpround float %1554 to float
  store float %1555, ptr addrspace(1) %1527, !tbaa !56
  %1556 = load i16, ptr %109, !tbaa !2
  %1557 = mul i16 %1556, 4
  %1558 = getelementptr i8, ptr @"Y!", i16 2
  %1559 = load i16, ptr %1558, !tbaa !2
  %1560 = add i16 0, %1557
  %1561 = inttoptr i16 %1559 to ptr addrspace(2)
  %1562 = addrspacecast ptr addrspace(2) %1561 to ptr addrspace(1)
  %1563 = getelementptr i8, ptr addrspace(1) %1562, i16 %1560
  %1564 = load float, ptr %23, !tbaa !2
  %1565 = load i16, ptr %109, !tbaa !2
  %1566 = mul i16 %1565, 2
  %1567 = getelementptr i8, ptr @"YO%", i16 2
  %1568 = load i16, ptr %1567, !tbaa !2
  %1569 = add i16 0, %1566
  %1570 = inttoptr i16 %1568 to ptr addrspace(2)
  %1571 = addrspacecast ptr addrspace(2) %1570 to ptr addrspace(1)
  %1572 = getelementptr i8, ptr addrspace(1) %1571, i16 %1569
  %1573 = load i16, ptr addrspace(1) %1572, !tbaa !20
  store i16 %1573, ptr %6, !tbaa !2
  %1574 = load i16, ptr %6, !tbaa !2
  %1575 = sitofp i16 %1574 to float
  %1576 = fmul float %1564, %1575
  %1577 = load float, ptr %22, !tbaa !2
  %1578 = load i16, ptr %109, !tbaa !2
  %1579 = mul i16 %1578, 4
  %1580 = getelementptr i8, ptr @"Z!", i16 2
  %1581 = load i16, ptr %1580, !tbaa !2
  %1582 = add i16 0, %1579
  %1583 = inttoptr i16 %1581 to ptr addrspace(2)
  %1584 = addrspacecast ptr addrspace(2) %1583 to ptr addrspace(1)
  %1585 = getelementptr i8, ptr addrspace(1) %1584, i16 %1582
  %1586 = load float, ptr addrspace(1) %1585, !tbaa !56
  %1587 = fmul float %1577, %1586
  %1588 = fsub float %1576, %1587
  %1589 = fpround float %1588 to float
  store float %1589, ptr addrspace(1) %1563, !tbaa !58
  %1590 = load i16, ptr %109, !tbaa !2
  %1591 = mul i16 %1590, 4
  %1592 = getelementptr i8, ptr @"Z!", i16 2
  %1593 = load i16, ptr %1592, !tbaa !2
  %1594 = add i16 0, %1591
  %1595 = inttoptr i16 %1593 to ptr addrspace(2)
  %1596 = addrspacecast ptr addrspace(2) %1595 to ptr addrspace(1)
  %1597 = getelementptr i8, ptr addrspace(1) %1596, i16 %1594
  %1598 = load float, ptr %22, !tbaa !2
  %1599 = load i16, ptr %109, !tbaa !2
  %1600 = mul i16 %1599, 2
  %1601 = getelementptr i8, ptr @"YO%", i16 2
  %1602 = load i16, ptr %1601, !tbaa !2
  %1603 = add i16 0, %1600
  %1604 = inttoptr i16 %1602 to ptr addrspace(2)
  %1605 = addrspacecast ptr addrspace(2) %1604 to ptr addrspace(1)
  %1606 = getelementptr i8, ptr addrspace(1) %1605, i16 %1603
  %1607 = load i16, ptr addrspace(1) %1606, !tbaa !20
  store i16 %1607, ptr %5, !tbaa !2
  %1608 = load i16, ptr %5, !tbaa !2
  %1609 = sitofp i16 %1608 to float
  %1610 = fmul float %1598, %1609
  %1611 = load float, ptr %23, !tbaa !2
  %1612 = load i16, ptr %109, !tbaa !2
  %1613 = mul i16 %1612, 4
  %1614 = getelementptr i8, ptr @"Z!", i16 2
  %1615 = load i16, ptr %1614, !tbaa !2
  %1616 = add i16 0, %1613
  %1617 = inttoptr i16 %1615 to ptr addrspace(2)
  %1618 = addrspacecast ptr addrspace(2) %1617 to ptr addrspace(1)
  %1619 = getelementptr i8, ptr addrspace(1) %1618, i16 %1616
  %1620 = load float, ptr addrspace(1) %1619, !tbaa !56
  %1621 = fmul float %1611, %1620
  %1622 = fadd float %1610, %1621
  %1623 = fpround float %1622 to float
  store float %1623, ptr addrspace(1) %1597, !tbaa !56
  %1624 = load i16, ptr %109, !tbaa !2
  %1625 = mul i16 %1624, 4
  %1626 = getelementptr i8, ptr @"X!", i16 2
  %1627 = load i16, ptr %1626, !tbaa !2
  %1628 = add i16 0, %1625
  %1629 = inttoptr i16 %1627 to ptr addrspace(2)
  %1630 = addrspacecast ptr addrspace(2) %1629 to ptr addrspace(1)
  %1631 = getelementptr i8, ptr addrspace(1) %1630, i16 %1628
  %1632 = load float, ptr addrspace(1) %1631, !tbaa !54
  store float %1632, ptr %4, !tbaa !2
  %1633 = load i16, ptr %109, !tbaa !2
  %1634 = mul i16 %1633, 4
  %1635 = getelementptr i8, ptr @"X!", i16 2
  %1636 = load i16, ptr %1635, !tbaa !2
  %1637 = add i16 0, %1634
  %1638 = inttoptr i16 %1636 to ptr addrspace(2)
  %1639 = addrspacecast ptr addrspace(2) %1638 to ptr addrspace(1)
  %1640 = getelementptr i8, ptr addrspace(1) %1639, i16 %1637
  %1641 = load float, ptr %43, !tbaa !2
  %1642 = load float, ptr %4, !tbaa !2
  %1643 = fmul float %1641, %1642
  %1644 = load float, ptr %42, !tbaa !2
  %1645 = load i16, ptr %109, !tbaa !2
  %1646 = mul i16 %1645, 4
  %1647 = getelementptr i8, ptr @"Y!", i16 2
  %1648 = load i16, ptr %1647, !tbaa !2
  %1649 = add i16 0, %1646
  %1650 = inttoptr i16 %1648 to ptr addrspace(2)
  %1651 = addrspacecast ptr addrspace(2) %1650 to ptr addrspace(1)
  %1652 = getelementptr i8, ptr addrspace(1) %1651, i16 %1649
  %1653 = load float, ptr addrspace(1) %1652, !tbaa !58
  %1654 = fmul float %1644, %1653
  %1655 = fsub float %1643, %1654
  %1656 = fpround float %1655 to float
  store float %1656, ptr addrspace(1) %1640, !tbaa !54
  %1657 = load i16, ptr %109, !tbaa !2
  %1658 = mul i16 %1657, 4
  %1659 = getelementptr i8, ptr @"Y!", i16 2
  %1660 = load i16, ptr %1659, !tbaa !2
  %1661 = add i16 0, %1658
  %1662 = inttoptr i16 %1660 to ptr addrspace(2)
  %1663 = addrspacecast ptr addrspace(2) %1662 to ptr addrspace(1)
  %1664 = getelementptr i8, ptr addrspace(1) %1663, i16 %1661
  %1665 = load float, ptr %42, !tbaa !2
  %1666 = load float, ptr %4, !tbaa !2
  %1667 = fmul float %1665, %1666
  %1668 = load float, ptr %43, !tbaa !2
  %1669 = load i16, ptr %109, !tbaa !2
  %1670 = mul i16 %1669, 4
  %1671 = getelementptr i8, ptr @"Y!", i16 2
  %1672 = load i16, ptr %1671, !tbaa !2
  %1673 = add i16 0, %1670
  %1674 = inttoptr i16 %1672 to ptr addrspace(2)
  %1675 = addrspacecast ptr addrspace(2) %1674 to ptr addrspace(1)
  %1676 = getelementptr i8, ptr addrspace(1) %1675, i16 %1673
  %1677 = load float, ptr addrspace(1) %1676, !tbaa !58
  %1678 = fmul float %1668, %1677
  %1679 = fadd float %1667, %1678
  %1680 = fpround float %1679 to float
  store float %1680, ptr addrspace(1) %1664, !tbaa !58
  %1681 = load i16, ptr %109, !tbaa !2
  %1682 = load i16, ptr %106, !tbaa !2
  %1683 = mul i16 %1682, 512
  %1684 = add i16 %1681, %1683
  %1685 = mul i16 %1684, 2
  %1686 = getelementptr i8, ptr @"XO%", i16 2
  %1687 = load i16, ptr %1686, !tbaa !2
  %1688 = add i16 0, %1685
  %1689 = inttoptr i16 %1687 to ptr addrspace(2)
  %1690 = addrspacecast ptr addrspace(2) %1689 to ptr addrspace(1)
  %1691 = getelementptr i8, ptr addrspace(1) %1690, i16 %1688
  %1692 = load i16, ptr %109, !tbaa !2
  %1693 = mul i16 %1692, 4
  %1694 = getelementptr i8, ptr @"X!", i16 2
  %1695 = load i16, ptr %1694, !tbaa !2
  %1696 = add i16 0, %1693
  %1697 = inttoptr i16 %1695 to ptr addrspace(2)
  %1698 = addrspacecast ptr addrspace(2) %1697 to ptr addrspace(1)
  %1699 = getelementptr i8, ptr addrspace(1) %1698, i16 %1696
  %1700 = load float, ptr addrspace(1) %1699, !tbaa !54
  %1701 = load i16, ptr %103, !tbaa !2
  store i16 %1701, ptr %3, !tbaa !2
  %1702 = load i16, ptr %3, !tbaa !2
  %1703 = sitofp i16 %1702 to float
  %1704 = fadd float %1700, %1703
  %1705 = call i16 @llvm.lrint.i16.f32(float %1704)
  store i16 %1705, ptr addrspace(1) %1691, !tbaa !16
  %1706 = load i16, ptr %109, !tbaa !2
  %1707 = load i16, ptr %106, !tbaa !2
  %1708 = mul i16 %1707, 512
  %1709 = add i16 %1706, %1708
  %1710 = mul i16 %1709, 2
  %1711 = getelementptr i8, ptr @"YO%", i16 2
  %1712 = load i16, ptr %1711, !tbaa !2
  %1713 = add i16 0, %1710
  %1714 = inttoptr i16 %1712 to ptr addrspace(2)
  %1715 = addrspacecast ptr addrspace(2) %1714 to ptr addrspace(1)
  %1716 = getelementptr i8, ptr addrspace(1) %1715, i16 %1713
  %1717 = load i16, ptr %109, !tbaa !2
  %1718 = mul i16 %1717, 4
  %1719 = getelementptr i8, ptr @"Y!", i16 2
  %1720 = load i16, ptr %1719, !tbaa !2
  %1721 = add i16 0, %1718
  %1722 = inttoptr i16 %1720 to ptr addrspace(2)
  %1723 = addrspacecast ptr addrspace(2) %1722 to ptr addrspace(1)
  %1724 = getelementptr i8, ptr addrspace(1) %1723, i16 %1721
  %1725 = load float, ptr addrspace(1) %1724, !tbaa !58
  %1726 = load i16, ptr %99, !tbaa !2
  store i16 %1726, ptr %2, !tbaa !2
  %1727 = load i16, ptr %2, !tbaa !2
  %1728 = sitofp i16 %1727 to float
  %1729 = fadd float %1725, %1728
  %1730 = call i16 @llvm.lrint.i16.f32(float %1729)
  store i16 %1730, ptr addrspace(1) %1716, !tbaa !20
  %1731 = load i16, ptr %109, !tbaa !2
  %1732 = load i16, ptr %106, !tbaa !2
  %1733 = mul i16 %1732, 512
  %1734 = add i16 %1731, %1733
  %1735 = mul i16 %1734, 2
  %1736 = getelementptr i8, ptr @"ZO%", i16 2
  %1737 = load i16, ptr %1736, !tbaa !2
  %1738 = add i16 0, %1735
  %1739 = inttoptr i16 %1737 to ptr addrspace(2)
  %1740 = addrspacecast ptr addrspace(2) %1739 to ptr addrspace(1)
  %1741 = getelementptr i8, ptr addrspace(1) %1740, i16 %1738
  %1742 = load i16, ptr %109, !tbaa !2
  %1743 = mul i16 %1742, 4
  %1744 = getelementptr i8, ptr @"Z!", i16 2
  %1745 = load i16, ptr %1744, !tbaa !2
  %1746 = add i16 0, %1743
  %1747 = inttoptr i16 %1745 to ptr addrspace(2)
  %1748 = addrspacecast ptr addrspace(2) %1747 to ptr addrspace(1)
  %1749 = getelementptr i8, ptr addrspace(1) %1748, i16 %1746
  %1750 = load float, ptr addrspace(1) %1749, !tbaa !56
  %1751 = load i16, ptr %95, !tbaa !2
  store i16 %1751, ptr %1, !tbaa !2
  %1752 = load i16, ptr %1, !tbaa !2
  %1753 = sitofp i16 %1752 to float
  %1754 = fadd float %1750, %1753
  %1755 = call i16 @llvm.lrint.i16.f32(float %1754)
  store i16 %1755, ptr addrspace(1) %1741, !tbaa !18
  %1756 = load i16, ptr %109, !tbaa !2
  %1757 = load i16, ptr %11, !tbaa !2
  %1758 = add nsw i16 %1756, %1757
  store i16 %1758, ptr %109, !tbaa !2
  br label %b106

b110:
  %1759 = load i16, ptr %106, !tbaa !2
  %1760 = load i16, ptr %34, !tbaa !2
  %1761 = add nsw i16 %1759, %1760
  store i16 %1761, ptr %106, !tbaa !2
  br label %b89
}

define cc 1000 void @CROSFADE(ptr dereferenceable(18) %0, ptr dereferenceable(18) %1, ptr dereferenceable(18) %2, ptr dereferenceable(18) %3, ptr dereferenceable(18) %4, ptr dereferenceable(18) %5) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  store i16 0, ptr %11, !tbaa !2
  store i16 63, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  br label %b3

b2:
  ret void

b3:
  %12 = load i16, ptr %9, !tbaa !2
  %13 = icmp sge i16 %12, 0
  br i1 %13, label %b4, label %b5

b4:
  %14 = load i16, ptr %11, !tbaa !2
  %15 = load i16, ptr %10, !tbaa !2
  %16 = icmp sle i16 %14, %15
  br i1 %16, label %b6, label %b7

b5:
  %17 = load i16, ptr %11, !tbaa !2
  %18 = load i16, ptr %10, !tbaa !2
  %19 = icmp sge i16 %17, %18
  br i1 %19, label %b6, label %b7

b6:
  %20 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %20, ptr @"BENCHFRAME&", !tbaa !2
  %21 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %21, ptr @"BENCHFRAME&", !tbaa !2
  %22 = load i16, ptr @"XIT%", !tbaa !2
  %23 = icmp eq i16 %22, 0
  br i1 %23, label %b8, label %b9

b7:
  br label %b2

b8:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b10

b9:
  br label %b10

b10:
  %24 = load i16, ptr @"XIT%", !tbaa !2
  %25 = icmp eq i16 %24, 1
  br i1 %25, label %b11, label %b12

b11:
  br label %b2

b12:
  br label %b13

b13:
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %8, !tbaa !2
  store i16 255, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b15

b15:
  %26 = load i16, ptr %6, !tbaa !2
  %27 = icmp sge i16 %26, 0
  br i1 %27, label %b16, label %b17

b16:
  %28 = load i16, ptr %8, !tbaa !2
  %29 = load i16, ptr %7, !tbaa !2
  %30 = icmp sle i16 %28, %29
  br i1 %30, label %b18, label %b19

b17:
  %31 = load i16, ptr %8, !tbaa !2
  %32 = load i16, ptr %7, !tbaa !2
  %33 = icmp sge i16 %31, %32
  br i1 %33, label %b18, label %b19

b18:
  %34 = load i16, ptr %8, !tbaa !2
  %35 = mul i16 %34, 2
  %36 = getelementptr i8, ptr %0, i16 2
  %37 = load i16, ptr %36
  %38 = getelementptr i8, ptr %0, i16 10
  %39 = load i16, ptr %38
  %40 = add i16 %39, %35
  %41 = inttoptr i16 %37 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 %40
  %44 = load i16, ptr addrspace(1) %43
  %45 = load i16, ptr %8, !tbaa !2
  %46 = mul i16 %45, 2
  %47 = getelementptr i8, ptr %1, i16 2
  %48 = load i16, ptr %47
  %49 = getelementptr i8, ptr %1, i16 10
  %50 = load i16, ptr %49
  %51 = add i16 %50, %46
  %52 = inttoptr i16 %48 to ptr addrspace(2)
  %53 = addrspacecast ptr addrspace(2) %52 to ptr addrspace(1)
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 %51
  %55 = load i16, ptr addrspace(1) %54
  %56 = icmp slt i16 %44, %55
  br i1 %56, label %b20, label %b21

b19:
  %57 = load i16, ptr %11, !tbaa !2
  %58 = load i16, ptr %9, !tbaa !2
  %59 = add nsw i16 %57, %58
  store i16 %59, ptr %11, !tbaa !2
  br label %b3

b20:
  %60 = load i16, ptr %8, !tbaa !2
  %61 = mul i16 %60, 2
  %62 = getelementptr i8, ptr %0, i16 2
  %63 = load i16, ptr %62
  %64 = getelementptr i8, ptr %0, i16 10
  %65 = load i16, ptr %64
  %66 = add i16 %65, %61
  %67 = inttoptr i16 %63 to ptr addrspace(2)
  %68 = addrspacecast ptr addrspace(2) %67 to ptr addrspace(1)
  %69 = getelementptr i8, ptr addrspace(1) %68, i16 %66
  %70 = load i16, ptr %8, !tbaa !2
  %71 = mul i16 %70, 2
  %72 = add i16 %65, %71
  %73 = inttoptr i16 %63 to ptr addrspace(2)
  %74 = addrspacecast ptr addrspace(2) %73 to ptr addrspace(1)
  %75 = getelementptr i8, ptr addrspace(1) %74, i16 %72
  %76 = load i16, ptr addrspace(1) %75
  %77 = add i16 %76, 1
  store i16 %77, ptr addrspace(1) %69
  br label %b22

b21:
  %78 = load i16, ptr %8, !tbaa !2
  %79 = mul i16 %78, 2
  %80 = getelementptr i8, ptr %0, i16 2
  %81 = load i16, ptr %80
  %82 = getelementptr i8, ptr %0, i16 10
  %83 = load i16, ptr %82
  %84 = add i16 %83, %79
  %85 = inttoptr i16 %81 to ptr addrspace(2)
  %86 = addrspacecast ptr addrspace(2) %85 to ptr addrspace(1)
  %87 = getelementptr i8, ptr addrspace(1) %86, i16 %84
  %88 = load i16, ptr addrspace(1) %87
  %89 = load i16, ptr %8, !tbaa !2
  %90 = mul i16 %89, 2
  %91 = getelementptr i8, ptr %1, i16 2
  %92 = load i16, ptr %91
  %93 = getelementptr i8, ptr %1, i16 10
  %94 = load i16, ptr %93
  %95 = add i16 %94, %90
  %96 = inttoptr i16 %92 to ptr addrspace(2)
  %97 = addrspacecast ptr addrspace(2) %96 to ptr addrspace(1)
  %98 = getelementptr i8, ptr addrspace(1) %97, i16 %95
  %99 = load i16, ptr addrspace(1) %98
  %100 = icmp sgt i16 %88, %99
  br i1 %100, label %b23, label %b24

b22:
  %101 = load i16, ptr %8, !tbaa !2
  %102 = mul i16 %101, 2
  %103 = getelementptr i8, ptr %2, i16 2
  %104 = load i16, ptr %103
  %105 = getelementptr i8, ptr %2, i16 10
  %106 = load i16, ptr %105
  %107 = add i16 %106, %102
  %108 = inttoptr i16 %104 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 %107
  %111 = load i16, ptr addrspace(1) %110
  %112 = load i16, ptr %8, !tbaa !2
  %113 = mul i16 %112, 2
  %114 = getelementptr i8, ptr %3, i16 2
  %115 = load i16, ptr %114
  %116 = getelementptr i8, ptr %3, i16 10
  %117 = load i16, ptr %116
  %118 = add i16 %117, %113
  %119 = inttoptr i16 %115 to ptr addrspace(2)
  %120 = addrspacecast ptr addrspace(2) %119 to ptr addrspace(1)
  %121 = getelementptr i8, ptr addrspace(1) %120, i16 %118
  %122 = load i16, ptr addrspace(1) %121
  %123 = icmp slt i16 %111, %122
  br i1 %123, label %b26, label %b27

b23:
  %124 = load i16, ptr %8, !tbaa !2
  %125 = mul i16 %124, 2
  %126 = getelementptr i8, ptr %0, i16 2
  %127 = load i16, ptr %126
  %128 = getelementptr i8, ptr %0, i16 10
  %129 = load i16, ptr %128
  %130 = add i16 %129, %125
  %131 = inttoptr i16 %127 to ptr addrspace(2)
  %132 = addrspacecast ptr addrspace(2) %131 to ptr addrspace(1)
  %133 = getelementptr i8, ptr addrspace(1) %132, i16 %130
  %134 = load i16, ptr %8, !tbaa !2
  %135 = mul i16 %134, 2
  %136 = add i16 %129, %135
  %137 = inttoptr i16 %127 to ptr addrspace(2)
  %138 = addrspacecast ptr addrspace(2) %137 to ptr addrspace(1)
  %139 = getelementptr i8, ptr addrspace(1) %138, i16 %136
  %140 = load i16, ptr addrspace(1) %139
  %141 = sub i16 %140, 1
  store i16 %141, ptr addrspace(1) %133
  br label %b25

b24:
  br label %b25

b25:
  br label %b22

b26:
  %142 = load i16, ptr %8, !tbaa !2
  %143 = mul i16 %142, 2
  %144 = getelementptr i8, ptr %2, i16 2
  %145 = load i16, ptr %144
  %146 = getelementptr i8, ptr %2, i16 10
  %147 = load i16, ptr %146
  %148 = add i16 %147, %143
  %149 = inttoptr i16 %145 to ptr addrspace(2)
  %150 = addrspacecast ptr addrspace(2) %149 to ptr addrspace(1)
  %151 = getelementptr i8, ptr addrspace(1) %150, i16 %148
  %152 = load i16, ptr %8, !tbaa !2
  %153 = mul i16 %152, 2
  %154 = add i16 %147, %153
  %155 = inttoptr i16 %145 to ptr addrspace(2)
  %156 = addrspacecast ptr addrspace(2) %155 to ptr addrspace(1)
  %157 = getelementptr i8, ptr addrspace(1) %156, i16 %154
  %158 = load i16, ptr addrspace(1) %157
  %159 = add i16 %158, 1
  store i16 %159, ptr addrspace(1) %151
  br label %b28

b27:
  %160 = load i16, ptr %8, !tbaa !2
  %161 = mul i16 %160, 2
  %162 = getelementptr i8, ptr %2, i16 2
  %163 = load i16, ptr %162
  %164 = getelementptr i8, ptr %2, i16 10
  %165 = load i16, ptr %164
  %166 = add i16 %165, %161
  %167 = inttoptr i16 %163 to ptr addrspace(2)
  %168 = addrspacecast ptr addrspace(2) %167 to ptr addrspace(1)
  %169 = getelementptr i8, ptr addrspace(1) %168, i16 %166
  %170 = load i16, ptr addrspace(1) %169
  %171 = load i16, ptr %8, !tbaa !2
  %172 = mul i16 %171, 2
  %173 = getelementptr i8, ptr %3, i16 2
  %174 = load i16, ptr %173
  %175 = getelementptr i8, ptr %3, i16 10
  %176 = load i16, ptr %175
  %177 = add i16 %176, %172
  %178 = inttoptr i16 %174 to ptr addrspace(2)
  %179 = addrspacecast ptr addrspace(2) %178 to ptr addrspace(1)
  %180 = getelementptr i8, ptr addrspace(1) %179, i16 %177
  %181 = load i16, ptr addrspace(1) %180
  %182 = icmp sgt i16 %170, %181
  br i1 %182, label %b29, label %b30

b28:
  %183 = load i16, ptr %8, !tbaa !2
  %184 = mul i16 %183, 2
  %185 = getelementptr i8, ptr %4, i16 2
  %186 = load i16, ptr %185
  %187 = getelementptr i8, ptr %4, i16 10
  %188 = load i16, ptr %187
  %189 = add i16 %188, %184
  %190 = inttoptr i16 %186 to ptr addrspace(2)
  %191 = addrspacecast ptr addrspace(2) %190 to ptr addrspace(1)
  %192 = getelementptr i8, ptr addrspace(1) %191, i16 %189
  %193 = load i16, ptr addrspace(1) %192
  %194 = load i16, ptr %8, !tbaa !2
  %195 = mul i16 %194, 2
  %196 = getelementptr i8, ptr %5, i16 2
  %197 = load i16, ptr %196
  %198 = getelementptr i8, ptr %5, i16 10
  %199 = load i16, ptr %198
  %200 = add i16 %199, %195
  %201 = inttoptr i16 %197 to ptr addrspace(2)
  %202 = addrspacecast ptr addrspace(2) %201 to ptr addrspace(1)
  %203 = getelementptr i8, ptr addrspace(1) %202, i16 %200
  %204 = load i16, ptr addrspace(1) %203
  %205 = icmp slt i16 %193, %204
  br i1 %205, label %b32, label %b33

b29:
  %206 = load i16, ptr %8, !tbaa !2
  %207 = mul i16 %206, 2
  %208 = getelementptr i8, ptr %2, i16 2
  %209 = load i16, ptr %208
  %210 = getelementptr i8, ptr %2, i16 10
  %211 = load i16, ptr %210
  %212 = add i16 %211, %207
  %213 = inttoptr i16 %209 to ptr addrspace(2)
  %214 = addrspacecast ptr addrspace(2) %213 to ptr addrspace(1)
  %215 = getelementptr i8, ptr addrspace(1) %214, i16 %212
  %216 = load i16, ptr %8, !tbaa !2
  %217 = mul i16 %216, 2
  %218 = add i16 %211, %217
  %219 = inttoptr i16 %209 to ptr addrspace(2)
  %220 = addrspacecast ptr addrspace(2) %219 to ptr addrspace(1)
  %221 = getelementptr i8, ptr addrspace(1) %220, i16 %218
  %222 = load i16, ptr addrspace(1) %221
  %223 = sub i16 %222, 1
  store i16 %223, ptr addrspace(1) %215
  br label %b31

b30:
  br label %b31

b31:
  br label %b28

b32:
  %224 = load i16, ptr %8, !tbaa !2
  %225 = mul i16 %224, 2
  %226 = getelementptr i8, ptr %4, i16 2
  %227 = load i16, ptr %226
  %228 = getelementptr i8, ptr %4, i16 10
  %229 = load i16, ptr %228
  %230 = add i16 %229, %225
  %231 = inttoptr i16 %227 to ptr addrspace(2)
  %232 = addrspacecast ptr addrspace(2) %231 to ptr addrspace(1)
  %233 = getelementptr i8, ptr addrspace(1) %232, i16 %230
  %234 = load i16, ptr %8, !tbaa !2
  %235 = mul i16 %234, 2
  %236 = add i16 %229, %235
  %237 = inttoptr i16 %227 to ptr addrspace(2)
  %238 = addrspacecast ptr addrspace(2) %237 to ptr addrspace(1)
  %239 = getelementptr i8, ptr addrspace(1) %238, i16 %236
  %240 = load i16, ptr addrspace(1) %239
  %241 = add i16 %240, 1
  store i16 %241, ptr addrspace(1) %233
  br label %b34

b33:
  %242 = load i16, ptr %8, !tbaa !2
  %243 = mul i16 %242, 2
  %244 = getelementptr i8, ptr %4, i16 2
  %245 = load i16, ptr %244
  %246 = getelementptr i8, ptr %4, i16 10
  %247 = load i16, ptr %246
  %248 = add i16 %247, %243
  %249 = inttoptr i16 %245 to ptr addrspace(2)
  %250 = addrspacecast ptr addrspace(2) %249 to ptr addrspace(1)
  %251 = getelementptr i8, ptr addrspace(1) %250, i16 %248
  %252 = load i16, ptr addrspace(1) %251
  %253 = load i16, ptr %8, !tbaa !2
  %254 = mul i16 %253, 2
  %255 = getelementptr i8, ptr %5, i16 2
  %256 = load i16, ptr %255
  %257 = getelementptr i8, ptr %5, i16 10
  %258 = load i16, ptr %257
  %259 = add i16 %258, %254
  %260 = inttoptr i16 %256 to ptr addrspace(2)
  %261 = addrspacecast ptr addrspace(2) %260 to ptr addrspace(1)
  %262 = getelementptr i8, ptr addrspace(1) %261, i16 %259
  %263 = load i16, ptr addrspace(1) %262
  %264 = icmp sgt i16 %252, %263
  br i1 %264, label %b35, label %b36

b34:
  %265 = load i16, ptr %8, !tbaa !2
  %266 = mul i16 %265, 2
  %267 = getelementptr i8, ptr %0, i16 2
  %268 = load i16, ptr %267
  %269 = getelementptr i8, ptr %0, i16 10
  %270 = load i16, ptr %269
  %271 = add i16 %270, %266
  %272 = inttoptr i16 %268 to ptr addrspace(2)
  %273 = addrspacecast ptr addrspace(2) %272 to ptr addrspace(1)
  %274 = getelementptr i8, ptr addrspace(1) %273, i16 %271
  %275 = load i16, ptr addrspace(1) %274
  %276 = trunc i16 %275 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %276)
  %277 = load i16, ptr %8, !tbaa !2
  %278 = mul i16 %277, 2
  %279 = getelementptr i8, ptr %2, i16 2
  %280 = load i16, ptr %279
  %281 = getelementptr i8, ptr %2, i16 10
  %282 = load i16, ptr %281
  %283 = add i16 %282, %278
  %284 = inttoptr i16 %280 to ptr addrspace(2)
  %285 = addrspacecast ptr addrspace(2) %284 to ptr addrspace(1)
  %286 = getelementptr i8, ptr addrspace(1) %285, i16 %283
  %287 = load i16, ptr addrspace(1) %286
  %288 = trunc i16 %287 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %288)
  %289 = load i16, ptr %8, !tbaa !2
  %290 = mul i16 %289, 2
  %291 = getelementptr i8, ptr %4, i16 2
  %292 = load i16, ptr %291
  %293 = getelementptr i8, ptr %4, i16 10
  %294 = load i16, ptr %293
  %295 = add i16 %294, %290
  %296 = inttoptr i16 %292 to ptr addrspace(2)
  %297 = addrspacecast ptr addrspace(2) %296 to ptr addrspace(1)
  %298 = getelementptr i8, ptr addrspace(1) %297, i16 %295
  %299 = load i16, ptr addrspace(1) %298
  %300 = trunc i16 %299 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %300)
  %301 = load i16, ptr %8, !tbaa !2
  %302 = load i16, ptr %6, !tbaa !2
  %303 = add nsw i16 %301, %302
  store i16 %303, ptr %8, !tbaa !2
  br label %b15

b35:
  %304 = load i16, ptr %8, !tbaa !2
  %305 = mul i16 %304, 2
  %306 = getelementptr i8, ptr %4, i16 2
  %307 = load i16, ptr %306
  %308 = getelementptr i8, ptr %4, i16 10
  %309 = load i16, ptr %308
  %310 = add i16 %309, %305
  %311 = inttoptr i16 %307 to ptr addrspace(2)
  %312 = addrspacecast ptr addrspace(2) %311 to ptr addrspace(1)
  %313 = getelementptr i8, ptr addrspace(1) %312, i16 %310
  %314 = load i16, ptr %8, !tbaa !2
  %315 = mul i16 %314, 2
  %316 = add i16 %309, %315
  %317 = inttoptr i16 %307 to ptr addrspace(2)
  %318 = addrspacecast ptr addrspace(2) %317 to ptr addrspace(1)
  %319 = getelementptr i8, ptr addrspace(1) %318, i16 %316
  %320 = load i16, ptr addrspace(1) %319
  %321 = sub i16 %320, 1
  store i16 %321, ptr addrspace(1) %313
  br label %b37

b36:
  br label %b37

b37:
  br label %b34
}

define cc 1000 void @CYCLEBLOBS() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca i16
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca i16
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca i16
  %79 = alloca i16
  %80 = alloca i16
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca i16
  %84 = alloca i16
  %85 = alloca i16
  %86 = alloca i16
  %87 = alloca i32
  %88 = alloca i32
  %89 = alloca float
  %90 = alloca float
  %91 = alloca float
  %92 = alloca float
  %93 = alloca i16
  %94 = alloca float
  %95 = alloca i16
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  %103 = alloca i16
  %104 = alloca i16
  %105 = alloca i16
  %106 = alloca i16
  store i16 0, ptr %96, !tbaa !2
  store i16 0, ptr %63, !tbaa !2
  store i16 0, ptr %50, !tbaa !2
  store i16 7, ptr %106, !tbaa !2
  store i16 0, ptr %105, !tbaa !2
  store i16 127, ptr %104, !tbaa !2
  store i16 1, ptr %103, !tbaa !2
  br label %b3

b2:
  ret void

b3:
  %107 = load i16, ptr %103, !tbaa !2
  %108 = icmp sge i16 %107, 0
  br i1 %108, label %b4, label %b5

b4:
  %109 = load i16, ptr %105, !tbaa !2
  %110 = load i16, ptr %104, !tbaa !2
  %111 = icmp sle i16 %109, %110
  br i1 %111, label %b6, label %b7

b5:
  %112 = load i16, ptr %105, !tbaa !2
  %113 = load i16, ptr %104, !tbaa !2
  %114 = icmp sge i16 %112, %113
  br i1 %114, label %b6, label %b7

b6:
  %115 = load i16, ptr %105, !tbaa !2
  %116 = trunc i16 %115 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %116)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %117 = load i16, ptr %105, !tbaa !2
  %118 = load i16, ptr %103, !tbaa !2
  %119 = add nsw i16 %117, %118
  store i16 %119, ptr %105, !tbaa !2
  br label %b3

b7:
  store i16 0, ptr %102, !tbaa !2
  store i16 99, ptr %101, !tbaa !2
  store i16 1, ptr %100, !tbaa !2
  br label %b8

b8:
  %120 = load i16, ptr %100, !tbaa !2
  %121 = icmp sge i16 %120, 0
  br i1 %121, label %b9, label %b10

b9:
  %122 = load i16, ptr %102, !tbaa !2
  %123 = load i16, ptr %101, !tbaa !2
  %124 = icmp sle i16 %122, %123
  br i1 %124, label %b11, label %b12

b10:
  %125 = load i16, ptr %102, !tbaa !2
  %126 = load i16, ptr %101, !tbaa !2
  %127 = icmp sge i16 %125, %126
  br i1 %127, label %b11, label %b12

b11:
  store i16 0, ptr %99, !tbaa !2
  store i16 159, ptr %98, !tbaa !2
  store i16 1, ptr %97, !tbaa !2
  br label %b13

b12:
  store i16 0, ptr %105, !tbaa !2
  store i16 319, ptr %86, !tbaa !2
  store i16 1, ptr %85, !tbaa !2
  br label %b38

b13:
  %128 = load i16, ptr %97, !tbaa !2
  %129 = icmp sge i16 %128, 0
  br i1 %129, label %b14, label %b15

b14:
  %130 = load i16, ptr %99, !tbaa !2
  %131 = load i16, ptr %98, !tbaa !2
  %132 = icmp sle i16 %130, %131
  br i1 %132, label %b16, label %b17

b15:
  %133 = load i16, ptr %99, !tbaa !2
  %134 = load i16, ptr %98, !tbaa !2
  %135 = icmp sge i16 %133, %134
  br i1 %135, label %b16, label %b17

b16:
  %136 = load i16, ptr %96, !tbaa !2
  %137 = load i16, ptr %99, !tbaa !2
  %138 = sub i16 160, %137
  store i16 %138, ptr %95, !tbaa !2
  %139 = load i16, ptr %95, !tbaa !2
  %140 = sitofp i16 %139 to float
  %141 = fpround float %140 to float
  store float %141, ptr %94, !tbaa !2
  %142 = load float, ptr %94, !tbaa !2
  %143 = load float, ptr %94, !tbaa !2
  %144 = fmul float %142, %143
  %145 = load i16, ptr %102, !tbaa !2
  %146 = sub i16 100, %145
  store i16 %146, ptr %93, !tbaa !2
  %147 = load i16, ptr %93, !tbaa !2
  %148 = sitofp i16 %147 to float
  %149 = fpround float %148 to float
  store float %149, ptr %92, !tbaa !2
  %150 = load float, ptr %92, !tbaa !2
  %151 = load float, ptr %92, !tbaa !2
  %152 = fmul float %150, %151
  %153 = fadd float %144, %152
  %154 = call float @llvm.sqrt.f32(float %153)
  %155 = load float, ptr @$float131, !tbaa !2
  %156 = fpround float %154 to float
  store float %156, ptr %91, !tbaa !2
  store float %155, ptr %90, !tbaa !2
  %157 = load float, ptr %91, !tbaa !2
  %158 = load float, ptr @$float218, !tbaa !2
  %159 = fcmp ogt float %157, %158
  br i1 %159, label %b18, label %b19

b17:
  %160 = load i16, ptr %102, !tbaa !2
  %161 = load i16, ptr %100, !tbaa !2
  %162 = add nsw i16 %160, %161
  store i16 %162, ptr %102, !tbaa !2
  br label %b8

b18:
  %163 = load float, ptr %91, !tbaa !2
  %164 = call float @llvm.log2.f32(float %163)
  %165 = load float, ptr %90, !tbaa !2
  %166 = fmul float %164, %165
  %167 = load float, ptr @$float219, !tbaa !2
  %168 = fcmp oge float %166, %167
  br i1 %168, label %b27, label %b28

b19:
  %169 = load float, ptr %91, !tbaa !2
  %170 = load float, ptr @$float218, !tbaa !2
  %171 = fcmp oeq float %169, %170
  br i1 %171, label %b20, label %b22

b20:
  %172 = load float, ptr %90, !tbaa !2
  %173 = load float, ptr @$float218, !tbaa !2
  %174 = fcmp ogt float %172, %173
  br i1 %174, label %b29, label %b21

b21:
  %175 = load float, ptr %90, !tbaa !2
  %176 = load float, ptr @$float218, !tbaa !2
  %177 = fcmp oeq float %175, %176
  br i1 %177, label %b30, label %b25

b22:
  %178 = load float, ptr %90, !tbaa !2
  %179 = call i32 @llvm.lrint.i32.f32(float %178)
  store i32 %179, ptr %88, !tbaa !2
  %180 = load i32, ptr %88, !tbaa !2
  store i32 %180, ptr %87, !tbaa !2
  %181 = load i32, ptr %87, !tbaa !2
  %182 = sitofp i32 %181 to float
  %183 = load float, ptr %90, !tbaa !2
  %184 = fcmp oeq float %182, %183
  br i1 %184, label %b23, label %b25

b23:
  %185 = load float, ptr %91, !tbaa !2
  %186 = fneg float %185
  %187 = call float @llvm.log2.f32(float %186)
  %188 = load float, ptr %90, !tbaa !2
  %189 = fmul float %187, %188
  %190 = load float, ptr @$float219, !tbaa !2
  %191 = fcmp oge float %189, %190
  br i1 %191, label %b31, label %b32

b24:
  %192 = load float, ptr %89, !tbaa !2
  %193 = fneg float %192
  %194 = fpround float %193 to float
  store float %194, ptr %89, !tbaa !2
  br label %b26

b25:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b26:
  %195 = load float, ptr %89, !tbaa !2
  %196 = call i32 @llvm.lrint.i32.f32(float %195)
  %197 = icmp eq i32 %196, 0
  br i1 %197, label %b33, label %b34

b27:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b28:
  %198 = call float @llvm.exp2.f32(float %166)
  %199 = fpround float %198 to float
  store float %199, ptr %89, !tbaa !2
  br label %b26

b29:
  %200 = load float, ptr @$float218, !tbaa !2
  store float %200, ptr %89, !tbaa !2
  br label %b26

b30:
  %201 = load float, ptr @$float220, !tbaa !2
  store float %201, ptr %89, !tbaa !2
  br label %b26

b31:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b32:
  %202 = call float @llvm.exp2.f32(float %189)
  %203 = fpround float %202 to float
  store float %203, ptr %89, !tbaa !2
  %204 = load i32, ptr %88, !tbaa !2
  %205 = and i32 %204, 1
  %206 = icmp ne i32 %205, 0
  br i1 %206, label %b24, label %b26

b33:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b34:
  %207 = sdiv i32 16384, %196
  %208 = trunc i32 %207 to i16
  %209 = sub i16 %136, 0
  %210 = getelementptr inbounds i16, ptr @"SP%", i16 %209
  store i16 %208, ptr %210, !tbaa !2
  %211 = load i16, ptr %96, !tbaa !2
  %212 = sub i16 %211, 0
  %213 = getelementptr inbounds i16, ptr @"SP%", i16 %212
  %214 = load i16, ptr %213, !tbaa !2
  %215 = icmp sgt i16 %214, 255
  br i1 %215, label %b35, label %b36

b35:
  %216 = load i16, ptr %96, !tbaa !2
  %217 = sub i16 %216, 0
  %218 = getelementptr inbounds i16, ptr @"SP%", i16 %217
  store i16 255, ptr %218, !tbaa !2
  br label %b37

b36:
  br label %b37

b37:
  %219 = load i16, ptr %96, !tbaa !2
  %220 = add i16 %219, 1
  store i16 %220, ptr %96, !tbaa !2
  %221 = load i16, ptr %99, !tbaa !2
  %222 = load i16, ptr %97, !tbaa !2
  %223 = add nsw i16 %221, %222
  store i16 %223, ptr %99, !tbaa !2
  br label %b13

b38:
  %224 = load i16, ptr %85, !tbaa !2
  %225 = icmp sge i16 %224, 0
  br i1 %225, label %b39, label %b40

b39:
  %226 = load i16, ptr %105, !tbaa !2
  %227 = load i16, ptr %86, !tbaa !2
  %228 = icmp sle i16 %226, %227
  br i1 %228, label %b41, label %b42

b40:
  %229 = load i16, ptr %105, !tbaa !2
  %230 = load i16, ptr %86, !tbaa !2
  %231 = icmp sge i16 %229, %230
  br i1 %231, label %b41, label %b42

b41:
  %232 = load i16, ptr %105, !tbaa !2
  %233 = icmp slt i16 %232, 160
  br i1 %233, label %b43, label %b44

b42:
  store i16 0, ptr %105, !tbaa !2
  store i16 199, ptr %84, !tbaa !2
  store i16 1, ptr %83, !tbaa !2
  br label %b46

b43:
  %234 = load i16, ptr %105, !tbaa !2
  %235 = load i16, ptr %105, !tbaa !2
  %236 = sub i16 %234, -640
  %237 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %236
  store i16 %235, ptr %237, !tbaa !2
  br label %b45

b44:
  %238 = load i16, ptr %105, !tbaa !2
  %239 = load i16, ptr %105, !tbaa !2
  %240 = sub i16 319, %239
  %241 = sub i16 %238, -640
  %242 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %241
  store i16 %240, ptr %242, !tbaa !2
  br label %b45

b45:
  %243 = load i16, ptr %105, !tbaa !2
  %244 = load i16, ptr %85, !tbaa !2
  %245 = add nsw i16 %243, %244
  store i16 %245, ptr %105, !tbaa !2
  br label %b38

b46:
  %246 = load i16, ptr %83, !tbaa !2
  %247 = icmp sge i16 %246, 0
  br i1 %247, label %b47, label %b48

b47:
  %248 = load i16, ptr %105, !tbaa !2
  %249 = load i16, ptr %84, !tbaa !2
  %250 = icmp sle i16 %248, %249
  br i1 %250, label %b49, label %b50

b48:
  %251 = load i16, ptr %105, !tbaa !2
  %252 = load i16, ptr %84, !tbaa !2
  %253 = icmp sge i16 %251, %252
  br i1 %253, label %b49, label %b50

b49:
  %254 = load i16, ptr %105, !tbaa !2
  %255 = icmp sgt i16 %254, 99
  br i1 %255, label %b51, label %b52

b50:
  store i16 0, ptr %105, !tbaa !2
  store i16 1792, ptr %82, !tbaa !2
  store i16 1, ptr %81, !tbaa !2
  br label %b54

b51:
  %256 = load i16, ptr %105, !tbaa !2
  %257 = load i16, ptr %105, !tbaa !2
  %258 = sub i16 199, %257
  %259 = mul i16 %258, 160
  %260 = sub i16 %256, -640
  %261 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %260
  store i16 %259, ptr %261, !tbaa !2
  br label %b53

b52:
  %262 = load i16, ptr %105, !tbaa !2
  %263 = load i16, ptr %105, !tbaa !2
  %264 = mul i16 %263, 160
  %265 = sub i16 %262, -640
  %266 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %265
  store i16 %264, ptr %266, !tbaa !2
  br label %b53

b53:
  %267 = load i16, ptr %105, !tbaa !2
  %268 = load i16, ptr %83, !tbaa !2
  %269 = add nsw i16 %267, %268
  store i16 %269, ptr %105, !tbaa !2
  br label %b46

b54:
  %270 = load i16, ptr %81, !tbaa !2
  %271 = icmp sge i16 %270, 0
  br i1 %271, label %b55, label %b56

b55:
  %272 = load i16, ptr %105, !tbaa !2
  %273 = load i16, ptr %82, !tbaa !2
  %274 = icmp sle i16 %272, %273
  br i1 %274, label %b57, label %b58

b56:
  %275 = load i16, ptr %105, !tbaa !2
  %276 = load i16, ptr %82, !tbaa !2
  %277 = icmp sge i16 %275, %276
  br i1 %277, label %b57, label %b58

b57:
  %278 = load i16, ptr %105, !tbaa !2
  %279 = icmp sgt i16 %278, 255
  br i1 %279, label %b59, label %b60

b58:
  store i16 1, ptr %105, !tbaa !2
  %280 = load i16, ptr %106, !tbaa !2
  store i16 %280, ptr %80, !tbaa !2
  store i16 1, ptr %79, !tbaa !2
  br label %b62

b59:
  %281 = load i16, ptr %105, !tbaa !2
  %282 = sub i16 %281, 0
  %283 = getelementptr inbounds i16, ptr @"CD%", i16 %282
  store i16 255, ptr %283, !tbaa !2
  br label %b61

b60:
  %284 = load i16, ptr %105, !tbaa !2
  %285 = load i16, ptr %105, !tbaa !2
  %286 = sub i16 %284, 0
  %287 = getelementptr inbounds i16, ptr @"CD%", i16 %286
  store i16 %285, ptr %287, !tbaa !2
  br label %b61

b61:
  %288 = load i16, ptr %105, !tbaa !2
  %289 = load i16, ptr %81, !tbaa !2
  %290 = add nsw i16 %288, %289
  store i16 %290, ptr %105, !tbaa !2
  br label %b54

b62:
  %291 = load i16, ptr %79, !tbaa !2
  %292 = icmp sge i16 %291, 0
  br i1 %292, label %b63, label %b64

b63:
  %293 = load i16, ptr %105, !tbaa !2
  %294 = load i16, ptr %80, !tbaa !2
  %295 = icmp sle i16 %293, %294
  br i1 %295, label %b65, label %b66

b64:
  %296 = load i16, ptr %105, !tbaa !2
  %297 = load i16, ptr %80, !tbaa !2
  %298 = icmp sge i16 %296, %297
  br i1 %298, label %b65, label %b66

b65:
  %299 = load i16, ptr %105, !tbaa !2
  %300 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %301 = load float, ptr %300
  store i16 35, ptr %78, !tbaa !2
  %302 = load i16, ptr %78, !tbaa !2
  %303 = sitofp i16 %302 to float
  %304 = fmul float %301, %303
  store i16 25, ptr %77, !tbaa !2
  %305 = load i16, ptr %77, !tbaa !2
  %306 = sitofp i16 %305 to float
  %307 = fadd float %304, %306
  %308 = call float @llvm.rint.f32(float %307)
  %309 = fcmp olt float %307, %308
  %310 = sext i1 %309 to i16
  store i16 %310, ptr %76, !tbaa !2
  %311 = load i16, ptr %76, !tbaa !2
  %312 = sitofp i16 %311 to float
  %313 = fadd float %308, %312
  %314 = call i16 @llvm.lrint.i16.f32(float %313)
  %315 = sub i16 %299, 0
  %316 = getelementptr inbounds i16, ptr @"DSX%", i16 %315
  store i16 %314, ptr %316, !tbaa !2
  %317 = load i16, ptr %105, !tbaa !2
  %318 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %319 = load float, ptr %318
  store i16 35, ptr %75, !tbaa !2
  %320 = load i16, ptr %75, !tbaa !2
  %321 = sitofp i16 %320 to float
  %322 = fmul float %319, %321
  store i16 25, ptr %74, !tbaa !2
  %323 = load i16, ptr %74, !tbaa !2
  %324 = sitofp i16 %323 to float
  %325 = fadd float %322, %324
  %326 = call float @llvm.rint.f32(float %325)
  %327 = fcmp olt float %325, %326
  %328 = sext i1 %327 to i16
  store i16 %328, ptr %73, !tbaa !2
  %329 = load i16, ptr %73, !tbaa !2
  %330 = sitofp i16 %329 to float
  %331 = fadd float %326, %330
  %332 = call i16 @llvm.lrint.i16.f32(float %331)
  %333 = sub i16 %317, 0
  %334 = getelementptr inbounds i16, ptr @"DSY%", i16 %333
  store i16 %332, ptr %334, !tbaa !2
  %335 = load i16, ptr %105, !tbaa !2
  %336 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %337 = load float, ptr %336
  store i16 64, ptr %72, !tbaa !2
  %338 = load i16, ptr %72, !tbaa !2
  %339 = sitofp i16 %338 to float
  %340 = fmul float %337, %339
  store i16 64, ptr %71, !tbaa !2
  %341 = load i16, ptr %71, !tbaa !2
  %342 = sitofp i16 %341 to float
  %343 = fadd float %340, %342
  %344 = call float @llvm.rint.f32(float %343)
  %345 = fcmp olt float %343, %344
  %346 = sext i1 %345 to i16
  store i16 %346, ptr %70, !tbaa !2
  %347 = load i16, ptr %70, !tbaa !2
  %348 = sitofp i16 %347 to float
  %349 = fadd float %344, %348
  %350 = call i16 @llvm.lrint.i16.f32(float %349)
  %351 = sub i16 %335, 0
  %352 = getelementptr inbounds i16, ptr @"PSX%", i16 %351
  store i16 %350, ptr %352, !tbaa !2
  %353 = load i16, ptr %105, !tbaa !2
  %354 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %355 = load float, ptr %354
  store i16 50, ptr %69, !tbaa !2
  %356 = load i16, ptr %69, !tbaa !2
  %357 = sitofp i16 %356 to float
  %358 = fmul float %355, %357
  store i16 50, ptr %68, !tbaa !2
  %359 = load i16, ptr %68, !tbaa !2
  %360 = sitofp i16 %359 to float
  %361 = fadd float %358, %360
  %362 = call float @llvm.rint.f32(float %361)
  %363 = fcmp olt float %361, %362
  %364 = sext i1 %363 to i16
  store i16 %364, ptr %67, !tbaa !2
  %365 = load i16, ptr %67, !tbaa !2
  %366 = sitofp i16 %365 to float
  %367 = fadd float %362, %366
  %368 = call i16 @llvm.lrint.i16.f32(float %367)
  %369 = sub i16 %353, 0
  %370 = getelementptr inbounds i16, ptr @"PSY%", i16 %369
  store i16 %368, ptr %370, !tbaa !2
  %371 = load i16, ptr %105, !tbaa !2
  %372 = load i16, ptr %79, !tbaa !2
  %373 = add nsw i16 %371, %372
  store i16 %373, ptr %105, !tbaa !2
  br label %b62

b66:
  store i16 0, ptr %66, !tbaa !2
  store i16 3, ptr %65, !tbaa !2
  %374 = getelementptr i8, ptr @"R%", i16 2
  %375 = load i16, ptr %374, !tbaa !2
  %376 = getelementptr i8, ptr @"R%", i16 10
  %377 = load i16, ptr %376, !tbaa !2
  %378 = add i16 %377, 2
  %379 = inttoptr i16 %375 to ptr addrspace(2)
  %380 = addrspacecast ptr addrspace(2) %379 to ptr addrspace(1)
  %381 = getelementptr i8, ptr addrspace(1) %380, i16 %378
  store i16 1, ptr addrspace(1) %381, !tbaa !60
  %382 = getelementptr i8, ptr @"G%", i16 2
  %383 = load i16, ptr %382, !tbaa !2
  %384 = getelementptr i8, ptr @"G%", i16 10
  %385 = load i16, ptr %384, !tbaa !2
  %386 = add i16 %385, 2
  %387 = inttoptr i16 %383 to ptr addrspace(2)
  %388 = addrspacecast ptr addrspace(2) %387 to ptr addrspace(1)
  %389 = getelementptr i8, ptr addrspace(1) %388, i16 %386
  store i16 1, ptr addrspace(1) %389, !tbaa !62
  %390 = getelementptr i8, ptr @"B%", i16 2
  %391 = load i16, ptr %390, !tbaa !2
  %392 = getelementptr i8, ptr @"B%", i16 10
  %393 = load i16, ptr %392, !tbaa !2
  %394 = add i16 %393, 2
  %395 = inttoptr i16 %391 to ptr addrspace(2)
  %396 = addrspacecast ptr addrspace(2) %395 to ptr addrspace(1)
  %397 = getelementptr i8, ptr addrspace(1) %396, i16 %394
  store i16 1, ptr addrspace(1) %397, !tbaa !64
  store i16 0, ptr %64, !tbaa !2
  br label %b68

b68:
  %398 = load i16, ptr %64, !tbaa !2
  %399 = add i16 %398, 1
  store i16 %399, ptr %64, !tbaa !2
  %400 = load i16, ptr %63, !tbaa !2
  %401 = add i16 %400, 1
  store i16 %401, ptr %63, !tbaa !2
  %402 = load i16, ptr %66, !tbaa !2
  %403 = add i16 %402, 1
  store i16 %403, ptr %66, !tbaa !2
  %404 = load i16, ptr %64, !tbaa !2
  %405 = icmp slt i16 %404, 160
  %406 = sext i1 %405 to i16
  %407 = load i16, ptr %66, !tbaa !2
  %408 = load i16, ptr %65, !tbaa !2
  %409 = icmp eq i16 %407, %408
  %410 = sext i1 %409 to i16
  %411 = and i16 %406, %410
  %412 = icmp ne i16 %411, 0
  br i1 %412, label %b70, label %b71

b70:
  store i16 0, ptr %66, !tbaa !2
  %413 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %414 = load float, ptr %413
  store i16 25, ptr %62, !tbaa !2
  %415 = load i16, ptr %62, !tbaa !2
  %416 = sitofp i16 %415 to float
  %417 = fmul float %414, %416
  store i16 1, ptr %61, !tbaa !2
  %418 = load i16, ptr %61, !tbaa !2
  %419 = sitofp i16 %418 to float
  %420 = fadd float %417, %419
  %421 = call float @llvm.rint.f32(float %420)
  %422 = fcmp olt float %420, %421
  %423 = sext i1 %422 to i16
  store i16 %423, ptr %60, !tbaa !2
  %424 = load i16, ptr %60, !tbaa !2
  %425 = sitofp i16 %424 to float
  %426 = fadd float %421, %425
  %427 = call i16 @llvm.lrint.i16.f32(float %426)
  store i16 %427, ptr %65, !tbaa !2
  %428 = getelementptr i8, ptr @"R%", i16 2
  %429 = load i16, ptr %428, !tbaa !2
  %430 = getelementptr i8, ptr @"R%", i16 10
  %431 = load i16, ptr %430, !tbaa !2
  %432 = add i16 %431, 2
  %433 = inttoptr i16 %429 to ptr addrspace(2)
  %434 = addrspacecast ptr addrspace(2) %433 to ptr addrspace(1)
  %435 = getelementptr i8, ptr addrspace(1) %434, i16 %432
  %436 = getelementptr i8, ptr @"R%", i16 2
  %437 = load i16, ptr %436, !tbaa !2
  %438 = getelementptr i8, ptr @"R%", i16 10
  %439 = load i16, ptr %438, !tbaa !2
  %440 = add i16 %439, 4
  %441 = inttoptr i16 %437 to ptr addrspace(2)
  %442 = addrspacecast ptr addrspace(2) %441 to ptr addrspace(1)
  %443 = getelementptr i8, ptr addrspace(1) %442, i16 %440
  %444 = load i16, ptr addrspace(1) %443, !tbaa !60
  store i16 %444, ptr addrspace(1) %435, !tbaa !60
  %445 = getelementptr i8, ptr @"G%", i16 2
  %446 = load i16, ptr %445, !tbaa !2
  %447 = getelementptr i8, ptr @"G%", i16 10
  %448 = load i16, ptr %447, !tbaa !2
  %449 = add i16 %448, 2
  %450 = inttoptr i16 %446 to ptr addrspace(2)
  %451 = addrspacecast ptr addrspace(2) %450 to ptr addrspace(1)
  %452 = getelementptr i8, ptr addrspace(1) %451, i16 %449
  %453 = getelementptr i8, ptr @"G%", i16 2
  %454 = load i16, ptr %453, !tbaa !2
  %455 = getelementptr i8, ptr @"G%", i16 10
  %456 = load i16, ptr %455, !tbaa !2
  %457 = add i16 %456, 4
  %458 = inttoptr i16 %454 to ptr addrspace(2)
  %459 = addrspacecast ptr addrspace(2) %458 to ptr addrspace(1)
  %460 = getelementptr i8, ptr addrspace(1) %459, i16 %457
  %461 = load i16, ptr addrspace(1) %460, !tbaa !62
  store i16 %461, ptr addrspace(1) %452, !tbaa !62
  %462 = getelementptr i8, ptr @"B%", i16 2
  %463 = load i16, ptr %462, !tbaa !2
  %464 = getelementptr i8, ptr @"B%", i16 10
  %465 = load i16, ptr %464, !tbaa !2
  %466 = add i16 %465, 2
  %467 = inttoptr i16 %463 to ptr addrspace(2)
  %468 = addrspacecast ptr addrspace(2) %467 to ptr addrspace(1)
  %469 = getelementptr i8, ptr addrspace(1) %468, i16 %466
  %470 = getelementptr i8, ptr @"B%", i16 2
  %471 = load i16, ptr %470, !tbaa !2
  %472 = getelementptr i8, ptr @"B%", i16 10
  %473 = load i16, ptr %472, !tbaa !2
  %474 = add i16 %473, 4
  %475 = inttoptr i16 %471 to ptr addrspace(2)
  %476 = addrspacecast ptr addrspace(2) %475 to ptr addrspace(1)
  %477 = getelementptr i8, ptr addrspace(1) %476, i16 %474
  %478 = load i16, ptr addrspace(1) %477, !tbaa !64
  store i16 %478, ptr addrspace(1) %469, !tbaa !64
  %479 = getelementptr i8, ptr @"R%", i16 2
  %480 = load i16, ptr %479, !tbaa !2
  %481 = getelementptr i8, ptr @"R%", i16 10
  %482 = load i16, ptr %481, !tbaa !2
  %483 = add i16 %482, 4
  %484 = inttoptr i16 %480 to ptr addrspace(2)
  %485 = addrspacecast ptr addrspace(2) %484 to ptr addrspace(1)
  %486 = getelementptr i8, ptr addrspace(1) %485, i16 %483
  %487 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %488 = load float, ptr %487
  store i16 63, ptr %59, !tbaa !2
  %489 = load i16, ptr %59, !tbaa !2
  %490 = sitofp i16 %489 to float
  %491 = fmul float %488, %490
  %492 = call float @llvm.rint.f32(float %491)
  %493 = fcmp olt float %491, %492
  %494 = sext i1 %493 to i16
  store i16 %494, ptr %58, !tbaa !2
  %495 = load i16, ptr %58, !tbaa !2
  %496 = sitofp i16 %495 to float
  %497 = fadd float %492, %496
  store i16 1, ptr %57, !tbaa !2
  %498 = load i16, ptr %57, !tbaa !2
  %499 = sitofp i16 %498 to float
  %500 = fadd float %497, %499
  %501 = call i16 @llvm.lrint.i16.f32(float %500)
  store i16 %501, ptr addrspace(1) %486, !tbaa !60
  %502 = getelementptr i8, ptr @"G%", i16 2
  %503 = load i16, ptr %502, !tbaa !2
  %504 = getelementptr i8, ptr @"G%", i16 10
  %505 = load i16, ptr %504, !tbaa !2
  %506 = add i16 %505, 4
  %507 = inttoptr i16 %503 to ptr addrspace(2)
  %508 = addrspacecast ptr addrspace(2) %507 to ptr addrspace(1)
  %509 = getelementptr i8, ptr addrspace(1) %508, i16 %506
  %510 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %511 = load float, ptr %510
  store i16 63, ptr %56, !tbaa !2
  %512 = load i16, ptr %56, !tbaa !2
  %513 = sitofp i16 %512 to float
  %514 = fmul float %511, %513
  %515 = call float @llvm.rint.f32(float %514)
  %516 = fcmp olt float %514, %515
  %517 = sext i1 %516 to i16
  store i16 %517, ptr %55, !tbaa !2
  %518 = load i16, ptr %55, !tbaa !2
  %519 = sitofp i16 %518 to float
  %520 = fadd float %515, %519
  store i16 1, ptr %54, !tbaa !2
  %521 = load i16, ptr %54, !tbaa !2
  %522 = sitofp i16 %521 to float
  %523 = fadd float %520, %522
  %524 = call i16 @llvm.lrint.i16.f32(float %523)
  store i16 %524, ptr addrspace(1) %509, !tbaa !62
  %525 = getelementptr i8, ptr @"B%", i16 2
  %526 = load i16, ptr %525, !tbaa !2
  %527 = getelementptr i8, ptr @"B%", i16 10
  %528 = load i16, ptr %527, !tbaa !2
  %529 = add i16 %528, 4
  %530 = inttoptr i16 %526 to ptr addrspace(2)
  %531 = addrspacecast ptr addrspace(2) %530 to ptr addrspace(1)
  %532 = getelementptr i8, ptr addrspace(1) %531, i16 %529
  %533 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %534 = load float, ptr %533
  store i16 63, ptr %53, !tbaa !2
  %535 = load i16, ptr %53, !tbaa !2
  %536 = sitofp i16 %535 to float
  %537 = fmul float %534, %536
  %538 = call float @llvm.rint.f32(float %537)
  %539 = fcmp olt float %537, %538
  %540 = sext i1 %539 to i16
  store i16 %540, ptr %52, !tbaa !2
  %541 = load i16, ptr %52, !tbaa !2
  %542 = sitofp i16 %541 to float
  %543 = fadd float %538, %542
  store i16 1, ptr %51, !tbaa !2
  %544 = load i16, ptr %51, !tbaa !2
  %545 = sitofp i16 %544 to float
  %546 = fadd float %543, %545
  %547 = call i16 @llvm.lrint.i16.f32(float %546)
  store i16 %547, ptr addrspace(1) %532, !tbaa !64
  br label %b72

b71:
  br label %b72

b72:
  %548 = load i16, ptr %64, !tbaa !2
  %549 = icmp sgt i16 %548, 160
  %550 = sext i1 %549 to i16
  %551 = load i16, ptr %64, !tbaa !2
  %552 = icmp slt i16 %551, 185
  %553 = sext i1 %552 to i16
  %554 = and i16 %550, %553
  %555 = load i16, ptr %66, !tbaa !2
  %556 = load i16, ptr %65, !tbaa !2
  %557 = icmp eq i16 %555, %556
  %558 = sext i1 %557 to i16
  %559 = and i16 %554, %558
  %560 = icmp ne i16 %559, 0
  br i1 %560, label %b73, label %b74

b73:
  store i16 1, ptr %50, !tbaa !2
  store i16 0, ptr %66, !tbaa !2
  store i16 25, ptr %65, !tbaa !2
  %561 = getelementptr i8, ptr @"R%", i16 2
  %562 = load i16, ptr %561, !tbaa !2
  %563 = getelementptr i8, ptr @"R%", i16 10
  %564 = load i16, ptr %563, !tbaa !2
  %565 = add i16 %564, 2
  %566 = inttoptr i16 %562 to ptr addrspace(2)
  %567 = addrspacecast ptr addrspace(2) %566 to ptr addrspace(1)
  %568 = getelementptr i8, ptr addrspace(1) %567, i16 %565
  %569 = getelementptr i8, ptr @"R%", i16 2
  %570 = load i16, ptr %569, !tbaa !2
  %571 = getelementptr i8, ptr @"R%", i16 10
  %572 = load i16, ptr %571, !tbaa !2
  %573 = add i16 %572, 4
  %574 = inttoptr i16 %570 to ptr addrspace(2)
  %575 = addrspacecast ptr addrspace(2) %574 to ptr addrspace(1)
  %576 = getelementptr i8, ptr addrspace(1) %575, i16 %573
  %577 = load i16, ptr addrspace(1) %576, !tbaa !60
  store i16 %577, ptr addrspace(1) %568, !tbaa !60
  %578 = getelementptr i8, ptr @"G%", i16 2
  %579 = load i16, ptr %578, !tbaa !2
  %580 = getelementptr i8, ptr @"G%", i16 10
  %581 = load i16, ptr %580, !tbaa !2
  %582 = add i16 %581, 2
  %583 = inttoptr i16 %579 to ptr addrspace(2)
  %584 = addrspacecast ptr addrspace(2) %583 to ptr addrspace(1)
  %585 = getelementptr i8, ptr addrspace(1) %584, i16 %582
  %586 = getelementptr i8, ptr @"G%", i16 2
  %587 = load i16, ptr %586, !tbaa !2
  %588 = getelementptr i8, ptr @"G%", i16 10
  %589 = load i16, ptr %588, !tbaa !2
  %590 = add i16 %589, 4
  %591 = inttoptr i16 %587 to ptr addrspace(2)
  %592 = addrspacecast ptr addrspace(2) %591 to ptr addrspace(1)
  %593 = getelementptr i8, ptr addrspace(1) %592, i16 %590
  %594 = load i16, ptr addrspace(1) %593, !tbaa !62
  store i16 %594, ptr addrspace(1) %585, !tbaa !62
  %595 = getelementptr i8, ptr @"B%", i16 2
  %596 = load i16, ptr %595, !tbaa !2
  %597 = getelementptr i8, ptr @"B%", i16 10
  %598 = load i16, ptr %597, !tbaa !2
  %599 = add i16 %598, 2
  %600 = inttoptr i16 %596 to ptr addrspace(2)
  %601 = addrspacecast ptr addrspace(2) %600 to ptr addrspace(1)
  %602 = getelementptr i8, ptr addrspace(1) %601, i16 %599
  %603 = getelementptr i8, ptr @"B%", i16 2
  %604 = load i16, ptr %603, !tbaa !2
  %605 = getelementptr i8, ptr @"B%", i16 10
  %606 = load i16, ptr %605, !tbaa !2
  %607 = add i16 %606, 4
  %608 = inttoptr i16 %604 to ptr addrspace(2)
  %609 = addrspacecast ptr addrspace(2) %608 to ptr addrspace(1)
  %610 = getelementptr i8, ptr addrspace(1) %609, i16 %607
  %611 = load i16, ptr addrspace(1) %610, !tbaa !64
  store i16 %611, ptr addrspace(1) %602, !tbaa !64
  %612 = getelementptr i8, ptr @"R%", i16 2
  %613 = load i16, ptr %612, !tbaa !2
  %614 = getelementptr i8, ptr @"R%", i16 10
  %615 = load i16, ptr %614, !tbaa !2
  %616 = add i16 %615, 4
  %617 = inttoptr i16 %613 to ptr addrspace(2)
  %618 = addrspacecast ptr addrspace(2) %617 to ptr addrspace(1)
  %619 = getelementptr i8, ptr addrspace(1) %618, i16 %616
  store i16 0, ptr addrspace(1) %619, !tbaa !60
  %620 = getelementptr i8, ptr @"G%", i16 2
  %621 = load i16, ptr %620, !tbaa !2
  %622 = getelementptr i8, ptr @"G%", i16 10
  %623 = load i16, ptr %622, !tbaa !2
  %624 = add i16 %623, 4
  %625 = inttoptr i16 %621 to ptr addrspace(2)
  %626 = addrspacecast ptr addrspace(2) %625 to ptr addrspace(1)
  %627 = getelementptr i8, ptr addrspace(1) %626, i16 %624
  store i16 0, ptr addrspace(1) %627, !tbaa !62
  %628 = getelementptr i8, ptr @"B%", i16 2
  %629 = load i16, ptr %628, !tbaa !2
  %630 = getelementptr i8, ptr @"B%", i16 10
  %631 = load i16, ptr %630, !tbaa !2
  %632 = add i16 %631, 4
  %633 = inttoptr i16 %629 to ptr addrspace(2)
  %634 = addrspacecast ptr addrspace(2) %633 to ptr addrspace(1)
  %635 = getelementptr i8, ptr addrspace(1) %634, i16 %632
  store i16 0, ptr addrspace(1) %635, !tbaa !64
  br label %b75

b74:
  br label %b75

b75:
  %636 = load i16, ptr %64, !tbaa !2
  %637 = icmp sgt i16 %636, 185
  %638 = sext i1 %637 to i16
  %639 = load i16, ptr %50, !tbaa !2
  %640 = icmp eq i16 %639, 1
  %641 = sext i1 %640 to i16
  %642 = and i16 %638, %641
  %643 = load i16, ptr %66, !tbaa !2
  %644 = load i16, ptr %65, !tbaa !2
  %645 = icmp sge i16 %643, %644
  %646 = sext i1 %645 to i16
  %647 = and i16 %642, %646
  %648 = icmp ne i16 %647, 0
  br i1 %648, label %b76, label %b77

b76:
  br label %b2

b77:
  br label %b78

b78:
  %649 = load i16, ptr %64, !tbaa !2
  %650 = icmp sgt i16 %649, 210
  br i1 %650, label %b80, label %b81

b80:
  br label %b2

b81:
  br label %b82

b82:
  %651 = getelementptr i8, ptr @"R%", i16 2
  %652 = load i16, ptr %651, !tbaa !2
  %653 = getelementptr i8, ptr @"R%", i16 10
  %654 = load i16, ptr %653, !tbaa !2
  %655 = add i16 %654, 2
  %656 = inttoptr i16 %652 to ptr addrspace(2)
  %657 = addrspacecast ptr addrspace(2) %656 to ptr addrspace(1)
  %658 = getelementptr i8, ptr addrspace(1) %657, i16 %655
  %659 = load i16, ptr addrspace(1) %658, !tbaa !60
  %660 = getelementptr i8, ptr @"R%", i16 2
  %661 = load i16, ptr %660, !tbaa !2
  %662 = getelementptr i8, ptr @"R%", i16 10
  %663 = load i16, ptr %662, !tbaa !2
  %664 = add i16 %663, 4
  %665 = inttoptr i16 %661 to ptr addrspace(2)
  %666 = addrspacecast ptr addrspace(2) %665 to ptr addrspace(1)
  %667 = getelementptr i8, ptr addrspace(1) %666, i16 %664
  %668 = load i16, ptr addrspace(1) %667, !tbaa !60
  %669 = getelementptr i8, ptr @"R%", i16 2
  %670 = load i16, ptr %669, !tbaa !2
  %671 = getelementptr i8, ptr @"R%", i16 10
  %672 = load i16, ptr %671, !tbaa !2
  %673 = add i16 %672, 2
  %674 = inttoptr i16 %670 to ptr addrspace(2)
  %675 = addrspacecast ptr addrspace(2) %674 to ptr addrspace(1)
  %676 = getelementptr i8, ptr addrspace(1) %675, i16 %673
  %677 = load i16, ptr addrspace(1) %676, !tbaa !60
  %678 = sub i16 %668, %677
  %679 = load i16, ptr %65, !tbaa !2
  store i16 %678, ptr %48, !tbaa !2
  %680 = load i16, ptr %48, !tbaa !2
  %681 = sitofp i16 %680 to float
  store i16 %679, ptr %47, !tbaa !2
  %682 = load i16, ptr %47, !tbaa !2
  %683 = sitofp i16 %682 to float
  %684 = fdiv float %681, %683
  %685 = load i16, ptr %66, !tbaa !2
  store i16 %685, ptr %46, !tbaa !2
  %686 = load i16, ptr %46, !tbaa !2
  %687 = sitofp i16 %686 to float
  %688 = fmul float %684, %687
  store i16 %659, ptr %45, !tbaa !2
  %689 = load i16, ptr %45, !tbaa !2
  %690 = sitofp i16 %689 to float
  %691 = fadd float %690, %688
  %692 = call i16 @llvm.lrint.i16.f32(float %691)
  store i16 %692, ptr %49, !tbaa !2
  %693 = getelementptr i8, ptr @"G%", i16 2
  %694 = load i16, ptr %693, !tbaa !2
  %695 = getelementptr i8, ptr @"G%", i16 10
  %696 = load i16, ptr %695, !tbaa !2
  %697 = add i16 %696, 2
  %698 = inttoptr i16 %694 to ptr addrspace(2)
  %699 = addrspacecast ptr addrspace(2) %698 to ptr addrspace(1)
  %700 = getelementptr i8, ptr addrspace(1) %699, i16 %697
  %701 = load i16, ptr addrspace(1) %700, !tbaa !62
  %702 = getelementptr i8, ptr @"G%", i16 2
  %703 = load i16, ptr %702, !tbaa !2
  %704 = getelementptr i8, ptr @"G%", i16 10
  %705 = load i16, ptr %704, !tbaa !2
  %706 = add i16 %705, 4
  %707 = inttoptr i16 %703 to ptr addrspace(2)
  %708 = addrspacecast ptr addrspace(2) %707 to ptr addrspace(1)
  %709 = getelementptr i8, ptr addrspace(1) %708, i16 %706
  %710 = load i16, ptr addrspace(1) %709, !tbaa !62
  %711 = getelementptr i8, ptr @"G%", i16 2
  %712 = load i16, ptr %711, !tbaa !2
  %713 = getelementptr i8, ptr @"G%", i16 10
  %714 = load i16, ptr %713, !tbaa !2
  %715 = add i16 %714, 2
  %716 = inttoptr i16 %712 to ptr addrspace(2)
  %717 = addrspacecast ptr addrspace(2) %716 to ptr addrspace(1)
  %718 = getelementptr i8, ptr addrspace(1) %717, i16 %715
  %719 = load i16, ptr addrspace(1) %718, !tbaa !62
  %720 = sub i16 %710, %719
  %721 = load i16, ptr %65, !tbaa !2
  store i16 %720, ptr %43, !tbaa !2
  %722 = load i16, ptr %43, !tbaa !2
  %723 = sitofp i16 %722 to float
  store i16 %721, ptr %42, !tbaa !2
  %724 = load i16, ptr %42, !tbaa !2
  %725 = sitofp i16 %724 to float
  %726 = fdiv float %723, %725
  %727 = load i16, ptr %66, !tbaa !2
  store i16 %727, ptr %41, !tbaa !2
  %728 = load i16, ptr %41, !tbaa !2
  %729 = sitofp i16 %728 to float
  %730 = fmul float %726, %729
  store i16 %701, ptr %40, !tbaa !2
  %731 = load i16, ptr %40, !tbaa !2
  %732 = sitofp i16 %731 to float
  %733 = fadd float %732, %730
  %734 = call i16 @llvm.lrint.i16.f32(float %733)
  store i16 %734, ptr %44, !tbaa !2
  %735 = getelementptr i8, ptr @"B%", i16 2
  %736 = load i16, ptr %735, !tbaa !2
  %737 = getelementptr i8, ptr @"B%", i16 10
  %738 = load i16, ptr %737, !tbaa !2
  %739 = add i16 %738, 2
  %740 = inttoptr i16 %736 to ptr addrspace(2)
  %741 = addrspacecast ptr addrspace(2) %740 to ptr addrspace(1)
  %742 = getelementptr i8, ptr addrspace(1) %741, i16 %739
  %743 = load i16, ptr addrspace(1) %742, !tbaa !64
  %744 = getelementptr i8, ptr @"B%", i16 2
  %745 = load i16, ptr %744, !tbaa !2
  %746 = getelementptr i8, ptr @"B%", i16 10
  %747 = load i16, ptr %746, !tbaa !2
  %748 = add i16 %747, 4
  %749 = inttoptr i16 %745 to ptr addrspace(2)
  %750 = addrspacecast ptr addrspace(2) %749 to ptr addrspace(1)
  %751 = getelementptr i8, ptr addrspace(1) %750, i16 %748
  %752 = load i16, ptr addrspace(1) %751, !tbaa !64
  %753 = getelementptr i8, ptr @"B%", i16 2
  %754 = load i16, ptr %753, !tbaa !2
  %755 = getelementptr i8, ptr @"B%", i16 10
  %756 = load i16, ptr %755, !tbaa !2
  %757 = add i16 %756, 2
  %758 = inttoptr i16 %754 to ptr addrspace(2)
  %759 = addrspacecast ptr addrspace(2) %758 to ptr addrspace(1)
  %760 = getelementptr i8, ptr addrspace(1) %759, i16 %757
  %761 = load i16, ptr addrspace(1) %760, !tbaa !64
  %762 = sub i16 %752, %761
  %763 = load i16, ptr %65, !tbaa !2
  store i16 %762, ptr %38, !tbaa !2
  %764 = load i16, ptr %38, !tbaa !2
  %765 = sitofp i16 %764 to float
  store i16 %763, ptr %37, !tbaa !2
  %766 = load i16, ptr %37, !tbaa !2
  %767 = sitofp i16 %766 to float
  %768 = fdiv float %765, %767
  %769 = load i16, ptr %66, !tbaa !2
  store i16 %769, ptr %36, !tbaa !2
  %770 = load i16, ptr %36, !tbaa !2
  %771 = sitofp i16 %770 to float
  %772 = fmul float %768, %771
  store i16 %743, ptr %35, !tbaa !2
  %773 = load i16, ptr %35, !tbaa !2
  %774 = sitofp i16 %773 to float
  %775 = fadd float %774, %772
  %776 = call i16 @llvm.lrint.i16.f32(float %775)
  store i16 %776, ptr %39, !tbaa !2
  %777 = load i16, ptr %49, !tbaa !2
  %778 = icmp eq i16 %777, 0
  br i1 %778, label %b84, label %b85

b84:
  store i16 1, ptr %49, !tbaa !2
  br label %b86

b85:
  br label %b86

b86:
  %779 = load i16, ptr %44, !tbaa !2
  %780 = icmp eq i16 %779, 0
  br i1 %780, label %b87, label %b88

b87:
  store i16 1, ptr %44, !tbaa !2
  br label %b89

b88:
  br label %b89

b89:
  %781 = load i16, ptr %39, !tbaa !2
  %782 = icmp eq i16 %781, 0
  br i1 %782, label %b90, label %b91

b90:
  store i16 1, ptr %39, !tbaa !2
  br label %b92

b91:
  br label %b92

b92:
  store i16 128, ptr %34, !tbaa !2
  store i16 255, ptr %33, !tbaa !2
  store i16 1, ptr %32, !tbaa !2
  br label %b93

b93:
  %783 = load i16, ptr %32, !tbaa !2
  %784 = icmp sge i16 %783, 0
  br i1 %784, label %b94, label %b95

b94:
  %785 = load i16, ptr %34, !tbaa !2
  %786 = load i16, ptr %33, !tbaa !2
  %787 = icmp sle i16 %785, %786
  br i1 %787, label %b96, label %b97

b95:
  %788 = load i16, ptr %34, !tbaa !2
  %789 = load i16, ptr %33, !tbaa !2
  %790 = icmp sge i16 %788, %789
  br i1 %790, label %b96, label %b97

b96:
  %791 = load i16, ptr %34, !tbaa !2
  %792 = trunc i16 %791 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %792)
  %793 = load i16, ptr %34, !tbaa !2
  %794 = icmp slt i16 %793, 192
  br i1 %794, label %b98, label %b99

b97:
  store i16 1, ptr %105, !tbaa !2
  %795 = load i16, ptr %106, !tbaa !2
  store i16 %795, ptr %13, !tbaa !2
  store i16 1, ptr %12, !tbaa !2
  br label %b101

b98:
  %796 = load i16, ptr %34, !tbaa !2
  %797 = sub i16 %796, 128
  %798 = load i16, ptr %49, !tbaa !2
  store i16 63, ptr %31, !tbaa !2
  %799 = load i16, ptr %31, !tbaa !2
  %800 = sitofp i16 %799 to float
  store i16 %798, ptr %30, !tbaa !2
  %801 = load i16, ptr %30, !tbaa !2
  %802 = sitofp i16 %801 to float
  %803 = fdiv float %800, %802
  store i16 %797, ptr %29, !tbaa !2
  %804 = load i16, ptr %29, !tbaa !2
  %805 = sitofp i16 %804 to float
  %806 = fdiv float %805, %803
  %807 = call i16 @llvm.lrint.i16.f32(float %806)
  %808 = trunc i16 %807 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %808)
  %809 = load i16, ptr %34, !tbaa !2
  %810 = sub i16 %809, 128
  %811 = load i16, ptr %44, !tbaa !2
  store i16 63, ptr %28, !tbaa !2
  %812 = load i16, ptr %28, !tbaa !2
  %813 = sitofp i16 %812 to float
  store i16 %811, ptr %27, !tbaa !2
  %814 = load i16, ptr %27, !tbaa !2
  %815 = sitofp i16 %814 to float
  %816 = fdiv float %813, %815
  store i16 %810, ptr %26, !tbaa !2
  %817 = load i16, ptr %26, !tbaa !2
  %818 = sitofp i16 %817 to float
  %819 = fdiv float %818, %816
  %820 = call i16 @llvm.lrint.i16.f32(float %819)
  %821 = trunc i16 %820 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %821)
  %822 = load i16, ptr %34, !tbaa !2
  %823 = sub i16 %822, 128
  %824 = load i16, ptr %39, !tbaa !2
  store i16 63, ptr %25, !tbaa !2
  %825 = load i16, ptr %25, !tbaa !2
  %826 = sitofp i16 %825 to float
  store i16 %824, ptr %24, !tbaa !2
  %827 = load i16, ptr %24, !tbaa !2
  %828 = sitofp i16 %827 to float
  %829 = fdiv float %826, %828
  store i16 %823, ptr %23, !tbaa !2
  %830 = load i16, ptr %23, !tbaa !2
  %831 = sitofp i16 %830 to float
  %832 = fdiv float %831, %829
  %833 = call i16 @llvm.lrint.i16.f32(float %832)
  %834 = trunc i16 %833 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %834)
  br label %b100

b99:
  %835 = load i16, ptr %34, !tbaa !2
  %836 = sub i16 255, %835
  %837 = load i16, ptr %49, !tbaa !2
  store i16 63, ptr %22, !tbaa !2
  %838 = load i16, ptr %22, !tbaa !2
  %839 = sitofp i16 %838 to float
  store i16 %837, ptr %21, !tbaa !2
  %840 = load i16, ptr %21, !tbaa !2
  %841 = sitofp i16 %840 to float
  %842 = fdiv float %839, %841
  store i16 %836, ptr %20, !tbaa !2
  %843 = load i16, ptr %20, !tbaa !2
  %844 = sitofp i16 %843 to float
  %845 = fdiv float %844, %842
  %846 = call i16 @llvm.lrint.i16.f32(float %845)
  %847 = trunc i16 %846 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %847)
  %848 = load i16, ptr %34, !tbaa !2
  %849 = sub i16 255, %848
  %850 = load i16, ptr %44, !tbaa !2
  store i16 63, ptr %19, !tbaa !2
  %851 = load i16, ptr %19, !tbaa !2
  %852 = sitofp i16 %851 to float
  store i16 %850, ptr %18, !tbaa !2
  %853 = load i16, ptr %18, !tbaa !2
  %854 = sitofp i16 %853 to float
  %855 = fdiv float %852, %854
  store i16 %849, ptr %17, !tbaa !2
  %856 = load i16, ptr %17, !tbaa !2
  %857 = sitofp i16 %856 to float
  %858 = fdiv float %857, %855
  %859 = call i16 @llvm.lrint.i16.f32(float %858)
  %860 = trunc i16 %859 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %860)
  %861 = load i16, ptr %34, !tbaa !2
  %862 = sub i16 255, %861
  %863 = load i16, ptr %39, !tbaa !2
  store i16 63, ptr %16, !tbaa !2
  %864 = load i16, ptr %16, !tbaa !2
  %865 = sitofp i16 %864 to float
  store i16 %863, ptr %15, !tbaa !2
  %866 = load i16, ptr %15, !tbaa !2
  %867 = sitofp i16 %866 to float
  %868 = fdiv float %865, %867
  store i16 %862, ptr %14, !tbaa !2
  %869 = load i16, ptr %14, !tbaa !2
  %870 = sitofp i16 %869 to float
  %871 = fdiv float %870, %868
  %872 = call i16 @llvm.lrint.i16.f32(float %871)
  %873 = trunc i16 %872 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %873)
  br label %b100

b100:
  %874 = load i16, ptr %34, !tbaa !2
  %875 = load i16, ptr %32, !tbaa !2
  %876 = add nsw i16 %874, %875
  store i16 %876, ptr %34, !tbaa !2
  br label %b93

b101:
  %877 = load i16, ptr %12, !tbaa !2
  %878 = icmp sge i16 %877, 0
  br i1 %878, label %b102, label %b103

b102:
  %879 = load i16, ptr %105, !tbaa !2
  %880 = load i16, ptr %13, !tbaa !2
  %881 = icmp sle i16 %879, %880
  br i1 %881, label %b104, label %b105

b103:
  %882 = load i16, ptr %105, !tbaa !2
  %883 = load i16, ptr %13, !tbaa !2
  %884 = icmp sge i16 %882, %883
  br i1 %884, label %b104, label %b105

b104:
  %885 = load i16, ptr %105, !tbaa !2
  %886 = load i16, ptr %63, !tbaa !2
  %887 = load i16, ptr %105, !tbaa !2
  %888 = sub i16 %887, 0
  %889 = getelementptr inbounds i16, ptr @"DSX%", i16 %888
  %890 = load i16, ptr %889, !tbaa !2
  store i16 %886, ptr %11, !tbaa !2
  %891 = load i16, ptr %11, !tbaa !2
  %892 = sitofp i16 %891 to float
  store i16 %890, ptr %10, !tbaa !2
  %893 = load i16, ptr %10, !tbaa !2
  %894 = sitofp i16 %893 to float
  %895 = fdiv float %892, %894
  %896 = call float @llvm.fabs.f32(float %895)
  %897 = load float, ptr @$float86, !tbaa !2
  %898 = fcmp ogt float %896, %897
  br i1 %898, label %b106, label %b107

b105:
  store i16 0, ptr %5, !tbaa !2
  store i16 0, ptr %102, !tbaa !2
  store i16 199, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b110

b106:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b107:
  %899 = call float @llvm.sin.f32(float %895)
  %900 = load i16, ptr %105, !tbaa !2
  %901 = sub i16 %900, 0
  %902 = getelementptr inbounds i16, ptr @"PSX%", i16 %901
  %903 = load i16, ptr %902, !tbaa !2
  store i16 %903, ptr %9, !tbaa !2
  %904 = load i16, ptr %9, !tbaa !2
  %905 = sitofp i16 %904 to float
  %906 = fmul float %899, %905
  %907 = call i16 @llvm.lrint.i16.f32(float %906)
  %908 = sub i16 %885, 0
  %909 = getelementptr inbounds i16, ptr @"XP1%", i16 %908
  store i16 %907, ptr %909, !tbaa !2
  %910 = load i16, ptr %105, !tbaa !2
  %911 = load i16, ptr %63, !tbaa !2
  %912 = load i16, ptr %105, !tbaa !2
  %913 = sub i16 %912, 0
  %914 = getelementptr inbounds i16, ptr @"DSY%", i16 %913
  %915 = load i16, ptr %914, !tbaa !2
  store i16 %911, ptr %8, !tbaa !2
  %916 = load i16, ptr %8, !tbaa !2
  %917 = sitofp i16 %916 to float
  store i16 %915, ptr %7, !tbaa !2
  %918 = load i16, ptr %7, !tbaa !2
  %919 = sitofp i16 %918 to float
  %920 = fdiv float %917, %919
  %921 = call float @llvm.fabs.f32(float %920)
  %922 = load float, ptr @$float86, !tbaa !2
  %923 = fcmp ogt float %921, %922
  br i1 %923, label %b108, label %b109

b108:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b109:
  %924 = call float @llvm.sin.f32(float %920)
  %925 = load i16, ptr %105, !tbaa !2
  %926 = sub i16 %925, 0
  %927 = getelementptr inbounds i16, ptr @"PSY%", i16 %926
  %928 = load i16, ptr %927, !tbaa !2
  store i16 %928, ptr %6, !tbaa !2
  %929 = load i16, ptr %6, !tbaa !2
  %930 = sitofp i16 %929 to float
  %931 = fmul float %924, %930
  %932 = call i16 @llvm.lrint.i16.f32(float %931)
  %933 = sub i16 %910, 0
  %934 = getelementptr inbounds i16, ptr @"YP1%", i16 %933
  store i16 %932, ptr %934, !tbaa !2
  %935 = load i16, ptr %105, !tbaa !2
  %936 = load i16, ptr %12, !tbaa !2
  %937 = add nsw i16 %935, %936
  store i16 %937, ptr %105, !tbaa !2
  br label %b101

b110:
  %938 = load i16, ptr %3, !tbaa !2
  %939 = icmp sge i16 %938, 0
  br i1 %939, label %b111, label %b112

b111:
  %940 = load i16, ptr %102, !tbaa !2
  %941 = load i16, ptr %4, !tbaa !2
  %942 = icmp sle i16 %940, %941
  br i1 %942, label %b113, label %b114

b112:
  %943 = load i16, ptr %102, !tbaa !2
  %944 = load i16, ptr %4, !tbaa !2
  %945 = icmp sge i16 %943, %944
  br i1 %945, label %b113, label %b114

b113:
  %946 = load i16, ptr %5, !tbaa !2
  %947 = add i16 -24576, %946
  store i16 %947, ptr @b$seg, !tbaa !2
  %948 = load i16, ptr %5, !tbaa !2
  %949 = add i16 %948, 20
  store i16 %949, ptr %5, !tbaa !2
  %950 = load i16, ptr %102, !tbaa !2
  %951 = getelementptr inbounds i16, ptr @"YP1%", i16 1
  %952 = load i16, ptr %951, !tbaa !2
  %953 = sub i16 %950, %952
  %954 = sub i16 %953, -640
  %955 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %954
  %956 = load i16, ptr %955, !tbaa !2
  %957 = getelementptr inbounds i16, ptr @"YY%", i16 1
  store i16 %956, ptr %957, !tbaa !2
  %958 = load i16, ptr %102, !tbaa !2
  %959 = getelementptr inbounds i16, ptr @"YP1%", i16 2
  %960 = load i16, ptr %959, !tbaa !2
  %961 = sub i16 %958, %960
  %962 = sub i16 %961, -640
  %963 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %962
  %964 = load i16, ptr %963, !tbaa !2
  %965 = getelementptr inbounds i16, ptr @"YY%", i16 2
  store i16 %964, ptr %965, !tbaa !2
  %966 = load i16, ptr %102, !tbaa !2
  %967 = getelementptr inbounds i16, ptr @"YP1%", i16 3
  %968 = load i16, ptr %967, !tbaa !2
  %969 = sub i16 %966, %968
  %970 = sub i16 %969, -640
  %971 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %970
  %972 = load i16, ptr %971, !tbaa !2
  %973 = getelementptr inbounds i16, ptr @"YY%", i16 3
  store i16 %972, ptr %973, !tbaa !2
  %974 = load i16, ptr %102, !tbaa !2
  %975 = getelementptr inbounds i16, ptr @"YP1%", i16 4
  %976 = load i16, ptr %975, !tbaa !2
  %977 = sub i16 %974, %976
  %978 = sub i16 %977, -640
  %979 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %978
  %980 = load i16, ptr %979, !tbaa !2
  %981 = getelementptr inbounds i16, ptr @"YY%", i16 4
  store i16 %980, ptr %981, !tbaa !2
  %982 = load i16, ptr %102, !tbaa !2
  %983 = getelementptr inbounds i16, ptr @"YP1%", i16 5
  %984 = load i16, ptr %983, !tbaa !2
  %985 = sub i16 %982, %984
  %986 = sub i16 %985, -640
  %987 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %986
  %988 = load i16, ptr %987, !tbaa !2
  %989 = getelementptr inbounds i16, ptr @"YY%", i16 5
  store i16 %988, ptr %989, !tbaa !2
  %990 = load i16, ptr %102, !tbaa !2
  %991 = getelementptr inbounds i16, ptr @"YP1%", i16 6
  %992 = load i16, ptr %991, !tbaa !2
  %993 = sub i16 %990, %992
  %994 = sub i16 %993, -640
  %995 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %994
  %996 = load i16, ptr %995, !tbaa !2
  %997 = getelementptr inbounds i16, ptr @"YY%", i16 6
  store i16 %996, ptr %997, !tbaa !2
  %998 = load i16, ptr %102, !tbaa !2
  %999 = getelementptr inbounds i16, ptr @"YP1%", i16 7
  %1000 = load i16, ptr %999, !tbaa !2
  %1001 = sub i16 %998, %1000
  %1002 = sub i16 %1001, -640
  %1003 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1002
  %1004 = load i16, ptr %1003, !tbaa !2
  %1005 = getelementptr inbounds i16, ptr @"YY%", i16 7
  store i16 %1004, ptr %1005, !tbaa !2
  store i16 24, ptr %99, !tbaa !2
  store i16 295, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b115

b114:
  %1006 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1006, ptr @"BENCHFRAME&", !tbaa !2
  %1007 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1007, ptr @"BENCHFRAME&", !tbaa !2
  %1008 = load i16, ptr @"XIT%", !tbaa !2
  %1009 = icmp eq i16 %1008, 0
  br i1 %1009, label %b120, label %b121

b115:
  %1010 = load i16, ptr %1, !tbaa !2
  %1011 = icmp sge i16 %1010, 0
  br i1 %1011, label %b116, label %b117

b116:
  %1012 = load i16, ptr %99, !tbaa !2
  %1013 = load i16, ptr %2, !tbaa !2
  %1014 = icmp sle i16 %1012, %1013
  br i1 %1014, label %b118, label %b119

b117:
  %1015 = load i16, ptr %99, !tbaa !2
  %1016 = load i16, ptr %2, !tbaa !2
  %1017 = icmp sge i16 %1015, %1016
  br i1 %1017, label %b118, label %b119

b118:
  %1018 = getelementptr inbounds i16, ptr @"YY%", i16 1
  %1019 = load i16, ptr %1018, !tbaa !2
  %1020 = load i16, ptr %99, !tbaa !2
  %1021 = getelementptr inbounds i16, ptr @"XP1%", i16 1
  %1022 = load i16, ptr %1021, !tbaa !2
  %1023 = sub i16 %1020, %1022
  %1024 = sub i16 %1023, -640
  %1025 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1024
  %1026 = load i16, ptr %1025, !tbaa !2
  %1027 = add i16 %1019, %1026
  %1028 = sub i16 %1027, 0
  %1029 = getelementptr inbounds i16, ptr @"SP%", i16 %1028
  %1030 = load i16, ptr %1029, !tbaa !2
  %1031 = getelementptr inbounds i16, ptr @"YY%", i16 2
  %1032 = load i16, ptr %1031, !tbaa !2
  %1033 = load i16, ptr %99, !tbaa !2
  %1034 = getelementptr inbounds i16, ptr @"XP1%", i16 2
  %1035 = load i16, ptr %1034, !tbaa !2
  %1036 = sub i16 %1033, %1035
  %1037 = sub i16 %1036, -640
  %1038 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1037
  %1039 = load i16, ptr %1038, !tbaa !2
  %1040 = add i16 %1032, %1039
  %1041 = sub i16 %1040, 0
  %1042 = getelementptr inbounds i16, ptr @"SP%", i16 %1041
  %1043 = load i16, ptr %1042, !tbaa !2
  %1044 = add i16 %1030, %1043
  %1045 = getelementptr inbounds i16, ptr @"YY%", i16 3
  %1046 = load i16, ptr %1045, !tbaa !2
  %1047 = load i16, ptr %99, !tbaa !2
  %1048 = getelementptr inbounds i16, ptr @"XP1%", i16 3
  %1049 = load i16, ptr %1048, !tbaa !2
  %1050 = sub i16 %1047, %1049
  %1051 = sub i16 %1050, -640
  %1052 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1051
  %1053 = load i16, ptr %1052, !tbaa !2
  %1054 = add i16 %1046, %1053
  %1055 = sub i16 %1054, 0
  %1056 = getelementptr inbounds i16, ptr @"SP%", i16 %1055
  %1057 = load i16, ptr %1056, !tbaa !2
  %1058 = add i16 %1044, %1057
  %1059 = getelementptr inbounds i16, ptr @"YY%", i16 4
  %1060 = load i16, ptr %1059, !tbaa !2
  %1061 = load i16, ptr %99, !tbaa !2
  %1062 = getelementptr inbounds i16, ptr @"XP1%", i16 4
  %1063 = load i16, ptr %1062, !tbaa !2
  %1064 = sub i16 %1061, %1063
  %1065 = sub i16 %1064, -640
  %1066 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1065
  %1067 = load i16, ptr %1066, !tbaa !2
  %1068 = add i16 %1060, %1067
  %1069 = sub i16 %1068, 0
  %1070 = getelementptr inbounds i16, ptr @"SP%", i16 %1069
  %1071 = load i16, ptr %1070, !tbaa !2
  %1072 = add i16 %1058, %1071
  %1073 = getelementptr inbounds i16, ptr @"YY%", i16 5
  %1074 = load i16, ptr %1073, !tbaa !2
  %1075 = load i16, ptr %99, !tbaa !2
  %1076 = getelementptr inbounds i16, ptr @"XP1%", i16 5
  %1077 = load i16, ptr %1076, !tbaa !2
  %1078 = sub i16 %1075, %1077
  %1079 = sub i16 %1078, -640
  %1080 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1079
  %1081 = load i16, ptr %1080, !tbaa !2
  %1082 = add i16 %1074, %1081
  %1083 = sub i16 %1082, 0
  %1084 = getelementptr inbounds i16, ptr @"SP%", i16 %1083
  %1085 = load i16, ptr %1084, !tbaa !2
  %1086 = add i16 %1072, %1085
  %1087 = getelementptr inbounds i16, ptr @"YY%", i16 6
  %1088 = load i16, ptr %1087, !tbaa !2
  %1089 = load i16, ptr %99, !tbaa !2
  %1090 = getelementptr inbounds i16, ptr @"XP1%", i16 6
  %1091 = load i16, ptr %1090, !tbaa !2
  %1092 = sub i16 %1089, %1091
  %1093 = sub i16 %1092, -640
  %1094 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1093
  %1095 = load i16, ptr %1094, !tbaa !2
  %1096 = add i16 %1088, %1095
  %1097 = sub i16 %1096, 0
  %1098 = getelementptr inbounds i16, ptr @"SP%", i16 %1097
  %1099 = load i16, ptr %1098, !tbaa !2
  %1100 = add i16 %1086, %1099
  %1101 = getelementptr inbounds i16, ptr @"YY%", i16 7
  %1102 = load i16, ptr %1101, !tbaa !2
  %1103 = load i16, ptr %99, !tbaa !2
  %1104 = getelementptr inbounds i16, ptr @"XP1%", i16 7
  %1105 = load i16, ptr %1104, !tbaa !2
  %1106 = sub i16 %1103, %1105
  %1107 = sub i16 %1106, -640
  %1108 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1107
  %1109 = load i16, ptr %1108, !tbaa !2
  %1110 = add i16 %1102, %1109
  %1111 = sub i16 %1110, 0
  %1112 = getelementptr inbounds i16, ptr @"SP%", i16 %1111
  %1113 = load i16, ptr %1112, !tbaa !2
  %1114 = add i16 %1100, %1113
  store i16 %1114, ptr %0, !tbaa !2
  %1115 = load i16, ptr %99, !tbaa !2
  %1116 = load i16, ptr %0, !tbaa !2
  %1117 = sub i16 %1116, 0
  %1118 = getelementptr inbounds i16, ptr @"CD%", i16 %1117
  %1119 = load i16, ptr %1118, !tbaa !2
  %1120 = trunc i16 %1119 to i8
  %1121 = load i16, ptr @b$seg, !tbaa !2
  %1122 = inttoptr i16 %1121 to ptr addrspace(2)
  %1123 = addrspacecast ptr addrspace(2) %1122 to ptr addrspace(1)
  %1124 = getelementptr i8, ptr addrspace(1) %1123, i16 %1115
  store volatile i8 %1120, ptr addrspace(1) %1124
  %1125 = load i16, ptr %99, !tbaa !2
  %1126 = load i16, ptr %1, !tbaa !2
  %1127 = add nsw i16 %1125, %1126
  store i16 %1127, ptr %99, !tbaa !2
  br label %b115

b119:
  %1128 = load i16, ptr %102, !tbaa !2
  %1129 = load i16, ptr %3, !tbaa !2
  %1130 = add nsw i16 %1128, %1129
  store i16 %1130, ptr %102, !tbaa !2
  br label %b110

b120:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b122

b121:
  br label %b122

b122:
  %1131 = load i16, ptr @"XIT%", !tbaa !2
  %1132 = icmp eq i16 %1131, 1
  br i1 %1132, label %b123, label %b124

b123:
  br label %b2

b124:
  br label %b125

b125:
  br label %b68
}

define cc 1000 void @DEEDLINESAX(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca i16
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca i16
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca i16
  %79 = alloca i16
  %80 = alloca i16
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca i16
  %84 = alloca i16
  %85 = alloca i16
  %86 = alloca i16
  %87 = alloca i16
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca i16
  %91 = alloca i16
  %92 = alloca i16
  %93 = alloca i16
  %94 = alloca i16
  %95 = alloca i16
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 20, i16 20)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 25, i16 80)
  %103 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %103, i16 -1, i16 0)
  store i16 10, ptr %102, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %102)
  %104 = load i16, ptr @"XIT%", !tbaa !2
  %105 = icmp eq i16 %104, 1
  br i1 %105, label %b3, label %b4

b2:
  ret void

b3:
  br label %b2

b4:
  br label %b5

b5:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 25, i16 80)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 60, i16 70)
  %106 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %106, i16 -1, i16 0)
  store i16 10, ptr %101, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %101)
  %107 = load i16, ptr @"XIT%", !tbaa !2
  %108 = icmp eq i16 %107, 1
  br i1 %108, label %b7, label %b8

b7:
  br label %b2

b8:
  br label %b9

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 60, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 55, i16 30)
  %109 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %109, i16 -1, i16 0)
  store i16 10, ptr %100, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %100)
  %110 = load i16, ptr @"XIT%", !tbaa !2
  %111 = icmp eq i16 %110, 1
  br i1 %111, label %b11, label %b12

b11:
  br label %b2

b12:
  br label %b13

b13:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 55, i16 30)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 20, i16 20)
  %112 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %112, i16 -1, i16 0)
  store i16 10, ptr %99, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %99)
  %113 = load i16, ptr @"XIT%", !tbaa !2
  %114 = icmp eq i16 %113, 1
  br i1 %114, label %b15, label %b16

b15:
  br label %b2

b16:
  br label %b17

b17:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 25, i16 25)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 30, i16 60)
  %115 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %115, i16 -1, i16 0)
  store i16 10, ptr %98, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %98)
  %116 = load i16, ptr @"XIT%", !tbaa !2
  %117 = icmp eq i16 %116, 1
  br i1 %117, label %b19, label %b20

b19:
  br label %b2

b20:
  br label %b21

b21:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 30, i16 60)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 50, i16 65)
  %118 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %118, i16 -1, i16 0)
  store i16 10, ptr %97, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %97)
  %119 = load i16, ptr @"XIT%", !tbaa !2
  %120 = icmp eq i16 %119, 1
  br i1 %120, label %b23, label %b24

b23:
  br label %b2

b24:
  br label %b25

b25:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 50, i16 65)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 25, i16 25)
  %121 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %121, i16 -1, i16 0)
  store i16 10, ptr %96, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %96)
  %122 = load i16, ptr @"XIT%", !tbaa !2
  %123 = icmp eq i16 %122, 1
  br i1 %123, label %b27, label %b28

b27:
  br label %b2

b28:
  br label %b29

b29:
  %124 = load i16, ptr %0
  %125 = icmp eq i16 %124, 15
  br i1 %125, label %b31, label %b32

b31:
  %126 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %127 = load float, ptr %126
  store i16 255, ptr %94, !tbaa !2
  %128 = load i16, ptr %94, !tbaa !2
  %129 = sitofp i16 %128 to float
  %130 = fmul float %127, %129
  %131 = call float @llvm.rint.f32(float %130)
  %132 = fcmp olt float %130, %131
  %133 = sext i1 %132 to i16
  store i16 %133, ptr %93, !tbaa !2
  %134 = load i16, ptr %93, !tbaa !2
  %135 = sitofp i16 %134 to float
  %136 = fadd float %131, %135
  %137 = call i16 @llvm.lrint.i16.f32(float %136)
  store i16 %137, ptr %95, !tbaa !2
  br label %b33

b32:
  store i16 0, ptr %95, !tbaa !2
  br label %b33

b33:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 23, i16 23)
  %138 = load i16, ptr %95, !tbaa !2
  %139 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %138, i16 %139)
  store i16 10, ptr %92, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %92)
  %140 = load i16, ptr @"XIT%", !tbaa !2
  %141 = icmp eq i16 %140, 1
  br i1 %141, label %b34, label %b35

b34:
  br label %b2

b35:
  br label %b36

b36:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 65, i16 20)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 100, i16 24)
  %142 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %142, i16 -1, i16 0)
  store i16 10, ptr %91, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %91)
  %143 = load i16, ptr @"XIT%", !tbaa !2
  %144 = icmp eq i16 %143, 1
  br i1 %144, label %b38, label %b39

b38:
  br label %b2

b39:
  br label %b40

b40:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 100, i16 24)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 70, i16 34)
  %145 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %145, i16 -1, i16 0)
  store i16 10, ptr %90, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %90)
  %146 = load i16, ptr @"XIT%", !tbaa !2
  %147 = icmp eq i16 %146, 1
  br i1 %147, label %b42, label %b43

b42:
  br label %b2

b43:
  br label %b44

b44:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 70, i16 34)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 85, i16 44)
  %148 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %148, i16 -1, i16 0)
  store i16 10, ptr %89, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %89)
  %149 = load i16, ptr @"XIT%", !tbaa !2
  %150 = icmp eq i16 %149, 1
  br i1 %150, label %b46, label %b47

b46:
  br label %b2

b47:
  br label %b48

b48:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 85, i16 44)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 75, i16 54)
  %151 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %151, i16 -1, i16 0)
  store i16 10, ptr %88, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %88)
  %152 = load i16, ptr @"XIT%", !tbaa !2
  %153 = icmp eq i16 %152, 1
  br i1 %153, label %b50, label %b51

b50:
  br label %b2

b51:
  br label %b52

b52:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 75, i16 54)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 75, i16 64)
  %154 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %154, i16 -1, i16 0)
  store i16 10, ptr %87, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %87)
  %155 = load i16, ptr @"XIT%", !tbaa !2
  %156 = icmp eq i16 %155, 1
  br i1 %156, label %b54, label %b55

b54:
  br label %b2

b55:
  br label %b56

b56:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 75, i16 64)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 85, i16 64)
  %157 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %157, i16 -1, i16 0)
  store i16 10, ptr %86, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %86)
  %158 = load i16, ptr @"XIT%", !tbaa !2
  %159 = icmp eq i16 %158, 1
  br i1 %159, label %b58, label %b59

b58:
  br label %b2

b59:
  br label %b60

b60:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 85, i16 64)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 70, i16 70)
  %160 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %160, i16 -1, i16 0)
  store i16 10, ptr %85, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %85)
  %161 = load i16, ptr @"XIT%", !tbaa !2
  %162 = icmp eq i16 %161, 1
  br i1 %162, label %b62, label %b63

b62:
  br label %b2

b63:
  br label %b64

b64:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 70, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 65, i16 20)
  %163 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %163, i16 -1, i16 0)
  store i16 10, ptr %84, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %84)
  %164 = load i16, ptr @"XIT%", !tbaa !2
  %165 = icmp eq i16 %164, 1
  br i1 %165, label %b66, label %b67

b66:
  br label %b2

b67:
  br label %b68

b68:
  %166 = load i16, ptr %0
  %167 = icmp eq i16 %166, 15
  br i1 %167, label %b70, label %b71

b70:
  %168 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %169 = load float, ptr %168
  store i16 255, ptr %83, !tbaa !2
  %170 = load i16, ptr %83, !tbaa !2
  %171 = sitofp i16 %170 to float
  %172 = fmul float %169, %171
  %173 = call float @llvm.rint.f32(float %172)
  %174 = fcmp olt float %172, %173
  %175 = sext i1 %174 to i16
  store i16 %175, ptr %82, !tbaa !2
  %176 = load i16, ptr %82, !tbaa !2
  %177 = sitofp i16 %176 to float
  %178 = fadd float %173, %177
  %179 = call i16 @llvm.lrint.i16.f32(float %178)
  store i16 %179, ptr %95, !tbaa !2
  br label %b72

b71:
  store i16 0, ptr %95, !tbaa !2
  br label %b72

b72:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 66, i16 21)
  %180 = load i16, ptr %95, !tbaa !2
  %181 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %180, i16 %181)
  store i16 10, ptr %81, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %81)
  %182 = load i16, ptr @"XIT%", !tbaa !2
  %183 = icmp eq i16 %182, 1
  br i1 %183, label %b73, label %b74

b73:
  br label %b2

b74:
  br label %b75

b75:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 30)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 130, i16 30)
  %184 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %184, i16 -1, i16 0)
  store i16 10, ptr %80, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %80)
  %185 = load i16, ptr @"XIT%", !tbaa !2
  %186 = icmp eq i16 %185, 1
  br i1 %186, label %b77, label %b78

b77:
  br label %b2

b78:
  br label %b79

b79:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 130, i16 30)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 40)
  %187 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %187, i16 -1, i16 0)
  store i16 10, ptr %79, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %79)
  %188 = load i16, ptr @"XIT%", !tbaa !2
  %189 = icmp eq i16 %188, 1
  br i1 %189, label %b81, label %b82

b81:
  br label %b2

b82:
  br label %b83

b83:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 40)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 50)
  %190 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %190, i16 -1, i16 0)
  store i16 10, ptr %78, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %78)
  %191 = load i16, ptr @"XIT%", !tbaa !2
  %192 = icmp eq i16 %191, 1
  br i1 %192, label %b85, label %b86

b85:
  br label %b2

b86:
  br label %b87

b87:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 50)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 125, i16 50)
  %193 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %193, i16 -1, i16 0)
  store i16 10, ptr %77, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %77)
  %194 = load i16, ptr @"XIT%", !tbaa !2
  %195 = icmp eq i16 %194, 1
  br i1 %195, label %b89, label %b90

b89:
  br label %b2

b90:
  br label %b91

b91:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 125, i16 50)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 125, i16 55)
  %196 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %196, i16 -1, i16 0)
  store i16 10, ptr %76, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %76)
  %197 = load i16, ptr @"XIT%", !tbaa !2
  %198 = icmp eq i16 %197, 1
  br i1 %198, label %b93, label %b94

b93:
  br label %b2

b94:
  br label %b95

b95:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 125, i16 55)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 55)
  %199 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %199, i16 -1, i16 0)
  store i16 10, ptr %75, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %75)
  %200 = load i16, ptr @"XIT%", !tbaa !2
  %201 = icmp eq i16 %200, 1
  br i1 %201, label %b97, label %b98

b97:
  br label %b2

b98:
  br label %b99

b99:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 55)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 65)
  %202 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %202, i16 -1, i16 0)
  store i16 10, ptr %74, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %74)
  %203 = load i16, ptr @"XIT%", !tbaa !2
  %204 = icmp eq i16 %203, 1
  br i1 %204, label %b101, label %b102

b101:
  br label %b2

b102:
  br label %b103

b103:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 65)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 120, i16 65)
  %205 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %205, i16 -1, i16 0)
  store i16 10, ptr %73, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %73)
  %206 = load i16, ptr @"XIT%", !tbaa !2
  %207 = icmp eq i16 %206, 1
  br i1 %207, label %b105, label %b106

b105:
  br label %b2

b106:
  br label %b107

b107:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 120, i16 65)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 120, i16 70)
  %208 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %208, i16 -1, i16 0)
  store i16 10, ptr %72, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %72)
  %209 = load i16, ptr @"XIT%", !tbaa !2
  %210 = icmp eq i16 %209, 1
  br i1 %210, label %b109, label %b110

b109:
  br label %b2

b110:
  br label %b111

b111:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 120, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 100, i16 70)
  %211 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %211, i16 -1, i16 0)
  store i16 10, ptr %71, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %71)
  %212 = load i16, ptr @"XIT%", !tbaa !2
  %213 = icmp eq i16 %212, 1
  br i1 %213, label %b113, label %b114

b113:
  br label %b2

b114:
  br label %b115

b115:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 100, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 30)
  %214 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %214, i16 -1, i16 0)
  store i16 10, ptr %70, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %70)
  %215 = load i16, ptr @"XIT%", !tbaa !2
  %216 = icmp eq i16 %215, 1
  br i1 %216, label %b117, label %b118

b117:
  br label %b2

b118:
  br label %b119

b119:
  %217 = load i16, ptr %0
  %218 = icmp eq i16 %217, 15
  br i1 %218, label %b121, label %b122

b121:
  %219 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %220 = load float, ptr %219
  store i16 255, ptr %69, !tbaa !2
  %221 = load i16, ptr %69, !tbaa !2
  %222 = sitofp i16 %221 to float
  %223 = fmul float %220, %222
  %224 = call float @llvm.rint.f32(float %223)
  %225 = fcmp olt float %223, %224
  %226 = sext i1 %225 to i16
  store i16 %226, ptr %68, !tbaa !2
  %227 = load i16, ptr %68, !tbaa !2
  %228 = sitofp i16 %227 to float
  %229 = fadd float %224, %228
  %230 = call i16 @llvm.lrint.i16.f32(float %229)
  store i16 %230, ptr %95, !tbaa !2
  br label %b123

b122:
  store i16 0, ptr %95, !tbaa !2
  br label %b123

b123:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 111, i16 31)
  %231 = load i16, ptr %95, !tbaa !2
  %232 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %231, i16 %232)
  store i16 10, ptr %67, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %67)
  %233 = load i16, ptr @"XIT%", !tbaa !2
  %234 = icmp eq i16 %233, 1
  br i1 %234, label %b124, label %b125

b124:
  br label %b2

b125:
  br label %b126

b126:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 150, i16 40)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 130, i16 20)
  %235 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %235, i16 -1, i16 0)
  store i16 10, ptr %66, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %66)
  %236 = load i16, ptr @"XIT%", !tbaa !2
  %237 = icmp eq i16 %236, 1
  br i1 %237, label %b128, label %b129

b128:
  br label %b2

b129:
  br label %b130

b130:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 130, i16 20)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 140, i16 70)
  %238 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %238, i16 -1, i16 0)
  store i16 10, ptr %65, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %65)
  %239 = load i16, ptr @"XIT%", !tbaa !2
  %240 = icmp eq i16 %239, 1
  br i1 %240, label %b132, label %b133

b132:
  br label %b2

b133:
  br label %b134

b134:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 140, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 150, i16 40)
  %241 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %241, i16 -1, i16 0)
  store i16 10, ptr %64, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %64)
  %242 = load i16, ptr @"XIT%", !tbaa !2
  %243 = icmp eq i16 %242, 1
  br i1 %243, label %b136, label %b137

b136:
  br label %b2

b137:
  br label %b138

b138:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 135, i16 30)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 140, i16 60)
  %244 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %244, i16 -1, i16 0)
  store i16 10, ptr %63, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %63)
  %245 = load i16, ptr @"XIT%", !tbaa !2
  %246 = icmp eq i16 %245, 1
  br i1 %246, label %b140, label %b141

b140:
  br label %b2

b141:
  br label %b142

b142:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 140, i16 60)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 145, i16 40)
  %247 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %247, i16 -1, i16 0)
  store i16 10, ptr %62, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %62)
  %248 = load i16, ptr @"XIT%", !tbaa !2
  %249 = icmp eq i16 %248, 1
  br i1 %249, label %b144, label %b145

b144:
  br label %b2

b145:
  br label %b146

b146:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 145, i16 40)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 135, i16 30)
  %250 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %250, i16 -1, i16 0)
  store i16 10, ptr %61, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %61)
  %251 = load i16, ptr @"XIT%", !tbaa !2
  %252 = icmp eq i16 %251, 1
  br i1 %252, label %b148, label %b149

b148:
  br label %b2

b149:
  br label %b150

b150:
  %253 = load i16, ptr %0
  %254 = icmp eq i16 %253, 15
  br i1 %254, label %b152, label %b153

b152:
  %255 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %256 = load float, ptr %255
  store i16 255, ptr %60, !tbaa !2
  %257 = load i16, ptr %60, !tbaa !2
  %258 = sitofp i16 %257 to float
  %259 = fmul float %256, %258
  %260 = call float @llvm.rint.f32(float %259)
  %261 = fcmp olt float %259, %260
  %262 = sext i1 %261 to i16
  store i16 %262, ptr %59, !tbaa !2
  %263 = load i16, ptr %59, !tbaa !2
  %264 = sitofp i16 %263 to float
  %265 = fadd float %260, %264
  %266 = call i16 @llvm.lrint.i16.f32(float %265)
  store i16 %266, ptr %95, !tbaa !2
  br label %b154

b153:
  store i16 0, ptr %95, !tbaa !2
  br label %b154

b154:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 133, i16 25)
  %267 = load i16, ptr %95, !tbaa !2
  %268 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %267, i16 %268)
  store i16 10, ptr %58, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %58)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 160, i16 30)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 165, i16 70)
  %269 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %269, i16 -1, i16 0)
  store i16 10, ptr %57, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %57)
  %270 = load i16, ptr @"XIT%", !tbaa !2
  %271 = icmp eq i16 %270, 1
  br i1 %271, label %b155, label %b156

b155:
  br label %b2

b156:
  br label %b157

b157:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 165, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 190, i16 70)
  %272 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %272, i16 -1, i16 0)
  store i16 10, ptr %56, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %56)
  %273 = load i16, ptr @"XIT%", !tbaa !2
  %274 = icmp eq i16 %273, 1
  br i1 %274, label %b159, label %b160

b159:
  br label %b2

b160:
  br label %b161

b161:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 190, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 175, i16 65)
  %275 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %275, i16 -1, i16 0)
  store i16 10, ptr %55, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %55)
  %276 = load i16, ptr @"XIT%", !tbaa !2
  %277 = icmp eq i16 %276, 1
  br i1 %277, label %b163, label %b164

b163:
  br label %b2

b164:
  br label %b165

b165:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 175, i16 65)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 160, i16 30)
  %278 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %278, i16 -1, i16 0)
  store i16 10, ptr %54, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %54)
  %279 = load i16, ptr @"XIT%", !tbaa !2
  %280 = icmp eq i16 %279, 1
  br i1 %280, label %b167, label %b168

b167:
  br label %b2

b168:
  br label %b169

b169:
  %281 = load i16, ptr %0
  %282 = icmp eq i16 %281, 15
  br i1 %282, label %b171, label %b172

b171:
  %283 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %284 = load float, ptr %283
  store i16 255, ptr %53, !tbaa !2
  %285 = load i16, ptr %53, !tbaa !2
  %286 = sitofp i16 %285 to float
  %287 = fmul float %284, %286
  %288 = call float @llvm.rint.f32(float %287)
  %289 = fcmp olt float %287, %288
  %290 = sext i1 %289 to i16
  store i16 %290, ptr %52, !tbaa !2
  %291 = load i16, ptr %52, !tbaa !2
  %292 = sitofp i16 %291 to float
  %293 = fadd float %288, %292
  %294 = call i16 @llvm.lrint.i16.f32(float %293)
  store i16 %294, ptr %95, !tbaa !2
  br label %b173

b172:
  store i16 0, ptr %95, !tbaa !2
  br label %b173

b173:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 164, i16 40)
  %295 = load i16, ptr %95, !tbaa !2
  %296 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %295, i16 %296)
  store i16 10, ptr %51, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %51)
  %297 = load i16, ptr @"XIT%", !tbaa !2
  %298 = icmp eq i16 %297, 1
  br i1 %298, label %b174, label %b175

b174:
  br label %b2

b175:
  br label %b176

b176:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 180, i16 20)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 190, i16 60)
  %299 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %299, i16 -1, i16 2)
  store i16 10, ptr %50, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %50)
  %300 = load i16, ptr @"XIT%", !tbaa !2
  %301 = icmp eq i16 %300, 1
  br i1 %301, label %b178, label %b179

b178:
  br label %b2

b179:
  br label %b180

b180:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 195, i16 20)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 195, i16 70)
  %302 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %302, i16 -1, i16 0)
  store i16 10, ptr %49, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %49)
  %303 = load i16, ptr @"XIT%", !tbaa !2
  %304 = icmp eq i16 %303, 1
  br i1 %304, label %b182, label %b183

b182:
  br label %b2

b183:
  br label %b184

b184:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 195, i16 70)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 200, i16 40)
  %305 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %305, i16 -1, i16 0)
  store i16 10, ptr %48, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %48)
  %306 = load i16, ptr @"XIT%", !tbaa !2
  %307 = icmp eq i16 %306, 1
  br i1 %307, label %b186, label %b187

b186:
  br label %b2

b187:
  br label %b188

b188:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 200, i16 40)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 220, i16 65)
  %308 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %308, i16 -1, i16 0)
  store i16 10, ptr %47, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %47)
  %309 = load i16, ptr @"XIT%", !tbaa !2
  %310 = icmp eq i16 %309, 1
  br i1 %310, label %b190, label %b191

b190:
  br label %b2

b191:
  br label %b192

b192:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 220, i16 65)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 220, i16 35)
  %311 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %311, i16 -1, i16 0)
  store i16 10, ptr %46, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %46)
  %312 = load i16, ptr @"XIT%", !tbaa !2
  %313 = icmp eq i16 %312, 1
  br i1 %313, label %b194, label %b195

b194:
  br label %b2

b195:
  br label %b196

b196:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 220, i16 35)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 215, i16 55)
  %314 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %314, i16 -1, i16 0)
  store i16 10, ptr %45, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %45)
  %315 = load i16, ptr @"XIT%", !tbaa !2
  %316 = icmp eq i16 %315, 1
  br i1 %316, label %b198, label %b199

b198:
  br label %b2

b199:
  br label %b200

b200:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 215, i16 55)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 195, i16 20)
  %317 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %317, i16 -1, i16 0)
  store i16 10, ptr %44, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %44)
  %318 = load i16, ptr @"XIT%", !tbaa !2
  %319 = icmp eq i16 %318, 1
  br i1 %319, label %b202, label %b203

b202:
  br label %b2

b203:
  br label %b204

b204:
  %320 = load i16, ptr %0
  %321 = icmp eq i16 %320, 15
  br i1 %321, label %b206, label %b207

b206:
  %322 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %323 = load float, ptr %322
  store i16 255, ptr %43, !tbaa !2
  %324 = load i16, ptr %43, !tbaa !2
  %325 = sitofp i16 %324 to float
  %326 = fmul float %323, %325
  %327 = call float @llvm.rint.f32(float %326)
  %328 = fcmp olt float %326, %327
  %329 = sext i1 %328 to i16
  store i16 %329, ptr %42, !tbaa !2
  %330 = load i16, ptr %42, !tbaa !2
  %331 = sitofp i16 %330 to float
  %332 = fadd float %327, %331
  %333 = call i16 @llvm.lrint.i16.f32(float %332)
  store i16 %333, ptr %95, !tbaa !2
  br label %b208

b207:
  store i16 0, ptr %95, !tbaa !2
  br label %b208

b208:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 196, i16 30)
  %334 = load i16, ptr %95, !tbaa !2
  %335 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %334, i16 %335)
  store i16 10, ptr %41, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %41)
  %336 = load i16, ptr @"XIT%", !tbaa !2
  %337 = icmp eq i16 %336, 1
  br i1 %337, label %b209, label %b210

b209:
  br label %b2

b210:
  br label %b211

b211:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 245, i16 20)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 225, i16 20)
  %338 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %338, i16 -1, i16 0)
  store i16 10, ptr %40, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %40)
  %339 = load i16, ptr @"XIT%", !tbaa !2
  %340 = icmp eq i16 %339, 1
  br i1 %340, label %b213, label %b214

b213:
  br label %b2

b214:
  br label %b215

b215:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 225, i16 20)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 225, i16 60)
  %341 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %341, i16 -1, i16 0)
  store i16 10, ptr %39, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %39)
  %342 = load i16, ptr @"XIT%", !tbaa !2
  %343 = icmp eq i16 %342, 1
  br i1 %343, label %b217, label %b218

b217:
  br label %b2

b218:
  br label %b219

b219:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 225, i16 60)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 250, i16 60)
  %344 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %344, i16 -1, i16 0)
  store i16 10, ptr %38, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %38)
  %345 = load i16, ptr @"XIT%", !tbaa !2
  %346 = icmp eq i16 %345, 1
  br i1 %346, label %b221, label %b222

b221:
  br label %b2

b222:
  br label %b223

b223:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 250, i16 60)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 250, i16 50)
  %347 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %347, i16 -1, i16 0)
  store i16 10, ptr %37, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %37)
  %348 = load i16, ptr @"XIT%", !tbaa !2
  %349 = icmp eq i16 %348, 1
  br i1 %349, label %b225, label %b226

b225:
  br label %b2

b226:
  br label %b227

b227:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 250, i16 50)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 230, i16 50)
  %350 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %350, i16 -1, i16 0)
  store i16 10, ptr %36, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %36)
  %351 = load i16, ptr @"XIT%", !tbaa !2
  %352 = icmp eq i16 %351, 1
  br i1 %352, label %b229, label %b230

b229:
  br label %b2

b230:
  br label %b231

b231:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 230, i16 50)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 230, i16 40)
  %353 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %353, i16 -1, i16 0)
  store i16 10, ptr %35, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %35)
  %354 = load i16, ptr @"XIT%", !tbaa !2
  %355 = icmp eq i16 %354, 1
  br i1 %355, label %b233, label %b234

b233:
  br label %b2

b234:
  br label %b235

b235:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 230, i16 40)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 240, i16 40)
  %356 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %356, i16 -1, i16 0)
  store i16 10, ptr %34, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %34)
  %357 = load i16, ptr @"XIT%", !tbaa !2
  %358 = icmp eq i16 %357, 1
  br i1 %358, label %b237, label %b238

b237:
  br label %b2

b238:
  br label %b239

b239:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 240, i16 40)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 240, i16 35)
  %359 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %359, i16 -1, i16 0)
  store i16 10, ptr %33, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %33)
  %360 = load i16, ptr @"XIT%", !tbaa !2
  %361 = icmp eq i16 %360, 1
  br i1 %361, label %b241, label %b242

b241:
  br label %b2

b242:
  br label %b243

b243:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 240, i16 35)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 228, i16 35)
  %362 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %362, i16 -1, i16 0)
  store i16 10, ptr %32, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %32)
  %363 = load i16, ptr @"XIT%", !tbaa !2
  %364 = icmp eq i16 %363, 1
  br i1 %364, label %b245, label %b246

b245:
  br label %b2

b246:
  br label %b247

b247:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 228, i16 35)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 230, i16 25)
  %365 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %365, i16 -1, i16 0)
  store i16 10, ptr %31, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %31)
  %366 = load i16, ptr @"XIT%", !tbaa !2
  %367 = icmp eq i16 %366, 1
  br i1 %367, label %b249, label %b250

b249:
  br label %b2

b250:
  br label %b251

b251:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 230, i16 25)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 245, i16 20)
  %368 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %368, i16 -1, i16 0)
  store i16 10, ptr %30, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %30)
  %369 = load i16, ptr @"XIT%", !tbaa !2
  %370 = icmp eq i16 %369, 1
  br i1 %370, label %b253, label %b254

b253:
  br label %b2

b254:
  br label %b255

b255:
  %371 = load i16, ptr %0
  %372 = icmp eq i16 %371, 15
  br i1 %372, label %b257, label %b258

b257:
  %373 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %374 = load float, ptr %373
  store i16 255, ptr %29, !tbaa !2
  %375 = load i16, ptr %29, !tbaa !2
  %376 = sitofp i16 %375 to float
  %377 = fmul float %374, %376
  %378 = call float @llvm.rint.f32(float %377)
  %379 = fcmp olt float %377, %378
  %380 = sext i1 %379 to i16
  store i16 %380, ptr %28, !tbaa !2
  %381 = load i16, ptr %28, !tbaa !2
  %382 = sitofp i16 %381 to float
  %383 = fadd float %378, %382
  %384 = call i16 @llvm.lrint.i16.f32(float %383)
  store i16 %384, ptr %95, !tbaa !2
  br label %b259

b258:
  store i16 0, ptr %95, !tbaa !2
  br label %b259

b259:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 227, i16 24)
  %385 = load i16, ptr %95, !tbaa !2
  %386 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %385, i16 %386)
  store i16 10, ptr %27, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %27)
  %387 = load i16, ptr @"XIT%", !tbaa !2
  %388 = icmp eq i16 %387, 1
  br i1 %388, label %b260, label %b261

b260:
  br label %b2

b261:
  br label %b262

b262:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 300, i16 5)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 260, i16 8)
  %389 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %389, i16 -1, i16 0)
  store i16 10, ptr %26, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %26)
  %390 = load i16, ptr @"XIT%", !tbaa !2
  %391 = icmp eq i16 %390, 1
  br i1 %391, label %b264, label %b265

b264:
  br label %b2

b265:
  br label %b266

b266:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 260, i16 8)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 290, i16 58)
  %392 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %392, i16 -1, i16 0)
  store i16 10, ptr %25, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %25)
  %393 = load i16, ptr @"XIT%", !tbaa !2
  %394 = icmp eq i16 %393, 1
  br i1 %394, label %b268, label %b269

b268:
  br label %b2

b269:
  br label %b270

b270:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 290, i16 58)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 250, i16 68)
  %395 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %395, i16 -1, i16 0)
  store i16 10, ptr %24, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %24)
  %396 = load i16, ptr @"XIT%", !tbaa !2
  %397 = icmp eq i16 %396, 1
  br i1 %397, label %b272, label %b273

b272:
  br label %b2

b273:
  br label %b274

b274:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 250, i16 68)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 305, i16 64)
  %398 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %398, i16 -1, i16 0)
  store i16 10, ptr %23, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %23)
  %399 = load i16, ptr @"XIT%", !tbaa !2
  %400 = icmp eq i16 %399, 1
  br i1 %400, label %b276, label %b277

b276:
  br label %b2

b277:
  br label %b278

b278:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 305, i16 64)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 270, i16 18)
  %401 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %401, i16 -1, i16 0)
  store i16 10, ptr %22, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %22)
  %402 = load i16, ptr @"XIT%", !tbaa !2
  %403 = icmp eq i16 %402, 1
  br i1 %403, label %b280, label %b281

b280:
  br label %b2

b281:
  br label %b282

b282:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 270, i16 18)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 300, i16 5)
  %404 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %404, i16 -1, i16 0)
  store i16 10, ptr %21, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %21)
  %405 = load i16, ptr @"XIT%", !tbaa !2
  %406 = icmp eq i16 %405, 1
  br i1 %406, label %b284, label %b285

b284:
  br label %b2

b285:
  br label %b286

b286:
  %407 = load i16, ptr %0
  %408 = icmp eq i16 %407, 15
  br i1 %408, label %b288, label %b289

b288:
  %409 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %410 = load float, ptr %409
  store i16 255, ptr %20, !tbaa !2
  %411 = load i16, ptr %20, !tbaa !2
  %412 = sitofp i16 %411 to float
  %413 = fmul float %410, %412
  %414 = call float @llvm.rint.f32(float %413)
  %415 = fcmp olt float %413, %414
  %416 = sext i1 %415 to i16
  store i16 %416, ptr %19, !tbaa !2
  %417 = load i16, ptr %19, !tbaa !2
  %418 = sitofp i16 %417 to float
  %419 = fadd float %414, %418
  %420 = call i16 @llvm.lrint.i16.f32(float %419)
  store i16 %420, ptr %95, !tbaa !2
  br label %b290

b289:
  store i16 0, ptr %95, !tbaa !2
  br label %b290

b290:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 265, i16 10)
  %421 = load i16, ptr %95, !tbaa !2
  %422 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %421, i16 %422)
  store i16 10, ptr %18, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %18)
  %423 = load i16, ptr @"XIT%", !tbaa !2
  %424 = icmp eq i16 %423, 1
  br i1 %424, label %b291, label %b292

b291:
  br label %b2

b292:
  br label %b293

b293:
  %425 = load i16, ptr %0
  %426 = sext i16 %425 to i32
  %427 = sdiv i32 %426, 15
  %428 = trunc i32 %427 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 %428, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 1, i16 1, i16 1, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string221)
  store i16 70, ptr %17, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %17)
  %429 = load i16, ptr @"XIT%", !tbaa !2
  %430 = icmp eq i16 %429, 1
  br i1 %430, label %b295, label %b296

b295:
  br label %b2

b296:
  br label %b297

b297:
  store i16 0, ptr %16, !tbaa !2
  store i16 63, ptr %15, !tbaa !2
  store i16 1, ptr %14, !tbaa !2
  br label %b299

b299:
  %431 = load i16, ptr %14, !tbaa !2
  %432 = icmp sge i16 %431, 0
  br i1 %432, label %b300, label %b301

b300:
  %433 = load i16, ptr %16, !tbaa !2
  %434 = load i16, ptr %15, !tbaa !2
  %435 = icmp sle i16 %433, %434
  br i1 %435, label %b302, label %b303

b301:
  %436 = load i16, ptr %16, !tbaa !2
  %437 = load i16, ptr %15, !tbaa !2
  %438 = icmp sge i16 %436, %437
  br i1 %438, label %b302, label %b303

b302:
  store i16 0, ptr %13, !tbaa !2
  store i16 255, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b304

b303:
  store i16 140, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %1)
  %439 = load i16, ptr @"XIT%", !tbaa !2
  %440 = icmp eq i16 %439, 1
  br i1 %440, label %b313, label %b314

b304:
  %441 = load i16, ptr %11, !tbaa !2
  %442 = icmp sge i16 %441, 0
  br i1 %442, label %b305, label %b306

b305:
  %443 = load i16, ptr %13, !tbaa !2
  %444 = load i16, ptr %12, !tbaa !2
  %445 = icmp sle i16 %443, %444
  br i1 %445, label %b307, label %b308

b306:
  %446 = load i16, ptr %13, !tbaa !2
  %447 = load i16, ptr %12, !tbaa !2
  %448 = icmp sge i16 %446, %447
  br i1 %448, label %b307, label %b308

b307:
  %449 = load i16, ptr %13, !tbaa !2
  %450 = sext i16 %449 to i32
  %451 = sdiv i32 %450, 8
  %452 = trunc i32 %451 to i16
  %453 = load i16, ptr %16, !tbaa !2
  %454 = sext i16 %453 to i32
  %455 = sdiv i32 %454, 8
  %456 = trunc i32 %455 to i16
  %457 = call cc 1000 addrspace(1) i16 @llrm.qb.B$PNI2(i16 %452, i16 %456)
  %458 = load i16, ptr %16, !tbaa !2
  store i16 %458, ptr %9, !tbaa !2
  %459 = load i16, ptr %9, !tbaa !2
  %460 = sitofp i16 %459 to float
  store i16 45, ptr %8, !tbaa !2
  %461 = load i16, ptr %8, !tbaa !2
  %462 = sitofp i16 %461 to float
  %463 = fdiv float %460, %462
  %464 = call float @llvm.fabs.f32(float %463)
  %465 = load float, ptr @$float86, !tbaa !2
  %466 = fcmp ogt float %464, %465
  br i1 %466, label %b309, label %b310

b308:
  %467 = load i16, ptr %16, !tbaa !2
  %468 = load i16, ptr %14, !tbaa !2
  %469 = add nsw i16 %467, %468
  store i16 %469, ptr %16, !tbaa !2
  br label %b299

b309:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b310:
  %470 = call float @llvm.sin.f32(float %463)
  %471 = load i16, ptr %0
  store i16 %471, ptr %7, !tbaa !2
  %472 = load i16, ptr %7, !tbaa !2
  %473 = sitofp i16 %472 to float
  %474 = fmul float %470, %473
  %475 = load i16, ptr %13, !tbaa !2
  store i16 %475, ptr %6, !tbaa !2
  %476 = load i16, ptr %6, !tbaa !2
  %477 = sitofp i16 %476 to float
  store i16 45, ptr %5, !tbaa !2
  %478 = load i16, ptr %5, !tbaa !2
  %479 = sitofp i16 %478 to float
  %480 = fdiv float %477, %479
  %481 = call float @llvm.fabs.f32(float %480)
  %482 = load float, ptr @$float86, !tbaa !2
  %483 = fcmp ogt float %481, %482
  br i1 %483, label %b311, label %b312

b311:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b312:
  %484 = call float @llvm.cos.f32(float %480)
  %485 = load i16, ptr %0
  store i16 %485, ptr %4, !tbaa !2
  %486 = load i16, ptr %4, !tbaa !2
  %487 = sitofp i16 %486 to float
  %488 = fmul float %484, %487
  %489 = fadd float %474, %488
  %490 = load i16, ptr %0
  %491 = mul i16 2, %490
  store i16 %491, ptr %3, !tbaa !2
  %492 = load i16, ptr %3, !tbaa !2
  %493 = sitofp i16 %492 to float
  %494 = fadd float %489, %493
  store i16 %457, ptr %2, !tbaa !2
  %495 = load i16, ptr %2, !tbaa !2
  %496 = sitofp i16 %495 to float
  %497 = fmul float %496, %494
  %498 = call i16 @llvm.lrint.i16.f32(float %497)
  store i16 %498, ptr %10, !tbaa !2
  %499 = load i16, ptr %13, !tbaa !2
  %500 = add i16 %499, 40
  %501 = load i16, ptr %16, !tbaa !2
  %502 = add i16 %501, 100
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %500, i16 %502)
  %503 = load i16, ptr %10, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSTC(i16 %503)
  %504 = load i16, ptr %13, !tbaa !2
  %505 = load i16, ptr %11, !tbaa !2
  %506 = add nsw i16 %504, %505
  store i16 %506, ptr %13, !tbaa !2
  br label %b304

b313:
  br label %b2

b314:
  br label %b315

b315:
  %507 = load i16, ptr %0
  call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 %507, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 22, i16 1, i16 10, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string222)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string223)
  br label %b2
}

define cc 1000 void @DELAY(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  ret void
}

define cc 1000 void @FADEFROMCOLOR(ptr dereferenceable(2) %0, ptr dereferenceable(2) %1, ptr dereferenceable(2) %2, ptr dereferenceable(2) %3, ptr dereferenceable(2) %4, ptr dereferenceable(2) %5) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  store i16 1, ptr %23, !tbaa !2
  %24 = load i16, ptr %2
  store i16 %24, ptr %22, !tbaa !2
  store i16 1, ptr %21, !tbaa !2
  br label %b3

b2:
  ret void

b3:
  %25 = load i16, ptr %21, !tbaa !2
  %26 = icmp sge i16 %25, 0
  br i1 %26, label %b4, label %b5

b4:
  %27 = load i16, ptr %23, !tbaa !2
  %28 = load i16, ptr %22, !tbaa !2
  %29 = icmp sle i16 %27, %28
  br i1 %29, label %b6, label %b7

b5:
  %30 = load i16, ptr %23, !tbaa !2
  %31 = load i16, ptr %22, !tbaa !2
  %32 = icmp sge i16 %30, %31
  br i1 %32, label %b6, label %b7

b6:
  %33 = load i16, ptr %0
  %34 = trunc i16 %33 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %34)
  %35 = load i16, ptr %0
  store i16 %35, ptr %20, !tbaa !2
  %36 = load i16, ptr %1
  store i16 %36, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b8

b7:
  br label %b2

b8:
  %37 = load i16, ptr %18, !tbaa !2
  %38 = icmp sge i16 %37, 0
  br i1 %38, label %b9, label %b10

b9:
  %39 = load i16, ptr %20, !tbaa !2
  %40 = load i16, ptr %19, !tbaa !2
  %41 = icmp sle i16 %39, %40
  br i1 %41, label %b11, label %b12

b10:
  %42 = load i16, ptr %20, !tbaa !2
  %43 = load i16, ptr %19, !tbaa !2
  %44 = icmp sge i16 %42, %43
  br i1 %44, label %b11, label %b12

b11:
  %45 = load i16, ptr %3
  %46 = load i16, ptr %23, !tbaa !2
  %47 = load i16, ptr %20, !tbaa !2
  %48 = mul i16 %47, 2
  %49 = getelementptr i8, ptr @"R%", i16 2
  %50 = load i16, ptr %49, !tbaa !2
  %51 = getelementptr i8, ptr @"R%", i16 10
  %52 = load i16, ptr %51, !tbaa !2
  %53 = add i16 %52, %48
  %54 = inttoptr i16 %50 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 %53
  %57 = load i16, ptr addrspace(1) %56, !tbaa !60
  %58 = load i16, ptr %3
  %59 = sub i16 %57, %58
  %60 = load i16, ptr %2
  store i16 %59, ptr %17, !tbaa !2
  %61 = load i16, ptr %17, !tbaa !2
  %62 = sitofp i16 %61 to float
  store i16 %60, ptr %16, !tbaa !2
  %63 = load i16, ptr %16, !tbaa !2
  %64 = sitofp i16 %63 to float
  %65 = fdiv float %62, %64
  store i16 %46, ptr %15, !tbaa !2
  %66 = load i16, ptr %15, !tbaa !2
  %67 = sitofp i16 %66 to float
  %68 = fmul float %67, %65
  store i16 %45, ptr %14, !tbaa !2
  %69 = load i16, ptr %14, !tbaa !2
  %70 = sitofp i16 %69 to float
  %71 = fadd float %70, %68
  %72 = call i16 @llvm.lrint.i16.f32(float %71)
  %73 = trunc i16 %72 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %73)
  %74 = load i16, ptr %4
  %75 = load i16, ptr %23, !tbaa !2
  %76 = load i16, ptr %20, !tbaa !2
  %77 = mul i16 %76, 2
  %78 = getelementptr i8, ptr @"G%", i16 2
  %79 = load i16, ptr %78, !tbaa !2
  %80 = getelementptr i8, ptr @"G%", i16 10
  %81 = load i16, ptr %80, !tbaa !2
  %82 = add i16 %81, %77
  %83 = inttoptr i16 %79 to ptr addrspace(2)
  %84 = addrspacecast ptr addrspace(2) %83 to ptr addrspace(1)
  %85 = getelementptr i8, ptr addrspace(1) %84, i16 %82
  %86 = load i16, ptr addrspace(1) %85, !tbaa !62
  %87 = load i16, ptr %4
  %88 = sub i16 %86, %87
  %89 = load i16, ptr %2
  store i16 %88, ptr %13, !tbaa !2
  %90 = load i16, ptr %13, !tbaa !2
  %91 = sitofp i16 %90 to float
  store i16 %89, ptr %12, !tbaa !2
  %92 = load i16, ptr %12, !tbaa !2
  %93 = sitofp i16 %92 to float
  %94 = fdiv float %91, %93
  store i16 %75, ptr %11, !tbaa !2
  %95 = load i16, ptr %11, !tbaa !2
  %96 = sitofp i16 %95 to float
  %97 = fmul float %96, %94
  store i16 %74, ptr %10, !tbaa !2
  %98 = load i16, ptr %10, !tbaa !2
  %99 = sitofp i16 %98 to float
  %100 = fadd float %99, %97
  %101 = call i16 @llvm.lrint.i16.f32(float %100)
  %102 = trunc i16 %101 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %102)
  %103 = load i16, ptr %5
  %104 = load i16, ptr %23, !tbaa !2
  %105 = load i16, ptr %20, !tbaa !2
  %106 = mul i16 %105, 2
  %107 = getelementptr i8, ptr @"B%", i16 2
  %108 = load i16, ptr %107, !tbaa !2
  %109 = getelementptr i8, ptr @"B%", i16 10
  %110 = load i16, ptr %109, !tbaa !2
  %111 = add i16 %110, %106
  %112 = inttoptr i16 %108 to ptr addrspace(2)
  %113 = addrspacecast ptr addrspace(2) %112 to ptr addrspace(1)
  %114 = getelementptr i8, ptr addrspace(1) %113, i16 %111
  %115 = load i16, ptr addrspace(1) %114, !tbaa !64
  %116 = load i16, ptr %5
  %117 = sub i16 %115, %116
  %118 = load i16, ptr %2
  store i16 %117, ptr %9, !tbaa !2
  %119 = load i16, ptr %9, !tbaa !2
  %120 = sitofp i16 %119 to float
  store i16 %118, ptr %8, !tbaa !2
  %121 = load i16, ptr %8, !tbaa !2
  %122 = sitofp i16 %121 to float
  %123 = fdiv float %120, %122
  store i16 %104, ptr %7, !tbaa !2
  %124 = load i16, ptr %7, !tbaa !2
  %125 = sitofp i16 %124 to float
  %126 = fmul float %125, %123
  store i16 %103, ptr %6, !tbaa !2
  %127 = load i16, ptr %6, !tbaa !2
  %128 = sitofp i16 %127 to float
  %129 = fadd float %128, %126
  %130 = call i16 @llvm.lrint.i16.f32(float %129)
  %131 = trunc i16 %130 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %131)
  %132 = load i16, ptr %20, !tbaa !2
  %133 = load i16, ptr %18, !tbaa !2
  %134 = add nsw i16 %132, %133
  store i16 %134, ptr %20, !tbaa !2
  br label %b8

b12:
  %135 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %135, ptr @"BENCHFRAME&", !tbaa !2
  %136 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %136, ptr @"BENCHFRAME&", !tbaa !2
  %137 = load i16, ptr @"XIT%", !tbaa !2
  %138 = icmp eq i16 %137, 0
  br i1 %138, label %b13, label %b14

b13:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b15

b14:
  br label %b15

b15:
  %139 = load i16, ptr @"XIT%", !tbaa !2
  %140 = icmp eq i16 %139, 1
  br i1 %140, label %b16, label %b17

b16:
  br label %b2

b17:
  br label %b18

b18:
  %141 = load i16, ptr %23, !tbaa !2
  %142 = load i16, ptr %21, !tbaa !2
  %143 = add nsw i16 %141, %142
  store i16 %143, ptr %23, !tbaa !2
  br label %b3
}

define cc 1000 void @FADETOCOLOR(ptr dereferenceable(2) %0, ptr dereferenceable(2) %1, ptr dereferenceable(2) %2, ptr dereferenceable(2) %3, ptr dereferenceable(2) %4, ptr dereferenceable(2) %5) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  store i16 1, ptr %23, !tbaa !2
  %24 = load i16, ptr %2
  store i16 %24, ptr %22, !tbaa !2
  store i16 1, ptr %21, !tbaa !2
  br label %b3

b2:
  ret void

b3:
  %25 = load i16, ptr %21, !tbaa !2
  %26 = icmp sge i16 %25, 0
  br i1 %26, label %b4, label %b5

b4:
  %27 = load i16, ptr %23, !tbaa !2
  %28 = load i16, ptr %22, !tbaa !2
  %29 = icmp sle i16 %27, %28
  br i1 %29, label %b6, label %b7

b5:
  %30 = load i16, ptr %23, !tbaa !2
  %31 = load i16, ptr %22, !tbaa !2
  %32 = icmp sge i16 %30, %31
  br i1 %32, label %b6, label %b7

b6:
  %33 = load i16, ptr %0
  %34 = trunc i16 %33 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %34)
  %35 = load i16, ptr %0
  store i16 %35, ptr %20, !tbaa !2
  %36 = load i16, ptr %1
  store i16 %36, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b8

b7:
  br label %b2

b8:
  %37 = load i16, ptr %18, !tbaa !2
  %38 = icmp sge i16 %37, 0
  br i1 %38, label %b9, label %b10

b9:
  %39 = load i16, ptr %20, !tbaa !2
  %40 = load i16, ptr %19, !tbaa !2
  %41 = icmp sle i16 %39, %40
  br i1 %41, label %b11, label %b12

b10:
  %42 = load i16, ptr %20, !tbaa !2
  %43 = load i16, ptr %19, !tbaa !2
  %44 = icmp sge i16 %42, %43
  br i1 %44, label %b11, label %b12

b11:
  %45 = load i16, ptr %20, !tbaa !2
  %46 = mul i16 %45, 2
  %47 = getelementptr i8, ptr @"R%", i16 2
  %48 = load i16, ptr %47, !tbaa !2
  %49 = getelementptr i8, ptr @"R%", i16 10
  %50 = load i16, ptr %49, !tbaa !2
  %51 = add i16 %50, %46
  %52 = inttoptr i16 %48 to ptr addrspace(2)
  %53 = addrspacecast ptr addrspace(2) %52 to ptr addrspace(1)
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 %51
  %55 = load i16, ptr addrspace(1) %54, !tbaa !60
  %56 = load i16, ptr %23, !tbaa !2
  %57 = load i16, ptr %3
  %58 = load i16, ptr %20, !tbaa !2
  %59 = mul i16 %58, 2
  %60 = getelementptr i8, ptr @"R%", i16 2
  %61 = load i16, ptr %60, !tbaa !2
  %62 = getelementptr i8, ptr @"R%", i16 10
  %63 = load i16, ptr %62, !tbaa !2
  %64 = add i16 %63, %59
  %65 = inttoptr i16 %61 to ptr addrspace(2)
  %66 = addrspacecast ptr addrspace(2) %65 to ptr addrspace(1)
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 %64
  %68 = load i16, ptr addrspace(1) %67, !tbaa !60
  %69 = sub i16 %57, %68
  %70 = load i16, ptr %2
  store i16 %69, ptr %17, !tbaa !2
  %71 = load i16, ptr %17, !tbaa !2
  %72 = sitofp i16 %71 to float
  store i16 %70, ptr %16, !tbaa !2
  %73 = load i16, ptr %16, !tbaa !2
  %74 = sitofp i16 %73 to float
  %75 = fdiv float %72, %74
  store i16 %56, ptr %15, !tbaa !2
  %76 = load i16, ptr %15, !tbaa !2
  %77 = sitofp i16 %76 to float
  %78 = fmul float %77, %75
  store i16 %55, ptr %14, !tbaa !2
  %79 = load i16, ptr %14, !tbaa !2
  %80 = sitofp i16 %79 to float
  %81 = fadd float %80, %78
  %82 = call i16 @llvm.lrint.i16.f32(float %81)
  %83 = trunc i16 %82 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %83)
  %84 = load i16, ptr %20, !tbaa !2
  %85 = mul i16 %84, 2
  %86 = getelementptr i8, ptr @"G%", i16 2
  %87 = load i16, ptr %86, !tbaa !2
  %88 = getelementptr i8, ptr @"G%", i16 10
  %89 = load i16, ptr %88, !tbaa !2
  %90 = add i16 %89, %85
  %91 = inttoptr i16 %87 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 %90
  %94 = load i16, ptr addrspace(1) %93, !tbaa !62
  %95 = load i16, ptr %23, !tbaa !2
  %96 = load i16, ptr %4
  %97 = load i16, ptr %20, !tbaa !2
  %98 = mul i16 %97, 2
  %99 = getelementptr i8, ptr @"G%", i16 2
  %100 = load i16, ptr %99, !tbaa !2
  %101 = getelementptr i8, ptr @"G%", i16 10
  %102 = load i16, ptr %101, !tbaa !2
  %103 = add i16 %102, %98
  %104 = inttoptr i16 %100 to ptr addrspace(2)
  %105 = addrspacecast ptr addrspace(2) %104 to ptr addrspace(1)
  %106 = getelementptr i8, ptr addrspace(1) %105, i16 %103
  %107 = load i16, ptr addrspace(1) %106, !tbaa !62
  %108 = sub i16 %96, %107
  %109 = load i16, ptr %2
  store i16 %108, ptr %13, !tbaa !2
  %110 = load i16, ptr %13, !tbaa !2
  %111 = sitofp i16 %110 to float
  store i16 %109, ptr %12, !tbaa !2
  %112 = load i16, ptr %12, !tbaa !2
  %113 = sitofp i16 %112 to float
  %114 = fdiv float %111, %113
  store i16 %95, ptr %11, !tbaa !2
  %115 = load i16, ptr %11, !tbaa !2
  %116 = sitofp i16 %115 to float
  %117 = fmul float %116, %114
  store i16 %94, ptr %10, !tbaa !2
  %118 = load i16, ptr %10, !tbaa !2
  %119 = sitofp i16 %118 to float
  %120 = fadd float %119, %117
  %121 = call i16 @llvm.lrint.i16.f32(float %120)
  %122 = trunc i16 %121 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %122)
  %123 = load i16, ptr %20, !tbaa !2
  %124 = mul i16 %123, 2
  %125 = getelementptr i8, ptr @"B%", i16 2
  %126 = load i16, ptr %125, !tbaa !2
  %127 = getelementptr i8, ptr @"B%", i16 10
  %128 = load i16, ptr %127, !tbaa !2
  %129 = add i16 %128, %124
  %130 = inttoptr i16 %126 to ptr addrspace(2)
  %131 = addrspacecast ptr addrspace(2) %130 to ptr addrspace(1)
  %132 = getelementptr i8, ptr addrspace(1) %131, i16 %129
  %133 = load i16, ptr addrspace(1) %132, !tbaa !64
  %134 = load i16, ptr %23, !tbaa !2
  %135 = load i16, ptr %5
  %136 = load i16, ptr %20, !tbaa !2
  %137 = mul i16 %136, 2
  %138 = getelementptr i8, ptr @"B%", i16 2
  %139 = load i16, ptr %138, !tbaa !2
  %140 = getelementptr i8, ptr @"B%", i16 10
  %141 = load i16, ptr %140, !tbaa !2
  %142 = add i16 %141, %137
  %143 = inttoptr i16 %139 to ptr addrspace(2)
  %144 = addrspacecast ptr addrspace(2) %143 to ptr addrspace(1)
  %145 = getelementptr i8, ptr addrspace(1) %144, i16 %142
  %146 = load i16, ptr addrspace(1) %145, !tbaa !64
  %147 = sub i16 %135, %146
  %148 = load i16, ptr %2
  store i16 %147, ptr %9, !tbaa !2
  %149 = load i16, ptr %9, !tbaa !2
  %150 = sitofp i16 %149 to float
  store i16 %148, ptr %8, !tbaa !2
  %151 = load i16, ptr %8, !tbaa !2
  %152 = sitofp i16 %151 to float
  %153 = fdiv float %150, %152
  store i16 %134, ptr %7, !tbaa !2
  %154 = load i16, ptr %7, !tbaa !2
  %155 = sitofp i16 %154 to float
  %156 = fmul float %155, %153
  store i16 %133, ptr %6, !tbaa !2
  %157 = load i16, ptr %6, !tbaa !2
  %158 = sitofp i16 %157 to float
  %159 = fadd float %158, %156
  %160 = call i16 @llvm.lrint.i16.f32(float %159)
  %161 = trunc i16 %160 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %161)
  %162 = load i16, ptr %20, !tbaa !2
  %163 = load i16, ptr %18, !tbaa !2
  %164 = add nsw i16 %162, %163
  store i16 %164, ptr %20, !tbaa !2
  br label %b8

b12:
  %165 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %165, ptr @"BENCHFRAME&", !tbaa !2
  %166 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %166, ptr @"BENCHFRAME&", !tbaa !2
  %167 = load i16, ptr @"XIT%", !tbaa !2
  %168 = icmp eq i16 %167, 0
  br i1 %168, label %b13, label %b14

b13:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b15

b14:
  br label %b15

b15:
  %169 = load i16, ptr @"XIT%", !tbaa !2
  %170 = icmp eq i16 %169, 1
  br i1 %170, label %b16, label %b17

b16:
  br label %b2

b17:
  br label %b18

b18:
  %171 = load i16, ptr %23, !tbaa !2
  %172 = load i16, ptr %21, !tbaa !2
  %173 = add nsw i16 %171, %172
  store i16 %173, ptr %23, !tbaa !2
  br label %b3
}

define cc 1000 void @GETMAP(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr %0, i16 1, i16 -1, i16 32)
  %10 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 1, ptr addrspace(1) %10, i16 0)
  %11 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %0)
  store i16 %11, ptr %9, !tbaa !2
  %12 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 2, ptr addrspace(1) %12, i16 0)
  %13 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %0)
  store i16 %13, ptr %8, !tbaa !2
  store i16 2, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %14 = load i16, ptr %8, !tbaa !2
  %15 = sub i16 %14, 1
  store i16 %15, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %16 = load i16, ptr %4, !tbaa !2
  %17 = icmp sge i16 %16, 0
  br i1 %17, label %b3, label %b4

b3:
  %18 = load i16, ptr %6, !tbaa !2
  %19 = load i16, ptr %5, !tbaa !2
  %20 = icmp sle i16 %18, %19
  br i1 %20, label %b5, label %b6

b4:
  %21 = load i16, ptr %6, !tbaa !2
  %22 = load i16, ptr %5, !tbaa !2
  %23 = icmp sge i16 %21, %22
  br i1 %23, label %b5, label %b6

b5:
  store i16 0, ptr %3, !tbaa !2
  %24 = load i16, ptr %9, !tbaa !2
  %25 = sub i16 %24, 1
  store i16 %25, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 0)
  ret void

b7:
  %26 = load i16, ptr %1, !tbaa !2
  %27 = icmp sge i16 %26, 0
  br i1 %27, label %b8, label %b9

b8:
  %28 = load i16, ptr %3, !tbaa !2
  %29 = load i16, ptr %2, !tbaa !2
  %30 = icmp sle i16 %28, %29
  br i1 %30, label %b10, label %b11

b9:
  %31 = load i16, ptr %3, !tbaa !2
  %32 = load i16, ptr %2, !tbaa !2
  %33 = icmp sge i16 %31, %32
  br i1 %33, label %b10, label %b11

b10:
  %34 = load i16, ptr %7, !tbaa !2
  %35 = add i16 %34, 1
  store i16 %35, ptr %7, !tbaa !2
  %36 = addrspacecast ptr %0 to ptr addrspace(1)
  %37 = load i16, ptr %7, !tbaa !2
  %38 = sext i16 %37 to i32
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 %38, ptr addrspace(1) %36, i16 0)
  %39 = load i16, ptr %6, !tbaa !2
  %40 = mul i16 %39, 32
  %41 = load i16, ptr %3, !tbaa !2
  %42 = add i16 %40, %41
  %43 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %0)
  %44 = sub i16 %42, 0
  %45 = getelementptr inbounds i16, ptr @"CD%", i16 %44
  store i16 %43, ptr %45, !tbaa !2
  %46 = load i16, ptr %3, !tbaa !2
  %47 = load i16, ptr %1, !tbaa !2
  %48 = add nsw i16 %46, %47
  store i16 %48, ptr %3, !tbaa !2
  br label %b7

b11:
  %49 = load i16, ptr %6, !tbaa !2
  %50 = load i16, ptr %4, !tbaa !2
  %51 = add nsw i16 %49, %50
  store i16 %51, ptr %6, !tbaa !2
  br label %b2
}

define cc 1000 void @GETPAL() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  call void @llrm.ia16.out.i8(i16 967, i8 0)
  store i16 0, ptr %2, !tbaa !2
  store i16 255, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %3 = load i16, ptr %0, !tbaa !2
  %4 = icmp sge i16 %3, 0
  br i1 %4, label %b3, label %b4

b3:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp sle i16 %5, %6
  br i1 %7, label %b5, label %b6

b4:
  %8 = load i16, ptr %2, !tbaa !2
  %9 = load i16, ptr %1, !tbaa !2
  %10 = icmp sge i16 %8, %9
  br i1 %10, label %b5, label %b6

b5:
  %11 = load i16, ptr %2, !tbaa !2
  %12 = mul i16 %11, 2
  %13 = getelementptr i8, ptr @"R%", i16 2
  %14 = load i16, ptr %13, !tbaa !2
  %15 = getelementptr i8, ptr @"R%", i16 10
  %16 = load i16, ptr %15, !tbaa !2
  %17 = add i16 %16, %12
  %18 = inttoptr i16 %14 to ptr addrspace(2)
  %19 = addrspacecast ptr addrspace(2) %18 to ptr addrspace(1)
  %20 = getelementptr i8, ptr addrspace(1) %19, i16 %17
  %21 = call i8 @llrm.ia16.in.i8(i16 969)
  %22 = zext i8 %21 to i16
  store i16 %22, ptr addrspace(1) %20, !tbaa !60
  %23 = load i16, ptr %2, !tbaa !2
  %24 = mul i16 %23, 2
  %25 = getelementptr i8, ptr @"G%", i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr i8, ptr @"G%", i16 10
  %28 = load i16, ptr %27, !tbaa !2
  %29 = add i16 %28, %24
  %30 = inttoptr i16 %26 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %33 = call i8 @llrm.ia16.in.i8(i16 969)
  %34 = zext i8 %33 to i16
  store i16 %34, ptr addrspace(1) %32, !tbaa !62
  %35 = load i16, ptr %2, !tbaa !2
  %36 = mul i16 %35, 2
  %37 = getelementptr i8, ptr @"B%", i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = getelementptr i8, ptr @"B%", i16 10
  %40 = load i16, ptr %39, !tbaa !2
  %41 = add i16 %40, %36
  %42 = inttoptr i16 %38 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 %41
  %45 = call i8 @llrm.ia16.in.i8(i16 969)
  %46 = zext i8 %45 to i16
  store i16 %46, ptr addrspace(1) %44, !tbaa !64
  %47 = load i16, ptr %2, !tbaa !2
  %48 = load i16, ptr %0, !tbaa !2
  %49 = add nsw i16 %47, %48
  store i16 %49, ptr %2, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @INITCROSFADEPICS(ptr dereferenceable(2) %0, ptr dereferenceable(2) %1) addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [4 x i8]
  %10 = alloca [4 x i8]
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 4, i1 false)
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %10, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 0, ptr %17, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 0, ptr %18, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store i16 11, ptr %15, !tbaa !2
  store i16 10, ptr %14, !tbaa !2
  br label %b2

b2:
  %19 = load i16, ptr %14, !tbaa !2
  %20 = icmp sge i16 %19, 0
  br i1 %20, label %b3, label %b4

b3:
  %21 = load i16, ptr %16, !tbaa !2
  %22 = load i16, ptr %15, !tbaa !2
  %23 = icmp sle i16 %21, %22
  br i1 %23, label %b5, label %b6

b4:
  %24 = load i16, ptr %16, !tbaa !2
  %25 = load i16, ptr %15, !tbaa !2
  %26 = icmp sge i16 %24, %25
  br i1 %26, label %b5, label %b6

b5:
  store i16 0, ptr %13, !tbaa !2
  store i16 19, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %10)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %9)
  ret void

b7:
  %27 = load i16, ptr %11, !tbaa !2
  %28 = icmp sge i16 %27, 0
  br i1 %28, label %b8, label %b9

b8:
  %29 = load i16, ptr %13, !tbaa !2
  %30 = load i16, ptr %12, !tbaa !2
  %31 = icmp sle i16 %29, %30
  br i1 %31, label %b10, label %b11

b9:
  %32 = load i16, ptr %13, !tbaa !2
  %33 = load i16, ptr %12, !tbaa !2
  %34 = icmp sge i16 %32, %33
  br i1 %34, label %b10, label %b11

b10:
  %35 = load i16, ptr %0
  %36 = load i16, ptr %16, !tbaa !2
  %37 = mul i16 %36, 4
  %38 = add i16 %37, %35
  %39 = mul i16 %38, 4
  %40 = getelementptr i8, ptr @TEXT$, i16 10
  %41 = load ptr, ptr %40, !tbaa !2
  %42 = getelementptr i8, ptr %41, i16 %39
  %43 = load i16, ptr %13, !tbaa !2
  %44 = add i16 %43, 1
  %45 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %42, i16 %44, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %45, ptr %10)
  %46 = load i16, ptr %1
  %47 = load i16, ptr %16, !tbaa !2
  %48 = mul i16 %47, 4
  %49 = add i16 %48, %46
  %50 = mul i16 %49, 4
  %51 = getelementptr i8, ptr @TEXT$, i16 10
  %52 = load ptr, ptr %51, !tbaa !2
  %53 = getelementptr i8, ptr %52, i16 %50
  %54 = load i16, ptr %13, !tbaa !2
  %55 = add i16 %54, 1
  %56 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %53, i16 %55, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %56, ptr %9)
  store i16 0, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 15, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b12

b11:
  %57 = load i16, ptr %16, !tbaa !2
  %58 = load i16, ptr %14, !tbaa !2
  %59 = add nsw i16 %57, %58
  store i16 %59, ptr %16, !tbaa !2
  br label %b2

b12:
  %60 = load i16, ptr %5, !tbaa !2
  %61 = icmp sge i16 %60, 0
  br i1 %61, label %b13, label %b14

b13:
  %62 = load i16, ptr %7, !tbaa !2
  %63 = load i16, ptr %6, !tbaa !2
  %64 = icmp sle i16 %62, %63
  br i1 %64, label %b15, label %b16

b14:
  %65 = load i16, ptr %7, !tbaa !2
  %66 = load i16, ptr %6, !tbaa !2
  %67 = icmp sge i16 %65, %66
  br i1 %67, label %b15, label %b16

b15:
  %68 = load i16, ptr %8, !tbaa !2
  %69 = add i16 -24576, %68
  %70 = load i16, ptr %16, !tbaa !2
  %71 = mul i16 %70, 320
  %72 = add i16 %69, %71
  store i16 %72, ptr @b$seg, !tbaa !2
  %73 = load i16, ptr %8, !tbaa !2
  %74 = add i16 %73, 20
  store i16 %74, ptr %8, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 15, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b17

b16:
  %75 = load i16, ptr %13, !tbaa !2
  %76 = load i16, ptr %11, !tbaa !2
  %77 = add nsw i16 %75, %76
  store i16 %77, ptr %13, !tbaa !2
  br label %b7

b17:
  %78 = load i16, ptr %2, !tbaa !2
  %79 = icmp sge i16 %78, 0
  br i1 %79, label %b18, label %b19

b18:
  %80 = load i16, ptr %4, !tbaa !2
  %81 = load i16, ptr %3, !tbaa !2
  %82 = icmp sle i16 %80, %81
  br i1 %82, label %b20, label %b21

b19:
  %83 = load i16, ptr %4, !tbaa !2
  %84 = load i16, ptr %3, !tbaa !2
  %85 = icmp sge i16 %83, %84
  br i1 %85, label %b20, label %b21

b20:
  %86 = load i16, ptr %4, !tbaa !2
  %87 = load i16, ptr %13, !tbaa !2
  %88 = mul i16 %87, 16
  %89 = add i16 %86, %88
  %90 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %10)
  %91 = mul i16 %90, 2
  %92 = getelementptr i8, ptr @"FPOS%", i16 2
  %93 = load i16, ptr %92, !tbaa !2
  %94 = add i16 0, %91
  %95 = inttoptr i16 %93 to ptr addrspace(2)
  %96 = addrspacecast ptr addrspace(2) %95 to ptr addrspace(1)
  %97 = getelementptr i8, ptr addrspace(1) %96, i16 %94
  %98 = load i16, ptr addrspace(1) %97, !tbaa !6
  %99 = mul i16 %98, 256
  %100 = load i16, ptr %7, !tbaa !2
  %101 = mul i16 %100, 16
  %102 = add i16 %99, %101
  %103 = load i16, ptr %4, !tbaa !2
  %104 = add i16 %102, %103
  %105 = mul i16 %104, 2
  %106 = getelementptr i8, ptr @"FONTS%", i16 2
  %107 = load i16, ptr %106, !tbaa !2
  %108 = getelementptr i8, ptr @"FONTS%", i16 10
  %109 = load i16, ptr %108, !tbaa !2
  %110 = add i16 %109, %105
  %111 = inttoptr i16 %107 to ptr addrspace(2)
  %112 = addrspacecast ptr addrspace(2) %111 to ptr addrspace(1)
  %113 = getelementptr i8, ptr addrspace(1) %112, i16 %110
  %114 = load i16, ptr addrspace(1) %113, !tbaa !10
  %115 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %9)
  %116 = mul i16 %115, 2
  %117 = getelementptr i8, ptr @"FPOS%", i16 2
  %118 = load i16, ptr %117, !tbaa !2
  %119 = add i16 0, %116
  %120 = inttoptr i16 %118 to ptr addrspace(2)
  %121 = addrspacecast ptr addrspace(2) %120 to ptr addrspace(1)
  %122 = getelementptr i8, ptr addrspace(1) %121, i16 %119
  %123 = load i16, ptr addrspace(1) %122, !tbaa !6
  %124 = mul i16 %123, 256
  %125 = load i16, ptr %7, !tbaa !2
  %126 = mul i16 %125, 16
  %127 = add i16 %124, %126
  %128 = load i16, ptr %4, !tbaa !2
  %129 = add i16 %127, %128
  %130 = mul i16 %129, 2
  %131 = getelementptr i8, ptr @"FONTS%", i16 2
  %132 = load i16, ptr %131, !tbaa !2
  %133 = getelementptr i8, ptr @"FONTS%", i16 10
  %134 = load i16, ptr %133, !tbaa !2
  %135 = add i16 %134, %130
  %136 = inttoptr i16 %132 to ptr addrspace(2)
  %137 = addrspacecast ptr addrspace(2) %136 to ptr addrspace(1)
  %138 = getelementptr i8, ptr addrspace(1) %137, i16 %135
  %139 = load i16, ptr addrspace(1) %138, !tbaa !10
  %140 = mul i16 %139, 16
  %141 = or i16 %114, %140
  %142 = trunc i16 %141 to i8
  %143 = load i16, ptr @b$seg, !tbaa !2
  %144 = inttoptr i16 %143 to ptr addrspace(2)
  %145 = addrspacecast ptr addrspace(2) %144 to ptr addrspace(1)
  %146 = getelementptr i8, ptr addrspace(1) %145, i16 %89
  store volatile i8 %142, ptr addrspace(1) %146
  %147 = load i16, ptr %4, !tbaa !2
  %148 = load i16, ptr %2, !tbaa !2
  %149 = add nsw i16 %147, %148
  store i16 %149, ptr %4, !tbaa !2
  br label %b17

b21:
  %150 = load i16, ptr %7, !tbaa !2
  %151 = load i16, ptr %5, !tbaa !2
  %152 = add nsw i16 %150, %151
  store i16 %152, ptr %7, !tbaa !2
  br label %b12
}

define cc 1000 void @INTERSECTIONS() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca float
  %7 = alloca i16
  %8 = alloca float
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca float
  %12 = alloca i16
  %13 = alloca float
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca float
  store i16 0, ptr %33, !tbaa !2
  store i16 0, ptr %32, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  %42 = load i16, ptr @"PA%", !tbaa !2
  %43 = load float, ptr @$float224, !tbaa !2
  store i16 %42, ptr %40, !tbaa !2
  %44 = load i16, ptr %40, !tbaa !2
  %45 = sitofp i16 %44 to float
  %46 = fadd float %45, %43
  %47 = fpround float %46 to float
  store float %47, ptr %41, !tbaa !2
  %48 = load float, ptr %41, !tbaa !2
  store i16 360, ptr %39, !tbaa !2
  %49 = load i16, ptr %39, !tbaa !2
  %50 = sitofp i16 %49 to float
  %51 = fcmp ogt float %48, %50
  br i1 %51, label %b5, label %b6

b2:
  %52 = load i16, ptr %27, !tbaa !2
  store i16 %52, ptr %22, !tbaa !2
  %53 = load i16, ptr %26, !tbaa !2
  store i16 %53, ptr %21, !tbaa !2
  %54 = load i16, ptr @"VA%", !tbaa !2
  %55 = icmp eq i16 %54, 90
  %56 = sext i1 %55 to i16
  %57 = load i16, ptr @"VA%", !tbaa !2
  %58 = icmp eq i16 %57, 270
  %59 = sext i1 %58 to i16
  %60 = or i16 %56, %59
  %61 = icmp ne i16 %60, 0
  br i1 %61, label %b81, label %b82

b3:
  %62 = load i16, ptr %27, !tbaa !2
  store i16 %62, ptr %17, !tbaa !2
  %63 = load i16, ptr %26, !tbaa !2
  store i16 %63, ptr %16, !tbaa !2
  %64 = load i16, ptr @"PX%", !tbaa !2
  %65 = load i16, ptr %22, !tbaa !2
  %66 = sub i16 %64, %65
  store i16 %66, ptr %14, !tbaa !2
  %67 = load i16, ptr %14, !tbaa !2
  %68 = sitofp i16 %67 to float
  %69 = fpround float %68 to float
  store float %69, ptr %13, !tbaa !2
  %70 = load float, ptr %13, !tbaa !2
  %71 = load float, ptr %13, !tbaa !2
  %72 = fmul float %70, %71
  %73 = load i16, ptr @"PY%", !tbaa !2
  %74 = load i16, ptr %21, !tbaa !2
  %75 = sub i16 %73, %74
  store i16 %75, ptr %12, !tbaa !2
  %76 = load i16, ptr %12, !tbaa !2
  %77 = sitofp i16 %76 to float
  %78 = fpround float %77 to float
  store float %78, ptr %11, !tbaa !2
  %79 = load float, ptr %11, !tbaa !2
  %80 = load float, ptr %11, !tbaa !2
  %81 = fmul float %79, %80
  %82 = fadd float %72, %81
  %83 = call float @llvm.sqrt.f32(float %82)
  %84 = call i16 @llvm.lrint.i16.f32(float %83)
  store i16 %84, ptr %15, !tbaa !2
  %85 = load i16, ptr @"PX%", !tbaa !2
  %86 = load i16, ptr %17, !tbaa !2
  %87 = sub i16 %85, %86
  store i16 %87, ptr %9, !tbaa !2
  %88 = load i16, ptr %9, !tbaa !2
  %89 = sitofp i16 %88 to float
  %90 = fpround float %89 to float
  store float %90, ptr %8, !tbaa !2
  %91 = load float, ptr %8, !tbaa !2
  %92 = load float, ptr %8, !tbaa !2
  %93 = fmul float %91, %92
  %94 = load i16, ptr @"PY%", !tbaa !2
  %95 = load i16, ptr %16, !tbaa !2
  %96 = sub i16 %94, %95
  store i16 %96, ptr %7, !tbaa !2
  %97 = load i16, ptr %7, !tbaa !2
  %98 = sitofp i16 %97 to float
  %99 = fpround float %98 to float
  store float %99, ptr %6, !tbaa !2
  %100 = load float, ptr %6, !tbaa !2
  %101 = load float, ptr %6, !tbaa !2
  %102 = fmul float %100, %101
  %103 = fadd float %93, %102
  %104 = call float @llvm.sqrt.f32(float %103)
  %105 = call i16 @llvm.lrint.i16.f32(float %104)
  store i16 %105, ptr %10, !tbaa !2
  %106 = load i16, ptr %15, !tbaa !2
  %107 = load i16, ptr %10, !tbaa !2
  %108 = icmp slt i16 %106, %107
  br i1 %108, label %b126, label %b127

b4:
  ret void

b5:
  %109 = load float, ptr @$float224, !tbaa !2
  %110 = load i16, ptr @"PA%", !tbaa !2
  %111 = sub i16 360, %110
  store i16 %111, ptr %38, !tbaa !2
  %112 = load i16, ptr %38, !tbaa !2
  %113 = sitofp i16 %112 to float
  %114 = fsub float %109, %113
  %115 = fpround float %114 to float
  store float %115, ptr %41, !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  store i16 0, ptr %37, !tbaa !2
  store i16 319, ptr %36, !tbaa !2
  store i16 1, ptr %35, !tbaa !2
  br label %b8

b8:
  %116 = load i16, ptr %35, !tbaa !2
  %117 = icmp sge i16 %116, 0
  br i1 %117, label %b9, label %b10

b9:
  %118 = load i16, ptr %37, !tbaa !2
  %119 = load i16, ptr %36, !tbaa !2
  %120 = icmp sle i16 %118, %119
  br i1 %120, label %b11, label %b12

b10:
  %121 = load i16, ptr %37, !tbaa !2
  %122 = load i16, ptr %36, !tbaa !2
  %123 = icmp sge i16 %121, %122
  br i1 %123, label %b11, label %b12

b11:
  %124 = load float, ptr %41, !tbaa !2
  %125 = load float, ptr @"RA!", !tbaa !2
  %126 = fsub float %124, %125
  %127 = fpround float %126 to float
  store float %127, ptr %41, !tbaa !2
  %128 = load float, ptr %41, !tbaa !2
  store i16 0, ptr %34, !tbaa !2
  %129 = load i16, ptr %34, !tbaa !2
  %130 = sitofp i16 %129 to float
  %131 = fcmp olt float %128, %130
  br i1 %131, label %b13, label %b14

b12:
  br label %b4

b13:
  %132 = load float, ptr @$float225, !tbaa !2
  store float %132, ptr %41, !tbaa !2
  br label %b15

b14:
  br label %b15

b15:
  %133 = load float, ptr %41, !tbaa !2
  %134 = call i16 @llvm.lrint.i16.f32(float %133)
  store i16 %134, ptr @"VA%", !tbaa !2
  %135 = load i16, ptr @"VA%", !tbaa !2
  %136 = icmp eq i16 %135, 0
  %137 = sext i1 %136 to i16
  %138 = load i16, ptr @"VA%", !tbaa !2
  %139 = icmp eq i16 %138, 180
  %140 = sext i1 %139 to i16
  %141 = or i16 %137, %140
  %142 = load i16, ptr @"VA%", !tbaa !2
  %143 = icmp eq i16 %142, 360
  %144 = sext i1 %143 to i16
  %145 = or i16 %141, %144
  %146 = icmp ne i16 %145, 0
  br i1 %146, label %b16, label %b17

b16:
  br label %b2

b17:
  br label %b18

b18:
  %147 = load i16, ptr @"VA%", !tbaa !2
  %148 = icmp sgt i16 %147, 0
  %149 = sext i1 %148 to i16
  %150 = load i16, ptr @"VA%", !tbaa !2
  %151 = icmp slt i16 %150, 180
  %152 = sext i1 %151 to i16
  %153 = and i16 %149, %152
  %154 = icmp ne i16 %153, 0
  br i1 %154, label %b20, label %b21

b20:
  %155 = load i16, ptr @"PY%", !tbaa !2
  %156 = sext i16 %155 to i32
  %157 = sdiv i32 %156, 64
  %158 = trunc i32 %157 to i16
  %159 = mul i16 %158, 64
  %160 = sub i16 %159, 1
  store i16 %160, ptr %33, !tbaa !2
  br label %b22

b21:
  %161 = load i16, ptr @"VA%", !tbaa !2
  %162 = icmp sgt i16 %161, 180
  %163 = sext i1 %162 to i16
  %164 = load i16, ptr @"VA%", !tbaa !2
  %165 = icmp slt i16 %164, 360
  %166 = sext i1 %165 to i16
  %167 = and i16 %163, %166
  %168 = icmp ne i16 %167, 0
  br i1 %168, label %b23, label %b24

b22:
  %169 = load i16, ptr @"PX%", !tbaa !2
  %170 = load i16, ptr @"PY%", !tbaa !2
  %171 = load i16, ptr %33, !tbaa !2
  %172 = sub i16 %170, %171
  %173 = load float, ptr %41, !tbaa !2
  %174 = load float, ptr @$float93, !tbaa !2
  %175 = fdiv float %173, %174
  %176 = call float @llvm.fabs.f32(float %175)
  %177 = load float, ptr @$float86, !tbaa !2
  %178 = fcmp ogt float %176, %177
  br i1 %178, label %b26, label %b27

b23:
  %179 = load i16, ptr @"PY%", !tbaa !2
  %180 = sext i16 %179 to i32
  %181 = sdiv i32 %180, 64
  %182 = trunc i32 %181 to i16
  %183 = mul i16 %182, 64
  %184 = add i16 %183, 64
  store i16 %184, ptr %33, !tbaa !2
  br label %b25

b24:
  br label %b25

b25:
  br label %b22

b26:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b27:
  %185 = call float @llvm.sin.f32(float %175)
  %186 = call float @llvm.cos.f32(float %175)
  %187 = fdiv float %185, %186
  store i16 %172, ptr %31, !tbaa !2
  %188 = load i16, ptr %31, !tbaa !2
  %189 = sitofp i16 %188 to float
  %190 = fdiv float %189, %187
  store i16 %169, ptr %30, !tbaa !2
  %191 = load i16, ptr %30, !tbaa !2
  %192 = sitofp i16 %191 to float
  %193 = fadd float %192, %190
  %194 = call i16 @llvm.lrint.i16.f32(float %193)
  store i16 %194, ptr %32, !tbaa !2
  %195 = load i16, ptr %32, !tbaa !2
  %196 = sext i16 %195 to i32
  %197 = sdiv i32 %196, 64
  %198 = trunc i32 %197 to i16
  store i16 %198, ptr %29, !tbaa !2
  %199 = load i16, ptr %33, !tbaa !2
  %200 = sext i16 %199 to i32
  %201 = sdiv i32 %200, 64
  %202 = trunc i32 %201 to i16
  store i16 %202, ptr %28, !tbaa !2
  %203 = load i16, ptr %29, !tbaa !2
  %204 = icmp slt i16 %203, 0
  br i1 %204, label %b28, label %b29

b28:
  store i16 0, ptr %29, !tbaa !2
  br label %b30

b29:
  br label %b30

b30:
  %205 = load i16, ptr %29, !tbaa !2
  %206 = icmp sgt i16 %205, 31
  br i1 %206, label %b31, label %b32

b31:
  store i16 31, ptr %29, !tbaa !2
  br label %b33

b32:
  br label %b33

b33:
  %207 = load i16, ptr %28, !tbaa !2
  %208 = icmp slt i16 %207, 0
  br i1 %208, label %b34, label %b35

b34:
  store i16 0, ptr %28, !tbaa !2
  br label %b36

b35:
  br label %b36

b36:
  %209 = load i16, ptr %28, !tbaa !2
  %210 = icmp sgt i16 %209, 31
  br i1 %210, label %b37, label %b38

b37:
  store i16 31, ptr %28, !tbaa !2
  br label %b39

b38:
  br label %b39

b39:
  %211 = load i16, ptr %28, !tbaa !2
  %212 = sub i16 %211, 0
  %213 = getelementptr inbounds i16, ptr @"MUL32%", i16 %212
  %214 = load i16, ptr %213, !tbaa !2
  %215 = load i16, ptr %29, !tbaa !2
  %216 = add i16 %214, %215
  %217 = sub i16 %216, 0
  %218 = getelementptr inbounds i16, ptr @"CD%", i16 %217
  %219 = load i16, ptr %218, !tbaa !2
  %220 = icmp eq i16 %219, 1
  br i1 %220, label %b40, label %b41

b40:
  %221 = load i16, ptr %32, !tbaa !2
  store i16 %221, ptr %27, !tbaa !2
  %222 = load i16, ptr %33, !tbaa !2
  store i16 %222, ptr %26, !tbaa !2
  br label %b2

b41:
  br label %b42

b42:
  %223 = load i16, ptr @"VA%", !tbaa !2
  %224 = icmp sgt i16 %223, 0
  %225 = sext i1 %224 to i16
  %226 = load i16, ptr @"VA%", !tbaa !2
  %227 = icmp slt i16 %226, 180
  %228 = sext i1 %227 to i16
  %229 = and i16 %225, %228
  %230 = icmp ne i16 %229, 0
  br i1 %230, label %b44, label %b45

b44:
  store i16 -64, ptr %25, !tbaa !2
  br label %b46

b45:
  %231 = load i16, ptr @"VA%", !tbaa !2
  %232 = icmp sgt i16 %231, 180
  %233 = sext i1 %232 to i16
  %234 = load i16, ptr @"VA%", !tbaa !2
  %235 = icmp slt i16 %234, 360
  %236 = sext i1 %235 to i16
  %237 = and i16 %233, %236
  %238 = icmp ne i16 %237, 0
  br i1 %238, label %b47, label %b48

b46:
  %239 = load i16, ptr %25, !tbaa !2
  %240 = sub i16 0, %239
  %241 = load float, ptr %41, !tbaa !2
  %242 = load float, ptr @$float93, !tbaa !2
  %243 = fdiv float %241, %242
  %244 = call float @llvm.fabs.f32(float %243)
  %245 = load float, ptr @$float86, !tbaa !2
  %246 = fcmp ogt float %244, %245
  br i1 %246, label %b50, label %b51

b47:
  store i16 64, ptr %25, !tbaa !2
  br label %b49

b48:
  br label %b49

b49:
  br label %b46

b50:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b51:
  %247 = call float @llvm.sin.f32(float %243)
  %248 = call float @llvm.cos.f32(float %243)
  %249 = fdiv float %247, %248
  store i16 %240, ptr %23, !tbaa !2
  %250 = load i16, ptr %23, !tbaa !2
  %251 = sitofp i16 %250 to float
  %252 = fdiv float %251, %249
  %253 = call i16 @llvm.lrint.i16.f32(float %252)
  store i16 %253, ptr %24, !tbaa !2
  %254 = load i16, ptr %24, !tbaa !2
  %255 = icmp sgt i16 %254, 2048
  br i1 %255, label %b52, label %b53

b52:
  store i16 2048, ptr %24, !tbaa !2
  br label %b54

b53:
  br label %b54

b54:
  %256 = load i16, ptr %24, !tbaa !2
  %257 = icmp slt i16 %256, -2048
  br i1 %257, label %b55, label %b56

b55:
  store i16 -2048, ptr %24, !tbaa !2
  br label %b57

b56:
  br label %b57

b57:
  %258 = load i16, ptr %32, !tbaa !2
  store i16 %258, ptr %27, !tbaa !2
  %259 = load i16, ptr %33, !tbaa !2
  store i16 %259, ptr %26, !tbaa !2
  br label %b59

b58:
  %260 = load i16, ptr %28, !tbaa !2
  %261 = sub i16 %260, 0
  %262 = getelementptr inbounds i16, ptr @"MUL32%", i16 %261
  %263 = load i16, ptr %262, !tbaa !2
  %264 = load i16, ptr %29, !tbaa !2
  %265 = add i16 %263, %264
  %266 = sub i16 %265, 0
  %267 = getelementptr inbounds i16, ptr @"CD%", i16 %266
  %268 = load i16, ptr %267, !tbaa !2
  %269 = icmp eq i16 %268, 1
  %270 = sext i1 %269 to i16
  %271 = load i16, ptr %29, !tbaa !2
  %272 = icmp eq i16 %271, 31
  %273 = sext i1 %272 to i16
  %274 = or i16 %270, %273
  %275 = load i16, ptr %28, !tbaa !2
  %276 = icmp eq i16 %275, 31
  %277 = sext i1 %276 to i16
  %278 = or i16 %274, %277
  %279 = icmp ne i16 %278, 0
  br i1 %279, label %b60, label %b59

b59:
  %280 = load i16, ptr %27, !tbaa !2
  %281 = load i16, ptr %24, !tbaa !2
  %282 = add i16 %280, %281
  store i16 %282, ptr %27, !tbaa !2
  %283 = load i16, ptr %27, !tbaa !2
  %284 = icmp sgt i16 %283, 2048
  %285 = sext i1 %284 to i16
  %286 = load i16, ptr %27, !tbaa !2
  %287 = icmp slt i16 %286, -2048
  %288 = sext i1 %287 to i16
  %289 = or i16 %285, %288
  %290 = icmp ne i16 %289, 0
  br i1 %290, label %b61, label %b62

b60:
  br label %b2

b61:
  br label %b2

b62:
  br label %b63

b63:
  %291 = load i16, ptr %26, !tbaa !2
  %292 = load i16, ptr %25, !tbaa !2
  %293 = add i16 %291, %292
  store i16 %293, ptr %26, !tbaa !2
  %294 = load i16, ptr %26, !tbaa !2
  %295 = icmp sgt i16 %294, 2048
  %296 = sext i1 %295 to i16
  %297 = load i16, ptr %26, !tbaa !2
  %298 = icmp slt i16 %297, -2048
  %299 = sext i1 %298 to i16
  %300 = or i16 %296, %299
  %301 = icmp ne i16 %300, 0
  br i1 %301, label %b65, label %b66

b65:
  br label %b2

b66:
  br label %b67

b67:
  %302 = load i16, ptr %27, !tbaa !2
  %303 = sext i16 %302 to i32
  %304 = sdiv i32 %303, 64
  %305 = trunc i32 %304 to i16
  store i16 %305, ptr %29, !tbaa !2
  %306 = load i16, ptr %26, !tbaa !2
  %307 = sext i16 %306 to i32
  %308 = sdiv i32 %307, 64
  %309 = trunc i32 %308 to i16
  store i16 %309, ptr %28, !tbaa !2
  %310 = load i16, ptr %29, !tbaa !2
  %311 = icmp slt i16 %310, 0
  br i1 %311, label %b69, label %b70

b69:
  store i16 0, ptr %29, !tbaa !2
  br label %b71

b70:
  br label %b71

b71:
  %312 = load i16, ptr %29, !tbaa !2
  %313 = icmp sgt i16 %312, 31
  br i1 %313, label %b72, label %b73

b72:
  store i16 31, ptr %29, !tbaa !2
  br label %b74

b73:
  br label %b74

b74:
  %314 = load i16, ptr %28, !tbaa !2
  %315 = icmp slt i16 %314, 0
  br i1 %315, label %b75, label %b76

b75:
  store i16 0, ptr %28, !tbaa !2
  br label %b77

b76:
  br label %b77

b77:
  %316 = load i16, ptr %28, !tbaa !2
  %317 = icmp sgt i16 %316, 31
  br i1 %317, label %b78, label %b79

b78:
  store i16 31, ptr %28, !tbaa !2
  br label %b80

b79:
  br label %b80

b80:
  br label %b58

b81:
  br label %b3

b82:
  br label %b83

b83:
  %318 = load i16, ptr @"VA%", !tbaa !2
  %319 = icmp sgt i16 %318, 270
  %320 = sext i1 %319 to i16
  %321 = load i16, ptr @"VA%", !tbaa !2
  %322 = icmp slt i16 %321, 90
  %323 = sext i1 %322 to i16
  %324 = or i16 %320, %323
  %325 = icmp ne i16 %324, 0
  br i1 %325, label %b85, label %b86

b85:
  %326 = load i16, ptr @"PX%", !tbaa !2
  %327 = sext i16 %326 to i32
  %328 = sdiv i32 %327, 64
  %329 = trunc i32 %328 to i16
  %330 = mul i16 %329, 64
  %331 = add i16 %330, 64
  store i16 %331, ptr %32, !tbaa !2
  br label %b87

b86:
  %332 = load i16, ptr @"VA%", !tbaa !2
  %333 = icmp sgt i16 %332, 90
  %334 = sext i1 %333 to i16
  %335 = load i16, ptr @"VA%", !tbaa !2
  %336 = icmp slt i16 %335, 270
  %337 = sext i1 %336 to i16
  %338 = or i16 %334, %337
  %339 = icmp ne i16 %338, 0
  br i1 %339, label %b88, label %b89

b87:
  %340 = load i16, ptr @"PY%", !tbaa !2
  %341 = load i16, ptr %32, !tbaa !2
  %342 = load i16, ptr @"PX%", !tbaa !2
  %343 = sub i16 %341, %342
  %344 = load float, ptr %41, !tbaa !2
  %345 = load float, ptr @$float93, !tbaa !2
  %346 = fdiv float %344, %345
  %347 = call float @llvm.fabs.f32(float %346)
  %348 = load float, ptr @$float86, !tbaa !2
  %349 = fcmp ogt float %347, %348
  br i1 %349, label %b91, label %b92

b88:
  %350 = load i16, ptr @"PX%", !tbaa !2
  %351 = sext i16 %350 to i32
  %352 = sdiv i32 %351, 64
  %353 = trunc i32 %352 to i16
  %354 = mul i16 %353, 64
  %355 = sub i16 %354, 1
  store i16 %355, ptr %32, !tbaa !2
  br label %b90

b89:
  br label %b90

b90:
  br label %b87

b91:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b92:
  %356 = call float @llvm.sin.f32(float %346)
  %357 = call float @llvm.cos.f32(float %346)
  %358 = fdiv float %356, %357
  store i16 %343, ptr %20, !tbaa !2
  %359 = load i16, ptr %20, !tbaa !2
  %360 = sitofp i16 %359 to float
  %361 = fmul float %360, %358
  store i16 %340, ptr %19, !tbaa !2
  %362 = load i16, ptr %19, !tbaa !2
  %363 = sitofp i16 %362 to float
  %364 = fsub float %363, %361
  %365 = call i16 @llvm.lrint.i16.f32(float %364)
  store i16 %365, ptr %33, !tbaa !2
  %366 = load i16, ptr %32, !tbaa !2
  %367 = sext i16 %366 to i32
  %368 = sdiv i32 %367, 64
  %369 = trunc i32 %368 to i16
  store i16 %369, ptr %29, !tbaa !2
  %370 = load i16, ptr %33, !tbaa !2
  %371 = sext i16 %370 to i32
  %372 = sdiv i32 %371, 64
  %373 = trunc i32 %372 to i16
  store i16 %373, ptr %28, !tbaa !2
  %374 = load i16, ptr %29, !tbaa !2
  %375 = icmp slt i16 %374, 0
  br i1 %375, label %b93, label %b94

b93:
  store i16 0, ptr %29, !tbaa !2
  br label %b95

b94:
  br label %b95

b95:
  %376 = load i16, ptr %29, !tbaa !2
  %377 = icmp sgt i16 %376, 31
  br i1 %377, label %b96, label %b97

b96:
  store i16 31, ptr %29, !tbaa !2
  br label %b98

b97:
  br label %b98

b98:
  %378 = load i16, ptr %28, !tbaa !2
  %379 = icmp slt i16 %378, 0
  br i1 %379, label %b99, label %b100

b99:
  store i16 0, ptr %28, !tbaa !2
  br label %b101

b100:
  br label %b101

b101:
  %380 = load i16, ptr %28, !tbaa !2
  %381 = icmp sgt i16 %380, 31
  br i1 %381, label %b102, label %b103

b102:
  store i16 31, ptr %28, !tbaa !2
  br label %b104

b103:
  br label %b104

b104:
  %382 = load i16, ptr %28, !tbaa !2
  %383 = sub i16 %382, 0
  %384 = getelementptr inbounds i16, ptr @"MUL32%", i16 %383
  %385 = load i16, ptr %384, !tbaa !2
  %386 = load i16, ptr %29, !tbaa !2
  %387 = add i16 %385, %386
  %388 = sub i16 %387, 0
  %389 = getelementptr inbounds i16, ptr @"CD%", i16 %388
  %390 = load i16, ptr %389, !tbaa !2
  %391 = icmp eq i16 %390, 1
  br i1 %391, label %b105, label %b106

b105:
  %392 = load i16, ptr %32, !tbaa !2
  store i16 %392, ptr %27, !tbaa !2
  %393 = load i16, ptr %33, !tbaa !2
  store i16 %393, ptr %26, !tbaa !2
  br label %b3

b106:
  br label %b107

b107:
  %394 = load i16, ptr @"VA%", !tbaa !2
  %395 = icmp sgt i16 %394, 270
  %396 = sext i1 %395 to i16
  %397 = load i16, ptr @"VA%", !tbaa !2
  %398 = icmp slt i16 %397, 90
  %399 = sext i1 %398 to i16
  %400 = or i16 %396, %399
  %401 = icmp ne i16 %400, 0
  br i1 %401, label %b109, label %b110

b109:
  store i16 64, ptr %24, !tbaa !2
  br label %b111

b110:
  %402 = load i16, ptr @"VA%", !tbaa !2
  %403 = icmp sgt i16 %402, 90
  %404 = sext i1 %403 to i16
  %405 = load i16, ptr @"VA%", !tbaa !2
  %406 = icmp slt i16 %405, 270
  %407 = sext i1 %406 to i16
  %408 = or i16 %404, %407
  %409 = icmp ne i16 %408, 0
  br i1 %409, label %b112, label %b113

b111:
  %410 = load i16, ptr %24, !tbaa !2
  %411 = sub i16 0, %410
  %412 = load float, ptr %41, !tbaa !2
  %413 = load float, ptr @$float93, !tbaa !2
  %414 = fdiv float %412, %413
  %415 = call float @llvm.fabs.f32(float %414)
  %416 = load float, ptr @$float86, !tbaa !2
  %417 = fcmp ogt float %415, %416
  br i1 %417, label %b115, label %b116

b112:
  store i16 -64, ptr %24, !tbaa !2
  br label %b114

b113:
  br label %b114

b114:
  br label %b111

b115:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b116:
  %418 = call float @llvm.sin.f32(float %414)
  %419 = call float @llvm.cos.f32(float %414)
  %420 = fdiv float %418, %419
  store i16 %411, ptr %18, !tbaa !2
  %421 = load i16, ptr %18, !tbaa !2
  %422 = sitofp i16 %421 to float
  %423 = fmul float %422, %420
  %424 = call i16 @llvm.lrint.i16.f32(float %423)
  store i16 %424, ptr %25, !tbaa !2
  %425 = load i16, ptr %32, !tbaa !2
  store i16 %425, ptr %27, !tbaa !2
  %426 = load i16, ptr %33, !tbaa !2
  store i16 %426, ptr %26, !tbaa !2
  br label %b118

b117:
  %427 = load i16, ptr %28, !tbaa !2
  %428 = sub i16 %427, 0
  %429 = getelementptr inbounds i16, ptr @"MUL32%", i16 %428
  %430 = load i16, ptr %429, !tbaa !2
  %431 = load i16, ptr %29, !tbaa !2
  %432 = add i16 %430, %431
  %433 = sub i16 %432, 0
  %434 = getelementptr inbounds i16, ptr @"CD%", i16 %433
  %435 = load i16, ptr %434, !tbaa !2
  %436 = icmp eq i16 %435, 1
  %437 = sext i1 %436 to i16
  %438 = load i16, ptr %29, !tbaa !2
  %439 = icmp eq i16 %438, 31
  %440 = sext i1 %439 to i16
  %441 = or i16 %437, %440
  %442 = load i16, ptr %28, !tbaa !2
  %443 = icmp eq i16 %442, 31
  %444 = sext i1 %443 to i16
  %445 = or i16 %441, %444
  %446 = icmp ne i16 %445, 0
  br i1 %446, label %b119, label %b118

b118:
  %447 = load i16, ptr %27, !tbaa !2
  %448 = load i16, ptr %24, !tbaa !2
  %449 = add i16 %447, %448
  store i16 %449, ptr %27, !tbaa !2
  %450 = load i16, ptr %26, !tbaa !2
  %451 = load i16, ptr %25, !tbaa !2
  %452 = add i16 %450, %451
  store i16 %452, ptr %26, !tbaa !2
  %453 = load i16, ptr %27, !tbaa !2
  %454 = sext i16 %453 to i32
  %455 = sdiv i32 %454, 64
  %456 = trunc i32 %455 to i16
  store i16 %456, ptr %29, !tbaa !2
  %457 = load i16, ptr %26, !tbaa !2
  %458 = sext i16 %457 to i32
  %459 = sdiv i32 %458, 64
  %460 = trunc i32 %459 to i16
  store i16 %460, ptr %28, !tbaa !2
  %461 = load i16, ptr %29, !tbaa !2
  %462 = icmp slt i16 %461, 0
  %463 = sext i1 %462 to i16
  %464 = load i16, ptr %29, !tbaa !2
  %465 = icmp sgt i16 %464, 31
  %466 = sext i1 %465 to i16
  %467 = or i16 %463, %466
  %468 = icmp ne i16 %467, 0
  br i1 %468, label %b120, label %b121

b119:
  br label %b3

b120:
  store i16 31, ptr %29, !tbaa !2
  br label %b122

b121:
  br label %b122

b122:
  %469 = load i16, ptr %28, !tbaa !2
  %470 = icmp slt i16 %469, 0
  %471 = sext i1 %470 to i16
  %472 = load i16, ptr %28, !tbaa !2
  %473 = icmp sgt i16 %472, 31
  %474 = sext i1 %473 to i16
  %475 = or i16 %471, %474
  %476 = icmp ne i16 %475, 0
  br i1 %476, label %b123, label %b124

b123:
  store i16 31, ptr %28, !tbaa !2
  br label %b125

b124:
  br label %b125

b125:
  br label %b117

b126:
  %477 = load i16, ptr %15, !tbaa !2
  store i16 %477, ptr %5, !tbaa !2
  br label %b128

b127:
  %478 = load i16, ptr %10, !tbaa !2
  store i16 %478, ptr %5, !tbaa !2
  br label %b128

b128:
  %479 = load i16, ptr %37, !tbaa !2
  %480 = load i16, ptr %5, !tbaa !2
  %481 = load i16, ptr @"PA%", !tbaa !2
  %482 = load float, ptr %41, !tbaa !2
  store i16 %481, ptr %4, !tbaa !2
  %483 = load i16, ptr %4, !tbaa !2
  %484 = sitofp i16 %483 to float
  %485 = fsub float %484, %482
  %486 = load float, ptr @$float93, !tbaa !2
  %487 = fdiv float %485, %486
  %488 = call float @llvm.fabs.f32(float %487)
  %489 = load float, ptr @$float86, !tbaa !2
  %490 = fcmp ogt float %488, %489
  br i1 %490, label %b129, label %b130

b129:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b130:
  %491 = call float @llvm.cos.f32(float %487)
  store i16 %480, ptr %3, !tbaa !2
  %492 = load i16, ptr %3, !tbaa !2
  %493 = sitofp i16 %492 to float
  %494 = fmul float %493, %491
  %495 = call i16 @llvm.lrint.i16.f32(float %494)
  %496 = sub i16 %479, 0
  %497 = getelementptr inbounds i16, ptr @"DIS%", i16 %496
  store i16 %495, ptr %497, !tbaa !2
  %498 = load i16, ptr %37, !tbaa !2
  %499 = load i16, ptr @"DPP%", !tbaa !2
  %500 = load i16, ptr %37, !tbaa !2
  %501 = sub i16 %500, 0
  %502 = getelementptr inbounds i16, ptr @"DIS%", i16 %501
  %503 = load i16, ptr %502, !tbaa !2
  store i16 %499, ptr %2, !tbaa !2
  %504 = load i16, ptr %2, !tbaa !2
  %505 = sitofp i16 %504 to float
  store i16 %503, ptr %1, !tbaa !2
  %506 = load i16, ptr %1, !tbaa !2
  %507 = sitofp i16 %506 to float
  %508 = fdiv float %505, %507
  store i16 64, ptr %0, !tbaa !2
  %509 = load i16, ptr %0, !tbaa !2
  %510 = sitofp i16 %509 to float
  %511 = fmul float %510, %508
  %512 = call i16 @llvm.lrint.i16.f32(float %511)
  %513 = sub i16 %498, 0
  %514 = getelementptr inbounds i16, ptr @"SLC%", i16 %513
  store i16 %512, ptr %514, !tbaa !2
  %515 = load i16, ptr %37, !tbaa !2
  %516 = sub i16 %515, 0
  %517 = getelementptr inbounds i16, ptr @"SLC%", i16 %516
  %518 = load i16, ptr %517, !tbaa !2
  %519 = icmp sgt i16 %518, 200
  br i1 %519, label %b131, label %b132

b131:
  %520 = load i16, ptr %37, !tbaa !2
  %521 = sub i16 %520, 0
  %522 = getelementptr inbounds i16, ptr @"SLC%", i16 %521
  store i16 200, ptr %522, !tbaa !2
  br label %b133

b132:
  br label %b133

b133:
  %523 = load i16, ptr %37, !tbaa !2
  %524 = load i16, ptr %37, !tbaa !2
  %525 = sub i16 %524, 0
  %526 = getelementptr inbounds i16, ptr @"DIS%", i16 %525
  %527 = load i16, ptr %526, !tbaa !2
  %528 = sext i16 %527 to i32
  %529 = sdiv i32 %528, 2
  %530 = trunc i32 %529 to i16
  %531 = sub i16 254, %530
  %532 = sub i16 %523, 0
  %533 = getelementptr inbounds i16, ptr @"SC%", i16 %532
  store i16 %531, ptr %533, !tbaa !2
  %534 = load i16, ptr %37, !tbaa !2
  %535 = sub i16 %534, 0
  %536 = getelementptr inbounds i16, ptr @"SC%", i16 %535
  %537 = load i16, ptr %536, !tbaa !2
  %538 = icmp slt i16 %537, 0
  br i1 %538, label %b134, label %b135

b134:
  %539 = load i16, ptr %37, !tbaa !2
  %540 = sub i16 %539, 0
  %541 = getelementptr inbounds i16, ptr @"SC%", i16 %540
  store i16 0, ptr %541, !tbaa !2
  br label %b136

b135:
  br label %b136

b136:
  %542 = load i16, ptr %37, !tbaa !2
  %543 = load i16, ptr %37, !tbaa !2
  %544 = sub i16 %543, 0
  %545 = getelementptr inbounds i16, ptr @"SLC%", i16 %544
  %546 = load i16, ptr %545, !tbaa !2
  %547 = sub i16 200, %546
  %548 = sext i16 %547 to i32
  %549 = sdiv i32 %548, 2
  %550 = trunc i32 %549 to i16
  %551 = sub i16 %542, 0
  %552 = getelementptr inbounds i16, ptr @"SLA%", i16 %551
  store i16 %550, ptr %552, !tbaa !2
  %553 = load i16, ptr %37, !tbaa !2
  %554 = load i16, ptr %37, !tbaa !2
  %555 = sub i16 %554, 0
  %556 = getelementptr inbounds i16, ptr @"SLA%", i16 %555
  %557 = load i16, ptr %556, !tbaa !2
  %558 = load i16, ptr %37, !tbaa !2
  %559 = sub i16 %558, 0
  %560 = getelementptr inbounds i16, ptr @"SLC%", i16 %559
  %561 = load i16, ptr %560, !tbaa !2
  %562 = add i16 %557, %561
  %563 = sub i16 %553, 0
  %564 = getelementptr inbounds i16, ptr @"SLB%", i16 %563
  store i16 %562, ptr %564, !tbaa !2
  %565 = load i16, ptr %37, !tbaa !2
  %566 = load i16, ptr %35, !tbaa !2
  %567 = add nsw i16 %565, %566
  store i16 %567, ptr %37, !tbaa !2
  br label %b8
}

define cc 1000 void @KEYACTION() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call i8 @llrm.ia16.in.i8(i16 96)
  %1 = zext i8 %0 to i16
  store i16 %1, ptr @"KB%", !tbaa !2
  %2 = load i16, ptr @"KB%", !tbaa !2
  %3 = icmp eq i16 %2, 1
  br i1 %3, label %b2, label %b3

b2:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  ret void
}

define cc 1000 void @LOAD3DRECORD() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = load i16, ptr @"GGI%", !tbaa !2
  %1 = load i16, ptr @"GI%", !tbaa !2
  %2 = icmp slt i16 %0, %1
  br i1 %2, label %b2, label %b3

b2:
  %3 = load i16, ptr @"GGI%", !tbaa !2
  %4 = add i16 %3, 1
  store i16 %4, ptr @"GGI%", !tbaa !2
  br label %b4

b3:
  %5 = addrspacecast ptr @GI$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) %5, i16 0)
  %6 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @GI$)
  store i16 %6, ptr @"GI%", !tbaa !2
  %7 = addrspacecast ptr @GG$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) %7, i16 0)
  %8 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @GG$)
  store i16 %8, ptr @"GG%", !tbaa !2
  store i16 1, ptr @"GGI%", !tbaa !2
  %9 = load i16, ptr @"FILEI%", !tbaa !2
  %10 = add i16 %9, 2
  store i16 %10, ptr @"FILEI%", !tbaa !2
  br label %b4

b4:
  ret void
}

define cc 1000 void @LOADOBJECT(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  store i16 0, ptr %8, !tbaa !2
  call cc 1000 addrspace(1) void @CLEARSCREEN(ptr %8)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr %0, i16 1, i16 -1, i16 32)
  %9 = addrspacecast ptr @"NDTS%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %9, i16 2)
  %10 = addrspacecast ptr @"NLNS%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %10, i16 2)
  %11 = addrspacecast ptr @"NPLS%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %11, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string226, ptr %0)
  store i16 0, ptr %7, !tbaa !2
  %12 = load i16, ptr @"NDTS%", !tbaa !2
  %13 = sub i16 %12, 1
  store i16 %13, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b2

b2:
  %14 = load i16, ptr %5, !tbaa !2
  %15 = icmp sge i16 %14, 0
  br i1 %15, label %b3, label %b4

b3:
  %16 = load i16, ptr %7, !tbaa !2
  %17 = load i16, ptr %6, !tbaa !2
  %18 = icmp sle i16 %16, %17
  br i1 %18, label %b5, label %b6

b4:
  %19 = load i16, ptr %7, !tbaa !2
  %20 = load i16, ptr %6, !tbaa !2
  %21 = icmp sge i16 %19, %20
  br i1 %21, label %b5, label %b6

b5:
  %22 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %22, i16 0)
  %23 = load i16, ptr %7, !tbaa !2
  %24 = mul i16 %23, 2
  %25 = getelementptr i8, ptr @"XO%", i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = add i16 0, %24
  %28 = inttoptr i16 %26 to ptr addrspace(2)
  %29 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %30 = getelementptr i8, ptr addrspace(1) %29, i16 %27
  %31 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %0)
  %32 = sub i16 %31, 128
  store i16 %32, ptr addrspace(1) %30, !tbaa !16
  %33 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %33, i16 0)
  %34 = load i16, ptr %7, !tbaa !2
  %35 = mul i16 %34, 2
  %36 = getelementptr i8, ptr @"YO%", i16 2
  %37 = load i16, ptr %36, !tbaa !2
  %38 = add i16 0, %35
  %39 = inttoptr i16 %37 to ptr addrspace(2)
  %40 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 %38
  %42 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %0)
  %43 = sub i16 %42, 128
  store i16 %43, ptr addrspace(1) %41, !tbaa !20
  %44 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %44, i16 0)
  %45 = load i16, ptr %7, !tbaa !2
  %46 = mul i16 %45, 2
  %47 = getelementptr i8, ptr @"ZO%", i16 2
  %48 = load i16, ptr %47, !tbaa !2
  %49 = add i16 0, %46
  %50 = inttoptr i16 %48 to ptr addrspace(2)
  %51 = addrspacecast ptr addrspace(2) %50 to ptr addrspace(1)
  %52 = getelementptr i8, ptr addrspace(1) %51, i16 %49
  %53 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %0)
  %54 = sub i16 %53, 128
  store i16 %54, ptr addrspace(1) %52, !tbaa !18
  %55 = load i16, ptr %7, !tbaa !2
  %56 = load i16, ptr %5, !tbaa !2
  %57 = add nsw i16 %55, %56
  store i16 %57, ptr %7, !tbaa !2
  br label %b2

b6:
  store i16 0, ptr %7, !tbaa !2
  %58 = load i16, ptr @"NLNS%", !tbaa !2
  %59 = sub i16 %58, 1
  store i16 %59, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b7

b7:
  %60 = load i16, ptr %3, !tbaa !2
  %61 = icmp sge i16 %60, 0
  br i1 %61, label %b8, label %b9

b8:
  %62 = load i16, ptr %7, !tbaa !2
  %63 = load i16, ptr %4, !tbaa !2
  %64 = icmp sle i16 %62, %63
  br i1 %64, label %b10, label %b11

b9:
  %65 = load i16, ptr %7, !tbaa !2
  %66 = load i16, ptr %4, !tbaa !2
  %67 = icmp sge i16 %65, %66
  br i1 %67, label %b10, label %b11

b10:
  %68 = load i16, ptr %7, !tbaa !2
  %69 = load i16, ptr %3, !tbaa !2
  %70 = add nsw i16 %68, %69
  store i16 %70, ptr %7, !tbaa !2
  br label %b7

b11:
  store i16 0, ptr %7, !tbaa !2
  %71 = load i16, ptr @"NPLS%", !tbaa !2
  %72 = sub i16 %71, 1
  store i16 %72, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b12

b12:
  %73 = load i16, ptr %1, !tbaa !2
  %74 = icmp sge i16 %73, 0
  br i1 %74, label %b13, label %b14

b13:
  %75 = load i16, ptr %7, !tbaa !2
  %76 = load i16, ptr %2, !tbaa !2
  %77 = icmp sle i16 %75, %76
  br i1 %77, label %b15, label %b16

b14:
  %78 = load i16, ptr %7, !tbaa !2
  %79 = load i16, ptr %2, !tbaa !2
  %80 = icmp sge i16 %78, %79
  br i1 %80, label %b15, label %b16

b15:
  %81 = load i16, ptr %7, !tbaa !2
  %82 = load i16, ptr %1, !tbaa !2
  %83 = add nsw i16 %81, %82
  store i16 %83, ptr %7, !tbaa !2
  br label %b12

b16:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  ret void
}

define cc 1000 void @LOADQBINSIDE() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [4 x i8]
  %12 = alloca [4 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 4, i1 false)
  store i16 0, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 0, ptr %13, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 0, ptr %14, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string227, ptr %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string228, ptr %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr %12, i16 1, i16 -1, i16 32)
  call void @llrm.ia16.out.i8(i16 968, i8 -128)
  store i16 0, ptr %10, !tbaa !2
  store i16 383, ptr %9, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  br label %b2

b2:
  %15 = load i16, ptr %8, !tbaa !2
  %16 = icmp sge i16 %15, 0
  br i1 %16, label %b3, label %b4

b3:
  %17 = load i16, ptr %10, !tbaa !2
  %18 = load i16, ptr %9, !tbaa !2
  %19 = icmp sle i16 %17, %18
  br i1 %19, label %b5, label %b6

b4:
  %20 = load i16, ptr %10, !tbaa !2
  %21 = load i16, ptr %9, !tbaa !2
  %22 = icmp sge i16 %20, %21
  br i1 %22, label %b5, label %b6

b5:
  %23 = addrspacecast ptr %11 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %23, i16 0)
  %24 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %11)
  %25 = trunc i16 %24 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %25)
  %26 = load i16, ptr %10, !tbaa !2
  %27 = load i16, ptr %8, !tbaa !2
  %28 = add nsw i16 %26, %27
  store i16 %28, ptr %10, !tbaa !2
  br label %b2

b6:
  store i16 100, ptr %7, !tbaa !2
  store i16 100, ptr %6, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  %29 = load i16, ptr %6, !tbaa !2
  store i16 %29, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b7

b7:
  %30 = load i16, ptr %3, !tbaa !2
  %31 = icmp sge i16 %30, 0
  br i1 %31, label %b8, label %b9

b8:
  %32 = load i16, ptr %5, !tbaa !2
  %33 = load i16, ptr %4, !tbaa !2
  %34 = icmp sle i16 %32, %33
  br i1 %34, label %b10, label %b11

b9:
  %35 = load i16, ptr %5, !tbaa !2
  %36 = load i16, ptr %4, !tbaa !2
  %37 = icmp sge i16 %35, %36
  br i1 %37, label %b10, label %b11

b10:
  store i16 1, ptr %2, !tbaa !2
  %38 = load i16, ptr %7, !tbaa !2
  store i16 %38, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %11)
  ret void

b12:
  %39 = load i16, ptr %0, !tbaa !2
  %40 = icmp sge i16 %39, 0
  br i1 %40, label %b13, label %b14

b13:
  %41 = load i16, ptr %2, !tbaa !2
  %42 = load i16, ptr %1, !tbaa !2
  %43 = icmp sle i16 %41, %42
  br i1 %43, label %b15, label %b16

b14:
  %44 = load i16, ptr %2, !tbaa !2
  %45 = load i16, ptr %1, !tbaa !2
  %46 = icmp sge i16 %44, %45
  br i1 %46, label %b15, label %b16

b15:
  %47 = addrspacecast ptr %11 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %47, i16 0)
  %48 = load i16, ptr %10, !tbaa !2
  %49 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %11)
  %50 = sub i16 %48, 0
  %51 = getelementptr inbounds i16, ptr @"SP%", i16 %50
  store i16 %49, ptr %51, !tbaa !2
  %52 = load i16, ptr %10, !tbaa !2
  %53 = add i16 %52, 1
  store i16 %53, ptr %10, !tbaa !2
  %54 = load i16, ptr %2, !tbaa !2
  %55 = load i16, ptr %0, !tbaa !2
  %56 = add nsw i16 %54, %55
  store i16 %56, ptr %2, !tbaa !2
  br label %b12

b16:
  %57 = load i16, ptr %5, !tbaa !2
  %58 = load i16, ptr %3, !tbaa !2
  %59 = add nsw i16 %57, %58
  store i16 %59, ptr %5, !tbaa !2
  br label %b7
}

define cc 1000 void @LOADRECORD() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = load i16, ptr @"GGI%", !tbaa !2
  %10 = load i16, ptr @"GI%", !tbaa !2
  %11 = icmp slt i16 %9, %10
  br i1 %11, label %b2, label %b3

b2:
  %12 = load i16, ptr @"GGI%", !tbaa !2
  %13 = add i16 %12, 1
  store i16 %13, ptr @"GGI%", !tbaa !2
  br label %b4

b3:
  %14 = addrspacecast ptr @GI$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) %14, i16 0)
  %15 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @GI$)
  store i16 %15, ptr @"GI%", !tbaa !2
  %16 = addrspacecast ptr @GG$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) %16, i16 0)
  %17 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @GG$)
  store i16 %17, ptr @"GG%", !tbaa !2
  store i16 1, ptr @"GGI%", !tbaa !2
  %18 = load i16, ptr @"FILEI%", !tbaa !2
  %19 = add i16 %18, 2
  store i16 %19, ptr @"FILEI%", !tbaa !2
  br label %b4

b4:
  %20 = load i16, ptr @"GG%", !tbaa !2
  store i16 %20, ptr %8, !tbaa !2
  store i16 4, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  store i16 -1, ptr %5, !tbaa !2
  br label %b5

b5:
  %21 = load i16, ptr %5, !tbaa !2
  %22 = icmp sge i16 %21, 0
  br i1 %22, label %b6, label %b7

b6:
  %23 = load i16, ptr %7, !tbaa !2
  %24 = load i16, ptr %6, !tbaa !2
  %25 = icmp sle i16 %23, %24
  br i1 %25, label %b8, label %b9

b7:
  %26 = load i16, ptr %7, !tbaa !2
  %27 = load i16, ptr %6, !tbaa !2
  %28 = icmp sge i16 %26, %27
  br i1 %28, label %b8, label %b9

b8:
  %29 = load i16, ptr @"GG%", !tbaa !2
  %30 = load i16, ptr %7, !tbaa !2
  %31 = sub i16 %30, 1
  store i16 %31, ptr %4, !tbaa !2
  %32 = load i16, ptr %4, !tbaa !2
  %33 = sitofp i16 %32 to float
  %34 = load float, ptr @$float220, !tbaa !2
  %35 = fmul float %34, %33
  %36 = load float, ptr @$float219, !tbaa !2
  %37 = fcmp oge float %35, %36
  br i1 %37, label %b13, label %b14

b9:
  %38 = load i16, ptr %8, !tbaa !2
  store i16 %38, ptr @"GG%", !tbaa !2
  %39 = getelementptr inbounds i16, ptr @"KA%", i16 1
  %40 = load i16, ptr %39, !tbaa !2
  %41 = getelementptr inbounds i16, ptr @"KON%", i16 75
  store i16 %40, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i16, ptr @"KA%", i16 2
  %43 = load i16, ptr %42, !tbaa !2
  %44 = getelementptr inbounds i16, ptr @"KON%", i16 77
  store i16 %43, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i16, ptr @"KA%", i16 3
  %46 = load i16, ptr %45, !tbaa !2
  %47 = getelementptr inbounds i16, ptr @"KON%", i16 72
  store i16 %46, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i16, ptr @"KA%", i16 4
  %49 = load i16, ptr %48, !tbaa !2
  %50 = getelementptr inbounds i16, ptr @"KON%", i16 80
  store i16 %49, ptr %50, !tbaa !2
  ret void

b10:
  %51 = load i16, ptr @"GG%", !tbaa !2
  %52 = load i16, ptr %7, !tbaa !2
  %53 = sub i16 %52, 1
  store i16 %53, ptr %1, !tbaa !2
  %54 = load i16, ptr %1, !tbaa !2
  %55 = sitofp i16 %54 to float
  %56 = load float, ptr @$float220, !tbaa !2
  %57 = fmul float %56, %55
  %58 = load float, ptr @$float219, !tbaa !2
  %59 = fcmp oge float %57, %58
  br i1 %59, label %b15, label %b16

b11:
  %60 = load i16, ptr %7, !tbaa !2
  %61 = sub i16 %60, 0
  %62 = getelementptr inbounds i16, ptr @"KA%", i16 %61
  store i16 0, ptr %62, !tbaa !2
  br label %b12

b12:
  %63 = load i16, ptr %7, !tbaa !2
  %64 = load i16, ptr %5, !tbaa !2
  %65 = add nsw i16 %63, %64
  store i16 %65, ptr %7, !tbaa !2
  br label %b5

b13:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b14:
  %66 = call float @llvm.exp2.f32(float %35)
  store i16 %29, ptr %3, !tbaa !2
  %67 = load i16, ptr %3, !tbaa !2
  %68 = sitofp i16 %67 to float
  %69 = fsub float %68, %66
  store i16 0, ptr %2, !tbaa !2
  %70 = load i16, ptr %2, !tbaa !2
  %71 = sitofp i16 %70 to float
  %72 = fcmp oge float %69, %71
  br i1 %72, label %b10, label %b11

b15:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b16:
  %73 = call float @llvm.exp2.f32(float %57)
  store i16 %51, ptr %0, !tbaa !2
  %74 = load i16, ptr %0, !tbaa !2
  %75 = sitofp i16 %74 to float
  %76 = fsub float %75, %73
  %77 = call i16 @llvm.lrint.i16.f32(float %76)
  store i16 %77, ptr @"GG%", !tbaa !2
  %78 = load i16, ptr %7, !tbaa !2
  %79 = sub i16 %78, 0
  %80 = getelementptr inbounds i16, ptr @"KA%", i16 %79
  store i16 1, ptr %80, !tbaa !2
  br label %b12
}

define cc 1000 void @MOV3DPOS(ptr dereferenceable(4) %0, ptr dereferenceable(4) %1, ptr dereferenceable(4) %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  store i16 0, ptr %5, !tbaa !2
  %6 = load i16, ptr @"NDTS%", !tbaa !2
  %7 = sub i16 %6, 1
  store i16 %7, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = icmp sge i16 %8, 0
  br i1 %9, label %b3, label %b4

b3:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = icmp sle i16 %10, %11
  br i1 %12, label %b5, label %b6

b4:
  %13 = load i16, ptr %5, !tbaa !2
  %14 = load i16, ptr %4, !tbaa !2
  %15 = icmp sge i16 %13, %14
  br i1 %15, label %b5, label %b6

b5:
  %16 = load i16, ptr %5, !tbaa !2
  %17 = mul i16 %16, 4
  %18 = getelementptr i8, ptr @"X!", i16 2
  %19 = load i16, ptr %18, !tbaa !2
  %20 = add i16 0, %17
  %21 = inttoptr i16 %19 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 %20
  %24 = load i16, ptr %5, !tbaa !2
  %25 = mul i16 %24, 4
  %26 = getelementptr i8, ptr @"X!", i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = add i16 0, %25
  %29 = inttoptr i16 %27 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %28
  %32 = load float, ptr addrspace(1) %31, !tbaa !54
  %33 = load float, ptr %0
  %34 = fadd float %32, %33
  %35 = fpround float %34 to float
  store float %35, ptr addrspace(1) %23, !tbaa !54
  %36 = load i16, ptr %5, !tbaa !2
  %37 = mul i16 %36, 4
  %38 = getelementptr i8, ptr @"Y!", i16 2
  %39 = load i16, ptr %38, !tbaa !2
  %40 = add i16 0, %37
  %41 = inttoptr i16 %39 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 %40
  %44 = load i16, ptr %5, !tbaa !2
  %45 = mul i16 %44, 4
  %46 = getelementptr i8, ptr @"Y!", i16 2
  %47 = load i16, ptr %46, !tbaa !2
  %48 = add i16 0, %45
  %49 = inttoptr i16 %47 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = getelementptr i8, ptr addrspace(1) %50, i16 %48
  %52 = load float, ptr addrspace(1) %51, !tbaa !58
  %53 = load float, ptr %1
  %54 = fadd float %52, %53
  %55 = fpround float %54 to float
  store float %55, ptr addrspace(1) %43, !tbaa !58
  %56 = load i16, ptr %5, !tbaa !2
  %57 = mul i16 %56, 4
  %58 = getelementptr i8, ptr @"Z!", i16 2
  %59 = load i16, ptr %58, !tbaa !2
  %60 = add i16 0, %57
  %61 = inttoptr i16 %59 to ptr addrspace(2)
  %62 = addrspacecast ptr addrspace(2) %61 to ptr addrspace(1)
  %63 = getelementptr i8, ptr addrspace(1) %62, i16 %60
  %64 = load i16, ptr %5, !tbaa !2
  %65 = mul i16 %64, 4
  %66 = getelementptr i8, ptr @"Z!", i16 2
  %67 = load i16, ptr %66, !tbaa !2
  %68 = add i16 0, %65
  %69 = inttoptr i16 %67 to ptr addrspace(2)
  %70 = addrspacecast ptr addrspace(2) %69 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 %68
  %72 = load float, ptr addrspace(1) %71, !tbaa !56
  %73 = load float, ptr %2
  %74 = fadd float %72, %73
  %75 = fpround float %74 to float
  store float %75, ptr addrspace(1) %63, !tbaa !56
  %76 = load i16, ptr %5, !tbaa !2
  %77 = load i16, ptr %3, !tbaa !2
  %78 = add nsw i16 %76, %77
  store i16 %78, ptr %5, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @OUTPUT0() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = load i16, ptr @"PX%", !tbaa !2
  %15 = sext i16 %14 to i32
  %16 = sdiv i32 %15, 64
  %17 = trunc i32 %16 to i16
  store i16 %17, ptr %13, !tbaa !2
  %18 = load i16, ptr @"PY%", !tbaa !2
  %19 = sext i16 %18 to i32
  %20 = sdiv i32 %19, 64
  %21 = trunc i32 %20 to i16
  store i16 %21, ptr %12, !tbaa !2
  %22 = load i16, ptr @"MAP%", !tbaa !2
  %23 = icmp eq i16 %22, 0
  br i1 %23, label %b3, label %b4

b2:
  ret void

b3:
  store i16 0, ptr %11, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 199, ptr %9, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  br label %b5

b4:
  %24 = icmp eq i16 %22, 1
  br i1 %24, label %b18, label %b19

b5:
  %25 = load i16, ptr %8, !tbaa !2
  %26 = icmp sge i16 %25, 0
  br i1 %26, label %b6, label %b7

b6:
  %27 = load i16, ptr %10, !tbaa !2
  %28 = load i16, ptr %9, !tbaa !2
  %29 = icmp sle i16 %27, %28
  br i1 %29, label %b8, label %b9

b7:
  %30 = load i16, ptr %10, !tbaa !2
  %31 = load i16, ptr %9, !tbaa !2
  %32 = icmp sge i16 %30, %31
  br i1 %32, label %b8, label %b9

b8:
  %33 = load i16, ptr %11, !tbaa !2
  %34 = add i16 -24576, %33
  store i16 %34, ptr @b$seg, !tbaa !2
  %35 = load i16, ptr %11, !tbaa !2
  %36 = add i16 %35, 20
  store i16 %36, ptr %11, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 319, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b10

b9:
  %37 = load i16, ptr @"PY%", !tbaa !2
  %38 = sext i16 %37 to i32
  %39 = sdiv i32 %38, 16
  %40 = trunc i32 %39 to i16
  %41 = sub i16 %40, 0
  %42 = getelementptr inbounds i16, ptr @"MUL128%", i16 %41
  %43 = load i16, ptr %42, !tbaa !2
  %44 = load i16, ptr @"PX%", !tbaa !2
  %45 = sext i16 %44 to i32
  %46 = sdiv i32 %45, 16
  %47 = trunc i32 %46 to i16
  %48 = add i16 %43, %47
  %49 = sub i16 %48, 0
  %50 = getelementptr inbounds i16, ptr @"SP%", i16 %49
  store i16 24, ptr %50, !tbaa !2
  br label %b2

b10:
  %51 = load i16, ptr %5, !tbaa !2
  %52 = icmp sge i16 %51, 0
  br i1 %52, label %b11, label %b12

b11:
  %53 = load i16, ptr %7, !tbaa !2
  %54 = load i16, ptr %6, !tbaa !2
  %55 = icmp sle i16 %53, %54
  br i1 %55, label %b13, label %b14

b12:
  %56 = load i16, ptr %7, !tbaa !2
  %57 = load i16, ptr %6, !tbaa !2
  %58 = icmp sge i16 %56, %57
  br i1 %58, label %b13, label %b14

b13:
  %59 = load i16, ptr %10, !tbaa !2
  %60 = load i16, ptr %7, !tbaa !2
  %61 = sub i16 %60, 0
  %62 = getelementptr inbounds i16, ptr @"SLA%", i16 %61
  %63 = load i16, ptr %62, !tbaa !2
  %64 = icmp sgt i16 %59, %63
  %65 = sext i1 %64 to i16
  %66 = load i16, ptr %10, !tbaa !2
  %67 = load i16, ptr %7, !tbaa !2
  %68 = sub i16 %67, 0
  %69 = getelementptr inbounds i16, ptr @"SLB%", i16 %68
  %70 = load i16, ptr %69, !tbaa !2
  %71 = icmp slt i16 %66, %70
  %72 = sext i1 %71 to i16
  %73 = and i16 %65, %72
  %74 = icmp ne i16 %73, 0
  br i1 %74, label %b15, label %b16

b14:
  %75 = load i16, ptr %10, !tbaa !2
  %76 = load i16, ptr %8, !tbaa !2
  %77 = add nsw i16 %75, %76
  store i16 %77, ptr %10, !tbaa !2
  br label %b5

b15:
  %78 = load i16, ptr %7, !tbaa !2
  %79 = load i16, ptr %7, !tbaa !2
  %80 = sub i16 %79, 0
  %81 = getelementptr inbounds i16, ptr @"SC%", i16 %80
  %82 = load i16, ptr %81, !tbaa !2
  %83 = trunc i16 %82 to i8
  %84 = load i16, ptr @b$seg, !tbaa !2
  %85 = inttoptr i16 %84 to ptr addrspace(2)
  %86 = addrspacecast ptr addrspace(2) %85 to ptr addrspace(1)
  %87 = getelementptr i8, ptr addrspace(1) %86, i16 %78
  store volatile i8 %83, ptr addrspace(1) %87
  br label %b17

b16:
  %88 = load i16, ptr %7, !tbaa !2
  %89 = load i16, ptr %10, !tbaa !2
  %90 = sub i16 %89, 0
  %91 = getelementptr inbounds i16, ptr @"FS%", i16 %90
  %92 = load i16, ptr %91, !tbaa !2
  %93 = trunc i16 %92 to i8
  %94 = load i16, ptr @b$seg, !tbaa !2
  %95 = inttoptr i16 %94 to ptr addrspace(2)
  %96 = addrspacecast ptr addrspace(2) %95 to ptr addrspace(1)
  %97 = getelementptr i8, ptr addrspace(1) %96, i16 %88
  store volatile i8 %93, ptr addrspace(1) %97
  br label %b17

b17:
  %98 = load i16, ptr %7, !tbaa !2
  %99 = load i16, ptr %5, !tbaa !2
  %100 = add nsw i16 %98, %99
  store i16 %100, ptr %7, !tbaa !2
  br label %b10

b18:
  store i16 0, ptr %11, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 199, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b20

b19:
  br label %b2

b20:
  %101 = load i16, ptr %3, !tbaa !2
  %102 = icmp sge i16 %101, 0
  br i1 %102, label %b21, label %b22

b21:
  %103 = load i16, ptr %10, !tbaa !2
  %104 = load i16, ptr %4, !tbaa !2
  %105 = icmp sle i16 %103, %104
  br i1 %105, label %b23, label %b24

b22:
  %106 = load i16, ptr %10, !tbaa !2
  %107 = load i16, ptr %4, !tbaa !2
  %108 = icmp sge i16 %106, %107
  br i1 %108, label %b23, label %b24

b23:
  %109 = load i16, ptr %11, !tbaa !2
  %110 = add i16 -24576, %109
  store i16 %110, ptr @b$seg, !tbaa !2
  %111 = load i16, ptr %11, !tbaa !2
  %112 = add i16 %111, 20
  store i16 %112, ptr %11, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 319, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b25

b24:
  %113 = load i16, ptr @"PY%", !tbaa !2
  %114 = sext i16 %113 to i32
  %115 = sdiv i32 %114, 16
  %116 = trunc i32 %115 to i16
  %117 = sub i16 %116, 0
  %118 = getelementptr inbounds i16, ptr @"MUL128%", i16 %117
  %119 = load i16, ptr %118, !tbaa !2
  %120 = load i16, ptr @"PX%", !tbaa !2
  %121 = sext i16 %120 to i32
  %122 = sdiv i32 %121, 16
  %123 = trunc i32 %122 to i16
  %124 = add i16 %119, %123
  %125 = sub i16 %124, 0
  %126 = getelementptr inbounds i16, ptr @"SP%", i16 %125
  store i16 24, ptr %126, !tbaa !2
  br label %b2

b25:
  %127 = load i16, ptr %1, !tbaa !2
  %128 = icmp sge i16 %127, 0
  br i1 %128, label %b26, label %b27

b26:
  %129 = load i16, ptr %7, !tbaa !2
  %130 = load i16, ptr %2, !tbaa !2
  %131 = icmp sle i16 %129, %130
  br i1 %131, label %b28, label %b29

b27:
  %132 = load i16, ptr %7, !tbaa !2
  %133 = load i16, ptr %2, !tbaa !2
  %134 = icmp sge i16 %132, %133
  br i1 %134, label %b28, label %b29

b28:
  %135 = load i16, ptr %10, !tbaa !2
  %136 = load i16, ptr %7, !tbaa !2
  %137 = sub i16 %136, 0
  %138 = getelementptr inbounds i16, ptr @"SLA%", i16 %137
  %139 = load i16, ptr %138, !tbaa !2
  %140 = icmp sgt i16 %135, %139
  %141 = sext i1 %140 to i16
  %142 = load i16, ptr %10, !tbaa !2
  %143 = load i16, ptr %7, !tbaa !2
  %144 = sub i16 %143, 0
  %145 = getelementptr inbounds i16, ptr @"SLB%", i16 %144
  %146 = load i16, ptr %145, !tbaa !2
  %147 = icmp slt i16 %142, %146
  %148 = sext i1 %147 to i16
  %149 = and i16 %141, %148
  %150 = icmp ne i16 %149, 0
  br i1 %150, label %b30, label %b31

b29:
  %151 = load i16, ptr %10, !tbaa !2
  %152 = load i16, ptr %3, !tbaa !2
  %153 = add nsw i16 %151, %152
  store i16 %153, ptr %10, !tbaa !2
  br label %b20

b30:
  %154 = load i16, ptr %7, !tbaa !2
  %155 = sub i16 %154, 0
  %156 = getelementptr inbounds i16, ptr @"SC%", i16 %155
  %157 = load i16, ptr %156, !tbaa !2
  store i16 %157, ptr %0, !tbaa !2
  br label %b32

b31:
  %158 = load i16, ptr %10, !tbaa !2
  %159 = sub i16 %158, 0
  %160 = getelementptr inbounds i16, ptr @"FS%", i16 %159
  %161 = load i16, ptr %160, !tbaa !2
  store i16 %161, ptr %0, !tbaa !2
  br label %b32

b32:
  %162 = load i16, ptr %7, !tbaa !2
  %163 = icmp sgt i16 %162, 127
  %164 = sext i1 %163 to i16
  %165 = load i16, ptr %10, !tbaa !2
  %166 = icmp sgt i16 %165, 127
  %167 = sext i1 %166 to i16
  %168 = or i16 %164, %167
  %169 = icmp ne i16 %168, 0
  br i1 %169, label %b33, label %b34

b33:
  %170 = load i16, ptr %7, !tbaa !2
  %171 = load i16, ptr %0, !tbaa !2
  %172 = trunc i16 %171 to i8
  %173 = load i16, ptr @b$seg, !tbaa !2
  %174 = inttoptr i16 %173 to ptr addrspace(2)
  %175 = addrspacecast ptr addrspace(2) %174 to ptr addrspace(1)
  %176 = getelementptr i8, ptr addrspace(1) %175, i16 %170
  store volatile i8 %172, ptr addrspace(1) %176
  br label %b35

b34:
  %177 = load i16, ptr %7, !tbaa !2
  %178 = load i16, ptr %0, !tbaa !2
  %179 = sub i16 %178, 0
  %180 = getelementptr inbounds i16, ptr @"DIV2%", i16 %179
  %181 = load i16, ptr %180, !tbaa !2
  %182 = load i16, ptr %10, !tbaa !2
  %183 = sub i16 %182, 0
  %184 = getelementptr inbounds i16, ptr @"MUL128%", i16 %183
  %185 = load i16, ptr %184, !tbaa !2
  %186 = load i16, ptr %7, !tbaa !2
  %187 = add i16 %185, %186
  %188 = sub i16 %187, 0
  %189 = getelementptr inbounds i16, ptr @"SP%", i16 %188
  %190 = load i16, ptr %189, !tbaa !2
  %191 = add i16 %181, %190
  %192 = trunc i16 %191 to i8
  %193 = load i16, ptr @b$seg, !tbaa !2
  %194 = inttoptr i16 %193 to ptr addrspace(2)
  %195 = addrspacecast ptr addrspace(2) %194 to ptr addrspace(1)
  %196 = getelementptr i8, ptr addrspace(1) %195, i16 %177
  store volatile i8 %192, ptr addrspace(1) %196
  br label %b35

b35:
  %197 = load i16, ptr %7, !tbaa !2
  %198 = load i16, ptr %1, !tbaa !2
  %199 = add nsw i16 %197, %198
  store i16 %199, ptr %7, !tbaa !2
  br label %b25
}

define cc 1000 void @OUTPUT3D() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = load i16, ptr @"OBJ%", !tbaa !2
  %14 = mul i16 %13, 2
  %15 = getelementptr i8, ptr @"ZPO%", i16 2
  %16 = load i16, ptr %15, !tbaa !2
  %17 = add i16 0, %14
  %18 = inttoptr i16 %16 to ptr addrspace(2)
  %19 = addrspacecast ptr addrspace(2) %18 to ptr addrspace(1)
  %20 = getelementptr i8, ptr addrspace(1) %19, i16 %17
  %21 = load i16, ptr addrspace(1) %20, !tbaa !14
  store i16 %21, ptr %12, !tbaa !2
  %22 = load i16, ptr @"FTYPE%", !tbaa !2
  %23 = icmp eq i16 %22, 0
  br i1 %23, label %b3, label %b4

b2:
  ret void

b3:
  store i16 0, ptr %11, !tbaa !2
  %24 = load i16, ptr @"NDTS%", !tbaa !2
  %25 = sub i16 %24, 1
  store i16 %25, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  br label %b5

b4:
  store i16 1, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @CLEARSCREEN(ptr %2)
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %26 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %26, ptr @"BENCHFRAME&", !tbaa !2
  %27 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %27, ptr @"BENCHFRAME&", !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  store i16 0, ptr %11, !tbaa !2
  %28 = load i16, ptr @"NLNS%", !tbaa !2
  %29 = sub i16 %28, 1
  store i16 %29, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b33

b5:
  %30 = load i16, ptr %9, !tbaa !2
  %31 = icmp sge i16 %30, 0
  br i1 %31, label %b6, label %b7

b6:
  %32 = load i16, ptr %11, !tbaa !2
  %33 = load i16, ptr %10, !tbaa !2
  %34 = icmp sle i16 %32, %33
  br i1 %34, label %b8, label %b9

b7:
  %35 = load i16, ptr %11, !tbaa !2
  %36 = load i16, ptr %10, !tbaa !2
  %37 = icmp sge i16 %35, %36
  br i1 %37, label %b8, label %b9

b8:
  %38 = load i16, ptr %11, !tbaa !2
  %39 = add i16 4097, %38
  %40 = mul i16 %39, 2
  %41 = getelementptr i8, ptr @"XS%", i16 2
  %42 = load i16, ptr %41, !tbaa !2
  %43 = add i16 0, %40
  %44 = inttoptr i16 %42 to ptr addrspace(2)
  %45 = addrspacecast ptr addrspace(2) %44 to ptr addrspace(1)
  %46 = getelementptr i8, ptr addrspace(1) %45, i16 %43
  %47 = load i16, ptr addrspace(1) %46, !tbaa !66
  %48 = add i16 %47, 160
  store i16 %48, ptr %8, !tbaa !2
  %49 = load i16, ptr %8, !tbaa !2
  %50 = icmp sgt i16 %49, -1
  %51 = sext i1 %50 to i16
  %52 = load i16, ptr %8, !tbaa !2
  %53 = icmp slt i16 %52, 320
  %54 = sext i1 %53 to i16
  %55 = and i16 %51, %54
  %56 = load i16, ptr %11, !tbaa !2
  %57 = add i16 4097, %56
  %58 = mul i16 %57, 2
  %59 = getelementptr i8, ptr @"YS%", i16 2
  %60 = load i16, ptr %59, !tbaa !2
  %61 = add i16 0, %58
  %62 = inttoptr i16 %60 to ptr addrspace(2)
  %63 = addrspacecast ptr addrspace(2) %62 to ptr addrspace(1)
  %64 = getelementptr i8, ptr addrspace(1) %63, i16 %61
  %65 = load i16, ptr addrspace(1) %64, !tbaa !68
  %66 = icmp sgt i16 %65, -101
  %67 = sext i1 %66 to i16
  %68 = and i16 %55, %67
  %69 = load i16, ptr %11, !tbaa !2
  %70 = add i16 4097, %69
  %71 = mul i16 %70, 2
  %72 = getelementptr i8, ptr @"YS%", i16 2
  %73 = load i16, ptr %72, !tbaa !2
  %74 = add i16 0, %71
  %75 = inttoptr i16 %73 to ptr addrspace(2)
  %76 = addrspacecast ptr addrspace(2) %75 to ptr addrspace(1)
  %77 = getelementptr i8, ptr addrspace(1) %76, i16 %74
  %78 = load i16, ptr addrspace(1) %77, !tbaa !68
  %79 = icmp slt i16 %78, 100
  %80 = sext i1 %79 to i16
  %81 = and i16 %68, %80
  %82 = icmp ne i16 %81, 0
  br i1 %82, label %b10, label %b11

b9:
  %83 = load i16, ptr @"FILEI%", !tbaa !2
  %84 = icmp sge i16 %83, 0
  br i1 %84, label %b23, label %b22

b10:
  %85 = load i16, ptr %11, !tbaa !2
  %86 = add i16 4097, %85
  %87 = mul i16 %86, 2
  %88 = getelementptr i8, ptr @"YS%", i16 2
  %89 = load i16, ptr %88, !tbaa !2
  %90 = add i16 0, %87
  %91 = inttoptr i16 %89 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 %90
  %94 = load i16, ptr addrspace(1) %93, !tbaa !68
  %95 = mul i16 %94, 4
  %96 = getelementptr i8, ptr @"YPK&", i16 2
  %97 = load i16, ptr %96, !tbaa !2
  %98 = add i16 400, %95
  %99 = inttoptr i16 %97 to ptr addrspace(2)
  %100 = addrspacecast ptr addrspace(2) %99 to ptr addrspace(1)
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 %98
  %102 = load i32, ptr addrspace(1) %101, !tbaa !12
  %103 = load i16, ptr %8, !tbaa !2
  %104 = sext i16 %103 to i32
  %105 = add i32 %102, %104
  %106 = trunc i32 %105 to i16
  %107 = load i16, ptr @b$seg, !tbaa !2
  %108 = inttoptr i16 %107 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 %106
  store volatile i8 0, ptr addrspace(1) %110
  br label %b12

b11:
  br label %b12

b12:
  %111 = load i16, ptr %11, !tbaa !2
  %112 = add i16 0, %111
  %113 = mul i16 %112, 2
  %114 = getelementptr i8, ptr @"XS%", i16 2
  %115 = load i16, ptr %114, !tbaa !2
  %116 = add i16 0, %113
  %117 = inttoptr i16 %115 to ptr addrspace(2)
  %118 = addrspacecast ptr addrspace(2) %117 to ptr addrspace(1)
  %119 = getelementptr i8, ptr addrspace(1) %118, i16 %116
  %120 = load i16, ptr addrspace(1) %119, !tbaa !66
  %121 = add i16 %120, 160
  store i16 %121, ptr %7, !tbaa !2
  %122 = load i16, ptr %11, !tbaa !2
  %123 = mul i16 %122, 4
  %124 = getelementptr i8, ptr @"Z!", i16 2
  %125 = load i16, ptr %124, !tbaa !2
  %126 = add i16 0, %123
  %127 = inttoptr i16 %125 to ptr addrspace(2)
  %128 = addrspacecast ptr addrspace(2) %127 to ptr addrspace(1)
  %129 = getelementptr i8, ptr addrspace(1) %128, i16 %126
  %130 = load float, ptr addrspace(1) %129, !tbaa !56
  %131 = load i16, ptr %12, !tbaa !2
  store i16 80, ptr %5, !tbaa !2
  %132 = load i16, ptr %5, !tbaa !2
  %133 = sitofp i16 %132 to float
  store i16 %131, ptr %4, !tbaa !2
  %134 = load i16, ptr %4, !tbaa !2
  %135 = sitofp i16 %134 to float
  %136 = fdiv float %133, %135
  %137 = fmul float %130, %136
  store i16 256, ptr %3, !tbaa !2
  %138 = load i16, ptr %3, !tbaa !2
  %139 = sitofp i16 %138 to float
  %140 = fsub float %139, %137
  %141 = call i16 @llvm.lrint.i16.f32(float %140)
  store i16 %141, ptr %6, !tbaa !2
  %142 = load i16, ptr %6, !tbaa !2
  %143 = icmp slt i16 %142, 80
  br i1 %143, label %b13, label %b14

b13:
  store i16 80, ptr %6, !tbaa !2
  br label %b15

b14:
  br label %b15

b15:
  %144 = load i16, ptr %7, !tbaa !2
  %145 = icmp sgt i16 %144, -1
  %146 = sext i1 %145 to i16
  %147 = load i16, ptr %7, !tbaa !2
  %148 = icmp slt i16 %147, 320
  %149 = sext i1 %148 to i16
  %150 = and i16 %146, %149
  %151 = load i16, ptr %11, !tbaa !2
  %152 = add i16 0, %151
  %153 = mul i16 %152, 2
  %154 = getelementptr i8, ptr @"YS%", i16 2
  %155 = load i16, ptr %154, !tbaa !2
  %156 = add i16 0, %153
  %157 = inttoptr i16 %155 to ptr addrspace(2)
  %158 = addrspacecast ptr addrspace(2) %157 to ptr addrspace(1)
  %159 = getelementptr i8, ptr addrspace(1) %158, i16 %156
  %160 = load i16, ptr addrspace(1) %159, !tbaa !68
  %161 = icmp sgt i16 %160, -101
  %162 = sext i1 %161 to i16
  %163 = and i16 %150, %162
  %164 = load i16, ptr %11, !tbaa !2
  %165 = add i16 0, %164
  %166 = mul i16 %165, 2
  %167 = getelementptr i8, ptr @"YS%", i16 2
  %168 = load i16, ptr %167, !tbaa !2
  %169 = add i16 0, %166
  %170 = inttoptr i16 %168 to ptr addrspace(2)
  %171 = addrspacecast ptr addrspace(2) %170 to ptr addrspace(1)
  %172 = getelementptr i8, ptr addrspace(1) %171, i16 %169
  %173 = load i16, ptr addrspace(1) %172, !tbaa !68
  %174 = icmp slt i16 %173, 100
  %175 = sext i1 %174 to i16
  %176 = and i16 %163, %175
  %177 = icmp ne i16 %176, 0
  br i1 %177, label %b16, label %b17

b16:
  %178 = load i16, ptr %11, !tbaa !2
  %179 = add i16 0, %178
  %180 = mul i16 %179, 2
  %181 = getelementptr i8, ptr @"YS%", i16 2
  %182 = load i16, ptr %181, !tbaa !2
  %183 = add i16 0, %180
  %184 = inttoptr i16 %182 to ptr addrspace(2)
  %185 = addrspacecast ptr addrspace(2) %184 to ptr addrspace(1)
  %186 = getelementptr i8, ptr addrspace(1) %185, i16 %183
  %187 = load i16, ptr addrspace(1) %186, !tbaa !68
  %188 = mul i16 %187, 4
  %189 = getelementptr i8, ptr @"YPK&", i16 2
  %190 = load i16, ptr %189, !tbaa !2
  %191 = add i16 400, %188
  %192 = inttoptr i16 %190 to ptr addrspace(2)
  %193 = addrspacecast ptr addrspace(2) %192 to ptr addrspace(1)
  %194 = getelementptr i8, ptr addrspace(1) %193, i16 %191
  %195 = load i32, ptr addrspace(1) %194, !tbaa !12
  %196 = load i16, ptr %7, !tbaa !2
  %197 = sext i16 %196 to i32
  %198 = add i32 %195, %197
  %199 = trunc i32 %198 to i16
  %200 = load i16, ptr %6, !tbaa !2
  %201 = trunc i16 %200 to i8
  %202 = load i16, ptr @b$seg, !tbaa !2
  %203 = inttoptr i16 %202 to ptr addrspace(2)
  %204 = addrspacecast ptr addrspace(2) %203 to ptr addrspace(1)
  %205 = getelementptr i8, ptr addrspace(1) %204, i16 %199
  store volatile i8 %201, ptr addrspace(1) %205
  br label %b18

b17:
  br label %b18

b18:
  %206 = load i16, ptr %11, !tbaa !2
  %207 = load i16, ptr %9, !tbaa !2
  %208 = add nsw i16 %206, %207
  store i16 %208, ptr %11, !tbaa !2
  br label %b5

b19:
  br label %b2

b20:
  br label %b19

b21:
  %209 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %209, ptr @"BENCHFRAME&", !tbaa !2
  %210 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %210, ptr @"BENCHFRAME&", !tbaa !2
  br label %b19

b22:
  %211 = icmp sge i16 %83, 780
  br i1 %211, label %b25, label %b24

b23:
  %212 = icmp sle i16 %83, 10
  br i1 %212, label %b20, label %b22

b24:
  %213 = icmp sge i16 %83, 1320
  br i1 %213, label %b27, label %b26

b25:
  %214 = icmp sle i16 %83, 936
  br i1 %214, label %b20, label %b24

b26:
  %215 = icmp sge i16 %83, 2162
  br i1 %215, label %b29, label %b28

b27:
  %216 = icmp sle i16 %83, 1440
  br i1 %216, label %b20, label %b26

b28:
  %217 = icmp sge i16 %83, 3520
  br i1 %217, label %b31, label %b30

b29:
  %218 = icmp sle i16 %83, 2400
  br i1 %218, label %b20, label %b28

b30:
  %219 = icmp sge i16 %83, 4180
  br i1 %219, label %b32, label %b21

b31:
  %220 = icmp sle i16 %83, 3640
  br i1 %220, label %b20, label %b30

b32:
  %221 = icmp sle i16 %83, 4230
  br i1 %221, label %b20, label %b21

b33:
  %222 = load i16, ptr %0, !tbaa !2
  %223 = icmp sge i16 %222, 0
  br i1 %223, label %b34, label %b35

b34:
  %224 = load i16, ptr %11, !tbaa !2
  %225 = load i16, ptr %1, !tbaa !2
  %226 = icmp sle i16 %224, %225
  br i1 %226, label %b36, label %b37

b35:
  %227 = load i16, ptr %11, !tbaa !2
  %228 = load i16, ptr %1, !tbaa !2
  %229 = icmp sge i16 %227, %228
  br i1 %229, label %b36, label %b37

b36:
  %230 = load i16, ptr %11, !tbaa !2
  %231 = load i16, ptr %0, !tbaa !2
  %232 = add nsw i16 %230, %231
  store i16 %232, ptr %11, !tbaa !2
  br label %b33

b37:
  br label %b2
}

define cc 1000 void @PLASMABLOBS() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca i16
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca i16
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca i16
  %79 = alloca i32
  %80 = alloca i32
  %81 = alloca float
  %82 = alloca float
  %83 = alloca float
  %84 = alloca float
  %85 = alloca i16
  %86 = alloca float
  %87 = alloca i16
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca i16
  %91 = alloca i16
  %92 = alloca i16
  %93 = alloca i16
  %94 = alloca i16
  %95 = alloca i16
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  %103 = alloca i16
  %104 = alloca i16
  %105 = alloca i16
  %106 = alloca i16
  %107 = alloca i16
  %108 = alloca i16
  %109 = alloca i16
  %110 = alloca i16
  %111 = alloca i16
  %112 = alloca i16
  %113 = alloca i16
  %114 = alloca i16
  %115 = alloca i16
  %116 = alloca i16
  %117 = alloca i16
  %118 = alloca i16
  %119 = alloca i16
  %120 = alloca i16
  %121 = alloca i16
  %122 = alloca i16
  %123 = alloca i16
  %124 = alloca i16
  %125 = alloca i16
  %126 = alloca i16
  %127 = alloca i16
  %128 = alloca i16
  %129 = alloca i16
  %130 = alloca i16
  %131 = alloca float
  %132 = alloca [18 x i8]
  store i16 0, ptr %132, !tbaa !2
  %133 = getelementptr inbounds i8, ptr %132, i16 2
  store i16 0, ptr %133, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %132, i16 4
  store i16 0, ptr %134, !tbaa !2
  %135 = getelementptr inbounds i8, ptr %132, i16 6
  store i16 0, ptr %135, !tbaa !2
  %136 = getelementptr inbounds i8, ptr %132, i16 8
  store i16 0, ptr %136, !tbaa !2
  %137 = getelementptr inbounds i8, ptr %132, i16 10
  store i16 0, ptr %137, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %132, i16 12
  store i16 0, ptr %138, !tbaa !2
  %139 = getelementptr inbounds i8, ptr %132, i16 14
  store i16 0, ptr %139, !tbaa !2
  %140 = getelementptr inbounds i8, ptr %132, i16 16
  store i16 0, ptr %140, !tbaa !2
  store i16 0, ptr %36, !tbaa !2
  %141 = load float, ptr @$float229, !tbaa !2
  store float %141, ptr %131, !tbaa !2
  store i16 3, ptr %130, !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %129, !tbaa !2
  store i16 127, ptr %128, !tbaa !2
  store i16 1, ptr %127, !tbaa !2
  br label %b5

b2:
  %142 = load i16, ptr %36, !tbaa !2
  %143 = icmp eq i16 %142, 0
  br i1 %143, label %b195, label %b196

b3:
  store i16 0, ptr %6, !tbaa !2
  store i16 0, ptr %93, !tbaa !2
  store i16 199, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b207

b4:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %132)
  ret void

b5:
  %144 = load i16, ptr %127, !tbaa !2
  %145 = icmp sge i16 %144, 0
  br i1 %145, label %b6, label %b7

b6:
  %146 = load i16, ptr %129, !tbaa !2
  %147 = load i16, ptr %128, !tbaa !2
  %148 = icmp sle i16 %146, %147
  br i1 %148, label %b8, label %b9

b7:
  %149 = load i16, ptr %129, !tbaa !2
  %150 = load i16, ptr %128, !tbaa !2
  %151 = icmp sge i16 %149, %150
  br i1 %151, label %b8, label %b9

b8:
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %152 = load i16, ptr %129, !tbaa !2
  %153 = load i16, ptr %127, !tbaa !2
  %154 = add nsw i16 %152, %153
  store i16 %154, ptr %129, !tbaa !2
  br label %b5

b9:
  store i16 128, ptr %129, !tbaa !2
  store i16 159, ptr %126, !tbaa !2
  store i16 1, ptr %125, !tbaa !2
  br label %b10

b10:
  %155 = load i16, ptr %125, !tbaa !2
  %156 = icmp sge i16 %155, 0
  br i1 %156, label %b11, label %b12

b11:
  %157 = load i16, ptr %129, !tbaa !2
  %158 = load i16, ptr %126, !tbaa !2
  %159 = icmp sle i16 %157, %158
  br i1 %159, label %b13, label %b14

b12:
  %160 = load i16, ptr %129, !tbaa !2
  %161 = load i16, ptr %126, !tbaa !2
  %162 = icmp sge i16 %160, %161
  br i1 %162, label %b13, label %b14

b13:
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %163 = load i16, ptr %129, !tbaa !2
  %164 = sub i16 %163, 128
  %165 = mul i16 %164, 2
  %166 = trunc i16 %165 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %166)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %167 = load i16, ptr %129, !tbaa !2
  %168 = load i16, ptr %125, !tbaa !2
  %169 = add nsw i16 %167, %168
  store i16 %169, ptr %129, !tbaa !2
  br label %b10

b14:
  store i16 160, ptr %129, !tbaa !2
  store i16 191, ptr %124, !tbaa !2
  store i16 1, ptr %123, !tbaa !2
  br label %b15

b15:
  %170 = load i16, ptr %123, !tbaa !2
  %171 = icmp sge i16 %170, 0
  br i1 %171, label %b16, label %b17

b16:
  %172 = load i16, ptr %129, !tbaa !2
  %173 = load i16, ptr %124, !tbaa !2
  %174 = icmp sle i16 %172, %173
  br i1 %174, label %b18, label %b19

b17:
  %175 = load i16, ptr %129, !tbaa !2
  %176 = load i16, ptr %124, !tbaa !2
  %177 = icmp sge i16 %175, %176
  br i1 %177, label %b18, label %b19

b18:
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %178 = load i16, ptr %129, !tbaa !2
  %179 = sub i16 191, %178
  %180 = mul i16 %179, 2
  %181 = trunc i16 %180 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %181)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %182 = load i16, ptr %129, !tbaa !2
  %183 = load i16, ptr %123, !tbaa !2
  %184 = add nsw i16 %182, %183
  store i16 %184, ptr %129, !tbaa !2
  br label %b15

b19:
  store i16 192, ptr %129, !tbaa !2
  store i16 255, ptr %122, !tbaa !2
  store i16 1, ptr %121, !tbaa !2
  br label %b20

b20:
  %185 = load i16, ptr %121, !tbaa !2
  %186 = icmp sge i16 %185, 0
  br i1 %186, label %b21, label %b22

b21:
  %187 = load i16, ptr %129, !tbaa !2
  %188 = load i16, ptr %122, !tbaa !2
  %189 = icmp sle i16 %187, %188
  br i1 %189, label %b23, label %b24

b22:
  %190 = load i16, ptr %129, !tbaa !2
  %191 = load i16, ptr %122, !tbaa !2
  %192 = icmp sge i16 %190, %191
  br i1 %192, label %b23, label %b24

b23:
  %193 = load i16, ptr %129, !tbaa !2
  %194 = sub i16 %193, 192
  %195 = trunc i16 %194 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %195)
  %196 = load i16, ptr %129, !tbaa !2
  %197 = sub i16 %196, 192
  %198 = trunc i16 %197 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %198)
  %199 = load i16, ptr %129, !tbaa !2
  %200 = sub i16 %199, 192
  %201 = trunc i16 %200 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %201)
  %202 = load i16, ptr %129, !tbaa !2
  %203 = load i16, ptr %121, !tbaa !2
  %204 = add nsw i16 %202, %203
  store i16 %204, ptr %129, !tbaa !2
  br label %b20

b24:
  call cc 1000 addrspace(1) void @GETPAL()
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %129, !tbaa !2
  store i16 255, ptr %120, !tbaa !2
  store i16 1, ptr %119, !tbaa !2
  br label %b25

b25:
  %205 = load i16, ptr %119, !tbaa !2
  %206 = icmp sge i16 %205, 0
  br i1 %206, label %b26, label %b27

b26:
  %207 = load i16, ptr %129, !tbaa !2
  %208 = load i16, ptr %120, !tbaa !2
  %209 = icmp sle i16 %207, %208
  br i1 %209, label %b28, label %b29

b27:
  %210 = load i16, ptr %129, !tbaa !2
  %211 = load i16, ptr %120, !tbaa !2
  %212 = icmp sge i16 %210, %211
  br i1 %212, label %b28, label %b29

b28:
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %213 = load i16, ptr %129, !tbaa !2
  %214 = load i16, ptr %119, !tbaa !2
  %215 = add nsw i16 %213, %214
  store i16 %215, ptr %129, !tbaa !2
  br label %b25

b29:
  store i16 -157, ptr %129, !tbaa !2
  store i16 518, ptr %118, !tbaa !2
  store i16 1, ptr %117, !tbaa !2
  br label %b30

b30:
  %216 = load i16, ptr %117, !tbaa !2
  %217 = icmp sge i16 %216, 0
  br i1 %217, label %b31, label %b32

b31:
  %218 = load i16, ptr %129, !tbaa !2
  %219 = load i16, ptr %118, !tbaa !2
  %220 = icmp sle i16 %218, %219
  br i1 %220, label %b33, label %b34

b32:
  %221 = load i16, ptr %129, !tbaa !2
  %222 = load i16, ptr %118, !tbaa !2
  %223 = icmp sge i16 %221, %222
  br i1 %223, label %b33, label %b34

b33:
  %224 = load i16, ptr %129, !tbaa !2
  %225 = icmp sgt i16 %224, -1
  %226 = sext i1 %225 to i16
  %227 = load i16, ptr %129, !tbaa !2
  %228 = icmp slt i16 %227, 519
  %229 = sext i1 %228 to i16
  %230 = and i16 %226, %229
  %231 = icmp ne i16 %230, 0
  br i1 %231, label %b35, label %b36

b34:
  store i16 -1, ptr %107, !tbaa !2
  store i16 -1, ptr %129, !tbaa !2
  store i16 -156, ptr %106, !tbaa !2
  store i16 -1, ptr %105, !tbaa !2
  br label %b47

b35:
  %232 = load i16, ptr %129, !tbaa !2
  %233 = load i16, ptr %129, !tbaa !2
  store i16 %233, ptr %116, !tbaa !2
  %234 = load i16, ptr %116, !tbaa !2
  %235 = sitofp i16 %234 to float
  store i16 45, ptr %115, !tbaa !2
  %236 = load i16, ptr %115, !tbaa !2
  %237 = sitofp i16 %236 to float
  %238 = fdiv float %235, %237
  %239 = call float @llvm.fabs.f32(float %238)
  %240 = load float, ptr @$float86, !tbaa !2
  %241 = fcmp ogt float %239, %240
  br i1 %241, label %b38, label %b39

b36:
  br label %b37

b37:
  %242 = load i16, ptr %129, !tbaa !2
  %243 = icmp sgt i16 %242, -158
  %244 = sext i1 %243 to i16
  %245 = load i16, ptr %129, !tbaa !2
  %246 = icmp slt i16 %245, 476
  %247 = sext i1 %246 to i16
  %248 = and i16 %244, %247
  %249 = icmp ne i16 %248, 0
  br i1 %249, label %b42, label %b43

b38:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b39:
  %250 = call float @llvm.sin.f32(float %238)
  store i16 63, ptr %114, !tbaa !2
  %251 = load i16, ptr %114, !tbaa !2
  %252 = sitofp i16 %251 to float
  %253 = fmul float %250, %252
  %254 = call i16 @llvm.lrint.i16.f32(float %253)
  %255 = sub i16 %232, -48
  %256 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %255
  store i16 %254, ptr %256, !tbaa !2
  %257 = load i16, ptr %129, !tbaa !2
  %258 = load i16, ptr %129, !tbaa !2
  store i16 %258, ptr %113, !tbaa !2
  %259 = load i16, ptr %113, !tbaa !2
  %260 = sitofp i16 %259 to float
  store i16 35, ptr %112, !tbaa !2
  %261 = load i16, ptr %112, !tbaa !2
  %262 = sitofp i16 %261 to float
  %263 = fdiv float %260, %262
  %264 = call float @llvm.fabs.f32(float %263)
  %265 = load float, ptr @$float86, !tbaa !2
  %266 = fcmp ogt float %264, %265
  br i1 %266, label %b40, label %b41

b40:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b41:
  %267 = call float @llvm.sin.f32(float %263)
  store i16 127, ptr %111, !tbaa !2
  %268 = load i16, ptr %111, !tbaa !2
  %269 = sitofp i16 %268 to float
  %270 = fmul float %267, %269
  %271 = call i16 @llvm.lrint.i16.f32(float %270)
  %272 = sub i16 %257, -640
  %273 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %272
  store i16 %271, ptr %273, !tbaa !2
  br label %b37

b42:
  %274 = load i16, ptr %129, !tbaa !2
  %275 = load i16, ptr %129, !tbaa !2
  store i16 %275, ptr %110, !tbaa !2
  %276 = load i16, ptr %110, !tbaa !2
  %277 = sitofp i16 %276 to float
  store i16 25, ptr %109, !tbaa !2
  %278 = load i16, ptr %109, !tbaa !2
  %279 = sitofp i16 %278 to float
  %280 = fdiv float %277, %279
  %281 = call float @llvm.fabs.f32(float %280)
  %282 = load float, ptr @$float86, !tbaa !2
  %283 = fcmp ogt float %281, %282
  br i1 %283, label %b45, label %b46

b43:
  br label %b44

b44:
  %284 = load i16, ptr %129, !tbaa !2
  %285 = load i16, ptr %117, !tbaa !2
  %286 = add nsw i16 %284, %285
  store i16 %286, ptr %129, !tbaa !2
  br label %b30

b45:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b46:
  %287 = call float @llvm.sin.f32(float %280)
  store i16 31, ptr %108, !tbaa !2
  %288 = load i16, ptr %108, !tbaa !2
  %289 = sitofp i16 %288 to float
  %290 = fmul float %287, %289
  %291 = call i16 @llvm.lrint.i16.f32(float %290)
  %292 = sub i16 %274, -640
  %293 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %292
  store i16 %291, ptr %293, !tbaa !2
  br label %b44

b47:
  %294 = load i16, ptr %105, !tbaa !2
  %295 = icmp sge i16 %294, 0
  br i1 %295, label %b48, label %b49

b48:
  %296 = load i16, ptr %129, !tbaa !2
  %297 = load i16, ptr %106, !tbaa !2
  %298 = icmp sle i16 %296, %297
  br i1 %298, label %b50, label %b51

b49:
  %299 = load i16, ptr %129, !tbaa !2
  %300 = load i16, ptr %106, !tbaa !2
  %301 = icmp sge i16 %299, %300
  br i1 %301, label %b50, label %b51

b50:
  %302 = load i16, ptr %129, !tbaa !2
  %303 = load i16, ptr %129, !tbaa !2
  %304 = load i16, ptr %129, !tbaa !2
  %305 = sub i16 %304, 63
  %306 = sext i16 %305 to i32
  %307 = sdiv i32 %306, 64
  %308 = trunc i32 %307 to i16
  %309 = mul i16 %308, 64
  %310 = sub i16 %303, %309
  %311 = sub i16 63, %310
  %312 = sub i16 %302, -180
  %313 = getelementptr inbounds i16, ptr @"DT100%", i16 %312
  store i16 %311, ptr %313, !tbaa !2
  %314 = load i16, ptr %107, !tbaa !2
  %315 = icmp eq i16 %314, 1
  br i1 %315, label %b52, label %b53

b51:
  store i16 1, ptr %107, !tbaa !2
  store i16 0, ptr %129, !tbaa !2
  store i16 156, ptr %104, !tbaa !2
  store i16 1, ptr %103, !tbaa !2
  br label %b61

b52:
  %316 = load i16, ptr %129, !tbaa !2
  %317 = load i16, ptr %129, !tbaa !2
  %318 = sub i16 %317, -180
  %319 = getelementptr inbounds i16, ptr @"DT100%", i16 %318
  %320 = load i16, ptr %319, !tbaa !2
  %321 = sub i16 63, %320
  %322 = sub i16 %316, -180
  %323 = getelementptr inbounds i16, ptr @"DT100%", i16 %322
  store i16 %321, ptr %323, !tbaa !2
  br label %b54

b53:
  br label %b54

b54:
  %324 = load i16, ptr %129, !tbaa !2
  %325 = sub i16 %324, -180
  %326 = getelementptr inbounds i16, ptr @"DT100%", i16 %325
  %327 = load i16, ptr %326, !tbaa !2
  %328 = icmp eq i16 %327, 0
  %329 = sext i1 %328 to i16
  %330 = load i16, ptr %107, !tbaa !2
  %331 = icmp eq i16 %330, 1
  %332 = sext i1 %331 to i16
  %333 = and i16 %329, %332
  %334 = icmp ne i16 %333, 0
  br i1 %334, label %b55, label %b56

b55:
  store i16 -1, ptr %107, !tbaa !2
  br label %b57

b56:
  br label %b57

b57:
  %335 = load i16, ptr %129, !tbaa !2
  %336 = sub i16 %335, -180
  %337 = getelementptr inbounds i16, ptr @"DT100%", i16 %336
  %338 = load i16, ptr %337, !tbaa !2
  %339 = icmp eq i16 %338, 63
  %340 = sext i1 %339 to i16
  %341 = load i16, ptr %107, !tbaa !2
  %342 = icmp eq i16 %341, -1
  %343 = sext i1 %342 to i16
  %344 = and i16 %340, %343
  %345 = icmp ne i16 %344, 0
  br i1 %345, label %b58, label %b59

b58:
  store i16 1, ptr %107, !tbaa !2
  br label %b60

b59:
  br label %b60

b60:
  %346 = load i16, ptr %129, !tbaa !2
  %347 = load i16, ptr %105, !tbaa !2
  %348 = add nsw i16 %346, %347
  store i16 %348, ptr %129, !tbaa !2
  br label %b47

b61:
  %349 = load i16, ptr %103, !tbaa !2
  %350 = icmp sge i16 %349, 0
  br i1 %350, label %b62, label %b63

b62:
  %351 = load i16, ptr %129, !tbaa !2
  %352 = load i16, ptr %104, !tbaa !2
  %353 = icmp sle i16 %351, %352
  br i1 %353, label %b64, label %b65

b63:
  %354 = load i16, ptr %129, !tbaa !2
  %355 = load i16, ptr %104, !tbaa !2
  %356 = icmp sge i16 %354, %355
  br i1 %356, label %b64, label %b65

b64:
  %357 = load i16, ptr %129, !tbaa !2
  %358 = load i16, ptr %129, !tbaa !2
  %359 = load i16, ptr %129, !tbaa !2
  %360 = sext i16 %359 to i32
  %361 = sdiv i32 %360, 64
  %362 = trunc i32 %361 to i16
  %363 = mul i16 %362, 64
  %364 = sub i16 %358, %363
  %365 = sub i16 %357, -180
  %366 = getelementptr inbounds i16, ptr @"DT100%", i16 %365
  store i16 %364, ptr %366, !tbaa !2
  %367 = load i16, ptr %107, !tbaa !2
  %368 = icmp eq i16 %367, -1
  br i1 %368, label %b66, label %b67

b65:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4095, i16 2, i16 257, ptr %132)
  store i16 0, ptr %102, !tbaa !2
  store i16 0, ptr %129, !tbaa !2
  store i16 63, ptr %101, !tbaa !2
  store i16 1, ptr %100, !tbaa !2
  br label %b75

b66:
  %369 = load i16, ptr %129, !tbaa !2
  %370 = load i16, ptr %129, !tbaa !2
  %371 = sub i16 %370, -180
  %372 = getelementptr inbounds i16, ptr @"DT100%", i16 %371
  %373 = load i16, ptr %372, !tbaa !2
  %374 = sub i16 63, %373
  %375 = sub i16 %369, -180
  %376 = getelementptr inbounds i16, ptr @"DT100%", i16 %375
  store i16 %374, ptr %376, !tbaa !2
  br label %b68

b67:
  br label %b68

b68:
  %377 = load i16, ptr %129, !tbaa !2
  %378 = sub i16 %377, -180
  %379 = getelementptr inbounds i16, ptr @"DT100%", i16 %378
  %380 = load i16, ptr %379, !tbaa !2
  %381 = icmp eq i16 %380, 63
  %382 = sext i1 %381 to i16
  %383 = load i16, ptr %107, !tbaa !2
  %384 = icmp eq i16 %383, 1
  %385 = sext i1 %384 to i16
  %386 = and i16 %382, %385
  %387 = icmp ne i16 %386, 0
  br i1 %387, label %b69, label %b70

b69:
  store i16 -1, ptr %107, !tbaa !2
  br label %b71

b70:
  br label %b71

b71:
  %388 = load i16, ptr %129, !tbaa !2
  %389 = sub i16 %388, -180
  %390 = getelementptr inbounds i16, ptr @"DT100%", i16 %389
  %391 = load i16, ptr %390, !tbaa !2
  %392 = icmp eq i16 %391, 0
  %393 = sext i1 %392 to i16
  %394 = load i16, ptr %107, !tbaa !2
  %395 = icmp eq i16 %394, -1
  %396 = sext i1 %395 to i16
  %397 = and i16 %393, %396
  %398 = icmp ne i16 %397, 0
  br i1 %398, label %b72, label %b73

b72:
  store i16 1, ptr %107, !tbaa !2
  br label %b74

b73:
  br label %b74

b74:
  %399 = load i16, ptr %129, !tbaa !2
  %400 = load i16, ptr %103, !tbaa !2
  %401 = add nsw i16 %399, %400
  store i16 %401, ptr %129, !tbaa !2
  br label %b61

b75:
  %402 = load i16, ptr %100, !tbaa !2
  %403 = icmp sge i16 %402, 0
  br i1 %403, label %b76, label %b77

b76:
  %404 = load i16, ptr %129, !tbaa !2
  %405 = load i16, ptr %101, !tbaa !2
  %406 = icmp sle i16 %404, %405
  br i1 %406, label %b78, label %b79

b77:
  %407 = load i16, ptr %129, !tbaa !2
  %408 = load i16, ptr %101, !tbaa !2
  %409 = icmp sge i16 %407, %408
  br i1 %409, label %b78, label %b79

b78:
  store i16 0, ptr %99, !tbaa !2
  store i16 63, ptr %98, !tbaa !2
  store i16 1, ptr %97, !tbaa !2
  br label %b80

b79:
  store i16 0, ptr %129, !tbaa !2
  store i16 0, ptr %93, !tbaa !2
  store i16 99, ptr %92, !tbaa !2
  store i16 1, ptr %91, !tbaa !2
  br label %b85

b80:
  %410 = load i16, ptr %97, !tbaa !2
  %411 = icmp sge i16 %410, 0
  br i1 %411, label %b81, label %b82

b81:
  %412 = load i16, ptr %99, !tbaa !2
  %413 = load i16, ptr %98, !tbaa !2
  %414 = icmp sle i16 %412, %413
  br i1 %414, label %b83, label %b84

b82:
  %415 = load i16, ptr %99, !tbaa !2
  %416 = load i16, ptr %98, !tbaa !2
  %417 = icmp sge i16 %415, %416
  br i1 %417, label %b83, label %b84

b83:
  %418 = load i16, ptr %102, !tbaa !2
  %419 = mul i16 %418, 2
  %420 = getelementptr i8, ptr %132, i16 2
  %421 = load i16, ptr %420, !tbaa !2
  %422 = add i16 0, %419
  %423 = inttoptr i16 %421 to ptr addrspace(2)
  %424 = addrspacecast ptr addrspace(2) %423 to ptr addrspace(1)
  %425 = getelementptr i8, ptr addrspace(1) %424, i16 %422
  %426 = load i16, ptr %129, !tbaa !2
  %427 = load i16, ptr %99, !tbaa !2
  store i16 %427, ptr %96, !tbaa !2
  %428 = load i16, ptr %96, !tbaa !2
  %429 = sitofp i16 %428 to float
  store i16 63, ptr %95, !tbaa !2
  %430 = load i16, ptr %95, !tbaa !2
  %431 = sitofp i16 %430 to float
  %432 = fdiv float %429, %431
  store i16 %426, ptr %94, !tbaa !2
  %433 = load i16, ptr %94, !tbaa !2
  %434 = sitofp i16 %433 to float
  %435 = fmul float %434, %432
  %436 = call i16 @llvm.lrint.i16.f32(float %435)
  store i16 %436, ptr addrspace(1) %425, !tbaa !70
  %437 = load i16, ptr %102, !tbaa !2
  %438 = add i16 %437, 1
  store i16 %438, ptr %102, !tbaa !2
  %439 = load i16, ptr %99, !tbaa !2
  %440 = load i16, ptr %97, !tbaa !2
  %441 = add nsw i16 %439, %440
  store i16 %441, ptr %99, !tbaa !2
  br label %b80

b84:
  %442 = load i16, ptr %129, !tbaa !2
  %443 = load i16, ptr %100, !tbaa !2
  %444 = add nsw i16 %442, %443
  store i16 %444, ptr %129, !tbaa !2
  br label %b75

b85:
  %445 = load i16, ptr %91, !tbaa !2
  %446 = icmp sge i16 %445, 0
  br i1 %446, label %b86, label %b87

b86:
  %447 = load i16, ptr %93, !tbaa !2
  %448 = load i16, ptr %92, !tbaa !2
  %449 = icmp sle i16 %447, %448
  br i1 %449, label %b88, label %b89

b87:
  %450 = load i16, ptr %93, !tbaa !2
  %451 = load i16, ptr %92, !tbaa !2
  %452 = icmp sge i16 %450, %451
  br i1 %452, label %b88, label %b89

b88:
  store i16 0, ptr %90, !tbaa !2
  store i16 159, ptr %89, !tbaa !2
  store i16 1, ptr %88, !tbaa !2
  br label %b90

b89:
  store i16 -160, ptr %129, !tbaa !2
  store i16 479, ptr %78, !tbaa !2
  store i16 1, ptr %77, !tbaa !2
  br label %b118

b90:
  %453 = load i16, ptr %88, !tbaa !2
  %454 = icmp sge i16 %453, 0
  br i1 %454, label %b91, label %b92

b91:
  %455 = load i16, ptr %90, !tbaa !2
  %456 = load i16, ptr %89, !tbaa !2
  %457 = icmp sle i16 %455, %456
  br i1 %457, label %b93, label %b94

b92:
  %458 = load i16, ptr %90, !tbaa !2
  %459 = load i16, ptr %89, !tbaa !2
  %460 = icmp sge i16 %458, %459
  br i1 %460, label %b93, label %b94

b93:
  %461 = load i16, ptr %90, !tbaa !2
  %462 = icmp eq i16 %461, 159
  %463 = sext i1 %462 to i16
  %464 = load i16, ptr %93, !tbaa !2
  %465 = icmp eq i16 %464, 99
  %466 = sext i1 %465 to i16
  %467 = and i16 %463, %466
  %468 = icmp ne i16 %467, 0
  br i1 %468, label %b95, label %b96

b94:
  %469 = load i16, ptr %93, !tbaa !2
  %470 = load i16, ptr %91, !tbaa !2
  %471 = add nsw i16 %469, %470
  store i16 %471, ptr %93, !tbaa !2
  br label %b85

b95:
  %472 = load i16, ptr %129, !tbaa !2
  %473 = sub i16 %472, 0
  %474 = getelementptr inbounds i16, ptr @"SP%", i16 %473
  store i16 255, ptr %474, !tbaa !2
  br label %b97

b96:
  %475 = load i16, ptr %129, !tbaa !2
  %476 = load i16, ptr %90, !tbaa !2
  %477 = sub i16 159, %476
  store i16 %477, ptr %87, !tbaa !2
  %478 = load i16, ptr %87, !tbaa !2
  %479 = sitofp i16 %478 to float
  %480 = fpround float %479 to float
  store float %480, ptr %86, !tbaa !2
  %481 = load float, ptr %86, !tbaa !2
  %482 = load float, ptr %86, !tbaa !2
  %483 = fmul float %481, %482
  %484 = load i16, ptr %93, !tbaa !2
  %485 = sub i16 99, %484
  store i16 %485, ptr %85, !tbaa !2
  %486 = load i16, ptr %85, !tbaa !2
  %487 = sitofp i16 %486 to float
  %488 = fpround float %487 to float
  store float %488, ptr %84, !tbaa !2
  %489 = load float, ptr %84, !tbaa !2
  %490 = load float, ptr %84, !tbaa !2
  %491 = fmul float %489, %490
  %492 = fadd float %483, %491
  %493 = call float @llvm.sqrt.f32(float %492)
  %494 = load float, ptr %131, !tbaa !2
  %495 = fpround float %493 to float
  store float %495, ptr %83, !tbaa !2
  store float %494, ptr %82, !tbaa !2
  %496 = load float, ptr %83, !tbaa !2
  %497 = load float, ptr @$float218, !tbaa !2
  %498 = fcmp ogt float %496, %497
  br i1 %498, label %b98, label %b99

b97:
  %499 = load i16, ptr %129, !tbaa !2
  %500 = sub i16 %499, 0
  %501 = getelementptr inbounds i16, ptr @"SP%", i16 %500
  %502 = load i16, ptr %501, !tbaa !2
  %503 = icmp sgt i16 %502, 255
  br i1 %503, label %b115, label %b116

b98:
  %504 = load float, ptr %83, !tbaa !2
  %505 = call float @llvm.log2.f32(float %504)
  %506 = load float, ptr %82, !tbaa !2
  %507 = fmul float %505, %506
  %508 = load float, ptr @$float219, !tbaa !2
  %509 = fcmp oge float %507, %508
  br i1 %509, label %b107, label %b108

b99:
  %510 = load float, ptr %83, !tbaa !2
  %511 = load float, ptr @$float218, !tbaa !2
  %512 = fcmp oeq float %510, %511
  br i1 %512, label %b100, label %b102

b100:
  %513 = load float, ptr %82, !tbaa !2
  %514 = load float, ptr @$float218, !tbaa !2
  %515 = fcmp ogt float %513, %514
  br i1 %515, label %b109, label %b101

b101:
  %516 = load float, ptr %82, !tbaa !2
  %517 = load float, ptr @$float218, !tbaa !2
  %518 = fcmp oeq float %516, %517
  br i1 %518, label %b110, label %b105

b102:
  %519 = load float, ptr %82, !tbaa !2
  %520 = call i32 @llvm.lrint.i32.f32(float %519)
  store i32 %520, ptr %80, !tbaa !2
  %521 = load i32, ptr %80, !tbaa !2
  store i32 %521, ptr %79, !tbaa !2
  %522 = load i32, ptr %79, !tbaa !2
  %523 = sitofp i32 %522 to float
  %524 = load float, ptr %82, !tbaa !2
  %525 = fcmp oeq float %523, %524
  br i1 %525, label %b103, label %b105

b103:
  %526 = load float, ptr %83, !tbaa !2
  %527 = fneg float %526
  %528 = call float @llvm.log2.f32(float %527)
  %529 = load float, ptr %82, !tbaa !2
  %530 = fmul float %528, %529
  %531 = load float, ptr @$float219, !tbaa !2
  %532 = fcmp oge float %530, %531
  br i1 %532, label %b111, label %b112

b104:
  %533 = load float, ptr %81, !tbaa !2
  %534 = fneg float %533
  %535 = fpround float %534 to float
  store float %535, ptr %81, !tbaa !2
  br label %b106

b105:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b106:
  %536 = load float, ptr %81, !tbaa !2
  %537 = call i32 @llvm.lrint.i32.f32(float %536)
  %538 = icmp eq i32 %537, 0
  br i1 %538, label %b113, label %b114

b107:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b108:
  %539 = call float @llvm.exp2.f32(float %507)
  %540 = fpround float %539 to float
  store float %540, ptr %81, !tbaa !2
  br label %b106

b109:
  %541 = load float, ptr @$float218, !tbaa !2
  store float %541, ptr %81, !tbaa !2
  br label %b106

b110:
  %542 = load float, ptr @$float220, !tbaa !2
  store float %542, ptr %81, !tbaa !2
  br label %b106

b111:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b112:
  %543 = call float @llvm.exp2.f32(float %530)
  %544 = fpround float %543 to float
  store float %544, ptr %81, !tbaa !2
  %545 = load i32, ptr %80, !tbaa !2
  %546 = and i32 %545, 1
  %547 = icmp ne i32 %546, 0
  br i1 %547, label %b104, label %b106

b113:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b114:
  %548 = sdiv i32 16384, %537
  %549 = trunc i32 %548 to i16
  %550 = sub i16 %475, 0
  %551 = getelementptr inbounds i16, ptr @"SP%", i16 %550
  store i16 %549, ptr %551, !tbaa !2
  br label %b97

b115:
  %552 = load i16, ptr %129, !tbaa !2
  %553 = sub i16 %552, 0
  %554 = getelementptr inbounds i16, ptr @"SP%", i16 %553
  store i16 255, ptr %554, !tbaa !2
  br label %b117

b116:
  br label %b117

b117:
  %555 = load i16, ptr %129, !tbaa !2
  %556 = add i16 %555, 1
  store i16 %556, ptr %129, !tbaa !2
  %557 = load i16, ptr %90, !tbaa !2
  %558 = load i16, ptr %88, !tbaa !2
  %559 = add nsw i16 %557, %558
  store i16 %559, ptr %90, !tbaa !2
  br label %b90

b118:
  %560 = load i16, ptr %77, !tbaa !2
  %561 = icmp sge i16 %560, 0
  br i1 %561, label %b119, label %b120

b119:
  %562 = load i16, ptr %129, !tbaa !2
  %563 = load i16, ptr %78, !tbaa !2
  %564 = icmp sle i16 %562, %563
  br i1 %564, label %b121, label %b122

b120:
  %565 = load i16, ptr %129, !tbaa !2
  %566 = load i16, ptr %78, !tbaa !2
  %567 = icmp sge i16 %565, %566
  br i1 %567, label %b121, label %b122

b121:
  %568 = load i16, ptr %129, !tbaa !2
  %569 = icmp slt i16 %568, 0
  %570 = sext i1 %569 to i16
  %571 = load i16, ptr %129, !tbaa !2
  %572 = icmp sgt i16 %571, 319
  %573 = sext i1 %572 to i16
  %574 = or i16 %570, %573
  %575 = icmp ne i16 %574, 0
  br i1 %575, label %b123, label %b124

b122:
  store i16 -100, ptr %129, !tbaa !2
  store i16 300, ptr %76, !tbaa !2
  store i16 1, ptr %75, !tbaa !2
  br label %b129

b123:
  %576 = load i16, ptr %129, !tbaa !2
  %577 = sub i16 %576, -319
  %578 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %577
  store i16 0, ptr %578, !tbaa !2
  br label %b125

b124:
  %579 = load i16, ptr %129, !tbaa !2
  %580 = icmp slt i16 %579, 160
  br i1 %580, label %b126, label %b127

b125:
  %581 = load i16, ptr %129, !tbaa !2
  %582 = load i16, ptr %77, !tbaa !2
  %583 = add nsw i16 %581, %582
  store i16 %583, ptr %129, !tbaa !2
  br label %b118

b126:
  %584 = load i16, ptr %129, !tbaa !2
  %585 = load i16, ptr %129, !tbaa !2
  %586 = sub i16 %584, -319
  %587 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %586
  store i16 %585, ptr %587, !tbaa !2
  br label %b128

b127:
  %588 = load i16, ptr %129, !tbaa !2
  %589 = load i16, ptr %129, !tbaa !2
  %590 = sub i16 319, %589
  %591 = sub i16 %588, -319
  %592 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %591
  store i16 %590, ptr %592, !tbaa !2
  br label %b128

b128:
  br label %b125

b129:
  %593 = load i16, ptr %75, !tbaa !2
  %594 = icmp sge i16 %593, 0
  br i1 %594, label %b130, label %b131

b130:
  %595 = load i16, ptr %129, !tbaa !2
  %596 = load i16, ptr %76, !tbaa !2
  %597 = icmp sle i16 %595, %596
  br i1 %597, label %b132, label %b133

b131:
  %598 = load i16, ptr %129, !tbaa !2
  %599 = load i16, ptr %76, !tbaa !2
  %600 = icmp sge i16 %598, %599
  br i1 %600, label %b132, label %b133

b132:
  %601 = load i16, ptr %129, !tbaa !2
  %602 = icmp slt i16 %601, 0
  %603 = sext i1 %602 to i16
  %604 = load i16, ptr %129, !tbaa !2
  %605 = icmp sgt i16 %604, 199
  %606 = sext i1 %605 to i16
  %607 = or i16 %603, %606
  %608 = icmp ne i16 %607, 0
  br i1 %608, label %b134, label %b135

b133:
  store i16 0, ptr %129, !tbaa !2
  store i16 767, ptr %74, !tbaa !2
  store i16 1, ptr %73, !tbaa !2
  br label %b140

b134:
  %609 = load i16, ptr %129, !tbaa !2
  %610 = sub i16 %609, -100
  %611 = getelementptr inbounds i16, ptr @"CY%", i16 %610
  store i16 0, ptr %611, !tbaa !2
  br label %b136

b135:
  %612 = load i16, ptr %129, !tbaa !2
  %613 = icmp sgt i16 %612, 99
  br i1 %613, label %b137, label %b138

b136:
  %614 = load i16, ptr %129, !tbaa !2
  %615 = load i16, ptr %129, !tbaa !2
  %616 = sub i16 %615, -100
  %617 = getelementptr inbounds i16, ptr @"CY%", i16 %616
  %618 = load i16, ptr %617, !tbaa !2
  %619 = mul i16 %618, 160
  %620 = sub i16 %614, -100
  %621 = getelementptr inbounds i16, ptr @"CY%", i16 %620
  store i16 %619, ptr %621, !tbaa !2
  %622 = load i16, ptr %129, !tbaa !2
  %623 = load i16, ptr %75, !tbaa !2
  %624 = add nsw i16 %622, %623
  store i16 %624, ptr %129, !tbaa !2
  br label %b129

b137:
  %625 = load i16, ptr %129, !tbaa !2
  %626 = load i16, ptr %129, !tbaa !2
  %627 = sub i16 199, %626
  %628 = sub i16 %625, -100
  %629 = getelementptr inbounds i16, ptr @"CY%", i16 %628
  store i16 %627, ptr %629, !tbaa !2
  br label %b139

b138:
  %630 = load i16, ptr %129, !tbaa !2
  %631 = load i16, ptr %129, !tbaa !2
  %632 = sub i16 %630, -100
  %633 = getelementptr inbounds i16, ptr @"CY%", i16 %632
  store i16 %631, ptr %633, !tbaa !2
  br label %b139

b139:
  br label %b136

b140:
  %634 = load i16, ptr %73, !tbaa !2
  %635 = icmp sge i16 %634, 0
  br i1 %635, label %b141, label %b142

b141:
  %636 = load i16, ptr %129, !tbaa !2
  %637 = load i16, ptr %74, !tbaa !2
  %638 = icmp sle i16 %636, %637
  br i1 %638, label %b143, label %b144

b142:
  %639 = load i16, ptr %129, !tbaa !2
  %640 = load i16, ptr %74, !tbaa !2
  %641 = icmp sge i16 %639, %640
  br i1 %641, label %b143, label %b144

b143:
  %642 = load i16, ptr %129, !tbaa !2
  %643 = icmp sgt i16 %642, 255
  br i1 %643, label %b145, label %b146

b144:
  store i16 -192, ptr %129, !tbaa !2
  store i16 63, ptr %72, !tbaa !2
  store i16 1, ptr %71, !tbaa !2
  br label %b148

b145:
  %644 = load i16, ptr %129, !tbaa !2
  %645 = sub i16 %644, 0
  %646 = getelementptr inbounds i16, ptr @"CD%", i16 %645
  store i16 255, ptr %646, !tbaa !2
  br label %b147

b146:
  %647 = load i16, ptr %129, !tbaa !2
  %648 = load i16, ptr %129, !tbaa !2
  %649 = sub i16 %647, 0
  %650 = getelementptr inbounds i16, ptr @"CD%", i16 %649
  store i16 %648, ptr %650, !tbaa !2
  br label %b147

b147:
  %651 = load i16, ptr %129, !tbaa !2
  %652 = load i16, ptr %73, !tbaa !2
  %653 = add nsw i16 %651, %652
  store i16 %653, ptr %129, !tbaa !2
  br label %b140

b148:
  %654 = load i16, ptr %71, !tbaa !2
  %655 = icmp sge i16 %654, 0
  br i1 %655, label %b149, label %b150

b149:
  %656 = load i16, ptr %129, !tbaa !2
  %657 = load i16, ptr %72, !tbaa !2
  %658 = icmp sle i16 %656, %657
  br i1 %658, label %b151, label %b152

b150:
  %659 = load i16, ptr %129, !tbaa !2
  %660 = load i16, ptr %72, !tbaa !2
  %661 = icmp sge i16 %659, %660
  br i1 %661, label %b151, label %b152

b151:
  %662 = load i16, ptr %129, !tbaa !2
  %663 = load i16, ptr %129, !tbaa !2
  %664 = mul i16 %663, 64
  %665 = sub i16 %662, -192
  %666 = getelementptr inbounds i16, ptr @"DT%", i16 %665
  store i16 %664, ptr %666, !tbaa !2
  %667 = load i16, ptr %129, !tbaa !2
  %668 = load i16, ptr %71, !tbaa !2
  %669 = add nsw i16 %667, %668
  store i16 %669, ptr %129, !tbaa !2
  br label %b148

b152:
  store i16 1, ptr %129, !tbaa !2
  %670 = load i16, ptr %130, !tbaa !2
  store i16 %670, ptr %70, !tbaa !2
  store i16 1, ptr %69, !tbaa !2
  br label %b153

b153:
  %671 = load i16, ptr %69, !tbaa !2
  %672 = icmp sge i16 %671, 0
  br i1 %672, label %b154, label %b155

b154:
  %673 = load i16, ptr %129, !tbaa !2
  %674 = load i16, ptr %70, !tbaa !2
  %675 = icmp sle i16 %673, %674
  br i1 %675, label %b156, label %b157

b155:
  %676 = load i16, ptr %129, !tbaa !2
  %677 = load i16, ptr %70, !tbaa !2
  %678 = icmp sge i16 %676, %677
  br i1 %678, label %b156, label %b157

b156:
  %679 = load i16, ptr %129, !tbaa !2
  %680 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %681 = load float, ptr %680
  store i16 45, ptr %68, !tbaa !2
  %682 = load i16, ptr %68, !tbaa !2
  %683 = sitofp i16 %682 to float
  %684 = fmul float %681, %683
  store i16 45, ptr %67, !tbaa !2
  %685 = load i16, ptr %67, !tbaa !2
  %686 = sitofp i16 %685 to float
  %687 = fadd float %684, %686
  %688 = call float @llvm.rint.f32(float %687)
  %689 = fcmp olt float %687, %688
  %690 = sext i1 %689 to i16
  store i16 %690, ptr %66, !tbaa !2
  %691 = load i16, ptr %66, !tbaa !2
  %692 = sitofp i16 %691 to float
  %693 = fadd float %688, %692
  %694 = call i16 @llvm.lrint.i16.f32(float %693)
  %695 = sub i16 %679, 0
  %696 = getelementptr inbounds i16, ptr @"DSX%", i16 %695
  store i16 %694, ptr %696, !tbaa !2
  %697 = load i16, ptr %129, !tbaa !2
  %698 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %699 = load float, ptr %698
  store i16 30, ptr %65, !tbaa !2
  %700 = load i16, ptr %65, !tbaa !2
  %701 = sitofp i16 %700 to float
  %702 = fmul float %699, %701
  store i16 30, ptr %64, !tbaa !2
  %703 = load i16, ptr %64, !tbaa !2
  %704 = sitofp i16 %703 to float
  %705 = fadd float %702, %704
  %706 = call float @llvm.rint.f32(float %705)
  %707 = fcmp olt float %705, %706
  %708 = sext i1 %707 to i16
  store i16 %708, ptr %63, !tbaa !2
  %709 = load i16, ptr %63, !tbaa !2
  %710 = sitofp i16 %709 to float
  %711 = fadd float %706, %710
  %712 = call i16 @llvm.lrint.i16.f32(float %711)
  %713 = sub i16 %697, 0
  %714 = getelementptr inbounds i16, ptr @"DSY%", i16 %713
  store i16 %712, ptr %714, !tbaa !2
  %715 = load i16, ptr %129, !tbaa !2
  %716 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %717 = load float, ptr %716
  store i16 80, ptr %62, !tbaa !2
  %718 = load i16, ptr %62, !tbaa !2
  %719 = sitofp i16 %718 to float
  %720 = fmul float %717, %719
  store i16 80, ptr %61, !tbaa !2
  %721 = load i16, ptr %61, !tbaa !2
  %722 = sitofp i16 %721 to float
  %723 = fadd float %720, %722
  %724 = call float @llvm.rint.f32(float %723)
  %725 = fcmp olt float %723, %724
  %726 = sext i1 %725 to i16
  store i16 %726, ptr %60, !tbaa !2
  %727 = load i16, ptr %60, !tbaa !2
  %728 = sitofp i16 %727 to float
  %729 = fadd float %724, %728
  %730 = call i16 @llvm.lrint.i16.f32(float %729)
  %731 = sub i16 %715, 0
  %732 = getelementptr inbounds i16, ptr @"PSX%", i16 %731
  store i16 %730, ptr %732, !tbaa !2
  %733 = load i16, ptr %129, !tbaa !2
  %734 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %735 = load float, ptr %734
  store i16 50, ptr %59, !tbaa !2
  %736 = load i16, ptr %59, !tbaa !2
  %737 = sitofp i16 %736 to float
  %738 = fmul float %735, %737
  store i16 50, ptr %58, !tbaa !2
  %739 = load i16, ptr %58, !tbaa !2
  %740 = sitofp i16 %739 to float
  %741 = fadd float %738, %740
  %742 = call float @llvm.rint.f32(float %741)
  %743 = fcmp olt float %741, %742
  %744 = sext i1 %743 to i16
  store i16 %744, ptr %57, !tbaa !2
  %745 = load i16, ptr %57, !tbaa !2
  %746 = sitofp i16 %745 to float
  %747 = fadd float %742, %746
  %748 = call i16 @llvm.lrint.i16.f32(float %747)
  %749 = sub i16 %733, 0
  %750 = getelementptr inbounds i16, ptr @"PSY%", i16 %749
  store i16 %748, ptr %750, !tbaa !2
  %751 = load i16, ptr %129, !tbaa !2
  %752 = load i16, ptr %69, !tbaa !2
  %753 = add nsw i16 %751, %752
  store i16 %753, ptr %129, !tbaa !2
  br label %b153

b157:
  store i16 0, ptr %99, !tbaa !2
  store i16 0, ptr %102, !tbaa !2
  store i16 0, ptr %56, !tbaa !2
  store i16 26, ptr %55, !tbaa !2
  store i16 0, ptr %54, !tbaa !2
  store i16 26, ptr %53, !tbaa !2
  store i16 128, ptr %52, !tbaa !2
  store i16 191, ptr %51, !tbaa !2
  store i16 128, ptr %50, !tbaa !2
  store i16 191, ptr %49, !tbaa !2
  store i16 0, ptr %48, !tbaa !2
  store i16 0, ptr %47, !tbaa !2
  store i16 0, ptr %46, !tbaa !2
  store i16 0, ptr %45, !tbaa !2
  br label %b159

b159:
  %754 = load i16, ptr %45, !tbaa !2
  %755 = add i16 %754, 1
  store i16 %755, ptr %45, !tbaa !2
  %756 = load i16, ptr %99, !tbaa !2
  %757 = add i16 %756, 2
  store i16 %757, ptr %99, !tbaa !2
  %758 = load i16, ptr %99, !tbaa !2
  %759 = icmp sgt i16 %758, 156
  br i1 %759, label %b161, label %b162

b161:
  store i16 0, ptr %99, !tbaa !2
  br label %b163

b162:
  br label %b163

b163:
  %760 = load i16, ptr %102, !tbaa !2
  %761 = add i16 %760, 1
  store i16 %761, ptr %102, !tbaa !2
  store i16 1, ptr %129, !tbaa !2
  %762 = load i16, ptr %130, !tbaa !2
  store i16 %762, ptr %44, !tbaa !2
  store i16 1, ptr %43, !tbaa !2
  br label %b164

b164:
  %763 = load i16, ptr %43, !tbaa !2
  %764 = icmp sge i16 %763, 0
  br i1 %764, label %b165, label %b166

b165:
  %765 = load i16, ptr %129, !tbaa !2
  %766 = load i16, ptr %44, !tbaa !2
  %767 = icmp sle i16 %765, %766
  br i1 %767, label %b167, label %b168

b166:
  %768 = load i16, ptr %129, !tbaa !2
  %769 = load i16, ptr %44, !tbaa !2
  %770 = icmp sge i16 %768, %769
  br i1 %770, label %b167, label %b168

b167:
  %771 = load i16, ptr %129, !tbaa !2
  %772 = load i16, ptr %102, !tbaa !2
  %773 = load i16, ptr %129, !tbaa !2
  %774 = sub i16 %773, 0
  %775 = getelementptr inbounds i16, ptr @"DSX%", i16 %774
  %776 = load i16, ptr %775, !tbaa !2
  store i16 %772, ptr %42, !tbaa !2
  %777 = load i16, ptr %42, !tbaa !2
  %778 = sitofp i16 %777 to float
  store i16 %776, ptr %41, !tbaa !2
  %779 = load i16, ptr %41, !tbaa !2
  %780 = sitofp i16 %779 to float
  %781 = fdiv float %778, %780
  %782 = call float @llvm.fabs.f32(float %781)
  %783 = load float, ptr @$float86, !tbaa !2
  %784 = fcmp ogt float %782, %783
  br i1 %784, label %b169, label %b170

b168:
  %785 = load i16, ptr %45, !tbaa !2
  %786 = icmp eq i16 %785, 51
  br i1 %786, label %b173, label %b174

b169:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b170:
  %787 = call float @llvm.sin.f32(float %781)
  %788 = load i16, ptr %129, !tbaa !2
  %789 = sub i16 %788, 0
  %790 = getelementptr inbounds i16, ptr @"PSX%", i16 %789
  %791 = load i16, ptr %790, !tbaa !2
  store i16 %791, ptr %40, !tbaa !2
  %792 = load i16, ptr %40, !tbaa !2
  %793 = sitofp i16 %792 to float
  %794 = fmul float %787, %793
  %795 = call i16 @llvm.lrint.i16.f32(float %794)
  %796 = sub i16 %771, 0
  %797 = getelementptr inbounds i16, ptr @"XP1%", i16 %796
  store i16 %795, ptr %797, !tbaa !2
  %798 = load i16, ptr %129, !tbaa !2
  %799 = load i16, ptr %102, !tbaa !2
  %800 = load i16, ptr %129, !tbaa !2
  %801 = sub i16 %800, 0
  %802 = getelementptr inbounds i16, ptr @"DSY%", i16 %801
  %803 = load i16, ptr %802, !tbaa !2
  store i16 %799, ptr %39, !tbaa !2
  %804 = load i16, ptr %39, !tbaa !2
  %805 = sitofp i16 %804 to float
  store i16 %803, ptr %38, !tbaa !2
  %806 = load i16, ptr %38, !tbaa !2
  %807 = sitofp i16 %806 to float
  %808 = fdiv float %805, %807
  %809 = call float @llvm.fabs.f32(float %808)
  %810 = load float, ptr @$float86, !tbaa !2
  %811 = fcmp ogt float %809, %810
  br i1 %811, label %b171, label %b172

b171:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b172:
  %812 = call float @llvm.sin.f32(float %808)
  %813 = load i16, ptr %129, !tbaa !2
  %814 = sub i16 %813, 0
  %815 = getelementptr inbounds i16, ptr @"PSY%", i16 %814
  %816 = load i16, ptr %815, !tbaa !2
  store i16 %816, ptr %37, !tbaa !2
  %817 = load i16, ptr %37, !tbaa !2
  %818 = sitofp i16 %817 to float
  %819 = fmul float %812, %818
  %820 = call i16 @llvm.lrint.i16.f32(float %819)
  %821 = sub i16 %798, 0
  %822 = getelementptr inbounds i16, ptr @"YP1%", i16 %821
  store i16 %820, ptr %822, !tbaa !2
  %823 = load i16, ptr %129, !tbaa !2
  %824 = load i16, ptr %43, !tbaa !2
  %825 = add nsw i16 %823, %824
  store i16 %825, ptr %129, !tbaa !2
  br label %b164

b173:
  store i16 192, ptr %52, !tbaa !2
  store i16 255, ptr %51, !tbaa !2
  store i16 0, ptr %56, !tbaa !2
  br label %b175

b174:
  br label %b175

b175:
  %826 = load i16, ptr %45, !tbaa !2
  %827 = icmp eq i16 %826, 151
  br i1 %827, label %b176, label %b177

b176:
  store i16 1, ptr %36, !tbaa !2
  br label %b178

b177:
  br label %b178

b178:
  %828 = load i16, ptr %45, !tbaa !2
  %829 = icmp eq i16 %828, 250
  br i1 %829, label %b179, label %b180

b179:
  call cc 1000 addrspace(1) void @GETPAL()
  store i16 0, ptr %54, !tbaa !2
  store i16 0, ptr %50, !tbaa !2
  store i16 255, ptr %49, !tbaa !2
  store i16 63, ptr %48, !tbaa !2
  store i16 63, ptr %47, !tbaa !2
  store i16 63, ptr %46, !tbaa !2
  br label %b181

b180:
  br label %b181

b181:
  %830 = load i16, ptr %45, !tbaa !2
  %831 = icmp eq i16 %830, 280
  br i1 %831, label %b182, label %b183

b182:
  br label %b4

b183:
  br label %b184

b184:
  %832 = load i16, ptr %56, !tbaa !2
  %833 = load i16, ptr %55, !tbaa !2
  %834 = icmp slt i16 %832, %833
  br i1 %834, label %b186, label %b187

b186:
  %835 = load i16, ptr %56, !tbaa !2
  %836 = add i16 %835, 1
  store i16 %836, ptr %56, !tbaa !2
  br label %b188

b187:
  br label %b2

b188:
  %837 = load i16, ptr %52, !tbaa !2
  store i16 %837, ptr %35, !tbaa !2
  %838 = load i16, ptr %51, !tbaa !2
  store i16 %838, ptr %34, !tbaa !2
  store i16 1, ptr %33, !tbaa !2
  br label %b190

b190:
  %839 = load i16, ptr %33, !tbaa !2
  %840 = icmp sge i16 %839, 0
  br i1 %840, label %b191, label %b192

b191:
  %841 = load i16, ptr %35, !tbaa !2
  %842 = load i16, ptr %34, !tbaa !2
  %843 = icmp sle i16 %841, %842
  br i1 %843, label %b193, label %b194

b192:
  %844 = load i16, ptr %35, !tbaa !2
  %845 = load i16, ptr %34, !tbaa !2
  %846 = icmp sge i16 %844, %845
  br i1 %846, label %b193, label %b194

b193:
  %847 = load i16, ptr %35, !tbaa !2
  %848 = trunc i16 %847 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %848)
  %849 = load i16, ptr %56, !tbaa !2
  %850 = load i16, ptr %35, !tbaa !2
  %851 = mul i16 %850, 2
  %852 = getelementptr i8, ptr @"R%", i16 2
  %853 = load i16, ptr %852, !tbaa !2
  %854 = getelementptr i8, ptr @"R%", i16 10
  %855 = load i16, ptr %854, !tbaa !2
  %856 = add i16 %855, %851
  %857 = inttoptr i16 %853 to ptr addrspace(2)
  %858 = addrspacecast ptr addrspace(2) %857 to ptr addrspace(1)
  %859 = getelementptr i8, ptr addrspace(1) %858, i16 %856
  %860 = load i16, ptr addrspace(1) %859, !tbaa !60
  %861 = sub i16 %860, 0
  %862 = load i16, ptr %55, !tbaa !2
  store i16 %861, ptr %32, !tbaa !2
  %863 = load i16, ptr %32, !tbaa !2
  %864 = sitofp i16 %863 to float
  store i16 %862, ptr %31, !tbaa !2
  %865 = load i16, ptr %31, !tbaa !2
  %866 = sitofp i16 %865 to float
  %867 = fdiv float %864, %866
  store i16 %849, ptr %30, !tbaa !2
  %868 = load i16, ptr %30, !tbaa !2
  %869 = sitofp i16 %868 to float
  %870 = fmul float %869, %867
  store i16 0, ptr %29, !tbaa !2
  %871 = load i16, ptr %29, !tbaa !2
  %872 = sitofp i16 %871 to float
  %873 = fadd float %872, %870
  %874 = call i16 @llvm.lrint.i16.f32(float %873)
  %875 = trunc i16 %874 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %875)
  %876 = load i16, ptr %56, !tbaa !2
  %877 = load i16, ptr %35, !tbaa !2
  %878 = mul i16 %877, 2
  %879 = getelementptr i8, ptr @"G%", i16 2
  %880 = load i16, ptr %879, !tbaa !2
  %881 = getelementptr i8, ptr @"G%", i16 10
  %882 = load i16, ptr %881, !tbaa !2
  %883 = add i16 %882, %878
  %884 = inttoptr i16 %880 to ptr addrspace(2)
  %885 = addrspacecast ptr addrspace(2) %884 to ptr addrspace(1)
  %886 = getelementptr i8, ptr addrspace(1) %885, i16 %883
  %887 = load i16, ptr addrspace(1) %886, !tbaa !62
  %888 = sub i16 %887, 0
  %889 = load i16, ptr %55, !tbaa !2
  store i16 %888, ptr %28, !tbaa !2
  %890 = load i16, ptr %28, !tbaa !2
  %891 = sitofp i16 %890 to float
  store i16 %889, ptr %27, !tbaa !2
  %892 = load i16, ptr %27, !tbaa !2
  %893 = sitofp i16 %892 to float
  %894 = fdiv float %891, %893
  store i16 %876, ptr %26, !tbaa !2
  %895 = load i16, ptr %26, !tbaa !2
  %896 = sitofp i16 %895 to float
  %897 = fmul float %896, %894
  store i16 0, ptr %25, !tbaa !2
  %898 = load i16, ptr %25, !tbaa !2
  %899 = sitofp i16 %898 to float
  %900 = fadd float %899, %897
  %901 = call i16 @llvm.lrint.i16.f32(float %900)
  %902 = trunc i16 %901 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %902)
  %903 = load i16, ptr %56, !tbaa !2
  %904 = load i16, ptr %35, !tbaa !2
  %905 = mul i16 %904, 2
  %906 = getelementptr i8, ptr @"B%", i16 2
  %907 = load i16, ptr %906, !tbaa !2
  %908 = getelementptr i8, ptr @"B%", i16 10
  %909 = load i16, ptr %908, !tbaa !2
  %910 = add i16 %909, %905
  %911 = inttoptr i16 %907 to ptr addrspace(2)
  %912 = addrspacecast ptr addrspace(2) %911 to ptr addrspace(1)
  %913 = getelementptr i8, ptr addrspace(1) %912, i16 %910
  %914 = load i16, ptr addrspace(1) %913, !tbaa !64
  %915 = sub i16 %914, 0
  %916 = load i16, ptr %55, !tbaa !2
  store i16 %915, ptr %24, !tbaa !2
  %917 = load i16, ptr %24, !tbaa !2
  %918 = sitofp i16 %917 to float
  store i16 %916, ptr %23, !tbaa !2
  %919 = load i16, ptr %23, !tbaa !2
  %920 = sitofp i16 %919 to float
  %921 = fdiv float %918, %920
  store i16 %903, ptr %22, !tbaa !2
  %922 = load i16, ptr %22, !tbaa !2
  %923 = sitofp i16 %922 to float
  %924 = fmul float %923, %921
  store i16 0, ptr %21, !tbaa !2
  %925 = load i16, ptr %21, !tbaa !2
  %926 = sitofp i16 %925 to float
  %927 = fadd float %926, %924
  %928 = call i16 @llvm.lrint.i16.f32(float %927)
  %929 = trunc i16 %928 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %929)
  %930 = load i16, ptr %35, !tbaa !2
  %931 = load i16, ptr %33, !tbaa !2
  %932 = add nsw i16 %930, %931
  store i16 %932, ptr %35, !tbaa !2
  br label %b190

b194:
  br label %b2

b195:
  br label %b3

b196:
  br label %b197

b197:
  %933 = load i16, ptr %54, !tbaa !2
  %934 = load i16, ptr %53, !tbaa !2
  %935 = icmp slt i16 %933, %934
  br i1 %935, label %b199, label %b200

b199:
  %936 = load i16, ptr %54, !tbaa !2
  %937 = add i16 %936, 1
  store i16 %937, ptr %54, !tbaa !2
  br label %b201

b200:
  br label %b201

b201:
  %938 = load i16, ptr %50, !tbaa !2
  store i16 %938, ptr %35, !tbaa !2
  %939 = load i16, ptr %49, !tbaa !2
  store i16 %939, ptr %20, !tbaa !2
  store i16 1, ptr %19, !tbaa !2
  br label %b202

b202:
  %940 = load i16, ptr %19, !tbaa !2
  %941 = icmp sge i16 %940, 0
  br i1 %941, label %b203, label %b204

b203:
  %942 = load i16, ptr %35, !tbaa !2
  %943 = load i16, ptr %20, !tbaa !2
  %944 = icmp sle i16 %942, %943
  br i1 %944, label %b205, label %b206

b204:
  %945 = load i16, ptr %35, !tbaa !2
  %946 = load i16, ptr %20, !tbaa !2
  %947 = icmp sge i16 %945, %946
  br i1 %947, label %b205, label %b206

b205:
  %948 = load i16, ptr %35, !tbaa !2
  %949 = trunc i16 %948 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %949)
  %950 = load i16, ptr %35, !tbaa !2
  %951 = mul i16 %950, 2
  %952 = getelementptr i8, ptr @"R%", i16 2
  %953 = load i16, ptr %952, !tbaa !2
  %954 = getelementptr i8, ptr @"R%", i16 10
  %955 = load i16, ptr %954, !tbaa !2
  %956 = add i16 %955, %951
  %957 = inttoptr i16 %953 to ptr addrspace(2)
  %958 = addrspacecast ptr addrspace(2) %957 to ptr addrspace(1)
  %959 = getelementptr i8, ptr addrspace(1) %958, i16 %956
  %960 = load i16, ptr addrspace(1) %959, !tbaa !60
  %961 = load i16, ptr %54, !tbaa !2
  %962 = load i16, ptr %48, !tbaa !2
  %963 = load i16, ptr %35, !tbaa !2
  %964 = mul i16 %963, 2
  %965 = getelementptr i8, ptr @"R%", i16 2
  %966 = load i16, ptr %965, !tbaa !2
  %967 = getelementptr i8, ptr @"R%", i16 10
  %968 = load i16, ptr %967, !tbaa !2
  %969 = add i16 %968, %964
  %970 = inttoptr i16 %966 to ptr addrspace(2)
  %971 = addrspacecast ptr addrspace(2) %970 to ptr addrspace(1)
  %972 = getelementptr i8, ptr addrspace(1) %971, i16 %969
  %973 = load i16, ptr addrspace(1) %972, !tbaa !60
  %974 = sub i16 %962, %973
  %975 = load i16, ptr %53, !tbaa !2
  store i16 %974, ptr %18, !tbaa !2
  %976 = load i16, ptr %18, !tbaa !2
  %977 = sitofp i16 %976 to float
  store i16 %975, ptr %17, !tbaa !2
  %978 = load i16, ptr %17, !tbaa !2
  %979 = sitofp i16 %978 to float
  %980 = fdiv float %977, %979
  store i16 %961, ptr %16, !tbaa !2
  %981 = load i16, ptr %16, !tbaa !2
  %982 = sitofp i16 %981 to float
  %983 = fmul float %982, %980
  store i16 %960, ptr %15, !tbaa !2
  %984 = load i16, ptr %15, !tbaa !2
  %985 = sitofp i16 %984 to float
  %986 = fadd float %985, %983
  %987 = call i16 @llvm.lrint.i16.f32(float %986)
  %988 = trunc i16 %987 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %988)
  %989 = load i16, ptr %35, !tbaa !2
  %990 = mul i16 %989, 2
  %991 = getelementptr i8, ptr @"G%", i16 2
  %992 = load i16, ptr %991, !tbaa !2
  %993 = getelementptr i8, ptr @"G%", i16 10
  %994 = load i16, ptr %993, !tbaa !2
  %995 = add i16 %994, %990
  %996 = inttoptr i16 %992 to ptr addrspace(2)
  %997 = addrspacecast ptr addrspace(2) %996 to ptr addrspace(1)
  %998 = getelementptr i8, ptr addrspace(1) %997, i16 %995
  %999 = load i16, ptr addrspace(1) %998, !tbaa !62
  %1000 = load i16, ptr %54, !tbaa !2
  %1001 = load i16, ptr %47, !tbaa !2
  %1002 = load i16, ptr %35, !tbaa !2
  %1003 = mul i16 %1002, 2
  %1004 = getelementptr i8, ptr @"G%", i16 2
  %1005 = load i16, ptr %1004, !tbaa !2
  %1006 = getelementptr i8, ptr @"G%", i16 10
  %1007 = load i16, ptr %1006, !tbaa !2
  %1008 = add i16 %1007, %1003
  %1009 = inttoptr i16 %1005 to ptr addrspace(2)
  %1010 = addrspacecast ptr addrspace(2) %1009 to ptr addrspace(1)
  %1011 = getelementptr i8, ptr addrspace(1) %1010, i16 %1008
  %1012 = load i16, ptr addrspace(1) %1011, !tbaa !62
  %1013 = sub i16 %1001, %1012
  %1014 = load i16, ptr %53, !tbaa !2
  store i16 %1013, ptr %14, !tbaa !2
  %1015 = load i16, ptr %14, !tbaa !2
  %1016 = sitofp i16 %1015 to float
  store i16 %1014, ptr %13, !tbaa !2
  %1017 = load i16, ptr %13, !tbaa !2
  %1018 = sitofp i16 %1017 to float
  %1019 = fdiv float %1016, %1018
  store i16 %1000, ptr %12, !tbaa !2
  %1020 = load i16, ptr %12, !tbaa !2
  %1021 = sitofp i16 %1020 to float
  %1022 = fmul float %1021, %1019
  store i16 %999, ptr %11, !tbaa !2
  %1023 = load i16, ptr %11, !tbaa !2
  %1024 = sitofp i16 %1023 to float
  %1025 = fadd float %1024, %1022
  %1026 = call i16 @llvm.lrint.i16.f32(float %1025)
  %1027 = trunc i16 %1026 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1027)
  %1028 = load i16, ptr %35, !tbaa !2
  %1029 = mul i16 %1028, 2
  %1030 = getelementptr i8, ptr @"B%", i16 2
  %1031 = load i16, ptr %1030, !tbaa !2
  %1032 = getelementptr i8, ptr @"B%", i16 10
  %1033 = load i16, ptr %1032, !tbaa !2
  %1034 = add i16 %1033, %1029
  %1035 = inttoptr i16 %1031 to ptr addrspace(2)
  %1036 = addrspacecast ptr addrspace(2) %1035 to ptr addrspace(1)
  %1037 = getelementptr i8, ptr addrspace(1) %1036, i16 %1034
  %1038 = load i16, ptr addrspace(1) %1037, !tbaa !64
  %1039 = load i16, ptr %54, !tbaa !2
  %1040 = load i16, ptr %46, !tbaa !2
  %1041 = load i16, ptr %35, !tbaa !2
  %1042 = mul i16 %1041, 2
  %1043 = getelementptr i8, ptr @"B%", i16 2
  %1044 = load i16, ptr %1043, !tbaa !2
  %1045 = getelementptr i8, ptr @"B%", i16 10
  %1046 = load i16, ptr %1045, !tbaa !2
  %1047 = add i16 %1046, %1042
  %1048 = inttoptr i16 %1044 to ptr addrspace(2)
  %1049 = addrspacecast ptr addrspace(2) %1048 to ptr addrspace(1)
  %1050 = getelementptr i8, ptr addrspace(1) %1049, i16 %1047
  %1051 = load i16, ptr addrspace(1) %1050, !tbaa !64
  %1052 = sub i16 %1040, %1051
  %1053 = load i16, ptr %53, !tbaa !2
  store i16 %1052, ptr %10, !tbaa !2
  %1054 = load i16, ptr %10, !tbaa !2
  %1055 = sitofp i16 %1054 to float
  store i16 %1053, ptr %9, !tbaa !2
  %1056 = load i16, ptr %9, !tbaa !2
  %1057 = sitofp i16 %1056 to float
  %1058 = fdiv float %1055, %1057
  store i16 %1039, ptr %8, !tbaa !2
  %1059 = load i16, ptr %8, !tbaa !2
  %1060 = sitofp i16 %1059 to float
  %1061 = fmul float %1060, %1058
  store i16 %1038, ptr %7, !tbaa !2
  %1062 = load i16, ptr %7, !tbaa !2
  %1063 = sitofp i16 %1062 to float
  %1064 = fadd float %1063, %1061
  %1065 = call i16 @llvm.lrint.i16.f32(float %1064)
  %1066 = trunc i16 %1065 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1066)
  %1067 = load i16, ptr %35, !tbaa !2
  %1068 = load i16, ptr %19, !tbaa !2
  %1069 = add nsw i16 %1067, %1068
  store i16 %1069, ptr %35, !tbaa !2
  br label %b202

b206:
  br label %b3

b207:
  %1070 = load i16, ptr %4, !tbaa !2
  %1071 = icmp sge i16 %1070, 0
  br i1 %1071, label %b208, label %b209

b208:
  %1072 = load i16, ptr %93, !tbaa !2
  %1073 = load i16, ptr %5, !tbaa !2
  %1074 = icmp sle i16 %1072, %1073
  br i1 %1074, label %b210, label %b211

b209:
  %1075 = load i16, ptr %93, !tbaa !2
  %1076 = load i16, ptr %5, !tbaa !2
  %1077 = icmp sge i16 %1075, %1076
  br i1 %1077, label %b210, label %b211

b210:
  %1078 = load i16, ptr %6, !tbaa !2
  %1079 = add i16 -24576, %1078
  store i16 %1079, ptr @b$seg, !tbaa !2
  %1080 = load i16, ptr %6, !tbaa !2
  %1081 = add i16 %1080, 20
  store i16 %1081, ptr %6, !tbaa !2
  %1082 = load i16, ptr %93, !tbaa !2
  %1083 = getelementptr inbounds i16, ptr @"YP1%", i16 1
  %1084 = load i16, ptr %1083, !tbaa !2
  %1085 = sub i16 %1082, %1084
  %1086 = sub i16 %1085, -100
  %1087 = getelementptr inbounds i16, ptr @"CY%", i16 %1086
  %1088 = load i16, ptr %1087, !tbaa !2
  %1089 = getelementptr inbounds i16, ptr @"YY%", i16 1
  store i16 %1088, ptr %1089, !tbaa !2
  %1090 = load i16, ptr %93, !tbaa !2
  %1091 = getelementptr inbounds i16, ptr @"YP1%", i16 2
  %1092 = load i16, ptr %1091, !tbaa !2
  %1093 = sub i16 %1090, %1092
  %1094 = sub i16 %1093, -100
  %1095 = getelementptr inbounds i16, ptr @"CY%", i16 %1094
  %1096 = load i16, ptr %1095, !tbaa !2
  %1097 = getelementptr inbounds i16, ptr @"YY%", i16 2
  store i16 %1096, ptr %1097, !tbaa !2
  %1098 = load i16, ptr %93, !tbaa !2
  %1099 = getelementptr inbounds i16, ptr @"YP1%", i16 3
  %1100 = load i16, ptr %1099, !tbaa !2
  %1101 = sub i16 %1098, %1100
  %1102 = sub i16 %1101, -100
  %1103 = getelementptr inbounds i16, ptr @"CY%", i16 %1102
  %1104 = load i16, ptr %1103, !tbaa !2
  %1105 = getelementptr inbounds i16, ptr @"YY%", i16 3
  store i16 %1104, ptr %1105, !tbaa !2
  store i16 0, ptr %90, !tbaa !2
  store i16 319, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b212

b211:
  %1106 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1106, ptr @"BENCHFRAME&", !tbaa !2
  %1107 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %1107, ptr @"BENCHFRAME&", !tbaa !2
  %1108 = load i16, ptr @"XIT%", !tbaa !2
  %1109 = icmp eq i16 %1108, 0
  br i1 %1109, label %b220, label %b221

b212:
  %1110 = load i16, ptr %2, !tbaa !2
  %1111 = icmp sge i16 %1110, 0
  br i1 %1111, label %b213, label %b214

b213:
  %1112 = load i16, ptr %90, !tbaa !2
  %1113 = load i16, ptr %3, !tbaa !2
  %1114 = icmp sle i16 %1112, %1113
  br i1 %1114, label %b215, label %b216

b214:
  %1115 = load i16, ptr %90, !tbaa !2
  %1116 = load i16, ptr %3, !tbaa !2
  %1117 = icmp sge i16 %1115, %1116
  br i1 %1117, label %b215, label %b216

b215:
  %1118 = getelementptr inbounds i16, ptr @"YY%", i16 1
  %1119 = load i16, ptr %1118, !tbaa !2
  %1120 = load i16, ptr %90, !tbaa !2
  %1121 = getelementptr inbounds i16, ptr @"XP1%", i16 1
  %1122 = load i16, ptr %1121, !tbaa !2
  %1123 = sub i16 %1120, %1122
  %1124 = sub i16 %1123, -319
  %1125 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %1124
  %1126 = load i16, ptr %1125, !tbaa !2
  %1127 = add i16 %1119, %1126
  %1128 = sub i16 %1127, 0
  %1129 = getelementptr inbounds i16, ptr @"SP%", i16 %1128
  %1130 = load i16, ptr %1129, !tbaa !2
  %1131 = getelementptr inbounds i16, ptr @"YY%", i16 2
  %1132 = load i16, ptr %1131, !tbaa !2
  %1133 = load i16, ptr %90, !tbaa !2
  %1134 = getelementptr inbounds i16, ptr @"XP1%", i16 2
  %1135 = load i16, ptr %1134, !tbaa !2
  %1136 = sub i16 %1133, %1135
  %1137 = sub i16 %1136, -319
  %1138 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %1137
  %1139 = load i16, ptr %1138, !tbaa !2
  %1140 = add i16 %1132, %1139
  %1141 = sub i16 %1140, 0
  %1142 = getelementptr inbounds i16, ptr @"SP%", i16 %1141
  %1143 = load i16, ptr %1142, !tbaa !2
  %1144 = add i16 %1130, %1143
  %1145 = getelementptr inbounds i16, ptr @"YY%", i16 3
  %1146 = load i16, ptr %1145, !tbaa !2
  %1147 = load i16, ptr %90, !tbaa !2
  %1148 = getelementptr inbounds i16, ptr @"XP1%", i16 3
  %1149 = load i16, ptr %1148, !tbaa !2
  %1150 = sub i16 %1147, %1149
  %1151 = sub i16 %1150, -319
  %1152 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %1151
  %1153 = load i16, ptr %1152, !tbaa !2
  %1154 = add i16 %1146, %1153
  %1155 = sub i16 %1154, 0
  %1156 = getelementptr inbounds i16, ptr @"SP%", i16 %1155
  %1157 = load i16, ptr %1156, !tbaa !2
  %1158 = add i16 %1144, %1157
  %1159 = sub i16 %1158, 0
  %1160 = getelementptr inbounds i16, ptr @"CD%", i16 %1159
  %1161 = load i16, ptr %1160, !tbaa !2
  store i16 %1161, ptr %1, !tbaa !2
  %1162 = load i16, ptr %1, !tbaa !2
  %1163 = icmp slt i16 %1162, 192
  br i1 %1163, label %b217, label %b218

b216:
  %1164 = load i16, ptr %93, !tbaa !2
  %1165 = load i16, ptr %4, !tbaa !2
  %1166 = add nsw i16 %1164, %1165
  store i16 %1166, ptr %93, !tbaa !2
  br label %b207

b217:
  %1167 = load i16, ptr %90, !tbaa !2
  %1168 = load i16, ptr %1, !tbaa !2
  %1169 = trunc i16 %1168 to i8
  %1170 = load i16, ptr @b$seg, !tbaa !2
  %1171 = inttoptr i16 %1170 to ptr addrspace(2)
  %1172 = addrspacecast ptr addrspace(2) %1171 to ptr addrspace(1)
  %1173 = getelementptr i8, ptr addrspace(1) %1172, i16 %1167
  store volatile i8 %1169, ptr addrspace(1) %1173
  br label %b219

b218:
  %1174 = load i16, ptr %90, !tbaa !2
  %1175 = load i16, ptr %99, !tbaa !2
  %1176 = add i16 %1174, %1175
  %1177 = sub i16 %1176, -640
  %1178 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1177
  %1179 = load i16, ptr %1178, !tbaa !2
  %1180 = load i16, ptr %93, !tbaa !2
  %1181 = sub i16 %1180, -640
  %1182 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1181
  %1183 = load i16, ptr %1182, !tbaa !2
  %1184 = add i16 %1179, %1183
  %1185 = load i16, ptr %90, !tbaa !2
  %1186 = load i16, ptr %93, !tbaa !2
  %1187 = add i16 %1185, %1186
  %1188 = sub i16 %1187, -48
  %1189 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %1188
  %1190 = load i16, ptr %1189, !tbaa !2
  %1191 = add i16 %1184, %1190
  %1192 = load i16, ptr %93, !tbaa !2
  %1193 = load i16, ptr %99, !tbaa !2
  %1194 = sub i16 %1192, %1193
  %1195 = sub i16 %1194, -640
  %1196 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1195
  %1197 = load i16, ptr %1196, !tbaa !2
  %1198 = load i16, ptr %90, !tbaa !2
  %1199 = sub i16 %1198, -640
  %1200 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %1199
  %1201 = load i16, ptr %1200, !tbaa !2
  %1202 = add i16 %1197, %1201
  %1203 = load i16, ptr %99, !tbaa !2
  %1204 = add i16 %1202, %1203
  %1205 = sub i16 %1204, -640
  %1206 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %1205
  %1207 = load i16, ptr %1206, !tbaa !2
  %1208 = add i16 %1191, %1207
  %1209 = sub i16 %1208, -180
  %1210 = getelementptr inbounds i16, ptr @"DT100%", i16 %1209
  %1211 = load i16, ptr %1210, !tbaa !2
  store i16 %1211, ptr %0, !tbaa !2
  %1212 = load i16, ptr %90, !tbaa !2
  %1213 = load i16, ptr %1, !tbaa !2
  %1214 = sub i16 %1213, 192
  %1215 = sub i16 %1214, -192
  %1216 = getelementptr inbounds i16, ptr @"DT%", i16 %1215
  %1217 = load i16, ptr %1216, !tbaa !2
  %1218 = load i16, ptr %0, !tbaa !2
  %1219 = add i16 %1217, %1218
  %1220 = mul i16 %1219, 2
  %1221 = getelementptr i8, ptr %132, i16 2
  %1222 = load i16, ptr %1221, !tbaa !2
  %1223 = add i16 0, %1220
  %1224 = inttoptr i16 %1222 to ptr addrspace(2)
  %1225 = addrspacecast ptr addrspace(2) %1224 to ptr addrspace(1)
  %1226 = getelementptr i8, ptr addrspace(1) %1225, i16 %1223
  %1227 = load i16, ptr addrspace(1) %1226, !tbaa !70
  %1228 = add i16 %1227, 192
  %1229 = trunc i16 %1228 to i8
  %1230 = load i16, ptr @b$seg, !tbaa !2
  %1231 = inttoptr i16 %1230 to ptr addrspace(2)
  %1232 = addrspacecast ptr addrspace(2) %1231 to ptr addrspace(1)
  %1233 = getelementptr i8, ptr addrspace(1) %1232, i16 %1212
  store volatile i8 %1229, ptr addrspace(1) %1233
  br label %b219

b219:
  %1234 = load i16, ptr %90, !tbaa !2
  %1235 = load i16, ptr %2, !tbaa !2
  %1236 = add nsw i16 %1234, %1235
  store i16 %1236, ptr %90, !tbaa !2
  br label %b212

b220:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b222

b221:
  br label %b222

b222:
  %1237 = load i16, ptr @"XIT%", !tbaa !2
  %1238 = icmp eq i16 %1237, 1
  br i1 %1238, label %b223, label %b224

b223:
  br label %b4

b224:
  br label %b225

b225:
  br label %b159
}

define cc 1000 void @PRECALCULATIONS() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca float
  %3 = alloca i16
  %4 = alloca float
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca float
  %10 = alloca float
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  store i16 0, ptr %25, !tbaa !2
  store i16 255, ptr %24, !tbaa !2
  store i16 1, ptr %23, !tbaa !2
  br label %b2

b2:
  %26 = load i16, ptr %23, !tbaa !2
  %27 = icmp sge i16 %26, 0
  br i1 %27, label %b3, label %b4

b3:
  %28 = load i16, ptr %25, !tbaa !2
  %29 = load i16, ptr %24, !tbaa !2
  %30 = icmp sle i16 %28, %29
  br i1 %30, label %b5, label %b6

b4:
  %31 = load i16, ptr %25, !tbaa !2
  %32 = load i16, ptr %24, !tbaa !2
  %33 = icmp sge i16 %31, %32
  br i1 %33, label %b5, label %b6

b5:
  %34 = load i16, ptr %25, !tbaa !2
  %35 = load i16, ptr %25, !tbaa !2
  %36 = sext i16 %35 to i32
  %37 = sdiv i32 %36, 2
  %38 = trunc i32 %37 to i16
  %39 = sub i16 %34, 0
  %40 = getelementptr inbounds i16, ptr @"DIV2%", i16 %39
  store i16 %38, ptr %40, !tbaa !2
  %41 = load i16, ptr %25, !tbaa !2
  %42 = load i16, ptr %23, !tbaa !2
  %43 = add nsw i16 %41, %42
  store i16 %43, ptr %25, !tbaa !2
  br label %b2

b6:
  store i16 0, ptr %25, !tbaa !2
  store i16 320, ptr %22, !tbaa !2
  store i16 1, ptr %21, !tbaa !2
  br label %b7

b7:
  %44 = load i16, ptr %21, !tbaa !2
  %45 = icmp sge i16 %44, 0
  br i1 %45, label %b8, label %b9

b8:
  %46 = load i16, ptr %25, !tbaa !2
  %47 = load i16, ptr %22, !tbaa !2
  %48 = icmp sle i16 %46, %47
  br i1 %48, label %b10, label %b11

b9:
  %49 = load i16, ptr %25, !tbaa !2
  %50 = load i16, ptr %22, !tbaa !2
  %51 = icmp sge i16 %49, %50
  br i1 %51, label %b10, label %b11

b10:
  %52 = load i16, ptr %25, !tbaa !2
  %53 = load i16, ptr %25, !tbaa !2
  %54 = sext i16 %53 to i32
  %55 = sdiv i32 %54, 4
  %56 = trunc i32 %55 to i16
  %57 = sub i16 %52, 0
  %58 = getelementptr inbounds i16, ptr @"DIV4%", i16 %57
  store i16 %56, ptr %58, !tbaa !2
  %59 = load i16, ptr %25, !tbaa !2
  %60 = load i16, ptr %21, !tbaa !2
  %61 = add nsw i16 %59, %60
  store i16 %61, ptr %25, !tbaa !2
  br label %b7

b11:
  store i16 0, ptr %25, !tbaa !2
  store i16 32, ptr %20, !tbaa !2
  store i16 1, ptr %19, !tbaa !2
  br label %b12

b12:
  %62 = load i16, ptr %19, !tbaa !2
  %63 = icmp sge i16 %62, 0
  br i1 %63, label %b13, label %b14

b13:
  %64 = load i16, ptr %25, !tbaa !2
  %65 = load i16, ptr %20, !tbaa !2
  %66 = icmp sle i16 %64, %65
  br i1 %66, label %b15, label %b16

b14:
  %67 = load i16, ptr %25, !tbaa !2
  %68 = load i16, ptr %20, !tbaa !2
  %69 = icmp sge i16 %67, %68
  br i1 %69, label %b15, label %b16

b15:
  %70 = load i16, ptr %25, !tbaa !2
  %71 = load i16, ptr %25, !tbaa !2
  %72 = mul i16 %71, 32
  %73 = sub i16 %70, 0
  %74 = getelementptr inbounds i16, ptr @"MUL32%", i16 %73
  store i16 %72, ptr %74, !tbaa !2
  %75 = load i16, ptr %25, !tbaa !2
  %76 = load i16, ptr %19, !tbaa !2
  %77 = add nsw i16 %75, %76
  store i16 %77, ptr %25, !tbaa !2
  br label %b12

b16:
  store i16 0, ptr %25, !tbaa !2
  store i16 127, ptr %18, !tbaa !2
  store i16 1, ptr %17, !tbaa !2
  br label %b17

b17:
  %78 = load i16, ptr %17, !tbaa !2
  %79 = icmp sge i16 %78, 0
  br i1 %79, label %b18, label %b19

b18:
  %80 = load i16, ptr %25, !tbaa !2
  %81 = load i16, ptr %18, !tbaa !2
  %82 = icmp sle i16 %80, %81
  br i1 %82, label %b20, label %b21

b19:
  %83 = load i16, ptr %25, !tbaa !2
  %84 = load i16, ptr %18, !tbaa !2
  %85 = icmp sge i16 %83, %84
  br i1 %85, label %b20, label %b21

b20:
  %86 = load i16, ptr %25, !tbaa !2
  %87 = load i16, ptr %25, !tbaa !2
  %88 = mul i16 %87, 128
  %89 = sub i16 %86, 0
  %90 = getelementptr inbounds i16, ptr @"MUL128%", i16 %89
  store i16 %88, ptr %90, !tbaa !2
  %91 = load i16, ptr %25, !tbaa !2
  %92 = load i16, ptr %17, !tbaa !2
  %93 = add nsw i16 %91, %92
  store i16 %93, ptr %25, !tbaa !2
  br label %b17

b21:
  store i16 0, ptr %25, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store i16 127, ptr %15, !tbaa !2
  store i16 1, ptr %14, !tbaa !2
  br label %b22

b22:
  %94 = load i16, ptr %14, !tbaa !2
  %95 = icmp sge i16 %94, 0
  br i1 %95, label %b23, label %b24

b23:
  %96 = load i16, ptr %16, !tbaa !2
  %97 = load i16, ptr %15, !tbaa !2
  %98 = icmp sle i16 %96, %97
  br i1 %98, label %b25, label %b26

b24:
  %99 = load i16, ptr %16, !tbaa !2
  %100 = load i16, ptr %15, !tbaa !2
  %101 = icmp sge i16 %99, %100
  br i1 %101, label %b25, label %b26

b25:
  store i16 0, ptr %13, !tbaa !2
  store i16 127, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b27

b26:
  %102 = load float, ptr @$float230, !tbaa !2
  store float %102, ptr %10, !tbaa !2
  store i16 0, ptr %8, !tbaa !2
  %103 = load i16, ptr %8, !tbaa !2
  %104 = sitofp i16 %103 to float
  %105 = fpround float %104 to float
  store float %105, ptr %9, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  store i16 99, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b32

b27:
  %106 = load i16, ptr %11, !tbaa !2
  %107 = icmp sge i16 %106, 0
  br i1 %107, label %b28, label %b29

b28:
  %108 = load i16, ptr %13, !tbaa !2
  %109 = load i16, ptr %12, !tbaa !2
  %110 = icmp sle i16 %108, %109
  br i1 %110, label %b30, label %b31

b29:
  %111 = load i16, ptr %13, !tbaa !2
  %112 = load i16, ptr %12, !tbaa !2
  %113 = icmp sge i16 %111, %112
  br i1 %113, label %b30, label %b31

b30:
  %114 = load i16, ptr %25, !tbaa !2
  %115 = load i16, ptr %16, !tbaa !2
  %116 = sext i16 %115 to i32
  %117 = sdiv i32 %116, 4
  %118 = trunc i32 %117 to i16
  %119 = mul i16 %118, 32
  %120 = load i16, ptr %13, !tbaa !2
  %121 = sext i16 %120 to i32
  %122 = sdiv i32 %121, 4
  %123 = trunc i32 %122 to i16
  %124 = add i16 %119, %123
  %125 = sub i16 %124, 0
  %126 = getelementptr inbounds i16, ptr @"CD%", i16 %125
  %127 = load i16, ptr %126, !tbaa !2
  %128 = mul i16 %127, 128
  %129 = sub i16 %114, 0
  %130 = getelementptr inbounds i16, ptr @"SP%", i16 %129
  store i16 %128, ptr %130, !tbaa !2
  %131 = load i16, ptr %25, !tbaa !2
  %132 = add i16 %131, 1
  store i16 %132, ptr %25, !tbaa !2
  %133 = load i16, ptr %13, !tbaa !2
  %134 = load i16, ptr %11, !tbaa !2
  %135 = add nsw i16 %133, %134
  store i16 %135, ptr %13, !tbaa !2
  br label %b27

b31:
  %136 = load i16, ptr %16, !tbaa !2
  %137 = load i16, ptr %14, !tbaa !2
  %138 = add nsw i16 %136, %137
  store i16 %138, ptr %16, !tbaa !2
  br label %b22

b32:
  %139 = load i16, ptr %6, !tbaa !2
  %140 = icmp sge i16 %139, 0
  br i1 %140, label %b33, label %b34

b33:
  %141 = load i16, ptr %25, !tbaa !2
  %142 = load i16, ptr %7, !tbaa !2
  %143 = icmp sle i16 %141, %142
  br i1 %143, label %b35, label %b36

b34:
  %144 = load i16, ptr %25, !tbaa !2
  %145 = load i16, ptr %7, !tbaa !2
  %146 = icmp sge i16 %144, %145
  br i1 %146, label %b35, label %b36

b35:
  %147 = load float, ptr %9, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  %148 = load i16, ptr %5, !tbaa !2
  %149 = sitofp i16 %148 to float
  %150 = fcmp oeq float %147, %149
  br i1 %150, label %b37, label %b38

b36:
  ret void

b37:
  %151 = load float, ptr @$float231, !tbaa !2
  store float %151, ptr %9, !tbaa !2
  br label %b39

b38:
  br label %b39

b39:
  %152 = load float, ptr %9, !tbaa !2
  %153 = load float, ptr @$float93, !tbaa !2
  %154 = fdiv float %152, %153
  %155 = call float @llvm.fabs.f32(float %154)
  %156 = load float, ptr @$float86, !tbaa !2
  %157 = fcmp ogt float %155, %156
  br i1 %157, label %b40, label %b41

b40:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b41:
  %158 = call float @llvm.sin.f32(float %154)
  store i16 24, ptr %3, !tbaa !2
  %159 = load i16, ptr %3, !tbaa !2
  %160 = sitofp i16 %159 to float
  %161 = fdiv float %160, %158
  %162 = fpround float %161 to float
  store float %162, ptr %4, !tbaa !2
  %163 = load float, ptr %4, !tbaa !2
  store i16 255, ptr %1, !tbaa !2
  %164 = load i16, ptr %1, !tbaa !2
  %165 = sitofp i16 %164 to float
  %166 = fsub float %165, %163
  %167 = fpround float %166 to float
  store float %167, ptr %2, !tbaa !2
  %168 = load float, ptr %2, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  %169 = load i16, ptr %0, !tbaa !2
  %170 = sitofp i16 %169 to float
  %171 = fcmp olt float %168, %170
  br i1 %171, label %b42, label %b43

b42:
  %172 = load i16, ptr %25, !tbaa !2
  %173 = sub i16 99, %172
  %174 = sub i16 %173, 0
  %175 = getelementptr inbounds i16, ptr @"FS%", i16 %174
  store i16 0, ptr %175, !tbaa !2
  br label %b44

b43:
  %176 = load i16, ptr %25, !tbaa !2
  %177 = sub i16 99, %176
  %178 = load float, ptr %2, !tbaa !2
  %179 = call i16 @llvm.lrint.i16.f32(float %178)
  %180 = sub i16 %177, 0
  %181 = getelementptr inbounds i16, ptr @"FS%", i16 %180
  store i16 %179, ptr %181, !tbaa !2
  br label %b44

b44:
  %182 = load i16, ptr %25, !tbaa !2
  %183 = add i16 %182, 100
  %184 = load i16, ptr %25, !tbaa !2
  %185 = sub i16 99, %184
  %186 = sub i16 %185, 0
  %187 = getelementptr inbounds i16, ptr @"FS%", i16 %186
  %188 = load i16, ptr %187, !tbaa !2
  %189 = sub i16 %183, 0
  %190 = getelementptr inbounds i16, ptr @"FS%", i16 %189
  store i16 %188, ptr %190, !tbaa !2
  %191 = load float, ptr %9, !tbaa !2
  %192 = load float, ptr %10, !tbaa !2
  %193 = fadd float %191, %192
  %194 = fpround float %193 to float
  store float %194, ptr %9, !tbaa !2
  %195 = load i16, ptr %25, !tbaa !2
  %196 = load i16, ptr %6, !tbaa !2
  %197 = add nsw i16 %195, %196
  store i16 %197, ptr %25, !tbaa !2
  br label %b32
}

define cc 1000 void @PREHISTORICODE() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = alloca i32
  %2 = alloca float
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i32
  %7 = alloca i32
  %8 = alloca i32
  %9 = alloca float
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca [4 x i8]
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca float
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  store i32 0, ptr %0
  store i32 0, ptr %1
  store float 0.000000e+00, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i32 0, ptr %6
  store i32 0, ptr %7
  store i32 0, ptr %8
  store float 0.000000e+00, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  call void @llvm.memset.p0.i16(ptr %18, i8 0, i16 4, i1 false)
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store float 0.000000e+00, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store i16 0, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  store i16 0, ptr %33
  store i16 0, ptr %34
  store i16 0, ptr %35
  store i16 0, ptr %36
  store i16 0, ptr %37
  store i16 0, ptr %38
  store i16 0, ptr %28, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 0, ptr %23, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %18, i16 2
  store i16 0, ptr %39, !tbaa !2
  store i16 15, ptr %38, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 12, i16 2)
  br label %b2

b2:
  call cc 1000 addrspace(1) void @KEYACTION()
  %40 = load i16, ptr @"XIT%", !tbaa !2
  %41 = icmp eq i16 %40, 1
  br i1 %41, label %b4, label %b5

b3:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %18)
  ret void

b4:
  br label %b3

b5:
  br label %b6

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 15, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 15, i16 1, i16 18, i16 4)
  %42 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 34)
  %43 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string232, ptr %42)
  %44 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %43, ptr @$string233)
  %45 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 34)
  %46 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %44, ptr %45)
  %47 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %46, ptr @$string234)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %47)
  %48 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %49 = load float, ptr %48
  store i16 63, ptr %36, !tbaa !2
  %50 = load i16, ptr %36, !tbaa !2
  %51 = sitofp i16 %50 to float
  %52 = fmul float %49, %51
  store i16 1, ptr %35, !tbaa !2
  %53 = load i16, ptr %35, !tbaa !2
  %54 = sitofp i16 %53 to float
  %55 = fadd float %52, %54
  %56 = call float @llvm.rint.f32(float %55)
  %57 = fcmp olt float %55, %56
  %58 = sext i1 %57 to i16
  store i16 %58, ptr %34, !tbaa !2
  %59 = load i16, ptr %34, !tbaa !2
  %60 = sitofp i16 %59 to float
  %61 = fadd float %56, %60
  %62 = call i16 @llvm.lrint.i16.f32(float %61)
  store i16 %62, ptr %37, !tbaa !2
  store i16 1, ptr %33, !tbaa !2
  store i16 640, ptr %32, !tbaa !2
  store i16 15, ptr %31, !tbaa !2
  br label %b8

b8:
  %63 = load i16, ptr %31, !tbaa !2
  %64 = icmp sge i16 %63, 0
  br i1 %64, label %b9, label %b10

b9:
  %65 = load i16, ptr %33, !tbaa !2
  %66 = load i16, ptr %32, !tbaa !2
  %67 = icmp sle i16 %65, %66
  br i1 %67, label %b11, label %b12

b10:
  %68 = load i16, ptr %33, !tbaa !2
  %69 = load i16, ptr %32, !tbaa !2
  %70 = icmp sge i16 %68, %69
  br i1 %70, label %b11, label %b12

b11:
  %71 = load i16, ptr %33, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %71, i16 480)
  %72 = load i16, ptr %33, !tbaa !2
  %73 = sub i16 480, %72
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 640, i16 %73)
  %74 = load i16, ptr %33, !tbaa !2
  %75 = load i16, ptr %37, !tbaa !2
  store i16 %74, ptr %30, !tbaa !2
  %76 = load i16, ptr %30, !tbaa !2
  %77 = sitofp i16 %76 to float
  store i16 %75, ptr %29, !tbaa !2
  %78 = load i16, ptr %29, !tbaa !2
  %79 = sitofp i16 %78 to float
  %80 = fdiv float %77, %79
  %81 = call i16 @llvm.lrint.i16.f32(float %80)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %81, i16 -1, i16 0)
  %82 = load i16, ptr %33, !tbaa !2
  %83 = load i16, ptr %31, !tbaa !2
  %84 = add nsw i16 %82, %83
  store i16 %84, ptr %33, !tbaa !2
  br label %b8

b12:
  %85 = load i16, ptr %28, !tbaa !2
  %86 = add i16 %85, 1
  store i16 %86, ptr %28, !tbaa !2
  %87 = load i16, ptr %27, !tbaa !2
  %88 = load i16, ptr %26, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %87, i16 %88)
  %89 = load i16, ptr %28, !tbaa !2
  store i16 %89, ptr %25, !tbaa !2
  %90 = load i16, ptr %25, !tbaa !2
  %91 = sitofp i16 %90 to float
  %92 = fpround float %91 to float
  store float %92, ptr %24, !tbaa !2
  %93 = load i16, ptr %38, !tbaa !2
  %94 = load float, ptr %24, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %94, i16 %93)
  %95 = load i16, ptr %28, !tbaa !2
  %96 = icmp eq i16 %95, 30
  br i1 %96, label %b13, label %b14

b13:
  store i16 0, ptr %28, !tbaa !2
  store i16 0, ptr %38, !tbaa !2
  store i16 1, ptr %23, !tbaa !2
  br label %b2

b14:
  br label %b15

b15:
  %97 = load i16, ptr %23, !tbaa !2
  %98 = icmp eq i16 %97, 1
  %99 = sext i1 %98 to i16
  %100 = load i16, ptr %28, !tbaa !2
  %101 = icmp eq i16 %100, 29
  %102 = sext i1 %101 to i16
  %103 = and i16 %99, %102
  %104 = icmp ne i16 %103, 0
  br i1 %104, label %b17, label %b18

b17:
  store i16 0, ptr %28, !tbaa !2
  store i16 15, ptr %38, !tbaa !2
  %105 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %106 = load float, ptr %105
  store i16 640, ptr %22, !tbaa !2
  %107 = load i16, ptr %22, !tbaa !2
  %108 = sitofp i16 %107 to float
  %109 = fmul float %106, %108
  %110 = call float @llvm.rint.f32(float %109)
  %111 = fcmp olt float %109, %110
  %112 = sext i1 %111 to i16
  store i16 %112, ptr %21, !tbaa !2
  %113 = load i16, ptr %21, !tbaa !2
  %114 = sitofp i16 %113 to float
  %115 = fadd float %110, %114
  %116 = call i16 @llvm.lrint.i16.f32(float %115)
  store i16 %116, ptr %27, !tbaa !2
  %117 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %118 = load float, ptr %117
  store i16 480, ptr %20, !tbaa !2
  %119 = load i16, ptr %20, !tbaa !2
  %120 = sitofp i16 %119 to float
  %121 = fmul float %118, %120
  %122 = call float @llvm.rint.f32(float %121)
  %123 = fcmp olt float %121, %122
  %124 = sext i1 %123 to i16
  store i16 %124, ptr %19, !tbaa !2
  %125 = load i16, ptr %19, !tbaa !2
  %126 = sitofp i16 %125 to float
  %127 = fadd float %122, %126
  %128 = call i16 @llvm.lrint.i16.f32(float %127)
  store i16 %128, ptr %26, !tbaa !2
  store i16 0, ptr %23, !tbaa !2
  br label %b19

b18:
  br label %b19

b19:
  %129 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %129, ptr %18)
  br label %b20

b20:
  %130 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %18, ptr @$string235)
  %131 = icmp eq i16 %130, 0
  br i1 %131, label %b21, label %b22

b21:
  br label %b2

b22:
  store i16 1, ptr %17, !tbaa !2
  store i16 480, ptr %16, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  br label %b24

b24:
  %132 = load i16, ptr %15, !tbaa !2
  %133 = icmp sge i16 %132, 0
  br i1 %133, label %b25, label %b26

b25:
  %134 = load i16, ptr %17, !tbaa !2
  %135 = load i16, ptr %16, !tbaa !2
  %136 = icmp sle i16 %134, %135
  br i1 %136, label %b27, label %b28

b26:
  %137 = load i16, ptr %17, !tbaa !2
  %138 = load i16, ptr %16, !tbaa !2
  %139 = icmp sge i16 %137, %138
  br i1 %139, label %b27, label %b28

b27:
  call cc 1000 addrspace(1) void @KEYACTION()
  %140 = load i16, ptr @"XIT%", !tbaa !2
  %141 = icmp eq i16 %140, 1
  br i1 %141, label %b29, label %b30

b28:
  store i16 1, ptr %17, !tbaa !2
  store i16 640, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b38

b29:
  br label %b3

b30:
  br label %b31

b31:
  %142 = load i16, ptr %17, !tbaa !2
  store i16 %142, ptr %13, !tbaa !2
  %143 = load i16, ptr %13, !tbaa !2
  %144 = sitofp i16 %143 to float
  store i16 16, ptr %12, !tbaa !2
  %145 = load i16, ptr %12, !tbaa !2
  %146 = sitofp i16 %145 to float
  %147 = fdiv float %144, %146
  store i16 31, ptr %11, !tbaa !2
  %148 = load i16, ptr %11, !tbaa !2
  %149 = sitofp i16 %148 to float
  %150 = fsub float %149, %147
  %151 = call i16 @llvm.lrint.i16.f32(float %150)
  store i16 %151, ptr %14, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 320, i16 240)
  %152 = load i16, ptr %17, !tbaa !2
  store i16 %152, ptr %10, !tbaa !2
  %153 = load i16, ptr %10, !tbaa !2
  %154 = sitofp i16 %153 to float
  %155 = fpround float %154 to float
  store float %155, ptr %9, !tbaa !2
  %156 = load i16, ptr %14, !tbaa !2
  %157 = load float, ptr %9, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %157, i16 %156)
  store i32 0, ptr %8, !tbaa !2
  store i32 16383, ptr %7, !tbaa !2
  store i32 1, ptr %6, !tbaa !2
  br label %b33

b33:
  %158 = load i32, ptr %6, !tbaa !2
  %159 = icmp sge i32 %158, 0
  br i1 %159, label %b34, label %b35

b34:
  %160 = load i32, ptr %8, !tbaa !2
  %161 = load i32, ptr %7, !tbaa !2
  %162 = icmp sle i32 %160, %161
  br i1 %162, label %b36, label %b37

b35:
  %163 = load i32, ptr %8, !tbaa !2
  %164 = load i32, ptr %7, !tbaa !2
  %165 = icmp sge i32 %163, %164
  br i1 %165, label %b36, label %b37

b36:
  %166 = load i32, ptr %8, !tbaa !2
  %167 = load i32, ptr %6, !tbaa !2
  %168 = add nsw i32 %166, %167
  store i32 %168, ptr %8, !tbaa !2
  br label %b33

b37:
  %169 = load i16, ptr %17, !tbaa !2
  %170 = load i16, ptr %15, !tbaa !2
  %171 = add nsw i16 %169, %170
  store i16 %171, ptr %17, !tbaa !2
  br label %b24

b38:
  %172 = load i16, ptr %4, !tbaa !2
  %173 = icmp sge i16 %172, 0
  br i1 %173, label %b39, label %b40

b39:
  %174 = load i16, ptr %17, !tbaa !2
  %175 = load i16, ptr %5, !tbaa !2
  %176 = icmp sle i16 %174, %175
  br i1 %176, label %b41, label %b42

b40:
  %177 = load i16, ptr %17, !tbaa !2
  %178 = load i16, ptr %5, !tbaa !2
  %179 = icmp sge i16 %177, %178
  br i1 %179, label %b41, label %b42

b41:
  call cc 1000 addrspace(1) void @KEYACTION()
  %180 = load i16, ptr @"XIT%", !tbaa !2
  %181 = icmp eq i16 %180, 1
  br i1 %181, label %b43, label %b44

b42:
  br label %b3

b43:
  br label %b3

b44:
  br label %b45

b45:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 320, i16 240)
  %182 = load i16, ptr %17, !tbaa !2
  store i16 %182, ptr %3, !tbaa !2
  %183 = load i16, ptr %3, !tbaa !2
  %184 = sitofp i16 %183 to float
  %185 = fpround float %184 to float
  store float %185, ptr %2, !tbaa !2
  %186 = load float, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %186, i16 0)
  store i32 0, ptr %8, !tbaa !2
  store i32 16383, ptr %1, !tbaa !2
  store i32 1, ptr %0, !tbaa !2
  br label %b47

b47:
  %187 = load i32, ptr %0, !tbaa !2
  %188 = icmp sge i32 %187, 0
  br i1 %188, label %b48, label %b49

b48:
  %189 = load i32, ptr %8, !tbaa !2
  %190 = load i32, ptr %1, !tbaa !2
  %191 = icmp sle i32 %189, %190
  br i1 %191, label %b50, label %b51

b49:
  %192 = load i32, ptr %8, !tbaa !2
  %193 = load i32, ptr %1, !tbaa !2
  %194 = icmp sge i32 %192, %193
  br i1 %194, label %b50, label %b51

b50:
  %195 = load i32, ptr %8, !tbaa !2
  %196 = load i32, ptr %0, !tbaa !2
  %197 = add nsw i32 %195, %196
  store i32 %197, ptr %8, !tbaa !2
  br label %b47

b51:
  %198 = load i16, ptr %17, !tbaa !2
  %199 = load i16, ptr %4, !tbaa !2
  %200 = add nsw i16 %198, %199
  store i16 %200, ptr %17, !tbaa !2
  br label %b38
}

define cc 1000 void @RGBLIGHTS() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca float
  %24 = alloca i16
  %25 = alloca float
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i32
  %48 = alloca i32
  %49 = alloca float
  %50 = alloca float
  %51 = alloca float
  %52 = alloca float
  %53 = alloca i16
  %54 = alloca float
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca i16
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca i16
  %79 = alloca i16
  %80 = alloca float
  %81 = load float, ptr @$float237, !tbaa !2
  store float %81, ptr %80, !tbaa !2
  store i16 3, ptr %79, !tbaa !2
  store i16 0, ptr %78, !tbaa !2
  store i16 127, ptr %77, !tbaa !2
  store i16 1, ptr %76, !tbaa !2
  br label %b3

b2:
  ret void

b3:
  %82 = load i16, ptr %76, !tbaa !2
  %83 = icmp sge i16 %82, 0
  br i1 %83, label %b4, label %b5

b4:
  %84 = load i16, ptr %78, !tbaa !2
  %85 = load i16, ptr %77, !tbaa !2
  %86 = icmp sle i16 %84, %85
  br i1 %86, label %b6, label %b7

b5:
  %87 = load i16, ptr %78, !tbaa !2
  %88 = load i16, ptr %77, !tbaa !2
  %89 = icmp sge i16 %87, %88
  br i1 %89, label %b6, label %b7

b6:
  %90 = load i16, ptr %78, !tbaa !2
  %91 = trunc i16 %90 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %91)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %92 = load i16, ptr %78, !tbaa !2
  %93 = load i16, ptr %76, !tbaa !2
  %94 = add nsw i16 %92, %93
  store i16 %94, ptr %78, !tbaa !2
  br label %b3

b7:
  store i16 0, ptr %78, !tbaa !2
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %75, !tbaa !2
  store i16 5, ptr %74, !tbaa !2
  store i16 1, ptr %73, !tbaa !2
  br label %b8

b8:
  %95 = load i16, ptr %73, !tbaa !2
  %96 = icmp sge i16 %95, 0
  br i1 %96, label %b9, label %b10

b9:
  %97 = load i16, ptr %75, !tbaa !2
  %98 = load i16, ptr %74, !tbaa !2
  %99 = icmp sle i16 %97, %98
  br i1 %99, label %b11, label %b12

b10:
  %100 = load i16, ptr %75, !tbaa !2
  %101 = load i16, ptr %74, !tbaa !2
  %102 = icmp sge i16 %100, %101
  br i1 %102, label %b11, label %b12

b11:
  store i16 0, ptr %72, !tbaa !2
  store i16 5, ptr %71, !tbaa !2
  store i16 1, ptr %70, !tbaa !2
  br label %b13

b12:
  store i16 0, ptr %78, !tbaa !2
  store i16 5, ptr %63, !tbaa !2
  store i16 1, ptr %62, !tbaa !2
  br label %b23

b13:
  %103 = load i16, ptr %70, !tbaa !2
  %104 = icmp sge i16 %103, 0
  br i1 %104, label %b14, label %b15

b14:
  %105 = load i16, ptr %72, !tbaa !2
  %106 = load i16, ptr %71, !tbaa !2
  %107 = icmp sle i16 %105, %106
  br i1 %107, label %b16, label %b17

b15:
  %108 = load i16, ptr %72, !tbaa !2
  %109 = load i16, ptr %71, !tbaa !2
  %110 = icmp sge i16 %108, %109
  br i1 %110, label %b16, label %b17

b16:
  store i16 0, ptr %69, !tbaa !2
  store i16 5, ptr %68, !tbaa !2
  store i16 1, ptr %67, !tbaa !2
  br label %b18

b17:
  %111 = load i16, ptr %75, !tbaa !2
  %112 = load i16, ptr %73, !tbaa !2
  %113 = add nsw i16 %111, %112
  store i16 %113, ptr %75, !tbaa !2
  br label %b8

b18:
  %114 = load i16, ptr %67, !tbaa !2
  %115 = icmp sge i16 %114, 0
  br i1 %115, label %b19, label %b20

b19:
  %116 = load i16, ptr %69, !tbaa !2
  %117 = load i16, ptr %68, !tbaa !2
  %118 = icmp sle i16 %116, %117
  br i1 %118, label %b21, label %b22

b20:
  %119 = load i16, ptr %69, !tbaa !2
  %120 = load i16, ptr %68, !tbaa !2
  %121 = icmp sge i16 %119, %120
  br i1 %121, label %b21, label %b22

b21:
  %122 = load i16, ptr %75, !tbaa !2
  %123 = load float, ptr @$float238, !tbaa !2
  store i16 %122, ptr %66, !tbaa !2
  %124 = load i16, ptr %66, !tbaa !2
  %125 = sitofp i16 %124 to float
  %126 = fmul float %125, %123
  %127 = call i16 @llvm.lrint.i16.f32(float %126)
  %128 = trunc i16 %127 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %128)
  %129 = load i16, ptr %72, !tbaa !2
  %130 = load float, ptr @$float238, !tbaa !2
  store i16 %129, ptr %65, !tbaa !2
  %131 = load i16, ptr %65, !tbaa !2
  %132 = sitofp i16 %131 to float
  %133 = fmul float %132, %130
  %134 = call i16 @llvm.lrint.i16.f32(float %133)
  %135 = trunc i16 %134 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %135)
  %136 = load i16, ptr %69, !tbaa !2
  %137 = load float, ptr @$float238, !tbaa !2
  store i16 %136, ptr %64, !tbaa !2
  %138 = load i16, ptr %64, !tbaa !2
  %139 = sitofp i16 %138 to float
  %140 = fmul float %139, %137
  %141 = call i16 @llvm.lrint.i16.f32(float %140)
  %142 = trunc i16 %141 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %142)
  %143 = load i16, ptr %78, !tbaa !2
  %144 = mul i16 %143, 2
  %145 = getelementptr i8, ptr @"R1%", i16 2
  %146 = load i16, ptr %145, !tbaa !2
  %147 = getelementptr i8, ptr @"R1%", i16 10
  %148 = load i16, ptr %147, !tbaa !2
  %149 = add i16 %148, %144
  %150 = inttoptr i16 %146 to ptr addrspace(2)
  %151 = addrspacecast ptr addrspace(2) %150 to ptr addrspace(1)
  %152 = getelementptr i8, ptr addrspace(1) %151, i16 %149
  %153 = load i16, ptr %78, !tbaa !2
  store i16 %153, ptr addrspace(1) %152, !tbaa !72
  %154 = load i16, ptr %78, !tbaa !2
  %155 = add i16 %154, 1
  store i16 %155, ptr %78, !tbaa !2
  %156 = load i16, ptr %69, !tbaa !2
  %157 = load i16, ptr %67, !tbaa !2
  %158 = add nsw i16 %156, %157
  store i16 %158, ptr %69, !tbaa !2
  br label %b18

b22:
  %159 = load i16, ptr %72, !tbaa !2
  %160 = load i16, ptr %70, !tbaa !2
  %161 = add nsw i16 %159, %160
  store i16 %161, ptr %72, !tbaa !2
  br label %b13

b23:
  %162 = load i16, ptr %62, !tbaa !2
  %163 = icmp sge i16 %162, 0
  br i1 %163, label %b24, label %b25

b24:
  %164 = load i16, ptr %78, !tbaa !2
  %165 = load i16, ptr %63, !tbaa !2
  %166 = icmp sle i16 %164, %165
  br i1 %166, label %b26, label %b27

b25:
  %167 = load i16, ptr %78, !tbaa !2
  %168 = load i16, ptr %63, !tbaa !2
  %169 = icmp sge i16 %167, %168
  br i1 %169, label %b26, label %b27

b26:
  %170 = load i16, ptr %78, !tbaa !2
  %171 = load i16, ptr %78, !tbaa !2
  %172 = mul i16 %171, 6
  %173 = sub i16 %170, 0
  %174 = getelementptr inbounds i16, ptr @"EPI6%", i16 %173
  store i16 %172, ptr %174, !tbaa !2
  %175 = load i16, ptr %78, !tbaa !2
  %176 = load i16, ptr %78, !tbaa !2
  %177 = mul i16 %176, 36
  %178 = sub i16 %175, 0
  %179 = getelementptr inbounds i16, ptr @"EPI36%", i16 %178
  store i16 %177, ptr %179, !tbaa !2
  %180 = load i16, ptr %78, !tbaa !2
  %181 = load i16, ptr %62, !tbaa !2
  %182 = add nsw i16 %180, %181
  store i16 %182, ptr %78, !tbaa !2
  br label %b23

b27:
  store i16 0, ptr %78, !tbaa !2
  store i16 0, ptr %61, !tbaa !2
  store i16 99, ptr %60, !tbaa !2
  store i16 1, ptr %59, !tbaa !2
  br label %b28

b28:
  %183 = load i16, ptr %59, !tbaa !2
  %184 = icmp sge i16 %183, 0
  br i1 %184, label %b29, label %b30

b29:
  %185 = load i16, ptr %61, !tbaa !2
  %186 = load i16, ptr %60, !tbaa !2
  %187 = icmp sle i16 %185, %186
  br i1 %187, label %b31, label %b32

b30:
  %188 = load i16, ptr %61, !tbaa !2
  %189 = load i16, ptr %60, !tbaa !2
  %190 = icmp sge i16 %188, %189
  br i1 %190, label %b31, label %b32

b31:
  store i16 0, ptr %58, !tbaa !2
  store i16 159, ptr %57, !tbaa !2
  store i16 1, ptr %56, !tbaa !2
  br label %b33

b32:
  store i16 -320, ptr %78, !tbaa !2
  store i16 639, ptr %46, !tbaa !2
  store i16 1, ptr %45, !tbaa !2
  br label %b61

b33:
  %191 = load i16, ptr %56, !tbaa !2
  %192 = icmp sge i16 %191, 0
  br i1 %192, label %b34, label %b35

b34:
  %193 = load i16, ptr %58, !tbaa !2
  %194 = load i16, ptr %57, !tbaa !2
  %195 = icmp sle i16 %193, %194
  br i1 %195, label %b36, label %b37

b35:
  %196 = load i16, ptr %58, !tbaa !2
  %197 = load i16, ptr %57, !tbaa !2
  %198 = icmp sge i16 %196, %197
  br i1 %198, label %b36, label %b37

b36:
  %199 = load i16, ptr %58, !tbaa !2
  %200 = icmp eq i16 %199, 159
  %201 = sext i1 %200 to i16
  %202 = load i16, ptr %61, !tbaa !2
  %203 = icmp eq i16 %202, 99
  %204 = sext i1 %203 to i16
  %205 = and i16 %201, %204
  %206 = icmp ne i16 %205, 0
  br i1 %206, label %b38, label %b39

b37:
  %207 = load i16, ptr %61, !tbaa !2
  %208 = load i16, ptr %59, !tbaa !2
  %209 = add nsw i16 %207, %208
  store i16 %209, ptr %61, !tbaa !2
  br label %b28

b38:
  %210 = load i16, ptr %78, !tbaa !2
  %211 = sub i16 %210, 0
  %212 = getelementptr inbounds i16, ptr @"SP%", i16 %211
  store i16 255, ptr %212, !tbaa !2
  br label %b40

b39:
  %213 = load i16, ptr %78, !tbaa !2
  %214 = load i16, ptr %58, !tbaa !2
  %215 = sub i16 159, %214
  store i16 %215, ptr %55, !tbaa !2
  %216 = load i16, ptr %55, !tbaa !2
  %217 = sitofp i16 %216 to float
  %218 = fpround float %217 to float
  store float %218, ptr %54, !tbaa !2
  %219 = load float, ptr %54, !tbaa !2
  %220 = load float, ptr %54, !tbaa !2
  %221 = fmul float %219, %220
  %222 = load i16, ptr %61, !tbaa !2
  %223 = sub i16 99, %222
  store i16 %223, ptr %53, !tbaa !2
  %224 = load i16, ptr %53, !tbaa !2
  %225 = sitofp i16 %224 to float
  %226 = fpround float %225 to float
  store float %226, ptr %52, !tbaa !2
  %227 = load float, ptr %52, !tbaa !2
  %228 = load float, ptr %52, !tbaa !2
  %229 = fmul float %227, %228
  %230 = fadd float %221, %229
  %231 = call float @llvm.sqrt.f32(float %230)
  %232 = load float, ptr %80, !tbaa !2
  %233 = fpround float %231 to float
  store float %233, ptr %51, !tbaa !2
  store float %232, ptr %50, !tbaa !2
  %234 = load float, ptr %51, !tbaa !2
  %235 = load float, ptr @$float218, !tbaa !2
  %236 = fcmp ogt float %234, %235
  br i1 %236, label %b41, label %b42

b40:
  %237 = load i16, ptr %78, !tbaa !2
  %238 = sub i16 %237, 0
  %239 = getelementptr inbounds i16, ptr @"SP%", i16 %238
  %240 = load i16, ptr %239, !tbaa !2
  %241 = icmp sgt i16 %240, 255
  br i1 %241, label %b58, label %b59

b41:
  %242 = load float, ptr %51, !tbaa !2
  %243 = call float @llvm.log2.f32(float %242)
  %244 = load float, ptr %50, !tbaa !2
  %245 = fmul float %243, %244
  %246 = load float, ptr @$float219, !tbaa !2
  %247 = fcmp oge float %245, %246
  br i1 %247, label %b50, label %b51

b42:
  %248 = load float, ptr %51, !tbaa !2
  %249 = load float, ptr @$float218, !tbaa !2
  %250 = fcmp oeq float %248, %249
  br i1 %250, label %b43, label %b45

b43:
  %251 = load float, ptr %50, !tbaa !2
  %252 = load float, ptr @$float218, !tbaa !2
  %253 = fcmp ogt float %251, %252
  br i1 %253, label %b52, label %b44

b44:
  %254 = load float, ptr %50, !tbaa !2
  %255 = load float, ptr @$float218, !tbaa !2
  %256 = fcmp oeq float %254, %255
  br i1 %256, label %b53, label %b48

b45:
  %257 = load float, ptr %50, !tbaa !2
  %258 = call i32 @llvm.lrint.i32.f32(float %257)
  store i32 %258, ptr %48, !tbaa !2
  %259 = load i32, ptr %48, !tbaa !2
  store i32 %259, ptr %47, !tbaa !2
  %260 = load i32, ptr %47, !tbaa !2
  %261 = sitofp i32 %260 to float
  %262 = load float, ptr %50, !tbaa !2
  %263 = fcmp oeq float %261, %262
  br i1 %263, label %b46, label %b48

b46:
  %264 = load float, ptr %51, !tbaa !2
  %265 = fneg float %264
  %266 = call float @llvm.log2.f32(float %265)
  %267 = load float, ptr %50, !tbaa !2
  %268 = fmul float %266, %267
  %269 = load float, ptr @$float219, !tbaa !2
  %270 = fcmp oge float %268, %269
  br i1 %270, label %b54, label %b55

b47:
  %271 = load float, ptr %49, !tbaa !2
  %272 = fneg float %271
  %273 = fpround float %272 to float
  store float %273, ptr %49, !tbaa !2
  br label %b49

b48:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b49:
  %274 = load float, ptr %49, !tbaa !2
  %275 = call i32 @llvm.lrint.i32.f32(float %274)
  %276 = icmp eq i32 %275, 0
  br i1 %276, label %b56, label %b57

b50:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b51:
  %277 = call float @llvm.exp2.f32(float %245)
  %278 = fpround float %277 to float
  store float %278, ptr %49, !tbaa !2
  br label %b49

b52:
  %279 = load float, ptr @$float218, !tbaa !2
  store float %279, ptr %49, !tbaa !2
  br label %b49

b53:
  %280 = load float, ptr @$float220, !tbaa !2
  store float %280, ptr %49, !tbaa !2
  br label %b49

b54:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b55:
  %281 = call float @llvm.exp2.f32(float %268)
  %282 = fpround float %281 to float
  store float %282, ptr %49, !tbaa !2
  %283 = load i32, ptr %48, !tbaa !2
  %284 = and i32 %283, 1
  %285 = icmp ne i32 %284, 0
  br i1 %285, label %b47, label %b49

b56:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b57:
  %286 = sdiv i32 16384, %275
  %287 = trunc i32 %286 to i16
  %288 = sub i16 %213, 0
  %289 = getelementptr inbounds i16, ptr @"SP%", i16 %288
  store i16 %287, ptr %289, !tbaa !2
  br label %b40

b58:
  %290 = load i16, ptr %78, !tbaa !2
  %291 = sub i16 %290, 0
  %292 = getelementptr inbounds i16, ptr @"SP%", i16 %291
  store i16 255, ptr %292, !tbaa !2
  br label %b60

b59:
  br label %b60

b60:
  %293 = load i16, ptr %78, !tbaa !2
  %294 = add i16 %293, 1
  store i16 %294, ptr %78, !tbaa !2
  %295 = load i16, ptr %58, !tbaa !2
  %296 = load i16, ptr %56, !tbaa !2
  %297 = add nsw i16 %295, %296
  store i16 %297, ptr %58, !tbaa !2
  br label %b33

b61:
  %298 = load i16, ptr %45, !tbaa !2
  %299 = icmp sge i16 %298, 0
  br i1 %299, label %b62, label %b63

b62:
  %300 = load i16, ptr %78, !tbaa !2
  %301 = load i16, ptr %46, !tbaa !2
  %302 = icmp sle i16 %300, %301
  br i1 %302, label %b64, label %b65

b63:
  %303 = load i16, ptr %78, !tbaa !2
  %304 = load i16, ptr %46, !tbaa !2
  %305 = icmp sge i16 %303, %304
  br i1 %305, label %b64, label %b65

b64:
  %306 = load i16, ptr %78, !tbaa !2
  %307 = icmp slt i16 %306, 0
  %308 = sext i1 %307 to i16
  %309 = load i16, ptr %78, !tbaa !2
  %310 = icmp sgt i16 %309, 319
  %311 = sext i1 %310 to i16
  %312 = or i16 %308, %311
  %313 = icmp ne i16 %312, 0
  br i1 %313, label %b66, label %b67

b65:
  store i16 -300, ptr %78, !tbaa !2
  store i16 500, ptr %44, !tbaa !2
  store i16 1, ptr %43, !tbaa !2
  br label %b72

b66:
  %314 = load i16, ptr %78, !tbaa !2
  %315 = sub i16 %314, -640
  %316 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %315
  store i16 0, ptr %316, !tbaa !2
  br label %b68

b67:
  %317 = load i16, ptr %78, !tbaa !2
  %318 = icmp slt i16 %317, 160
  br i1 %318, label %b69, label %b70

b68:
  %319 = load i16, ptr %78, !tbaa !2
  %320 = load i16, ptr %45, !tbaa !2
  %321 = add nsw i16 %319, %320
  store i16 %321, ptr %78, !tbaa !2
  br label %b61

b69:
  %322 = load i16, ptr %78, !tbaa !2
  %323 = load i16, ptr %78, !tbaa !2
  %324 = sub i16 %322, -640
  %325 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %324
  store i16 %323, ptr %325, !tbaa !2
  br label %b71

b70:
  %326 = load i16, ptr %78, !tbaa !2
  %327 = load i16, ptr %78, !tbaa !2
  %328 = sub i16 319, %327
  %329 = sub i16 %326, -640
  %330 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %329
  store i16 %328, ptr %330, !tbaa !2
  br label %b71

b71:
  br label %b68

b72:
  %331 = load i16, ptr %43, !tbaa !2
  %332 = icmp sge i16 %331, 0
  br i1 %332, label %b73, label %b74

b73:
  %333 = load i16, ptr %78, !tbaa !2
  %334 = load i16, ptr %44, !tbaa !2
  %335 = icmp sle i16 %333, %334
  br i1 %335, label %b75, label %b76

b74:
  %336 = load i16, ptr %78, !tbaa !2
  %337 = load i16, ptr %44, !tbaa !2
  %338 = icmp sge i16 %336, %337
  br i1 %338, label %b75, label %b76

b75:
  %339 = load i16, ptr %78, !tbaa !2
  %340 = icmp slt i16 %339, 0
  %341 = sext i1 %340 to i16
  %342 = load i16, ptr %78, !tbaa !2
  %343 = icmp sgt i16 %342, 199
  %344 = sext i1 %343 to i16
  %345 = or i16 %341, %344
  %346 = icmp ne i16 %345, 0
  br i1 %346, label %b77, label %b78

b76:
  store i16 0, ptr %78, !tbaa !2
  store i16 1792, ptr %42, !tbaa !2
  store i16 1, ptr %41, !tbaa !2
  br label %b83

b77:
  %347 = load i16, ptr %78, !tbaa !2
  %348 = sub i16 %347, -319
  %349 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %348
  store i16 0, ptr %349, !tbaa !2
  br label %b79

b78:
  %350 = load i16, ptr %78, !tbaa !2
  %351 = icmp sgt i16 %350, 99
  br i1 %351, label %b80, label %b81

b79:
  %352 = load i16, ptr %78, !tbaa !2
  %353 = load i16, ptr %78, !tbaa !2
  %354 = sub i16 %353, -319
  %355 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %354
  %356 = load i16, ptr %355, !tbaa !2
  %357 = mul i16 %356, 160
  %358 = sub i16 %352, -319
  %359 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %358
  store i16 %357, ptr %359, !tbaa !2
  %360 = load i16, ptr %78, !tbaa !2
  %361 = load i16, ptr %43, !tbaa !2
  %362 = add nsw i16 %360, %361
  store i16 %362, ptr %78, !tbaa !2
  br label %b72

b80:
  %363 = load i16, ptr %78, !tbaa !2
  %364 = load i16, ptr %78, !tbaa !2
  %365 = sub i16 199, %364
  %366 = sub i16 %363, -319
  %367 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %366
  store i16 %365, ptr %367, !tbaa !2
  br label %b82

b81:
  %368 = load i16, ptr %78, !tbaa !2
  %369 = load i16, ptr %78, !tbaa !2
  %370 = sub i16 %368, -319
  %371 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %370
  store i16 %369, ptr %371, !tbaa !2
  br label %b82

b82:
  br label %b79

b83:
  %372 = load i16, ptr %41, !tbaa !2
  %373 = icmp sge i16 %372, 0
  br i1 %373, label %b84, label %b85

b84:
  %374 = load i16, ptr %78, !tbaa !2
  %375 = load i16, ptr %42, !tbaa !2
  %376 = icmp sle i16 %374, %375
  br i1 %376, label %b86, label %b87

b85:
  %377 = load i16, ptr %78, !tbaa !2
  %378 = load i16, ptr %42, !tbaa !2
  %379 = icmp sge i16 %377, %378
  br i1 %379, label %b86, label %b87

b86:
  %380 = load i16, ptr %78, !tbaa !2
  %381 = icmp sgt i16 %380, 5
  br i1 %381, label %b88, label %b89

b87:
  store i16 1, ptr %78, !tbaa !2
  %382 = load i16, ptr %79, !tbaa !2
  store i16 %382, ptr %40, !tbaa !2
  store i16 1, ptr %39, !tbaa !2
  br label %b91

b88:
  %383 = load i16, ptr %78, !tbaa !2
  %384 = sub i16 %383, 0
  %385 = getelementptr inbounds i16, ptr @"CD%", i16 %384
  store i16 5, ptr %385, !tbaa !2
  br label %b90

b89:
  %386 = load i16, ptr %78, !tbaa !2
  %387 = load i16, ptr %78, !tbaa !2
  %388 = sub i16 %386, 0
  %389 = getelementptr inbounds i16, ptr @"CD%", i16 %388
  store i16 %387, ptr %389, !tbaa !2
  br label %b90

b90:
  %390 = load i16, ptr %78, !tbaa !2
  %391 = load i16, ptr %41, !tbaa !2
  %392 = add nsw i16 %390, %391
  store i16 %392, ptr %78, !tbaa !2
  br label %b83

b91:
  %393 = load i16, ptr %39, !tbaa !2
  %394 = icmp sge i16 %393, 0
  br i1 %394, label %b92, label %b93

b92:
  %395 = load i16, ptr %78, !tbaa !2
  %396 = load i16, ptr %40, !tbaa !2
  %397 = icmp sle i16 %395, %396
  br i1 %397, label %b94, label %b95

b93:
  %398 = load i16, ptr %78, !tbaa !2
  %399 = load i16, ptr %40, !tbaa !2
  %400 = icmp sge i16 %398, %399
  br i1 %400, label %b94, label %b95

b94:
  %401 = load i16, ptr %78, !tbaa !2
  %402 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %403 = load float, ptr %402
  store i16 35, ptr %38, !tbaa !2
  %404 = load i16, ptr %38, !tbaa !2
  %405 = sitofp i16 %404 to float
  %406 = fmul float %403, %405
  store i16 15, ptr %37, !tbaa !2
  %407 = load i16, ptr %37, !tbaa !2
  %408 = sitofp i16 %407 to float
  %409 = fadd float %406, %408
  %410 = call float @llvm.rint.f32(float %409)
  %411 = fcmp olt float %409, %410
  %412 = sext i1 %411 to i16
  store i16 %412, ptr %36, !tbaa !2
  %413 = load i16, ptr %36, !tbaa !2
  %414 = sitofp i16 %413 to float
  %415 = fadd float %410, %414
  %416 = call i16 @llvm.lrint.i16.f32(float %415)
  %417 = sub i16 %401, 0
  %418 = getelementptr inbounds i16, ptr @"DSX%", i16 %417
  store i16 %416, ptr %418, !tbaa !2
  %419 = load i16, ptr %78, !tbaa !2
  %420 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %421 = load float, ptr %420
  store i16 35, ptr %35, !tbaa !2
  %422 = load i16, ptr %35, !tbaa !2
  %423 = sitofp i16 %422 to float
  %424 = fmul float %421, %423
  store i16 15, ptr %34, !tbaa !2
  %425 = load i16, ptr %34, !tbaa !2
  %426 = sitofp i16 %425 to float
  %427 = fadd float %424, %426
  %428 = call float @llvm.rint.f32(float %427)
  %429 = fcmp olt float %427, %428
  %430 = sext i1 %429 to i16
  store i16 %430, ptr %33, !tbaa !2
  %431 = load i16, ptr %33, !tbaa !2
  %432 = sitofp i16 %431 to float
  %433 = fadd float %428, %432
  %434 = call i16 @llvm.lrint.i16.f32(float %433)
  %435 = sub i16 %419, 0
  %436 = getelementptr inbounds i16, ptr @"DSY%", i16 %435
  store i16 %434, ptr %436, !tbaa !2
  %437 = load i16, ptr %78, !tbaa !2
  %438 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %439 = load float, ptr %438
  store i16 80, ptr %32, !tbaa !2
  %440 = load i16, ptr %32, !tbaa !2
  %441 = sitofp i16 %440 to float
  %442 = fmul float %439, %441
  store i16 80, ptr %31, !tbaa !2
  %443 = load i16, ptr %31, !tbaa !2
  %444 = sitofp i16 %443 to float
  %445 = fadd float %442, %444
  %446 = call float @llvm.rint.f32(float %445)
  %447 = fcmp olt float %445, %446
  %448 = sext i1 %447 to i16
  store i16 %448, ptr %30, !tbaa !2
  %449 = load i16, ptr %30, !tbaa !2
  %450 = sitofp i16 %449 to float
  %451 = fadd float %446, %450
  %452 = call i16 @llvm.lrint.i16.f32(float %451)
  %453 = sub i16 %437, 0
  %454 = getelementptr inbounds i16, ptr @"PSX%", i16 %453
  store i16 %452, ptr %454, !tbaa !2
  %455 = load i16, ptr %78, !tbaa !2
  %456 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %457 = load float, ptr %456
  store i16 50, ptr %29, !tbaa !2
  %458 = load i16, ptr %29, !tbaa !2
  %459 = sitofp i16 %458 to float
  %460 = fmul float %457, %459
  store i16 50, ptr %28, !tbaa !2
  %461 = load i16, ptr %28, !tbaa !2
  %462 = sitofp i16 %461 to float
  %463 = fadd float %460, %462
  %464 = call float @llvm.rint.f32(float %463)
  %465 = fcmp olt float %463, %464
  %466 = sext i1 %465 to i16
  store i16 %466, ptr %27, !tbaa !2
  %467 = load i16, ptr %27, !tbaa !2
  %468 = sitofp i16 %467 to float
  %469 = fadd float %464, %468
  %470 = call i16 @llvm.lrint.i16.f32(float %469)
  %471 = sub i16 %455, 0
  %472 = getelementptr inbounds i16, ptr @"PSY%", i16 %471
  store i16 %470, ptr %472, !tbaa !2
  %473 = load i16, ptr %78, !tbaa !2
  %474 = load i16, ptr %39, !tbaa !2
  %475 = add nsw i16 %473, %474
  store i16 %475, ptr %78, !tbaa !2
  br label %b91

b95:
  store i16 40, ptr %26, !tbaa !2
  store i16 8, ptr %24, !tbaa !2
  %476 = load i16, ptr %24, !tbaa !2
  %477 = sitofp i16 %476 to float
  %478 = fpround float %477 to float
  store float %478, ptr %25, !tbaa !2
  store i16 8, ptr %22, !tbaa !2
  %479 = load i16, ptr %22, !tbaa !2
  %480 = sitofp i16 %479 to float
  %481 = fpround float %480 to float
  store float %481, ptr %23, !tbaa !2
  br label %b97

b97:
  %482 = load i16, ptr %26, !tbaa !2
  %483 = add i16 %482, 1
  store i16 %483, ptr %26, !tbaa !2
  %484 = load float, ptr %25, !tbaa !2
  store i16 1, ptr %21, !tbaa !2
  %485 = load i16, ptr %21, !tbaa !2
  %486 = sitofp i16 %485 to float
  %487 = fcmp ogt float %484, %486
  %488 = sext i1 %487 to i16
  %489 = load i16, ptr %26, !tbaa !2
  %490 = icmp slt i16 %489, 102
  %491 = sext i1 %490 to i16
  %492 = and i16 %488, %491
  %493 = icmp ne i16 %492, 0
  br i1 %493, label %b99, label %b100

b99:
  %494 = load float, ptr %25, !tbaa !2
  %495 = load float, ptr @$float159, !tbaa !2
  %496 = fsub float %494, %495
  %497 = fpround float %496 to float
  store float %497, ptr %25, !tbaa !2
  br label %b101

b100:
  br label %b101

b101:
  %498 = load float, ptr %23, !tbaa !2
  store i16 1, ptr %20, !tbaa !2
  %499 = load i16, ptr %20, !tbaa !2
  %500 = sitofp i16 %499 to float
  %501 = fcmp ogt float %498, %500
  %502 = sext i1 %501 to i16
  %503 = load i16, ptr %26, !tbaa !2
  %504 = icmp slt i16 %503, 102
  %505 = sext i1 %504 to i16
  %506 = and i16 %502, %505
  %507 = icmp ne i16 %506, 0
  br i1 %507, label %b102, label %b103

b102:
  %508 = load float, ptr %23, !tbaa !2
  %509 = load float, ptr @$float159, !tbaa !2
  %510 = fsub float %508, %509
  %511 = fpround float %510 to float
  store float %511, ptr %23, !tbaa !2
  br label %b104

b103:
  br label %b104

b104:
  %512 = load i16, ptr %26, !tbaa !2
  %513 = icmp sgt i16 %512, 102
  br i1 %513, label %b105, label %b106

b105:
  %514 = load float, ptr %25, !tbaa !2
  %515 = load float, ptr @$float147, !tbaa !2
  %516 = fadd float %514, %515
  %517 = fpround float %516 to float
  store float %517, ptr %25, !tbaa !2
  %518 = load float, ptr %23, !tbaa !2
  %519 = load float, ptr @$float147, !tbaa !2
  %520 = fadd float %518, %519
  %521 = fpround float %520 to float
  store float %521, ptr %23, !tbaa !2
  br label %b107

b106:
  br label %b107

b107:
  %522 = load i16, ptr %26, !tbaa !2
  %523 = icmp eq i16 %522, 122
  br i1 %523, label %b108, label %b109

b108:
  br label %b2

b109:
  br label %b110

b110:
  store i16 1, ptr %78, !tbaa !2
  %524 = load i16, ptr %79, !tbaa !2
  store i16 %524, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b112

b112:
  %525 = load i16, ptr %18, !tbaa !2
  %526 = icmp sge i16 %525, 0
  br i1 %526, label %b113, label %b114

b113:
  %527 = load i16, ptr %78, !tbaa !2
  %528 = load i16, ptr %19, !tbaa !2
  %529 = icmp sle i16 %527, %528
  br i1 %529, label %b115, label %b116

b114:
  %530 = load i16, ptr %78, !tbaa !2
  %531 = load i16, ptr %19, !tbaa !2
  %532 = icmp sge i16 %530, %531
  br i1 %532, label %b115, label %b116

b115:
  %533 = load i16, ptr %78, !tbaa !2
  %534 = sub i16 %533, 0
  %535 = getelementptr inbounds i16, ptr @"PSX%", i16 %534
  %536 = load i16, ptr %535, !tbaa !2
  %537 = load float, ptr %25, !tbaa !2
  store i16 %536, ptr %16, !tbaa !2
  %538 = load i16, ptr %16, !tbaa !2
  %539 = sitofp i16 %538 to float
  %540 = fmul float %539, %537
  %541 = call i16 @llvm.lrint.i16.f32(float %540)
  store i16 %541, ptr %17, !tbaa !2
  %542 = load i16, ptr %78, !tbaa !2
  %543 = sub i16 %542, 0
  %544 = getelementptr inbounds i16, ptr @"PSY%", i16 %543
  %545 = load i16, ptr %544, !tbaa !2
  %546 = load float, ptr %23, !tbaa !2
  store i16 %545, ptr %14, !tbaa !2
  %547 = load i16, ptr %14, !tbaa !2
  %548 = sitofp i16 %547 to float
  %549 = fmul float %548, %546
  %550 = call i16 @llvm.lrint.i16.f32(float %549)
  store i16 %550, ptr %15, !tbaa !2
  %551 = load i16, ptr %17, !tbaa !2
  %552 = icmp sgt i16 %551, 320
  br i1 %552, label %b117, label %b118

b116:
  store i16 0, ptr %7, !tbaa !2
  store i16 0, ptr %61, !tbaa !2
  store i16 199, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b127

b117:
  store i16 320, ptr %17, !tbaa !2
  br label %b119

b118:
  br label %b119

b119:
  %553 = load i16, ptr %15, !tbaa !2
  %554 = icmp sgt i16 %553, 300
  br i1 %554, label %b120, label %b121

b120:
  store i16 300, ptr %15, !tbaa !2
  br label %b122

b121:
  br label %b122

b122:
  %555 = load i16, ptr %78, !tbaa !2
  %556 = load i16, ptr %26, !tbaa !2
  %557 = load i16, ptr %78, !tbaa !2
  %558 = sub i16 %557, 0
  %559 = getelementptr inbounds i16, ptr @"DSX%", i16 %558
  %560 = load i16, ptr %559, !tbaa !2
  store i16 %556, ptr %13, !tbaa !2
  %561 = load i16, ptr %13, !tbaa !2
  %562 = sitofp i16 %561 to float
  store i16 %560, ptr %12, !tbaa !2
  %563 = load i16, ptr %12, !tbaa !2
  %564 = sitofp i16 %563 to float
  %565 = fdiv float %562, %564
  %566 = call float @llvm.fabs.f32(float %565)
  %567 = load float, ptr @$float86, !tbaa !2
  %568 = fcmp ogt float %566, %567
  br i1 %568, label %b123, label %b124

b123:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b124:
  %569 = call float @llvm.sin.f32(float %565)
  %570 = load i16, ptr %17, !tbaa !2
  store i16 %570, ptr %11, !tbaa !2
  %571 = load i16, ptr %11, !tbaa !2
  %572 = sitofp i16 %571 to float
  %573 = fmul float %569, %572
  %574 = call i16 @llvm.lrint.i16.f32(float %573)
  %575 = sub i16 %555, 0
  %576 = getelementptr inbounds i16, ptr @"XP1%", i16 %575
  store i16 %574, ptr %576, !tbaa !2
  %577 = load i16, ptr %78, !tbaa !2
  %578 = load i16, ptr %26, !tbaa !2
  %579 = load i16, ptr %78, !tbaa !2
  %580 = sub i16 %579, 0
  %581 = getelementptr inbounds i16, ptr @"DSY%", i16 %580
  %582 = load i16, ptr %581, !tbaa !2
  store i16 %578, ptr %10, !tbaa !2
  %583 = load i16, ptr %10, !tbaa !2
  %584 = sitofp i16 %583 to float
  store i16 %582, ptr %9, !tbaa !2
  %585 = load i16, ptr %9, !tbaa !2
  %586 = sitofp i16 %585 to float
  %587 = fdiv float %584, %586
  %588 = call float @llvm.fabs.f32(float %587)
  %589 = load float, ptr @$float86, !tbaa !2
  %590 = fcmp ogt float %588, %589
  br i1 %590, label %b125, label %b126

b125:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b126:
  %591 = call float @llvm.sin.f32(float %587)
  %592 = load i16, ptr %15, !tbaa !2
  store i16 %592, ptr %8, !tbaa !2
  %593 = load i16, ptr %8, !tbaa !2
  %594 = sitofp i16 %593 to float
  %595 = fmul float %591, %594
  %596 = call i16 @llvm.lrint.i16.f32(float %595)
  %597 = sub i16 %577, 0
  %598 = getelementptr inbounds i16, ptr @"YP1%", i16 %597
  store i16 %596, ptr %598, !tbaa !2
  %599 = load i16, ptr %78, !tbaa !2
  %600 = load i16, ptr %18, !tbaa !2
  %601 = add nsw i16 %599, %600
  store i16 %601, ptr %78, !tbaa !2
  br label %b112

b127:
  %602 = load i16, ptr %5, !tbaa !2
  %603 = icmp sge i16 %602, 0
  br i1 %603, label %b128, label %b129

b128:
  %604 = load i16, ptr %61, !tbaa !2
  %605 = load i16, ptr %6, !tbaa !2
  %606 = icmp sle i16 %604, %605
  br i1 %606, label %b130, label %b131

b129:
  %607 = load i16, ptr %61, !tbaa !2
  %608 = load i16, ptr %6, !tbaa !2
  %609 = icmp sge i16 %607, %608
  br i1 %609, label %b130, label %b131

b130:
  %610 = load i16, ptr %7, !tbaa !2
  %611 = add i16 -24576, %610
  store i16 %611, ptr @b$seg, !tbaa !2
  %612 = load i16, ptr %7, !tbaa !2
  %613 = add i16 %612, 20
  store i16 %613, ptr %7, !tbaa !2
  %614 = load i16, ptr %61, !tbaa !2
  %615 = getelementptr inbounds i16, ptr @"YP1%", i16 1
  %616 = load i16, ptr %615, !tbaa !2
  %617 = sub i16 %614, %616
  %618 = sub i16 %617, -319
  %619 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %618
  %620 = load i16, ptr %619, !tbaa !2
  %621 = getelementptr inbounds i16, ptr @"YY%", i16 1
  store i16 %620, ptr %621, !tbaa !2
  %622 = load i16, ptr %61, !tbaa !2
  %623 = getelementptr inbounds i16, ptr @"YP1%", i16 2
  %624 = load i16, ptr %623, !tbaa !2
  %625 = sub i16 %622, %624
  %626 = sub i16 %625, -319
  %627 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %626
  %628 = load i16, ptr %627, !tbaa !2
  %629 = getelementptr inbounds i16, ptr @"YY%", i16 2
  store i16 %628, ptr %629, !tbaa !2
  %630 = load i16, ptr %61, !tbaa !2
  %631 = getelementptr inbounds i16, ptr @"YP1%", i16 3
  %632 = load i16, ptr %631, !tbaa !2
  %633 = sub i16 %630, %632
  %634 = sub i16 %633, -319
  %635 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %634
  %636 = load i16, ptr %635, !tbaa !2
  %637 = getelementptr inbounds i16, ptr @"YY%", i16 3
  store i16 %636, ptr %637, !tbaa !2
  store i16 0, ptr %58, !tbaa !2
  store i16 319, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b132

b131:
  %638 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %638, ptr @"BENCHFRAME&", !tbaa !2
  %639 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %639, ptr @"BENCHFRAME&", !tbaa !2
  %640 = load i16, ptr @"XIT%", !tbaa !2
  %641 = icmp eq i16 %640, 0
  br i1 %641, label %b137, label %b138

b132:
  %642 = load i16, ptr %3, !tbaa !2
  %643 = icmp sge i16 %642, 0
  br i1 %643, label %b133, label %b134

b133:
  %644 = load i16, ptr %58, !tbaa !2
  %645 = load i16, ptr %4, !tbaa !2
  %646 = icmp sle i16 %644, %645
  br i1 %646, label %b135, label %b136

b134:
  %647 = load i16, ptr %58, !tbaa !2
  %648 = load i16, ptr %4, !tbaa !2
  %649 = icmp sge i16 %647, %648
  br i1 %649, label %b135, label %b136

b135:
  %650 = getelementptr inbounds i16, ptr @"YY%", i16 1
  %651 = load i16, ptr %650, !tbaa !2
  %652 = load i16, ptr %58, !tbaa !2
  %653 = getelementptr inbounds i16, ptr @"XP1%", i16 1
  %654 = load i16, ptr %653, !tbaa !2
  %655 = sub i16 %652, %654
  %656 = sub i16 %655, -640
  %657 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %656
  %658 = load i16, ptr %657, !tbaa !2
  %659 = add i16 %651, %658
  %660 = sub i16 %659, 0
  %661 = getelementptr inbounds i16, ptr @"SP%", i16 %660
  %662 = load i16, ptr %661, !tbaa !2
  %663 = sub i16 %662, 0
  %664 = getelementptr inbounds i16, ptr @"CD%", i16 %663
  %665 = load i16, ptr %664, !tbaa !2
  store i16 %665, ptr %2, !tbaa !2
  %666 = getelementptr inbounds i16, ptr @"YY%", i16 2
  %667 = load i16, ptr %666, !tbaa !2
  %668 = load i16, ptr %58, !tbaa !2
  %669 = getelementptr inbounds i16, ptr @"XP1%", i16 2
  %670 = load i16, ptr %669, !tbaa !2
  %671 = sub i16 %668, %670
  %672 = sub i16 %671, -640
  %673 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %672
  %674 = load i16, ptr %673, !tbaa !2
  %675 = add i16 %667, %674
  %676 = sub i16 %675, 0
  %677 = getelementptr inbounds i16, ptr @"SP%", i16 %676
  %678 = load i16, ptr %677, !tbaa !2
  %679 = sub i16 %678, 0
  %680 = getelementptr inbounds i16, ptr @"CD%", i16 %679
  %681 = load i16, ptr %680, !tbaa !2
  store i16 %681, ptr %1, !tbaa !2
  %682 = getelementptr inbounds i16, ptr @"YY%", i16 3
  %683 = load i16, ptr %682, !tbaa !2
  %684 = load i16, ptr %58, !tbaa !2
  %685 = getelementptr inbounds i16, ptr @"XP1%", i16 3
  %686 = load i16, ptr %685, !tbaa !2
  %687 = sub i16 %684, %686
  %688 = sub i16 %687, -640
  %689 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %688
  %690 = load i16, ptr %689, !tbaa !2
  %691 = add i16 %683, %690
  %692 = sub i16 %691, 0
  %693 = getelementptr inbounds i16, ptr @"SP%", i16 %692
  %694 = load i16, ptr %693, !tbaa !2
  %695 = sub i16 %694, 0
  %696 = getelementptr inbounds i16, ptr @"CD%", i16 %695
  %697 = load i16, ptr %696, !tbaa !2
  store i16 %697, ptr %0, !tbaa !2
  %698 = load i16, ptr %58, !tbaa !2
  %699 = load i16, ptr %2, !tbaa !2
  %700 = load i16, ptr %1, !tbaa !2
  %701 = sub i16 %700, 0
  %702 = getelementptr inbounds i16, ptr @"EPI6%", i16 %701
  %703 = load i16, ptr %702, !tbaa !2
  %704 = add i16 %699, %703
  %705 = load i16, ptr %0, !tbaa !2
  %706 = sub i16 %705, 0
  %707 = getelementptr inbounds i16, ptr @"EPI36%", i16 %706
  %708 = load i16, ptr %707, !tbaa !2
  %709 = add i16 %704, %708
  %710 = mul i16 %709, 2
  %711 = getelementptr i8, ptr @"R1%", i16 2
  %712 = load i16, ptr %711, !tbaa !2
  %713 = getelementptr i8, ptr @"R1%", i16 10
  %714 = load i16, ptr %713, !tbaa !2
  %715 = add i16 %714, %710
  %716 = inttoptr i16 %712 to ptr addrspace(2)
  %717 = addrspacecast ptr addrspace(2) %716 to ptr addrspace(1)
  %718 = getelementptr i8, ptr addrspace(1) %717, i16 %715
  %719 = load i16, ptr addrspace(1) %718, !tbaa !72
  %720 = trunc i16 %719 to i8
  %721 = load i16, ptr @b$seg, !tbaa !2
  %722 = inttoptr i16 %721 to ptr addrspace(2)
  %723 = addrspacecast ptr addrspace(2) %722 to ptr addrspace(1)
  %724 = getelementptr i8, ptr addrspace(1) %723, i16 %698
  store volatile i8 %720, ptr addrspace(1) %724
  %725 = load i16, ptr %58, !tbaa !2
  %726 = load i16, ptr %3, !tbaa !2
  %727 = add nsw i16 %725, %726
  store i16 %727, ptr %58, !tbaa !2
  br label %b132

b136:
  %728 = load i16, ptr %61, !tbaa !2
  %729 = load i16, ptr %5, !tbaa !2
  %730 = add nsw i16 %728, %729
  store i16 %730, ptr %61, !tbaa !2
  br label %b127

b137:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b139

b138:
  br label %b139

b139:
  %731 = load i16, ptr @"XIT%", !tbaa !2
  %732 = icmp eq i16 %731, 1
  br i1 %732, label %b140, label %b141

b140:
  br label %b2

b141:
  br label %b142

b142:
  br label %b97
}

define cc 1000 void @ROTATE3D(ptr dereferenceable(4) %0, ptr dereferenceable(4) %1, ptr dereferenceable(4) %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca float
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca float
  %14 = alloca float
  %15 = alloca float
  %16 = alloca float
  %17 = alloca float
  %18 = alloca float
  %19 = load float, ptr %0
  %20 = call float @llvm.fabs.f32(float %19)
  %21 = load float, ptr @$float86, !tbaa !2
  %22 = fcmp ogt float %20, %21
  br i1 %22, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b3:
  %23 = call float @llvm.cos.f32(float %19)
  %24 = fpround float %23 to float
  store float %24, ptr %18, !tbaa !2
  %25 = load float, ptr %1
  %26 = call float @llvm.fabs.f32(float %25)
  %27 = load float, ptr @$float86, !tbaa !2
  %28 = fcmp ogt float %26, %27
  br i1 %28, label %b4, label %b5

b4:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b5:
  %29 = call float @llvm.cos.f32(float %25)
  %30 = fpround float %29 to float
  store float %30, ptr %17, !tbaa !2
  %31 = load float, ptr %2
  %32 = call float @llvm.fabs.f32(float %31)
  %33 = load float, ptr @$float86, !tbaa !2
  %34 = fcmp ogt float %32, %33
  br i1 %34, label %b6, label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b7:
  %35 = call float @llvm.cos.f32(float %31)
  %36 = fpround float %35 to float
  store float %36, ptr %16, !tbaa !2
  %37 = load float, ptr %0
  %38 = call float @llvm.fabs.f32(float %37)
  %39 = load float, ptr @$float86, !tbaa !2
  %40 = fcmp ogt float %38, %39
  br i1 %40, label %b8, label %b9

b8:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b9:
  %41 = call float @llvm.sin.f32(float %37)
  %42 = fpround float %41 to float
  store float %42, ptr %15, !tbaa !2
  %43 = load float, ptr %1
  %44 = call float @llvm.fabs.f32(float %43)
  %45 = load float, ptr @$float86, !tbaa !2
  %46 = fcmp ogt float %44, %45
  br i1 %46, label %b10, label %b11

b10:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b11:
  %47 = call float @llvm.sin.f32(float %43)
  %48 = fpround float %47 to float
  store float %48, ptr %14, !tbaa !2
  %49 = load float, ptr %2
  %50 = call float @llvm.fabs.f32(float %49)
  %51 = load float, ptr @$float86, !tbaa !2
  %52 = fcmp ogt float %50, %51
  br i1 %52, label %b12, label %b13

b12:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b13:
  %53 = call float @llvm.sin.f32(float %49)
  %54 = fpround float %53 to float
  store float %54, ptr %13, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  %55 = load i16, ptr @"NDTS%", !tbaa !2
  %56 = sub i16 %55, 1
  store i16 %56, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b14

b14:
  %57 = load i16, ptr %10, !tbaa !2
  %58 = icmp sge i16 %57, 0
  br i1 %58, label %b15, label %b16

b15:
  %59 = load i16, ptr %12, !tbaa !2
  %60 = load i16, ptr %11, !tbaa !2
  %61 = icmp sle i16 %59, %60
  br i1 %61, label %b17, label %b18

b16:
  %62 = load i16, ptr %12, !tbaa !2
  %63 = load i16, ptr %11, !tbaa !2
  %64 = icmp sge i16 %62, %63
  br i1 %64, label %b17, label %b18

b17:
  %65 = load i16, ptr %12, !tbaa !2
  %66 = mul i16 %65, 4
  %67 = getelementptr i8, ptr @"X!", i16 2
  %68 = load i16, ptr %67, !tbaa !2
  %69 = add i16 0, %66
  %70 = inttoptr i16 %68 to ptr addrspace(2)
  %71 = addrspacecast ptr addrspace(2) %70 to ptr addrspace(1)
  %72 = getelementptr i8, ptr addrspace(1) %71, i16 %69
  %73 = load float, ptr %17, !tbaa !2
  %74 = load i16, ptr %12, !tbaa !2
  %75 = mul i16 %74, 2
  %76 = getelementptr i8, ptr @"XO%", i16 2
  %77 = load i16, ptr %76, !tbaa !2
  %78 = add i16 0, %75
  %79 = inttoptr i16 %77 to ptr addrspace(2)
  %80 = addrspacecast ptr addrspace(2) %79 to ptr addrspace(1)
  %81 = getelementptr i8, ptr addrspace(1) %80, i16 %78
  %82 = load i16, ptr addrspace(1) %81, !tbaa !16
  store i16 %82, ptr %9, !tbaa !2
  %83 = load i16, ptr %9, !tbaa !2
  %84 = sitofp i16 %83 to float
  %85 = fmul float %73, %84
  %86 = load float, ptr %14, !tbaa !2
  %87 = load i16, ptr %12, !tbaa !2
  %88 = mul i16 %87, 2
  %89 = getelementptr i8, ptr @"ZO%", i16 2
  %90 = load i16, ptr %89, !tbaa !2
  %91 = add i16 0, %88
  %92 = inttoptr i16 %90 to ptr addrspace(2)
  %93 = addrspacecast ptr addrspace(2) %92 to ptr addrspace(1)
  %94 = getelementptr i8, ptr addrspace(1) %93, i16 %91
  %95 = load i16, ptr addrspace(1) %94, !tbaa !18
  store i16 %95, ptr %8, !tbaa !2
  %96 = load i16, ptr %8, !tbaa !2
  %97 = sitofp i16 %96 to float
  %98 = fmul float %86, %97
  %99 = fsub float %85, %98
  %100 = fpround float %99 to float
  store float %100, ptr addrspace(1) %72, !tbaa !54
  %101 = load i16, ptr %12, !tbaa !2
  %102 = mul i16 %101, 4
  %103 = getelementptr i8, ptr @"Z!", i16 2
  %104 = load i16, ptr %103, !tbaa !2
  %105 = add i16 0, %102
  %106 = inttoptr i16 %104 to ptr addrspace(2)
  %107 = addrspacecast ptr addrspace(2) %106 to ptr addrspace(1)
  %108 = getelementptr i8, ptr addrspace(1) %107, i16 %105
  %109 = load float, ptr %14, !tbaa !2
  %110 = load i16, ptr %12, !tbaa !2
  %111 = mul i16 %110, 2
  %112 = getelementptr i8, ptr @"XO%", i16 2
  %113 = load i16, ptr %112, !tbaa !2
  %114 = add i16 0, %111
  %115 = inttoptr i16 %113 to ptr addrspace(2)
  %116 = addrspacecast ptr addrspace(2) %115 to ptr addrspace(1)
  %117 = getelementptr i8, ptr addrspace(1) %116, i16 %114
  %118 = load i16, ptr addrspace(1) %117, !tbaa !16
  store i16 %118, ptr %7, !tbaa !2
  %119 = load i16, ptr %7, !tbaa !2
  %120 = sitofp i16 %119 to float
  %121 = fmul float %109, %120
  %122 = load float, ptr %17, !tbaa !2
  %123 = load i16, ptr %12, !tbaa !2
  %124 = mul i16 %123, 2
  %125 = getelementptr i8, ptr @"ZO%", i16 2
  %126 = load i16, ptr %125, !tbaa !2
  %127 = add i16 0, %124
  %128 = inttoptr i16 %126 to ptr addrspace(2)
  %129 = addrspacecast ptr addrspace(2) %128 to ptr addrspace(1)
  %130 = getelementptr i8, ptr addrspace(1) %129, i16 %127
  %131 = load i16, ptr addrspace(1) %130, !tbaa !18
  store i16 %131, ptr %6, !tbaa !2
  %132 = load i16, ptr %6, !tbaa !2
  %133 = sitofp i16 %132 to float
  %134 = fmul float %122, %133
  %135 = fadd float %121, %134
  %136 = fpround float %135 to float
  store float %136, ptr addrspace(1) %108, !tbaa !56
  %137 = load i16, ptr %12, !tbaa !2
  %138 = mul i16 %137, 4
  %139 = getelementptr i8, ptr @"Y!", i16 2
  %140 = load i16, ptr %139, !tbaa !2
  %141 = add i16 0, %138
  %142 = inttoptr i16 %140 to ptr addrspace(2)
  %143 = addrspacecast ptr addrspace(2) %142 to ptr addrspace(1)
  %144 = getelementptr i8, ptr addrspace(1) %143, i16 %141
  %145 = load float, ptr %18, !tbaa !2
  %146 = load i16, ptr %12, !tbaa !2
  %147 = mul i16 %146, 2
  %148 = getelementptr i8, ptr @"YO%", i16 2
  %149 = load i16, ptr %148, !tbaa !2
  %150 = add i16 0, %147
  %151 = inttoptr i16 %149 to ptr addrspace(2)
  %152 = addrspacecast ptr addrspace(2) %151 to ptr addrspace(1)
  %153 = getelementptr i8, ptr addrspace(1) %152, i16 %150
  %154 = load i16, ptr addrspace(1) %153, !tbaa !20
  store i16 %154, ptr %5, !tbaa !2
  %155 = load i16, ptr %5, !tbaa !2
  %156 = sitofp i16 %155 to float
  %157 = fmul float %145, %156
  %158 = load float, ptr %15, !tbaa !2
  %159 = load i16, ptr %12, !tbaa !2
  %160 = mul i16 %159, 4
  %161 = getelementptr i8, ptr @"Z!", i16 2
  %162 = load i16, ptr %161, !tbaa !2
  %163 = add i16 0, %160
  %164 = inttoptr i16 %162 to ptr addrspace(2)
  %165 = addrspacecast ptr addrspace(2) %164 to ptr addrspace(1)
  %166 = getelementptr i8, ptr addrspace(1) %165, i16 %163
  %167 = load float, ptr addrspace(1) %166, !tbaa !56
  %168 = fmul float %158, %167
  %169 = fsub float %157, %168
  %170 = fpround float %169 to float
  store float %170, ptr addrspace(1) %144, !tbaa !58
  %171 = load i16, ptr %12, !tbaa !2
  %172 = mul i16 %171, 4
  %173 = getelementptr i8, ptr @"Z!", i16 2
  %174 = load i16, ptr %173, !tbaa !2
  %175 = add i16 0, %172
  %176 = inttoptr i16 %174 to ptr addrspace(2)
  %177 = addrspacecast ptr addrspace(2) %176 to ptr addrspace(1)
  %178 = getelementptr i8, ptr addrspace(1) %177, i16 %175
  %179 = load float, ptr %15, !tbaa !2
  %180 = load i16, ptr %12, !tbaa !2
  %181 = mul i16 %180, 2
  %182 = getelementptr i8, ptr @"YO%", i16 2
  %183 = load i16, ptr %182, !tbaa !2
  %184 = add i16 0, %181
  %185 = inttoptr i16 %183 to ptr addrspace(2)
  %186 = addrspacecast ptr addrspace(2) %185 to ptr addrspace(1)
  %187 = getelementptr i8, ptr addrspace(1) %186, i16 %184
  %188 = load i16, ptr addrspace(1) %187, !tbaa !20
  store i16 %188, ptr %4, !tbaa !2
  %189 = load i16, ptr %4, !tbaa !2
  %190 = sitofp i16 %189 to float
  %191 = fmul float %179, %190
  %192 = load float, ptr %18, !tbaa !2
  %193 = load i16, ptr %12, !tbaa !2
  %194 = mul i16 %193, 4
  %195 = getelementptr i8, ptr @"Z!", i16 2
  %196 = load i16, ptr %195, !tbaa !2
  %197 = add i16 0, %194
  %198 = inttoptr i16 %196 to ptr addrspace(2)
  %199 = addrspacecast ptr addrspace(2) %198 to ptr addrspace(1)
  %200 = getelementptr i8, ptr addrspace(1) %199, i16 %197
  %201 = load float, ptr addrspace(1) %200, !tbaa !56
  %202 = fmul float %192, %201
  %203 = fadd float %191, %202
  %204 = fpround float %203 to float
  store float %204, ptr addrspace(1) %178, !tbaa !56
  %205 = load i16, ptr %12, !tbaa !2
  %206 = mul i16 %205, 4
  %207 = getelementptr i8, ptr @"X!", i16 2
  %208 = load i16, ptr %207, !tbaa !2
  %209 = add i16 0, %206
  %210 = inttoptr i16 %208 to ptr addrspace(2)
  %211 = addrspacecast ptr addrspace(2) %210 to ptr addrspace(1)
  %212 = getelementptr i8, ptr addrspace(1) %211, i16 %209
  %213 = load float, ptr addrspace(1) %212, !tbaa !54
  store float %213, ptr %3, !tbaa !2
  %214 = load i16, ptr %12, !tbaa !2
  %215 = mul i16 %214, 4
  %216 = getelementptr i8, ptr @"X!", i16 2
  %217 = load i16, ptr %216, !tbaa !2
  %218 = add i16 0, %215
  %219 = inttoptr i16 %217 to ptr addrspace(2)
  %220 = addrspacecast ptr addrspace(2) %219 to ptr addrspace(1)
  %221 = getelementptr i8, ptr addrspace(1) %220, i16 %218
  %222 = load float, ptr %16, !tbaa !2
  %223 = load float, ptr %3, !tbaa !2
  %224 = fmul float %222, %223
  %225 = load float, ptr %13, !tbaa !2
  %226 = load i16, ptr %12, !tbaa !2
  %227 = mul i16 %226, 4
  %228 = getelementptr i8, ptr @"Y!", i16 2
  %229 = load i16, ptr %228, !tbaa !2
  %230 = add i16 0, %227
  %231 = inttoptr i16 %229 to ptr addrspace(2)
  %232 = addrspacecast ptr addrspace(2) %231 to ptr addrspace(1)
  %233 = getelementptr i8, ptr addrspace(1) %232, i16 %230
  %234 = load float, ptr addrspace(1) %233, !tbaa !58
  %235 = fmul float %225, %234
  %236 = fsub float %224, %235
  %237 = fpround float %236 to float
  store float %237, ptr addrspace(1) %221, !tbaa !54
  %238 = load i16, ptr %12, !tbaa !2
  %239 = mul i16 %238, 4
  %240 = getelementptr i8, ptr @"Y!", i16 2
  %241 = load i16, ptr %240, !tbaa !2
  %242 = add i16 0, %239
  %243 = inttoptr i16 %241 to ptr addrspace(2)
  %244 = addrspacecast ptr addrspace(2) %243 to ptr addrspace(1)
  %245 = getelementptr i8, ptr addrspace(1) %244, i16 %242
  %246 = load float, ptr %13, !tbaa !2
  %247 = load float, ptr %3, !tbaa !2
  %248 = fmul float %246, %247
  %249 = load float, ptr %16, !tbaa !2
  %250 = load i16, ptr %12, !tbaa !2
  %251 = mul i16 %250, 4
  %252 = getelementptr i8, ptr @"Y!", i16 2
  %253 = load i16, ptr %252, !tbaa !2
  %254 = add i16 0, %251
  %255 = inttoptr i16 %253 to ptr addrspace(2)
  %256 = addrspacecast ptr addrspace(2) %255 to ptr addrspace(1)
  %257 = getelementptr i8, ptr addrspace(1) %256, i16 %254
  %258 = load float, ptr addrspace(1) %257, !tbaa !58
  %259 = fmul float %249, %258
  %260 = fadd float %248, %259
  %261 = fpround float %260 to float
  store float %261, ptr addrspace(1) %245, !tbaa !58
  %262 = load i16, ptr %12, !tbaa !2
  %263 = load i16, ptr %10, !tbaa !2
  %264 = add nsw i16 %262, %263
  store i16 %264, ptr %12, !tbaa !2
  br label %b14

b18:
  ret void
}

define cc 1000 void @SAVERECORD() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = getelementptr inbounds i16, ptr @"KON%", i16 75
  %1 = load i16, ptr %0, !tbaa !2
  %2 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %2, ptr @K$)
  %3 = addrspacecast ptr @K$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) %3, i16 0)
  %4 = getelementptr inbounds i16, ptr @"KON%", i16 77
  %5 = load i16, ptr %4, !tbaa !2
  %6 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %5)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %6, ptr @K$)
  %7 = addrspacecast ptr @K$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) %7, i16 0)
  %8 = getelementptr inbounds i16, ptr @"KON%", i16 72
  %9 = load i16, ptr %8, !tbaa !2
  %10 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %9)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %10, ptr @K$)
  %11 = addrspacecast ptr @K$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) %11, i16 0)
  %12 = getelementptr inbounds i16, ptr @"KON%", i16 80
  %13 = load i16, ptr %12, !tbaa !2
  %14 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %13)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %14, ptr @K$)
  %15 = addrspacecast ptr @K$ to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) %15, i16 0)
  ret void
}

define cc 1000 void @SETCROSFADEPAL(ptr dereferenceable(18) %0, ptr dereferenceable(18) %1, ptr dereferenceable(18) %2, ptr dereferenceable(18) %3, ptr dereferenceable(18) %4, ptr dereferenceable(18) %5) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %13, !tbaa !2
  store i16 15, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b2

b2:
  %14 = load i16, ptr %11, !tbaa !2
  %15 = icmp sge i16 %14, 0
  br i1 %15, label %b3, label %b4

b3:
  %16 = load i16, ptr %13, !tbaa !2
  %17 = load i16, ptr %12, !tbaa !2
  %18 = icmp sle i16 %16, %17
  br i1 %18, label %b5, label %b6

b4:
  %19 = load i16, ptr %13, !tbaa !2
  %20 = load i16, ptr %12, !tbaa !2
  %21 = icmp sge i16 %19, %20
  br i1 %21, label %b5, label %b6

b5:
  store i16 0, ptr %10, !tbaa !2
  store i16 15, ptr %9, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %10, !tbaa !2
  store i16 255, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b12

b7:
  %22 = load i16, ptr %8, !tbaa !2
  %23 = icmp sge i16 %22, 0
  br i1 %23, label %b8, label %b9

b8:
  %24 = load i16, ptr %10, !tbaa !2
  %25 = load i16, ptr %9, !tbaa !2
  %26 = icmp sle i16 %24, %25
  br i1 %26, label %b10, label %b11

b9:
  %27 = load i16, ptr %10, !tbaa !2
  %28 = load i16, ptr %9, !tbaa !2
  %29 = icmp sge i16 %27, %28
  br i1 %29, label %b10, label %b11

b10:
  %30 = load i16, ptr %10, !tbaa !2
  %31 = mul i16 %30, 4
  %32 = trunc i16 %31 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %32)
  %33 = load i16, ptr %13, !tbaa !2
  %34 = mul i16 %33, 16
  %35 = load i16, ptr %10, !tbaa !2
  %36 = add i16 %34, %35
  %37 = mul i16 %36, 2
  %38 = getelementptr i8, ptr %0, i16 2
  %39 = load i16, ptr %38
  %40 = getelementptr i8, ptr %0, i16 10
  %41 = load i16, ptr %40
  %42 = add i16 %41, %37
  %43 = inttoptr i16 %39 to ptr addrspace(2)
  %44 = addrspacecast ptr addrspace(2) %43 to ptr addrspace(1)
  %45 = getelementptr i8, ptr addrspace(1) %44, i16 %42
  %46 = load i16, ptr %10, !tbaa !2
  %47 = mul i16 %46, 4
  store i16 %47, ptr addrspace(1) %45
  %48 = load i16, ptr %10, !tbaa !2
  %49 = mul i16 %48, 4
  %50 = trunc i16 %49 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %50)
  %51 = load i16, ptr %13, !tbaa !2
  %52 = mul i16 %51, 16
  %53 = load i16, ptr %10, !tbaa !2
  %54 = add i16 %52, %53
  %55 = mul i16 %54, 2
  %56 = getelementptr i8, ptr %2, i16 2
  %57 = load i16, ptr %56
  %58 = getelementptr i8, ptr %2, i16 10
  %59 = load i16, ptr %58
  %60 = add i16 %59, %55
  %61 = inttoptr i16 %57 to ptr addrspace(2)
  %62 = addrspacecast ptr addrspace(2) %61 to ptr addrspace(1)
  %63 = getelementptr i8, ptr addrspace(1) %62, i16 %60
  %64 = load i16, ptr %10, !tbaa !2
  %65 = mul i16 %64, 4
  store i16 %65, ptr addrspace(1) %63
  %66 = load i16, ptr %10, !tbaa !2
  %67 = mul i16 %66, 4
  %68 = trunc i16 %67 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %68)
  %69 = load i16, ptr %13, !tbaa !2
  %70 = mul i16 %69, 16
  %71 = load i16, ptr %10, !tbaa !2
  %72 = add i16 %70, %71
  %73 = mul i16 %72, 2
  %74 = getelementptr i8, ptr %4, i16 2
  %75 = load i16, ptr %74
  %76 = getelementptr i8, ptr %4, i16 10
  %77 = load i16, ptr %76
  %78 = add i16 %77, %73
  %79 = inttoptr i16 %75 to ptr addrspace(2)
  %80 = addrspacecast ptr addrspace(2) %79 to ptr addrspace(1)
  %81 = getelementptr i8, ptr addrspace(1) %80, i16 %78
  %82 = load i16, ptr %10, !tbaa !2
  %83 = mul i16 %82, 4
  store i16 %83, ptr addrspace(1) %81
  %84 = load i16, ptr %10, !tbaa !2
  %85 = load i16, ptr %8, !tbaa !2
  %86 = add nsw i16 %84, %85
  store i16 %86, ptr %10, !tbaa !2
  br label %b7

b11:
  %87 = load i16, ptr %13, !tbaa !2
  %88 = load i16, ptr %11, !tbaa !2
  %89 = add nsw i16 %87, %88
  store i16 %89, ptr %13, !tbaa !2
  br label %b2

b12:
  %90 = load i16, ptr %6, !tbaa !2
  %91 = icmp sge i16 %90, 0
  br i1 %91, label %b13, label %b14

b13:
  %92 = load i16, ptr %10, !tbaa !2
  %93 = load i16, ptr %7, !tbaa !2
  %94 = icmp sle i16 %92, %93
  br i1 %94, label %b15, label %b16

b14:
  %95 = load i16, ptr %10, !tbaa !2
  %96 = load i16, ptr %7, !tbaa !2
  %97 = icmp sge i16 %95, %96
  br i1 %97, label %b15, label %b16

b15:
  %98 = load i16, ptr %10, !tbaa !2
  %99 = mul i16 %98, 2
  %100 = getelementptr i8, ptr %1, i16 2
  %101 = load i16, ptr %100
  %102 = getelementptr i8, ptr %1, i16 10
  %103 = load i16, ptr %102
  %104 = add i16 %103, %99
  %105 = inttoptr i16 %101 to ptr addrspace(2)
  %106 = addrspacecast ptr addrspace(2) %105 to ptr addrspace(1)
  %107 = getelementptr i8, ptr addrspace(1) %106, i16 %104
  %108 = load i16, ptr %10, !tbaa !2
  %109 = sext i16 %108 to i32
  %110 = sdiv i32 %109, 16
  %111 = trunc i32 %110 to i16
  %112 = mul i16 %111, 4
  store i16 %112, ptr addrspace(1) %107
  %113 = load i16, ptr %10, !tbaa !2
  %114 = mul i16 %113, 2
  %115 = getelementptr i8, ptr %3, i16 2
  %116 = load i16, ptr %115
  %117 = getelementptr i8, ptr %3, i16 10
  %118 = load i16, ptr %117
  %119 = add i16 %118, %114
  %120 = inttoptr i16 %116 to ptr addrspace(2)
  %121 = addrspacecast ptr addrspace(2) %120 to ptr addrspace(1)
  %122 = getelementptr i8, ptr addrspace(1) %121, i16 %119
  %123 = load i16, ptr %10, !tbaa !2
  %124 = sext i16 %123 to i32
  %125 = sdiv i32 %124, 16
  %126 = trunc i32 %125 to i16
  %127 = mul i16 %126, 4
  store i16 %127, ptr addrspace(1) %122
  %128 = load i16, ptr %10, !tbaa !2
  %129 = mul i16 %128, 2
  %130 = getelementptr i8, ptr %5, i16 2
  %131 = load i16, ptr %130
  %132 = getelementptr i8, ptr %5, i16 10
  %133 = load i16, ptr %132
  %134 = add i16 %133, %129
  %135 = inttoptr i16 %131 to ptr addrspace(2)
  %136 = addrspacecast ptr addrspace(2) %135 to ptr addrspace(1)
  %137 = getelementptr i8, ptr addrspace(1) %136, i16 %134
  %138 = load i16, ptr %10, !tbaa !2
  %139 = sext i16 %138 to i32
  %140 = sdiv i32 %139, 16
  %141 = trunc i32 %140 to i16
  %142 = mul i16 %141, 4
  store i16 %142, ptr addrspace(1) %137
  %143 = load i16, ptr %10, !tbaa !2
  %144 = load i16, ptr %6, !tbaa !2
  %145 = add nsw i16 %143, %144
  store i16 %145, ptr %10, !tbaa !2
  br label %b12

b16:
  ret void
}

define cc 1000 void @SETPAL(ptr dereferenceable(2) %0, ptr dereferenceable(2) %1, ptr dereferenceable(2) %2, ptr dereferenceable(2) %3, ptr dereferenceable(2) %4, ptr dereferenceable(2) %5, ptr dereferenceable(2) %6, ptr dereferenceable(2) %7) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca float
  %19 = alloca i16
  %20 = alloca float
  %21 = alloca i16
  %22 = alloca float
  %23 = alloca i16
  %24 = load i16, ptr %1
  %25 = load i16, ptr %0
  %26 = sub i16 %24, %25
  store i16 %26, ptr %23, !tbaa !2
  %27 = load i16, ptr %2
  store i16 %27, ptr %21, !tbaa !2
  %28 = load i16, ptr %21, !tbaa !2
  %29 = sitofp i16 %28 to float
  %30 = fpround float %29 to float
  store float %30, ptr %22, !tbaa !2
  %31 = load i16, ptr %3
  store i16 %31, ptr %19, !tbaa !2
  %32 = load i16, ptr %19, !tbaa !2
  %33 = sitofp i16 %32 to float
  %34 = fpround float %33 to float
  store float %34, ptr %20, !tbaa !2
  %35 = load i16, ptr %4
  store i16 %35, ptr %17, !tbaa !2
  %36 = load i16, ptr %17, !tbaa !2
  %37 = sitofp i16 %36 to float
  %38 = fpround float %37 to float
  store float %38, ptr %18, !tbaa !2
  %39 = load i16, ptr %0
  %40 = trunc i16 %39 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %40)
  %41 = load i16, ptr %0
  store i16 %41, ptr %16, !tbaa !2
  %42 = load i16, ptr %1
  store i16 %42, ptr %15, !tbaa !2
  store i16 1, ptr %14, !tbaa !2
  br label %b2

b2:
  %43 = load i16, ptr %14, !tbaa !2
  %44 = icmp sge i16 %43, 0
  br i1 %44, label %b3, label %b4

b3:
  %45 = load i16, ptr %16, !tbaa !2
  %46 = load i16, ptr %15, !tbaa !2
  %47 = icmp sle i16 %45, %46
  br i1 %47, label %b5, label %b6

b4:
  %48 = load i16, ptr %16, !tbaa !2
  %49 = load i16, ptr %15, !tbaa !2
  %50 = icmp sge i16 %48, %49
  br i1 %50, label %b5, label %b6

b5:
  %51 = load float, ptr %22, !tbaa !2
  %52 = call i16 @llvm.lrint.i16.f32(float %51)
  %53 = trunc i16 %52 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %53)
  %54 = load float, ptr %20, !tbaa !2
  %55 = call i16 @llvm.lrint.i16.f32(float %54)
  %56 = trunc i16 %55 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %56)
  %57 = load float, ptr %18, !tbaa !2
  %58 = call i16 @llvm.lrint.i16.f32(float %57)
  %59 = trunc i16 %58 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %59)
  %60 = load float, ptr %22, !tbaa !2
  %61 = load i16, ptr %5
  %62 = load i16, ptr %2
  %63 = sub i16 %61, %62
  %64 = load i16, ptr %23, !tbaa !2
  store i16 %63, ptr %13, !tbaa !2
  %65 = load i16, ptr %13, !tbaa !2
  %66 = sitofp i16 %65 to float
  store i16 %64, ptr %12, !tbaa !2
  %67 = load i16, ptr %12, !tbaa !2
  %68 = sitofp i16 %67 to float
  %69 = fdiv float %66, %68
  %70 = fadd float %60, %69
  %71 = fpround float %70 to float
  store float %71, ptr %22, !tbaa !2
  %72 = load float, ptr %20, !tbaa !2
  %73 = load i16, ptr %6
  %74 = load i16, ptr %3
  %75 = sub i16 %73, %74
  %76 = load i16, ptr %23, !tbaa !2
  store i16 %75, ptr %11, !tbaa !2
  %77 = load i16, ptr %11, !tbaa !2
  %78 = sitofp i16 %77 to float
  store i16 %76, ptr %10, !tbaa !2
  %79 = load i16, ptr %10, !tbaa !2
  %80 = sitofp i16 %79 to float
  %81 = fdiv float %78, %80
  %82 = fadd float %72, %81
  %83 = fpround float %82 to float
  store float %83, ptr %20, !tbaa !2
  %84 = load float, ptr %18, !tbaa !2
  %85 = load i16, ptr %7
  %86 = load i16, ptr %4
  %87 = sub i16 %85, %86
  %88 = load i16, ptr %23, !tbaa !2
  store i16 %87, ptr %9, !tbaa !2
  %89 = load i16, ptr %9, !tbaa !2
  %90 = sitofp i16 %89 to float
  store i16 %88, ptr %8, !tbaa !2
  %91 = load i16, ptr %8, !tbaa !2
  %92 = sitofp i16 %91 to float
  %93 = fdiv float %90, %92
  %94 = fadd float %84, %93
  %95 = fpround float %94 to float
  store float %95, ptr %18, !tbaa !2
  %96 = load i16, ptr %16, !tbaa !2
  %97 = load i16, ptr %14, !tbaa !2
  %98 = add nsw i16 %96, %97
  store i16 %98, ptr %16, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc 1000 void @SPHEREMAPLASMA() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca i16
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca float
  %64 = alloca i16
  %65 = alloca float
  %66 = alloca i16
  %67 = alloca float
  %68 = alloca i16
  %69 = alloca float
  %70 = alloca i16
  %71 = alloca float
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca i16
  %79 = alloca i16
  %80 = alloca i16
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca i16
  %84 = alloca i16
  %85 = alloca i16
  %86 = alloca i16
  %87 = alloca i16
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca i16
  %91 = alloca i16
  %92 = alloca i16
  %93 = alloca i16
  %94 = alloca i32
  %95 = alloca [4 x i8]
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  %103 = alloca i16
  %104 = alloca i16
  %105 = alloca i16
  %106 = alloca i16
  %107 = alloca i16
  %108 = alloca i16
  %109 = alloca i16
  %110 = alloca i16
  %111 = alloca i16
  %112 = alloca i16
  %113 = alloca i16
  %114 = alloca i16
  %115 = alloca i16
  %116 = alloca i16
  %117 = alloca i16
  %118 = alloca i16
  %119 = alloca i16
  %120 = alloca i16
  %121 = alloca i16
  %122 = alloca i16
  %123 = alloca i16
  %124 = alloca i16
  %125 = alloca i16
  %126 = alloca i16
  %127 = alloca i16
  %128 = alloca i16
  %129 = alloca i16
  %130 = alloca i16
  %131 = alloca i16
  %132 = alloca i16
  %133 = alloca i16
  %134 = alloca i16
  %135 = alloca i16
  %136 = alloca i16
  %137 = alloca i16
  %138 = alloca i16
  %139 = alloca i16
  %140 = alloca i16
  %141 = alloca i16
  %142 = alloca i16
  %143 = alloca i16
  %144 = alloca i16
  %145 = alloca i16
  %146 = alloca i16
  %147 = alloca i16
  %148 = alloca i16
  %149 = alloca i16
  %150 = alloca i16
  %151 = alloca i16
  %152 = alloca i16
  %153 = alloca i16
  %154 = alloca i16
  %155 = alloca i16
  %156 = alloca i16
  %157 = alloca i16
  %158 = alloca i16
  %159 = alloca i16
  %160 = alloca i16
  %161 = alloca i16
  %162 = alloca i16
  %163 = alloca i16
  %164 = alloca i16
  %165 = alloca i16
  %166 = alloca i16
  %167 = alloca i16
  %168 = alloca i16
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store i16 0, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  store i16 0, ptr %33
  store i16 0, ptr %34
  store i16 0, ptr %35
  store i16 0, ptr %36
  store i16 0, ptr %37
  store i16 0, ptr %38
  store i16 0, ptr %39
  store i16 0, ptr %40
  store i16 0, ptr %41
  store i16 0, ptr %42
  store i16 0, ptr %43
  store i16 0, ptr %44
  store i16 0, ptr %45
  store i16 0, ptr %46
  store i16 0, ptr %47
  store i16 0, ptr %48
  store i16 0, ptr %49
  store i16 0, ptr %50
  store i16 0, ptr %51
  store i16 0, ptr %52
  store i16 0, ptr %53
  store i16 0, ptr %54
  store i16 0, ptr %55
  store i16 0, ptr %56
  store i16 0, ptr %57
  store i16 0, ptr %58
  store i16 0, ptr %59
  store i16 0, ptr %60
  store i16 0, ptr %61
  store i16 0, ptr %62
  store float 0.000000e+00, ptr %63
  store i16 0, ptr %64
  store float 0.000000e+00, ptr %65
  store i16 0, ptr %66
  store float 0.000000e+00, ptr %67
  store i16 0, ptr %68
  store float 0.000000e+00, ptr %69
  store i16 0, ptr %70
  store float 0.000000e+00, ptr %71
  store i16 0, ptr %72
  store i16 0, ptr %73
  store i16 0, ptr %74
  store i16 0, ptr %75
  store i16 0, ptr %76
  store i16 0, ptr %77
  store i16 0, ptr %78
  store i16 0, ptr %79
  store i16 0, ptr %80
  store i16 0, ptr %81
  store i16 0, ptr %82
  store i16 0, ptr %83
  store i16 0, ptr %84
  store i16 0, ptr %85
  store i16 0, ptr %86
  store i16 0, ptr %87
  store i16 0, ptr %88
  store i16 0, ptr %89
  store i16 0, ptr %90
  store i16 0, ptr %91
  store i16 0, ptr %92
  store i16 0, ptr %93
  store i32 0, ptr %94
  call void @llvm.memset.p0.i16(ptr %95, i8 0, i16 4, i1 false)
  store i16 0, ptr %96
  store i16 0, ptr %97
  store i16 0, ptr %98
  store i16 0, ptr %99
  store i16 0, ptr %100
  store i16 0, ptr %101
  store i16 0, ptr %102
  store i16 0, ptr %103
  store i16 0, ptr %104
  store i16 0, ptr %105
  store i16 0, ptr %106
  store i16 0, ptr %107
  store i16 0, ptr %108
  store i16 0, ptr %109
  store i16 0, ptr %110
  store i16 0, ptr %111
  store i16 0, ptr %112
  store i16 0, ptr %113
  store i16 0, ptr %114
  store i16 0, ptr %115
  store i16 0, ptr %116
  store i16 0, ptr %117
  store i16 0, ptr %118
  store i16 0, ptr %119
  store i16 0, ptr %120
  store i16 0, ptr %121
  store i16 0, ptr %122
  store i16 0, ptr %123
  store i16 0, ptr %124
  store i16 0, ptr %125
  store i16 0, ptr %126
  store i16 0, ptr %127
  store i16 0, ptr %128
  store i16 0, ptr %129
  store i16 0, ptr %130
  store i16 0, ptr %131
  store i16 0, ptr %132
  store i16 0, ptr %133
  store i16 0, ptr %134
  store i16 0, ptr %135
  store i16 0, ptr %136
  store i16 0, ptr %137
  store i16 0, ptr %138
  store i16 0, ptr %139
  store i16 0, ptr %140
  store i16 0, ptr %141
  store i16 0, ptr %142
  store i16 0, ptr %143
  store i16 0, ptr %144
  store i16 0, ptr %145
  store i16 0, ptr %146
  store i16 0, ptr %147
  store i16 0, ptr %148
  store i16 0, ptr %149
  store i16 0, ptr %150
  store i16 0, ptr %151
  store i16 0, ptr %152
  store i16 0, ptr %153
  store i16 0, ptr %154
  store i16 0, ptr %155
  store i16 0, ptr %156
  store i16 0, ptr %157
  store i16 0, ptr %158
  store i16 0, ptr %159
  store i16 0, ptr %160
  store i16 0, ptr %161
  store i16 0, ptr %162
  store i16 0, ptr %163
  store i16 0, ptr %164
  store i16 0, ptr %165
  store i16 0, ptr %166
  store i16 0, ptr %167
  store i16 0, ptr %168
  store i16 0, ptr %95, !tbaa !2
  %169 = getelementptr inbounds i8, ptr %95, i16 2
  store i16 0, ptr %169, !tbaa !2
  store i16 0, ptr %52, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  store i16 0, ptr %168, !tbaa !2
  store i16 31, ptr %167, !tbaa !2
  store i16 0, ptr %166, !tbaa !2
  store i16 0, ptr %165, !tbaa !2
  store i16 0, ptr %164, !tbaa !2
  store i16 63, ptr %163, !tbaa !2
  store i16 0, ptr %162, !tbaa !2
  store i16 0, ptr %161, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %168, ptr %167, ptr %166, ptr %165, ptr %164, ptr %163, ptr %162, ptr %161)
  store i16 32, ptr %160, !tbaa !2
  store i16 63, ptr %159, !tbaa !2
  store i16 63, ptr %158, !tbaa !2
  store i16 0, ptr %157, !tbaa !2
  store i16 0, ptr %156, !tbaa !2
  store i16 63, ptr %155, !tbaa !2
  store i16 0, ptr %154, !tbaa !2
  store i16 63, ptr %153, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %160, ptr %159, ptr %158, ptr %157, ptr %156, ptr %155, ptr %154, ptr %153)
  store i16 64, ptr %152, !tbaa !2
  store i16 95, ptr %151, !tbaa !2
  store i16 63, ptr %150, !tbaa !2
  store i16 0, ptr %149, !tbaa !2
  store i16 63, ptr %148, !tbaa !2
  store i16 0, ptr %147, !tbaa !2
  store i16 63, ptr %146, !tbaa !2
  store i16 63, ptr %145, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %152, ptr %151, ptr %150, ptr %149, ptr %148, ptr %147, ptr %146, ptr %145)
  store i16 96, ptr %144, !tbaa !2
  store i16 127, ptr %143, !tbaa !2
  store i16 0, ptr %142, !tbaa !2
  store i16 63, ptr %141, !tbaa !2
  store i16 63, ptr %140, !tbaa !2
  store i16 0, ptr %139, !tbaa !2
  store i16 0, ptr %138, !tbaa !2
  store i16 0, ptr %137, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %144, ptr %143, ptr %142, ptr %141, ptr %140, ptr %139, ptr %138, ptr %137)
  store i16 128, ptr %136, !tbaa !2
  store i16 159, ptr %135, !tbaa !2
  store i16 0, ptr %134, !tbaa !2
  store i16 0, ptr %133, !tbaa !2
  store i16 31, ptr %132, !tbaa !2
  store i16 31, ptr %131, !tbaa !2
  store i16 0, ptr %130, !tbaa !2
  store i16 31, ptr %129, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %136, ptr %135, ptr %134, ptr %133, ptr %132, ptr %131, ptr %130, ptr %129)
  store i16 160, ptr %128, !tbaa !2
  store i16 191, ptr %127, !tbaa !2
  store i16 31, ptr %126, !tbaa !2
  store i16 0, ptr %125, !tbaa !2
  store i16 31, ptr %124, !tbaa !2
  store i16 31, ptr %123, !tbaa !2
  store i16 0, ptr %122, !tbaa !2
  store i16 63, ptr %121, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %128, ptr %127, ptr %126, ptr %125, ptr %124, ptr %123, ptr %122, ptr %121)
  store i16 192, ptr %120, !tbaa !2
  store i16 223, ptr %119, !tbaa !2
  store i16 31, ptr %118, !tbaa !2
  store i16 0, ptr %117, !tbaa !2
  store i16 63, ptr %116, !tbaa !2
  store i16 0, ptr %115, !tbaa !2
  store i16 47, ptr %114, !tbaa !2
  store i16 63, ptr %113, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %120, ptr %119, ptr %118, ptr %117, ptr %116, ptr %115, ptr %114, ptr %113)
  store i16 224, ptr %112, !tbaa !2
  store i16 254, ptr %111, !tbaa !2
  store i16 0, ptr %110, !tbaa !2
  store i16 47, ptr %109, !tbaa !2
  store i16 63, ptr %108, !tbaa !2
  store i16 0, ptr %107, !tbaa !2
  store i16 0, ptr %106, !tbaa !2
  store i16 31, ptr %105, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %112, ptr %111, ptr %110, ptr %109, ptr %108, ptr %107, ptr %106, ptr %105)
  call cc 1000 addrspace(1) void @GETPAL()
  %170 = getelementptr i8, ptr @"R%", i16 2
  %171 = load i16, ptr %170, !tbaa !2
  %172 = getelementptr i8, ptr @"R%", i16 10
  %173 = load i16, ptr %172, !tbaa !2
  %174 = add i16 %173, 510
  %175 = inttoptr i16 %171 to ptr addrspace(2)
  %176 = addrspacecast ptr addrspace(2) %175 to ptr addrspace(1)
  %177 = getelementptr i8, ptr addrspace(1) %176, i16 %174
  store i16 0, ptr addrspace(1) %177, !tbaa !60
  %178 = getelementptr i8, ptr @"G%", i16 2
  %179 = load i16, ptr %178, !tbaa !2
  %180 = getelementptr i8, ptr @"G%", i16 10
  %181 = load i16, ptr %180, !tbaa !2
  %182 = add i16 %181, 510
  %183 = inttoptr i16 %179 to ptr addrspace(2)
  %184 = addrspacecast ptr addrspace(2) %183 to ptr addrspace(1)
  %185 = getelementptr i8, ptr addrspace(1) %184, i16 %182
  store i16 0, ptr addrspace(1) %185, !tbaa !62
  %186 = getelementptr i8, ptr @"B%", i16 2
  %187 = load i16, ptr %186, !tbaa !2
  %188 = getelementptr i8, ptr @"B%", i16 10
  %189 = load i16, ptr %188, !tbaa !2
  %190 = add i16 %189, 510
  %191 = inttoptr i16 %187 to ptr addrspace(2)
  %192 = addrspacecast ptr addrspace(2) %191 to ptr addrspace(1)
  %193 = getelementptr i8, ptr addrspace(1) %192, i16 %190
  store i16 31, ptr addrspace(1) %193, !tbaa !64
  store i16 0, ptr %104, !tbaa !2
  store i16 255, ptr %103, !tbaa !2
  store i16 63, ptr %102, !tbaa !2
  store i16 63, ptr %101, !tbaa !2
  store i16 63, ptr %100, !tbaa !2
  store i16 63, ptr %99, !tbaa !2
  store i16 63, ptr %98, !tbaa !2
  store i16 63, ptr %97, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %104, ptr %103, ptr %102, ptr %101, ptr %100, ptr %99, ptr %98, ptr %97)
  store i16 210, ptr %96, !tbaa !2
  call cc 1000 addrspace(1) void @DELAY(ptr %96)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string239, ptr %95)
  store i32 -1, ptr %94, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr %95, i16 1, i16 -1, i16 32)
  store i16 0, ptr %93, !tbaa !2
  store i16 16383, ptr %92, !tbaa !2
  store i16 1, ptr %91, !tbaa !2
  br label %b7

b2:
  %194 = load i16, ptr %52, !tbaa !2
  %195 = icmp eq i16 %194, 0
  br i1 %195, label %b90, label %b91

b3:
  %196 = load float, ptr %71, !tbaa !2
  %197 = load float, ptr %69, !tbaa !2
  %198 = fadd float %196, %197
  %199 = fpround float %198 to float
  store float %199, ptr %71, !tbaa !2
  %200 = load float, ptr %71, !tbaa !2
  store i16 254, ptr %22, !tbaa !2
  %201 = load i16, ptr %22, !tbaa !2
  %202 = sitofp i16 %201 to float
  %203 = fcmp ogt float %200, %202
  %204 = sext i1 %203 to i16
  %205 = load float, ptr %71, !tbaa !2
  store i16 65, ptr %21, !tbaa !2
  %206 = load i16, ptr %21, !tbaa !2
  %207 = sitofp i16 %206 to float
  %208 = fcmp olt float %205, %207
  %209 = sext i1 %208 to i16
  %210 = or i16 %204, %209
  %211 = icmp ne i16 %210, 0
  br i1 %211, label %b103, label %b104

b4:
  store i16 0, ptr %8, !tbaa !2
  store i16 319, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b127

b5:
  %212 = load i16, ptr %11, !tbaa !2
  %213 = load i16, ptr %9, !tbaa !2
  %214 = add nsw i16 %212, %213
  store i16 %214, ptr %11, !tbaa !2
  br label %b112

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %95)
  ret void

b7:
  %215 = load i16, ptr %91, !tbaa !2
  %216 = icmp sge i16 %215, 0
  br i1 %216, label %b8, label %b9

b8:
  %217 = load i16, ptr %93, !tbaa !2
  %218 = load i16, ptr %92, !tbaa !2
  %219 = icmp sle i16 %217, %218
  br i1 %219, label %b10, label %b11

b9:
  %220 = load i16, ptr %93, !tbaa !2
  %221 = load i16, ptr %92, !tbaa !2
  %222 = icmp sge i16 %220, %221
  br i1 %222, label %b10, label %b11

b10:
  %223 = load i32, ptr %94, !tbaa !2
  %224 = add i32 %223, 2
  store i32 %224, ptr %94, !tbaa !2
  %225 = addrspacecast ptr %95 to ptr addrspace(1)
  %226 = load i32, ptr %94, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 %226, ptr addrspace(1) %225, i16 0)
  %227 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %95)
  store i16 %227, ptr %90, !tbaa !2
  %228 = addrspacecast ptr %95 to ptr addrspace(1)
  %229 = load i32, ptr %94, !tbaa !2
  %230 = add i32 %229, 1
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 %230, ptr addrspace(1) %228, i16 0)
  %231 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %95)
  store i16 %231, ptr %89, !tbaa !2
  %232 = load i16, ptr %93, !tbaa !2
  %233 = load i16, ptr %89, !tbaa !2
  %234 = mul i16 %233, 128
  %235 = load i16, ptr %90, !tbaa !2
  %236 = add i16 %234, %235
  %237 = sub i16 %232, 0
  %238 = getelementptr inbounds i16, ptr @"SP%", i16 %237
  store i16 %236, ptr %238, !tbaa !2
  %239 = load i16, ptr %93, !tbaa !2
  %240 = icmp eq i16 %239, 8256
  br i1 %240, label %b12, label %b13

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 0)
  store i16 99, ptr %88, !tbaa !2
  store i16 -180, ptr %93, !tbaa !2
  store i16 -1, ptr %87, !tbaa !2
  store i16 1, ptr %86, !tbaa !2
  br label %b15

b12:
  %241 = load i16, ptr %93, !tbaa !2
  %242 = load i16, ptr %93, !tbaa !2
  %243 = sub i16 %242, 0
  %244 = getelementptr inbounds i16, ptr @"SP%", i16 %243
  %245 = load i16, ptr %244, !tbaa !2
  %246 = add i16 %245, 1
  %247 = sub i16 %241, 0
  %248 = getelementptr inbounds i16, ptr @"SP%", i16 %247
  store i16 %246, ptr %248, !tbaa !2
  br label %b14

b13:
  br label %b14

b14:
  %249 = load i16, ptr %93, !tbaa !2
  %250 = load i16, ptr %91, !tbaa !2
  %251 = add nsw i16 %249, %250
  store i16 %251, ptr %93, !tbaa !2
  br label %b7

b15:
  %252 = load i16, ptr %86, !tbaa !2
  %253 = icmp sge i16 %252, 0
  br i1 %253, label %b16, label %b17

b16:
  %254 = load i16, ptr %93, !tbaa !2
  %255 = load i16, ptr %87, !tbaa !2
  %256 = icmp sle i16 %254, %255
  br i1 %256, label %b18, label %b19

b17:
  %257 = load i16, ptr %93, !tbaa !2
  %258 = load i16, ptr %87, !tbaa !2
  %259 = icmp sge i16 %257, %258
  br i1 %259, label %b18, label %b19

b18:
  %260 = load i16, ptr %93, !tbaa !2
  %261 = load i16, ptr %93, !tbaa !2
  %262 = load i16, ptr %93, !tbaa !2
  %263 = load i16, ptr %88, !tbaa !2
  %264 = sub i16 %262, %263
  %265 = add i16 %264, 1
  %266 = load i16, ptr %88, !tbaa !2
  %267 = sext i16 %265 to i32
  %268 = sext i16 %266 to i32
  %269 = icmp eq i32 %268, 0
  br i1 %269, label %b20, label %b21

b19:
  store i16 0, ptr %93, !tbaa !2
  store i16 180, ptr %85, !tbaa !2
  store i16 1, ptr %84, !tbaa !2
  br label %b28

b20:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b21:
  %270 = add i32 %268, 1
  %271 = add i32 %267, 32768
  %272 = or i32 %270, %271
  %273 = icmp eq i32 %272, 0
  br i1 %273, label %b22, label %b23

b22:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b23:
  %274 = sdiv i32 %267, %268
  %275 = trunc i32 %274 to i16
  %276 = load i16, ptr %88, !tbaa !2
  %277 = mul i16 %275, %276
  %278 = sub i16 %261, %277
  %279 = sub i16 %260, -192
  %280 = getelementptr inbounds i16, ptr @"DT%", i16 %279
  store i16 %278, ptr %280, !tbaa !2
  %281 = load i16, ptr %93, !tbaa !2
  %282 = load i16, ptr %93, !tbaa !2
  %283 = load i16, ptr %93, !tbaa !2
  %284 = load i16, ptr %88, !tbaa !2
  %285 = sub i16 %283, %284
  %286 = add i16 %285, 1
  %287 = load i16, ptr %88, !tbaa !2
  %288 = sext i16 %286 to i32
  %289 = sext i16 %287 to i32
  %290 = icmp eq i32 %289, 0
  br i1 %290, label %b24, label %b25

b24:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b25:
  %291 = add i32 %289, 1
  %292 = add i32 %288, 32768
  %293 = or i32 %291, %292
  %294 = icmp eq i32 %293, 0
  br i1 %294, label %b26, label %b27

b26:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b27:
  %295 = sdiv i32 %288, %289
  %296 = trunc i32 %295 to i16
  %297 = load i16, ptr %88, !tbaa !2
  %298 = mul i16 %296, %297
  %299 = sub i16 %282, %298
  %300 = mul i16 %299, 100
  %301 = sub i16 %281, -180
  %302 = getelementptr inbounds i16, ptr @"DT100%", i16 %301
  store i16 %300, ptr %302, !tbaa !2
  %303 = load i16, ptr %93, !tbaa !2
  %304 = load i16, ptr %86, !tbaa !2
  %305 = add nsw i16 %303, %304
  store i16 %305, ptr %93, !tbaa !2
  br label %b15

b28:
  %306 = load i16, ptr %84, !tbaa !2
  %307 = icmp sge i16 %306, 0
  br i1 %307, label %b29, label %b30

b29:
  %308 = load i16, ptr %93, !tbaa !2
  %309 = load i16, ptr %85, !tbaa !2
  %310 = icmp sle i16 %308, %309
  br i1 %310, label %b31, label %b32

b30:
  %311 = load i16, ptr %93, !tbaa !2
  %312 = load i16, ptr %85, !tbaa !2
  %313 = icmp sge i16 %311, %312
  br i1 %313, label %b31, label %b32

b31:
  %314 = load i16, ptr %93, !tbaa !2
  %315 = load i16, ptr %93, !tbaa !2
  %316 = load i16, ptr %93, !tbaa !2
  %317 = sub i16 %316, 1
  %318 = load i16, ptr %88, !tbaa !2
  %319 = sext i16 %317 to i32
  %320 = sext i16 %318 to i32
  %321 = icmp eq i32 %320, 0
  br i1 %321, label %b33, label %b34

b32:
  store i16 -640, ptr %93, !tbaa !2
  store i16 518, ptr %83, !tbaa !2
  store i16 1, ptr %82, !tbaa !2
  br label %b41

b33:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b34:
  %322 = add i32 %320, 1
  %323 = add i32 %319, 32768
  %324 = or i32 %322, %323
  %325 = icmp eq i32 %324, 0
  br i1 %325, label %b35, label %b36

b35:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b36:
  %326 = sdiv i32 %319, %320
  %327 = trunc i32 %326 to i16
  %328 = load i16, ptr %88, !tbaa !2
  %329 = mul i16 %327, %328
  %330 = sub i16 %315, %329
  %331 = sub i16 %314, -192
  %332 = getelementptr inbounds i16, ptr @"DT%", i16 %331
  store i16 %330, ptr %332, !tbaa !2
  %333 = load i16, ptr %93, !tbaa !2
  %334 = load i16, ptr %93, !tbaa !2
  %335 = load i16, ptr %93, !tbaa !2
  %336 = sub i16 %335, 1
  %337 = load i16, ptr %88, !tbaa !2
  %338 = sext i16 %336 to i32
  %339 = sext i16 %337 to i32
  %340 = icmp eq i32 %339, 0
  br i1 %340, label %b37, label %b38

b37:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b38:
  %341 = add i32 %339, 1
  %342 = add i32 %338, 32768
  %343 = or i32 %341, %342
  %344 = icmp eq i32 %343, 0
  br i1 %344, label %b39, label %b40

b39:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b40:
  %345 = sdiv i32 %338, %339
  %346 = trunc i32 %345 to i16
  %347 = load i16, ptr %88, !tbaa !2
  %348 = mul i16 %346, %347
  %349 = sub i16 %334, %348
  %350 = mul i16 %349, 100
  %351 = sub i16 %333, -180
  %352 = getelementptr inbounds i16, ptr @"DT100%", i16 %351
  store i16 %350, ptr %352, !tbaa !2
  %353 = load i16, ptr %93, !tbaa !2
  %354 = load i16, ptr %84, !tbaa !2
  %355 = add nsw i16 %353, %354
  store i16 %355, ptr %93, !tbaa !2
  br label %b28

b41:
  %356 = load i16, ptr %82, !tbaa !2
  %357 = icmp sge i16 %356, 0
  br i1 %357, label %b42, label %b43

b42:
  %358 = load i16, ptr %93, !tbaa !2
  %359 = load i16, ptr %83, !tbaa !2
  %360 = icmp sle i16 %358, %359
  br i1 %360, label %b44, label %b45

b43:
  %361 = load i16, ptr %93, !tbaa !2
  %362 = load i16, ptr %83, !tbaa !2
  %363 = icmp sge i16 %361, %362
  br i1 %363, label %b44, label %b45

b44:
  %364 = load i16, ptr %93, !tbaa !2
  %365 = load i16, ptr %93, !tbaa !2
  store i16 %365, ptr %81, !tbaa !2
  %366 = load i16, ptr %81, !tbaa !2
  %367 = sitofp i16 %366 to float
  store i16 16, ptr %80, !tbaa !2
  %368 = load i16, ptr %80, !tbaa !2
  %369 = sitofp i16 %368 to float
  %370 = fdiv float %367, %369
  %371 = call float @llvm.fabs.f32(float %370)
  %372 = load float, ptr @$float86, !tbaa !2
  %373 = fcmp ogt float %371, %372
  br i1 %373, label %b46, label %b47

b45:
  store i16 -640, ptr %93, !tbaa !2
  store i16 715, ptr %78, !tbaa !2
  store i16 1, ptr %77, !tbaa !2
  br label %b48

b46:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b47:
  %374 = call float @llvm.sin.f32(float %370)
  store i16 31, ptr %79, !tbaa !2
  %375 = load i16, ptr %79, !tbaa !2
  %376 = sitofp i16 %375 to float
  %377 = fmul float %374, %376
  %378 = call i16 @llvm.lrint.i16.f32(float %377)
  %379 = sub i16 %364, -640
  %380 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %379
  store i16 %378, ptr %380, !tbaa !2
  %381 = load i16, ptr %93, !tbaa !2
  %382 = load i16, ptr %82, !tbaa !2
  %383 = add nsw i16 %381, %382
  store i16 %383, ptr %93, !tbaa !2
  br label %b41

b48:
  %384 = load i16, ptr %77, !tbaa !2
  %385 = icmp sge i16 %384, 0
  br i1 %385, label %b49, label %b50

b49:
  %386 = load i16, ptr %93, !tbaa !2
  %387 = load i16, ptr %78, !tbaa !2
  %388 = icmp sle i16 %386, %387
  br i1 %388, label %b51, label %b52

b50:
  %389 = load i16, ptr %93, !tbaa !2
  %390 = load i16, ptr %78, !tbaa !2
  %391 = icmp sge i16 %389, %390
  br i1 %391, label %b51, label %b52

b51:
  %392 = load i16, ptr %93, !tbaa !2
  %393 = load i16, ptr %93, !tbaa !2
  store i16 %393, ptr %76, !tbaa !2
  %394 = load i16, ptr %76, !tbaa !2
  %395 = sitofp i16 %394 to float
  store i16 63, ptr %75, !tbaa !2
  %396 = load i16, ptr %75, !tbaa !2
  %397 = sitofp i16 %396 to float
  %398 = fdiv float %395, %397
  %399 = call float @llvm.fabs.f32(float %398)
  %400 = load float, ptr @$float86, !tbaa !2
  %401 = fcmp ogt float %399, %400
  br i1 %401, label %b53, label %b54

b52:
  store i16 -168, ptr %93, !tbaa !2
  store i16 168, ptr %73, !tbaa !2
  store i16 1, ptr %72, !tbaa !2
  br label %b55

b53:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b54:
  %402 = call float @llvm.sin.f32(float %398)
  store i16 75, ptr %74, !tbaa !2
  %403 = load i16, ptr %74, !tbaa !2
  %404 = sitofp i16 %403 to float
  %405 = fmul float %402, %404
  %406 = call i16 @llvm.lrint.i16.f32(float %405)
  %407 = sub i16 %392, -640
  %408 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %407
  store i16 %406, ptr %408, !tbaa !2
  %409 = load i16, ptr %93, !tbaa !2
  %410 = load i16, ptr %77, !tbaa !2
  %411 = add nsw i16 %409, %410
  store i16 %411, ptr %93, !tbaa !2
  br label %b48

b55:
  %412 = load i16, ptr %72, !tbaa !2
  %413 = icmp sge i16 %412, 0
  br i1 %413, label %b56, label %b57

b56:
  %414 = load i16, ptr %93, !tbaa !2
  %415 = load i16, ptr %73, !tbaa !2
  %416 = icmp sle i16 %414, %415
  br i1 %416, label %b58, label %b59

b57:
  %417 = load i16, ptr %93, !tbaa !2
  %418 = load i16, ptr %73, !tbaa !2
  %419 = icmp sge i16 %417, %418
  br i1 %419, label %b58, label %b59

b58:
  %420 = load i16, ptr %93, !tbaa !2
  %421 = load i16, ptr %93, !tbaa !2
  %422 = add i16 %421, 512
  %423 = sext i16 %422 to i32
  %424 = srem i32 %423, 256
  %425 = trunc i32 %424 to i16
  %426 = sext i16 %425 to i32
  %427 = sdiv i32 %426, 2
  %428 = trunc i32 %427 to i16
  %429 = sub i16 %420, -168
  %430 = getelementptr inbounds i16, ptr @"MOD256128%", i16 %429
  store i16 %428, ptr %430, !tbaa !2
  %431 = load i16, ptr %93, !tbaa !2
  %432 = load i16, ptr %72, !tbaa !2
  %433 = add nsw i16 %431, %432
  store i16 %433, ptr %93, !tbaa !2
  br label %b55

b59:
  store i16 91, ptr %70, !tbaa !2
  %434 = load i16, ptr %70, !tbaa !2
  %435 = sitofp i16 %434 to float
  %436 = fpround float %435 to float
  store float %436, ptr %71, !tbaa !2
  store i16 1, ptr %68, !tbaa !2
  %437 = load i16, ptr %68, !tbaa !2
  %438 = sitofp i16 %437 to float
  %439 = fpround float %438 to float
  store float %439, ptr %69, !tbaa !2
  store i16 -96, ptr %66, !tbaa !2
  %440 = load i16, ptr %66, !tbaa !2
  %441 = sitofp i16 %440 to float
  %442 = fpround float %441 to float
  store float %442, ptr %67, !tbaa !2
  store i16 0, ptr %64, !tbaa !2
  %443 = load i16, ptr %64, !tbaa !2
  %444 = sitofp i16 %443 to float
  %445 = fpround float %444 to float
  store float %445, ptr %65, !tbaa !2
  store i16 0, ptr %62, !tbaa !2
  %446 = load i16, ptr %62, !tbaa !2
  %447 = sitofp i16 %446 to float
  %448 = fpround float %447 to float
  store float %448, ptr %63, !tbaa !2
  store i16 134, ptr %61, !tbaa !2
  br label %b60

b60:
  %449 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %450 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %449, ptr @$string240)
  %451 = icmp ne i16 %450, 0
  br i1 %451, label %b61, label %b62

b61:
  br label %b60

b62:
  store i16 0, ptr %60, !tbaa !2
  store i16 26, ptr %59, !tbaa !2
  store i16 0, ptr %58, !tbaa !2
  store i16 26, ptr %57, !tbaa !2
  store i16 0, ptr %56, !tbaa !2
  store i16 0, ptr %55, !tbaa !2
  store i16 0, ptr %54, !tbaa !2
  br label %b64

b64:
  %452 = load i16, ptr %56, !tbaa !2
  %453 = add i16 %452, 1
  store i16 %453, ptr %56, !tbaa !2
  %454 = load i16, ptr %55, !tbaa !2
  %455 = add i16 %454, 1
  store i16 %455, ptr %55, !tbaa !2
  %456 = load i16, ptr %54, !tbaa !2
  %457 = add i16 %456, 1
  store i16 %457, ptr %54, !tbaa !2
  %458 = load i16, ptr %55, !tbaa !2
  %459 = icmp eq i16 %458, 101
  br i1 %459, label %b66, label %b67

b66:
  store i16 0, ptr %55, !tbaa !2
  br label %b68

b67:
  br label %b68

b68:
  %460 = load i16, ptr %54, !tbaa !2
  %461 = icmp eq i16 %460, 396
  br i1 %461, label %b69, label %b70

b69:
  store i16 0, ptr %54, !tbaa !2
  br label %b71

b70:
  br label %b71

b71:
  store i16 -1, ptr %53, !tbaa !2
  %462 = load i16, ptr %56, !tbaa !2
  %463 = icmp eq i16 %462, 26
  br i1 %463, label %b72, label %b73

b72:
  %464 = load float, ptr @$float241, !tbaa !2
  store float %464, ptr %63, !tbaa !2
  br label %b74

b73:
  br label %b74

b74:
  %465 = load i16, ptr %56, !tbaa !2
  %466 = icmp eq i16 %465, 102
  br i1 %466, label %b75, label %b76

b75:
  store i16 654, ptr %61, !tbaa !2
  br label %b77

b76:
  br label %b77

b77:
  %467 = load i16, ptr %56, !tbaa !2
  %468 = icmp eq i16 %467, 128
  br i1 %468, label %b78, label %b79

b78:
  store i16 1, ptr %52, !tbaa !2
  br label %b80

b79:
  br label %b80

b80:
  %469 = load i16, ptr %60, !tbaa !2
  %470 = load i16, ptr %59, !tbaa !2
  %471 = icmp slt i16 %469, %470
  br i1 %471, label %b81, label %b82

b81:
  %472 = load i16, ptr %60, !tbaa !2
  %473 = add i16 %472, 1
  store i16 %473, ptr %60, !tbaa !2
  br label %b83

b82:
  br label %b2

b83:
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %51, !tbaa !2
  store i16 255, ptr %50, !tbaa !2
  store i16 10, ptr %49, !tbaa !2
  br label %b85

b85:
  %474 = load i16, ptr %49, !tbaa !2
  %475 = icmp sge i16 %474, 0
  br i1 %475, label %b86, label %b87

b86:
  %476 = load i16, ptr %51, !tbaa !2
  %477 = load i16, ptr %50, !tbaa !2
  %478 = icmp sle i16 %476, %477
  br i1 %478, label %b88, label %b89

b87:
  %479 = load i16, ptr %51, !tbaa !2
  %480 = load i16, ptr %50, !tbaa !2
  %481 = icmp sge i16 %479, %480
  br i1 %481, label %b88, label %b89

b88:
  %482 = load i16, ptr %60, !tbaa !2
  %483 = load i16, ptr %51, !tbaa !2
  %484 = mul i16 %483, 2
  %485 = getelementptr i8, ptr @"R%", i16 2
  %486 = load i16, ptr %485, !tbaa !2
  %487 = getelementptr i8, ptr @"R%", i16 10
  %488 = load i16, ptr %487, !tbaa !2
  %489 = add i16 %488, %484
  %490 = inttoptr i16 %486 to ptr addrspace(2)
  %491 = addrspacecast ptr addrspace(2) %490 to ptr addrspace(1)
  %492 = getelementptr i8, ptr addrspace(1) %491, i16 %489
  %493 = load i16, ptr addrspace(1) %492, !tbaa !60
  %494 = sub i16 %493, 63
  %495 = load i16, ptr %59, !tbaa !2
  store i16 %494, ptr %48, !tbaa !2
  %496 = load i16, ptr %48, !tbaa !2
  %497 = sitofp i16 %496 to float
  store i16 %495, ptr %47, !tbaa !2
  %498 = load i16, ptr %47, !tbaa !2
  %499 = sitofp i16 %498 to float
  %500 = fdiv float %497, %499
  store i16 %482, ptr %46, !tbaa !2
  %501 = load i16, ptr %46, !tbaa !2
  %502 = sitofp i16 %501 to float
  %503 = fmul float %502, %500
  store i16 63, ptr %45, !tbaa !2
  %504 = load i16, ptr %45, !tbaa !2
  %505 = sitofp i16 %504 to float
  %506 = fadd float %505, %503
  %507 = call i16 @llvm.lrint.i16.f32(float %506)
  %508 = trunc i16 %507 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %508)
  %509 = load i16, ptr %60, !tbaa !2
  %510 = load i16, ptr %51, !tbaa !2
  %511 = mul i16 %510, 2
  %512 = getelementptr i8, ptr @"G%", i16 2
  %513 = load i16, ptr %512, !tbaa !2
  %514 = getelementptr i8, ptr @"G%", i16 10
  %515 = load i16, ptr %514, !tbaa !2
  %516 = add i16 %515, %511
  %517 = inttoptr i16 %513 to ptr addrspace(2)
  %518 = addrspacecast ptr addrspace(2) %517 to ptr addrspace(1)
  %519 = getelementptr i8, ptr addrspace(1) %518, i16 %516
  %520 = load i16, ptr addrspace(1) %519, !tbaa !62
  %521 = sub i16 %520, 63
  %522 = load i16, ptr %59, !tbaa !2
  store i16 %521, ptr %44, !tbaa !2
  %523 = load i16, ptr %44, !tbaa !2
  %524 = sitofp i16 %523 to float
  store i16 %522, ptr %43, !tbaa !2
  %525 = load i16, ptr %43, !tbaa !2
  %526 = sitofp i16 %525 to float
  %527 = fdiv float %524, %526
  store i16 %509, ptr %42, !tbaa !2
  %528 = load i16, ptr %42, !tbaa !2
  %529 = sitofp i16 %528 to float
  %530 = fmul float %529, %527
  store i16 63, ptr %41, !tbaa !2
  %531 = load i16, ptr %41, !tbaa !2
  %532 = sitofp i16 %531 to float
  %533 = fadd float %532, %530
  %534 = call i16 @llvm.lrint.i16.f32(float %533)
  %535 = trunc i16 %534 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %535)
  %536 = load i16, ptr %60, !tbaa !2
  %537 = load i16, ptr %51, !tbaa !2
  %538 = mul i16 %537, 2
  %539 = getelementptr i8, ptr @"B%", i16 2
  %540 = load i16, ptr %539, !tbaa !2
  %541 = getelementptr i8, ptr @"B%", i16 10
  %542 = load i16, ptr %541, !tbaa !2
  %543 = add i16 %542, %538
  %544 = inttoptr i16 %540 to ptr addrspace(2)
  %545 = addrspacecast ptr addrspace(2) %544 to ptr addrspace(1)
  %546 = getelementptr i8, ptr addrspace(1) %545, i16 %543
  %547 = load i16, ptr addrspace(1) %546, !tbaa !64
  %548 = sub i16 %547, 63
  %549 = load i16, ptr %59, !tbaa !2
  store i16 %548, ptr %40, !tbaa !2
  %550 = load i16, ptr %40, !tbaa !2
  %551 = sitofp i16 %550 to float
  store i16 %549, ptr %39, !tbaa !2
  %552 = load i16, ptr %39, !tbaa !2
  %553 = sitofp i16 %552 to float
  %554 = fdiv float %551, %553
  store i16 %536, ptr %38, !tbaa !2
  %555 = load i16, ptr %38, !tbaa !2
  %556 = sitofp i16 %555 to float
  %557 = fmul float %556, %554
  store i16 63, ptr %37, !tbaa !2
  %558 = load i16, ptr %37, !tbaa !2
  %559 = sitofp i16 %558 to float
  %560 = fadd float %559, %557
  %561 = call i16 @llvm.lrint.i16.f32(float %560)
  %562 = trunc i16 %561 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %562)
  %563 = load i16, ptr %51, !tbaa !2
  %564 = load i16, ptr %49, !tbaa !2
  %565 = add nsw i16 %563, %564
  store i16 %565, ptr %51, !tbaa !2
  br label %b85

b89:
  br label %b2

b90:
  br label %b3

b91:
  br label %b92

b92:
  %566 = load i16, ptr %58, !tbaa !2
  %567 = load i16, ptr %57, !tbaa !2
  %568 = icmp slt i16 %566, %567
  br i1 %568, label %b94, label %b95

b94:
  %569 = load i16, ptr %58, !tbaa !2
  %570 = add i16 %569, 1
  store i16 %570, ptr %58, !tbaa !2
  br label %b96

b95:
  br label %b6

b96:
  call void @llrm.ia16.out.i8(i16 968, i8 0)
  store i16 0, ptr %51, !tbaa !2
  store i16 255, ptr %36, !tbaa !2
  store i16 10, ptr %35, !tbaa !2
  br label %b98

b98:
  %571 = load i16, ptr %35, !tbaa !2
  %572 = icmp sge i16 %571, 0
  br i1 %572, label %b99, label %b100

b99:
  %573 = load i16, ptr %51, !tbaa !2
  %574 = load i16, ptr %36, !tbaa !2
  %575 = icmp sle i16 %573, %574
  br i1 %575, label %b101, label %b102

b100:
  %576 = load i16, ptr %51, !tbaa !2
  %577 = load i16, ptr %36, !tbaa !2
  %578 = icmp sge i16 %576, %577
  br i1 %578, label %b101, label %b102

b101:
  %579 = load i16, ptr %51, !tbaa !2
  %580 = mul i16 %579, 2
  %581 = getelementptr i8, ptr @"R%", i16 2
  %582 = load i16, ptr %581, !tbaa !2
  %583 = getelementptr i8, ptr @"R%", i16 10
  %584 = load i16, ptr %583, !tbaa !2
  %585 = add i16 %584, %580
  %586 = inttoptr i16 %582 to ptr addrspace(2)
  %587 = addrspacecast ptr addrspace(2) %586 to ptr addrspace(1)
  %588 = getelementptr i8, ptr addrspace(1) %587, i16 %585
  %589 = load i16, ptr addrspace(1) %588, !tbaa !60
  %590 = load i16, ptr %58, !tbaa !2
  %591 = load i16, ptr %51, !tbaa !2
  %592 = mul i16 %591, 2
  %593 = getelementptr i8, ptr @"R%", i16 2
  %594 = load i16, ptr %593, !tbaa !2
  %595 = getelementptr i8, ptr @"R%", i16 10
  %596 = load i16, ptr %595, !tbaa !2
  %597 = add i16 %596, %592
  %598 = inttoptr i16 %594 to ptr addrspace(2)
  %599 = addrspacecast ptr addrspace(2) %598 to ptr addrspace(1)
  %600 = getelementptr i8, ptr addrspace(1) %599, i16 %597
  %601 = load i16, ptr addrspace(1) %600, !tbaa !60
  %602 = sub i16 0, %601
  %603 = load i16, ptr %57, !tbaa !2
  store i16 %602, ptr %34, !tbaa !2
  %604 = load i16, ptr %34, !tbaa !2
  %605 = sitofp i16 %604 to float
  store i16 %603, ptr %33, !tbaa !2
  %606 = load i16, ptr %33, !tbaa !2
  %607 = sitofp i16 %606 to float
  %608 = fdiv float %605, %607
  store i16 %590, ptr %32, !tbaa !2
  %609 = load i16, ptr %32, !tbaa !2
  %610 = sitofp i16 %609 to float
  %611 = fmul float %610, %608
  store i16 %589, ptr %31, !tbaa !2
  %612 = load i16, ptr %31, !tbaa !2
  %613 = sitofp i16 %612 to float
  %614 = fadd float %613, %611
  %615 = call i16 @llvm.lrint.i16.f32(float %614)
  %616 = trunc i16 %615 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %616)
  %617 = load i16, ptr %51, !tbaa !2
  %618 = mul i16 %617, 2
  %619 = getelementptr i8, ptr @"G%", i16 2
  %620 = load i16, ptr %619, !tbaa !2
  %621 = getelementptr i8, ptr @"G%", i16 10
  %622 = load i16, ptr %621, !tbaa !2
  %623 = add i16 %622, %618
  %624 = inttoptr i16 %620 to ptr addrspace(2)
  %625 = addrspacecast ptr addrspace(2) %624 to ptr addrspace(1)
  %626 = getelementptr i8, ptr addrspace(1) %625, i16 %623
  %627 = load i16, ptr addrspace(1) %626, !tbaa !62
  %628 = load i16, ptr %58, !tbaa !2
  %629 = load i16, ptr %51, !tbaa !2
  %630 = mul i16 %629, 2
  %631 = getelementptr i8, ptr @"G%", i16 2
  %632 = load i16, ptr %631, !tbaa !2
  %633 = getelementptr i8, ptr @"G%", i16 10
  %634 = load i16, ptr %633, !tbaa !2
  %635 = add i16 %634, %630
  %636 = inttoptr i16 %632 to ptr addrspace(2)
  %637 = addrspacecast ptr addrspace(2) %636 to ptr addrspace(1)
  %638 = getelementptr i8, ptr addrspace(1) %637, i16 %635
  %639 = load i16, ptr addrspace(1) %638, !tbaa !62
  %640 = sub i16 0, %639
  %641 = load i16, ptr %57, !tbaa !2
  store i16 %640, ptr %30, !tbaa !2
  %642 = load i16, ptr %30, !tbaa !2
  %643 = sitofp i16 %642 to float
  store i16 %641, ptr %29, !tbaa !2
  %644 = load i16, ptr %29, !tbaa !2
  %645 = sitofp i16 %644 to float
  %646 = fdiv float %643, %645
  store i16 %628, ptr %28, !tbaa !2
  %647 = load i16, ptr %28, !tbaa !2
  %648 = sitofp i16 %647 to float
  %649 = fmul float %648, %646
  store i16 %627, ptr %27, !tbaa !2
  %650 = load i16, ptr %27, !tbaa !2
  %651 = sitofp i16 %650 to float
  %652 = fadd float %651, %649
  %653 = call i16 @llvm.lrint.i16.f32(float %652)
  %654 = trunc i16 %653 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %654)
  %655 = load i16, ptr %51, !tbaa !2
  %656 = mul i16 %655, 2
  %657 = getelementptr i8, ptr @"B%", i16 2
  %658 = load i16, ptr %657, !tbaa !2
  %659 = getelementptr i8, ptr @"B%", i16 10
  %660 = load i16, ptr %659, !tbaa !2
  %661 = add i16 %660, %656
  %662 = inttoptr i16 %658 to ptr addrspace(2)
  %663 = addrspacecast ptr addrspace(2) %662 to ptr addrspace(1)
  %664 = getelementptr i8, ptr addrspace(1) %663, i16 %661
  %665 = load i16, ptr addrspace(1) %664, !tbaa !64
  %666 = load i16, ptr %58, !tbaa !2
  %667 = load i16, ptr %51, !tbaa !2
  %668 = mul i16 %667, 2
  %669 = getelementptr i8, ptr @"B%", i16 2
  %670 = load i16, ptr %669, !tbaa !2
  %671 = getelementptr i8, ptr @"B%", i16 10
  %672 = load i16, ptr %671, !tbaa !2
  %673 = add i16 %672, %668
  %674 = inttoptr i16 %670 to ptr addrspace(2)
  %675 = addrspacecast ptr addrspace(2) %674 to ptr addrspace(1)
  %676 = getelementptr i8, ptr addrspace(1) %675, i16 %673
  %677 = load i16, ptr addrspace(1) %676, !tbaa !64
  %678 = sub i16 0, %677
  %679 = load i16, ptr %57, !tbaa !2
  store i16 %678, ptr %26, !tbaa !2
  %680 = load i16, ptr %26, !tbaa !2
  %681 = sitofp i16 %680 to float
  store i16 %679, ptr %25, !tbaa !2
  %682 = load i16, ptr %25, !tbaa !2
  %683 = sitofp i16 %682 to float
  %684 = fdiv float %681, %683
  store i16 %666, ptr %24, !tbaa !2
  %685 = load i16, ptr %24, !tbaa !2
  %686 = sitofp i16 %685 to float
  %687 = fmul float %686, %684
  store i16 %665, ptr %23, !tbaa !2
  %688 = load i16, ptr %23, !tbaa !2
  %689 = sitofp i16 %688 to float
  %690 = fadd float %689, %687
  %691 = call i16 @llvm.lrint.i16.f32(float %690)
  %692 = trunc i16 %691 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %692)
  %693 = load i16, ptr %51, !tbaa !2
  %694 = load i16, ptr %35, !tbaa !2
  %695 = add nsw i16 %693, %694
  store i16 %695, ptr %51, !tbaa !2
  br label %b98

b102:
  br label %b3

b103:
  %696 = load float, ptr %69, !tbaa !2
  %697 = fneg float %696
  %698 = fpround float %697 to float
  store float %698, ptr %69, !tbaa !2
  br label %b105

b104:
  br label %b105

b105:
  %699 = load float, ptr %65, !tbaa !2
  %700 = load float, ptr %63, !tbaa !2
  %701 = fadd float %699, %700
  %702 = fpround float %701 to float
  store float %702, ptr %65, !tbaa !2
  %703 = load float, ptr %67, !tbaa !2
  %704 = load float, ptr %65, !tbaa !2
  %705 = fadd float %703, %704
  %706 = fpround float %705 to float
  store float %706, ptr %67, !tbaa !2
  %707 = load float, ptr %67, !tbaa !2
  %708 = load i16, ptr %61, !tbaa !2
  store i16 %708, ptr %20, !tbaa !2
  %709 = load i16, ptr %20, !tbaa !2
  %710 = sitofp i16 %709 to float
  %711 = fcmp ogt float %707, %710
  br i1 %711, label %b106, label %b107

b106:
  %712 = load float, ptr %67, !tbaa !2
  %713 = load float, ptr %65, !tbaa !2
  %714 = fsub float %712, %713
  %715 = fpround float %714 to float
  store float %715, ptr %67, !tbaa !2
  %716 = load float, ptr %65, !tbaa !2
  %717 = fneg float %716
  %718 = load float, ptr @$float242, !tbaa !2
  %719 = fmul float %717, %718
  %720 = fpround float %719 to float
  store float %720, ptr %65, !tbaa !2
  br label %b108

b107:
  br label %b108

b108:
  %721 = load float, ptr %71, !tbaa !2
  %722 = call i16 @llvm.lrint.i16.f32(float %721)
  store i16 %722, ptr %19, !tbaa !2
  %723 = load float, ptr %67, !tbaa !2
  %724 = call i16 @llvm.lrint.i16.f32(float %723)
  store i16 %724, ptr %18, !tbaa !2
  %725 = load i16, ptr %18, !tbaa !2
  %726 = sub i16 %725, 63
  store i16 %726, ptr %17, !tbaa !2
  %727 = load i16, ptr %19, !tbaa !2
  %728 = sub i16 %727, 63
  store i16 %728, ptr %16, !tbaa !2
  %729 = load i16, ptr %17, !tbaa !2
  %730 = icmp slt i16 %729, 0
  %731 = sext i1 %730 to i16
  %732 = load i16, ptr %17, !tbaa !2
  %733 = icmp sgt i16 %732, -128
  %734 = sext i1 %733 to i16
  %735 = and i16 %731, %734
  %736 = icmp ne i16 %735, 0
  br i1 %736, label %b109, label %b110

b109:
  %737 = load i16, ptr %17, !tbaa !2
  %738 = mul i16 %737, -128
  %739 = sub i16 %738, 1
  store i16 %739, ptr %53, !tbaa !2
  br label %b111

b110:
  br label %b111

b111:
  %740 = load i16, ptr %17, !tbaa !2
  %741 = add i16 -1, %740
  store i16 %741, ptr %15, !tbaa !2
  %742 = load i16, ptr %17, !tbaa !2
  %743 = add i16 128, %742
  store i16 %743, ptr %14, !tbaa !2
  %744 = load i16, ptr %16, !tbaa !2
  %745 = add i16 -1, %744
  store i16 %745, ptr %13, !tbaa !2
  %746 = load i16, ptr %16, !tbaa !2
  %747 = add i16 128, %746
  store i16 %747, ptr %12, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  store i16 199, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  br label %b112

b112:
  %748 = load i16, ptr %9, !tbaa !2
  %749 = icmp sge i16 %748, 0
  br i1 %749, label %b113, label %b114

b113:
  %750 = load i16, ptr %11, !tbaa !2
  %751 = load i16, ptr %10, !tbaa !2
  %752 = icmp sle i16 %750, %751
  br i1 %752, label %b115, label %b116

b114:
  %753 = load i16, ptr %11, !tbaa !2
  %754 = load i16, ptr %10, !tbaa !2
  %755 = icmp sge i16 %753, %754
  br i1 %755, label %b115, label %b116

b115:
  %756 = load i16, ptr %11, !tbaa !2
  %757 = mul i16 %756, 20
  %758 = add i16 -24576, %757
  store i16 %758, ptr @b$seg, !tbaa !2
  %759 = load i16, ptr %11, !tbaa !2
  %760 = load i16, ptr %15, !tbaa !2
  %761 = icmp sgt i16 %759, %760
  %762 = sext i1 %761 to i16
  %763 = load i16, ptr %11, !tbaa !2
  %764 = load i16, ptr %14, !tbaa !2
  %765 = icmp slt i16 %763, %764
  %766 = sext i1 %765 to i16
  %767 = and i16 %762, %766
  %768 = icmp ne i16 %767, 0
  br i1 %768, label %b117, label %b118

b116:
  %769 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %769, ptr @"BENCHFRAME&", !tbaa !2
  %770 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %770, ptr @"BENCHFRAME&", !tbaa !2
  %771 = load i16, ptr @"XIT%", !tbaa !2
  %772 = icmp eq i16 %771, 0
  br i1 %772, label %b138, label %b139

b117:
  br label %b4

b118:
  br label %b119

b119:
  store i16 0, ptr %8, !tbaa !2
  store i16 319, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b121

b121:
  %773 = load i16, ptr %6, !tbaa !2
  %774 = icmp sge i16 %773, 0
  br i1 %774, label %b122, label %b123

b122:
  %775 = load i16, ptr %8, !tbaa !2
  %776 = load i16, ptr %7, !tbaa !2
  %777 = icmp sle i16 %775, %776
  br i1 %777, label %b124, label %b125

b123:
  %778 = load i16, ptr %8, !tbaa !2
  %779 = load i16, ptr %7, !tbaa !2
  %780 = icmp sge i16 %778, %779
  br i1 %780, label %b124, label %b125

b124:
  %781 = load i16, ptr %8, !tbaa !2
  %782 = sub i16 %781, -640
  %783 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %782
  %784 = load i16, ptr %783, !tbaa !2
  %785 = load i16, ptr %11, !tbaa !2
  %786 = load i16, ptr %54, !tbaa !2
  %787 = add i16 %785, %786
  %788 = sub i16 %787, -640
  %789 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %788
  %790 = load i16, ptr %789, !tbaa !2
  %791 = add i16 %784, %790
  %792 = load i16, ptr %8, !tbaa !2
  %793 = load i16, ptr %11, !tbaa !2
  %794 = add i16 %792, %793
  %795 = sub i16 %794, -640
  %796 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %795
  %797 = load i16, ptr %796, !tbaa !2
  %798 = add i16 %791, %797
  %799 = load i16, ptr %8, !tbaa !2
  %800 = load i16, ptr %54, !tbaa !2
  %801 = add i16 %799, %800
  %802 = sub i16 %801, -640
  %803 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %802
  %804 = load i16, ptr %803, !tbaa !2
  %805 = load i16, ptr %8, !tbaa !2
  %806 = load i16, ptr %11, !tbaa !2
  %807 = sub i16 %805, %806
  %808 = load i16, ptr %55, !tbaa !2
  %809 = sub i16 %807, %808
  %810 = sub i16 %809, -640
  %811 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %810
  %812 = load i16, ptr %811, !tbaa !2
  %813 = sub i16 %804, %812
  %814 = sub i16 %813, -640
  %815 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %814
  %816 = load i16, ptr %815, !tbaa !2
  %817 = add i16 %798, %816
  store i16 %817, ptr %5, !tbaa !2
  %818 = load i16, ptr %8, !tbaa !2
  %819 = load i16, ptr %5, !tbaa !2
  %820 = sub i16 %819, -168
  %821 = getelementptr inbounds i16, ptr @"MOD256128%", i16 %820
  %822 = load i16, ptr %821, !tbaa !2
  %823 = trunc i16 %822 to i8
  %824 = load i16, ptr @b$seg, !tbaa !2
  %825 = inttoptr i16 %824 to ptr addrspace(2)
  %826 = addrspacecast ptr addrspace(2) %825 to ptr addrspace(1)
  %827 = getelementptr i8, ptr addrspace(1) %826, i16 %818
  store volatile i8 %823, ptr addrspace(1) %827
  %828 = load i16, ptr %8, !tbaa !2
  %829 = load i16, ptr %6, !tbaa !2
  %830 = add nsw i16 %828, %829
  store i16 %830, ptr %8, !tbaa !2
  br label %b121

b125:
  br label %b5

b127:
  %831 = load i16, ptr %3, !tbaa !2
  %832 = icmp sge i16 %831, 0
  br i1 %832, label %b128, label %b129

b128:
  %833 = load i16, ptr %8, !tbaa !2
  %834 = load i16, ptr %4, !tbaa !2
  %835 = icmp sle i16 %833, %834
  br i1 %835, label %b130, label %b131

b129:
  %836 = load i16, ptr %8, !tbaa !2
  %837 = load i16, ptr %4, !tbaa !2
  %838 = icmp sge i16 %836, %837
  br i1 %838, label %b130, label %b131

b130:
  %839 = load i16, ptr %8, !tbaa !2
  %840 = load i16, ptr %13, !tbaa !2
  %841 = icmp sgt i16 %839, %840
  %842 = sext i1 %841 to i16
  %843 = load i16, ptr %8, !tbaa !2
  %844 = load i16, ptr %12, !tbaa !2
  %845 = icmp slt i16 %843, %844
  %846 = sext i1 %845 to i16
  %847 = and i16 %842, %846
  %848 = icmp ne i16 %847, 0
  br i1 %848, label %b132, label %b133

b131:
  br label %b5

b132:
  %849 = load i16, ptr %53, !tbaa !2
  %850 = add i16 %849, 1
  store i16 %850, ptr %53, !tbaa !2
  %851 = load i16, ptr %53, !tbaa !2
  %852 = sub i16 %851, 0
  %853 = getelementptr inbounds i16, ptr @"SP%", i16 %852
  %854 = load i16, ptr %853, !tbaa !2
  %855 = sext i16 %854 to i32
  %856 = sdiv i32 %855, 128
  %857 = trunc i32 %856 to i16
  store i16 %857, ptr %2, !tbaa !2
  %858 = load i16, ptr %53, !tbaa !2
  %859 = sub i16 %858, 0
  %860 = getelementptr inbounds i16, ptr @"SP%", i16 %859
  %861 = load i16, ptr %860, !tbaa !2
  %862 = load i16, ptr %2, !tbaa !2
  %863 = mul i16 %862, 128
  %864 = sub i16 %861, %863
  %865 = load i16, ptr %16, !tbaa !2
  %866 = add i16 %864, %865
  store i16 %866, ptr %1, !tbaa !2
  %867 = load i16, ptr %2, !tbaa !2
  %868 = load i16, ptr %17, !tbaa !2
  %869 = add i16 %867, %868
  store i16 %869, ptr %2, !tbaa !2
  store i16 128, ptr %0, !tbaa !2
  br label %b134

b133:
  %870 = load i16, ptr %8, !tbaa !2
  store i16 %870, ptr %1, !tbaa !2
  %871 = load i16, ptr %11, !tbaa !2
  store i16 %871, ptr %2, !tbaa !2
  br label %b134

b134:
  %872 = load i16, ptr %1, !tbaa !2
  %873 = load i16, ptr %8, !tbaa !2
  %874 = icmp eq i16 %872, %873
  %875 = sext i1 %874 to i16
  %876 = load i16, ptr %2, !tbaa !2
  %877 = load i16, ptr %11, !tbaa !2
  %878 = icmp eq i16 %876, %877
  %879 = sext i1 %878 to i16
  %880 = and i16 %875, %879
  %881 = icmp ne i16 %880, 0
  br i1 %881, label %b135, label %b136

b135:
  store i16 0, ptr %0, !tbaa !2
  br label %b137

b136:
  br label %b137

b137:
  %882 = load i16, ptr %1, !tbaa !2
  %883 = sub i16 %882, -640
  %884 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %883
  %885 = load i16, ptr %884, !tbaa !2
  %886 = load i16, ptr %2, !tbaa !2
  %887 = load i16, ptr %54, !tbaa !2
  %888 = add i16 %886, %887
  %889 = sub i16 %888, -640
  %890 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %889
  %891 = load i16, ptr %890, !tbaa !2
  %892 = add i16 %885, %891
  %893 = load i16, ptr %1, !tbaa !2
  %894 = load i16, ptr %2, !tbaa !2
  %895 = add i16 %893, %894
  %896 = sub i16 %895, -640
  %897 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %896
  %898 = load i16, ptr %897, !tbaa !2
  %899 = add i16 %892, %898
  %900 = load i16, ptr %1, !tbaa !2
  %901 = load i16, ptr %54, !tbaa !2
  %902 = add i16 %900, %901
  %903 = sub i16 %902, -640
  %904 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %903
  %905 = load i16, ptr %904, !tbaa !2
  %906 = load i16, ptr %1, !tbaa !2
  %907 = load i16, ptr %2, !tbaa !2
  %908 = sub i16 %906, %907
  %909 = load i16, ptr %55, !tbaa !2
  %910 = sub i16 %908, %909
  %911 = sub i16 %910, -640
  %912 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %911
  %913 = load i16, ptr %912, !tbaa !2
  %914 = sub i16 %905, %913
  %915 = sub i16 %914, -640
  %916 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %915
  %917 = load i16, ptr %916, !tbaa !2
  %918 = add i16 %899, %917
  store i16 %918, ptr %5, !tbaa !2
  %919 = load i16, ptr %8, !tbaa !2
  %920 = load i16, ptr %5, !tbaa !2
  %921 = sub i16 %920, -168
  %922 = getelementptr inbounds i16, ptr @"MOD256128%", i16 %921
  %923 = load i16, ptr %922, !tbaa !2
  %924 = load i16, ptr %0, !tbaa !2
  %925 = add i16 %923, %924
  %926 = trunc i16 %925 to i8
  %927 = load i16, ptr @b$seg, !tbaa !2
  %928 = inttoptr i16 %927 to ptr addrspace(2)
  %929 = addrspacecast ptr addrspace(2) %928 to ptr addrspace(1)
  %930 = getelementptr i8, ptr addrspace(1) %929, i16 %919
  store volatile i8 %926, ptr addrspace(1) %930
  %931 = load i16, ptr %8, !tbaa !2
  %932 = load i16, ptr %3, !tbaa !2
  %933 = add nsw i16 %931, %932
  store i16 %933, ptr %8, !tbaa !2
  br label %b127

b138:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b140

b139:
  br label %b140

b140:
  %934 = load i16, ptr @"XIT%", !tbaa !2
  %935 = icmp eq i16 %934, 1
  br i1 %935, label %b141, label %b142

b141:
  br label %b6

b142:
  br label %b143

b143:
  br label %b64
}

define cc 1000 void @SUCKING() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca float
  %1 = alloca i16
  %2 = alloca float
  %3 = alloca i16
  %4 = alloca float
  %5 = alloca i16
  %6 = alloca float
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 15, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 1, i16 1, i16 1, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string243)
  store i16 7, ptr %17, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store i16 -1, ptr %15, !tbaa !2
  br label %b2

b2:
  %18 = load i16, ptr %15, !tbaa !2
  %19 = icmp sge i16 %18, 0
  br i1 %19, label %b3, label %b4

b3:
  %20 = load i16, ptr %17, !tbaa !2
  %21 = load i16, ptr %16, !tbaa !2
  %22 = icmp sle i16 %20, %21
  br i1 %22, label %b5, label %b6

b4:
  %23 = load i16, ptr %17, !tbaa !2
  %24 = load i16, ptr %16, !tbaa !2
  %25 = icmp sge i16 %23, %24
  br i1 %25, label %b5, label %b6

b5:
  store i16 111, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  store i16 -1, ptr %12, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %26 = load i16, ptr %12, !tbaa !2
  %27 = icmp sge i16 %26, 0
  br i1 %27, label %b8, label %b9

b8:
  %28 = load i16, ptr %14, !tbaa !2
  %29 = load i16, ptr %13, !tbaa !2
  %30 = icmp sle i16 %28, %29
  br i1 %30, label %b10, label %b11

b9:
  %31 = load i16, ptr %14, !tbaa !2
  %32 = load i16, ptr %13, !tbaa !2
  %33 = icmp sge i16 %31, %32
  br i1 %33, label %b10, label %b11

b10:
  %34 = load i16, ptr %14, !tbaa !2
  %35 = load i16, ptr %17, !tbaa !2
  %36 = call cc 1000 addrspace(1) i16 @llrm.qb.B$PNI2(i16 %34, i16 %35)
  %37 = load i16, ptr %17, !tbaa !2
  %38 = add i16 %37, 1
  %39 = mul i16 %36, %38
  %40 = load i16, ptr %14, !tbaa !2
  %41 = load i16, ptr %17, !tbaa !2
  %42 = add i16 %40, %41
  store i16 %42, ptr %10, !tbaa !2
  %43 = load i16, ptr %10, !tbaa !2
  %44 = sitofp i16 %43 to float
  store i16 45, ptr %9, !tbaa !2
  %45 = load i16, ptr %9, !tbaa !2
  %46 = sitofp i16 %45 to float
  %47 = fdiv float %44, %46
  %48 = call float @llvm.fabs.f32(float %47)
  %49 = load float, ptr @$float86, !tbaa !2
  %50 = fcmp ogt float %48, %49
  br i1 %50, label %b12, label %b13

b11:
  %51 = load i16, ptr %17, !tbaa !2
  %52 = load i16, ptr %15, !tbaa !2
  %53 = add nsw i16 %51, %52
  store i16 %53, ptr %17, !tbaa !2
  br label %b2

b12:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b13:
  %54 = call float @llvm.sin.f32(float %47)
  store i16 %39, ptr %8, !tbaa !2
  %55 = load i16, ptr %8, !tbaa !2
  %56 = sitofp i16 %55 to float
  %57 = fmul float %56, %54
  %58 = call i16 @llvm.lrint.i16.f32(float %57)
  store i16 %58, ptr %11, !tbaa !2
  %59 = load i16, ptr %14, !tbaa !2
  %60 = load float, ptr @$float244, !tbaa !2
  store i16 %59, ptr %7, !tbaa !2
  %61 = load i16, ptr %7, !tbaa !2
  %62 = sitofp i16 %61 to float
  %63 = fmul float %62, %60
  %64 = load i16, ptr %17, !tbaa !2
  %65 = mul i16 %64, 16
  %66 = add i16 %65, 32
  %67 = fpround float %63 to float
  store float %67, ptr %6, !tbaa !2
  store i16 %66, ptr %5, !tbaa !2
  %68 = load i16, ptr %5, !tbaa !2
  %69 = sitofp i16 %68 to float
  %70 = fpround float %69 to float
  store float %70, ptr %4, !tbaa !2
  %71 = load float, ptr %6, !tbaa !2
  %72 = load float, ptr %4, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N1R4(float %71, float %72)
  %73 = load i16, ptr %14, !tbaa !2
  %74 = add i16 %73, 1
  %75 = load float, ptr @$float244, !tbaa !2
  store i16 %74, ptr %3, !tbaa !2
  %76 = load i16, ptr %3, !tbaa !2
  %77 = sitofp i16 %76 to float
  %78 = fmul float %77, %75
  %79 = load i16, ptr %17, !tbaa !2
  %80 = add i16 %79, 1
  %81 = mul i16 %80, 16
  %82 = add i16 %81, 32
  %83 = fpround float %78 to float
  store float %83, ptr %2, !tbaa !2
  store i16 %82, ptr %1, !tbaa !2
  %84 = load i16, ptr %1, !tbaa !2
  %85 = sitofp i16 %84 to float
  %86 = fpround float %85 to float
  store float %86, ptr %0, !tbaa !2
  %87 = load float, ptr %2, !tbaa !2
  %88 = load float, ptr %0, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N2R4(float %87, float %88)
  %89 = load i16, ptr %11, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %89, i16 -1, i16 2)
  %90 = load i16, ptr %14, !tbaa !2
  %91 = load i16, ptr %12, !tbaa !2
  %92 = add nsw i16 %90, %91
  store i16 %92, ptr %14, !tbaa !2
  br label %b7
}

define cc 1000 void @TRANSLATE3D() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  %7 = load i16, ptr @"OBJ%", !tbaa !2
  %8 = icmp eq i16 %7, 1
  br i1 %8, label %b3, label %b4

b2:
  %9 = load i16, ptr %5, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = add nsw i16 %9, %10
  store i16 %11, ptr %5, !tbaa !2
  br label %b15

b3:
  store i16 1, ptr %6, !tbaa !2
  br label %b5

b4:
  br label %b5

b5:
  %12 = load i16, ptr @"OBJ%", !tbaa !2
  %13 = icmp eq i16 %12, 2
  %14 = sext i1 %13 to i16
  %15 = load i16, ptr @"OBJ%", !tbaa !2
  %16 = icmp eq i16 %15, 3
  %17 = sext i1 %16 to i16
  %18 = or i16 %14, %17
  %19 = icmp ne i16 %18, 0
  br i1 %19, label %b6, label %b7

b6:
  store i16 4, ptr %6, !tbaa !2
  br label %b8

b7:
  br label %b8

b8:
  %20 = load i16, ptr @"OBJ%", !tbaa !2
  %21 = icmp eq i16 %20, 4
  br i1 %21, label %b9, label %b10

b9:
  store i16 16, ptr %6, !tbaa !2
  br label %b11

b10:
  br label %b11

b11:
  %22 = load i16, ptr @"OBJ%", !tbaa !2
  %23 = icmp sgt i16 %22, 4
  br i1 %23, label %b12, label %b13

b12:
  store i16 8, ptr %6, !tbaa !2
  br label %b14

b13:
  br label %b14

b14:
  store i16 0, ptr %5, !tbaa !2
  %24 = load i16, ptr @"NDTS%", !tbaa !2
  %25 = sub i16 %24, 1
  store i16 %25, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b15

b15:
  %26 = load i16, ptr %3, !tbaa !2
  %27 = icmp sge i16 %26, 0
  br i1 %27, label %b16, label %b17

b16:
  %28 = load i16, ptr %5, !tbaa !2
  %29 = load i16, ptr %4, !tbaa !2
  %30 = icmp sle i16 %28, %29
  br i1 %30, label %b18, label %b19

b17:
  %31 = load i16, ptr %5, !tbaa !2
  %32 = load i16, ptr %4, !tbaa !2
  %33 = icmp sge i16 %31, %32
  br i1 %33, label %b18, label %b19

b18:
  %34 = load i16, ptr %5, !tbaa !2
  %35 = add i16 4097, %34
  %36 = mul i16 %35, 2
  %37 = getelementptr i8, ptr @"XS%", i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = add i16 0, %36
  %40 = inttoptr i16 %38 to ptr addrspace(2)
  %41 = addrspacecast ptr addrspace(2) %40 to ptr addrspace(1)
  %42 = getelementptr i8, ptr addrspace(1) %41, i16 %39
  %43 = load i16, ptr %5, !tbaa !2
  %44 = add i16 0, %43
  %45 = mul i16 %44, 2
  %46 = getelementptr i8, ptr @"XS%", i16 2
  %47 = load i16, ptr %46, !tbaa !2
  %48 = add i16 0, %45
  %49 = inttoptr i16 %47 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = getelementptr i8, ptr addrspace(1) %50, i16 %48
  %52 = load i16, ptr addrspace(1) %51, !tbaa !66
  store i16 %52, ptr addrspace(1) %42, !tbaa !66
  %53 = load i16, ptr %5, !tbaa !2
  %54 = add i16 4097, %53
  %55 = mul i16 %54, 2
  %56 = getelementptr i8, ptr @"YS%", i16 2
  %57 = load i16, ptr %56, !tbaa !2
  %58 = add i16 0, %55
  %59 = inttoptr i16 %57 to ptr addrspace(2)
  %60 = addrspacecast ptr addrspace(2) %59 to ptr addrspace(1)
  %61 = getelementptr i8, ptr addrspace(1) %60, i16 %58
  %62 = load i16, ptr %5, !tbaa !2
  %63 = add i16 0, %62
  %64 = mul i16 %63, 2
  %65 = getelementptr i8, ptr @"YS%", i16 2
  %66 = load i16, ptr %65, !tbaa !2
  %67 = add i16 0, %64
  %68 = inttoptr i16 %66 to ptr addrspace(2)
  %69 = addrspacecast ptr addrspace(2) %68 to ptr addrspace(1)
  %70 = getelementptr i8, ptr addrspace(1) %69, i16 %67
  %71 = load i16, ptr addrspace(1) %70, !tbaa !68
  store i16 %71, ptr addrspace(1) %61, !tbaa !68
  %72 = load i16, ptr %5, !tbaa !2
  %73 = mul i16 %72, 4
  %74 = getelementptr i8, ptr @"Z!", i16 2
  %75 = load i16, ptr %74, !tbaa !2
  %76 = add i16 0, %73
  %77 = inttoptr i16 %75 to ptr addrspace(2)
  %78 = addrspacecast ptr addrspace(2) %77 to ptr addrspace(1)
  %79 = getelementptr i8, ptr addrspace(1) %78, i16 %76
  %80 = load float, ptr addrspace(1) %79, !tbaa !56
  %81 = load i16, ptr %6, !tbaa !2
  store i16 %81, ptr %2, !tbaa !2
  %82 = load i16, ptr %2, !tbaa !2
  %83 = sitofp i16 %82 to float
  %84 = fcmp ole float %80, %83
  br i1 %84, label %b20, label %b21

b19:
  ret void

b20:
  br label %b2

b21:
  br label %b22

b22:
  %85 = load i16, ptr %5, !tbaa !2
  %86 = add i16 0, %85
  %87 = mul i16 %86, 2
  %88 = getelementptr i8, ptr @"XS%", i16 2
  %89 = load i16, ptr %88, !tbaa !2
  %90 = add i16 0, %87
  %91 = inttoptr i16 %89 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 %90
  %94 = load i16, ptr %5, !tbaa !2
  %95 = mul i16 %94, 4
  %96 = getelementptr i8, ptr @"X!", i16 2
  %97 = load i16, ptr %96, !tbaa !2
  %98 = add i16 0, %95
  %99 = inttoptr i16 %97 to ptr addrspace(2)
  %100 = addrspacecast ptr addrspace(2) %99 to ptr addrspace(1)
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 %98
  %102 = load float, ptr addrspace(1) %101, !tbaa !54
  store i16 256, ptr %1, !tbaa !2
  %103 = load i16, ptr %1, !tbaa !2
  %104 = sitofp i16 %103 to float
  %105 = fmul float %102, %104
  %106 = load i16, ptr %5, !tbaa !2
  %107 = mul i16 %106, 4
  %108 = getelementptr i8, ptr @"Z!", i16 2
  %109 = load i16, ptr %108, !tbaa !2
  %110 = add i16 0, %107
  %111 = inttoptr i16 %109 to ptr addrspace(2)
  %112 = addrspacecast ptr addrspace(2) %111 to ptr addrspace(1)
  %113 = getelementptr i8, ptr addrspace(1) %112, i16 %110
  %114 = load float, ptr addrspace(1) %113, !tbaa !56
  %115 = fdiv float %105, %114
  %116 = call i16 @llvm.lrint.i16.f32(float %115)
  store i16 %116, ptr addrspace(1) %93, !tbaa !66
  %117 = load i16, ptr %5, !tbaa !2
  %118 = add i16 0, %117
  %119 = mul i16 %118, 2
  %120 = getelementptr i8, ptr @"YS%", i16 2
  %121 = load i16, ptr %120, !tbaa !2
  %122 = add i16 0, %119
  %123 = inttoptr i16 %121 to ptr addrspace(2)
  %124 = addrspacecast ptr addrspace(2) %123 to ptr addrspace(1)
  %125 = getelementptr i8, ptr addrspace(1) %124, i16 %122
  %126 = load i16, ptr %5, !tbaa !2
  %127 = mul i16 %126, 4
  %128 = getelementptr i8, ptr @"Y!", i16 2
  %129 = load i16, ptr %128, !tbaa !2
  %130 = add i16 0, %127
  %131 = inttoptr i16 %129 to ptr addrspace(2)
  %132 = addrspacecast ptr addrspace(2) %131 to ptr addrspace(1)
  %133 = getelementptr i8, ptr addrspace(1) %132, i16 %130
  %134 = load float, ptr addrspace(1) %133, !tbaa !58
  store i16 256, ptr %0, !tbaa !2
  %135 = load i16, ptr %0, !tbaa !2
  %136 = sitofp i16 %135 to float
  %137 = fmul float %134, %136
  %138 = load i16, ptr %5, !tbaa !2
  %139 = mul i16 %138, 4
  %140 = getelementptr i8, ptr @"Z!", i16 2
  %141 = load i16, ptr %140, !tbaa !2
  %142 = add i16 0, %139
  %143 = inttoptr i16 %141 to ptr addrspace(2)
  %144 = addrspacecast ptr addrspace(2) %143 to ptr addrspace(1)
  %145 = getelementptr i8, ptr addrspace(1) %144, i16 %142
  %146 = load float, ptr addrspace(1) %145, !tbaa !56
  %147 = fdiv float %137, %146
  %148 = call i16 @llvm.lrint.i16.f32(float %147)
  store i16 %148, ptr addrspace(1) %125, !tbaa !68
  br label %b2
}

define cc 1000 void @ZOOMDISTORT() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca i16
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca i16
  %50 = alloca i16
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i16
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca i16
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i16
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca i16
  %70 = alloca [4 x i8]
  %71 = alloca [4 x i8]
  %72 = alloca i16
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca float
  %78 = alloca i16
  %79 = alloca i16
  %80 = alloca i16
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca i16
  %84 = alloca i16
  %85 = alloca i16
  %86 = alloca i16
  %87 = alloca i16
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca i16
  %91 = alloca i16
  %92 = alloca i16
  %93 = alloca i16
  %94 = alloca i16
  %95 = alloca i16
  %96 = alloca i16
  %97 = alloca i16
  %98 = alloca i16
  %99 = alloca i16
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca i16
  %103 = alloca i16
  %104 = alloca [22 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store i16 0, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  store i16 0, ptr %33
  store i16 0, ptr %34
  store i16 0, ptr %35
  store i16 0, ptr %36
  store i16 0, ptr %37
  store i16 0, ptr %38
  store i16 0, ptr %39
  store i16 0, ptr %40
  store i16 0, ptr %41
  store i16 0, ptr %42
  store i16 0, ptr %43
  store i16 0, ptr %44
  store i16 0, ptr %45
  store i16 0, ptr %46
  store i16 0, ptr %47
  store i16 0, ptr %48
  store i16 0, ptr %49
  store i16 0, ptr %50
  store i16 0, ptr %51
  store i16 0, ptr %52
  store i16 0, ptr %53
  store i16 0, ptr %54
  store i16 0, ptr %55
  store i16 0, ptr %56
  store i16 0, ptr %57
  store i16 0, ptr %58
  store i16 0, ptr %59
  store i16 0, ptr %60
  store i16 0, ptr %61
  store i16 0, ptr %62
  store i16 0, ptr %63
  store i16 0, ptr %64
  store i16 0, ptr %65
  store i16 0, ptr %66
  store i16 0, ptr %67
  store i16 0, ptr %68
  store i16 0, ptr %69
  call void @llvm.memset.p0.i16(ptr %70, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %71, i8 0, i16 4, i1 false)
  store i16 0, ptr %72
  store i16 0, ptr %73
  store i16 0, ptr %74
  store i16 0, ptr %75
  store i16 0, ptr %76
  store float 0.000000e+00, ptr %77
  store i16 0, ptr %78
  store i16 0, ptr %79
  store i16 0, ptr %80
  store i16 0, ptr %81
  store i16 0, ptr %82
  store i16 0, ptr %83
  store i16 0, ptr %84
  store i16 0, ptr %85
  store i16 0, ptr %86
  store i16 0, ptr %87
  store i16 0, ptr %88
  store i16 0, ptr %89
  store i16 0, ptr %90
  store i16 0, ptr %91
  store i16 0, ptr %92
  store i16 0, ptr %93
  store i16 0, ptr %94
  store i16 0, ptr %95
  store i16 0, ptr %96
  store i16 0, ptr %97
  store i16 0, ptr %98
  store i16 0, ptr %99
  store i16 0, ptr %100
  store i16 0, ptr %101
  store i16 0, ptr %102
  store i16 0, ptr %103
  call void @llvm.memset.p0.i16(ptr %104, i8 0, i16 22, i1 false)
  store i16 0, ptr %104, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %104, i16 2
  store i16 0, ptr %105, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %104, i16 4
  store i16 0, ptr %106, !tbaa !2
  %107 = getelementptr inbounds i8, ptr %104, i16 6
  store i16 0, ptr %107, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %104, i16 8
  store i16 0, ptr %108, !tbaa !2
  %109 = getelementptr inbounds i8, ptr %104, i16 10
  store i16 0, ptr %109, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %104, i16 12
  store i16 0, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %104, i16 14
  store i16 0, ptr %111, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %104, i16 16
  store i16 0, ptr %112, !tbaa !2
  %113 = getelementptr inbounds i8, ptr %104, i16 18
  store i16 0, ptr %113, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %104, i16 20
  store i16 0, ptr %114, !tbaa !2
  store i16 0, ptr %71, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %71, i16 2
  store i16 0, ptr %115, !tbaa !2
  store i16 0, ptr %70, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %70, i16 2
  store i16 0, ptr %116, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  br label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %71)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %70)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %104)
  ret void

b3:
  %117 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %118 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %117, ptr @$string245)
  %119 = icmp ne i16 %118, 0
  br i1 %119, label %b4, label %b5

b4:
  br label %b3

b5:
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 -640, ptr %103, !tbaa !2
  store i16 518, ptr %102, !tbaa !2
  store i16 1, ptr %101, !tbaa !2
  br label %b6

b6:
  %120 = load i16, ptr %101, !tbaa !2
  %121 = icmp sge i16 %120, 0
  br i1 %121, label %b7, label %b8

b7:
  %122 = load i16, ptr %103, !tbaa !2
  %123 = load i16, ptr %102, !tbaa !2
  %124 = icmp sle i16 %122, %123
  br i1 %124, label %b9, label %b10

b8:
  %125 = load i16, ptr %103, !tbaa !2
  %126 = load i16, ptr %102, !tbaa !2
  %127 = icmp sge i16 %125, %126
  br i1 %127, label %b9, label %b10

b9:
  %128 = load i16, ptr %103, !tbaa !2
  %129 = load i16, ptr %103, !tbaa !2
  store i16 %129, ptr %100, !tbaa !2
  %130 = load i16, ptr %100, !tbaa !2
  %131 = sitofp i16 %130 to float
  store i16 16, ptr %99, !tbaa !2
  %132 = load i16, ptr %99, !tbaa !2
  %133 = sitofp i16 %132 to float
  %134 = fdiv float %131, %133
  %135 = call float @llvm.fabs.f32(float %134)
  %136 = load float, ptr @$float86, !tbaa !2
  %137 = fcmp ogt float %135, %136
  br i1 %137, label %b11, label %b12

b10:
  store i16 -640, ptr %103, !tbaa !2
  store i16 715, ptr %97, !tbaa !2
  store i16 1, ptr %96, !tbaa !2
  br label %b13

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %138 = call float @llvm.sin.f32(float %134)
  store i16 31, ptr %98, !tbaa !2
  %139 = load i16, ptr %98, !tbaa !2
  %140 = sitofp i16 %139 to float
  %141 = fmul float %138, %140
  %142 = call i16 @llvm.lrint.i16.f32(float %141)
  %143 = sub i16 %128, -640
  %144 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %143
  store i16 %142, ptr %144, !tbaa !2
  %145 = load i16, ptr %103, !tbaa !2
  %146 = load i16, ptr %101, !tbaa !2
  %147 = add nsw i16 %145, %146
  store i16 %147, ptr %103, !tbaa !2
  br label %b6

b13:
  %148 = load i16, ptr %96, !tbaa !2
  %149 = icmp sge i16 %148, 0
  br i1 %149, label %b14, label %b15

b14:
  %150 = load i16, ptr %103, !tbaa !2
  %151 = load i16, ptr %97, !tbaa !2
  %152 = icmp sle i16 %150, %151
  br i1 %152, label %b16, label %b17

b15:
  %153 = load i16, ptr %103, !tbaa !2
  %154 = load i16, ptr %97, !tbaa !2
  %155 = icmp sge i16 %153, %154
  br i1 %155, label %b16, label %b17

b16:
  %156 = load i16, ptr %103, !tbaa !2
  %157 = load i16, ptr %103, !tbaa !2
  store i16 %157, ptr %95, !tbaa !2
  %158 = load i16, ptr %95, !tbaa !2
  %159 = sitofp i16 %158 to float
  store i16 63, ptr %94, !tbaa !2
  %160 = load i16, ptr %94, !tbaa !2
  %161 = sitofp i16 %160 to float
  %162 = fdiv float %159, %161
  %163 = call float @llvm.fabs.f32(float %162)
  %164 = load float, ptr @$float86, !tbaa !2
  %165 = fcmp ogt float %163, %164
  br i1 %165, label %b18, label %b19

b17:
  store i16 -48, ptr %103, !tbaa !2
  store i16 539, ptr %92, !tbaa !2
  store i16 1, ptr %91, !tbaa !2
  br label %b20

b18:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b19:
  %166 = call float @llvm.sin.f32(float %162)
  store i16 75, ptr %93, !tbaa !2
  %167 = load i16, ptr %93, !tbaa !2
  %168 = sitofp i16 %167 to float
  %169 = fmul float %166, %168
  %170 = call i16 @llvm.lrint.i16.f32(float %169)
  %171 = sub i16 %156, -640
  %172 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %171
  store i16 %170, ptr %172, !tbaa !2
  %173 = load i16, ptr %103, !tbaa !2
  %174 = load i16, ptr %96, !tbaa !2
  %175 = add nsw i16 %173, %174
  store i16 %175, ptr %103, !tbaa !2
  br label %b13

b20:
  %176 = load i16, ptr %91, !tbaa !2
  %177 = icmp sge i16 %176, 0
  br i1 %177, label %b21, label %b22

b21:
  %178 = load i16, ptr %103, !tbaa !2
  %179 = load i16, ptr %92, !tbaa !2
  %180 = icmp sle i16 %178, %179
  br i1 %180, label %b23, label %b24

b22:
  %181 = load i16, ptr %103, !tbaa !2
  %182 = load i16, ptr %92, !tbaa !2
  %183 = icmp sge i16 %181, %182
  br i1 %183, label %b23, label %b24

b23:
  %184 = load i16, ptr %103, !tbaa !2
  %185 = load i16, ptr %103, !tbaa !2
  store i16 %185, ptr %90, !tbaa !2
  %186 = load i16, ptr %90, !tbaa !2
  %187 = sitofp i16 %186 to float
  store i16 35, ptr %89, !tbaa !2
  %188 = load i16, ptr %89, !tbaa !2
  %189 = sitofp i16 %188 to float
  %190 = fdiv float %187, %189
  %191 = call float @llvm.fabs.f32(float %190)
  %192 = load float, ptr @$float86, !tbaa !2
  %193 = fcmp ogt float %191, %192
  br i1 %193, label %b25, label %b26

b24:
  store i16 -319, ptr %103, !tbaa !2
  store i16 602, ptr %87, !tbaa !2
  store i16 1, ptr %86, !tbaa !2
  br label %b27

b25:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b26:
  %194 = call float @llvm.sin.f32(float %190)
  store i16 24, ptr %88, !tbaa !2
  %195 = load i16, ptr %88, !tbaa !2
  %196 = sitofp i16 %195 to float
  %197 = fmul float %194, %196
  %198 = call i16 @llvm.lrint.i16.f32(float %197)
  %199 = sub i16 %184, -48
  %200 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %199
  store i16 %198, ptr %200, !tbaa !2
  %201 = load i16, ptr %103, !tbaa !2
  %202 = load i16, ptr %91, !tbaa !2
  %203 = add nsw i16 %201, %202
  store i16 %203, ptr %103, !tbaa !2
  br label %b20

b27:
  %204 = load i16, ptr %86, !tbaa !2
  %205 = icmp sge i16 %204, 0
  br i1 %205, label %b28, label %b29

b28:
  %206 = load i16, ptr %103, !tbaa !2
  %207 = load i16, ptr %87, !tbaa !2
  %208 = icmp sle i16 %206, %207
  br i1 %208, label %b30, label %b31

b29:
  %209 = load i16, ptr %103, !tbaa !2
  %210 = load i16, ptr %87, !tbaa !2
  %211 = icmp sge i16 %209, %210
  br i1 %211, label %b30, label %b31

b30:
  %212 = load i16, ptr %103, !tbaa !2
  %213 = load i16, ptr %103, !tbaa !2
  store i16 %213, ptr %85, !tbaa !2
  %214 = load i16, ptr %85, !tbaa !2
  %215 = sitofp i16 %214 to float
  store i16 45, ptr %84, !tbaa !2
  %216 = load i16, ptr %84, !tbaa !2
  %217 = sitofp i16 %216 to float
  %218 = fdiv float %215, %217
  %219 = call float @llvm.fabs.f32(float %218)
  %220 = load float, ptr @$float86, !tbaa !2
  %221 = fcmp ogt float %219, %220
  br i1 %221, label %b32, label %b33

b31:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -160, i16 160, i16 -48, i16 48, i16 2, i16 258, ptr %104)
  store i16 -48, ptr %82, !tbaa !2
  store i16 48, ptr %81, !tbaa !2
  store i16 1, ptr %80, !tbaa !2
  br label %b34

b32:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b33:
  %222 = call float @llvm.sin.f32(float %218)
  store i16 24, ptr %83, !tbaa !2
  %223 = load i16, ptr %83, !tbaa !2
  %224 = sitofp i16 %223 to float
  %225 = fmul float %222, %224
  %226 = call i16 @llvm.lrint.i16.f32(float %225)
  %227 = sub i16 %212, -319
  %228 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %227
  store i16 %226, ptr %228, !tbaa !2
  %229 = load i16, ptr %103, !tbaa !2
  %230 = load i16, ptr %86, !tbaa !2
  %231 = add nsw i16 %229, %230
  store i16 %231, ptr %103, !tbaa !2
  br label %b27

b34:
  %232 = load i16, ptr %80, !tbaa !2
  %233 = icmp sge i16 %232, 0
  br i1 %233, label %b35, label %b36

b35:
  %234 = load i16, ptr %82, !tbaa !2
  %235 = load i16, ptr %81, !tbaa !2
  %236 = icmp sle i16 %234, %235
  br i1 %236, label %b37, label %b38

b36:
  %237 = load i16, ptr %82, !tbaa !2
  %238 = load i16, ptr %81, !tbaa !2
  %239 = icmp sge i16 %237, %238
  br i1 %239, label %b37, label %b38

b37:
  store i16 -160, ptr %103, !tbaa !2
  store i16 160, ptr %79, !tbaa !2
  store i16 1, ptr %78, !tbaa !2
  br label %b39

b38:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string246, ptr %71)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string247, ptr %70)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr %71, i16 1, i16 -1, i16 32)
  call void @llrm.ia16.out.i8(i16 968, i8 -128)
  store i16 0, ptr %103, !tbaa !2
  store i16 383, ptr %69, !tbaa !2
  store i16 1, ptr %68, !tbaa !2
  br label %b47

b39:
  %240 = load i16, ptr %78, !tbaa !2
  %241 = icmp sge i16 %240, 0
  br i1 %241, label %b40, label %b41

b40:
  %242 = load i16, ptr %103, !tbaa !2
  %243 = load i16, ptr %79, !tbaa !2
  %244 = icmp sle i16 %242, %243
  br i1 %244, label %b42, label %b43

b41:
  %245 = load i16, ptr %103, !tbaa !2
  %246 = load i16, ptr %79, !tbaa !2
  %247 = icmp sge i16 %245, %246
  br i1 %247, label %b42, label %b43

b42:
  %248 = load i16, ptr %82, !tbaa !2
  store i16 %248, ptr %76, !tbaa !2
  %249 = load i16, ptr %76, !tbaa !2
  %250 = sitofp i16 %249 to float
  store i16 64, ptr %75, !tbaa !2
  %251 = load i16, ptr %75, !tbaa !2
  %252 = sitofp i16 %251 to float
  %253 = fdiv float %250, %252
  store i16 2, ptr %74, !tbaa !2
  %254 = load i16, ptr %74, !tbaa !2
  %255 = sitofp i16 %254 to float
  %256 = fadd float %253, %255
  %257 = fpround float %256 to float
  store float %257, ptr %77, !tbaa !2
  %258 = load float, ptr %77, !tbaa !2
  store i16 0, ptr %73, !tbaa !2
  %259 = load i16, ptr %73, !tbaa !2
  %260 = sitofp i16 %259 to float
  %261 = fcmp une float %258, %260
  br i1 %261, label %b44, label %b45

b43:
  %262 = load i16, ptr %82, !tbaa !2
  %263 = load i16, ptr %80, !tbaa !2
  %264 = add nsw i16 %262, %263
  store i16 %264, ptr %82, !tbaa !2
  br label %b34

b44:
  %265 = load i16, ptr %103, !tbaa !2
  %266 = load i16, ptr %82, !tbaa !2
  %267 = mul i16 %266, 321
  %268 = add i16 %267, %265
  %269 = mul i16 %268, 2
  %270 = getelementptr i8, ptr %104, i16 2
  %271 = load i16, ptr %270, !tbaa !2
  %272 = add i16 31136, %269
  %273 = inttoptr i16 %271 to ptr addrspace(2)
  %274 = addrspacecast ptr addrspace(2) %273 to ptr addrspace(1)
  %275 = getelementptr i8, ptr addrspace(1) %274, i16 %272
  %276 = load i16, ptr %103, !tbaa !2
  %277 = load float, ptr %77, !tbaa !2
  store i16 %276, ptr %72, !tbaa !2
  %278 = load i16, ptr %72, !tbaa !2
  %279 = sitofp i16 %278 to float
  %280 = fdiv float %279, %277
  %281 = call i16 @llvm.lrint.i16.f32(float %280)
  store i16 %281, ptr addrspace(1) %275, !tbaa !74
  br label %b46

b45:
  %282 = load i16, ptr %103, !tbaa !2
  %283 = load i16, ptr %82, !tbaa !2
  %284 = mul i16 %283, 321
  %285 = add i16 %284, %282
  %286 = mul i16 %285, 2
  %287 = getelementptr i8, ptr %104, i16 2
  %288 = load i16, ptr %287, !tbaa !2
  %289 = add i16 31136, %286
  %290 = inttoptr i16 %288 to ptr addrspace(2)
  %291 = addrspacecast ptr addrspace(2) %290 to ptr addrspace(1)
  %292 = getelementptr i8, ptr addrspace(1) %291, i16 %289
  %293 = load i16, ptr %103, !tbaa !2
  store i16 %293, ptr addrspace(1) %292, !tbaa !74
  br label %b46

b46:
  %294 = load i16, ptr %103, !tbaa !2
  %295 = load i16, ptr %78, !tbaa !2
  %296 = add nsw i16 %294, %295
  store i16 %296, ptr %103, !tbaa !2
  br label %b39

b47:
  %297 = load i16, ptr %68, !tbaa !2
  %298 = icmp sge i16 %297, 0
  br i1 %298, label %b48, label %b49

b48:
  %299 = load i16, ptr %103, !tbaa !2
  %300 = load i16, ptr %69, !tbaa !2
  %301 = icmp sle i16 %299, %300
  br i1 %301, label %b50, label %b51

b49:
  %302 = load i16, ptr %103, !tbaa !2
  %303 = load i16, ptr %69, !tbaa !2
  %304 = icmp sge i16 %302, %303
  br i1 %304, label %b50, label %b51

b50:
  %305 = addrspacecast ptr %70 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %305, i16 0)
  %306 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %70)
  %307 = trunc i16 %306 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %307)
  %308 = load i16, ptr %103, !tbaa !2
  %309 = load i16, ptr %68, !tbaa !2
  %310 = add nsw i16 %308, %309
  store i16 %310, ptr %103, !tbaa !2
  br label %b47

b51:
  store i16 100, ptr %67, !tbaa !2
  store i16 100, ptr %66, !tbaa !2
  store i16 0, ptr %103, !tbaa !2
  store i16 1, ptr %65, !tbaa !2
  %311 = load i16, ptr %66, !tbaa !2
  store i16 %311, ptr %64, !tbaa !2
  store i16 1, ptr %63, !tbaa !2
  br label %b52

b52:
  %312 = load i16, ptr %63, !tbaa !2
  %313 = icmp sge i16 %312, 0
  br i1 %313, label %b53, label %b54

b53:
  %314 = load i16, ptr %65, !tbaa !2
  %315 = load i16, ptr %64, !tbaa !2
  %316 = icmp sle i16 %314, %315
  br i1 %316, label %b55, label %b56

b54:
  %317 = load i16, ptr %65, !tbaa !2
  %318 = load i16, ptr %64, !tbaa !2
  %319 = icmp sge i16 %317, %318
  br i1 %319, label %b55, label %b56

b55:
  store i16 1, ptr %62, !tbaa !2
  %320 = load i16, ptr %67, !tbaa !2
  store i16 %320, ptr %61, !tbaa !2
  store i16 1, ptr %60, !tbaa !2
  br label %b57

b56:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  store i16 0, ptr %59, !tbaa !2
  store i16 31, ptr %58, !tbaa !2
  store i16 0, ptr %57, !tbaa !2
  store i16 0, ptr %56, !tbaa !2
  store i16 0, ptr %55, !tbaa !2
  store i16 0, ptr %54, !tbaa !2
  store i16 31, ptr %53, !tbaa !2
  store i16 63, ptr %52, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %59, ptr %58, ptr %57, ptr %56, ptr %55, ptr %54, ptr %53, ptr %52)
  store i16 32, ptr %51, !tbaa !2
  store i16 63, ptr %50, !tbaa !2
  store i16 0, ptr %49, !tbaa !2
  store i16 31, ptr %48, !tbaa !2
  store i16 63, ptr %47, !tbaa !2
  store i16 31, ptr %46, !tbaa !2
  store i16 0, ptr %45, !tbaa !2
  store i16 63, ptr %44, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %51, ptr %50, ptr %49, ptr %48, ptr %47, ptr %46, ptr %45, ptr %44)
  store i16 64, ptr %43, !tbaa !2
  store i16 95, ptr %42, !tbaa !2
  store i16 31, ptr %41, !tbaa !2
  store i16 0, ptr %40, !tbaa !2
  store i16 63, ptr %39, !tbaa !2
  store i16 0, ptr %38, !tbaa !2
  store i16 63, ptr %37, !tbaa !2
  store i16 0, ptr %36, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %43, ptr %42, ptr %41, ptr %40, ptr %39, ptr %38, ptr %37, ptr %36)
  store i16 96, ptr %35, !tbaa !2
  store i16 127, ptr %34, !tbaa !2
  store i16 0, ptr %33, !tbaa !2
  store i16 63, ptr %32, !tbaa !2
  store i16 0, ptr %31, !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  store i16 0, ptr %29, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr %35, ptr %34, ptr %33, ptr %32, ptr %31, ptr %30, ptr %29, ptr %28)
  store i16 15, ptr %27, !tbaa !2
  store i16 305, ptr %26, !tbaa !2
  store i16 10, ptr %25, !tbaa !2
  br label %b62

b57:
  %321 = load i16, ptr %60, !tbaa !2
  %322 = icmp sge i16 %321, 0
  br i1 %322, label %b58, label %b59

b58:
  %323 = load i16, ptr %62, !tbaa !2
  %324 = load i16, ptr %61, !tbaa !2
  %325 = icmp sle i16 %323, %324
  br i1 %325, label %b60, label %b61

b59:
  %326 = load i16, ptr %62, !tbaa !2
  %327 = load i16, ptr %61, !tbaa !2
  %328 = icmp sge i16 %326, %327
  br i1 %328, label %b60, label %b61

b60:
  %329 = addrspacecast ptr %70 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %329, i16 0)
  %330 = load i16, ptr %103, !tbaa !2
  %331 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %70)
  %332 = sub i16 %330, 0
  %333 = getelementptr inbounds i16, ptr @"SP%", i16 %332
  store i16 %331, ptr %333, !tbaa !2
  %334 = load i16, ptr %103, !tbaa !2
  %335 = add i16 %334, 1
  store i16 %335, ptr %103, !tbaa !2
  %336 = load i16, ptr %62, !tbaa !2
  %337 = load i16, ptr %60, !tbaa !2
  %338 = add nsw i16 %336, %337
  store i16 %338, ptr %62, !tbaa !2
  br label %b57

b61:
  %339 = load i16, ptr %65, !tbaa !2
  %340 = load i16, ptr %63, !tbaa !2
  %341 = add nsw i16 %339, %340
  store i16 %341, ptr %65, !tbaa !2
  br label %b52

b62:
  %342 = load i16, ptr %25, !tbaa !2
  %343 = icmp sge i16 %342, 0
  br i1 %343, label %b63, label %b64

b63:
  %344 = load i16, ptr %27, !tbaa !2
  %345 = load i16, ptr %26, !tbaa !2
  %346 = icmp sle i16 %344, %345
  br i1 %346, label %b65, label %b66

b64:
  %347 = load i16, ptr %27, !tbaa !2
  %348 = load i16, ptr %26, !tbaa !2
  %349 = icmp sge i16 %347, %348
  br i1 %349, label %b65, label %b66

b65:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 14, i16 6)
  %350 = load i16, ptr %27, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 %350, i16 7)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 13, i16 5)
  %351 = load i16, ptr %27, !tbaa !2
  %352 = add i16 %351, 1
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 %352, i16 8)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 31, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 12, i16 4)
  %353 = load i16, ptr %27, !tbaa !2
  %354 = add i16 %353, 2
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 %354, i16 9)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  %355 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %355, ptr @"BENCHFRAME&", !tbaa !2
  %356 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %356, ptr @"BENCHFRAME&", !tbaa !2
  %357 = load i16, ptr @"XIT%", !tbaa !2
  %358 = icmp eq i16 %357, 0
  br i1 %358, label %b67, label %b68

b66:
  store i16 7, ptr %24, !tbaa !2
  store i16 193, ptr %23, !tbaa !2
  store i16 10, ptr %22, !tbaa !2
  br label %b74

b67:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b69

b68:
  br label %b69

b69:
  %359 = load i16, ptr @"XIT%", !tbaa !2
  %360 = icmp eq i16 %359, 1
  br i1 %360, label %b70, label %b71

b70:
  br label %b2

b71:
  br label %b72

b72:
  %361 = load i16, ptr %27, !tbaa !2
  %362 = load i16, ptr %25, !tbaa !2
  %363 = add nsw i16 %361, %362
  store i16 %363, ptr %27, !tbaa !2
  br label %b62

b74:
  %364 = load i16, ptr %22, !tbaa !2
  %365 = icmp sge i16 %364, 0
  br i1 %365, label %b75, label %b76

b75:
  %366 = load i16, ptr %24, !tbaa !2
  %367 = load i16, ptr %23, !tbaa !2
  %368 = icmp sle i16 %366, %367
  br i1 %368, label %b77, label %b78

b76:
  %369 = load i16, ptr %24, !tbaa !2
  %370 = load i16, ptr %23, !tbaa !2
  %371 = icmp sge i16 %369, %370
  br i1 %371, label %b77, label %b78

b77:
  %372 = load i16, ptr %24, !tbaa !2
  %373 = icmp sgt i16 %372, 7
  %374 = sext i1 %373 to i16
  %375 = load i16, ptr %24, !tbaa !2
  %376 = icmp slt i16 %375, 194
  %377 = sext i1 %376 to i16
  %378 = and i16 %374, %377
  %379 = icmp ne i16 %378, 0
  br i1 %379, label %b79, label %b80

b78:
  %380 = load i16, ptr %20, !tbaa !2
  store i16 %380, ptr %21, !tbaa !2
  %381 = load i16, ptr %27, !tbaa !2
  store i16 %381, ptr %19, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  br label %b90

b79:
  %382 = load i16, ptr %24, !tbaa !2
  %383 = sub i16 %382, 1
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 15, i16 %383)
  %384 = load i16, ptr %24, !tbaa !2
  %385 = sub i16 %384, 1
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 304, i16 %385)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 0, i16 -1, i16 1)
  br label %b81

b80:
  br label %b81

b81:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 14, i16 6)
  %386 = load i16, ptr %24, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 305, i16 %386)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 13, i16 5)
  %387 = load i16, ptr %24, !tbaa !2
  %388 = add i16 %387, 1
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 306, i16 %388)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 31, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 12, i16 4)
  %389 = load i16, ptr %24, !tbaa !2
  %390 = add i16 %389, 2
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 307, i16 %390)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  %391 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %391, ptr @"BENCHFRAME&", !tbaa !2
  %392 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %392, ptr @"BENCHFRAME&", !tbaa !2
  %393 = load i16, ptr @"XIT%", !tbaa !2
  %394 = icmp eq i16 %393, 0
  br i1 %394, label %b82, label %b83

b82:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b84

b83:
  br label %b84

b84:
  %395 = load i16, ptr @"XIT%", !tbaa !2
  %396 = icmp eq i16 %395, 1
  br i1 %396, label %b85, label %b86

b85:
  br label %b2

b86:
  br label %b87

b87:
  %397 = load i16, ptr %24, !tbaa !2
  %398 = load i16, ptr %22, !tbaa !2
  %399 = add nsw i16 %397, %398
  store i16 %399, ptr %24, !tbaa !2
  br label %b74

b90:
  %400 = load i16, ptr %17, !tbaa !2
  %401 = add i16 %400, 1
  store i16 %401, ptr %17, !tbaa !2
  %402 = load i16, ptr %17, !tbaa !2
  %403 = icmp sgt i16 %402, 140
  %404 = sext i1 %403 to i16
  %405 = load i16, ptr %18, !tbaa !2
  %406 = icmp sgt i16 %405, -100
  %407 = sext i1 %406 to i16
  %408 = and i16 %404, %407
  %409 = icmp ne i16 %408, 0
  br i1 %409, label %b92, label %b93

b92:
  %410 = load i16, ptr %18, !tbaa !2
  %411 = sub i16 %410, 1010
  store i16 %411, ptr %18, !tbaa !2
  br label %b94

b93:
  %412 = load i16, ptr %17, !tbaa !2
  %413 = icmp sle i16 %412, 100
  %414 = sext i1 %413 to i16
  %415 = load i16, ptr %18, !tbaa !2
  %416 = icmp slt i16 %415, 10000
  %417 = sext i1 %416 to i16
  %418 = and i16 %414, %417
  %419 = icmp ne i16 %418, 0
  br i1 %419, label %b95, label %b96

b94:
  %420 = load i16, ptr %18, !tbaa !2
  %421 = icmp eq i16 %420, -100
  br i1 %421, label %b98, label %b99

b95:
  %422 = load i16, ptr %18, !tbaa !2
  %423 = add i16 %422, 1000
  store i16 %423, ptr %18, !tbaa !2
  br label %b97

b96:
  br label %b97

b97:
  br label %b94

b98:
  br label %b2

b99:
  br label %b100

b100:
  %424 = load i16, ptr %20, !tbaa !2
  %425 = add i16 %424, 1
  store i16 %425, ptr %20, !tbaa !2
  %426 = load i16, ptr %20, !tbaa !2
  %427 = icmp eq i16 %426, 220
  br i1 %427, label %b102, label %b103

b102:
  store i16 0, ptr %20, !tbaa !2
  br label %b104

b103:
  br label %b104

b104:
  %428 = load i16, ptr %27, !tbaa !2
  %429 = add i16 %428, 1
  store i16 %429, ptr %27, !tbaa !2
  %430 = load i16, ptr %27, !tbaa !2
  %431 = icmp eq i16 %430, 283
  br i1 %431, label %b105, label %b106

b105:
  store i16 0, ptr %27, !tbaa !2
  br label %b107

b106:
  br label %b107

b107:
  %432 = load i16, ptr %21, !tbaa !2
  %433 = add i16 %432, 1
  store i16 %433, ptr %21, !tbaa !2
  %434 = load i16, ptr %21, !tbaa !2
  %435 = icmp eq i16 %434, 101
  br i1 %435, label %b108, label %b109

b108:
  store i16 0, ptr %21, !tbaa !2
  br label %b110

b109:
  br label %b110

b110:
  %436 = load i16, ptr %19, !tbaa !2
  %437 = add i16 %436, 1
  store i16 %437, ptr %19, !tbaa !2
  %438 = load i16, ptr %19, !tbaa !2
  %439 = icmp eq i16 %438, 396
  br i1 %439, label %b111, label %b112

b111:
  store i16 0, ptr %19, !tbaa !2
  br label %b113

b112:
  br label %b113

b113:
  %440 = load i16, ptr %20, !tbaa !2
  store i16 %440, ptr %15, !tbaa !2
  %441 = load i16, ptr %15, !tbaa !2
  %442 = sitofp i16 %441 to float
  store i16 35, ptr %14, !tbaa !2
  %443 = load i16, ptr %14, !tbaa !2
  %444 = sitofp i16 %443 to float
  %445 = fdiv float %442, %444
  %446 = call float @llvm.fabs.f32(float %445)
  %447 = load float, ptr @$float86, !tbaa !2
  %448 = fcmp ogt float %446, %447
  br i1 %448, label %b114, label %b115

b114:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b115:
  %449 = call float @llvm.sin.f32(float %445)
  store i16 48, ptr %13, !tbaa !2
  %450 = load i16, ptr %13, !tbaa !2
  %451 = sitofp i16 %450 to float
  %452 = fmul float %449, %451
  %453 = call i16 @llvm.lrint.i16.f32(float %452)
  store i16 %453, ptr %16, !tbaa !2
  %454 = load i16, ptr %27, !tbaa !2
  store i16 %454, ptr %11, !tbaa !2
  %455 = load i16, ptr %11, !tbaa !2
  %456 = sitofp i16 %455 to float
  store i16 45, ptr %10, !tbaa !2
  %457 = load i16, ptr %10, !tbaa !2
  %458 = sitofp i16 %457 to float
  %459 = fdiv float %456, %458
  %460 = call float @llvm.fabs.f32(float %459)
  %461 = load float, ptr @$float86, !tbaa !2
  %462 = fcmp ogt float %460, %461
  br i1 %462, label %b116, label %b117

b116:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b117:
  %463 = call float @llvm.sin.f32(float %459)
  store i16 64, ptr %9, !tbaa !2
  %464 = load i16, ptr %9, !tbaa !2
  %465 = sitofp i16 %464 to float
  %466 = fmul float %463, %465
  %467 = call i16 @llvm.lrint.i16.f32(float %466)
  store i16 %467, ptr %12, !tbaa !2
  store i16 160, ptr %8, !tbaa !2
  store i16 8, ptr %65, !tbaa !2
  store i16 191, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b118

b118:
  %468 = load i16, ptr %6, !tbaa !2
  %469 = icmp sge i16 %468, 0
  br i1 %469, label %b119, label %b120

b119:
  %470 = load i16, ptr %65, !tbaa !2
  %471 = load i16, ptr %7, !tbaa !2
  %472 = icmp sle i16 %470, %471
  br i1 %472, label %b121, label %b122

b120:
  %473 = load i16, ptr %65, !tbaa !2
  %474 = load i16, ptr %7, !tbaa !2
  %475 = icmp sge i16 %473, %474
  br i1 %475, label %b121, label %b122

b121:
  %476 = load i16, ptr %8, !tbaa !2
  %477 = add i16 -24576, %476
  store i16 %477, ptr @b$seg, !tbaa !2
  %478 = load i16, ptr %8, !tbaa !2
  %479 = add i16 %478, 20
  store i16 %479, ptr %8, !tbaa !2
  %480 = load i16, ptr %65, !tbaa !2
  %481 = load i16, ptr %27, !tbaa !2
  %482 = add i16 %480, %481
  %483 = sub i16 %482, -319
  %484 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %483
  %485 = load i16, ptr %484, !tbaa !2
  %486 = load i16, ptr %65, !tbaa !2
  %487 = sub i16 %486, -48
  %488 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %487
  %489 = load i16, ptr %488, !tbaa !2
  %490 = load i16, ptr %65, !tbaa !2
  %491 = load i16, ptr %27, !tbaa !2
  %492 = add i16 %490, %491
  %493 = sub i16 %492, -319
  %494 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %493
  %495 = load i16, ptr %494, !tbaa !2
  %496 = add i16 %489, %495
  %497 = sub i16 %496, -48
  %498 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %497
  %499 = load i16, ptr %498, !tbaa !2
  %500 = add i16 %485, %499
  store i16 %500, ptr %5, !tbaa !2
  store i16 16, ptr %62, !tbaa !2
  store i16 303, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b123

b122:
  %501 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %501, ptr @"BENCHFRAME&", !tbaa !2
  %502 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %502, ptr @"BENCHFRAME&", !tbaa !2
  %503 = load i16, ptr @"XIT%", !tbaa !2
  %504 = icmp eq i16 %503, 0
  br i1 %504, label %b134, label %b135

b123:
  %505 = load i16, ptr %3, !tbaa !2
  %506 = icmp sge i16 %505, 0
  br i1 %506, label %b124, label %b125

b124:
  %507 = load i16, ptr %62, !tbaa !2
  %508 = load i16, ptr %4, !tbaa !2
  %509 = icmp sle i16 %507, %508
  br i1 %509, label %b126, label %b127

b125:
  %510 = load i16, ptr %62, !tbaa !2
  %511 = load i16, ptr %4, !tbaa !2
  %512 = icmp sge i16 %510, %511
  br i1 %512, label %b126, label %b127

b126:
  %513 = load i16, ptr %62, !tbaa !2
  %514 = load i16, ptr %20, !tbaa !2
  %515 = add i16 %513, %514
  %516 = sub i16 %515, -48
  %517 = getelementptr inbounds i16, ptr @"FSIN1%", i16 %516
  %518 = load i16, ptr %517, !tbaa !2
  %519 = load i16, ptr %65, !tbaa !2
  %520 = load i16, ptr %62, !tbaa !2
  %521 = sub i16 %519, %520
  %522 = sub i16 %521, -319
  %523 = getelementptr inbounds i16, ptr @"FSIN4%", i16 %522
  %524 = load i16, ptr %523, !tbaa !2
  %525 = add i16 %518, %524
  store i16 %525, ptr %2, !tbaa !2
  %526 = load i16, ptr %65, !tbaa !2
  %527 = sub i16 %526, 100
  %528 = load i16, ptr %2, !tbaa !2
  %529 = mul i16 %528, 321
  %530 = add i16 %529, %527
  %531 = mul i16 %530, 2
  %532 = getelementptr i8, ptr %104, i16 2
  %533 = load i16, ptr %532, !tbaa !2
  %534 = add i16 31136, %531
  %535 = inttoptr i16 %533 to ptr addrspace(2)
  %536 = addrspacecast ptr addrspace(2) %535 to ptr addrspace(1)
  %537 = getelementptr i8, ptr addrspace(1) %536, i16 %534
  %538 = load i16, ptr addrspace(1) %537, !tbaa !74
  %539 = load i16, ptr %12, !tbaa !2
  %540 = add i16 %538, %539
  %541 = sub i16 %540, -180
  %542 = getelementptr inbounds i16, ptr @"DT100%", i16 %541
  %543 = load i16, ptr %542, !tbaa !2
  %544 = load i16, ptr %62, !tbaa !2
  %545 = sub i16 %544, 160
  %546 = load i16, ptr %5, !tbaa !2
  %547 = mul i16 %546, 321
  %548 = add i16 %547, %545
  %549 = mul i16 %548, 2
  %550 = getelementptr i8, ptr %104, i16 2
  %551 = load i16, ptr %550, !tbaa !2
  %552 = add i16 31136, %549
  %553 = inttoptr i16 %551 to ptr addrspace(2)
  %554 = addrspacecast ptr addrspace(2) %553 to ptr addrspace(1)
  %555 = getelementptr i8, ptr addrspace(1) %554, i16 %552
  %556 = load i16, ptr addrspace(1) %555, !tbaa !74
  %557 = load i16, ptr %16, !tbaa !2
  %558 = add i16 %556, %557
  %559 = sub i16 %558, -192
  %560 = getelementptr inbounds i16, ptr @"DT%", i16 %559
  %561 = load i16, ptr %560, !tbaa !2
  %562 = add i16 %543, %561
  store i16 %562, ptr %1, !tbaa !2
  %563 = load i16, ptr %1, !tbaa !2
  %564 = load i16, ptr %18, !tbaa !2
  %565 = icmp sgt i16 %563, %564
  br i1 %565, label %b128, label %b129

b127:
  %566 = load i16, ptr %65, !tbaa !2
  %567 = load i16, ptr %6, !tbaa !2
  %568 = add nsw i16 %566, %567
  store i16 %568, ptr %65, !tbaa !2
  br label %b118

b128:
  store i16 0, ptr %0, !tbaa !2
  br label %b130

b129:
  %569 = load i16, ptr %1, !tbaa !2
  %570 = sub i16 %569, 0
  %571 = getelementptr inbounds i16, ptr @"SP%", i16 %570
  %572 = load i16, ptr %571, !tbaa !2
  %573 = icmp eq i16 %572, 129
  br i1 %573, label %b131, label %b132

b130:
  %574 = load i16, ptr %62, !tbaa !2
  %575 = load i16, ptr %0, !tbaa !2
  %576 = trunc i16 %575 to i8
  %577 = load i16, ptr @b$seg, !tbaa !2
  %578 = inttoptr i16 %577 to ptr addrspace(2)
  %579 = addrspacecast ptr addrspace(2) %578 to ptr addrspace(1)
  %580 = getelementptr i8, ptr addrspace(1) %579, i16 %574
  store volatile i8 %576, ptr addrspace(1) %580
  %581 = load i16, ptr %62, !tbaa !2
  %582 = load i16, ptr %3, !tbaa !2
  %583 = add nsw i16 %581, %582
  store i16 %583, ptr %62, !tbaa !2
  br label %b123

b131:
  %584 = load i16, ptr %62, !tbaa !2
  %585 = sub i16 %584, -640
  %586 = getelementptr inbounds i16, ptr @"FSIN3%", i16 %585
  %587 = load i16, ptr %586, !tbaa !2
  %588 = load i16, ptr %62, !tbaa !2
  %589 = load i16, ptr %21, !tbaa !2
  %590 = add i16 %588, %589
  %591 = sub i16 %590, -640
  %592 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %591
  %593 = load i16, ptr %592, !tbaa !2
  %594 = add i16 %587, %593
  %595 = load i16, ptr %65, !tbaa !2
  %596 = sub i16 %595, -640
  %597 = getelementptr inbounds i16, ptr @"FSIN2%", i16 %596
  %598 = load i16, ptr %597, !tbaa !2
  %599 = add i16 %594, %598
  %600 = sub i16 %599, -168
  %601 = getelementptr inbounds i16, ptr @"MOD256128%", i16 %600
  %602 = load i16, ptr %601, !tbaa !2
  store i16 %602, ptr %0, !tbaa !2
  br label %b133

b132:
  %603 = load i16, ptr %1, !tbaa !2
  %604 = sub i16 %603, 0
  %605 = getelementptr inbounds i16, ptr @"SP%", i16 %604
  %606 = load i16, ptr %605, !tbaa !2
  store i16 %606, ptr %0, !tbaa !2
  br label %b133

b133:
  br label %b130

b134:
  call cc 1000 addrspace(1) void @KEYACTION()
  br label %b136

b135:
  br label %b136

b136:
  %607 = load i16, ptr @"XIT%", !tbaa !2
  %608 = icmp eq i16 %607, 1
  br i1 %608, label %b137, label %b138

b137:
  br label %b2

b138:
  br label %b139

b139:
  br label %b90
}

define cc 1000 void @BENCHMARK(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  %5 = alloca i16
  %6 = alloca i32
  %7 = alloca i32
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca [18 x i8]
  %12 = alloca [18 x i8]
  store i16 0, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 0, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %12, i16 4
  store i16 0, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %12, i16 6
  store i16 0, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %12, i16 8
  store i16 0, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %12, i16 10
  store i16 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %12, i16 12
  store i16 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %12, i16 14
  store i16 0, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %12, i16 16
  store i16 0, ptr %20, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %11, i16 4
  store i16 0, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %11, i16 6
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %11, i16 8
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %11, i16 10
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %11, i16 12
  store i16 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %11, i16 14
  store i16 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %11, i16 16
  store i16 0, ptr %28, !tbaa !2
  store i32 0, ptr %10, !tbaa !2
  store i32 0, ptr %9, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 767, i16 2, i16 257, ptr %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 5, i16 4, i16 257, ptr %11)
  call cc 1000 addrspace(1) void @TSCSNAP(ptr %10, ptr %9)
  store i32 1, ptr %7, !tbaa !2
  store i32 0, ptr %6, !tbaa !2
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i32 0, ptr %8, !tbaa !2
  store i32 63999, ptr %4, !tbaa !2
  store i32 1, ptr %3, !tbaa !2
  br label %b2

b2:
  %29 = load i32, ptr %3, !tbaa !2
  %30 = icmp sge i32 %29, 0
  br i1 %30, label %b3, label %b4

b3:
  %31 = load i32, ptr %8, !tbaa !2
  %32 = load i32, ptr %4, !tbaa !2
  %33 = icmp sle i32 %31, %32
  br i1 %33, label %b5, label %b6

b4:
  %34 = load i32, ptr %8, !tbaa !2
  %35 = load i32, ptr %4, !tbaa !2
  %36 = icmp sge i32 %34, %35
  br i1 %36, label %b5, label %b6

b5:
  %37 = load i32, ptr %7, !tbaa !2
  %38 = load i32, ptr %8, !tbaa !2
  %39 = trunc i32 %38 to i16
  %40 = load i16, ptr @b$seg, !tbaa !2
  %41 = inttoptr i16 %40 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 %39
  %44 = load volatile i8, ptr addrspace(1) %43
  %45 = zext i8 %44 to i16
  %46 = sext i16 %45 to i32
  %47 = add i32 %37, %46
  %48 = srem i32 %47, 65521
  store i32 %48, ptr %7, !tbaa !2
  %49 = load i32, ptr %6, !tbaa !2
  %50 = load i32, ptr %7, !tbaa !2
  %51 = add i32 %49, %50
  %52 = srem i32 %51, 65521
  store i32 %52, ptr %6, !tbaa !2
  %53 = load i32, ptr %8, !tbaa !2
  %54 = load i32, ptr %3, !tbaa !2
  %55 = add nsw i32 %53, %54
  store i32 %55, ptr %8, !tbaa !2
  br label %b2

b6:
  %56 = load i16, ptr %0
  %57 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %56)
  %58 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %57)
  %59 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string248, ptr %58)
  %60 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %59, ptr @$string249)
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %60, i16 0, i16 -1536)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call void @llrm.ia16.out.i8(i16 967, i8 0)
  store i16 0, ptr %5, !tbaa !2
  store i16 767, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b7:
  %61 = load i16, ptr %1, !tbaa !2
  %62 = icmp sge i16 %61, 0
  br i1 %62, label %b8, label %b9

b8:
  %63 = load i16, ptr %5, !tbaa !2
  %64 = load i16, ptr %2, !tbaa !2
  %65 = icmp sle i16 %63, %64
  br i1 %65, label %b10, label %b11

b9:
  %66 = load i16, ptr %5, !tbaa !2
  %67 = load i16, ptr %2, !tbaa !2
  %68 = icmp sge i16 %66, %67
  br i1 %68, label %b10, label %b11

b10:
  %69 = load i16, ptr %5, !tbaa !2
  %70 = mul i16 %69, 2
  %71 = getelementptr i8, ptr %12, i16 2
  %72 = load i16, ptr %71, !tbaa !2
  %73 = add i16 0, %70
  %74 = inttoptr i16 %72 to ptr addrspace(2)
  %75 = addrspacecast ptr addrspace(2) %74 to ptr addrspace(1)
  %76 = getelementptr i8, ptr addrspace(1) %75, i16 %73
  %77 = call i8 @llrm.ia16.in.i8(i16 969)
  %78 = zext i8 %77 to i16
  store i16 %78, ptr addrspace(1) %76, !tbaa !76
  %79 = load i16, ptr %5, !tbaa !2
  %80 = load i16, ptr %1, !tbaa !2
  %81 = add nsw i16 %79, %80
  store i16 %81, ptr %5, !tbaa !2
  br label %b7

b11:
  %82 = getelementptr i8, ptr %12, i16 2
  %83 = load i16, ptr %82, !tbaa !2
  %84 = inttoptr i16 %83 to ptr addrspace(2)
  %85 = addrspacecast ptr addrspace(2) %84 to ptr addrspace(1)
  %86 = getelementptr i8, ptr addrspace(1) %85, i16 0
  %87 = addrspacecast ptr addrspace(1) %86 to ptr addrspace(2)
  %88 = ptrtoint ptr addrspace(2) %87 to i16
  store i16 %88, ptr @b$seg, !tbaa !2
  %89 = load i16, ptr %0
  %90 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %89)
  %91 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %90)
  %92 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string250, ptr %91)
  %93 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %92, ptr @$string251)
  %94 = getelementptr i8, ptr %12, i16 2
  %95 = load i16, ptr %94, !tbaa !2
  %96 = inttoptr i16 %95 to ptr addrspace(2)
  %97 = addrspacecast ptr addrspace(2) %96 to ptr addrspace(1)
  %98 = getelementptr i8, ptr addrspace(1) %97, i16 0
  %99 = ptrtoint ptr addrspace(1) %98 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %93, i16 %99, i16 1536)
  %100 = getelementptr i8, ptr %11, i16 2
  %101 = load i16, ptr %100, !tbaa !2
  %102 = inttoptr i16 %101 to ptr addrspace(2)
  %103 = addrspacecast ptr addrspace(2) %102 to ptr addrspace(1)
  %104 = getelementptr i8, ptr addrspace(1) %103, i16 0
  %105 = load i32, ptr @"BENCHHI&", !tbaa !2
  store i32 %105, ptr addrspace(1) %104, !tbaa !78
  %106 = getelementptr i8, ptr %11, i16 2
  %107 = load i16, ptr %106, !tbaa !2
  %108 = inttoptr i16 %107 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 4
  %111 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %111, ptr addrspace(1) %110, !tbaa !78
  %112 = getelementptr i8, ptr %11, i16 2
  %113 = load i16, ptr %112, !tbaa !2
  %114 = inttoptr i16 %113 to ptr addrspace(2)
  %115 = addrspacecast ptr addrspace(2) %114 to ptr addrspace(1)
  %116 = getelementptr i8, ptr addrspace(1) %115, i16 8
  %117 = load i32, ptr %10, !tbaa !2
  store i32 %117, ptr addrspace(1) %116, !tbaa !78
  %118 = getelementptr i8, ptr %11, i16 2
  %119 = load i16, ptr %118, !tbaa !2
  %120 = inttoptr i16 %119 to ptr addrspace(2)
  %121 = addrspacecast ptr addrspace(2) %120 to ptr addrspace(1)
  %122 = getelementptr i8, ptr addrspace(1) %121, i16 12
  %123 = load i32, ptr %9, !tbaa !2
  store i32 %123, ptr addrspace(1) %122, !tbaa !78
  %124 = getelementptr i8, ptr %11, i16 2
  %125 = load i16, ptr %124, !tbaa !2
  %126 = inttoptr i16 %125 to ptr addrspace(2)
  %127 = addrspacecast ptr addrspace(2) %126 to ptr addrspace(1)
  %128 = getelementptr i8, ptr addrspace(1) %127, i16 16
  %129 = load i32, ptr %6, !tbaa !2
  store i32 %129, ptr addrspace(1) %128, !tbaa !78
  %130 = getelementptr i8, ptr %11, i16 2
  %131 = load i16, ptr %130, !tbaa !2
  %132 = inttoptr i16 %131 to ptr addrspace(2)
  %133 = addrspacecast ptr addrspace(2) %132 to ptr addrspace(1)
  %134 = getelementptr i8, ptr addrspace(1) %133, i16 20
  %135 = load i32, ptr %7, !tbaa !2
  store i32 %135, ptr addrspace(1) %134, !tbaa !78
  %136 = getelementptr i8, ptr %11, i16 2
  %137 = load i16, ptr %136, !tbaa !2
  %138 = inttoptr i16 %137 to ptr addrspace(2)
  %139 = addrspacecast ptr addrspace(2) %138 to ptr addrspace(1)
  %140 = getelementptr i8, ptr addrspace(1) %139, i16 0
  %141 = addrspacecast ptr addrspace(1) %140 to ptr addrspace(2)
  %142 = ptrtoint ptr addrspace(2) %141 to i16
  store i16 %142, ptr @b$seg, !tbaa !2
  %143 = load i16, ptr %0
  %144 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %143)
  %145 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %144)
  %146 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string252, ptr %145)
  %147 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %146, ptr @$string253)
  %148 = getelementptr i8, ptr %11, i16 2
  %149 = load i16, ptr %148, !tbaa !2
  %150 = inttoptr i16 %149 to ptr addrspace(2)
  %151 = addrspacecast ptr addrspace(2) %150 to ptr addrspace(1)
  %152 = getelementptr i8, ptr addrspace(1) %151, i16 0
  %153 = ptrtoint ptr addrspace(1) %152 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %147, i16 %153, i16 24)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc 1000 addrspace(1) void @TSCSNAP(ptr @"BENCHHI&", ptr @"BENCHLO&")
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %12)
  ret void
}

declare cc 1000 void @llrm.qb.B$RNZP(double) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FCMD() addrspace(1)

declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1) threeway

declare cc 1000 void @llrm.qb.B$N1I2(i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$N2I2(i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$LINE(i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$SCLS(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$GET3(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FASC(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CLOS(i16, i16) addrspace(1)

declare float @llvm.fabs.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$STDL(ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$INKY() addrspace(1)

declare float @llvm.cos.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1000 void @llrm.qb.B$COLR(i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$LOCT(i16, i16, i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$RND0() addrspace(1)

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.sqrt.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.log2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.exp2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PAIN(i16, i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$PNI2(i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSTC(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$GET4(i16, i32, ptr addrspace(1), i16) addrspace(1)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$FMID(ptr, i16, i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FCHR(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CIRC(float, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PUT3(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$N1R4(float, float) addrspace(1)

declare cc 1000 void @llrm.qb.B$N2R4(float, float) addrspace(1)

declare cc 1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
!9 = !{!"allocation.2", !3, i64 0}
!10 = !{!9, !9, i64 0}
!11 = !{!"allocation.3", !3, i64 0}
!12 = !{!11, !11, i64 0}
!13 = !{!"allocation.4", !3, i64 0}
!14 = !{!13, !13, i64 0}
!15 = !{!"allocation.5", !3, i64 0}
!16 = !{!15, !15, i64 0}
!17 = !{!"allocation.6", !3, i64 0}
!18 = !{!17, !17, i64 0}
!19 = !{!"allocation.7", !3, i64 0}
!20 = !{!19, !19, i64 0}
!21 = !{!"allocation.8", !3, i64 0}
!22 = !{!21, !21, i64 0}
!23 = !{!"allocation.9", !3, i64 0}
!24 = !{!23, !23, i64 0}
!25 = !{!"allocation.10", !3, i64 0}
!26 = !{!25, !25, i64 0}
!27 = !{!"allocation.11", !3, i64 0}
!28 = !{!27, !27, i64 0}
!29 = !{!"allocation.12", !3, i64 0}
!30 = !{!29, !29, i64 0}
!31 = !{!"allocation.13", !3, i64 0}
!32 = !{!31, !31, i64 0}
!33 = !{!"allocation.14", !3, i64 0}
!34 = !{!33, !33, i64 0}
!35 = !{!"allocation.15", !3, i64 0}
!36 = !{!35, !35, i64 0}
!37 = !{!"allocation.16", !3, i64 0}
!38 = !{!37, !37, i64 0}
!39 = !{!"allocation.17", !3, i64 0}
!40 = !{!39, !39, i64 0}
!41 = !{!"allocation.18", !3, i64 0}
!42 = !{!41, !41, i64 0}
!43 = !{!"allocation.19", !3, i64 0}
!44 = !{!43, !43, i64 0}
!45 = !{!"allocation.20", !3, i64 0}
!46 = !{!45, !45, i64 0}
!47 = !{!"allocation.21", !3, i64 0}
!48 = !{!47, !47, i64 0}
!49 = !{!"allocation.22", !3, i64 0}
!50 = !{!49, !49, i64 0}
!51 = !{!"allocation.23", !3, i64 0}
!52 = !{!51, !51, i64 0}
!53 = !{!"allocation.24", !3, i64 0}
!54 = !{!53, !53, i64 0}
!55 = !{!"allocation.25", !3, i64 0}
!56 = !{!55, !55, i64 0}
!57 = !{!"allocation.26", !3, i64 0}
!58 = !{!57, !57, i64 0}
!59 = !{!"allocation.27", !3, i64 0}
!60 = !{!59, !59, i64 0}
!61 = !{!"allocation.28", !3, i64 0}
!62 = !{!61, !61, i64 0}
!63 = !{!"allocation.29", !3, i64 0}
!64 = !{!63, !63, i64 0}
!65 = !{!"allocation.30", !3, i64 0}
!66 = !{!65, !65, i64 0}
!67 = !{!"allocation.31", !3, i64 0}
!68 = !{!67, !67, i64 0}
!69 = !{!"allocation.32", !3, i64 0}
!70 = !{!69, !69, i64 0}
!71 = !{!"allocation.33", !3, i64 0}
!72 = !{!71, !71, i64 0}
!73 = !{!"allocation.34", !3, i64 0}
!74 = !{!73, !73, i64 0}
!75 = !{!"allocation.35", !3, i64 0}
!76 = !{!75, !75, i64 0}
!77 = !{!"allocation.36", !3, i64 0}
!78 = !{!77, !77, i64 0}
