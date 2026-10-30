target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [32 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"R%" = internal global [18 x i8] zeroinitializer
@"G%" = internal global [18 x i8] zeroinitializer
@"B%" = internal global [18 x i8] zeroinitializer
@"R1%" = internal global [18 x i8] zeroinitializer
@"G1%" = internal global [18 x i8] zeroinitializer
@"B1%" = internal global [18 x i8] zeroinitializer
@"FPOS%" = internal global [18 x i8] zeroinitializer
@"FONTS%" = internal global [18 x i8] zeroinitializer
@"FSIN4%" = internal global [1844 x i8] zeroinitializer, align 2
@"FSIN1%" = internal global [2264 x i8] zeroinitializer, align 2
@"FSIN2%" = internal global [3196 x i8] zeroinitializer, align 2
@"FSIN2%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FSIN2%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"FSIN2%", i16 1280), [6 x i8] c"\02\00>\06\80\FD" }>
@"FSIN3%" = internal global [3024 x i8] zeroinitializer, align 2
@"FSIN3%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FSIN3%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"FSIN3%", i16 1280), [6 x i8] c"\02\00\E8\05\80\FD" }>
@"MOD256128%" = internal global [674 x i8] zeroinitializer, align 2
@"SP%" = internal global [32770 x i8] zeroinitializer, align 2
@"DT%" = internal global [746 x i8] zeroinitializer, align 2
@"DT100%" = internal global [722 x i8] zeroinitializer, align 2
@"CD%" = internal global [3586 x i8] zeroinitializer, align 2
@"DSX%" = internal global [22 x i8] zeroinitializer, align 2
@"DSY%" = internal global [22 x i8] zeroinitializer, align 2
@"PSX%" = internal global [22 x i8] zeroinitializer, align 2
@"PSY%" = internal global [22 x i8] zeroinitializer, align 2
@"XP1%" = internal global [22 x i8] zeroinitializer, align 2
@"YP1%" = internal global [22 x i8] zeroinitializer, align 2
@"EPI6%" = internal global [12 x i8] zeroinitializer, align 2
@"EPI36%" = internal global [12 x i8] zeroinitializer, align 2
@"CY%" = internal global [802 x i8] zeroinitializer, align 2
@"SLA%" = internal global [642 x i8] zeroinitializer, align 2
@"SLB%" = internal global [642 x i8] zeroinitializer, align 2
@"SC%" = internal global [642 x i8] zeroinitializer, align 2
@"DIV2%" = internal global [512 x i8] zeroinitializer, align 2
@"MUL32%" = internal global [66 x i8] zeroinitializer, align 2
@"MUL128%" = internal global [258 x i8] zeroinitializer, align 2
@"FS%" = internal global [400 x i8] zeroinitializer, align 2
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
@$string87 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string87, i16 4), [12 x i8] c"wolfmap0.wad" }>
@K$ = internal global [4 x i8] zeroinitializer
@$string88 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string88, i16 4), [2 x i8] c" \00" }>
@$string89 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string89, i16 4), [10 x i8] c"3rdrec.rec" }>
@GI$ = internal global [4 x i8] zeroinitializer
@$string90 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string90, i16 4), [2 x i8] c" \00" }>
@GG$ = internal global [4 x i8] zeroinitializer
@$string91 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string91, i16 4), [2 x i8] c" \00" }>
@"GI%" = internal global [2 x i8] zeroinitializer
@"GG%" = internal global [2 x i8] zeroinitializer
@$string92 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string92, i16 4) }>
@"PX%" = internal global [2 x i8] zeroinitializer
@"PY%" = internal global [2 x i8] zeroinitializer
@"PA%" = internal global [2 x i8] zeroinitializer
@"PS%" = internal global [2 x i8] zeroinitializer
@"RA!" = internal global [4 x i8] zeroinitializer
@"DPP%" = internal global [2 x i8] zeroinitializer
@"MAP%" = internal global [2 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@"FPS%" = internal global [2 x i8] zeroinitializer
@"FILEI%" = internal global [2 x i8] zeroinitializer
@$string94 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string94, i16 4) }>
@"NDTS%" = internal global [2 x i8] zeroinitializer
@"NDTSO%" = internal global [2 x i8] zeroinitializer
@"XP!" = internal global [4 x i8] zeroinitializer
@"OBJ%" = internal global [2 x i8] zeroinitializer
@"FTYPE%" = internal global [2 x i8] zeroinitializer
@"GGI%" = internal global [2 x i8] zeroinitializer
@"NLNS%" = internal global [2 x i8] zeroinitializer
@"NPLS%" = internal global [2 x i8] zeroinitializer
@$string95 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string95, i16 4) }>
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
@$string213 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string213, i16 4) }>
@$string214 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string214, i16 4) }>
@$string221 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string221, i16 4), [4 x i8] c"SAX!" }>
@$string222 = internal constant <{ [2 x i8], ptr, [22 x i8] }> <{ [2 x i8] c"\16\00", ptr getelementptr (i8, ptr @$string222, i16 4), [22 x i8] c"Not so serious side :)" }>
@$string223 = internal constant <{ [2 x i8], ptr, [40 x i8] }> <{ [2 x i8] c"(\00", ptr getelementptr (i8, ptr @$string223, i16 4), [40 x i8] c"Just for fun or boredom to draw anything" }>
@$string226 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string226, i16 4), [2 x i8] c" \00" }>
@$string227 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string227, i16 4), [12 x i8] c"qbrules.spr\00" }>
@$string228 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string228, i16 4), [2 x i8] c" \00" }>
@$string232 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string232, i16 4), [10 x i8] c"Press the\00" }>
@$string233 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string233, i16 4), [8 x i8] c"any key\00" }>
@$string234 = internal constant <{ [2 x i8], ptr, [26 x i8] }> <{ [2 x i8] c"\19\00", ptr getelementptr (i8, ptr @$string234, i16 4), [26 x i8] c" button to continue.. ;-)\00" }>
@$string235 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string235, i16 4) }>
@$string239 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string239, i16 4), [12 x i8] c"sphrprec.dat" }>
@$string240 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string240, i16 4) }>
@$string243 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0E\00", ptr getelementptr (i8, ptr @$string243, i16 4), [14 x i8] c"Still Sucking?" }>
@$string245 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string245, i16 4) }>
@$string246 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string246, i16 4), [12 x i8] c"qbinside.spr" }>
@$string247 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string247, i16 4), [2 x i8] c" \00" }>
@$string248 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string248, i16 4), [2 x i8] c"V\00" }>
@$string249 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string249, i16 4), [4 x i8] c".BIN" }>
@$string250 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string250, i16 4), [2 x i8] c"P\00" }>
@$string251 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string251, i16 4), [4 x i8] c".BIN" }>
@$string252 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string252, i16 4), [2 x i8] c"T\00" }>
@$string253 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string253, i16 4), [4 x i8] c".BIN" }>
@"YY%.0" = internal global i16 0, align 2
@"YY%.1" = internal global i16 0, align 2
@"YY%.2" = internal global i16 0, align 2
@"KON%.0" = internal global i16 0, align 2
@"KON%.1" = internal global i16 0, align 2
@"KON%.2" = internal global i16 0, align 2
@"KON%.3" = internal global i16 0, align 2
@"KON%.4" = internal global i16 0, align 2
@"KON%.5" = internal global i16 0, align 2

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn {
b1:
  store i16 1, ptr @$arguments$85, align 16, !tbaa !2
  store double 1.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 2), !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RNZP(double 1.000000e+00)
  %0 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %1 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %0, ptr @$string35)
  %2 = icmp eq i16 %1, 0
  br i1 %2, label %b4, label %35

3:
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 0, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 320, i16 200)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 255, i16 -1, i16 2)
  store i16 1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 172), align 4, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 172))
  tail call cc 1000 addrspace(1) void @SPHEREMAPLASMA()
  store i16 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 174), !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 174))
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @ZOOMDISTORT()
  store i16 3, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 176), align 16, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 176))
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %6 = inttoptr i16 %4 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %10 = inttoptr i16 %8 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 %9
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %14 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %15 = inttoptr i16 %13 to ptr addrspace(2)
  %16 = addrspacecast ptr addrspace(2) %15 to ptr addrspace(1)
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 %14
  %18 = getelementptr i8, ptr addrspace(1) %7, i16 %5
  br label %19

19:
  %20 = phi i16 [ %33, %19 ], [ -512, %3 ]
  %21 = getelementptr i8, ptr addrspace(1) %18, i16 %20
  %22 = getelementptr i8, ptr addrspace(1) %21, i16 512
  %23 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %24 = zext i8 %23 to i16
  store i16 %24, ptr addrspace(1) %22, !tbaa !60
  %25 = getelementptr i8, ptr addrspace(1) %12, i16 %20
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 512
  %27 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %28 = zext i8 %27 to i16
  store i16 %28, ptr addrspace(1) %26, !tbaa !62
  %29 = getelementptr i8, ptr addrspace(1) %17, i16 %20
  %30 = getelementptr i8, ptr addrspace(1) %29, i16 512
  %31 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %32 = zext i8 %31 to i16
  store i16 %32, ptr addrspace(1) %30, !tbaa !64
  %33 = add nsw i16 %20, 2
  %exitcond5 = icmp eq i16 %33, 0
  br i1 %exitcond5, label %34, label %19

34:
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 178), !tbaa !2
  store i32 2031871, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 180), align 4
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 184), align 8
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 188), align 4, !tbaa !2
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 178), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 180), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 182), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 184), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 186), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 188))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  tail call cc 1000 addrspace(1) void @RGBLIGHTS()
  store i16 4, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 190), !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 190))
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"X!")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"Y!")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"Z!")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr @"XO%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr @"YO%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr @"ZO%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 0, i16 1, i16 2, i16 258, ptr @"XS%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 0, i16 1, i16 2, i16 258, ptr @"YS%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -100, i16 99, i16 4, i16 257, ptr @"YPK&")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 8, i16 2, i16 257, ptr @"ZPO%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i32 66362, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 16), align 16
  store i16 50, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 194), !tbaa !2
  br label %b63

b3:
  store i16 8, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 228), align 4, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 228))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b4:
  tail call cc 1000 addrspace(1) void @COPPER()
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

35:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  store i16 15, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 10), !tbaa !2
  tail call cc 1000 addrspace(1) void @DEEDLINESAX(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 10))
  store i32 140, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 12), align 4
  tail call cc 1000 addrspace(1) void @DEEDLINESAX(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 14))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i16 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 16), align 16, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 16))
  tail call cc 1000 addrspace(1) void @SUCKING()
  store i16 140, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 18), !tbaa !2
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"R%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"G%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"B%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"R1%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"G1%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"B1%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 255, i16 2, i16 257, ptr @"FPOS%")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 22016, i16 2, i16 257, ptr @"FONTS%")
  tail call cc 1000 addrspace(1) void @SETCROSFADEPAL(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %36 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %37 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %38 = inttoptr i16 %36 to ptr addrspace(2)
  %39 = addrspacecast ptr addrspace(2) %38 to ptr addrspace(1)
  %40 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %41 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %42 = inttoptr i16 %40 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 %41
  %45 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %46 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %47 = inttoptr i16 %45 to ptr addrspace(2)
  %48 = addrspacecast ptr addrspace(2) %47 to ptr addrspace(1)
  %49 = getelementptr i8, ptr addrspace(1) %48, i16 %46
  %50 = getelementptr i8, ptr addrspace(1) %39, i16 %37
  br label %51

51:
  %52 = phi i16 [ %65, %51 ], [ -512, %35 ]
  %53 = getelementptr i8, ptr addrspace(1) %50, i16 %52
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 512
  %55 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %56 = zext i8 %55 to i16
  store i16 %56, ptr addrspace(1) %54, !tbaa !60
  %57 = getelementptr i8, ptr addrspace(1) %44, i16 %52
  %58 = getelementptr i8, ptr addrspace(1) %57, i16 512
  %59 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %60 = zext i8 %59 to i16
  store i16 %60, ptr addrspace(1) %58, !tbaa !62
  %61 = getelementptr i8, ptr addrspace(1) %49, i16 %52
  %62 = getelementptr i8, ptr addrspace(1) %61, i16 512
  %63 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %64 = zext i8 %63 to i16
  store i16 %64, ptr addrspace(1) %62, !tbaa !64
  %65 = add nsw i16 %52, 2
  %exitcond4 = icmp eq i16 %65, 0
  br i1 %exitcond4, label %66, label %51

66:
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 20), align 4
  store i32 1, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 24), align 8
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 28), align 4
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 20), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 22), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 24), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 26), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 28), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 30))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string36, ptr @A$)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string37, ptr @C$)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @A$, i16 1, i16 -1, i16 32)
  store i16 0, ptr @"I%", !tbaa !2
  store i32 65791, ptr @$data, align 16
  br label %b8

b8:
  %67 = phi i16 [ %82, %73 ], [ 0, %66 ]
  %68 = phi i16 [ %81, %73 ], [ 1, %66 ]
  %69 = icmp sgt i16 %68, -1
  %70 = load i16, ptr @$data, align 16, !tbaa !2
  br i1 %69, label %b9, label %b10

b9:
  %71 = icmp sgt i16 %67, %70
  br i1 %71, label %b12, label %73

b10:
  %72 = icmp slt i16 %67, %70
  br i1 %72, label %b12, label %73

73:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) addrspacecast (ptr @C$ to ptr addrspace(1)), i16 0)
  %74 = load i16, ptr @"I%", !tbaa !2
  %75 = shl i16 %74, 1
  %76 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FPOS%", i16 2), !tbaa !2
  %77 = inttoptr i16 %76 to ptr addrspace(2)
  %78 = addrspacecast ptr addrspace(2) %77 to ptr addrspace(1)
  %79 = getelementptr i8, ptr addrspace(1) %78, i16 %75
  %data3 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @C$, i16 2)
  %byte3 = load i8, ptr %data3
  %80 = zext i8 %byte3 to i16
  store i16 %80, ptr addrspace(1) %79, !tbaa !6
  %81 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 2), !tbaa !2
  %82 = add nsw i16 %81, %74
  store i16 %82, ptr @"I%", !tbaa !2
  br label %b8

b12:
  store i16 0, ptr @"I%", !tbaa !2
  store i32 65621, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 4), align 4
  br label %b13

b13:
  %83 = phi i16 [ %197, %b22 ], [ 0, %b12 ]
  %84 = phi i16 [ %196, %b22 ], [ 1, %b12 ]
  %85 = icmp sgt i16 %84, -1
  %86 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 4), align 4, !tbaa !2
  br i1 %85, label %b14, label %b15

b14:
  %87 = icmp sgt i16 %83, %86
  br i1 %87, label %b17, label %b16

b15:
  %88 = icmp slt i16 %83, %86
  br i1 %88, label %b17, label %b16

b16:
  store i16 0, ptr @"Y%", !tbaa !2
  store i32 65551, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 8), align 8
  %89 = shl i16 %83, 8
  br label %b18

b17:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 3, i16 0, i16 11, i16 4, i16 -32766, ptr @TEXT$)
  %90 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string38, ptr %90)
  %91 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %92 = getelementptr i8, ptr %91, i16 16
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string39, ptr %92)
  %93 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %94 = getelementptr i8, ptr %93, i16 32
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string40, ptr %94)
  %95 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %96 = getelementptr i8, ptr %95, i16 48
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string41, ptr %96)
  %97 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %98 = getelementptr i8, ptr %97, i16 64
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string42, ptr %98)
  %99 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %100 = getelementptr i8, ptr %99, i16 80
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string43, ptr %100)
  %101 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %102 = getelementptr i8, ptr %101, i16 96
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string44, ptr %102)
  %103 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %104 = getelementptr i8, ptr %103, i16 112
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string45, ptr %104)
  %105 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %106 = getelementptr i8, ptr %105, i16 128
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string46, ptr %106)
  %107 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %108 = getelementptr i8, ptr %107, i16 144
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string47, ptr %108)
  %109 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %110 = getelementptr i8, ptr %109, i16 160
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string48, ptr %110)
  %111 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %112 = getelementptr i8, ptr %111, i16 176
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string49, ptr %112)
  %113 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %114 = getelementptr i8, ptr %113, i16 4
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string50, ptr %114)
  %115 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %116 = getelementptr i8, ptr %115, i16 20
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string51, ptr %116)
  %117 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %118 = getelementptr i8, ptr %117, i16 36
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string52, ptr %118)
  %119 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %120 = getelementptr i8, ptr %119, i16 52
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string53, ptr %120)
  %121 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %122 = getelementptr i8, ptr %121, i16 68
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string54, ptr %122)
  %123 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %124 = getelementptr i8, ptr %123, i16 84
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string55, ptr %124)
  %125 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %126 = getelementptr i8, ptr %125, i16 100
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string56, ptr %126)
  %127 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %128 = getelementptr i8, ptr %127, i16 116
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string57, ptr %128)
  %129 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %130 = getelementptr i8, ptr %129, i16 132
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string58, ptr %130)
  %131 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %132 = getelementptr i8, ptr %131, i16 148
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string59, ptr %132)
  %133 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %134 = getelementptr i8, ptr %133, i16 164
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string60, ptr %134)
  %135 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %136 = getelementptr i8, ptr %135, i16 180
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string61, ptr %136)
  %137 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %138 = getelementptr i8, ptr %137, i16 8
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string62, ptr %138)
  %139 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %140 = getelementptr i8, ptr %139, i16 24
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string63, ptr %140)
  %141 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %142 = getelementptr i8, ptr %141, i16 40
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string64, ptr %142)
  %143 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %144 = getelementptr i8, ptr %143, i16 56
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string65, ptr %144)
  %145 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %146 = getelementptr i8, ptr %145, i16 72
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string66, ptr %146)
  %147 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %148 = getelementptr i8, ptr %147, i16 88
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string67, ptr %148)
  %149 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %150 = getelementptr i8, ptr %149, i16 104
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string68, ptr %150)
  %151 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %152 = getelementptr i8, ptr %151, i16 120
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string69, ptr %152)
  %153 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %154 = getelementptr i8, ptr %153, i16 136
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string70, ptr %154)
  %155 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %156 = getelementptr i8, ptr %155, i16 152
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string71, ptr %156)
  %157 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %158 = getelementptr i8, ptr %157, i16 168
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string72, ptr %158)
  %159 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %160 = getelementptr i8, ptr %159, i16 184
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string73, ptr %160)
  %161 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %162 = getelementptr i8, ptr %161, i16 12
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string74, ptr %162)
  %163 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %164 = getelementptr i8, ptr %163, i16 28
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string75, ptr %164)
  %165 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %166 = getelementptr i8, ptr %165, i16 44
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string76, ptr %166)
  %167 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %168 = getelementptr i8, ptr %167, i16 60
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string77, ptr %168)
  %169 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %170 = getelementptr i8, ptr %169, i16 76
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string78, ptr %170)
  %171 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %172 = getelementptr i8, ptr %171, i16 92
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string79, ptr %172)
  %173 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %174 = getelementptr i8, ptr %173, i16 108
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string80, ptr %174)
  %175 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %176 = getelementptr i8, ptr %175, i16 124
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string81, ptr %176)
  %177 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %178 = getelementptr i8, ptr %177, i16 140
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string82, ptr %178)
  %179 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %180 = getelementptr i8, ptr %179, i16 156
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string83, ptr %180)
  %181 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %182 = getelementptr i8, ptr %181, i16 172
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string84, ptr %182)
  %183 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %184 = getelementptr i8, ptr %183, i16 188
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string85, ptr %184)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  store i32 65536, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 32), align 16
  tail call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 32), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 34))
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 36), align 4
  store i32 256, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 40), align 8
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 44), align 4
  tail call cc 1000 addrspace(1) void @FADEFROMCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 36), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 38), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 40), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 42), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 44), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 46))
  %185 = load i16, ptr @"XIT%", !tbaa !2
  %186 = icmp eq i16 %185, 1
  br i1 %186, label %220, label %b30

b18:
  %187 = phi i16 [ %219, %b27 ], [ 0, %b16 ]
  %188 = phi i16 [ %218, %b27 ], [ 1, %b16 ]
  %189 = icmp sgt i16 %188, -1
  %190 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 8), align 8, !tbaa !2
  br i1 %189, label %b19, label %b20

b19:
  %191 = icmp sgt i16 %187, %190
  br i1 %191, label %b22, label %b21

b20:
  %192 = icmp slt i16 %187, %190
  br i1 %192, label %b22, label %b21

b21:
  store i16 0, ptr @"X%", !tbaa !2
  store i32 65551, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 12), align 4
  %193 = shl i16 %187, 4
  %194 = add i16 %193, %89
  br label %b23

b22:
  %195 = load i16, ptr @"I%", !tbaa !2
  %196 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 6), !tbaa !2
  %197 = add nsw i16 %196, %195
  store i16 %197, ptr @"I%", !tbaa !2
  br label %b13

b23:
  %198 = phi i16 [ %216, %204 ], [ 0, %b21 ]
  %199 = phi i16 [ %215, %204 ], [ 1, %b21 ]
  %200 = icmp sgt i16 %199, -1
  %201 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 12), align 4, !tbaa !2
  br i1 %200, label %b24, label %b25

b24:
  %202 = icmp sgt i16 %198, %201
  br i1 %202, label %b27, label %204

b25:
  %203 = icmp slt i16 %198, %201
  br i1 %203, label %b27, label %204

204:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) addrspacecast (ptr @C$ to ptr addrspace(1)), i16 0)
  %205 = load i16, ptr @"X%", !tbaa !2
  %206 = add i16 %194, %205
  %207 = shl i16 %206, 1
  %208 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FONTS%", i16 2), !tbaa !2
  %209 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FONTS%", i16 10), !tbaa !2
  %210 = inttoptr i16 %208 to ptr addrspace(2)
  %211 = addrspacecast ptr addrspace(2) %210 to ptr addrspace(1)
  %212 = getelementptr i8, ptr addrspace(1) %211, i16 %209
  %213 = getelementptr i8, ptr addrspace(1) %212, i16 %207
  %data2 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @C$, i16 2)
  %byte2 = load i8, ptr %data2
  %214 = zext i8 %byte2 to i16
  store i16 %214, ptr addrspace(1) %213, !tbaa !10
  %215 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 14), !tbaa !2
  %216 = add nsw i16 %215, %205
  store i16 %216, ptr @"X%", !tbaa !2
  br label %b23

b27:
  %217 = load i16, ptr @"Y%", !tbaa !2
  %218 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 10), !tbaa !2
  %219 = add nsw i16 %218, %217
  store i16 %219, ptr @"Y%", !tbaa !2
  br label %b18

220:
  store i16 2, ptr @"XIT%", !tbaa !2
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %221 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %222 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %223 = inttoptr i16 %221 to ptr addrspace(2)
  %224 = addrspacecast ptr addrspace(2) %223 to ptr addrspace(1)
  %225 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %226 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %227 = inttoptr i16 %225 to ptr addrspace(2)
  %228 = addrspacecast ptr addrspace(2) %227 to ptr addrspace(1)
  %229 = getelementptr i8, ptr addrspace(1) %228, i16 %226
  %230 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %231 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %232 = inttoptr i16 %230 to ptr addrspace(2)
  %233 = addrspacecast ptr addrspace(2) %232 to ptr addrspace(1)
  %234 = getelementptr i8, ptr addrspace(1) %233, i16 %231
  %235 = getelementptr i8, ptr addrspace(1) %224, i16 %222
  br label %236

236:
  %237 = phi i16 [ %250, %236 ], [ -512, %220 ]
  %238 = getelementptr i8, ptr addrspace(1) %235, i16 %237
  %239 = getelementptr i8, ptr addrspace(1) %238, i16 512
  %240 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %241 = zext i8 %240 to i16
  store i16 %241, ptr addrspace(1) %239, !tbaa !60
  %242 = getelementptr i8, ptr addrspace(1) %229, i16 %237
  %243 = getelementptr i8, ptr addrspace(1) %242, i16 512
  %244 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %245 = zext i8 %244 to i16
  store i16 %245, ptr addrspace(1) %243, !tbaa !62
  %246 = getelementptr i8, ptr addrspace(1) %234, i16 %237
  %247 = getelementptr i8, ptr addrspace(1) %246, i16 512
  %248 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %249 = zext i8 %248 to i16
  store i16 %249, ptr addrspace(1) %247, !tbaa !64
  %250 = add nsw i16 %237, 2
  %exitcond3 = icmp eq i16 %250, 0
  br i1 %exitcond3, label %251, label %236

251:
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 48), align 16
  store i32 64, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 52), align 4
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 56), align 8
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 48), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 50), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 52), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 54), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 56), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 58))
  br label %3

b30:
  tail call cc 1000 addrspace(1) void @CROSFADE(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %252 = load i16, ptr @"XIT%", !tbaa !2
  %253 = icmp eq i16 %252, 1
  br i1 %253, label %254, label %b34

254:
  store i16 2, ptr @"XIT%", !tbaa !2
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %255 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %256 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %257 = inttoptr i16 %255 to ptr addrspace(2)
  %258 = addrspacecast ptr addrspace(2) %257 to ptr addrspace(1)
  %259 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %260 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %261 = inttoptr i16 %259 to ptr addrspace(2)
  %262 = addrspacecast ptr addrspace(2) %261 to ptr addrspace(1)
  %263 = getelementptr i8, ptr addrspace(1) %262, i16 %260
  %264 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %265 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %266 = inttoptr i16 %264 to ptr addrspace(2)
  %267 = addrspacecast ptr addrspace(2) %266 to ptr addrspace(1)
  %268 = getelementptr i8, ptr addrspace(1) %267, i16 %265
  %269 = getelementptr i8, ptr addrspace(1) %258, i16 %256
  br label %270

270:
  %271 = phi i16 [ %284, %270 ], [ -512, %254 ]
  %272 = getelementptr i8, ptr addrspace(1) %269, i16 %271
  %273 = getelementptr i8, ptr addrspace(1) %272, i16 512
  %274 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %275 = zext i8 %274 to i16
  store i16 %275, ptr addrspace(1) %273, !tbaa !60
  %276 = getelementptr i8, ptr addrspace(1) %263, i16 %271
  %277 = getelementptr i8, ptr addrspace(1) %276, i16 512
  %278 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %279 = zext i8 %278 to i16
  store i16 %279, ptr addrspace(1) %277, !tbaa !62
  %280 = getelementptr i8, ptr addrspace(1) %268, i16 %271
  %281 = getelementptr i8, ptr addrspace(1) %280, i16 512
  %282 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %283 = zext i8 %282 to i16
  store i16 %283, ptr addrspace(1) %281, !tbaa !64
  %284 = add nsw i16 %271, 2
  %exitcond2 = icmp eq i16 %284, 0
  br i1 %exitcond2, label %285, label %270

285:
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 60), align 4
  store i32 64, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 64), align 16
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 68), align 4
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 60), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 62), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 64), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 66), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 68), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 70))
  br label %3

b34:
  store i32 65537, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 72), align 8
  tail call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 72), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 74))
  tail call cc 1000 addrspace(1) void @SETCROSFADEPAL(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  store i32 131073, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 76), align 4
  tail call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 76), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 78))
  %286 = load i16, ptr @"XIT%", !tbaa !2
  %287 = icmp eq i16 %286, 1
  br i1 %287, label %288, label %b38

288:
  store i16 2, ptr @"XIT%", !tbaa !2
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %289 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %290 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %291 = inttoptr i16 %289 to ptr addrspace(2)
  %292 = addrspacecast ptr addrspace(2) %291 to ptr addrspace(1)
  %293 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %294 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %295 = inttoptr i16 %293 to ptr addrspace(2)
  %296 = addrspacecast ptr addrspace(2) %295 to ptr addrspace(1)
  %297 = getelementptr i8, ptr addrspace(1) %296, i16 %294
  %298 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %299 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %300 = inttoptr i16 %298 to ptr addrspace(2)
  %301 = addrspacecast ptr addrspace(2) %300 to ptr addrspace(1)
  %302 = getelementptr i8, ptr addrspace(1) %301, i16 %299
  %303 = getelementptr i8, ptr addrspace(1) %292, i16 %290
  br label %304

304:
  %305 = phi i16 [ %318, %304 ], [ -512, %288 ]
  %306 = getelementptr i8, ptr addrspace(1) %303, i16 %305
  %307 = getelementptr i8, ptr addrspace(1) %306, i16 512
  %308 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %309 = zext i8 %308 to i16
  store i16 %309, ptr addrspace(1) %307, !tbaa !60
  %310 = getelementptr i8, ptr addrspace(1) %297, i16 %305
  %311 = getelementptr i8, ptr addrspace(1) %310, i16 512
  %312 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %313 = zext i8 %312 to i16
  store i16 %313, ptr addrspace(1) %311, !tbaa !62
  %314 = getelementptr i8, ptr addrspace(1) %302, i16 %305
  %315 = getelementptr i8, ptr addrspace(1) %314, i16 512
  %316 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %317 = zext i8 %316 to i16
  store i16 %317, ptr addrspace(1) %315, !tbaa !64
  %318 = add nsw i16 %305, 2
  %exitcond1 = icmp eq i16 %318, 0
  br i1 %exitcond1, label %319, label %304

319:
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 80), align 16
  store i32 64, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 84), align 4
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 88), align 8
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 80), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 82), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 84), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 86), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 88), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 90))
  br label %3

b38:
  tail call cc 1000 addrspace(1) void @CROSFADE(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %320 = load i16, ptr @"XIT%", !tbaa !2
  %321 = icmp eq i16 %320, 1
  br i1 %321, label %322, label %b46

322:
  store i16 2, ptr @"XIT%", !tbaa !2
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %323 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %324 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %325 = inttoptr i16 %323 to ptr addrspace(2)
  %326 = addrspacecast ptr addrspace(2) %325 to ptr addrspace(1)
  %327 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %328 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %329 = inttoptr i16 %327 to ptr addrspace(2)
  %330 = addrspacecast ptr addrspace(2) %329 to ptr addrspace(1)
  %331 = getelementptr i8, ptr addrspace(1) %330, i16 %328
  %332 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %333 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %334 = inttoptr i16 %332 to ptr addrspace(2)
  %335 = addrspacecast ptr addrspace(2) %334 to ptr addrspace(1)
  %336 = getelementptr i8, ptr addrspace(1) %335, i16 %333
  %337 = getelementptr i8, ptr addrspace(1) %326, i16 %324
  br label %338

338:
  %339 = phi i16 [ %352, %338 ], [ -512, %322 ]
  %340 = getelementptr i8, ptr addrspace(1) %337, i16 %339
  %341 = getelementptr i8, ptr addrspace(1) %340, i16 512
  %342 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %343 = zext i8 %342 to i16
  store i16 %343, ptr addrspace(1) %341, !tbaa !60
  %344 = getelementptr i8, ptr addrspace(1) %331, i16 %339
  %345 = getelementptr i8, ptr addrspace(1) %344, i16 512
  %346 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %347 = zext i8 %346 to i16
  store i16 %347, ptr addrspace(1) %345, !tbaa !62
  %348 = getelementptr i8, ptr addrspace(1) %336, i16 %339
  %349 = getelementptr i8, ptr addrspace(1) %348, i16 512
  %350 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %351 = zext i8 %350 to i16
  store i16 %351, ptr addrspace(1) %349, !tbaa !64
  %352 = add nsw i16 %339, 2
  %exitcond = icmp eq i16 %352, 0
  br i1 %exitcond, label %353, label %338

353:
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 92), align 4
  store i32 64, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 96), align 16
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 100), align 4
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 92), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 94), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 96), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 98), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 100), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 102))
  br label %3

b46:
  store i32 131074, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 116), align 4
  tail call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 116), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 118))
  tail call cc 1000 addrspace(1) void @SETCROSFADEPAL(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  store i32 196610, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 120), align 8
  tail call cc 1000 addrspace(1) void @INITCROSFADEPICS(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 120), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 122))
  tail call cc 1000 addrspace(1) void @CROSFADE(ptr @"R%", ptr @"R1%", ptr @"G%", ptr @"G1%", ptr @"B%", ptr @"B1%")
  %354 = load i16, ptr @"XIT%", !tbaa !2
  %355 = icmp eq i16 %354, 1
  br i1 %355, label %b48, label %b54

b48:
  store i16 2, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @GETPAL()
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 124), align 4
  store i32 64, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 128), align 16
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 132), align 4
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 124), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 126), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 128), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 130), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 132), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 134))
  br label %3

b54:
  tail call cc 1000 addrspace(1) void @GETPAL()
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 148), align 4
  store i32 4129024, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 152), align 8
  store i32 4128831, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 156), align 4
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 148), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 150), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 152), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 154), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 156), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 158))
  %356 = load i16, ptr @"XIT%", !tbaa !2
  %357 = icmp eq i16 %356, 1
  br i1 %357, label %b56, label %3

b56:
  store i16 2, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @GETPAL()
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 160), align 16
  store i32 64, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 164), align 4
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 168), align 8
  tail call cc 1000 addrspace(1) void @FADETOCOLOR(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 160), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 162), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 164), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 166), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 168), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 170))
  br label %3

b63:
  %358 = phi i16 [ -1654, %34 ], [ %373, %b66 ]
  %359 = phi i16 [ 0, %34 ], [ %372, %b66 ]
  %360 = sitofp i16 %359 to float
  %361 = fdiv float %360, 5.000000e+01
  %362 = tail call float @llvm.fabs.f32(float %361)
  %363 = fcmp ogt float %362, 0x41E0000000000000
  br i1 %363, label %b65, label %b66

b64:
  store i16 827, ptr @"I%", align 2
  store i16 826, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 192), align 16, !tbaa !2
  store i32 99, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 20), align 4, !tbaa !2
  store i32 1, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 24), align 8, !tbaa !2
  %364 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YPK&", i16 2), !tbaa !2
  %365 = inttoptr i16 %364 to ptr addrspace(2)
  %366 = addrspacecast ptr addrspace(2) %365 to ptr addrspace(1)
  br label %b70

b65:
  store i16 %359, ptr @"I%", align 2
  store i16 %359, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 192), align 16, !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b66:
  %367 = tail call float @llvm.sin.f32(float %361)
  store i16 48, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 196), align 4, !tbaa !2
  %368 = fmul float %367, 4.800000e+01
  %369 = tail call i16 @llvm.lrint.i16.f32(float %368)
  %370 = getelementptr i8, ptr @"FSIN1%", i16 %358
  %371 = getelementptr i8, ptr %370, i16 1750
  store i16 %369, ptr %371, !tbaa !2
  %372 = add nuw nsw i16 %359, 1
  %373 = add nsw i16 %358, 2
  %exitcond7 = icmp eq i16 %373, 0
  br i1 %exitcond7, label %b64, label %b63

b70:
  %374 = phi ptr addrspace(1) [ %366, %b64 ], [ %377, %b70 ]
  %375 = phi i32 [ 0, %b64 ], [ %376, %b70 ]
  store i32 %375, ptr addrspace(1) %374, !tbaa !12
  %376 = add nuw nsw i32 %375, 320
  %377 = getelementptr i8, ptr addrspace(1) %374, i16 4
  %exitcond6 = icmp eq i32 %376, 64000
  br i1 %exitcond6, label %378, label %b70

378:
  tail call cc 1000 addrspace(1) void @ACTIONS3D()
  store i16 5, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 198), !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 198))
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FSIN2%$descriptor")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FSIN3%$descriptor")
  tail call cc 1000 addrspace(1) void @CYCLEBLOBS()
  store i16 6, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 200), align 8, !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 200))
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  tail call cc 1000 addrspace(1) void @PLASMABLOBS()
  store i16 7, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 202), !tbaa !2
  tail call cc 1000 addrspace(1) void @BENCHMARK(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 202))
  store i16 0, ptr @"XIT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 0, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 320, i16 200)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 255, i16 -1, i16 2)
  store i32 16711680, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 204), align 4
  store i32 0, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 208), align 16
  store i32 4128768, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 212), align 4
  store i32 4128831, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 216), align 8
  tail call cc 1000 addrspace(1) void @SETPAL(ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 204), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 206), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 208), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 210), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 212), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 214), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 216), ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 218))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string87, ptr getelementptr inbounds nuw (i8, ptr @$data, i16 28))
  tail call cc 1000 addrspace(1) void @GETMAP(ptr getelementptr inbounds nuw (i8, ptr @$data, i16 28))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr getelementptr inbounds nuw (i8, ptr @$data, i16 28))
  tail call cc 1000 addrspace(1) void @PRECALCULATIONS()
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string88, ptr @K$)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string89, ptr @A$)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string90, ptr @GI$)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string91, ptr @GG$)
  store i16 0, ptr @"GI%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @A$, i16 2, i16 -1, i16 32)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GI$ to ptr addrspace(1)), i16 0)
  %data1 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GI$, i16 2)
  %byte1 = load i8, ptr %data1
  %379 = zext i8 %byte1 to i16
  store i16 %379, ptr @"GI%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GG$ to ptr addrspace(1)), i16 0)
  %data = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GG$, i16 2)
  %byte = load i8, ptr %data
  %380 = zext i8 %byte to i16
  store i16 %380, ptr @"GG%", !tbaa !2
  br label %b72

b72:
  %381 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %382 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %381, ptr @$string92)
  %383 = icmp ne i16 %382, 0
  br i1 %383, label %b72, label %b74

b74:
  store i16 96, ptr @"PX%", !tbaa !2
  store i16 96, ptr @"PY%", !tbaa !2
  store i16 90, ptr @"PA%", !tbaa !2
  store i16 3, ptr @"PS%", !tbaa !2
  store i32 20971580, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 220), align 4
  store float 1.875000e-01, ptr @"RA!", !tbaa !2
  store i16 30, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 224), align 16, !tbaa !2
  %384 = fdiv float 3.000000e+01, 0x404CA66660000000
  %385 = tail call float @llvm.fabs.f32(float %384)
  %386 = fcmp ogt float %385, 0x41E0000000000000
  br i1 %386, label %b75, label %b76

b75:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b76:
  %387 = tail call float @llvm.sin.f32(float %384)
  %388 = tail call float @llvm.cos.f32(float %384)
  %389 = fdiv float %387, %388
  store i16 160, ptr getelementptr inbounds nuw (i8, ptr @$arguments$85, i16 226), !tbaa !2
  %390 = fdiv float 1.600000e+02, %389
  %391 = tail call i16 @llvm.lrint.i16.f32(float %390)
  store i16 %391, ptr @"DPP%", !tbaa !2
  store i16 0, ptr @"MAP%", !tbaa !2
  store i16 0, ptr @"K%", !tbaa !2
  store i16 0, ptr @"FPS%", !tbaa !2
  store i16 0, ptr @"FILEI%", !tbaa !2
  %392 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %393 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %392, ptr @$string94)
  %394 = icmp eq i16 %393, 0
  br i1 %394, label %395, label %b3

395:
  br label %b78

b77:
  %396 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %397 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %396, ptr @$string94)
  %398 = icmp eq i16 %397, 0
  br i1 %398, label %b78, label %b3

b78:
  %399 = load i16, ptr @"FPS%", !tbaa !2
  %400 = add i16 %399, 1
  store i16 %400, ptr @"FPS%", !tbaa !2
  %401 = icmp slt i16 %400, 3500
  br i1 %401, label %402, label %406

402:
  %403 = icmp eq i16 %400, 558
  br i1 %403, label %410, label %404

404:
  %405 = icmp eq i16 %400, 2148
  br i1 %405, label %b83, label %411

406:
  %407 = icmp eq i16 %400, 3500
  br i1 %407, label %410, label %408

408:
  %409 = icmp eq i16 %400, 4760
  br i1 %409, label %b83, label %411

b83:
  br label %410

410:
  %sink = phi i16 [ 0, %b83 ], [ 1, %406 ], [ 1, %402 ]
  store i16 %sink, ptr @"MAP%", !tbaa !2
  br label %411

411:
  %412 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %413 = icmp eq i8 %412, 1
  br i1 %413, label %414, label %415

414:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %415

415:
  tail call cc 1000 addrspace(1) void @LOADRECORD()
  tail call cc 1000 addrspace(1) void @ACTIONS()
  %416 = load i16, ptr @"PY%", !tbaa !2
  %417 = sdiv i16 %416, 16
  %418 = getelementptr inbounds [2 x i8], ptr @"MUL128%", i16 %417
  %419 = load i16, ptr %418, !tbaa !2
  %420 = load i16, ptr @"PX%", !tbaa !2
  %421 = sdiv i16 %420, 16
  %422 = getelementptr [2 x i8], ptr @"SP%", i16 %419
  %423 = getelementptr [2 x i8], ptr %422, i16 %421
  store i16 64, ptr %423, !tbaa !2
  tail call cc 1000 addrspace(1) void @INTERSECTIONS()
  tail call cc 1000 addrspace(1) void @OUTPUT0()
  %424 = load i16, ptr @"FILEI%", !tbaa !2
  %425 = icmp eq i16 %424, 20
  br i1 %425, label %b3, label %b77
}

define cc 1000 void @ACTIONS() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = load i16, ptr @"KON%.3", !tbaa !2
  %1 = icmp eq i16 %0, 1
  br i1 %1, label %b8, label %b14

b8:
  %2 = load i16, ptr @"PA%", !tbaa !2
  %3 = add i16 %2, 1
  store i16 %3, ptr @"PA%", !tbaa !2
  %4 = icmp sgt i16 %3, 360
  br i1 %4, label %b10, label %b14

b10:
  %5 = add nsw i16 %2, -359
  store i16 %5, ptr @"PA%", !tbaa !2
  br label %b14

b14:
  %6 = load i16, ptr @"KON%.4", !tbaa !2
  %7 = icmp eq i16 %6, 1
  br i1 %7, label %b15, label %b21

b15:
  %8 = load i16, ptr @"PA%", !tbaa !2
  %9 = add i16 %8, -1
  store i16 %9, ptr @"PA%", !tbaa !2
  %10 = icmp slt i16 %9, 0
  br i1 %10, label %b17, label %b21

b17:
  %11 = add nsw i16 %8, 359
  store i16 %11, ptr @"PA%", !tbaa !2
  br label %b21

b21:
  %12 = load i16, ptr @"KON%.2", !tbaa !2
  %13 = icmp eq i16 %12, 1
  br i1 %13, label %b22, label %b35

b22:
  %14 = load i16, ptr @"PA%", !tbaa !2
  %15 = sitofp i16 %14 to float
  %16 = fdiv float %15, 0x404CA66660000000
  %17 = tail call float @llvm.fabs.f32(float %16)
  %18 = fcmp ogt float %17, 0x41E0000000000000
  br i1 %18, label %b24, label %b27

b24:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b27:
  %19 = tail call float @llvm.cos.f32(float %16)
  %20 = load i16, ptr @"PS%", !tbaa !2
  %21 = sitofp i16 %20 to float
  %22 = fmul float %19, %21
  %23 = tail call i16 @llvm.lrint.i16.f32(float %22)
  %24 = tail call float @llvm.sin.f32(float %16)
  %25 = fmul float %24, %21
  %26 = tail call i16 @llvm.lrint.i16.f32(float %25)
  %27 = load i16, ptr @"PX%", !tbaa !2
  %28 = add i16 %23, %27
  store i16 %28, ptr @"PX%", !tbaa !2
  %29 = load i16, ptr @"PY%", !tbaa !2
  %30 = sdiv i16 %29, 64
  %31 = getelementptr inbounds [2 x i8], ptr @"MUL32%", i16 %30
  %32 = load i16, ptr %31, !tbaa !2
  %33 = sdiv i16 %28, 64
  %34 = getelementptr [2 x i8], ptr @"CD%", i16 %32
  %35 = getelementptr [2 x i8], ptr %34, i16 %33
  %36 = load i16, ptr %35, !tbaa !2
  %37 = icmp eq i16 %36, 1
  br i1 %37, label %b28, label %b30

b28:
  store i16 %27, ptr @"PX%", !tbaa !2
  %38 = sdiv i16 %27, 64
  br label %b30

b30:
  %39 = phi i16 [ %33, %b27 ], [ %38, %b28 ]
  %40 = sub i16 %29, %26
  store i16 %40, ptr @"PY%", !tbaa !2
  %41 = sdiv i16 %40, 64
  %42 = getelementptr inbounds [2 x i8], ptr @"MUL32%", i16 %41
  %43 = load i16, ptr %42, !tbaa !2
  %44 = getelementptr [2 x i8], ptr @"CD%", i16 %43
  %45 = getelementptr [2 x i8], ptr %44, i16 %39
  %46 = load i16, ptr %45, !tbaa !2
  %47 = icmp eq i16 %46, 1
  %spec.store.select = select i1 %47, i16 %29, i16 %40
  store i16 %spec.store.select, ptr @"PY%"
  br label %b35

b35:
  %48 = load i16, ptr @"KON%.5", !tbaa !2
  %49 = icmp eq i16 %48, 1
  br i1 %49, label %b36, label %b49

b36:
  %50 = load i16, ptr @"PA%", !tbaa !2
  %51 = sitofp i16 %50 to float
  %52 = fdiv float %51, 0x404CA66660000000
  %53 = tail call float @llvm.fabs.f32(float %52)
  %54 = fcmp ogt float %53, 0x41E0000000000000
  br i1 %54, label %b38, label %b41

b38:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b41:
  %55 = tail call float @llvm.cos.f32(float %52)
  %56 = load i16, ptr @"PS%", !tbaa !2
  %57 = sitofp i16 %56 to float
  %58 = fmul float %55, %57
  %59 = tail call i16 @llvm.lrint.i16.f32(float %58)
  %60 = tail call float @llvm.sin.f32(float %52)
  %61 = fmul float %60, %57
  %62 = tail call i16 @llvm.lrint.i16.f32(float %61)
  %63 = load i16, ptr @"PX%", !tbaa !2
  %64 = sub i16 %63, %59
  store i16 %64, ptr @"PX%", !tbaa !2
  %65 = load i16, ptr @"PY%", !tbaa !2
  %66 = sdiv i16 %65, 64
  %67 = getelementptr inbounds [2 x i8], ptr @"MUL32%", i16 %66
  %68 = load i16, ptr %67, !tbaa !2
  %69 = sdiv i16 %64, 64
  %70 = getelementptr [2 x i8], ptr @"CD%", i16 %68
  %71 = getelementptr [2 x i8], ptr %70, i16 %69
  %72 = load i16, ptr %71, !tbaa !2
  %73 = icmp eq i16 %72, 1
  br i1 %73, label %b42, label %b44

b42:
  store i16 %63, ptr @"PX%", !tbaa !2
  %74 = sdiv i16 %63, 64
  br label %b44

b44:
  %75 = phi i16 [ %69, %b41 ], [ %74, %b42 ]
  %76 = add i16 %65, %62
  store i16 %76, ptr @"PY%", !tbaa !2
  %77 = sdiv i16 %76, 64
  %78 = getelementptr inbounds [2 x i8], ptr @"MUL32%", i16 %77
  %79 = load i16, ptr %78, !tbaa !2
  %80 = getelementptr [2 x i8], ptr @"CD%", i16 %79
  %81 = getelementptr [2 x i8], ptr %80, i16 %75
  %82 = load i16, ptr %81, !tbaa !2
  %83 = icmp eq i16 %82, 1
  %spec.store.select1 = select i1 %83, i16 %65, i16 %76
  store i16 %spec.store.select1, ptr @"PY%"
  br label %b49

b49:
  %84 = load i16, ptr @"KON%.1", !tbaa !2
  %85 = icmp eq i16 %84, 1
  br i1 %85, label %b50, label %b56

b50:
  %86 = load i16, ptr @"MAP%", !tbaa !2
  %87 = icmp eq i16 %86, 0
  %88 = zext i1 %87 to i16
  store i16 %88, ptr @"MAP%", !tbaa !2
  br label %b56

b56:
  %89 = load i16, ptr @"KON%.0", !tbaa !2
  %90 = icmp eq i16 %89, 1
  br i1 %90, label %b57, label %91

91:
  br label %b60

b57:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b60:
  %92 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %93 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %92, ptr @$string95)
  %94 = icmp ne i16 %93, 0
  br i1 %94, label %b60, label %b62

b62:
  ret void
}

define cc 1000 void @ACTIONS3D() addrspace(1) noinline "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = alloca float
  %1 = alloca float
  %2 = alloca float
  %3 = alloca [4 x i8]
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [4 x i8]
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
  %53 = alloca [4 x i8]
  %54 = alloca [4 x i8]
  %55 = alloca [4 x i8]
  store float 0.000000e+00, ptr %0
  store float 0.000000e+00, ptr %1
  store float 0.000000e+00, ptr %2
  call void @llvm.memset.p0.i16(ptr nonnull %3, i8 0, i16 4, i1 false)
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  call void @llvm.memset.p0.i16(ptr nonnull %12, i8 0, i16 4, i1 false)
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
  call void @llvm.memset.p0.i16(ptr nonnull %53, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %54, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %55, i8 0, i16 4, i1 false)
  store i16 0, ptr @"K%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string97, ptr @GI$)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string98, ptr @GG$)
  store float 0.000000e+00, ptr @"XP!", !tbaa !2
  %56 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZPO%", i16 2), !tbaa !2
  %57 = inttoptr i16 %56 to ptr addrspace(2)
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 2
  store i16 100, ptr addrspace(1) %59, !tbaa !14
  %60 = getelementptr i8, ptr addrspace(1) %58, i16 4
  store i16 200, ptr addrspace(1) %60, !tbaa !14
  %61 = getelementptr i8, ptr addrspace(1) %58, i16 6
  store i16 600, ptr addrspace(1) %61, !tbaa !14
  %62 = getelementptr i8, ptr addrspace(1) %58, i16 8
  store i16 800, ptr addrspace(1) %62, !tbaa !14
  %63 = getelementptr i8, ptr addrspace(1) %58, i16 10
  store i16 300, ptr addrspace(1) %63, !tbaa !14
  %64 = getelementptr i8, ptr addrspace(1) %58, i16 12
  store i16 300, ptr addrspace(1) %64, !tbaa !14
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 255, i16 2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string100, ptr nonnull %55)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr nonnull %55, i16 2, i16 -1, i16 32)
  store i16 0, ptr @"K%", !tbaa !2
  %65 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %65, ptr nonnull %54)
  %66 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %54, ptr @$string101)
  %67 = icmp eq i16 %66, 0
  br i1 %67, label %b6, label %71

b2:
  store i16 0, ptr @"FILEI%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 2, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %55)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %54)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %53)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %3)
  ret void

b4:
  %68 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %68, ptr nonnull %54)
  %69 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %54, ptr @$string101)
  %70 = icmp eq i16 %69, 0
  br i1 %70, label %b6, label %71

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

71:
  %72 = phi float [ 8.000000e+02, %b1 ], [ %694, %b4 ]
  %73 = phi float [ 0.000000e+00, %b1 ], [ %692, %b4 ]
  %74 = phi float [ 0.000000e+00, %b1 ], [ %621, %b4 ]
  %75 = phi float [ 0.000000e+00, %b1 ], [ %585, %b4 ]
  %76 = phi float [ 0.000000e+00, %b1 ], [ %603, %b4 ]
  %77 = phi float [ 0.000000e+00, %b1 ], [ %539, %b4 ]
  %78 = phi float [ 0.000000e+00, %b1 ], [ %467, %b4 ]
  %79 = phi float [ 0.000000e+00, %b1 ], [ %509, %b4 ]
  %80 = load i16, ptr @"GGI%", !tbaa !2
  %81 = load i16, ptr @"GI%", !tbaa !2
  %82 = icmp slt i16 %80, %81
  br i1 %82, label %83, label %85

83:
  %84 = add nsw i16 %80, 1
  store i16 %84, ptr @"GGI%", !tbaa !2
  %data.pre = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GG$, i16 2)
  br label %90

85:
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GI$ to ptr addrspace(1)), i16 0)
  %data1 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GI$, i16 2)
  %byte1 = load i8, ptr %data1
  %86 = zext i8 %byte1 to i16
  store i16 %86, ptr @"GI%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GG$ to ptr addrspace(1)), i16 0)
  %data2 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GG$, i16 2)
  %byte2 = load i8, ptr %data2
  %87 = zext i8 %byte2 to i16
  store i16 %87, ptr @"GG%", !tbaa !2
  store i16 1, ptr @"GGI%", !tbaa !2
  %88 = load i16, ptr @"FILEI%", !tbaa !2
  %89 = add i16 %88, 2
  store i16 %89, ptr @"FILEI%", !tbaa !2
  br label %90

90:
  %data3 = phi ptr [ %data2, %85 ], [ %data.pre, %83 ]
  %byte = load i8, ptr %data3
  %91 = icmp eq i8 %byte, 0
  %92 = select i1 %91, ptr @$string102, ptr @GG$
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr nonnull %92, ptr nonnull %53)
  %93 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string103)
  %94 = icmp eq i16 %93, 0
  %95 = sext i1 %94 to i16
  %96 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string104)
  %97 = icmp eq i16 %96, 0
  %98 = sext i1 %97 to i16
  %99 = or i16 %98, %95
  %100 = icmp eq i16 %99, 0
  br i1 %100, label %b15, label %101

101:
  store i16 0, ptr %52, !tbaa !2
  store i16 255, ptr %51, !tbaa !2
  store i16 0, ptr %50, !tbaa !2
  store i16 0, ptr %49, !tbaa !2
  store i16 0, ptr %48, !tbaa !2
  store i16 63, ptr %47, !tbaa !2
  store i16 63, ptr %46, !tbaa !2
  store i16 63, ptr %45, !tbaa !2
  br label %b14

b14:
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %52, ptr nonnull %51, ptr nonnull %50, ptr nonnull %49, ptr nonnull %48, ptr nonnull %47, ptr nonnull %46, ptr nonnull %45)
  store i16 1, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 64, ptr @"NDTSO%", !tbaa !2
  store i16 64, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string105, ptr nonnull %53)
  %102 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string103)
  %103 = icmp eq i16 %102, 0
  %104 = sext i1 %103 to i16
  %105 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string104)
  %106 = icmp eq i16 %105, 0
  %107 = sext i1 %106 to i16
  %108 = or i16 %107, %104
  %109 = icmp eq i16 %108, 0
  br i1 %109, label %b15, label %b14

b15:
  %110 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string106)
  %111 = icmp eq i16 %110, 0
  %112 = sext i1 %111 to i16
  %113 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string107)
  %114 = icmp eq i16 %113, 0
  %115 = sext i1 %114 to i16
  %116 = or i16 %115, %112
  %117 = icmp eq i16 %116, 0
  br i1 %117, label %b18, label %118

118:
  store i16 0, ptr %44, !tbaa !2
  store i16 255, ptr %43, !tbaa !2
  store i16 0, ptr %42, !tbaa !2
  store i16 0, ptr %41, !tbaa !2
  store i16 0, ptr %40, !tbaa !2
  store i16 63, ptr %39, !tbaa !2
  store i16 47, ptr %38, !tbaa !2
  store i16 15, ptr %37, !tbaa !2
  br label %b17

b17:
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %44, ptr nonnull %43, ptr nonnull %42, ptr nonnull %41, ptr nonnull %40, ptr nonnull %39, ptr nonnull %38, ptr nonnull %37)
  store i16 2, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 256, ptr @"NDTSO%", !tbaa !2
  store i16 256, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string108, ptr nonnull %53)
  %119 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string106)
  %120 = icmp eq i16 %119, 0
  %121 = sext i1 %120 to i16
  %122 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string107)
  %123 = icmp eq i16 %122, 0
  %124 = sext i1 %123 to i16
  %125 = or i16 %124, %121
  %126 = icmp eq i16 %125, 0
  br i1 %126, label %b18, label %b17

b18:
  %127 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string109)
  %128 = icmp eq i16 %127, 0
  %129 = sext i1 %128 to i16
  %130 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string110)
  %131 = icmp eq i16 %130, 0
  %132 = sext i1 %131 to i16
  %133 = or i16 %132, %129
  %134 = icmp eq i16 %133, 0
  br i1 %134, label %b21, label %135

135:
  store i16 0, ptr %36, !tbaa !2
  store i16 255, ptr %35, !tbaa !2
  store i16 0, ptr %34, !tbaa !2
  store i16 0, ptr %33, !tbaa !2
  store i16 0, ptr %32, !tbaa !2
  store i16 31, ptr %31, !tbaa !2
  store i16 63, ptr %30, !tbaa !2
  store i16 47, ptr %29, !tbaa !2
  br label %b20

b20:
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %36, ptr nonnull %35, ptr nonnull %34, ptr nonnull %33, ptr nonnull %32, ptr nonnull %31, ptr nonnull %30, ptr nonnull %29)
  store i16 3, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 512, ptr @"NDTSO%", !tbaa !2
  store i16 512, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string111, ptr nonnull %53)
  %136 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string109)
  %137 = icmp eq i16 %136, 0
  %138 = sext i1 %137 to i16
  %139 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string110)
  %140 = icmp eq i16 %139, 0
  %141 = sext i1 %140 to i16
  %142 = or i16 %141, %138
  %143 = icmp eq i16 %142, 0
  br i1 %143, label %b21, label %b20

b21:
  %144 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string112)
  %145 = icmp eq i16 %144, 0
  %146 = sext i1 %145 to i16
  %147 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string113)
  %148 = icmp eq i16 %147, 0
  %149 = sext i1 %148 to i16
  %150 = or i16 %149, %146
  %151 = icmp eq i16 %150, 0
  br i1 %151, label %b24, label %152

152:
  store i16 0, ptr %28, !tbaa !2
  store i16 255, ptr %27, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  store i16 15, ptr %23, !tbaa !2
  store i16 31, ptr %22, !tbaa !2
  store i16 63, ptr %21, !tbaa !2
  br label %b23

b23:
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %28, ptr nonnull %27, ptr nonnull %26, ptr nonnull %25, ptr nonnull %24, ptr nonnull %23, ptr nonnull %22, ptr nonnull %21)
  store i16 4, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @CREATEOBJECT(ptr @"OBJ%")
  store i16 64, ptr @"NDTSO%", !tbaa !2
  store i16 0, ptr @"NDTS%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string114, ptr nonnull %53)
  %153 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string112)
  %154 = icmp eq i16 %153, 0
  %155 = sext i1 %154 to i16
  %156 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string113)
  %157 = icmp eq i16 %156, 0
  %158 = sext i1 %157 to i16
  %159 = or i16 %158, %155
  %160 = icmp eq i16 %159, 0
  br i1 %160, label %b24, label %b23

b24:
  %161 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string115)
  %162 = icmp eq i16 %161, 0
  %163 = sext i1 %162 to i16
  %164 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string116)
  %165 = icmp eq i16 %164, 0
  %166 = sext i1 %165 to i16
  %167 = or i16 %166, %163
  %168 = icmp eq i16 %167, 0
  br i1 %168, label %b27, label %169

169:
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %19, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i16 63, ptr %14, !tbaa !2
  store i16 63, ptr %13, !tbaa !2
  br label %b26

b26:
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %20, ptr nonnull %19, ptr nonnull %18, ptr nonnull %17, ptr nonnull %16, ptr nonnull %15, ptr nonnull %14, ptr nonnull %13)
  store i16 5, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string117, ptr nonnull %12)
  call cc 1000 addrspace(1) void @LOADOBJECT(ptr nonnull %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %12)
  store i16 0, ptr @"NDTSO%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string118, ptr nonnull %53)
  %170 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string115)
  %171 = icmp eq i16 %170, 0
  %172 = sext i1 %171 to i16
  %173 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string116)
  %174 = icmp eq i16 %173, 0
  %175 = sext i1 %174 to i16
  %176 = or i16 %175, %172
  %177 = icmp eq i16 %176, 0
  br i1 %177, label %b27, label %b26

b27:
  %178 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string119)
  %179 = icmp eq i16 %178, 0
  %180 = sext i1 %179 to i16
  %181 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string120)
  %182 = icmp eq i16 %181, 0
  %183 = sext i1 %182 to i16
  %184 = or i16 %183, %180
  %185 = icmp eq i16 %184, 0
  br i1 %185, label %b30, label %186

186:
  store i16 0, ptr %11, !tbaa !2
  store i16 255, ptr %10, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i16 0, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 63, ptr %6, !tbaa !2
  store i16 31, ptr %5, !tbaa !2
  store i16 47, ptr %4, !tbaa !2
  br label %b29

b29:
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %11, ptr nonnull %10, ptr nonnull %9, ptr nonnull %8, ptr nonnull %7, ptr nonnull %6, ptr nonnull %5, ptr nonnull %4)
  store i16 6, ptr @"OBJ%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string121, ptr nonnull %3)
  call cc 1000 addrspace(1) void @LOADOBJECT(ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %3)
  store i16 0, ptr @"NDTSO%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string122, ptr nonnull %53)
  %187 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string119)
  %188 = icmp eq i16 %187, 0
  %189 = sext i1 %188 to i16
  %190 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string120)
  %191 = icmp eq i16 %190, 0
  %192 = sext i1 %191 to i16
  %193 = or i16 %192, %189
  %194 = icmp eq i16 %193, 0
  br i1 %194, label %b30, label %b29

b30:
  %195 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string123)
  %196 = icmp eq i16 %195, 0
  %197 = sext i1 %196 to i16
  %198 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string124)
  %199 = icmp eq i16 %198, 0
  %200 = sext i1 %199 to i16
  %201 = or i16 %200, %197
  %202 = icmp eq i16 %201, 0
  br i1 %202, label %b33, label %203

203:
  br label %204

204:
  %205 = phi i16 [ 0, %203 ], [ %217, %216 ], [ 0, %b36 ]
  %206 = mul nuw nsw i16 %205, 20
  %207 = add nuw nsw i16 %206, -24576
  store i16 %207, ptr @b$seg, !tbaa !2
  br label %208

208:
  %209 = phi i16 [ %215, %208 ], [ -320, %204 ]
  %210 = load i16, ptr @b$seg, !tbaa !2
  %211 = inttoptr i16 %210 to ptr addrspace(2)
  %212 = addrspacecast ptr addrspace(2) %211 to ptr addrspace(1)
  %213 = getelementptr i8, ptr addrspace(1) %212, i16 %209
  %214 = getelementptr i8, ptr addrspace(1) %213, i16 320
  store volatile i8 0, ptr addrspace(1) %214
  %215 = add nsw i16 %209, 1
  %exitcond2 = icmp eq i16 %215, 0
  br i1 %exitcond2, label %216, label %208

216:
  %217 = add nuw nsw i16 %205, 1
  %exitcond12 = icmp eq i16 %217, 200
  br i1 %exitcond12, label %218, label %204

218:
  store i16 -24576, ptr @b$seg, !tbaa !2
  %219 = load i16, ptr @"NDTS%", !tbaa !2
  %220 = load i16, ptr @"NDTSO%", !tbaa !2
  %221 = sub i16 %219, %220
  %222 = icmp sgt i16 %221, 0
  br i1 %222, label %b34, label %b36

b33:
  %223 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string126)
  %224 = icmp eq i16 %223, 0
  %225 = sext i1 %224 to i16
  %226 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string127)
  %227 = icmp eq i16 %226, 0
  %228 = sext i1 %227 to i16
  %229 = or i16 %228, %225
  %230 = icmp eq i16 %229, 0
  br i1 %230, label %b39, label %231

231:
  %232 = load i16, ptr @"NDTSO%", !tbaa !2
  br label %b38

b34:
  store i16 %221, ptr @"NDTS%", !tbaa !2
  br label %b36

b36:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string125, ptr nonnull %53)
  %233 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string123)
  %234 = icmp eq i16 %233, 0
  %235 = sext i1 %234 to i16
  %236 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string124)
  %237 = icmp eq i16 %236, 0
  %238 = sext i1 %237 to i16
  %239 = or i16 %238, %235
  %240 = icmp eq i16 %239, 0
  br i1 %240, label %b33, label %204

b38:
  %241 = load i16, ptr @"NDTS%", !tbaa !2
  %242 = add i16 %241, %232
  %243 = icmp slt i16 %242, 4096
  br i1 %243, label %b40, label %b42

b39:
  %244 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string129)
  %245 = icmp eq i16 %244, 0
  %246 = sext i1 %245 to i16
  %247 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string130)
  %248 = icmp eq i16 %247, 0
  %249 = sext i1 %248 to i16
  %250 = or i16 %249, %246
  %251 = icmp eq i16 %250, 0
  br i1 %251, label %b45, label %252

252:
  br label %b44

b40:
  store i16 %242, ptr @"NDTS%", !tbaa !2
  br label %b42

b42:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string128, ptr nonnull %53)
  %253 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string126)
  %254 = icmp eq i16 %253, 0
  %255 = sext i1 %254 to i16
  %256 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string127)
  %257 = icmp eq i16 %256, 0
  %258 = sext i1 %257 to i16
  %259 = or i16 %258, %255
  %260 = icmp eq i16 %259, 0
  br i1 %260, label %b39, label %b38

b44:
  %261 = load i16, ptr @"OBJ%", !tbaa !2
  %262 = shl i16 %261, 1
  %263 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZPO%", i16 2), !tbaa !2
  %264 = inttoptr i16 %263 to ptr addrspace(2)
  %265 = addrspacecast ptr addrspace(2) %264 to ptr addrspace(1)
  %266 = getelementptr i8, ptr addrspace(1) %265, i16 %262
  %267 = load i16, ptr addrspace(1) %266, !tbaa !14
  %268 = sub i16 0, %267
  %269 = sitofp i16 %268 to float
  %270 = fmul float %269, 1.500000e+00
  %271 = fpround float %270 to float
  store float %271, ptr @"XP!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string132, ptr nonnull %53)
  %272 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string129)
  %273 = icmp eq i16 %272, 0
  %274 = sext i1 %273 to i16
  %275 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string130)
  %276 = icmp eq i16 %275, 0
  %277 = sext i1 %276 to i16
  %278 = or i16 %277, %274
  %279 = icmp eq i16 %278, 0
  br i1 %279, label %280, label %b44

280:
  %281 = sitofp i16 %267 to float
  %282 = fpround float %281 to float
  br label %b45

b45:
  %283 = phi float [ %72, %b39 ], [ %282, %280 ]
  %284 = phi float [ %73, %b39 ], [ 0.000000e+00, %280 ]
  %285 = phi float [ %77, %b39 ], [ 0.000000e+00, %280 ]
  %286 = phi float [ %78, %b39 ], [ 0.000000e+00, %280 ]
  %287 = phi float [ %79, %b39 ], [ 0.000000e+00, %280 ]
  %288 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string133)
  %289 = icmp eq i16 %288, 0
  %290 = sext i1 %289 to i16
  %291 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string134)
  %292 = icmp eq i16 %291, 0
  %293 = sext i1 %292 to i16
  %294 = or i16 %293, %290
  %295 = icmp eq i16 %294, 0
  br i1 %295, label %b48, label %296

296:
  br label %b47

b47:
  %297 = load i16, ptr @"OBJ%", !tbaa !2
  %298 = shl i16 %297, 1
  %299 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZPO%", i16 2), !tbaa !2
  %300 = inttoptr i16 %299 to ptr addrspace(2)
  %301 = addrspacecast ptr addrspace(2) %300 to ptr addrspace(1)
  %302 = getelementptr i8, ptr addrspace(1) %301, i16 %298
  %303 = load i16, ptr addrspace(1) %302, !tbaa !14
  %304 = sitofp i16 %303 to float
  %305 = fmul float %304, 1.500000e+00
  %306 = fpround float %305 to float
  store float %306, ptr @"XP!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string135, ptr nonnull %53)
  %307 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string133)
  %308 = icmp eq i16 %307, 0
  %309 = sext i1 %308 to i16
  %310 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string134)
  %311 = icmp eq i16 %310, 0
  %312 = sext i1 %311 to i16
  %313 = or i16 %312, %309
  %314 = icmp eq i16 %313, 0
  br i1 %314, label %315, label %b47

315:
  %316 = fpround float %304 to float
  br label %b48

b48:
  %317 = phi float [ %283, %b45 ], [ %316, %315 ]
  %318 = phi float [ %284, %b45 ], [ 0.000000e+00, %315 ]
  %319 = phi float [ %285, %b45 ], [ 0.000000e+00, %315 ]
  %320 = phi float [ %286, %b45 ], [ 0.000000e+00, %315 ]
  %321 = phi float [ %287, %b45 ], [ 0.000000e+00, %315 ]
  %322 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string136)
  %323 = icmp eq i16 %322, 0
  %324 = sext i1 %323 to i16
  %325 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string137)
  %326 = icmp eq i16 %325, 0
  %327 = sext i1 %326 to i16
  %328 = or i16 %327, %324
  %329 = icmp eq i16 %328, 0
  br i1 %329, label %b51, label %330

330:
  br label %b50

b50:
  store float 0.000000e+00, ptr @"XP!", !tbaa !2
  %331 = load i16, ptr @"OBJ%", !tbaa !2
  %332 = shl i16 %331, 1
  %333 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZPO%", i16 2), !tbaa !2
  %334 = inttoptr i16 %333 to ptr addrspace(2)
  %335 = addrspacecast ptr addrspace(2) %334 to ptr addrspace(1)
  %336 = getelementptr i8, ptr addrspace(1) %335, i16 %332
  %337 = load i16, ptr addrspace(1) %336, !tbaa !14
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string138, ptr nonnull %53)
  %338 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string136)
  %339 = icmp eq i16 %338, 0
  %340 = sext i1 %339 to i16
  %341 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string137)
  %342 = icmp eq i16 %341, 0
  %343 = sext i1 %342 to i16
  %344 = or i16 %343, %340
  %345 = icmp eq i16 %344, 0
  br i1 %345, label %346, label %b50

346:
  %347 = sitofp i16 %337 to float
  %348 = fmul float %347, 1.500000e+00
  %349 = fpround float %348 to float
  %350 = fpround float %347 to float
  br label %b51

b51:
  %351 = phi float [ %317, %b48 ], [ %350, %346 ]
  %352 = phi float [ %318, %b48 ], [ %349, %346 ]
  %353 = phi float [ %319, %b48 ], [ 0.000000e+00, %346 ]
  %354 = phi float [ %320, %b48 ], [ 0.000000e+00, %346 ]
  %355 = phi float [ %321, %b48 ], [ 0.000000e+00, %346 ]
  %356 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string139)
  %357 = icmp eq i16 %356, 0
  %358 = sext i1 %357 to i16
  %359 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string140)
  %360 = icmp eq i16 %359, 0
  %361 = sext i1 %360 to i16
  %362 = or i16 %361, %358
  %363 = icmp eq i16 %362, 0
  br i1 %363, label %b54, label %364

364:
  br label %b53

b53:
  store float 0.000000e+00, ptr @"XP!", !tbaa !2
  %365 = load i16, ptr @"OBJ%", !tbaa !2
  %366 = shl i16 %365, 1
  %367 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZPO%", i16 2), !tbaa !2
  %368 = inttoptr i16 %367 to ptr addrspace(2)
  %369 = addrspacecast ptr addrspace(2) %368 to ptr addrspace(1)
  %370 = getelementptr i8, ptr addrspace(1) %369, i16 %366
  %371 = load i16, ptr addrspace(1) %370, !tbaa !14
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string141, ptr nonnull %53)
  %372 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string139)
  %373 = icmp eq i16 %372, 0
  %374 = sext i1 %373 to i16
  %375 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string140)
  %376 = icmp eq i16 %375, 0
  %377 = sext i1 %376 to i16
  %378 = or i16 %377, %374
  %379 = icmp eq i16 %378, 0
  br i1 %379, label %380, label %b53

380:
  %381 = sub i16 0, %371
  %382 = sitofp i16 %381 to float
  %383 = fmul float %382, 1.500000e+00
  %384 = fpround float %383 to float
  %385 = sitofp i16 %371 to float
  %386 = fpround float %385 to float
  br label %b54

b54:
  %387 = phi float [ %351, %b51 ], [ %386, %380 ]
  %388 = phi float [ %352, %b51 ], [ %384, %380 ]
  %389 = phi float [ %353, %b51 ], [ 0.000000e+00, %380 ]
  %390 = phi float [ %354, %b51 ], [ 0.000000e+00, %380 ]
  %391 = phi float [ %355, %b51 ], [ 0.000000e+00, %380 ]
  %392 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string142)
  %393 = icmp eq i16 %392, 0
  %394 = sext i1 %393 to i16
  %395 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string143)
  %396 = icmp eq i16 %395, 0
  %397 = sext i1 %396 to i16
  %398 = or i16 %397, %394
  %399 = icmp eq i16 %398, 0
  br i1 %399, label %b57, label %400

400:
  br label %b56

b56:
  store float 0.000000e+00, ptr @"XP!", !tbaa !2
  %401 = load i16, ptr @"OBJ%", !tbaa !2
  %402 = shl i16 %401, 1
  %403 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZPO%", i16 2), !tbaa !2
  %404 = inttoptr i16 %403 to ptr addrspace(2)
  %405 = addrspacecast ptr addrspace(2) %404 to ptr addrspace(1)
  %406 = getelementptr i8, ptr addrspace(1) %405, i16 %402
  %407 = load i16, ptr addrspace(1) %406, !tbaa !14
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string144, ptr nonnull %53)
  %408 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string142)
  %409 = icmp eq i16 %408, 0
  %410 = sext i1 %409 to i16
  %411 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string143)
  %412 = icmp eq i16 %411, 0
  %413 = sext i1 %412 to i16
  %414 = or i16 %413, %410
  %415 = icmp eq i16 %414, 0
  br i1 %415, label %416, label %b56

416:
  %417 = sub i16 0, %407
  %418 = sdiv i16 %417, 2
  %419 = sitofp i16 %418 to float
  %420 = fpround float %419 to float
  br label %b57

b57:
  %421 = phi float [ %387, %b54 ], [ %420, %416 ]
  %422 = phi float [ %388, %b54 ], [ 0.000000e+00, %416 ]
  %423 = phi float [ %389, %b54 ], [ 0.000000e+00, %416 ]
  %424 = phi float [ %390, %b54 ], [ 0.000000e+00, %416 ]
  %425 = phi float [ %391, %b54 ], [ 0.000000e+00, %416 ]
  %426 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string145)
  %427 = icmp eq i16 %426, 0
  %428 = sext i1 %427 to i16
  %429 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string146)
  %430 = icmp eq i16 %429, 0
  %431 = sext i1 %430 to i16
  %432 = or i16 %431, %428
  %433 = icmp eq i16 %432, 0
  br i1 %433, label %b60, label %434

434:
  br label %b59

b59:
  %435 = phi float [ %437, %b59 ], [ %424, %434 ]
  %436 = fadd float %435, 0x3FB99999A0000000
  %437 = fpround float %436 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string148, ptr nonnull %53)
  %438 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string145)
  %439 = icmp eq i16 %438, 0
  %440 = sext i1 %439 to i16
  %441 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string146)
  %442 = icmp eq i16 %441, 0
  %443 = sext i1 %442 to i16
  %444 = or i16 %443, %440
  %445 = icmp eq i16 %444, 0
  br i1 %445, label %b60, label %b59

b60:
  %446 = phi float [ %424, %b57 ], [ %437, %b59 ]
  %447 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string149)
  %448 = icmp eq i16 %447, 0
  %449 = sext i1 %448 to i16
  %450 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string150)
  %451 = icmp eq i16 %450, 0
  %452 = sext i1 %451 to i16
  %453 = or i16 %452, %449
  %454 = icmp eq i16 %453, 0
  br i1 %454, label %b63, label %455

455:
  br label %b62

b62:
  %456 = phi float [ %458, %b62 ], [ %446, %455 ]
  %457 = fsub float %456, 0x3FB99999A0000000
  %458 = fpround float %457 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string151, ptr nonnull %53)
  %459 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string149)
  %460 = icmp eq i16 %459, 0
  %461 = sext i1 %460 to i16
  %462 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string150)
  %463 = icmp eq i16 %462, 0
  %464 = sext i1 %463 to i16
  %465 = or i16 %464, %461
  %466 = icmp eq i16 %465, 0
  br i1 %466, label %b63, label %b62

b63:
  %467 = phi float [ %446, %b60 ], [ %458, %b62 ]
  %468 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string152)
  %469 = icmp eq i16 %468, 0
  %470 = sext i1 %469 to i16
  %471 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string153)
  %472 = icmp eq i16 %471, 0
  %473 = sext i1 %472 to i16
  %474 = or i16 %473, %470
  %475 = icmp eq i16 %474, 0
  br i1 %475, label %b66, label %476

476:
  br label %b65

b65:
  %477 = phi float [ %479, %b65 ], [ %425, %476 ]
  %478 = fsub float %477, 0x3FB99999A0000000
  %479 = fpround float %478 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string154, ptr nonnull %53)
  %480 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string152)
  %481 = icmp eq i16 %480, 0
  %482 = sext i1 %481 to i16
  %483 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string153)
  %484 = icmp eq i16 %483, 0
  %485 = sext i1 %484 to i16
  %486 = or i16 %485, %482
  %487 = icmp eq i16 %486, 0
  br i1 %487, label %b66, label %b65

b66:
  %488 = phi float [ %425, %b63 ], [ %479, %b65 ]
  %489 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string155)
  %490 = icmp eq i16 %489, 0
  %491 = sext i1 %490 to i16
  %492 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string156)
  %493 = icmp eq i16 %492, 0
  %494 = sext i1 %493 to i16
  %495 = or i16 %494, %491
  %496 = icmp eq i16 %495, 0
  br i1 %496, label %b69, label %497

497:
  br label %b68

b68:
  %498 = phi float [ %500, %b68 ], [ %488, %497 ]
  %499 = fadd float %498, 0x3FB99999A0000000
  %500 = fpround float %499 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string157, ptr nonnull %53)
  %501 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string155)
  %502 = icmp eq i16 %501, 0
  %503 = sext i1 %502 to i16
  %504 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string156)
  %505 = icmp eq i16 %504, 0
  %506 = sext i1 %505 to i16
  %507 = or i16 %506, %503
  %508 = icmp eq i16 %507, 0
  br i1 %508, label %b69, label %b68

b69:
  %509 = phi float [ %488, %b66 ], [ %500, %b68 ]
  %510 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string158)
  %511 = icmp eq i16 %510, 0
  br i1 %511, label %512, label %b72

512:
  br label %b71

b71:
  %513 = phi float [ %515, %b71 ], [ %423, %512 ]
  %514 = fadd float %513, 2.500000e-01
  %515 = fpround float %514 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string160, ptr nonnull %53)
  %516 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string158)
  %517 = icmp eq i16 %516, 0
  br i1 %517, label %b71, label %b72

b72:
  %518 = phi float [ %423, %b69 ], [ %515, %b71 ]
  %519 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string161)
  %520 = icmp eq i16 %519, 0
  %521 = sext i1 %520 to i16
  %522 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string162)
  %523 = icmp eq i16 %522, 0
  %524 = sext i1 %523 to i16
  %525 = or i16 %524, %521
  %526 = icmp eq i16 %525, 0
  br i1 %526, label %b75, label %527

527:
  br label %b74

b74:
  %528 = phi float [ %530, %b74 ], [ %518, %527 ]
  %529 = fsub float %528, 2.500000e-01
  %530 = fpround float %529 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string163, ptr nonnull %53)
  %531 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string161)
  %532 = icmp eq i16 %531, 0
  %533 = sext i1 %532 to i16
  %534 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string162)
  %535 = icmp eq i16 %534, 0
  %536 = sext i1 %535 to i16
  %537 = or i16 %536, %533
  %538 = icmp eq i16 %537, 0
  br i1 %538, label %b75, label %b74

b75:
  %539 = phi float [ %518, %b72 ], [ %530, %b74 ]
  %540 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string164)
  %541 = icmp eq i16 %540, 0
  %542 = sext i1 %541 to i16
  %543 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string165)
  %544 = icmp eq i16 %543, 0
  %545 = sext i1 %544 to i16
  %546 = or i16 %545, %542
  %547 = icmp eq i16 %546, 0
  br i1 %547, label %b78, label %548

548:
  store float 0.000000e+00, ptr %2, !tbaa !2
  store float 0.000000e+00, ptr %1, !tbaa !2
  store float 0.000000e+00, ptr %0, !tbaa !2
  br label %b77

b77:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string166, ptr nonnull %53)
  %549 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string164)
  %550 = icmp eq i16 %549, 0
  %551 = sext i1 %550 to i16
  %552 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string165)
  %553 = icmp eq i16 %552, 0
  %554 = sext i1 %553 to i16
  %555 = or i16 %554, %551
  %556 = icmp eq i16 %555, 0
  br i1 %556, label %b78, label %b77

b78:
  %557 = phi float [ %74, %b75 ], [ 0.000000e+00, %b77 ]
  %558 = phi float [ %75, %b75 ], [ 0.000000e+00, %b77 ]
  %559 = phi float [ %76, %b75 ], [ 0.000000e+00, %b77 ]
  %560 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string167)
  %561 = icmp eq i16 %560, 0
  %562 = sext i1 %561 to i16
  %563 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string168)
  %564 = icmp eq i16 %563, 0
  %565 = sext i1 %564 to i16
  %566 = or i16 %565, %562
  %567 = icmp eq i16 %566, 0
  br i1 %567, label %b81, label %b80

b80:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b81:
  %568 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string169)
  %569 = icmp eq i16 %568, 0
  br i1 %569, label %570, label %b85

570:
  br label %b84

b84:
  %571 = phi float [ %573, %b84 ], [ %558, %570 ]
  %572 = fsub float %571, 0x3F747AE140000000
  %573 = fpround float %572 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string171, ptr nonnull %53)
  %574 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string169)
  %575 = icmp eq i16 %574, 0
  br i1 %575, label %b84, label %b85

b85:
  %576 = phi float [ %558, %b81 ], [ %573, %b84 ]
  %577 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string172)
  %578 = icmp eq i16 %577, 0
  br i1 %578, label %579, label %b88

579:
  br label %b87

b87:
  %580 = phi float [ %582, %b87 ], [ %576, %579 ]
  %581 = fadd float %580, 0x3F747AE140000000
  %582 = fpround float %581 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string173, ptr nonnull %53)
  %583 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string172)
  %584 = icmp eq i16 %583, 0
  br i1 %584, label %b87, label %b88

b88:
  %585 = phi float [ %576, %b85 ], [ %582, %b87 ]
  %586 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string174)
  %587 = icmp eq i16 %586, 0
  br i1 %587, label %588, label %b91

588:
  br label %b90

b90:
  %589 = phi float [ %591, %b90 ], [ %559, %588 ]
  %590 = fsub float %589, 0x3F747AE140000000
  %591 = fpround float %590 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string175, ptr nonnull %53)
  %592 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string174)
  %593 = icmp eq i16 %592, 0
  br i1 %593, label %b90, label %b91

b91:
  %594 = phi float [ %559, %b88 ], [ %591, %b90 ]
  %595 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string176)
  %596 = icmp eq i16 %595, 0
  br i1 %596, label %597, label %b94

597:
  br label %b93

b93:
  %598 = phi float [ %600, %b93 ], [ %594, %597 ]
  %599 = fadd float %598, 0x3F747AE140000000
  %600 = fpround float %599 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string177, ptr nonnull %53)
  %601 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string176)
  %602 = icmp eq i16 %601, 0
  br i1 %602, label %b93, label %b94

b94:
  %603 = phi float [ %594, %b91 ], [ %600, %b93 ]
  %604 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string178)
  %605 = icmp eq i16 %604, 0
  br i1 %605, label %606, label %b97

606:
  br label %b96

b96:
  %607 = phi float [ %609, %b96 ], [ %557, %606 ]
  %608 = fsub float %607, 0x3F747AE140000000
  %609 = fpround float %608 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string179, ptr nonnull %53)
  %610 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string178)
  %611 = icmp eq i16 %610, 0
  br i1 %611, label %b96, label %b97

b97:
  %612 = phi float [ %557, %b94 ], [ %609, %b96 ]
  %613 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string180)
  %614 = icmp eq i16 %613, 0
  br i1 %614, label %615, label %b100

615:
  br label %b99

b99:
  %616 = phi float [ %618, %b99 ], [ %612, %615 ]
  %617 = fadd float %616, 0x3F747AE140000000
  %618 = fpround float %617 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string181, ptr nonnull %53)
  %619 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string180)
  %620 = icmp eq i16 %619, 0
  br i1 %620, label %b99, label %b100

b100:
  %621 = phi float [ %612, %b97 ], [ %618, %b99 ]
  %622 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string182)
  %623 = icmp eq i16 %622, 0
  %624 = sext i1 %623 to i16
  %625 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string183)
  %626 = icmp eq i16 %625, 0
  %627 = sext i1 %626 to i16
  %628 = or i16 %627, %624
  %629 = icmp eq i16 %628, 0
  br i1 %629, label %b103, label %630

630:
  br label %631

631:
  %632 = phi i16 [ 0, %630 ], [ %644, %643 ], [ 0, %645 ]
  %633 = mul nuw nsw i16 %632, 20
  %634 = add nuw nsw i16 %633, -24576
  store i16 %634, ptr @b$seg, !tbaa !2
  br label %635

635:
  %636 = phi i16 [ %642, %635 ], [ -320, %631 ]
  %637 = load i16, ptr @b$seg, !tbaa !2
  %638 = inttoptr i16 %637 to ptr addrspace(2)
  %639 = addrspacecast ptr addrspace(2) %638 to ptr addrspace(1)
  %640 = getelementptr i8, ptr addrspace(1) %639, i16 %636
  %641 = getelementptr i8, ptr addrspace(1) %640, i16 320
  store volatile i8 1, ptr addrspace(1) %641
  %642 = add nsw i16 %636, 1
  %exitcond1 = icmp eq i16 %642, 0
  br i1 %exitcond1, label %643, label %635

643:
  %644 = add nuw nsw i16 %632, 1
  %exitcond11 = icmp eq i16 %644, 200
  br i1 %exitcond11, label %645, label %631

645:
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 0, ptr @"FTYPE%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string184, ptr nonnull %53)
  %646 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string182)
  %647 = icmp eq i16 %646, 0
  %648 = sext i1 %647 to i16
  %649 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string183)
  %650 = icmp eq i16 %649, 0
  %651 = sext i1 %650 to i16
  %652 = or i16 %651, %648
  %653 = icmp eq i16 %652, 0
  br i1 %653, label %b103, label %631

b103:
  %654 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string185)
  %655 = icmp eq i16 %654, 0
  %656 = sext i1 %655 to i16
  %657 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string186)
  %658 = icmp eq i16 %657, 0
  %659 = sext i1 %658 to i16
  %660 = or i16 %659, %656
  %661 = icmp eq i16 %660, 0
  br i1 %661, label %b106, label %662

662:
  store i16 1, ptr @"FTYPE%", !tbaa !2
  br label %b105

b105:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string187, ptr nonnull %53)
  %663 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string185)
  %664 = icmp eq i16 %663, 0
  %665 = sext i1 %664 to i16
  %666 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string186)
  %667 = icmp eq i16 %666, 0
  %668 = sext i1 %667 to i16
  %669 = or i16 %668, %665
  %670 = icmp eq i16 %669, 0
  br i1 %670, label %b106, label %b105

b106:
  %671 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string188)
  %672 = icmp eq i16 %671, 0
  %673 = sext i1 %672 to i16
  %674 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string189)
  %675 = icmp eq i16 %674, 0
  %676 = sext i1 %675 to i16
  %677 = or i16 %676, %673
  %678 = icmp eq i16 %677, 0
  br i1 %678, label %b109, label %679

679:
  store i16 2, ptr @"FTYPE%", !tbaa !2
  br label %b108

b108:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string190, ptr nonnull %53)
  %680 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string188)
  %681 = icmp eq i16 %680, 0
  %682 = sext i1 %681 to i16
  %683 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %53, ptr @$string189)
  %684 = icmp eq i16 %683, 0
  %685 = sext i1 %684 to i16
  %686 = or i16 %685, %682
  %687 = icmp eq i16 %686, 0
  br i1 %687, label %b109, label %b108

b109:
  %688 = load float, ptr @"XP!", !tbaa !2
  %689 = fadd float %509, %688
  %690 = fpround float %689 to float
  store float %690, ptr @"XP!", !tbaa !2
  %691 = fadd float %422, %467
  %692 = fpround float %691 to float
  %693 = fadd float %421, %539
  %694 = fpround float %693 to float
  %695 = load float, ptr %2, !tbaa !2
  %696 = fadd float %603, %695
  %697 = fpround float %696 to float
  store float %697, ptr %2, !tbaa !2
  %698 = load float, ptr %1, !tbaa !2
  %699 = fadd float %585, %698
  %700 = fpround float %699 to float
  store float %700, ptr %1, !tbaa !2
  %701 = load float, ptr %0, !tbaa !2
  %702 = fadd float %621, %701
  %703 = fpround float %702 to float
  store float %703, ptr %0, !tbaa !2
  %704 = load i16, ptr @"OBJ%", !tbaa !2
  %705 = icmp eq i16 %704, 4
  br i1 %705, label %706, label %748

706:
  %707 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %708 = inttoptr i16 %707 to ptr addrspace(2)
  %709 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %710 = inttoptr i16 %709 to ptr addrspace(2)
  %711 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %712 = inttoptr i16 %711 to ptr addrspace(2)
  br label %713

713:
  %714 = phi i16 [ 0, %706 ], [ %743, %742 ]
  %715 = phi i16 [ 0, %706 ], [ %739, %742 ]
  %716 = add nsw i16 %714, -256
  %717 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %714
  %718 = add i16 %715, 63
  br label %719

719:
  %720 = phi i16 [ %715, %713 ], [ %739, %719 ]
  %721 = phi i16 [ 0, %713 ], [ %740, %719 ]
  %722 = addrspacecast ptr addrspace(2) %712 to ptr addrspace(1)
  %723 = addrspacecast ptr addrspace(2) %710 to ptr addrspace(1)
  %724 = addrspacecast ptr addrspace(2) %708 to ptr addrspace(1)
  %725 = shl i16 %720, 1
  %726 = getelementptr i8, ptr addrspace(1) %724, i16 %725
  %727 = add nsw i16 %721, -256
  store i16 %727, ptr addrspace(1) %726, !tbaa !16
  %728 = getelementptr i8, ptr addrspace(1) %723, i16 %725
  store i16 %716, ptr addrspace(1) %728, !tbaa !18
  %729 = getelementptr i8, ptr addrspace(1) %722, i16 %725
  %730 = load i16, ptr @"K%"
  %731 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %721
  %732 = getelementptr [2 x i8], ptr %731, i16 %730
  %733 = getelementptr i8, ptr %732, i16 96
  %734 = load i16, ptr %733, !tbaa !2
  %735 = getelementptr [2 x i8], ptr %717, i16 %730
  %736 = getelementptr i8, ptr %735, i16 96
  %737 = load i16, ptr %736, !tbaa !2
  %738 = add i16 %737, %734
  store i16 %738, ptr addrspace(1) %729, !tbaa !20
  %739 = add i16 %720, 1
  %740 = add nuw nsw i16 %721, 8
  %741 = icmp eq i16 %720, %718
  br i1 %741, label %742, label %719

742:
  %743 = add nuw nsw i16 %714, 8
  %744 = icmp samesign ult i16 %714, 504
  br i1 %744, label %713, label %745

745:
  %746 = add i16 %730, 1
  %747 = icmp eq i16 %746, 314
  %spec.store.select = select i1 %747, i16 0, i16 %746
  store i16 %spec.store.select, ptr @"K%"
  br label %748

748:
  call cc 1000 addrspace(1) void @ROTATE3D(ptr nonnull %2, ptr nonnull %1, ptr nonnull %0)
  %749 = load i16, ptr @"NDTS%", !tbaa !2
  %750 = add i16 %749, -1
  %751 = icmp slt i16 %750, 0
  br i1 %751, label %780, label %752

752:
  %753 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"X!", i16 2), !tbaa !2
  %754 = inttoptr i16 %753 to ptr addrspace(2)
  %755 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Y!", i16 2), !tbaa !2
  %756 = inttoptr i16 %755 to ptr addrspace(2)
  %757 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Z!", i16 2), !tbaa !2
  %758 = inttoptr i16 %757 to ptr addrspace(2)
  br label %759

759:
  %760 = phi i16 [ %779, %759 ], [ 0, %752 ]
  %761 = phi i16 [ %778, %759 ], [ %749, %752 ]
  %762 = addrspacecast ptr addrspace(2) %758 to ptr addrspace(1)
  %763 = addrspacecast ptr addrspace(2) %756 to ptr addrspace(1)
  %764 = addrspacecast ptr addrspace(2) %754 to ptr addrspace(1)
  %765 = getelementptr i8, ptr addrspace(1) %764, i16 %760
  %766 = load float, ptr addrspace(1) %765, !tbaa !54
  %767 = load float, ptr @"XP!"
  %768 = fadd float %766, %767
  %769 = fpround float %768 to float
  store float %769, ptr addrspace(1) %765, !tbaa !54
  %770 = getelementptr i8, ptr addrspace(1) %763, i16 %760
  %771 = load float, ptr addrspace(1) %770, !tbaa !58
  %772 = fadd float %771, %692
  %773 = fpround float %772 to float
  store float %773, ptr addrspace(1) %770, !tbaa !58
  %774 = getelementptr i8, ptr addrspace(1) %762, i16 %760
  %775 = load float, ptr addrspace(1) %774, !tbaa !56
  %776 = fadd float %775, %694
  %777 = fpround float %776 to float
  store float %777, ptr addrspace(1) %774, !tbaa !56
  %778 = add i16 %761, -1
  %779 = add i16 %760, 4
  %exitcond = icmp eq i16 %778, 0
  br i1 %exitcond, label %780, label %759

780:
  call cc 1000 addrspace(1) void @TRANSLATE3D()
  call cc 1000 addrspace(1) void @OUTPUT3D()
  %781 = load i16, ptr @"FILEI%", !tbaa !2
  %782 = icmp eq i16 %781, 444
  br i1 %782, label %b2, label %b118

b118:
  %783 = load i16, ptr @"XIT%", !tbaa !2
  %784 = icmp eq i16 %783, 1
  br i1 %784, label %b2, label %785

785:
  %786 = icmp eq i16 %783, 0
  br i1 %786, label %787, label %b4

787:
  %788 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %789 = icmp eq i8 %788, 1
  br i1 %789, label %b122.thread, label %b4

b122.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b2
}

define cc 1000 void @ANIMATEWAVELET(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %2 = inttoptr i16 %1 to ptr addrspace(2)
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %4 = inttoptr i16 %3 to ptr addrspace(2)
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %6 = inttoptr i16 %5 to ptr addrspace(2)
  br label %b5

b5:
  %7 = phi i16 [ 0, %b1 ], [ %34, %b11 ]
  %8 = phi i16 [ 0, %b1 ], [ %31, %b11 ]
  %9 = add nsw i16 %7, -256
  %10 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %7
  %11 = add i16 %8, 63
  br label %b10

b6:
  ret void

b10:
  %12 = phi i16 [ %8, %b5 ], [ %31, %b10 ]
  %13 = phi i16 [ 0, %b5 ], [ %32, %b10 ]
  %14 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %15 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)
  %16 = addrspacecast ptr addrspace(2) %2 to ptr addrspace(1)
  %17 = shl i16 %12, 1
  %18 = getelementptr i8, ptr addrspace(1) %16, i16 %17
  %19 = add nsw i16 %13, -256
  store i16 %19, ptr addrspace(1) %18, !tbaa !16
  %20 = getelementptr i8, ptr addrspace(1) %15, i16 %17
  store i16 %9, ptr addrspace(1) %20, !tbaa !18
  %21 = getelementptr i8, ptr addrspace(1) %14, i16 %17
  %22 = load i16, ptr %0
  %23 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %13
  %24 = getelementptr [2 x i8], ptr %23, i16 %22
  %25 = getelementptr i8, ptr %24, i16 96
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr [2 x i8], ptr %10, i16 %22
  %28 = getelementptr i8, ptr %27, i16 96
  %29 = load i16, ptr %28, !tbaa !2
  %30 = add i16 %29, %26
  store i16 %30, ptr addrspace(1) %21, !tbaa !20
  %31 = add i16 %12, 1
  %32 = add nuw nsw i16 %13, 8
  %33 = icmp eq i16 %12, %11
  br i1 %33, label %b11, label %b10

b11:
  %34 = add nuw nsw i16 %7, 8
  %35 = icmp samesign ult i16 %7, 504
  br i1 %35, label %b5, label %b6
}

define cc 1000 void @CLEARSCREEN(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree nosync norecurse willreturn {
b1:
  br label %b5

b5:
  %1 = phi i16 [ 0, %b1 ], [ %13, %b11 ]
  %2 = mul nuw nsw i16 %1, 20
  %3 = add nuw nsw i16 %2, -24576
  store i16 %3, ptr @b$seg, !tbaa !2
  br label %b10

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  ret void

b10:
  %4 = phi i16 [ %12, %b10 ], [ -320, %b5 ]
  %5 = load i16, ptr %0
  %6 = trunc i16 %5 to i8
  %7 = load i16, ptr @b$seg, !tbaa !2
  %8 = inttoptr i16 %7 to ptr addrspace(2)
  %9 = addrspacecast ptr addrspace(2) %8 to ptr addrspace(1)
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 %4
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 320
  store volatile i8 %6, ptr addrspace(1) %11
  %12 = add nsw i16 %4, 1
  %exitcond = icmp eq i16 %12, 0
  br i1 %exitcond, label %b11, label %b10

b11:
  %13 = add nuw nsw i16 %1, 1
  %exitcond1 = icmp eq i16 %13, 200
  br i1 %exitcond1, label %b6, label %b5
}

define cc 1000 void @COPPER() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = alloca [292 x i8]
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 18
  %2 = getelementptr inbounds nuw i8, ptr %0, i16 36
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 54
  %4 = getelementptr inbounds nuw i8, ptr %0, i16 72
  %5 = getelementptr inbounds nuw i8, ptr %0, i16 90
  %6 = getelementptr inbounds nuw i8, ptr %0, i16 112
  %7 = getelementptr inbounds nuw i8, ptr %0, i16 130
  %8 = getelementptr inbounds nuw i8, ptr %0, i16 148
  %9 = getelementptr inbounds nuw i8, ptr %0, i16 166
  %10 = getelementptr inbounds nuw i8, ptr %0, i16 184
  %11 = getelementptr inbounds nuw i8, ptr %0, i16 202
  %12 = getelementptr inbounds nuw i8, ptr %0, i16 204
  %13 = getelementptr inbounds nuw i8, ptr %0, i16 220
  %14 = getelementptr inbounds nuw i8, ptr %0, i16 238
  %15 = getelementptr inbounds nuw i8, ptr %0, i16 256
  %16 = getelementptr inbounds nuw i8, ptr %0, i16 274
  call void @llvm.memset.p0.i16(ptr align 2 %0, i8 0, i16 292, i1 false)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 2, i16 1, i16 32, i16 4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string191)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 4, i16 1, i16 2, i16 4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string192)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 5, i16 1, i16 2, i16 4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string193)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string194)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string195)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string196)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string197)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string198)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string199)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string200)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string201)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string202)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string203)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string204)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string205)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string206)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string207)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string208)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string209)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 32, i16 2, i16 257, ptr nonnull %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 319, i16 2, i16 257, ptr nonnull %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 32, i16 2, i16 257, ptr nonnull %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 32, i16 2, i16 257, ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32, i16 2, i16 257, ptr nonnull %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 15, i16 0, i16 15, i16 2, i16 258, ptr nonnull %5)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -640, i16 640, i16 2, i16 257, ptr nonnull %6)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -640, i16 640, i16 2, i16 257, ptr nonnull %7)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 399, i16 2, i16 257, ptr nonnull %8)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 399, i16 2, i16 257, ptr nonnull %9)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 3, i16 2, i16 257, ptr nonnull %10)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -200, i16 500, i16 2, i16 257, ptr nonnull %11)
  %17 = load i16, ptr %12, !tbaa !2
  %18 = inttoptr i16 %17 to ptr addrspace(2)
  br label %b8

b3:
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 28) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 1) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 33) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 2) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 38) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 3) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 43) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 4) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 48) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 5) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 53) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 6) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 58) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 7) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 8) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 28) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 9) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 33) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 10) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 38) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 11) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 43) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 12) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 48) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 13) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 53) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 14) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 58) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 15) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 16) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 28) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 17) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 33) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 18) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 38) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 19) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 43) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 20) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 48) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 21) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 53) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 22) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 58) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 968, i8 23) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 980, i8 19) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 981, i8 0) memory(inaccessiblemem: readwrite)
  %19 = getelementptr i8, ptr %0, i16 240
  %20 = load i16, ptr %19, !tbaa !2
  %21 = inttoptr i16 %20 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr %0, i16 258
  %24 = load i16, ptr %23, !tbaa !2
  %25 = inttoptr i16 %24 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr %0, i16 276
  %28 = load i16, ptr %27, !tbaa !2
  %29 = inttoptr i16 %28 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  store i16 0, ptr addrspace(1) %22, !tbaa !40
  store i16 8, ptr addrspace(1) %26, !tbaa !42
  store i16 16, ptr addrspace(1) %30, !tbaa !44
  %31 = getelementptr i8, ptr addrspace(1) %22, i16 2
  store i16 1, ptr addrspace(1) %31, !tbaa !40
  %32 = getelementptr i8, ptr addrspace(1) %26, i16 2
  store i16 9, ptr addrspace(1) %32, !tbaa !42
  %33 = getelementptr i8, ptr addrspace(1) %30, i16 2
  store i16 17, ptr addrspace(1) %33, !tbaa !44
  %34 = getelementptr i8, ptr addrspace(1) %22, i16 4
  store i16 2, ptr addrspace(1) %34, !tbaa !40
  %35 = getelementptr i8, ptr addrspace(1) %26, i16 4
  store i16 10, ptr addrspace(1) %35, !tbaa !42
  %36 = getelementptr i8, ptr addrspace(1) %30, i16 4
  store i16 18, ptr addrspace(1) %36, !tbaa !44
  %37 = getelementptr i8, ptr addrspace(1) %22, i16 6
  store i16 3, ptr addrspace(1) %37, !tbaa !40
  %38 = getelementptr i8, ptr addrspace(1) %26, i16 6
  store i16 11, ptr addrspace(1) %38, !tbaa !42
  %39 = getelementptr i8, ptr addrspace(1) %30, i16 6
  store i16 19, ptr addrspace(1) %39, !tbaa !44
  %40 = getelementptr i8, ptr addrspace(1) %22, i16 8
  store i16 4, ptr addrspace(1) %40, !tbaa !40
  %41 = getelementptr i8, ptr addrspace(1) %26, i16 8
  store i16 12, ptr addrspace(1) %41, !tbaa !42
  %42 = getelementptr i8, ptr addrspace(1) %30, i16 8
  store i16 20, ptr addrspace(1) %42, !tbaa !44
  %43 = getelementptr i8, ptr addrspace(1) %22, i16 10
  store i16 5, ptr addrspace(1) %43, !tbaa !40
  %44 = getelementptr i8, ptr addrspace(1) %26, i16 10
  store i16 13, ptr addrspace(1) %44, !tbaa !42
  %45 = getelementptr i8, ptr addrspace(1) %30, i16 10
  store i16 21, ptr addrspace(1) %45, !tbaa !44
  %46 = getelementptr i8, ptr addrspace(1) %22, i16 12
  store i16 6, ptr addrspace(1) %46, !tbaa !40
  %47 = getelementptr i8, ptr addrspace(1) %26, i16 12
  store i16 14, ptr addrspace(1) %47, !tbaa !42
  %48 = getelementptr i8, ptr addrspace(1) %30, i16 12
  store i16 22, ptr addrspace(1) %48, !tbaa !44
  %49 = getelementptr i8, ptr addrspace(1) %22, i16 14
  store i16 7, ptr addrspace(1) %49, !tbaa !40
  %50 = getelementptr i8, ptr addrspace(1) %26, i16 14
  store i16 15, ptr addrspace(1) %50, !tbaa !42
  %51 = getelementptr i8, ptr addrspace(1) %30, i16 14
  store i16 23, ptr addrspace(1) %51, !tbaa !44
  %52 = load i16, ptr %19, !tbaa !2
  %53 = inttoptr i16 %52 to ptr addrspace(2)
  %54 = addrspacecast ptr addrspace(2) %53 to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %54, !tbaa !40
  %55 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %56 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %55, ptr @$string214)
  %57 = icmp eq i16 %56, 0
  br i1 %57, label %58, label %b4

58:
  br label %b245

b4:
  call void @llrm.ia16.out.i8(i16 980, i8 40) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 981, i8 19) memory(inaccessiblemem: readwrite)
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 1, i16 2)
  call void @llrm.ia16.out.i8(i16 980, i8 40) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 981, i8 19) memory(inaccessiblemem: readwrite)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %6)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %7)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %14)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %15)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %16)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %10)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %9)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %13)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %5)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %8)
  ret void

b8:
  %59 = phi i16 [ %66, %b8 ], [ -66, %b1 ]
  %60 = phi i16 [ 0, %b1 ], [ %65, %b8 ]
  %61 = addrspacecast ptr addrspace(2) %18 to ptr addrspace(1)
  %62 = call i16 @llvm.smin.i16(i16 %60, i16 63)
  %63 = getelementptr i8, ptr addrspace(1) %61, i16 %59
  %64 = getelementptr i8, ptr addrspace(1) %63, i16 802
  store i16 %62, ptr addrspace(1) %64, !tbaa !22
  %65 = add nsw i16 %62, 2
  %66 = add nsw i16 %59, 2
  %exitcond38 = icmp eq i16 %66, 0
  br i1 %exitcond38, label %b9, label %b8

b9:
  %67 = getelementptr i8, ptr %0, i16 204
  %68 = load i16, ptr %67, !tbaa !2
  %69 = inttoptr i16 %68 to ptr addrspace(2)
  br label %b16

b16:
  %70 = phi i16 [ %77, %b16 ], [ -64, %b9 ]
  %71 = phi i16 [ %65, %b9 ], [ %74, %b16 ]
  %72 = addrspacecast ptr addrspace(2) %69 to ptr addrspace(1)
  %73 = add nsw i16 %71, -2
  %74 = call i16 @llvm.smax.i16(i16 %73, i16 0)
  %75 = getelementptr i8, ptr addrspace(1) %72, i16 %70
  %76 = getelementptr i8, ptr addrspace(1) %75, i16 866
  store i16 %74, ptr addrspace(1) %76, !tbaa !22
  %77 = add nsw i16 %70, 2
  %exitcond37 = icmp eq i16 %77, 0
  br i1 %exitcond37, label %b17, label %b16

b17:
  %78 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %79 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %78, ptr @$string210)
  %80 = icmp eq i16 %79, 0
  br i1 %80, label %81, label %b23

81:
  br label %b22

b22:
  %82 = phi i16 [ %83, %b34 ], [ 0, %81 ]
  %83 = add i16 %82, 1
  %84 = sitofp i16 %83 to float
  %85 = fdiv float %84, 2.500000e+01
  %86 = call float @llvm.fabs.f32(float %85)
  %87 = fcmp ogt float %86, 0x41E0000000000000
  br i1 %87, label %b24, label %b25

b23:
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call void @llrm.ia16.out.i8(i16 980, i8 19) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 981, i8 0) memory(inaccessiblemem: readwrite)
  br label %b38

b24:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b25:
  %88 = getelementptr i8, ptr %0, i16 186
  %89 = load i16, ptr %88, !tbaa !2
  %90 = inttoptr i16 %89 to ptr addrspace(2)
  %91 = addrspacecast ptr addrspace(2) %90 to ptr addrspace(1)
  %92 = getelementptr i8, ptr addrspace(1) %91, i16 2
  %93 = call float @llvm.sin.f32(float %85)
  %94 = fmul float %93, 1.200000e+02
  %95 = call i16 @llvm.lrint.i16.f32(float %94)
  store i16 %95, ptr addrspace(1) %92, !tbaa !24
  %96 = fdiv float %84, 4.500000e+01
  %97 = call float @llvm.fabs.f32(float %96)
  %98 = fcmp ogt float %97, 0x41E0000000000000
  br i1 %98, label %b26, label %b27

b26:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b27:
  %99 = inttoptr i16 %89 to ptr addrspace(2)
  %100 = addrspacecast ptr addrspace(2) %99 to ptr addrspace(1)
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 4
  %102 = call float @llvm.sin.f32(float %96)
  %103 = fmul float %102, 1.100000e+02
  %104 = call i16 @llvm.lrint.i16.f32(float %103)
  store i16 %104, ptr addrspace(1) %101, !tbaa !24
  %105 = fdiv float %84, 3.500000e+01
  %106 = call float @llvm.fabs.f32(float %105)
  %107 = fcmp ogt float %106, 0x41E0000000000000
  br i1 %107, label %b28, label %b29

b28:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b29:
  %108 = inttoptr i16 %89 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 6
  %111 = call float @llvm.sin.f32(float %105)
  %112 = fmul float %111, 1.000000e+02
  %113 = call i16 @llvm.lrint.i16.f32(float %112)
  store i16 %113, ptr addrspace(1) %110, !tbaa !24
  %114 = getelementptr i8, ptr %0, i16 204
  %115 = load i16, ptr %114, !tbaa !2
  %116 = inttoptr i16 %115 to ptr addrspace(2)
  %117 = addrspacecast ptr addrspace(2) %116 to ptr addrspace(1)
  %118 = getelementptr i8, ptr addrspace(1) %117, i16 400
  %119 = shl i16 %113, 1
  %120 = getelementptr i8, ptr addrspace(1) %118, i16 %119
  %121 = shl i16 %104, 1
  %122 = getelementptr i8, ptr addrspace(1) %118, i16 %121
  %123 = shl i16 %95, 1
  %124 = getelementptr i8, ptr addrspace(1) %118, i16 %123
  br label %b33

b33:
  %125 = phi i16 [ %138, %b33 ], [ -762, %b29 ]
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %126 = getelementptr i8, ptr addrspace(1) %124, i16 %125
  %127 = getelementptr i8, ptr addrspace(1) %126, i16 762
  %128 = load i16, ptr addrspace(1) %127, !tbaa !22
  %129 = trunc i16 %128 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %129) memory(inaccessiblemem: readwrite)
  %130 = getelementptr i8, ptr addrspace(1) %122, i16 %125
  %131 = getelementptr i8, ptr addrspace(1) %130, i16 762
  %132 = load i16, ptr addrspace(1) %131, !tbaa !22
  %133 = trunc i16 %132 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %133) memory(inaccessiblemem: readwrite)
  %134 = getelementptr i8, ptr addrspace(1) %120, i16 %125
  %135 = getelementptr i8, ptr addrspace(1) %134, i16 762
  %136 = load i16, ptr addrspace(1) %135, !tbaa !22
  %137 = trunc i16 %136 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %137) memory(inaccessiblemem: readwrite)
  %138 = add nsw i16 %125, 2
  %exitcond36 = icmp eq i16 %138, 0
  br i1 %exitcond36, label %b34, label %b33

b34:
  %139 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %140 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %139, ptr @$string210)
  %141 = icmp eq i16 %140, 0
  br i1 %141, label %b22, label %b23

b38:
  %142 = phi i8 [ %145, %b38 ], [ 1, %b23 ]
  %143 = phi i16 [ %144, %b38 ], [ 0, %b23 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %142) memory(inaccessiblemem: readwrite)
  %tmp10 = trunc i16 %143 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp10) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %144 = add nuw nsw i16 %143, 4
  %145 = add nuw nsw i8 %142, 1
  %exitcond35 = icmp eq i16 %144, 64
  br i1 %exitcond35, label %146, label %b38

146:
  br label %b43

b43:
  %147 = phi i8 [ 17, %146 ], [ %150, %b43 ]
  %148 = phi i16 [ 0, %146 ], [ %149, %b43 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %147) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %tmp9 = trunc i16 %148 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp9) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %149 = add nuw nsw i16 %148, 4
  %150 = add nuw nsw i8 %147, 1
  %exitcond34 = icmp eq i16 %149, 64
  br i1 %exitcond34, label %151, label %b43

151:
  br label %b48

b48:
  %152 = phi i8 [ 33, %151 ], [ %155, %b48 ]
  %153 = phi i16 [ 0, %151 ], [ %154, %b48 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %152) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %tmp8 = trunc i16 %153 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp8) memory(inaccessiblemem: readwrite)
  %154 = add nuw nsw i16 %153, 4
  %155 = add nuw nsw i8 %152, 1
  %exitcond33 = icmp eq i16 %154, 64
  br i1 %exitcond33, label %156, label %b48

156:
  br label %b53

b53:
  %157 = phi i8 [ 49, %156 ], [ %160, %b53 ]
  %158 = phi i16 [ 0, %156 ], [ %159, %b53 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %157) memory(inaccessiblemem: readwrite)
  %tmp7 = trunc i16 %158 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp7) memory(inaccessiblemem: readwrite)
  %tmp6 = trunc i16 %158 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp6) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %159 = add nuw nsw i16 %158, 4
  %160 = add nuw nsw i8 %157, 1
  %exitcond32 = icmp eq i16 %159, 64
  br i1 %exitcond32, label %161, label %b53

161:
  br label %b58

b58:
  %162 = phi i8 [ 65, %161 ], [ %165, %b58 ]
  %163 = phi i16 [ 0, %161 ], [ %164, %b58 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %162) memory(inaccessiblemem: readwrite)
  %tmp5 = trunc i16 %163 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp5) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %tmp4 = trunc i16 %163 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp4) memory(inaccessiblemem: readwrite)
  %164 = add nuw nsw i16 %163, 4
  %165 = add nuw nsw i8 %162, 1
  %exitcond31 = icmp eq i16 %164, 64
  br i1 %exitcond31, label %166, label %b58

166:
  br label %b63

b63:
  %167 = phi i8 [ 81, %166 ], [ %170, %b63 ]
  %168 = phi i16 [ 0, %166 ], [ %169, %b63 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %167) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %tmp3 = trunc i16 %168 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp3) memory(inaccessiblemem: readwrite)
  %tmp2 = trunc i16 %168 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp2) memory(inaccessiblemem: readwrite)
  %169 = add nuw nsw i16 %168, 4
  %170 = add nuw nsw i8 %167, 1
  %exitcond30 = icmp eq i16 %169, 64
  br i1 %exitcond30, label %171, label %b63

171:
  br label %b68

b68:
  %172 = phi i8 [ 97, %171 ], [ %177, %b68 ]
  %173 = phi i8 [ 0, %171 ], [ %176, %b68 ]
  %174 = phi i16 [ 0, %171 ], [ %175, %b68 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %172) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 %173) memory(inaccessiblemem: readwrite)
  %tmp1 = trunc i16 %174 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp1) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %175 = add nuw nsw i16 %174, 2
  %176 = add nuw nsw i8 %173, 4
  %177 = add nuw nsw i8 %172, 1
  %exitcond29 = icmp eq i16 %175, 32
  br i1 %exitcond29, label %178, label %b68

178:
  br label %b73

b73:
  %179 = phi i8 [ 113, %178 ], [ %184, %b73 ]
  %180 = phi i8 [ 0, %178 ], [ %183, %b73 ]
  %181 = phi i16 [ 0, %178 ], [ %182, %b73 ]
  call void @llrm.ia16.out.i8(i16 968, i8 %179) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %tmp = trunc i16 %181 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %tmp) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 %180) memory(inaccessiblemem: readwrite)
  %182 = add nuw nsw i16 %181, 2
  %183 = add nuw nsw i8 %180, 4
  %184 = add nuw i8 %179, 1
  %exitcond28 = icmp eq i16 %182, 32
  br i1 %exitcond28, label %b74, label %b73

b74:
  %185 = getelementptr i8, ptr %0, i16 2
  %186 = load i16, ptr %185, !tbaa !2
  %187 = inttoptr i16 %186 to ptr addrspace(2)
  %188 = addrspacecast ptr addrspace(2) %187 to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %188, !tbaa !26
  %189 = getelementptr i8, ptr addrspace(1) %188, i16 2
  store i16 2, ptr addrspace(1) %189, !tbaa !26
  %190 = getelementptr i8, ptr addrspace(1) %188, i16 4
  store i16 3, ptr addrspace(1) %190, !tbaa !26
  %191 = getelementptr i8, ptr addrspace(1) %188, i16 6
  store i16 4, ptr addrspace(1) %191, !tbaa !26
  %192 = getelementptr i8, ptr addrspace(1) %188, i16 8
  store i16 5, ptr addrspace(1) %192, !tbaa !26
  %193 = getelementptr i8, ptr addrspace(1) %188, i16 10
  store i16 6, ptr addrspace(1) %193, !tbaa !26
  %194 = getelementptr i8, ptr addrspace(1) %188, i16 12
  store i16 7, ptr addrspace(1) %194, !tbaa !26
  %195 = getelementptr i8, ptr addrspace(1) %188, i16 14
  store i16 8, ptr addrspace(1) %195, !tbaa !26
  %196 = getelementptr i8, ptr addrspace(1) %188, i16 16
  store i16 9, ptr addrspace(1) %196, !tbaa !26
  %197 = getelementptr i8, ptr addrspace(1) %188, i16 18
  store i16 10, ptr addrspace(1) %197, !tbaa !26
  %198 = getelementptr i8, ptr addrspace(1) %188, i16 20
  store i16 11, ptr addrspace(1) %198, !tbaa !26
  %199 = getelementptr i8, ptr addrspace(1) %188, i16 22
  store i16 12, ptr addrspace(1) %199, !tbaa !26
  %200 = getelementptr i8, ptr addrspace(1) %188, i16 24
  store i16 13, ptr addrspace(1) %200, !tbaa !26
  %201 = getelementptr i8, ptr addrspace(1) %188, i16 26
  store i16 14, ptr addrspace(1) %201, !tbaa !26
  %202 = getelementptr i8, ptr addrspace(1) %188, i16 28
  store i16 15, ptr addrspace(1) %202, !tbaa !26
  %203 = getelementptr i8, ptr addrspace(1) %188, i16 30
  store i16 16, ptr addrspace(1) %203, !tbaa !26
  %204 = load i16, ptr %185, !tbaa !2
  %205 = inttoptr i16 %204 to ptr addrspace(2)
  %206 = addrspacecast ptr addrspace(2) %205 to ptr addrspace(1)
  %207 = getelementptr i8, ptr addrspace(1) %206, i16 32
  store i16 16, ptr addrspace(1) %207, !tbaa !26
  %208 = getelementptr i8, ptr addrspace(1) %206, i16 34
  store i16 15, ptr addrspace(1) %208, !tbaa !26
  %209 = getelementptr i8, ptr addrspace(1) %206, i16 36
  store i16 14, ptr addrspace(1) %209, !tbaa !26
  %210 = getelementptr i8, ptr addrspace(1) %206, i16 38
  store i16 13, ptr addrspace(1) %210, !tbaa !26
  %211 = getelementptr i8, ptr addrspace(1) %206, i16 40
  store i16 12, ptr addrspace(1) %211, !tbaa !26
  %212 = getelementptr i8, ptr addrspace(1) %206, i16 42
  store i16 11, ptr addrspace(1) %212, !tbaa !26
  %213 = getelementptr i8, ptr addrspace(1) %206, i16 44
  store i16 10, ptr addrspace(1) %213, !tbaa !26
  %214 = getelementptr i8, ptr addrspace(1) %206, i16 46
  store i16 9, ptr addrspace(1) %214, !tbaa !26
  %215 = getelementptr i8, ptr addrspace(1) %206, i16 48
  store i16 8, ptr addrspace(1) %215, !tbaa !26
  %216 = getelementptr i8, ptr addrspace(1) %206, i16 50
  store i16 7, ptr addrspace(1) %216, !tbaa !26
  %217 = getelementptr i8, ptr addrspace(1) %206, i16 52
  store i16 6, ptr addrspace(1) %217, !tbaa !26
  %218 = getelementptr i8, ptr addrspace(1) %206, i16 54
  store i16 5, ptr addrspace(1) %218, !tbaa !26
  %219 = getelementptr i8, ptr addrspace(1) %206, i16 56
  store i16 4, ptr addrspace(1) %219, !tbaa !26
  %220 = getelementptr i8, ptr addrspace(1) %206, i16 58
  store i16 3, ptr addrspace(1) %220, !tbaa !26
  %221 = getelementptr i8, ptr addrspace(1) %206, i16 60
  store i16 2, ptr addrspace(1) %221, !tbaa !26
  %222 = getelementptr i8, ptr addrspace(1) %206, i16 62
  store i16 1, ptr addrspace(1) %222, !tbaa !26
  br label %b88

b88:
  %223 = phi i16 [ %255, %b88 ], [ -64, %b74 ]
  %224 = getelementptr i8, ptr %0, i16 38
  %225 = load i16, ptr %224, !tbaa !2
  %226 = inttoptr i16 %225 to ptr addrspace(2)
  %227 = addrspacecast ptr addrspace(2) %226 to ptr addrspace(1)
  %228 = getelementptr i8, ptr addrspace(1) %227, i16 %223
  %229 = getelementptr i8, ptr addrspace(1) %228, i16 64
  %230 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %231 = load float, ptr %230
  %232 = fmul float %231, 6.300000e+01
  %233 = fadd float %232, 3.200000e+01
  %234 = call float @llvm.rint.f32(float %233)
  %235 = fcmp olt float %233, %234
  %236 = sext i1 %235 to i16
  %237 = sitofp i16 %236 to float
  %238 = fadd float %234, %237
  %239 = call i16 @llvm.lrint.i16.f32(float %238)
  store i16 %239, ptr addrspace(1) %229, !tbaa !28
  %240 = getelementptr i8, ptr %0, i16 56
  %241 = load i16, ptr %240, !tbaa !2
  %242 = inttoptr i16 %241 to ptr addrspace(2)
  %243 = addrspacecast ptr addrspace(2) %242 to ptr addrspace(1)
  %244 = getelementptr i8, ptr addrspace(1) %243, i16 %223
  %245 = getelementptr i8, ptr addrspace(1) %244, i16 64
  %246 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %247 = load float, ptr %246
  %248 = fmul float %247, 1.270000e+02
  %249 = call float @llvm.rint.f32(float %248)
  %250 = fcmp olt float %248, %249
  %251 = sext i1 %250 to i16
  %252 = sitofp i16 %251 to float
  %253 = fadd float %249, %252
  %254 = call i16 @llvm.lrint.i16.f32(float %253)
  store i16 %254, ptr addrspace(1) %245, !tbaa !30
  %255 = add nsw i16 %223, 2
  %exitcond25 = icmp eq i16 %255, 0
  br i1 %exitcond25, label %b89, label %b88

b89:
  store i16 -24576, ptr @b$seg, !tbaa !2
  %256 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %257 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %256, ptr @$string211)
  %258 = icmp eq i16 %257, 0
  br i1 %258, label %259, label %b92

259:
  br label %b91

b91:
  %260 = phi i16 [ %261, %b125 ], [ 0, %259 ]
  %261 = add i16 %260, 1
  %262 = getelementptr i8, ptr %0, i16 38
  %263 = load i16, ptr %262, !tbaa !2
  %264 = sitofp i16 %261 to float
  %265 = getelementptr i8, ptr %0, i16 74
  %266 = load i16, ptr %265, !tbaa !2
  %267 = getelementptr i8, ptr %0, i16 56
  %268 = load i16, ptr %267, !tbaa !2
  br label %b96

b92:
  %269 = phi i16 [ 0, %b89 ], [ %261, %b125 ]
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %270 = getelementptr i8, ptr %0, i16 92
  %271 = load i16, ptr %270, !tbaa !2
  %272 = inttoptr i16 %271 to ptr addrspace(2)
  %273 = addrspacecast ptr addrspace(2) %272 to ptr addrspace(1)
  br label %b134

b96:
  %274 = phi i16 [ %310, %b99 ], [ -16, %b91 ]
  %275 = inttoptr i16 %263 to ptr addrspace(2)
  %276 = addrspacecast ptr addrspace(2) %275 to ptr addrspace(1)
  %277 = getelementptr i8, ptr addrspace(1) %276, i16 %274
  %278 = getelementptr i8, ptr addrspace(1) %277, i16 16
  %279 = load i16, ptr addrspace(1) %278, !tbaa !28
  %280 = sitofp i16 %279 to float
  %281 = fdiv float %264, %280
  %282 = call float @llvm.fabs.f32(float %281)
  %283 = fcmp ogt float %282, 0x41E0000000000000
  br i1 %283, label %b98, label %b99

b97:
  %284 = getelementptr i8, ptr %0, i16 74
  %285 = load i16, ptr %284, !tbaa !2
  %286 = inttoptr i16 %285 to ptr addrspace(2)
  %287 = getelementptr i8, ptr %0, i16 20
  %288 = load i16, ptr %287, !tbaa !2
  %289 = getelementptr i8, ptr %0, i16 2
  %290 = load i16, ptr %289, !tbaa !2
  br label %b108

b98:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b99:
  %291 = inttoptr i16 %268 to ptr addrspace(2)
  %292 = inttoptr i16 %266 to ptr addrspace(2)
  %293 = addrspacecast ptr addrspace(2) %291 to ptr addrspace(1)
  %294 = addrspacecast ptr addrspace(2) %292 to ptr addrspace(1)
  %295 = getelementptr i8, ptr addrspace(1) %294, i16 %274
  %296 = getelementptr i8, ptr addrspace(1) %295, i16 18
  %297 = call float @llvm.sin.f32(float %281)
  %298 = getelementptr i8, ptr addrspace(1) %293, i16 %274
  %299 = getelementptr i8, ptr addrspace(1) %298, i16 16
  %300 = load i16, ptr addrspace(1) %299, !tbaa !30
  %301 = sitofp i16 %300 to float
  %302 = fmul float %297, %301
  %303 = fadd float %302, %301
  %304 = shl i16 %300, 1
  %305 = sub i16 280, %304
  %306 = ashr exact i16 %305, 1
  %307 = sitofp i16 %306 to float
  %308 = fadd float %303, %307
  %309 = call i16 @llvm.lrint.i16.f32(float %308)
  store i16 %309, ptr addrspace(1) %296, !tbaa !32
  %310 = add nsw i16 %274, 2
  %exitcond24 = icmp eq i16 %310, 0
  br i1 %exitcond24, label %b97, label %b96

b108:
  %311 = phi i16 [ %328, %b108 ], [ -32, %b97 ]
  %312 = phi i16 [ %327, %b108 ], [ -64, %b97 ]
  %313 = inttoptr i16 %290 to ptr addrspace(2)
  %314 = inttoptr i16 %288 to ptr addrspace(2)
  %315 = addrspacecast ptr addrspace(2) %286 to ptr addrspace(1)
  %316 = addrspacecast ptr addrspace(2) %313 to ptr addrspace(1)
  %317 = addrspacecast ptr addrspace(2) %314 to ptr addrspace(1)
  %318 = getelementptr i8, ptr addrspace(1) %315, i16 2
  %319 = load i16, ptr addrspace(1) %318, !tbaa !32
  %320 = add i16 %311, %319
  %321 = add i16 %320, 33
  %322 = shl i16 %321, 1
  %323 = getelementptr i8, ptr addrspace(1) %317, i16 %322
  %324 = getelementptr i8, ptr addrspace(1) %316, i16 %312
  %325 = getelementptr i8, ptr addrspace(1) %324, i16 64
  %326 = load i16, ptr addrspace(1) %325, !tbaa !26
  store i16 %326, ptr addrspace(1) %323, !tbaa !34
  %327 = add nsw i16 %312, 2
  %328 = add nsw i16 %311, 1
  %exitcond22 = icmp eq i16 %327, 0
  br i1 %exitcond22, label %b109, label %b108

b109:
  %329 = inttoptr i16 %285 to ptr addrspace(2)
  br label %330

330:
  %331 = phi i16 [ %349, %330 ], [ -32, %b109 ]
  %332 = phi i16 [ %348, %330 ], [ -64, %b109 ]
  %333 = addrspacecast ptr addrspace(2) %329 to ptr addrspace(1)
  %334 = inttoptr i16 %290 to ptr addrspace(2)
  %335 = inttoptr i16 %288 to ptr addrspace(2)
  %336 = addrspacecast ptr addrspace(2) %334 to ptr addrspace(1)
  %337 = addrspacecast ptr addrspace(2) %335 to ptr addrspace(1)
  %338 = getelementptr i8, ptr addrspace(1) %333, i16 4
  %339 = load i16, ptr addrspace(1) %338, !tbaa !32
  %340 = add i16 %331, %339
  %341 = add i16 %340, 33
  %342 = shl i16 %341, 1
  %343 = getelementptr i8, ptr addrspace(1) %337, i16 %342
  %344 = getelementptr i8, ptr addrspace(1) %336, i16 %332
  %345 = getelementptr i8, ptr addrspace(1) %344, i16 64
  %346 = load i16, ptr addrspace(1) %345, !tbaa !26
  %347 = add i16 %346, 16
  store i16 %347, ptr addrspace(1) %343, !tbaa !34
  %348 = add nsw i16 %332, 2
  %349 = add nsw i16 %331, 1
  %350 = icmp eq i16 %348, 0
  br i1 %350, label %351, label %330

351:
  %352 = inttoptr i16 %285 to ptr addrspace(2)
  br label %353

353:
  %354 = phi i16 [ %372, %353 ], [ -32, %351 ]
  %355 = phi i16 [ %371, %353 ], [ -64, %351 ]
  %356 = addrspacecast ptr addrspace(2) %352 to ptr addrspace(1)
  %357 = inttoptr i16 %290 to ptr addrspace(2)
  %358 = inttoptr i16 %288 to ptr addrspace(2)
  %359 = addrspacecast ptr addrspace(2) %357 to ptr addrspace(1)
  %360 = addrspacecast ptr addrspace(2) %358 to ptr addrspace(1)
  %361 = getelementptr i8, ptr addrspace(1) %356, i16 6
  %362 = load i16, ptr addrspace(1) %361, !tbaa !32
  %363 = add i16 %354, %362
  %364 = add i16 %363, 33
  %365 = shl i16 %364, 1
  %366 = getelementptr i8, ptr addrspace(1) %360, i16 %365
  %367 = getelementptr i8, ptr addrspace(1) %359, i16 %355
  %368 = getelementptr i8, ptr addrspace(1) %367, i16 64
  %369 = load i16, ptr addrspace(1) %368, !tbaa !26
  %370 = add i16 %369, 32
  store i16 %370, ptr addrspace(1) %366, !tbaa !34
  %371 = add nsw i16 %355, 2
  %372 = add nsw i16 %354, 1
  %373 = icmp eq i16 %371, 0
  br i1 %373, label %374, label %353

374:
  %375 = inttoptr i16 %285 to ptr addrspace(2)
  br label %376

376:
  %377 = phi i16 [ %395, %376 ], [ -32, %374 ]
  %378 = phi i16 [ %394, %376 ], [ -64, %374 ]
  %379 = addrspacecast ptr addrspace(2) %375 to ptr addrspace(1)
  %380 = inttoptr i16 %290 to ptr addrspace(2)
  %381 = inttoptr i16 %288 to ptr addrspace(2)
  %382 = addrspacecast ptr addrspace(2) %380 to ptr addrspace(1)
  %383 = addrspacecast ptr addrspace(2) %381 to ptr addrspace(1)
  %384 = getelementptr i8, ptr addrspace(1) %379, i16 8
  %385 = load i16, ptr addrspace(1) %384, !tbaa !32
  %386 = add i16 %377, %385
  %387 = add i16 %386, 33
  %388 = shl i16 %387, 1
  %389 = getelementptr i8, ptr addrspace(1) %383, i16 %388
  %390 = getelementptr i8, ptr addrspace(1) %382, i16 %378
  %391 = getelementptr i8, ptr addrspace(1) %390, i16 64
  %392 = load i16, ptr addrspace(1) %391, !tbaa !26
  %393 = add i16 %392, 48
  store i16 %393, ptr addrspace(1) %389, !tbaa !34
  %394 = add nsw i16 %378, 2
  %395 = add nsw i16 %377, 1
  %396 = icmp eq i16 %394, 0
  br i1 %396, label %397, label %376

397:
  %398 = inttoptr i16 %285 to ptr addrspace(2)
  br label %399

399:
  %400 = phi i16 [ %418, %399 ], [ -32, %397 ]
  %401 = phi i16 [ %417, %399 ], [ -64, %397 ]
  %402 = addrspacecast ptr addrspace(2) %398 to ptr addrspace(1)
  %403 = inttoptr i16 %290 to ptr addrspace(2)
  %404 = inttoptr i16 %288 to ptr addrspace(2)
  %405 = addrspacecast ptr addrspace(2) %403 to ptr addrspace(1)
  %406 = addrspacecast ptr addrspace(2) %404 to ptr addrspace(1)
  %407 = getelementptr i8, ptr addrspace(1) %402, i16 10
  %408 = load i16, ptr addrspace(1) %407, !tbaa !32
  %409 = add i16 %400, %408
  %410 = add i16 %409, 33
  %411 = shl i16 %410, 1
  %412 = getelementptr i8, ptr addrspace(1) %406, i16 %411
  %413 = getelementptr i8, ptr addrspace(1) %405, i16 %401
  %414 = getelementptr i8, ptr addrspace(1) %413, i16 64
  %415 = load i16, ptr addrspace(1) %414, !tbaa !26
  %416 = add i16 %415, 64
  store i16 %416, ptr addrspace(1) %412, !tbaa !34
  %417 = add nsw i16 %401, 2
  %418 = add nsw i16 %400, 1
  %419 = icmp eq i16 %417, 0
  br i1 %419, label %420, label %399

420:
  %421 = inttoptr i16 %285 to ptr addrspace(2)
  br label %422

422:
  %423 = phi i16 [ %441, %422 ], [ -32, %420 ]
  %424 = phi i16 [ %440, %422 ], [ -64, %420 ]
  %425 = addrspacecast ptr addrspace(2) %421 to ptr addrspace(1)
  %426 = inttoptr i16 %290 to ptr addrspace(2)
  %427 = inttoptr i16 %288 to ptr addrspace(2)
  %428 = addrspacecast ptr addrspace(2) %426 to ptr addrspace(1)
  %429 = addrspacecast ptr addrspace(2) %427 to ptr addrspace(1)
  %430 = getelementptr i8, ptr addrspace(1) %425, i16 12
  %431 = load i16, ptr addrspace(1) %430, !tbaa !32
  %432 = add i16 %423, %431
  %433 = add i16 %432, 33
  %434 = shl i16 %433, 1
  %435 = getelementptr i8, ptr addrspace(1) %429, i16 %434
  %436 = getelementptr i8, ptr addrspace(1) %428, i16 %424
  %437 = getelementptr i8, ptr addrspace(1) %436, i16 64
  %438 = load i16, ptr addrspace(1) %437, !tbaa !26
  %439 = add i16 %438, 80
  store i16 %439, ptr addrspace(1) %435, !tbaa !34
  %440 = add nsw i16 %424, 2
  %441 = add nsw i16 %423, 1
  %442 = icmp eq i16 %440, 0
  br i1 %442, label %443, label %422

443:
  %444 = inttoptr i16 %285 to ptr addrspace(2)
  br label %445

445:
  %446 = phi i16 [ %464, %445 ], [ -32, %443 ]
  %447 = phi i16 [ %463, %445 ], [ -64, %443 ]
  %448 = addrspacecast ptr addrspace(2) %444 to ptr addrspace(1)
  %449 = inttoptr i16 %290 to ptr addrspace(2)
  %450 = inttoptr i16 %288 to ptr addrspace(2)
  %451 = addrspacecast ptr addrspace(2) %449 to ptr addrspace(1)
  %452 = addrspacecast ptr addrspace(2) %450 to ptr addrspace(1)
  %453 = getelementptr i8, ptr addrspace(1) %448, i16 14
  %454 = load i16, ptr addrspace(1) %453, !tbaa !32
  %455 = add i16 %446, %454
  %456 = add i16 %455, 33
  %457 = shl i16 %456, 1
  %458 = getelementptr i8, ptr addrspace(1) %452, i16 %457
  %459 = getelementptr i8, ptr addrspace(1) %451, i16 %447
  %460 = getelementptr i8, ptr addrspace(1) %459, i16 64
  %461 = load i16, ptr addrspace(1) %460, !tbaa !26
  %462 = add i16 %461, 96
  store i16 %462, ptr addrspace(1) %458, !tbaa !34
  %463 = add nsw i16 %447, 2
  %464 = add nsw i16 %446, 1
  %465 = icmp eq i16 %463, 0
  br i1 %465, label %466, label %445

466:
  %467 = inttoptr i16 %285 to ptr addrspace(2)
  br label %468

468:
  %469 = phi i16 [ %487, %468 ], [ -32, %466 ]
  %470 = phi i16 [ %486, %468 ], [ -64, %466 ]
  %471 = addrspacecast ptr addrspace(2) %467 to ptr addrspace(1)
  %472 = inttoptr i16 %290 to ptr addrspace(2)
  %473 = inttoptr i16 %288 to ptr addrspace(2)
  %474 = addrspacecast ptr addrspace(2) %472 to ptr addrspace(1)
  %475 = addrspacecast ptr addrspace(2) %473 to ptr addrspace(1)
  %476 = getelementptr i8, ptr addrspace(1) %471, i16 16
  %477 = load i16, ptr addrspace(1) %476, !tbaa !32
  %478 = add i16 %469, %477
  %479 = add i16 %478, 33
  %480 = shl i16 %479, 1
  %481 = getelementptr i8, ptr addrspace(1) %475, i16 %480
  %482 = getelementptr i8, ptr addrspace(1) %474, i16 %470
  %483 = getelementptr i8, ptr addrspace(1) %482, i16 64
  %484 = load i16, ptr addrspace(1) %483, !tbaa !26
  %485 = add i16 %484, 112
  store i16 %485, ptr addrspace(1) %481, !tbaa !34
  %486 = add nsw i16 %470, 2
  %487 = add nsw i16 %469, 1
  %488 = icmp eq i16 %486, 0
  br i1 %488, label %489, label %468

489:
  %490 = getelementptr i8, ptr %0, i16 20
  %491 = load i16, ptr %490, !tbaa !2
  %492 = load i16, ptr @b$seg, !tbaa !2
  %493 = inttoptr i16 %492 to ptr addrspace(2)
  %494 = addrspacecast ptr addrspace(2) %493 to ptr addrspace(1)
  br label %b113

b113:
  %495 = phi ptr addrspace(1) [ %507, %b113 ], [ %494, %489 ]
  %496 = phi i16 [ %506, %b113 ], [ -640, %489 ]
  %497 = inttoptr i16 %491 to ptr addrspace(2)
  %498 = addrspacecast ptr addrspace(2) %497 to ptr addrspace(1)
  %499 = getelementptr i8, ptr addrspace(1) %498, i16 %496
  %500 = getelementptr i8, ptr addrspace(1) %499, i16 640
  %501 = load i16, ptr addrspace(1) %500, !tbaa !34
  %502 = trunc i16 %501 to i8
  store volatile i8 %502, ptr addrspace(1) %495, !tbaa !4
  %503 = load i16, ptr addrspace(1) %500, !tbaa !34
  %504 = trunc i16 %503 to i8
  %505 = getelementptr i8, ptr addrspace(1) %495, i16 80
  store volatile i8 %504, ptr addrspace(1) %505, !tbaa !4
  %506 = add nsw i16 %496, 2
  %507 = getelementptr i8, ptr addrspace(1) %495, i16 1
  %exitcond21 = icmp eq i16 %506, 0
  br i1 %exitcond21, label %b114, label %b113

b114:
  %508 = fdiv float %264, 2.500000e+01
  %509 = call float @llvm.fabs.f32(float %508)
  %510 = fcmp ogt float %509, 0x41E0000000000000
  br i1 %510, label %b115, label %b116

b115:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b116:
  %511 = getelementptr i8, ptr %0, i16 186
  %512 = load i16, ptr %511, !tbaa !2
  %513 = inttoptr i16 %512 to ptr addrspace(2)
  %514 = addrspacecast ptr addrspace(2) %513 to ptr addrspace(1)
  %515 = getelementptr i8, ptr addrspace(1) %514, i16 2
  %516 = call float @llvm.sin.f32(float %508)
  %517 = fmul float %516, 1.200000e+02
  %518 = call i16 @llvm.lrint.i16.f32(float %517)
  store i16 %518, ptr addrspace(1) %515, !tbaa !24
  %519 = fdiv float %264, 4.500000e+01
  %520 = call float @llvm.fabs.f32(float %519)
  %521 = fcmp ogt float %520, 0x41E0000000000000
  br i1 %521, label %b117, label %b118

b117:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b118:
  %522 = inttoptr i16 %512 to ptr addrspace(2)
  %523 = addrspacecast ptr addrspace(2) %522 to ptr addrspace(1)
  %524 = getelementptr i8, ptr addrspace(1) %523, i16 4
  %525 = call float @llvm.sin.f32(float %519)
  %526 = fmul float %525, 1.100000e+02
  %527 = call i16 @llvm.lrint.i16.f32(float %526)
  store i16 %527, ptr addrspace(1) %524, !tbaa !24
  %528 = fdiv float %264, 3.500000e+01
  %529 = call float @llvm.fabs.f32(float %528)
  %530 = fcmp ogt float %529, 0x41E0000000000000
  br i1 %530, label %b119, label %b120

b119:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b120:
  %531 = inttoptr i16 %512 to ptr addrspace(2)
  %532 = addrspacecast ptr addrspace(2) %531 to ptr addrspace(1)
  %533 = getelementptr i8, ptr addrspace(1) %532, i16 6
  %534 = call float @llvm.sin.f32(float %528)
  %535 = fmul float %534, 1.000000e+02
  %536 = call i16 @llvm.lrint.i16.f32(float %535)
  store i16 %536, ptr addrspace(1) %533, !tbaa !24
  %537 = getelementptr i8, ptr %0, i16 204
  %538 = load i16, ptr %537, !tbaa !2
  %539 = inttoptr i16 %538 to ptr addrspace(2)
  %540 = addrspacecast ptr addrspace(2) %539 to ptr addrspace(1)
  %541 = getelementptr i8, ptr addrspace(1) %540, i16 400
  %542 = shl i16 %536, 1
  %543 = getelementptr i8, ptr addrspace(1) %541, i16 %542
  %544 = shl i16 %527, 1
  %545 = getelementptr i8, ptr addrspace(1) %541, i16 %544
  %546 = shl i16 %518, 1
  %547 = getelementptr i8, ptr addrspace(1) %541, i16 %546
  br label %b124

b124:
  %548 = phi i16 [ %561, %b124 ], [ -762, %b120 ]
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %549 = getelementptr i8, ptr addrspace(1) %547, i16 %548
  %550 = getelementptr i8, ptr addrspace(1) %549, i16 762
  %551 = load i16, ptr addrspace(1) %550, !tbaa !22
  %552 = trunc i16 %551 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %552) memory(inaccessiblemem: readwrite)
  %553 = getelementptr i8, ptr addrspace(1) %545, i16 %548
  %554 = getelementptr i8, ptr addrspace(1) %553, i16 762
  %555 = load i16, ptr addrspace(1) %554, !tbaa !22
  %556 = trunc i16 %555 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %556) memory(inaccessiblemem: readwrite)
  %557 = getelementptr i8, ptr addrspace(1) %543, i16 %548
  %558 = getelementptr i8, ptr addrspace(1) %557, i16 762
  %559 = load i16, ptr addrspace(1) %558, !tbaa !22
  %560 = trunc i16 %559 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %560) memory(inaccessiblemem: readwrite)
  %561 = add nsw i16 %548, 2
  %exitcond20 = icmp eq i16 %561, 0
  br i1 %exitcond20, label %b125, label %b124

b125:
  %562 = getelementptr i8, ptr %0, i16 20
  %563 = load i16, ptr %562, !tbaa !2
  %564 = inttoptr i16 %563 to ptr addrspace(2)
  %565 = addrspacecast ptr addrspace(2) %564 to ptr addrspace(1)
  call void @llvm.memset.p1.i16(ptr addrspace(1) align 2 %565, i8 0, i16 640, i1 false)
  %566 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %567 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %566, ptr @$string211)
  %568 = icmp eq i16 %567, 0
  br i1 %568, label %b91, label %b92

b134:
  %569 = phi ptr addrspace(1) [ %584, %b140 ], [ %273, %b92 ]
  %570 = phi i16 [ 0, %b92 ], [ %580, %b140 ]
  %571 = phi i16 [ 0, %b92 ], [ %583, %b140 ]
  %572 = trunc nuw nsw i16 %571 to i8
  %573 = shl nuw nsw i8 %572, 1
  %574 = add nuw nsw i8 %573, 33
  br label %b139

b135:
  call void @llrm.ia16.out.i8(i16 980, i8 19) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 981, i8 0) memory(inaccessiblemem: readwrite)
  br label %b144

b139:
  %575 = phi i8 [ %582, %b139 ], [ 33, %b134 ]
  %576 = phi i16 [ %581, %b139 ], [ -512, %b134 ]
  %577 = phi i16 [ %570, %b134 ], [ %580, %b139 ]
  call void @llrm.ia16.out.i8(i16 969, i8 %574) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 %575) memory(inaccessiblemem: readwrite)
  %578 = getelementptr i8, ptr addrspace(1) %569, i16 %576
  %579 = getelementptr i8, ptr addrspace(1) %578, i16 512
  store i16 %577, ptr addrspace(1) %579, !tbaa !36
  %580 = add i16 %577, 1
  %581 = add nsw i16 %576, 32
  %582 = add nuw nsw i8 %575, 2
  %exitcond18 = icmp eq i16 %581, 0
  br i1 %exitcond18, label %b140, label %b139

b140:
  %583 = add nuw nsw i16 %571, 1
  %exitcond19 = icmp eq i16 %583, 16
  %584 = getelementptr i8, ptr addrspace(1) %569, i16 2
  br i1 %exitcond19, label %b135, label %b134

b144:
  %585 = phi i16 [ %598, %b147 ], [ -2168, %b135 ]
  %586 = phi i16 [ 0, %b135 ], [ %597, %b147 ]
  %587 = uitofp nneg i16 %586 to float
  %588 = fdiv float %587, 4.500000e+01
  %589 = call float @llvm.fabs.f32(float %588)
  %590 = fcmp ogt float %589, 0x41E0000000000000
  br i1 %590, label %b146, label %b147

b146:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b147:
  %591 = call float @llvm.sin.f32(float %588)
  %592 = fmul float %591, 6.300000e+01
  %593 = fadd float %592, 9.200000e+01
  %594 = call i16 @llvm.lrint.i16.f32(float %593)
  %595 = getelementptr i8, ptr @"FSIN1%", i16 %585
  %596 = getelementptr i8, ptr %595, i16 2264
  store i16 %594, ptr %596, !tbaa !2
  %597 = add nuw nsw i16 %586, 1
  %598 = add nsw i16 %585, 2
  %exitcond17 = icmp eq i16 %598, 0
  br i1 %exitcond17, label %599, label %b144

599:
  br label %b151

b151:
  %600 = phi i16 [ -1916, %599 ], [ %613, %b154 ]
  %601 = phi i16 [ %612, %b154 ], [ 0, %599 ]
  %602 = uitofp nneg i16 %601 to float
  %603 = fdiv float %602, 2.500000e+01
  %604 = call float @llvm.fabs.f32(float %603)
  %605 = fcmp ogt float %604, 0x41E0000000000000
  br i1 %605, label %b153, label %b154

b153:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b154:
  %606 = call float @llvm.sin.f32(float %603)
  %607 = fmul float %606, 3.100000e+01
  %608 = fadd float %607, 3.100000e+01
  %609 = call i16 @llvm.lrint.i16.f32(float %608)
  %610 = getelementptr i8, ptr @"FSIN2%", i16 %600
  %611 = getelementptr i8, ptr %610, i16 3196
  store i16 %609, ptr %611, !tbaa !2
  %612 = add nuw nsw i16 %601, 1
  %613 = add nsw i16 %600, 2
  %exitcond16 = icmp eq i16 %613, 0
  br i1 %exitcond16, label %614, label %b151

614:
  br label %b158

b158:
  %615 = phi i16 [ 338, %614 ], [ %675, %b161 ]
  %616 = phi i16 [ %674, %b161 ], [ -471, %614 ]
  %617 = sitofp i16 %616 to float
  %618 = fdiv float %617, 7.500000e+01
  %619 = call float @llvm.fabs.f32(float %618)
  %620 = fcmp ogt float %619, 0x41E0000000000000
  br i1 %620, label %b160, label %b161

b159:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 15, i16 2, i16 257, ptr nonnull %13)
  %621 = getelementptr i8, ptr %0, i16 222
  %622 = load i16, ptr %621, !tbaa !2
  %623 = inttoptr i16 %622 to ptr addrspace(2)
  %624 = addrspacecast ptr addrspace(2) %623 to ptr addrspace(1)
  store i16 0, ptr addrspace(1) %624, !tbaa !38
  %625 = getelementptr i8, ptr addrspace(1) %624, i16 2
  store i16 16, ptr addrspace(1) %625, !tbaa !38
  %626 = getelementptr i8, ptr addrspace(1) %624, i16 4
  store i16 32, ptr addrspace(1) %626, !tbaa !38
  %627 = getelementptr i8, ptr addrspace(1) %624, i16 6
  store i16 48, ptr addrspace(1) %627, !tbaa !38
  %628 = getelementptr i8, ptr addrspace(1) %624, i16 8
  store i16 64, ptr addrspace(1) %628, !tbaa !38
  %629 = getelementptr i8, ptr addrspace(1) %624, i16 10
  store i16 80, ptr addrspace(1) %629, !tbaa !38
  %630 = getelementptr i8, ptr addrspace(1) %624, i16 12
  store i16 96, ptr addrspace(1) %630, !tbaa !38
  %631 = getelementptr i8, ptr addrspace(1) %624, i16 14
  store i16 112, ptr addrspace(1) %631, !tbaa !38
  %632 = getelementptr i8, ptr addrspace(1) %624, i16 16
  store i16 128, ptr addrspace(1) %632, !tbaa !38
  %633 = getelementptr i8, ptr addrspace(1) %624, i16 18
  store i16 144, ptr addrspace(1) %633, !tbaa !38
  %634 = getelementptr i8, ptr addrspace(1) %624, i16 20
  store i16 160, ptr addrspace(1) %634, !tbaa !38
  %635 = getelementptr i8, ptr addrspace(1) %624, i16 22
  store i16 176, ptr addrspace(1) %635, !tbaa !38
  %636 = getelementptr i8, ptr addrspace(1) %624, i16 24
  store i16 192, ptr addrspace(1) %636, !tbaa !38
  %637 = getelementptr i8, ptr addrspace(1) %624, i16 26
  store i16 208, ptr addrspace(1) %637, !tbaa !38
  %638 = getelementptr i8, ptr addrspace(1) %624, i16 28
  store i16 224, ptr addrspace(1) %638, !tbaa !38
  %639 = getelementptr i8, ptr addrspace(1) %624, i16 30
  store i16 240, ptr addrspace(1) %639, !tbaa !38
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 7, i16 2, i16 257, ptr nonnull %14)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 7, i16 2, i16 257, ptr nonnull %15)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 7, i16 2, i16 257, ptr nonnull %16)
  %640 = getelementptr i8, ptr %0, i16 240
  %641 = load i16, ptr %640, !tbaa !2
  %642 = inttoptr i16 %641 to ptr addrspace(2)
  %643 = addrspacecast ptr addrspace(2) %642 to ptr addrspace(1)
  %644 = getelementptr i8, ptr %0, i16 258
  %645 = load i16, ptr %644, !tbaa !2
  %646 = inttoptr i16 %645 to ptr addrspace(2)
  %647 = addrspacecast ptr addrspace(2) %646 to ptr addrspace(1)
  %648 = getelementptr i8, ptr %0, i16 276
  %649 = load i16, ptr %648, !tbaa !2
  %650 = inttoptr i16 %649 to ptr addrspace(2)
  %651 = addrspacecast ptr addrspace(2) %650 to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %643, !tbaa !40
  store i16 7, ptr addrspace(1) %647, !tbaa !42
  store i16 13, ptr addrspace(1) %651, !tbaa !44
  %652 = getelementptr i8, ptr addrspace(1) %643, i16 2
  store i16 2, ptr addrspace(1) %652, !tbaa !40
  %653 = getelementptr i8, ptr addrspace(1) %647, i16 2
  store i16 8, ptr addrspace(1) %653, !tbaa !42
  %654 = getelementptr i8, ptr addrspace(1) %651, i16 2
  store i16 14, ptr addrspace(1) %654, !tbaa !44
  %655 = getelementptr i8, ptr addrspace(1) %643, i16 4
  store i16 3, ptr addrspace(1) %655, !tbaa !40
  %656 = getelementptr i8, ptr addrspace(1) %647, i16 4
  store i16 9, ptr addrspace(1) %656, !tbaa !42
  %657 = getelementptr i8, ptr addrspace(1) %651, i16 4
  store i16 15, ptr addrspace(1) %657, !tbaa !44
  %658 = getelementptr i8, ptr addrspace(1) %643, i16 6
  store i16 4, ptr addrspace(1) %658, !tbaa !40
  %659 = getelementptr i8, ptr addrspace(1) %647, i16 6
  store i16 10, ptr addrspace(1) %659, !tbaa !42
  %660 = getelementptr i8, ptr addrspace(1) %651, i16 6
  store i16 16, ptr addrspace(1) %660, !tbaa !44
  %661 = getelementptr i8, ptr addrspace(1) %643, i16 8
  store i16 5, ptr addrspace(1) %661, !tbaa !40
  %662 = getelementptr i8, ptr addrspace(1) %647, i16 8
  store i16 11, ptr addrspace(1) %662, !tbaa !42
  %663 = getelementptr i8, ptr addrspace(1) %651, i16 8
  store i16 17, ptr addrspace(1) %663, !tbaa !44
  %664 = getelementptr i8, ptr addrspace(1) %643, i16 10
  store i16 6, ptr addrspace(1) %664, !tbaa !40
  %665 = getelementptr i8, ptr addrspace(1) %647, i16 10
  store i16 12, ptr addrspace(1) %665, !tbaa !42
  %666 = getelementptr i8, ptr addrspace(1) %651, i16 10
  store i16 18, ptr addrspace(1) %666, !tbaa !44
  %667 = getelementptr i8, ptr %0, i16 150
  %668 = load i16, ptr %667, !tbaa !2
  br label %b175

b160:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b161:
  %669 = call float @llvm.sin.f32(float %618)
  %670 = fmul float %669, 9.100000e+01
  %671 = fadd float %670, 1.270000e+02
  %672 = call i16 @llvm.lrint.i16.f32(float %671)
  %673 = getelementptr i8, ptr @"FSIN3%", i16 %615
  store i16 %672, ptr %673, !tbaa !2
  %674 = add nsw i16 %616, 1
  %675 = add nuw nsw i16 %615, 2
  %exitcond15 = icmp eq i16 %675, 3024
  br i1 %exitcond15, label %b159, label %b158

b175:
  %676 = phi i16 [ %680, %b175 ], [ 0, %b159 ]
  %677 = inttoptr i16 %668 to ptr addrspace(2)
  %678 = addrspacecast ptr addrspace(2) %677 to ptr addrspace(1)
  %679 = getelementptr i8, ptr addrspace(1) %678, i16 %676
  store i16 %676, ptr addrspace(1) %679, !tbaa !46
  %680 = add nuw nsw i16 %676, 2
  %exitcond12 = icmp eq i16 %680, 800
  br i1 %exitcond12, label %b176, label %b175

b176:
  %681 = getelementptr i8, ptr %0, i16 168
  %682 = load i16, ptr %681, !tbaa !2
  br label %b180

b180:
  %683 = phi i16 [ %690, %b180 ], [ -800, %b176 ]
  %IV.S. = phi float [ 0.000000e+00, %b176 ], [ %IV.S.next., %b180 ]
  %684 = inttoptr i16 %682 to ptr addrspace(2)
  %685 = addrspacecast ptr addrspace(2) %684 to ptr addrspace(1)
  %686 = getelementptr i8, ptr addrspace(1) %685, i16 %683
  %687 = getelementptr i8, ptr addrspace(1) %686, i16 800
  %688 = fdiv float %IV.S., 0x40199999A0000000
  %689 = call i16 @llvm.lrint.i16.f32(float %688)
  store i16 %689, ptr addrspace(1) %687, !tbaa !48
  %IV.S.next. = fadd float %IV.S., 1.000000e+00
  %690 = add nsw i16 %683, 2
  %exitcond11 = icmp eq i16 %690, 0
  br i1 %exitcond11, label %b181, label %b180

b181:
  %691 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %692 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %691, ptr @$string213)
  %693 = icmp eq i16 %692, 0
  br i1 %693, label %694, label %b3

694:
  %695 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 1
  %696 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 1
  br label %b183

b183:
  %697 = phi i16 [ %spec.store.select4, %b223 ], [ 0, %694 ]
  %698 = phi i16 [ %spec.store.select3, %b223 ], [ 0, %694 ]
  %699 = phi i16 [ %spec.store.select2, %b223 ], [ %269, %694 ]
  %700 = phi i16 [ %718, %b223 ], [ 0, %694 ]
  %701 = phi i16 [ %708, %b223 ], [ 0, %694 ]
  %702 = add i16 %699, 3
  %703 = icmp sgt i16 %702, 282
  %spec.store.select2 = select i1 %703, i16 0, i16 %702
  %704 = add i16 %698, 2
  %705 = icmp sgt i16 %704, 156
  %spec.store.select3 = select i1 %705, i16 0, i16 %704
  %706 = add i16 %697, 1
  %707 = icmp sgt i16 %706, 470
  %spec.store.select4 = select i1 %707, i16 0, i16 %706
  %708 = add i16 %701, 1
  %709 = icmp slt i16 %708, 400
  %710 = icmp slt i16 %700, 395
  %711 = and i1 %709, %710
  %712 = zext i1 %711 to i16
  %713 = add i16 %700, %712
  %714 = icmp sgt i16 %708, 1200
  %715 = icmp ne i16 %713, 0
  %716 = and i1 %714, %715
  %717 = sext i1 %716 to i16
  %718 = add i16 %713, %717
  %719 = icmp eq i16 %718, 0
  br i1 %719, label %b3, label %b202

b202:
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  store i16 -24576, ptr @b$seg, !tbaa !2
  %invariant.gep3 = getelementptr [2 x i8], ptr @"FSIN2%", i16 %spec.store.select3
  %invariant.gep5 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %spec.store.select2
  %720 = icmp slt i16 %718, 0
  br i1 %720, label %814, label %721

721:
  %722 = icmp ne i16 %713, 0
  %723 = icmp sgt i16 %708, 1200
  %724 = icmp slt i16 %700, 395
  %725 = icmp slt i16 %708, 400
  %726 = getelementptr i8, ptr %0, i16 150
  %727 = load i16, ptr %726, !tbaa !2
  %728 = getelementptr i8, ptr %0, i16 114
  %729 = load i16, ptr %728, !tbaa !2
  %730 = inttoptr i16 %729 to ptr addrspace(2)
  %731 = addrspacecast ptr addrspace(2) %730 to ptr addrspace(1)
  %732 = getelementptr i8, ptr %0, i16 132
  %733 = load i16, ptr %732, !tbaa !2
  %734 = inttoptr i16 %733 to ptr addrspace(2)
  %735 = addrspacecast ptr addrspace(2) %734 to ptr addrspace(1)
  %736 = getelementptr i8, ptr %0, i16 222
  %737 = load i16, ptr %736, !tbaa !2
  %738 = inttoptr i16 %737 to ptr addrspace(2)
  %739 = getelementptr i8, ptr %0, i16 168
  %740 = load i16, ptr %739, !tbaa !2
  %741 = add i16 %700, 1
  %742 = and i1 %724, %725
  %743 = zext i1 %742 to i16
  %744 = add i16 %741, %743
  %745 = and i1 %722, %723
  %746 = sext i1 %745 to i16
  %747 = add i16 %744, %746
  %748 = getelementptr i8, ptr addrspace(1) %731, i16 1282
  %749 = getelementptr i8, ptr addrspace(1) %735, i16 1282
  %750 = getelementptr i8, ptr addrspace(1) %735, i16 1282
  %751 = getelementptr i8, ptr addrspace(1) %731, i16 1282
  br label %b207

b207:
  %752 = phi i16 [ 0, %721 ], [ %813, %b213 ]
  %753 = inttoptr i16 %727 to ptr addrspace(2)
  %754 = addrspacecast ptr addrspace(2) %753 to ptr addrspace(1)
  %755 = shl nuw i16 %752, 1
  %756 = getelementptr i8, ptr addrspace(1) %754, i16 %755
  %757 = load i16, ptr addrspace(1) %756, !tbaa !46
  %gep3 = getelementptr [2 x i8], ptr %invariant.gep3, i16 %757
  %758 = getelementptr i8, ptr %gep3, i16 1280
  %759 = load i16, ptr %758, !tbaa !2
  %760 = sub i16 %752, %spec.store.select4
  %761 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %760
  %762 = getelementptr i8, ptr %761, i16 1280
  %763 = load i16, ptr %762, !tbaa !2
  %gep4 = getelementptr [2 x i8], ptr %invariant.gep3, i16 %752
  %764 = getelementptr i8, ptr %gep4, i16 1280
  %765 = load i16, ptr %764, !tbaa !2
  %gep5 = getelementptr [2 x i8], ptr %invariant.gep5, i16 %757
  %766 = getelementptr i8, ptr %gep5, i16 96
  %767 = load i16, ptr %766, !tbaa !2
  %768 = add i16 %767, %765
  %769 = getelementptr i8, ptr addrspace(1) %695, i16 %768
  %770 = shl i16 %768, 1
  %771 = getelementptr i8, ptr addrspace(1) %748, i16 %770
  %772 = getelementptr i8, ptr addrspace(1) %749, i16 %770
  %773 = add i16 %763, %759
  %774 = getelementptr i8, ptr addrspace(1) %696, i16 %773
  %775 = shl i16 %773, 1
  %776 = getelementptr i8, ptr addrspace(1) %750, i16 %775
  %777 = getelementptr i8, ptr addrspace(1) %751, i16 %775
  br label %b212

b212:
  %778 = phi i16 [ %801, %b212 ], [ 0, %b207 ]
  %779 = addrspacecast ptr addrspace(2) %738 to ptr addrspace(1)
  %780 = add i16 %778, 1
  %781 = shl nuw nsw i16 %778, 1
  %782 = getelementptr i8, ptr addrspace(1) %777, i16 %781
  store i16 %780, ptr addrspace(1) %782, !tbaa !50
  %783 = shl nuw nsw i16 %778, 1
  %784 = getelementptr i8, ptr addrspace(1) %772, i16 %783
  %785 = shl nuw nsw i16 %778, 1
  %786 = getelementptr i8, ptr addrspace(1) %779, i16 %785
  %787 = getelementptr i8, ptr addrspace(1) %786, i16 2
  %788 = load i16, ptr addrspace(1) %787, !tbaa !38
  store i16 %788, ptr addrspace(1) %784, !tbaa !52
  %789 = getelementptr i8, ptr addrspace(1) %776, i16 %785
  %790 = load i16, ptr addrspace(1) %789, !tbaa !52
  %791 = or i16 %790, %780
  %792 = trunc i16 %791 to i8
  %793 = getelementptr i8, ptr addrspace(1) %774, i16 %778
  store volatile i8 %792, ptr addrspace(1) %793, !tbaa !4
  %794 = load i16, ptr addrspace(1) %787, !tbaa !38
  %795 = shl nuw nsw i16 %778, 1
  %796 = getelementptr i8, ptr addrspace(1) %771, i16 %795
  %797 = load i16, ptr addrspace(1) %796, !tbaa !50
  %798 = or i16 %797, %794
  %799 = trunc i16 %798 to i8
  %800 = getelementptr i8, ptr addrspace(1) %769, i16 %778
  store volatile i8 %799, ptr addrspace(1) %800, !tbaa !4
  %801 = add nuw nsw i16 %778, 1
  %exitcond9 = icmp eq i16 %801, 15
  br i1 %exitcond9, label %b213, label %b212

b213:
  %802 = inttoptr i16 %740 to ptr addrspace(2)
  %803 = addrspacecast ptr addrspace(2) %802 to ptr addrspace(1)
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %804 = getelementptr i8, ptr addrspace(1) %803, i16 %755
  %805 = load i16, ptr addrspace(1) %804, !tbaa !48
  %806 = trunc i16 %805 to i8
  %807 = sub i8 63, %806
  call void @llrm.ia16.out.i8(i16 969, i8 %807) memory(inaccessiblemem: readwrite)
  %808 = sub i16 63, %805
  %809 = sitofp i16 %808 to float
  %810 = fmul float %809, 5.000000e-01
  %811 = call i16 @llvm.lrint.i16.f32(float %810)
  %812 = trunc i16 %811 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %812) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 %806) memory(inaccessiblemem: readwrite)
  %813 = add nuw i16 %752, 1
  %exitcond10 = icmp eq i16 %813, %747
  br i1 %exitcond10, label %814, label %b207

814:
  br label %b217

b217:
  %815 = phi i16 [ -320, %814 ], [ %818, %b217 ]
  %816 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %815
  %817 = getelementptr i8, ptr addrspace(1) %816, i16 320
  store volatile i8 0, ptr addrspace(1) %817, !tbaa !4
  %818 = add nsw i16 %815, 1
  %exitcond8 = icmp eq i16 %818, 0
  br i1 %exitcond8, label %b218, label %b217

b218:
  %819 = getelementptr i8, ptr %0, i16 114
  %820 = load i16, ptr %819, !tbaa !2
  %821 = inttoptr i16 %820 to ptr addrspace(2)
  %822 = getelementptr i8, ptr %0, i16 132
  %823 = load i16, ptr %822, !tbaa !2
  %824 = inttoptr i16 %823 to ptr addrspace(2)
  br label %b222

b222:
  %825 = phi i16 [ %832, %b222 ], [ -640, %b218 ]
  %826 = addrspacecast ptr addrspace(2) %824 to ptr addrspace(1)
  %827 = addrspacecast ptr addrspace(2) %821 to ptr addrspace(1)
  %828 = getelementptr i8, ptr addrspace(1) %827, i16 %825
  %829 = getelementptr i8, ptr addrspace(1) %828, i16 1920
  store i16 0, ptr addrspace(1) %829, !tbaa !50
  %830 = getelementptr i8, ptr addrspace(1) %826, i16 %825
  %831 = getelementptr i8, ptr addrspace(1) %830, i16 1920
  store i16 0, ptr addrspace(1) %831, !tbaa !52
  %832 = add nsw i16 %825, 2
  %exitcond7 = icmp eq i16 %832, 0
  br i1 %exitcond7, label %b223, label %b222

b223:
  %833 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %834 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %833, ptr @$string213)
  %835 = icmp eq i16 %834, 0
  br i1 %835, label %b183, label %b3

b245:
  %836 = phi i16 [ %spec.store.select7, %b280 ], [ 0, %58 ]
  %837 = phi i16 [ %spec.store.select6, %b280 ], [ 0, %58 ]
  %838 = phi i16 [ %spec.store.select5, %b280 ], [ 0, %58 ]
  %839 = phi i16 [ %857, %b280 ], [ 0, %58 ]
  %840 = phi i16 [ %847, %b280 ], [ 0, %58 ]
  %841 = add i16 %838, 3
  %842 = icmp sgt i16 %841, 282
  %spec.store.select5 = select i1 %842, i16 0, i16 %841
  %843 = add i16 %837, 2
  %844 = icmp sgt i16 %843, 156
  %spec.store.select6 = select i1 %844, i16 0, i16 %843
  %845 = add i16 %836, 1
  %846 = icmp sgt i16 %845, 470
  %spec.store.select7 = select i1 %846, i16 0, i16 %845
  %847 = add i16 %840, 1
  %848 = icmp slt i16 %847, 400
  %849 = icmp slt i16 %839, 395
  %850 = and i1 %848, %849
  %851 = zext i1 %850 to i16
  %852 = add i16 %839, %851
  %853 = icmp sgt i16 %847, 1200
  %854 = icmp ne i16 %852, 0
  %855 = and i1 %853, %854
  %856 = sext i1 %855 to i16
  %857 = add i16 %852, %856
  %858 = icmp eq i16 %857, 0
  br i1 %858, label %b4, label %b264

b264:
  store i16 -24576, ptr @b$seg, !tbaa !2
  %859 = icmp slt i16 %857, 0
  br i1 %859, label %1169, label %860

860:
  %861 = icmp ne i16 %852, 0
  %862 = icmp sgt i16 %847, 1200
  %863 = icmp slt i16 %839, 395
  %864 = icmp slt i16 %847, 400
  %865 = getelementptr i8, ptr %0, i16 150
  %866 = load i16, ptr %865, !tbaa !2
  %invariant.gep1 = getelementptr [2 x i8], ptr @"FSIN2%", i16 %spec.store.select6
  %867 = getelementptr i8, ptr %0, i16 240
  %868 = load i16, ptr %867, !tbaa !2
  %869 = inttoptr i16 %868 to ptr addrspace(2)
  %invariant.gep2 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %spec.store.select5
  %870 = getelementptr i8, ptr %0, i16 258
  %871 = load i16, ptr %870, !tbaa !2
  %872 = inttoptr i16 %871 to ptr addrspace(2)
  %873 = getelementptr i8, ptr %0, i16 276
  %874 = load i16, ptr %873, !tbaa !2
  %875 = inttoptr i16 %874 to ptr addrspace(2)
  %876 = getelementptr i8, ptr %0, i16 168
  %877 = load i16, ptr %876, !tbaa !2
  %878 = add i16 %839, 1
  %879 = zext i1 %863 to i16
  %880 = zext i1 %864 to i16
  %881 = call i16 @llvm.umin.i16(i16 %879, i16 %880)
  %882 = add i16 %878, %881
  %883 = call i1 @llvm.umin.i1(i1 %862, i1 %861)
  %884 = sext i1 %883 to i16
  %885 = add i16 %882, %884
  %886 = shl i16 %spec.store.select7, 1
  %887 = shl i16 %spec.store.select5, 1
  %888 = shl i16 %spec.store.select6, 1
  %889 = mul i16 %spec.store.select7, -2
  br label %b269

b269:
  %890 = phi i16 [ 0, %860 ], [ %1168, %b269 ]
  %891 = inttoptr i16 %877 to ptr addrspace(2)
  %892 = addrspacecast ptr addrspace(2) %875 to ptr addrspace(1)
  %893 = inttoptr i16 %874 to ptr addrspace(2)
  %894 = addrspacecast ptr addrspace(2) %872 to ptr addrspace(1)
  %895 = inttoptr i16 %871 to ptr addrspace(2)
  %896 = addrspacecast ptr addrspace(2) %869 to ptr addrspace(1)
  %897 = inttoptr i16 %868 to ptr addrspace(2)
  %898 = inttoptr i16 %866 to ptr addrspace(2)
  %899 = addrspacecast ptr addrspace(2) %891 to ptr addrspace(1)
  %900 = addrspacecast ptr addrspace(2) %893 to ptr addrspace(1)
  %901 = addrspacecast ptr addrspace(2) %895 to ptr addrspace(1)
  %902 = addrspacecast ptr addrspace(2) %897 to ptr addrspace(1)
  %903 = addrspacecast ptr addrspace(2) %898 to ptr addrspace(1)
  %904 = shl i16 %890, 1
  %905 = getelementptr i8, ptr addrspace(1) %903, i16 %904
  %906 = shl i16 %890, 1
  %907 = shl i16 %890, 1
  %908 = shl i16 %890, 1
  %909 = shl i16 %890, 1
  %910 = load i16, ptr addrspace(1) %905, !tbaa !46
  %gep1 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %910
  %911 = getelementptr i8, ptr %gep1, i16 1280
  %912 = load i16, ptr %911, !tbaa !2
  %913 = getelementptr i8, ptr @"FSIN3%", i16 %889
  %914 = getelementptr i8, ptr %913, i16 %906
  %915 = getelementptr i8, ptr %914, i16 1280
  %916 = load i16, ptr %915, !tbaa !2
  %917 = load i16, ptr addrspace(1) %902, !tbaa !40
  %918 = trunc i16 %917 to i8
  %919 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %912
  %920 = getelementptr i8, ptr addrspace(1) %919, i16 %916
  store volatile i8 %918, ptr addrspace(1) %920, !tbaa !4
  %921 = getelementptr i8, ptr @"FSIN2%", i16 %888
  %922 = getelementptr i8, ptr %921, i16 %907
  %923 = getelementptr i8, ptr %922, i16 1280
  %924 = load i16, ptr %923, !tbaa !2
  %925 = load i16, ptr addrspace(1) %905, !tbaa !46
  %gep2 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %925
  %926 = getelementptr i8, ptr %gep2, i16 96
  %927 = load i16, ptr %926, !tbaa !2
  %928 = load i16, ptr addrspace(1) %901, !tbaa !42
  %929 = trunc i16 %928 to i8
  %930 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %924
  %931 = getelementptr i8, ptr addrspace(1) %930, i16 %927
  store volatile i8 %929, ptr addrspace(1) %931, !tbaa !4
  %932 = getelementptr i8, ptr @"FSIN1%", i16 %887
  %933 = getelementptr i8, ptr %932, i16 %908
  %934 = getelementptr i8, ptr %933, i16 96
  %935 = load i16, ptr %934, !tbaa !2
  %936 = getelementptr i8, ptr @"FSIN3%", i16 %886
  %937 = getelementptr i8, ptr %936, i16 %909
  %938 = getelementptr i8, ptr %937, i16 1280
  %939 = load i16, ptr %938, !tbaa !2
  %940 = load i16, ptr addrspace(1) %900, !tbaa !44
  %941 = trunc i16 %940 to i8
  %942 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %935
  %943 = getelementptr i8, ptr addrspace(1) %942, i16 %924
  %944 = getelementptr i8, ptr addrspace(1) %943, i16 %939
  %945 = getelementptr i8, ptr addrspace(1) %944, i16 -99
  store volatile i8 %941, ptr addrspace(1) %945, !tbaa !4
  %946 = load i16, ptr addrspace(1) %905, !tbaa !46
  %947 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %946
  %948 = getelementptr i8, ptr %947, i16 1280
  %949 = load i16, ptr %948, !tbaa !2
  %950 = load i16, ptr %915, !tbaa !2
  %951 = getelementptr i8, ptr addrspace(1) %896, i16 2
  %952 = load i16, ptr addrspace(1) %951, !tbaa !40
  %953 = trunc i16 %952 to i8
  %954 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %949
  %955 = getelementptr i8, ptr addrspace(1) %954, i16 %950
  %956 = getelementptr i8, ptr addrspace(1) %955, i16 1
  store volatile i8 %953, ptr addrspace(1) %956, !tbaa !4
  %957 = load i16, ptr %923, !tbaa !2
  %958 = load i16, ptr addrspace(1) %905, !tbaa !46
  %959 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %958
  %960 = getelementptr i8, ptr %959, i16 96
  %961 = load i16, ptr %960, !tbaa !2
  %962 = getelementptr i8, ptr addrspace(1) %894, i16 2
  %963 = load i16, ptr addrspace(1) %962, !tbaa !42
  %964 = trunc i16 %963 to i8
  %965 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %957
  %966 = getelementptr i8, ptr addrspace(1) %965, i16 %961
  %967 = getelementptr i8, ptr addrspace(1) %966, i16 1
  store volatile i8 %964, ptr addrspace(1) %967, !tbaa !4
  %968 = load i16, ptr %934, !tbaa !2
  %969 = load i16, ptr %938, !tbaa !2
  %970 = getelementptr i8, ptr addrspace(1) %892, i16 2
  %971 = load i16, ptr addrspace(1) %970, !tbaa !44
  %972 = trunc i16 %971 to i8
  %973 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %968
  %974 = getelementptr i8, ptr addrspace(1) %973, i16 %957
  %975 = getelementptr i8, ptr addrspace(1) %974, i16 %969
  %976 = getelementptr i8, ptr addrspace(1) %975, i16 -98
  store volatile i8 %972, ptr addrspace(1) %976, !tbaa !4
  %977 = load i16, ptr addrspace(1) %905, !tbaa !46
  %978 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %977
  %979 = getelementptr i8, ptr %978, i16 1280
  %980 = load i16, ptr %979, !tbaa !2
  %981 = load i16, ptr %915, !tbaa !2
  %982 = getelementptr i8, ptr addrspace(1) %896, i16 4
  %983 = load i16, ptr addrspace(1) %982, !tbaa !40
  %984 = trunc i16 %983 to i8
  %985 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %980
  %986 = getelementptr i8, ptr addrspace(1) %985, i16 %981
  %987 = getelementptr i8, ptr addrspace(1) %986, i16 2
  store volatile i8 %984, ptr addrspace(1) %987, !tbaa !4
  %988 = load i16, ptr %923, !tbaa !2
  %989 = load i16, ptr addrspace(1) %905, !tbaa !46
  %990 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %989
  %991 = getelementptr i8, ptr %990, i16 96
  %992 = load i16, ptr %991, !tbaa !2
  %993 = getelementptr i8, ptr addrspace(1) %894, i16 4
  %994 = load i16, ptr addrspace(1) %993, !tbaa !42
  %995 = trunc i16 %994 to i8
  %996 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %988
  %997 = getelementptr i8, ptr addrspace(1) %996, i16 %992
  %998 = getelementptr i8, ptr addrspace(1) %997, i16 2
  store volatile i8 %995, ptr addrspace(1) %998, !tbaa !4
  %999 = load i16, ptr %934, !tbaa !2
  %1000 = load i16, ptr %938, !tbaa !2
  %1001 = getelementptr i8, ptr addrspace(1) %892, i16 4
  %1002 = load i16, ptr addrspace(1) %1001, !tbaa !44
  %1003 = trunc i16 %1002 to i8
  %1004 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %999
  %1005 = getelementptr i8, ptr addrspace(1) %1004, i16 %988
  %1006 = getelementptr i8, ptr addrspace(1) %1005, i16 %1000
  %1007 = getelementptr i8, ptr addrspace(1) %1006, i16 -97
  store volatile i8 %1003, ptr addrspace(1) %1007, !tbaa !4
  %1008 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1009 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %1008
  %1010 = getelementptr i8, ptr %1009, i16 1280
  %1011 = load i16, ptr %1010, !tbaa !2
  %1012 = load i16, ptr %915, !tbaa !2
  %1013 = getelementptr i8, ptr addrspace(1) %896, i16 6
  %1014 = load i16, ptr addrspace(1) %1013, !tbaa !40
  %1015 = trunc i16 %1014 to i8
  %1016 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1011
  %1017 = getelementptr i8, ptr addrspace(1) %1016, i16 %1012
  %1018 = getelementptr i8, ptr addrspace(1) %1017, i16 3
  store volatile i8 %1015, ptr addrspace(1) %1018, !tbaa !4
  %1019 = load i16, ptr %923, !tbaa !2
  %1020 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1021 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %1020
  %1022 = getelementptr i8, ptr %1021, i16 96
  %1023 = load i16, ptr %1022, !tbaa !2
  %1024 = getelementptr i8, ptr addrspace(1) %894, i16 6
  %1025 = load i16, ptr addrspace(1) %1024, !tbaa !42
  %1026 = trunc i16 %1025 to i8
  %1027 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1019
  %1028 = getelementptr i8, ptr addrspace(1) %1027, i16 %1023
  %1029 = getelementptr i8, ptr addrspace(1) %1028, i16 3
  store volatile i8 %1026, ptr addrspace(1) %1029, !tbaa !4
  %1030 = load i16, ptr %934, !tbaa !2
  %1031 = load i16, ptr %938, !tbaa !2
  %1032 = getelementptr i8, ptr addrspace(1) %892, i16 6
  %1033 = load i16, ptr addrspace(1) %1032, !tbaa !44
  %1034 = trunc i16 %1033 to i8
  %1035 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1030
  %1036 = getelementptr i8, ptr addrspace(1) %1035, i16 %1019
  %1037 = getelementptr i8, ptr addrspace(1) %1036, i16 %1031
  %1038 = getelementptr i8, ptr addrspace(1) %1037, i16 -96
  store volatile i8 %1034, ptr addrspace(1) %1038, !tbaa !4
  %1039 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1040 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %1039
  %1041 = getelementptr i8, ptr %1040, i16 1280
  %1042 = load i16, ptr %1041, !tbaa !2
  %1043 = load i16, ptr %915, !tbaa !2
  %1044 = getelementptr i8, ptr addrspace(1) %896, i16 8
  %1045 = load i16, ptr addrspace(1) %1044, !tbaa !40
  %1046 = trunc i16 %1045 to i8
  %1047 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1042
  %1048 = getelementptr i8, ptr addrspace(1) %1047, i16 %1043
  %1049 = getelementptr i8, ptr addrspace(1) %1048, i16 4
  store volatile i8 %1046, ptr addrspace(1) %1049, !tbaa !4
  %1050 = load i16, ptr %923, !tbaa !2
  %1051 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1052 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %1051
  %1053 = getelementptr i8, ptr %1052, i16 96
  %1054 = load i16, ptr %1053, !tbaa !2
  %1055 = getelementptr i8, ptr addrspace(1) %894, i16 8
  %1056 = load i16, ptr addrspace(1) %1055, !tbaa !42
  %1057 = trunc i16 %1056 to i8
  %1058 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1050
  %1059 = getelementptr i8, ptr addrspace(1) %1058, i16 %1054
  %1060 = getelementptr i8, ptr addrspace(1) %1059, i16 4
  store volatile i8 %1057, ptr addrspace(1) %1060, !tbaa !4
  %1061 = load i16, ptr %934, !tbaa !2
  %1062 = load i16, ptr %938, !tbaa !2
  %1063 = getelementptr i8, ptr addrspace(1) %892, i16 8
  %1064 = load i16, ptr addrspace(1) %1063, !tbaa !44
  %1065 = trunc i16 %1064 to i8
  %1066 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1061
  %1067 = getelementptr i8, ptr addrspace(1) %1066, i16 %1050
  %1068 = getelementptr i8, ptr addrspace(1) %1067, i16 %1062
  %1069 = getelementptr i8, ptr addrspace(1) %1068, i16 -95
  store volatile i8 %1065, ptr addrspace(1) %1069, !tbaa !4
  %1070 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1071 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %1070
  %1072 = getelementptr i8, ptr %1071, i16 1280
  %1073 = load i16, ptr %1072, !tbaa !2
  %1074 = load i16, ptr %915, !tbaa !2
  %1075 = getelementptr i8, ptr addrspace(1) %896, i16 10
  %1076 = load i16, ptr addrspace(1) %1075, !tbaa !40
  %1077 = trunc i16 %1076 to i8
  %1078 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1073
  %1079 = getelementptr i8, ptr addrspace(1) %1078, i16 %1074
  %1080 = getelementptr i8, ptr addrspace(1) %1079, i16 5
  store volatile i8 %1077, ptr addrspace(1) %1080, !tbaa !4
  %1081 = load i16, ptr %923, !tbaa !2
  %1082 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1083 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %1082
  %1084 = getelementptr i8, ptr %1083, i16 96
  %1085 = load i16, ptr %1084, !tbaa !2
  %1086 = getelementptr i8, ptr addrspace(1) %894, i16 10
  %1087 = load i16, ptr addrspace(1) %1086, !tbaa !42
  %1088 = trunc i16 %1087 to i8
  %1089 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1081
  %1090 = getelementptr i8, ptr addrspace(1) %1089, i16 %1085
  %1091 = getelementptr i8, ptr addrspace(1) %1090, i16 5
  store volatile i8 %1088, ptr addrspace(1) %1091, !tbaa !4
  %1092 = load i16, ptr %934, !tbaa !2
  %1093 = load i16, ptr %938, !tbaa !2
  %1094 = getelementptr i8, ptr addrspace(1) %892, i16 10
  %1095 = load i16, ptr addrspace(1) %1094, !tbaa !44
  %1096 = trunc i16 %1095 to i8
  %1097 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1092
  %1098 = getelementptr i8, ptr addrspace(1) %1097, i16 %1081
  %1099 = getelementptr i8, ptr addrspace(1) %1098, i16 %1093
  %1100 = getelementptr i8, ptr addrspace(1) %1099, i16 -94
  store volatile i8 %1096, ptr addrspace(1) %1100, !tbaa !4
  %1101 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1102 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %1101
  %1103 = getelementptr i8, ptr %1102, i16 1280
  %1104 = load i16, ptr %1103, !tbaa !2
  %1105 = load i16, ptr %915, !tbaa !2
  %1106 = getelementptr i8, ptr addrspace(1) %896, i16 12
  %1107 = load i16, ptr addrspace(1) %1106, !tbaa !40
  %1108 = trunc i16 %1107 to i8
  %1109 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1104
  %1110 = getelementptr i8, ptr addrspace(1) %1109, i16 %1105
  %1111 = getelementptr i8, ptr addrspace(1) %1110, i16 6
  store volatile i8 %1108, ptr addrspace(1) %1111, !tbaa !4
  %1112 = load i16, ptr %923, !tbaa !2
  %1113 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1114 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %1113
  %1115 = getelementptr i8, ptr %1114, i16 96
  %1116 = load i16, ptr %1115, !tbaa !2
  %1117 = getelementptr i8, ptr addrspace(1) %894, i16 12
  %1118 = load i16, ptr addrspace(1) %1117, !tbaa !42
  %1119 = trunc i16 %1118 to i8
  %1120 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1112
  %1121 = getelementptr i8, ptr addrspace(1) %1120, i16 %1116
  %1122 = getelementptr i8, ptr addrspace(1) %1121, i16 6
  store volatile i8 %1119, ptr addrspace(1) %1122, !tbaa !4
  %1123 = load i16, ptr %934, !tbaa !2
  %1124 = load i16, ptr %938, !tbaa !2
  %1125 = getelementptr i8, ptr addrspace(1) %892, i16 12
  %1126 = load i16, ptr addrspace(1) %1125, !tbaa !44
  %1127 = trunc i16 %1126 to i8
  %1128 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1123
  %1129 = getelementptr i8, ptr addrspace(1) %1128, i16 %1112
  %1130 = getelementptr i8, ptr addrspace(1) %1129, i16 %1124
  %1131 = getelementptr i8, ptr addrspace(1) %1130, i16 -93
  store volatile i8 %1127, ptr addrspace(1) %1131, !tbaa !4
  %1132 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1133 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %1132
  %1134 = getelementptr i8, ptr %1133, i16 1280
  %1135 = load i16, ptr %1134, !tbaa !2
  %1136 = load i16, ptr %915, !tbaa !2
  %1137 = getelementptr i8, ptr addrspace(1) %896, i16 14
  %1138 = load i16, ptr addrspace(1) %1137, !tbaa !40
  %1139 = trunc i16 %1138 to i8
  %1140 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1135
  %1141 = getelementptr i8, ptr addrspace(1) %1140, i16 %1136
  %1142 = getelementptr i8, ptr addrspace(1) %1141, i16 7
  store volatile i8 %1139, ptr addrspace(1) %1142, !tbaa !4
  %1143 = load i16, ptr %923, !tbaa !2
  %1144 = load i16, ptr addrspace(1) %905, !tbaa !46
  %1145 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %1144
  %1146 = getelementptr i8, ptr %1145, i16 96
  %1147 = load i16, ptr %1146, !tbaa !2
  %1148 = getelementptr i8, ptr addrspace(1) %894, i16 14
  %1149 = load i16, ptr addrspace(1) %1148, !tbaa !42
  %1150 = trunc i16 %1149 to i8
  %1151 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1143
  %1152 = getelementptr i8, ptr addrspace(1) %1151, i16 %1147
  %1153 = getelementptr i8, ptr addrspace(1) %1152, i16 7
  store volatile i8 %1150, ptr addrspace(1) %1153, !tbaa !4
  %1154 = load i16, ptr %934, !tbaa !2
  %1155 = load i16, ptr %938, !tbaa !2
  %1156 = getelementptr i8, ptr addrspace(1) %892, i16 14
  %1157 = load i16, ptr addrspace(1) %1156, !tbaa !44
  %1158 = trunc i16 %1157 to i8
  %1159 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1154
  %1160 = getelementptr i8, ptr addrspace(1) %1159, i16 %1143
  %1161 = getelementptr i8, ptr addrspace(1) %1160, i16 %1155
  %1162 = getelementptr i8, ptr addrspace(1) %1161, i16 -92
  store volatile i8 %1158, ptr addrspace(1) %1162, !tbaa !4
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %1163 = shl i16 %890, 1
  %1164 = getelementptr i8, ptr addrspace(1) %899, i16 %1163
  %1165 = load i16, ptr addrspace(1) %1164, !tbaa !48
  %1166 = trunc i16 %1165 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %1166) memory(inaccessiblemem: readwrite)
  %1167 = sub i8 63, %1166
  call void @llrm.ia16.out.i8(i16 969, i8 %1167) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 969, i8 %1167) memory(inaccessiblemem: readwrite)
  %1168 = add i16 %890, 1
  %exitcond2 = icmp eq i16 %885, %1168
  br i1 %exitcond2, label %1169, label %b269

1169:
  br label %b279

b279:
  %1170 = phi i16 [ -320, %1169 ], [ %1173, %b279 ]
  %1171 = getelementptr i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), i16 %1170
  %1172 = getelementptr i8, ptr addrspace(1) %1171, i16 320
  store volatile i8 0, ptr addrspace(1) %1172, !tbaa !4
  %1173 = add nsw i16 %1170, 1
  %exitcond = icmp eq i16 %1173, 0
  br i1 %exitcond, label %b280, label %b279

b280:
  %1174 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %1175 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %1174, ptr @$string214)
  %1176 = icmp eq i16 %1175, 0
  br i1 %1176, label %b245, label %b4
}

define cc 1000 void @CREATEOBJECT(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  br label %2

2:
  %3 = phi i16 [ 0, %1 ], [ %15, %14 ]
  %4 = mul nuw nsw i16 %3, 20
  %5 = add nuw nsw i16 %4, -24576
  store i16 %5, ptr @b$seg, !tbaa !2
  br label %6

6:
  %7 = phi i16 [ %13, %6 ], [ -320, %2 ]
  %8 = load i16, ptr @b$seg, !tbaa !2
  %9 = inttoptr i16 %8 to ptr addrspace(2)
  %10 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 %7
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 320
  store volatile i8 0, ptr addrspace(1) %12
  %13 = add nsw i16 %7, 1
  %exitcond = icmp eq i16 %13, 0
  br i1 %exitcond, label %14, label %6

14:
  %15 = add nuw nsw i16 %3, 1
  %exitcond1 = icmp eq i16 %15, 200
  br i1 %exitcond1, label %16, label %2

16:
  store i16 -24576, ptr @b$seg, !tbaa !2
  %17 = load i16, ptr %0
  %18 = icmp eq i16 %17, 1
  br i1 %18, label %b3, label %19

19:
  %20 = icmp eq i16 %17, 2
  br i1 %20, label %b20, label %21

21:
  %22 = icmp eq i16 %17, 3
  br i1 %22, label %b55, label %b2

b2:
  ret void

b3:
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %24 = inttoptr i16 %23 to ptr addrspace(2)
  %25 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %26 = inttoptr i16 %25 to ptr addrspace(2)
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %28 = inttoptr i16 %27 to ptr addrspace(2)
  br label %b8

b8:
  %29 = phi i16 [ %106, %b8 ], [ -10, %b3 ]
  %30 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %31 = addrspacecast ptr addrspace(2) %26 to ptr addrspace(1)
  %32 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %33 = add i16 %29, 5
  %34 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 10
  store i16 %33, ptr addrspace(1) %35, !tbaa !16
  %36 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %37 = getelementptr i8, ptr addrspace(1) %36, i16 10
  store i16 5, ptr addrspace(1) %37, !tbaa !20
  %38 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 10
  store i16 5, ptr addrspace(1) %39, !tbaa !18
  %40 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 20
  store i16 %33, ptr addrspace(1) %41, !tbaa !16
  %42 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 20
  store i16 5, ptr addrspace(1) %43, !tbaa !20
  %44 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %45 = getelementptr i8, ptr addrspace(1) %44, i16 20
  store i16 -5, ptr addrspace(1) %45, !tbaa !18
  %46 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 30
  store i16 %33, ptr addrspace(1) %47, !tbaa !16
  %48 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %49 = getelementptr i8, ptr addrspace(1) %48, i16 30
  store i16 -5, ptr addrspace(1) %49, !tbaa !20
  %50 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %51 = getelementptr i8, ptr addrspace(1) %50, i16 30
  store i16 5, ptr addrspace(1) %51, !tbaa !18
  %52 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %53 = getelementptr i8, ptr addrspace(1) %52, i16 40
  store i16 %33, ptr addrspace(1) %53, !tbaa !16
  %54 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %55 = getelementptr i8, ptr addrspace(1) %54, i16 40
  store i16 -5, ptr addrspace(1) %55, !tbaa !20
  %56 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %57 = getelementptr i8, ptr addrspace(1) %56, i16 40
  store i16 -5, ptr addrspace(1) %57, !tbaa !18
  %58 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 50
  store i16 -5, ptr addrspace(1) %59, !tbaa !16
  %60 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %61 = getelementptr i8, ptr addrspace(1) %60, i16 50
  store i16 5, ptr addrspace(1) %61, !tbaa !20
  %62 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %63 = getelementptr i8, ptr addrspace(1) %62, i16 50
  store i16 %33, ptr addrspace(1) %63, !tbaa !18
  %64 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %65 = getelementptr i8, ptr addrspace(1) %64, i16 60
  store i16 5, ptr addrspace(1) %65, !tbaa !16
  %66 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 60
  store i16 5, ptr addrspace(1) %67, !tbaa !20
  %68 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %69 = getelementptr i8, ptr addrspace(1) %68, i16 60
  store i16 %33, ptr addrspace(1) %69, !tbaa !18
  %70 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 70
  store i16 -5, ptr addrspace(1) %71, !tbaa !16
  %72 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %73 = getelementptr i8, ptr addrspace(1) %72, i16 70
  store i16 -5, ptr addrspace(1) %73, !tbaa !20
  %74 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %75 = getelementptr i8, ptr addrspace(1) %74, i16 70
  store i16 %33, ptr addrspace(1) %75, !tbaa !18
  %76 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %77 = getelementptr i8, ptr addrspace(1) %76, i16 80
  store i16 5, ptr addrspace(1) %77, !tbaa !16
  %78 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %79 = getelementptr i8, ptr addrspace(1) %78, i16 80
  store i16 -5, ptr addrspace(1) %79, !tbaa !20
  %80 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %81 = getelementptr i8, ptr addrspace(1) %80, i16 80
  store i16 %33, ptr addrspace(1) %81, !tbaa !18
  %82 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %83 = getelementptr i8, ptr addrspace(1) %82, i16 90
  store i16 5, ptr addrspace(1) %83, !tbaa !16
  %84 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %85 = getelementptr i8, ptr addrspace(1) %84, i16 90
  store i16 %33, ptr addrspace(1) %85, !tbaa !20
  %86 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %87 = getelementptr i8, ptr addrspace(1) %86, i16 90
  store i16 5, ptr addrspace(1) %87, !tbaa !18
  %88 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %89 = getelementptr i8, ptr addrspace(1) %88, i16 100
  store i16 5, ptr addrspace(1) %89, !tbaa !16
  %90 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %91 = getelementptr i8, ptr addrspace(1) %90, i16 100
  store i16 %33, ptr addrspace(1) %91, !tbaa !20
  %92 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 100
  store i16 -5, ptr addrspace(1) %93, !tbaa !18
  %94 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %95 = getelementptr i8, ptr addrspace(1) %94, i16 110
  store i16 -5, ptr addrspace(1) %95, !tbaa !16
  %96 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %97 = getelementptr i8, ptr addrspace(1) %96, i16 110
  store i16 %33, ptr addrspace(1) %97, !tbaa !20
  %98 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %99 = getelementptr i8, ptr addrspace(1) %98, i16 110
  store i16 5, ptr addrspace(1) %99, !tbaa !18
  %100 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 120
  store i16 -5, ptr addrspace(1) %101, !tbaa !16
  %102 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %103 = getelementptr i8, ptr addrspace(1) %102, i16 120
  store i16 %33, ptr addrspace(1) %103, !tbaa !20
  %104 = getelementptr i8, ptr addrspace(1) %30, i16 %29
  %105 = getelementptr i8, ptr addrspace(1) %104, i16 120
  store i16 -5, ptr addrspace(1) %105, !tbaa !18
  %106 = add nsw i16 %29, 2
  %exitcond4 = icmp eq i16 %106, 0
  br i1 %exitcond4, label %b9, label %b8

b9:
  %107 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %108 = inttoptr i16 %107 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 120
  store i16 5, ptr addrspace(1) %110, !tbaa !16
  %111 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %112 = inttoptr i16 %111 to ptr addrspace(2)
  %113 = addrspacecast ptr addrspace(2) %112 to ptr addrspace(1)
  %114 = getelementptr i8, ptr addrspace(1) %113, i16 120
  store i16 5, ptr addrspace(1) %114, !tbaa !20
  %115 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %116 = inttoptr i16 %115 to ptr addrspace(2)
  %117 = addrspacecast ptr addrspace(2) %116 to ptr addrspace(1)
  %118 = getelementptr i8, ptr addrspace(1) %117, i16 120
  store i16 5, ptr addrspace(1) %118, !tbaa !18
  %119 = getelementptr i8, ptr addrspace(1) %109, i16 122
  store i16 5, ptr addrspace(1) %119, !tbaa !16
  %120 = getelementptr i8, ptr addrspace(1) %113, i16 122
  store i16 5, ptr addrspace(1) %120, !tbaa !20
  %121 = getelementptr i8, ptr addrspace(1) %117, i16 122
  store i16 5, ptr addrspace(1) %121, !tbaa !18
  %122 = getelementptr i8, ptr addrspace(1) %109, i16 124
  store i16 5, ptr addrspace(1) %122, !tbaa !16
  %123 = getelementptr i8, ptr addrspace(1) %113, i16 124
  store i16 5, ptr addrspace(1) %123, !tbaa !20
  %124 = getelementptr i8, ptr addrspace(1) %117, i16 124
  store i16 5, ptr addrspace(1) %124, !tbaa !18
  %125 = getelementptr i8, ptr addrspace(1) %109, i16 126
  store i16 5, ptr addrspace(1) %125, !tbaa !16
  %126 = getelementptr i8, ptr addrspace(1) %113, i16 126
  store i16 5, ptr addrspace(1) %126, !tbaa !20
  %127 = getelementptr i8, ptr addrspace(1) %117, i16 126
  store i16 5, ptr addrspace(1) %127, !tbaa !18
  br label %b13

b13:
  %128 = phi i16 [ %193, %b19 ], [ 128, %b9 ]
  %129 = phi i16 [ 1, %b9 ], [ %192, %b19 ]
  %130 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %131 = load float, ptr %130
  %132 = fmul float %131, 6.400000e+01
  %133 = fsub float %132, 3.200000e+01
  %134 = tail call float @llvm.rint.f32(float %133)
  %135 = fcmp olt float %133, %134
  %136 = sext i1 %135 to i16
  %137 = sitofp i16 %136 to float
  %138 = fadd float %134, %137
  %139 = tail call i16 @llvm.lrint.i16.f32(float %138)
  %140 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %141 = load float, ptr %140
  %142 = fmul float %141, 6.400000e+01
  %143 = fsub float %142, 3.200000e+01
  %144 = tail call float @llvm.rint.f32(float %143)
  %145 = fcmp olt float %143, %144
  %146 = sext i1 %145 to i16
  %147 = sitofp i16 %146 to float
  %148 = fadd float %144, %147
  %149 = tail call i16 @llvm.lrint.i16.f32(float %148)
  %150 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %151 = load float, ptr %150
  %152 = fmul float %151, 6.400000e+01
  %153 = fsub float %152, 3.200000e+01
  %154 = tail call float @llvm.rint.f32(float %153)
  %155 = fcmp olt float %153, %154
  %156 = sext i1 %155 to i16
  %157 = sitofp i16 %156 to float
  %158 = fadd float %154, %157
  %159 = tail call i16 @llvm.lrint.i16.f32(float %158)
  %160 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %161 = inttoptr i16 %160 to ptr addrspace(2)
  %162 = addrspacecast ptr addrspace(2) %161 to ptr addrspace(1)
  %163 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %164 = inttoptr i16 %163 to ptr addrspace(2)
  %165 = addrspacecast ptr addrspace(2) %164 to ptr addrspace(1)
  %166 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %167 = inttoptr i16 %166 to ptr addrspace(2)
  %168 = addrspacecast ptr addrspace(2) %167 to ptr addrspace(1)
  %169 = getelementptr i8, ptr addrspace(1) %168, i16 %128
  %170 = getelementptr i8, ptr addrspace(1) %165, i16 %128
  %171 = getelementptr i8, ptr addrspace(1) %162, i16 %128
  br label %b18

b18:
  %172 = phi i16 [ %191, %b18 ], [ 0, %b13 ]
  %173 = inttoptr i16 %166 to ptr addrspace(2)
  %174 = inttoptr i16 %163 to ptr addrspace(2)
  %175 = inttoptr i16 %160 to ptr addrspace(2)
  %176 = addrspacecast ptr addrspace(2) %173 to ptr addrspace(1)
  %177 = addrspacecast ptr addrspace(2) %174 to ptr addrspace(1)
  %178 = addrspacecast ptr addrspace(2) %175 to ptr addrspace(1)
  %179 = getelementptr i8, ptr addrspace(1) %171, i16 %172
  %180 = getelementptr i8, ptr addrspace(1) %178, i16 %172
  %181 = load i16, ptr addrspace(1) %180, !tbaa !16
  %182 = add i16 %181, %139
  store i16 %182, ptr addrspace(1) %179, !tbaa !16
  %183 = getelementptr i8, ptr addrspace(1) %170, i16 %172
  %184 = getelementptr i8, ptr addrspace(1) %177, i16 %172
  %185 = load i16, ptr addrspace(1) %184, !tbaa !20
  %186 = add i16 %185, %149
  store i16 %186, ptr addrspace(1) %183, !tbaa !20
  %187 = getelementptr i8, ptr addrspace(1) %169, i16 %172
  %188 = getelementptr i8, ptr addrspace(1) %176, i16 %172
  %189 = load i16, ptr addrspace(1) %188, !tbaa !18
  %190 = add i16 %189, %159
  store i16 %190, ptr addrspace(1) %187, !tbaa !18
  %191 = add nuw nsw i16 %172, 2
  %exitcond2 = icmp eq i16 %191, 128
  br i1 %exitcond2, label %b19, label %b18

b19:
  %192 = add nuw nsw i16 %129, 1
  %exitcond3 = icmp eq i16 %192, 64
  %193 = add nuw nsw i16 %128, 128
  br i1 %exitcond3, label %b2, label %b13

b20:
  %194 = tail call i16 @llvm.lrint.i16.f32(float 1.125000e+01)
  %195 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %196 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %197 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  br label %b22

b22:
  %198 = phi i16 [ %236, %b30 ], [ 0, %b20 ]
  %199 = phi i16 [ 90, %b20 ], [ %235, %b30 ]
  %200 = icmp sgt i16 %194, -1
  br i1 %200, label %b23, label %b24

b23:
  %201 = icmp slt i16 %199, 271
  br i1 %201, label %b25, label %b26

b24:
  %202 = icmp sgt i16 %199, 269
  br i1 %202, label %b25, label %b26

b25:
  %203 = sitofp i16 %199 to float
  %204 = fdiv float %203, 0x404CA5DC20000000
  %205 = fpround float %204 to float
  %206 = tail call float @llvm.fabs.f32(float %205)
  %207 = fcmp ogt float %206, 0x41E0000000000000
  br i1 %207, label %b27, label %b30

b26:
  %208 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %209 = inttoptr i16 %208 to ptr addrspace(2)
  %210 = addrspacecast ptr addrspace(2) %209 to ptr addrspace(1)
  %211 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %212 = inttoptr i16 %211 to ptr addrspace(2)
  %213 = addrspacecast ptr addrspace(2) %212 to ptr addrspace(1)
  %214 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %215 = inttoptr i16 %214 to ptr addrspace(2)
  %216 = addrspacecast ptr addrspace(2) %215 to ptr addrspace(1)
  %217 = getelementptr i8, ptr addrspace(1) %216, i16 32
  %218 = getelementptr i8, ptr addrspace(1) %213, i16 32
  %219 = getelementptr i8, ptr addrspace(1) %210, i16 32
  br label %b34

b27:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b30:
  %220 = inttoptr i16 %197 to ptr addrspace(2)
  %221 = inttoptr i16 %196 to ptr addrspace(2)
  %222 = inttoptr i16 %195 to ptr addrspace(2)
  %223 = addrspacecast ptr addrspace(2) %220 to ptr addrspace(1)
  %224 = addrspacecast ptr addrspace(2) %221 to ptr addrspace(1)
  %225 = addrspacecast ptr addrspace(2) %222 to ptr addrspace(1)
  %226 = getelementptr i8, ptr addrspace(1) %225, i16 %198
  %227 = tail call float @llvm.cos.f32(float %205)
  %228 = fmul float %227, 6.300000e+01
  %229 = tail call i16 @llvm.lrint.i16.f32(float %228)
  store i16 %229, ptr addrspace(1) %226, !tbaa !16
  %230 = getelementptr i8, ptr addrspace(1) %224, i16 %198
  %231 = tail call float @llvm.sin.f32(float %205)
  %232 = fmul float %231, 6.300000e+01
  %233 = tail call i16 @llvm.lrint.i16.f32(float %232)
  store i16 %233, ptr addrspace(1) %230, !tbaa !20
  %234 = getelementptr i8, ptr addrspace(1) %223, i16 %198
  store i16 0, ptr addrspace(1) %234, !tbaa !18
  %235 = add nsw i16 %199, %194
  %236 = add i16 %198, 2
  br label %b22

b34:
  %237 = phi ptr addrspace(1) [ %284, %b44 ], [ %219, %b26 ]
  %238 = phi ptr addrspace(1) [ %283, %b44 ], [ %218, %b26 ]
  %239 = phi ptr addrspace(1) [ %282, %b44 ], [ %217, %b26 ]
  %240 = phi i16 [ 1, %b26 ], [ %281, %b44 ]
  %241 = phi float [ 0.000000e+00, %b26 ], [ %243, %b44 ]
  %242 = fadd float %241, 2.250000e+01
  %243 = fpround float %242 to float
  %244 = fdiv float %243, 0x404CA5DC20000000
  %245 = fpround float %244 to float
  %246 = tail call float @llvm.fabs.f32(float %245)
  %247 = fcmp ogt float %246, 0x41E0000000000000
  br i1 %247, label %b36, label %b39

b36:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b39:
  %248 = tail call float @llvm.cos.f32(float %245)
  %249 = fpround float %248 to float
  %250 = tail call float @llvm.sin.f32(float %245)
  %251 = fpround float %250 to float
  br label %b43

b43:
  %252 = phi i16 [ %280, %b43 ], [ 0, %b39 ]
  %253 = inttoptr i16 %214 to ptr addrspace(2)
  %254 = inttoptr i16 %211 to ptr addrspace(2)
  %255 = inttoptr i16 %208 to ptr addrspace(2)
  %256 = addrspacecast ptr addrspace(2) %253 to ptr addrspace(1)
  %257 = addrspacecast ptr addrspace(2) %254 to ptr addrspace(1)
  %258 = addrspacecast ptr addrspace(2) %255 to ptr addrspace(1)
  %259 = getelementptr i8, ptr addrspace(1) %237, i16 %252
  %260 = getelementptr i8, ptr addrspace(1) %258, i16 %252
  %261 = load i16, ptr addrspace(1) %260, !tbaa !16
  %262 = sitofp i16 %261 to float
  %263 = fmul float %249, %262
  %264 = getelementptr i8, ptr addrspace(1) %257, i16 %252
  %265 = load i16, ptr addrspace(1) %264, !tbaa !18
  %266 = sitofp i16 %265 to float
  %267 = fmul float %251, %266
  %268 = fsub float %263, %267
  %269 = tail call i16 @llvm.lrint.i16.f32(float %268)
  store i16 %269, ptr addrspace(1) %259, !tbaa !16
  %270 = getelementptr i8, ptr addrspace(1) %238, i16 %252
  %271 = load i16, ptr addrspace(1) %260, !tbaa !16
  %272 = sitofp i16 %271 to float
  %273 = fmul float %251, %272
  %274 = fmul float %249, %266
  %275 = fadd float %274, %273
  %276 = tail call i16 @llvm.lrint.i16.f32(float %275)
  store i16 %276, ptr addrspace(1) %270, !tbaa !18
  %277 = getelementptr i8, ptr addrspace(1) %239, i16 %252
  %278 = getelementptr i8, ptr addrspace(1) %256, i16 %252
  %279 = load i16, ptr addrspace(1) %278, !tbaa !20
  store i16 %279, ptr addrspace(1) %277, !tbaa !20
  %280 = add nuw nsw i16 %252, 2
  %exitcond7 = icmp eq i16 %280, 32
  br i1 %exitcond7, label %b44, label %b43

b44:
  %281 = add nuw nsw i16 %240, 1
  %exitcond8 = icmp eq i16 %281, 16
  %282 = getelementptr i8, ptr addrspace(1) %239, i16 32
  %283 = getelementptr i8, ptr addrspace(1) %238, i16 32
  %284 = getelementptr i8, ptr addrspace(1) %237, i16 32
  br i1 %exitcond8, label %285, label %b34

285:
  br label %b48

b48:
  %286 = phi i16 [ 512, %285 ], [ %351, %b54 ]
  %287 = phi i16 [ %350, %b54 ], [ 1, %285 ]
  %288 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %289 = load float, ptr %288
  %290 = fmul float %289, 5.120000e+02
  %291 = fsub float %290, 2.560000e+02
  %292 = tail call float @llvm.rint.f32(float %291)
  %293 = fcmp olt float %291, %292
  %294 = sext i1 %293 to i16
  %295 = sitofp i16 %294 to float
  %296 = fadd float %292, %295
  %297 = tail call i16 @llvm.lrint.i16.f32(float %296)
  %298 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %299 = load float, ptr %298
  %300 = fmul float %299, 5.120000e+02
  %301 = fsub float %300, 2.560000e+02
  %302 = tail call float @llvm.rint.f32(float %301)
  %303 = fcmp olt float %301, %302
  %304 = sext i1 %303 to i16
  %305 = sitofp i16 %304 to float
  %306 = fadd float %302, %305
  %307 = tail call i16 @llvm.lrint.i16.f32(float %306)
  %308 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %309 = load float, ptr %308
  %310 = fmul float %309, 5.120000e+02
  %311 = fsub float %310, 2.560000e+02
  %312 = tail call float @llvm.rint.f32(float %311)
  %313 = fcmp olt float %311, %312
  %314 = sext i1 %313 to i16
  %315 = sitofp i16 %314 to float
  %316 = fadd float %312, %315
  %317 = tail call i16 @llvm.lrint.i16.f32(float %316)
  %318 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %319 = inttoptr i16 %318 to ptr addrspace(2)
  %320 = addrspacecast ptr addrspace(2) %319 to ptr addrspace(1)
  %321 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %322 = inttoptr i16 %321 to ptr addrspace(2)
  %323 = addrspacecast ptr addrspace(2) %322 to ptr addrspace(1)
  %324 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %325 = inttoptr i16 %324 to ptr addrspace(2)
  %326 = addrspacecast ptr addrspace(2) %325 to ptr addrspace(1)
  %327 = getelementptr i8, ptr addrspace(1) %326, i16 %286
  %328 = getelementptr i8, ptr addrspace(1) %323, i16 %286
  %329 = getelementptr i8, ptr addrspace(1) %320, i16 %286
  br label %b53

b53:
  %330 = phi i16 [ %349, %b53 ], [ 0, %b48 ]
  %331 = inttoptr i16 %324 to ptr addrspace(2)
  %332 = inttoptr i16 %321 to ptr addrspace(2)
  %333 = inttoptr i16 %318 to ptr addrspace(2)
  %334 = addrspacecast ptr addrspace(2) %331 to ptr addrspace(1)
  %335 = addrspacecast ptr addrspace(2) %332 to ptr addrspace(1)
  %336 = addrspacecast ptr addrspace(2) %333 to ptr addrspace(1)
  %337 = getelementptr i8, ptr addrspace(1) %329, i16 %330
  %338 = getelementptr i8, ptr addrspace(1) %336, i16 %330
  %339 = load i16, ptr addrspace(1) %338, !tbaa !16
  %340 = add i16 %339, %297
  store i16 %340, ptr addrspace(1) %337, !tbaa !16
  %341 = getelementptr i8, ptr addrspace(1) %328, i16 %330
  %342 = getelementptr i8, ptr addrspace(1) %335, i16 %330
  %343 = load i16, ptr addrspace(1) %342, !tbaa !20
  %344 = add i16 %343, %307
  store i16 %344, ptr addrspace(1) %341, !tbaa !20
  %345 = getelementptr i8, ptr addrspace(1) %327, i16 %330
  %346 = getelementptr i8, ptr addrspace(1) %334, i16 %330
  %347 = load i16, ptr addrspace(1) %346, !tbaa !18
  %348 = add i16 %347, %317
  store i16 %348, ptr addrspace(1) %345, !tbaa !18
  %349 = add nuw nsw i16 %330, 2
  %exitcond5 = icmp eq i16 %349, 512
  br i1 %exitcond5, label %b54, label %b53

b54:
  %350 = add nuw nsw i16 %287, 1
  %exitcond6 = icmp eq i16 %350, 16
  %351 = add nuw nsw i16 %286, 512
  br i1 %exitcond6, label %b2, label %b48

b55:
  %352 = tail call i16 @llvm.lrint.i16.f32(float 2.250000e+01)
  %353 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %354 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %355 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  br label %b57

b57:
  %356 = phi i16 [ %386, %b65 ], [ 0, %b55 ]
  %357 = phi i16 [ 0, %b55 ], [ %385, %b65 ]
  %358 = icmp sgt i16 %352, -1
  br i1 %358, label %b58, label %b59

b58:
  %359 = icmp slt i16 %357, 360
  br i1 %359, label %b60, label %b61

b59:
  %360 = icmp sgt i16 %357, 358
  br i1 %360, label %b60, label %b61

b60:
  %361 = sitofp i16 %357 to float
  %362 = fdiv float %361, 0x404CA5DC20000000
  %363 = fpround float %362 to float
  %364 = tail call float @llvm.fabs.f32(float %363)
  %365 = fcmp ogt float %364, 0x41E0000000000000
  br i1 %365, label %b62, label %b65

b61:
  %366 = fdiv float 9.000000e+01, 0x404CA5DC20000000
  %367 = fpround float %366 to float
  %368 = tail call float @llvm.fabs.f32(float %367)
  %369 = fcmp ogt float %368, 0x41E0000000000000
  br i1 %369, label %b66, label %b69

b62:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b65:
  %370 = inttoptr i16 %355 to ptr addrspace(2)
  %371 = inttoptr i16 %354 to ptr addrspace(2)
  %372 = inttoptr i16 %353 to ptr addrspace(2)
  %373 = addrspacecast ptr addrspace(2) %370 to ptr addrspace(1)
  %374 = addrspacecast ptr addrspace(2) %371 to ptr addrspace(1)
  %375 = addrspacecast ptr addrspace(2) %372 to ptr addrspace(1)
  %376 = getelementptr i8, ptr addrspace(1) %375, i16 %356
  %377 = tail call float @llvm.cos.f32(float %363)
  %378 = fmul float %377, 6.400000e+01
  %379 = tail call i16 @llvm.lrint.i16.f32(float %378)
  store i16 %379, ptr addrspace(1) %376, !tbaa !16
  %380 = getelementptr i8, ptr addrspace(1) %374, i16 %356
  %381 = tail call float @llvm.sin.f32(float %363)
  %382 = fmul float %381, 6.400000e+01
  %383 = tail call i16 @llvm.lrint.i16.f32(float %382)
  store i16 %383, ptr addrspace(1) %380, !tbaa !20
  %384 = getelementptr i8, ptr addrspace(1) %373, i16 %356
  store i16 0, ptr addrspace(1) %384, !tbaa !18
  %385 = add nsw i16 %357, %352
  %386 = add i16 %356, 2
  br label %b57

b66:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b69:
  %387 = tail call float @llvm.cos.f32(float %367)
  %388 = fpround float %387 to float
  %389 = tail call float @llvm.sin.f32(float %367)
  %390 = fpround float %389 to float
  %391 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %392 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %393 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  br label %b73

b73:
  %394 = phi i16 [ %422, %b73 ], [ -32, %b69 ]
  %395 = inttoptr i16 %393 to ptr addrspace(2)
  %396 = inttoptr i16 %392 to ptr addrspace(2)
  %397 = inttoptr i16 %391 to ptr addrspace(2)
  %398 = addrspacecast ptr addrspace(2) %395 to ptr addrspace(1)
  %399 = addrspacecast ptr addrspace(2) %396 to ptr addrspace(1)
  %400 = addrspacecast ptr addrspace(2) %397 to ptr addrspace(1)
  %401 = getelementptr i8, ptr addrspace(1) %400, i16 %394
  %402 = getelementptr i8, ptr addrspace(1) %401, i16 32
  %403 = load i16, ptr addrspace(1) %402, !tbaa !16
  %404 = sitofp i16 %403 to float
  %405 = fpround float %404 to float
  %406 = fmul float %388, %405
  %407 = getelementptr i8, ptr addrspace(1) %399, i16 %394
  %408 = getelementptr i8, ptr addrspace(1) %407, i16 32
  %409 = load i16, ptr addrspace(1) %408, !tbaa !18
  %410 = sitofp i16 %409 to float
  %411 = fmul float %390, %410
  %412 = fsub float %406, %411
  %413 = tail call i16 @llvm.lrint.i16.f32(float %412)
  store i16 %413, ptr addrspace(1) %402, !tbaa !16
  %414 = fmul float %390, %405
  %415 = fmul float %388, %410
  %416 = fadd float %414, %415
  %417 = tail call i16 @llvm.lrint.i16.f32(float %416)
  store i16 %417, ptr addrspace(1) %408, !tbaa !18
  %418 = getelementptr i8, ptr addrspace(1) %398, i16 %394
  %419 = getelementptr i8, ptr addrspace(1) %418, i16 32
  %420 = load i16, ptr addrspace(1) %419, !tbaa !20
  %421 = add i16 %420, -192
  store i16 %421, ptr addrspace(1) %419, !tbaa !20
  %422 = add nsw i16 %394, 2
  %exitcond13 = icmp eq i16 %422, 0
  br i1 %exitcond13, label %b74, label %b73

b74:
  store float %405, ptr @"XP!", align 1, !tbaa !2
  %423 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %424 = inttoptr i16 %423 to ptr addrspace(2)
  %425 = addrspacecast ptr addrspace(2) %424 to ptr addrspace(1)
  %426 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %427 = inttoptr i16 %426 to ptr addrspace(2)
  %428 = addrspacecast ptr addrspace(2) %427 to ptr addrspace(1)
  %429 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %430 = inttoptr i16 %429 to ptr addrspace(2)
  %431 = addrspacecast ptr addrspace(2) %430 to ptr addrspace(1)
  %432 = getelementptr i8, ptr addrspace(1) %431, i16 32
  %433 = getelementptr i8, ptr addrspace(1) %428, i16 32
  %434 = getelementptr i8, ptr addrspace(1) %425, i16 32
  br label %b78

b78:
  %435 = phi ptr addrspace(1) [ %482, %b88 ], [ %434, %b74 ]
  %436 = phi ptr addrspace(1) [ %481, %b88 ], [ %433, %b74 ]
  %437 = phi ptr addrspace(1) [ %480, %b88 ], [ %432, %b74 ]
  %438 = phi float [ 0.000000e+00, %b74 ], [ %441, %b88 ]
  %439 = phi i16 [ 1, %b74 ], [ %479, %b88 ]
  %440 = fadd float %438, 1.125000e+01
  %441 = fpround float %440 to float
  %442 = fdiv float %441, 0x404CA5DC20000000
  %443 = fpround float %442 to float
  %444 = tail call float @llvm.fabs.f32(float %443)
  %445 = fcmp ogt float %444, 0x41E0000000000000
  br i1 %445, label %b80, label %b83

b80:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b83:
  %446 = tail call float @llvm.cos.f32(float %443)
  %447 = fpround float %446 to float
  %448 = tail call float @llvm.sin.f32(float %443)
  %449 = fpround float %448 to float
  br label %b87

b87:
  %450 = phi i16 [ %478, %b87 ], [ 0, %b83 ]
  %451 = inttoptr i16 %429 to ptr addrspace(2)
  %452 = inttoptr i16 %426 to ptr addrspace(2)
  %453 = inttoptr i16 %423 to ptr addrspace(2)
  %454 = addrspacecast ptr addrspace(2) %451 to ptr addrspace(1)
  %455 = addrspacecast ptr addrspace(2) %452 to ptr addrspace(1)
  %456 = addrspacecast ptr addrspace(2) %453 to ptr addrspace(1)
  %457 = getelementptr i8, ptr addrspace(1) %435, i16 %450
  %458 = getelementptr i8, ptr addrspace(1) %456, i16 %450
  %459 = load i16, ptr addrspace(1) %458, !tbaa !16
  %460 = sitofp i16 %459 to float
  %461 = fmul float %447, %460
  %462 = getelementptr i8, ptr addrspace(1) %455, i16 %450
  %463 = load i16, ptr addrspace(1) %462, !tbaa !20
  %464 = sitofp i16 %463 to float
  %465 = fmul float %449, %464
  %466 = fsub float %461, %465
  %467 = tail call i16 @llvm.lrint.i16.f32(float %466)
  store i16 %467, ptr addrspace(1) %457, !tbaa !16
  %468 = getelementptr i8, ptr addrspace(1) %436, i16 %450
  %469 = load i16, ptr addrspace(1) %458, !tbaa !16
  %470 = sitofp i16 %469 to float
  %471 = fmul float %449, %470
  %472 = fmul float %447, %464
  %473 = fadd float %472, %471
  %474 = tail call i16 @llvm.lrint.i16.f32(float %473)
  store i16 %474, ptr addrspace(1) %468, !tbaa !20
  %475 = getelementptr i8, ptr addrspace(1) %437, i16 %450
  %476 = getelementptr i8, ptr addrspace(1) %454, i16 %450
  %477 = load i16, ptr addrspace(1) %476, !tbaa !18
  store i16 %477, ptr addrspace(1) %475, !tbaa !18
  %478 = add nuw nsw i16 %450, 2
  %exitcond11 = icmp eq i16 %478, 32
  br i1 %exitcond11, label %b88, label %b87

b88:
  %479 = add nuw nsw i16 %439, 1
  %exitcond12 = icmp eq i16 %479, 32
  %480 = getelementptr i8, ptr addrspace(1) %437, i16 32
  %481 = getelementptr i8, ptr addrspace(1) %436, i16 32
  %482 = getelementptr i8, ptr addrspace(1) %435, i16 32
  br i1 %exitcond12, label %483, label %b78

483:
  br label %b92

b92:
  %484 = phi i16 [ 1024, %483 ], [ %648, %b110 ]
  %485 = phi i16 [ %647, %b110 ], [ 1, %483 ]
  %486 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %487 = load float, ptr %486
  %488 = fmul float %487, 1.800000e+02
  %489 = tail call float @llvm.rint.f32(float %488)
  %490 = fcmp olt float %488, %489
  %491 = sext i1 %490 to i16
  %492 = sitofp i16 %491 to float
  %493 = fadd float %489, %492
  %494 = fsub float %493, 9.000000e+01
  %495 = fdiv float %494, 0x404CA5DC20000000
  %496 = fpround float %495 to float
  %497 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %498 = load float, ptr %497
  %499 = fmul float %498, 1.800000e+02
  %500 = tail call float @llvm.rint.f32(float %499)
  %501 = fcmp olt float %499, %500
  %502 = sext i1 %501 to i16
  %503 = sitofp i16 %502 to float
  %504 = fadd float %500, %503
  %505 = fsub float %504, 9.000000e+01
  %506 = fdiv float %505, 0x404CA5DC20000000
  %507 = fpround float %506 to float
  %508 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %509 = load float, ptr %508
  %510 = fmul float %509, 1.800000e+02
  %511 = tail call float @llvm.rint.f32(float %510)
  %512 = fcmp olt float %510, %511
  %513 = sext i1 %512 to i16
  %514 = sitofp i16 %513 to float
  %515 = fadd float %511, %514
  %516 = fsub float %515, 9.000000e+01
  %517 = fdiv float %516, 0x404CA5DC20000000
  %518 = fpround float %517 to float
  %519 = tail call float @llvm.fabs.f32(float %496)
  %520 = fcmp ogt float %519, 0x41E0000000000000
  br i1 %520, label %b94, label %b95

b94:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b95:
  %521 = tail call float @llvm.cos.f32(float %496)
  %522 = fpround float %521 to float
  %523 = tail call float @llvm.fabs.f32(float %507)
  %524 = fcmp ogt float %523, 0x41E0000000000000
  br i1 %524, label %b96, label %b97

b96:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b97:
  %525 = tail call float @llvm.cos.f32(float %507)
  %526 = fpround float %525 to float
  %527 = tail call float @llvm.fabs.f32(float %518)
  %528 = fcmp ogt float %527, 0x41E0000000000000
  br i1 %528, label %b98, label %b105

b98:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b105:
  %529 = tail call float @llvm.cos.f32(float %518)
  %530 = fpround float %529 to float
  %531 = tail call float @llvm.sin.f32(float %496)
  %532 = fpround float %531 to float
  %533 = tail call float @llvm.sin.f32(float %507)
  %534 = fpround float %533 to float
  %535 = tail call float @llvm.sin.f32(float %518)
  %536 = fpround float %535 to float
  %537 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %538 = load float, ptr %537
  %539 = fmul float %538, 1.024000e+03
  %540 = fsub float %539, 5.120000e+02
  %541 = tail call float @llvm.rint.f32(float %540)
  %542 = fcmp olt float %540, %541
  %543 = sext i1 %542 to i16
  %544 = sitofp i16 %543 to float
  %545 = fadd float %541, %544
  %546 = tail call i16 @llvm.lrint.i16.f32(float %545)
  %547 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %548 = load float, ptr %547
  %549 = fmul float %548, 1.024000e+03
  %550 = fsub float %549, 5.120000e+02
  %551 = tail call float @llvm.rint.f32(float %550)
  %552 = fcmp olt float %550, %551
  %553 = sext i1 %552 to i16
  %554 = sitofp i16 %553 to float
  %555 = fadd float %551, %554
  %556 = tail call i16 @llvm.lrint.i16.f32(float %555)
  %557 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %558 = load float, ptr %557
  %559 = fmul float %558, 1.024000e+03
  %560 = fsub float %559, 5.120000e+02
  %561 = tail call float @llvm.rint.f32(float %560)
  %562 = fcmp olt float %560, %561
  %563 = sext i1 %562 to i16
  %564 = sitofp i16 %563 to float
  %565 = fadd float %561, %564
  %566 = tail call i16 @llvm.lrint.i16.f32(float %565)
  %567 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"X!", i16 2), !tbaa !2
  %568 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %569 = inttoptr i16 %568 to ptr addrspace(2)
  %570 = addrspacecast ptr addrspace(2) %569 to ptr addrspace(1)
  %571 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %572 = inttoptr i16 %571 to ptr addrspace(2)
  %573 = addrspacecast ptr addrspace(2) %572 to ptr addrspace(1)
  %574 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Z!", i16 2), !tbaa !2
  %575 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Y!", i16 2), !tbaa !2
  %576 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %577 = inttoptr i16 %576 to ptr addrspace(2)
  %578 = addrspacecast ptr addrspace(2) %577 to ptr addrspace(1)
  %579 = sitofp i16 %546 to float
  %580 = sitofp i16 %556 to float
  %581 = sitofp i16 %566 to float
  %582 = getelementptr i8, ptr addrspace(1) %573, i16 %484
  %583 = getelementptr i8, ptr addrspace(1) %578, i16 %484
  %584 = getelementptr i8, ptr addrspace(1) %570, i16 %484
  br label %b109

b109:
  %585 = phi i16 [ %646, %b109 ], [ 0, %b105 ]
  %586 = inttoptr i16 %576 to ptr addrspace(2)
  %587 = inttoptr i16 %575 to ptr addrspace(2)
  %588 = inttoptr i16 %574 to ptr addrspace(2)
  %589 = inttoptr i16 %571 to ptr addrspace(2)
  %590 = inttoptr i16 %568 to ptr addrspace(2)
  %591 = inttoptr i16 %567 to ptr addrspace(2)
  %592 = addrspacecast ptr addrspace(2) %586 to ptr addrspace(1)
  %593 = addrspacecast ptr addrspace(2) %587 to ptr addrspace(1)
  %594 = addrspacecast ptr addrspace(2) %588 to ptr addrspace(1)
  %595 = addrspacecast ptr addrspace(2) %589 to ptr addrspace(1)
  %596 = addrspacecast ptr addrspace(2) %590 to ptr addrspace(1)
  %597 = addrspacecast ptr addrspace(2) %591 to ptr addrspace(1)
  %598 = shl nuw nsw i16 %585, 1
  %599 = getelementptr i8, ptr addrspace(1) %597, i16 %598
  %600 = getelementptr i8, ptr addrspace(1) %596, i16 %585
  %601 = load i16, ptr addrspace(1) %600, !tbaa !16
  %602 = sitofp i16 %601 to float
  %603 = fmul float %526, %602
  %604 = getelementptr i8, ptr addrspace(1) %595, i16 %585
  %605 = load i16, ptr addrspace(1) %604, !tbaa !18
  %606 = sitofp i16 %605 to float
  %607 = fmul float %534, %606
  %608 = fsub float %603, %607
  %609 = fpround float %608 to float
  %610 = shl nuw nsw i16 %585, 1
  %611 = getelementptr i8, ptr addrspace(1) %594, i16 %610
  %612 = fmul float %534, %602
  %613 = fmul float %526, %606
  %614 = fadd float %612, %613
  %615 = fpround float %614 to float
  %616 = shl nuw nsw i16 %585, 1
  %617 = getelementptr i8, ptr addrspace(1) %593, i16 %616
  %618 = getelementptr i8, ptr addrspace(1) %592, i16 %585
  %619 = load i16, ptr addrspace(1) %618, !tbaa !20
  %620 = sitofp i16 %619 to float
  %621 = fmul float %522, %620
  %622 = fmul float %532, %615
  %623 = fsub float %621, %622
  %624 = fpround float %623 to float
  %625 = fmul float %532, %620
  %626 = fmul float %522, %615
  %627 = fadd float %626, %625
  %628 = fpround float %627 to float
  store float %628, ptr addrspace(1) %611, !tbaa !56
  %629 = fmul float %530, %609
  %630 = fmul float %536, %624
  %631 = fsub float %629, %630
  %632 = fpround float %631 to float
  store float %632, ptr addrspace(1) %599, !tbaa !54
  %633 = fmul float %536, %609
  %634 = fmul float %530, %624
  %635 = fadd float %633, %634
  %636 = fpround float %635 to float
  store float %636, ptr addrspace(1) %617, !tbaa !58
  %637 = getelementptr i8, ptr addrspace(1) %584, i16 %585
  %638 = fadd float %579, %632
  %639 = tail call i16 @llvm.lrint.i16.f32(float %638)
  store i16 %639, ptr addrspace(1) %637, !tbaa !16
  %640 = getelementptr i8, ptr addrspace(1) %583, i16 %585
  %641 = fadd float %580, %636
  %642 = tail call i16 @llvm.lrint.i16.f32(float %641)
  store i16 %642, ptr addrspace(1) %640, !tbaa !20
  %643 = getelementptr i8, ptr addrspace(1) %582, i16 %585
  %644 = fadd float %581, %628
  %645 = tail call i16 @llvm.lrint.i16.f32(float %644)
  store i16 %645, ptr addrspace(1) %643, !tbaa !18
  %646 = add nuw nsw i16 %585, 2
  %exitcond9 = icmp eq i16 %646, 1024
  br i1 %exitcond9, label %b110, label %b109

b110:
  %647 = add nuw nsw i16 %485, 1
  %exitcond10 = icmp eq i16 %647, 8
  %648 = add nuw nsw i16 %484, 1024
  br i1 %exitcond10, label %b2, label %b92
}

define cc 1000 void @CROSFADE(ptr dereferenceable(18) nocapture readonly %0, ptr dereferenceable(18) nocapture readonly %1, ptr dereferenceable(18) nocapture readonly %2, ptr dereferenceable(18) nocapture readonly %3, ptr dereferenceable(18) nocapture readonly %4, ptr dereferenceable(18) nocapture readonly %5) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree norecurse {
b1:
  br label %b6

b2:
  ret void

b6:
  %6 = phi i16 [ 0, %b1 ], [ %36, %b19 ]
  %7 = load i16, ptr @"XIT%", !tbaa !2
  %8 = icmp eq i16 %7, 1
  br i1 %8, label %b2, label %9

9:
  %10 = icmp eq i16 %7, 0
  br i1 %10, label %11, label %b13

11:
  %12 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %13 = icmp eq i8 %12, 1
  br i1 %13, label %b10.thread, label %b13

b10.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b2

b13:
  tail call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %14 = getelementptr i8, ptr %0, i16 2
  %15 = load i16, ptr %14
  %16 = getelementptr i8, ptr %0, i16 10
  %17 = load i16, ptr %16
  br label %b18

b18:
  %18 = phi i16 [ %114, %b34 ], [ 0, %b13 ]
  %19 = phi i16 [ %17, %b13 ], [ %87, %b34 ]
  %20 = phi i16 [ %15, %b13 ], [ %85, %b34 ]
  %21 = inttoptr i16 %20 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 %19
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 %18
  %25 = load i16, ptr addrspace(1) %24
  %26 = getelementptr i8, ptr %1, i16 2
  %27 = load i16, ptr %26
  %28 = getelementptr i8, ptr %1, i16 10
  %29 = load i16, ptr %28
  %30 = inttoptr i16 %27 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 %18
  %34 = load i16, ptr addrspace(1) %33
  %35 = icmp slt i16 %25, %34
  br i1 %35, label %38, label %b21

b19:
  %36 = add nuw nsw i16 %6, 1
  %exitcond1 = icmp eq i16 %36, 64
  br i1 %exitcond1, label %b2, label %b6

b21:
  %37 = icmp sgt i16 %25, %34
  br i1 %37, label %38, label %b22

38:
  %sink1 = phi i16 [ -1, %b21 ], [ 1, %b18 ]
  %39 = add nsw i16 %25, %sink1
  store i16 %39, ptr addrspace(1) %24
  br label %b22

b22:
  %40 = getelementptr i8, ptr %2, i16 2
  %41 = load i16, ptr %40
  %42 = getelementptr i8, ptr %2, i16 10
  %43 = load i16, ptr %42
  %44 = inttoptr i16 %41 to ptr addrspace(2)
  %45 = addrspacecast ptr addrspace(2) %44 to ptr addrspace(1)
  %46 = getelementptr i8, ptr addrspace(1) %45, i16 %43
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %18
  %48 = load i16, ptr addrspace(1) %47
  %49 = getelementptr i8, ptr %3, i16 2
  %50 = load i16, ptr %49
  %51 = getelementptr i8, ptr %3, i16 10
  %52 = load i16, ptr %51
  %53 = inttoptr i16 %50 to ptr addrspace(2)
  %54 = addrspacecast ptr addrspace(2) %53 to ptr addrspace(1)
  %55 = getelementptr i8, ptr addrspace(1) %54, i16 %52
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 %18
  %57 = load i16, ptr addrspace(1) %56
  %58 = icmp slt i16 %48, %57
  br i1 %58, label %60, label %b27

b27:
  %59 = icmp sgt i16 %48, %57
  br i1 %59, label %60, label %b28

60:
  %sink3 = phi i16 [ -1, %b27 ], [ 1, %b22 ]
  %61 = add nsw i16 %48, %sink3
  store i16 %61, ptr addrspace(1) %47
  br label %b28

b28:
  %62 = getelementptr i8, ptr %4, i16 2
  %63 = load i16, ptr %62
  %64 = getelementptr i8, ptr %4, i16 10
  %65 = load i16, ptr %64
  %66 = inttoptr i16 %63 to ptr addrspace(2)
  %67 = addrspacecast ptr addrspace(2) %66 to ptr addrspace(1)
  %68 = getelementptr i8, ptr addrspace(1) %67, i16 %65
  %69 = getelementptr i8, ptr addrspace(1) %68, i16 %18
  %70 = load i16, ptr addrspace(1) %69
  %71 = getelementptr i8, ptr %5, i16 2
  %72 = load i16, ptr %71
  %73 = getelementptr i8, ptr %5, i16 10
  %74 = load i16, ptr %73
  %75 = inttoptr i16 %72 to ptr addrspace(2)
  %76 = addrspacecast ptr addrspace(2) %75 to ptr addrspace(1)
  %77 = getelementptr i8, ptr addrspace(1) %76, i16 %74
  %78 = getelementptr i8, ptr addrspace(1) %77, i16 %18
  %79 = load i16, ptr addrspace(1) %78
  %80 = icmp slt i16 %70, %79
  br i1 %80, label %82, label %b33

b33:
  %81 = icmp sgt i16 %70, %79
  br i1 %81, label %82, label %b34

82:
  %sink5 = phi i16 [ -1, %b33 ], [ 1, %b28 ]
  %83 = add nsw i16 %70, %sink5
  store i16 %83, ptr addrspace(1) %69
  br label %b34

b34:
  %84 = getelementptr i8, ptr %0, i16 2
  %85 = load i16, ptr %84
  %86 = getelementptr i8, ptr %0, i16 10
  %87 = load i16, ptr %86
  %88 = inttoptr i16 %85 to ptr addrspace(2)
  %89 = addrspacecast ptr addrspace(2) %88 to ptr addrspace(1)
  %90 = getelementptr i8, ptr addrspace(1) %89, i16 %87
  %91 = getelementptr i8, ptr addrspace(1) %90, i16 %18
  %92 = load i16, ptr addrspace(1) %91
  %93 = trunc i16 %92 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %93) memory(inaccessiblemem: readwrite)
  %94 = getelementptr i8, ptr %2, i16 2
  %95 = load i16, ptr %94
  %96 = getelementptr i8, ptr %2, i16 10
  %97 = load i16, ptr %96
  %98 = inttoptr i16 %95 to ptr addrspace(2)
  %99 = addrspacecast ptr addrspace(2) %98 to ptr addrspace(1)
  %100 = getelementptr i8, ptr addrspace(1) %99, i16 %97
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 %18
  %102 = load i16, ptr addrspace(1) %101
  %103 = trunc i16 %102 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %103) memory(inaccessiblemem: readwrite)
  %104 = getelementptr i8, ptr %4, i16 2
  %105 = load i16, ptr %104
  %106 = getelementptr i8, ptr %4, i16 10
  %107 = load i16, ptr %106
  %108 = inttoptr i16 %105 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 %107
  %111 = getelementptr i8, ptr addrspace(1) %110, i16 %18
  %112 = load i16, ptr addrspace(1) %111
  %113 = trunc i16 %112 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %113) memory(inaccessiblemem: readwrite)
  %114 = add nuw nsw i16 %18, 2
  %exitcond = icmp eq i16 %114, 512
  br i1 %exitcond, label %b19, label %b18
}

define cc 1000 void @CYCLEBLOBS() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  br label %b6

b2:
  ret void

b6:
  %0 = phi i16 [ 0, %b1 ], [ %1, %b6 ]
  %tmp1 = trunc i16 %0 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp1) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %1 = add nuw nsw i16 %0, 1
  %exitcond10 = icmp eq i16 %1, 128
  br i1 %exitcond10, label %b7, label %b6

b7:
  %2 = tail call i32 @llvm.lrint.i32.f32(float 1.500000e+00)
  %3 = sitofp i32 %2 to float
  br label %b11

b11:
  %4 = phi i16 [ 0, %b7 ], [ %24, %b17 ]
  %5 = phi i16 [ 0, %b7 ], [ %23, %b17 ]
  %6 = sub nuw nsw i16 100, %5
  %7 = uitofp nneg i16 %6 to float
  %8 = fpround float %7 to float
  %9 = fmul float %8, %8
  %10 = mul i16 %4, -1
  %11 = shl i16 %4, 1
  %12 = getelementptr i8, ptr @"SP%", i16 %11
  br label %b16

b16:
  %13 = phi i16 [ %51, %b34 ], [ 160, %b11 ]
  %14 = phi ptr [ %50, %b34 ], [ %12, %b11 ]
  %15 = phi i16 [ %49, %b34 ], [ %10, %b11 ]
  %16 = uitofp nneg i16 %13 to float
  %17 = fpround float %16 to float
  %18 = fmul float %17, %17
  %19 = fadd float %9, %18
  %20 = tail call float @llvm.sqrt.f32(float %19)
  %21 = fpround float %20 to float
  %22 = fcmp ogt float %21, 0.000000e+00
  br i1 %22, label %b18, label %b19

b17:
  %23 = add nuw nsw i16 %5, 1
  %exitcond9 = icmp eq i16 %23, 100
  %24 = mul i16 %49, -1
  br i1 %exitcond9, label %25, label %b11

25:
  br label %b41

b18:
  %26 = tail call float @llvm.log2.f32(float %21)
  %27 = fmul float %26, 1.500000e+00
  %28 = fcmp oge float %27, 3.276750e+04
  br i1 %28, label %b27, label %b28

b19:
  %29 = fcmp oeq float %21, 0.000000e+00
  br i1 %29, label %b26, label %b22

b22:
  %30 = fcmp oeq float %3, 1.500000e+00
  br i1 %30, label %b23, label %b25

b23:
  %31 = fneg float %21
  %32 = tail call float @llvm.log2.f32(float %31)
  %33 = fmul float %32, 1.500000e+00
  %34 = fcmp oge float %33, 3.276750e+04
  br i1 %34, label %b31, label %b32

b24:
  %35 = fneg float %45
  %36 = fpround float %35 to float
  br label %b26

b25:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b26:
  %37 = phi float [ %41, %b28 ], [ %45, %b32 ], [ %36, %b24 ], [ 0.000000e+00, %b19 ]
  %38 = tail call i32 @llvm.lrint.i32.f32(float %37)
  %39 = icmp eq i32 %38, 0
  br i1 %39, label %b33, label %b34

b27:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b28:
  %40 = tail call float @llvm.exp2.f32(float %27)
  %41 = fpround float %40 to float
  br label %b26

b31:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b32:
  %42 = and i32 %2, 1
  %43 = icmp eq i32 %42, 0
  %44 = tail call float @llvm.exp2.f32(float %33)
  %45 = fpround float %44 to float
  br i1 %43, label %b26, label %b24

b33:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b34:
  %46 = sdiv i32 16384, %38
  %47 = trunc nsw i32 %46 to i16
  %48 = icmp sgt i32 %46, 255
  %spec.store.select3 = select i1 %48, i16 255, i16 %47
  store i16 %spec.store.select3, ptr %14
  %49 = add i16 %15, -1
  %50 = getelementptr i8, ptr %14, i16 2
  %51 = add nsw i16 %13, -1
  %exitcond8 = icmp eq i16 %51, 0
  br i1 %exitcond8, label %b17, label %b16

b41:
  %52 = phi i16 [ 319, %25 ], [ %61, %b41 ]
  %53 = phi i16 [ -640, %25 ], [ %60, %b41 ]
  %54 = phi i16 [ %59, %b41 ], [ 0, %25 ]
  %55 = icmp samesign ult i16 %54, 160
  %56 = select i1 %55, i16 %54, i16 %52
  %57 = getelementptr i8, ptr @"FSIN2%", i16 %53
  %58 = getelementptr i8, ptr %57, i16 1920
  store i16 %56, ptr %58, !tbaa !2
  %59 = add nuw nsw i16 %54, 1
  %60 = add nsw i16 %53, 2
  %61 = add nsw i16 %52, -1
  %exitcond7 = icmp eq i16 %60, 0
  br i1 %exitcond7, label %62, label %b41

62:
  br label %b49

b49:
  %63 = phi i16 [ 199, %62 ], [ %73, %b49 ]
  %64 = phi i16 [ -400, %62 ], [ %72, %b49 ]
  %65 = phi i16 [ %71, %b49 ], [ 0, %62 ]
  %66 = icmp samesign ugt i16 %65, 99
  %67 = select i1 %66, i16 %63, i16 %65
  %68 = mul nuw nsw i16 %67, 160
  %69 = getelementptr i8, ptr @"FSIN3%", i16 %64
  %70 = getelementptr i8, ptr %69, i16 1680
  store i16 %68, ptr %70, !tbaa !2
  %71 = add nuw nsw i16 %65, 1
  %72 = add nsw i16 %64, 2
  %73 = add nsw i16 %63, -1
  %exitcond6 = icmp eq i16 %72, 0
  br i1 %exitcond6, label %74, label %b49

74:
  br label %b57

b57:
  %75 = phi i16 [ -1793, %74 ], [ %82, %b57 ]
  %76 = phi i16 [ -3586, %74 ], [ %81, %b57 ]
  %77 = add i16 %75, 1793
  %78 = getelementptr i8, ptr @"CD%", i16 %76
  %79 = getelementptr i8, ptr %78, i16 3586
  %80 = tail call i16 @llvm.umin.i16(i16 %77, i16 255)
  store i16 %80, ptr %79, !tbaa !2
  %81 = add nsw i16 %76, 2
  %82 = add nsw i16 %75, 1
  %exitcond5 = icmp eq i16 %81, 0
  br i1 %exitcond5, label %83, label %b57

83:
  br label %b65

b65:
  %84 = phi i16 [ -14, %83 ], [ %133, %b65 ]
  %85 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %86 = load float, ptr %85
  %87 = fmul float %86, 3.500000e+01
  %88 = fadd float %87, 2.500000e+01
  %89 = tail call float @llvm.rint.f32(float %88)
  %90 = fcmp olt float %88, %89
  %91 = sext i1 %90 to i16
  %92 = sitofp i16 %91 to float
  %93 = fadd float %89, %92
  %94 = tail call i16 @llvm.lrint.i16.f32(float %93)
  %95 = getelementptr i8, ptr @"DSX%", i16 %84
  %96 = getelementptr i8, ptr %95, i16 16
  store i16 %94, ptr %96, !tbaa !2
  %97 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %98 = load float, ptr %97
  %99 = fmul float %98, 3.500000e+01
  %100 = fadd float %99, 2.500000e+01
  %101 = tail call float @llvm.rint.f32(float %100)
  %102 = fcmp olt float %100, %101
  %103 = sext i1 %102 to i16
  %104 = sitofp i16 %103 to float
  %105 = fadd float %101, %104
  %106 = tail call i16 @llvm.lrint.i16.f32(float %105)
  %107 = getelementptr i8, ptr @"DSY%", i16 %84
  %108 = getelementptr i8, ptr %107, i16 16
  store i16 %106, ptr %108, !tbaa !2
  %109 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %110 = load float, ptr %109
  %111 = fmul float %110, 6.400000e+01
  %112 = fadd float %111, 6.400000e+01
  %113 = tail call float @llvm.rint.f32(float %112)
  %114 = fcmp olt float %112, %113
  %115 = sext i1 %114 to i16
  %116 = sitofp i16 %115 to float
  %117 = fadd float %113, %116
  %118 = tail call i16 @llvm.lrint.i16.f32(float %117)
  %119 = getelementptr i8, ptr @"PSX%", i16 %84
  %120 = getelementptr i8, ptr %119, i16 16
  store i16 %118, ptr %120, !tbaa !2
  %121 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %122 = load float, ptr %121
  %123 = fmul float %122, 5.000000e+01
  %124 = fadd float %123, 5.000000e+01
  %125 = tail call float @llvm.rint.f32(float %124)
  %126 = fcmp olt float %124, %125
  %127 = sext i1 %126 to i16
  %128 = sitofp i16 %127 to float
  %129 = fadd float %125, %128
  %130 = tail call i16 @llvm.lrint.i16.f32(float %129)
  %131 = getelementptr i8, ptr @"PSY%", i16 %84
  %132 = getelementptr i8, ptr %131, i16 16
  store i16 %130, ptr %132, !tbaa !2
  %133 = add nsw i16 %84, 2
  %exitcond4 = icmp eq i16 %133, 0
  br i1 %exitcond4, label %b66, label %b65

b66:
  %134 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %135 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %136 = inttoptr i16 %134 to ptr addrspace(2)
  %137 = addrspacecast ptr addrspace(2) %136 to ptr addrspace(1)
  %138 = getelementptr i8, ptr addrspace(1) %137, i16 %135
  %139 = getelementptr i8, ptr addrspace(1) %138, i16 2
  store i16 1, ptr addrspace(1) %139, !tbaa !60
  %140 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %141 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %142 = inttoptr i16 %140 to ptr addrspace(2)
  %143 = addrspacecast ptr addrspace(2) %142 to ptr addrspace(1)
  %144 = getelementptr i8, ptr addrspace(1) %143, i16 %141
  %145 = getelementptr i8, ptr addrspace(1) %144, i16 2
  store i16 1, ptr addrspace(1) %145, !tbaa !62
  %146 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %147 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %148 = inttoptr i16 %146 to ptr addrspace(2)
  %149 = addrspacecast ptr addrspace(2) %148 to ptr addrspace(1)
  %150 = getelementptr i8, ptr addrspace(1) %149, i16 %147
  %151 = getelementptr i8, ptr addrspace(1) %150, i16 2
  store i16 1, ptr addrspace(1) %151, !tbaa !64
  br label %b68

b68:
  %152 = phi i16 [ 0, %b66 ], [ %156, %b122 ], [ %156, %434 ]
  %153 = phi i16 [ 3, %b66 ], [ %266, %b122 ], [ %266, %434 ]
  %154 = phi i16 [ 0, %b66 ], [ %267, %b122 ], [ %267, %434 ]
  %155 = phi i16 [ 0, %b66 ], [ %268, %b122 ], [ %268, %434 ]
  %156 = add nuw nsw i16 %152, 1
  %157 = add i16 %154, 1
  %158 = icmp ult i16 %152, 159
  %159 = icmp eq i16 %157, %153
  %160 = and i1 %158, %159
  br i1 %160, label %b72.thread, label %b72

b72.thread:
  %161 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %162 = load float, ptr %161
  %163 = fmul float %162, 2.500000e+01
  %164 = fadd float %163, 1.000000e+00
  %165 = tail call float @llvm.rint.f32(float %164)
  %166 = fcmp olt float %164, %165
  %167 = sext i1 %166 to i16
  %168 = sitofp i16 %167 to float
  %169 = fadd float %165, %168
  %170 = tail call i16 @llvm.lrint.i16.f32(float %169)
  %171 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %172 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %173 = inttoptr i16 %171 to ptr addrspace(2)
  %174 = addrspacecast ptr addrspace(2) %173 to ptr addrspace(1)
  %175 = getelementptr i8, ptr addrspace(1) %174, i16 %172
  %176 = getelementptr i8, ptr addrspace(1) %175, i16 2
  %177 = getelementptr i8, ptr addrspace(1) %175, i16 4
  %178 = load i16, ptr addrspace(1) %177, !tbaa !60
  store i16 %178, ptr addrspace(1) %176, !tbaa !60
  %179 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %180 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %181 = inttoptr i16 %179 to ptr addrspace(2)
  %182 = addrspacecast ptr addrspace(2) %181 to ptr addrspace(1)
  %183 = getelementptr i8, ptr addrspace(1) %182, i16 %180
  %184 = getelementptr i8, ptr addrspace(1) %183, i16 2
  %185 = getelementptr i8, ptr addrspace(1) %183, i16 4
  %186 = load i16, ptr addrspace(1) %185, !tbaa !62
  store i16 %186, ptr addrspace(1) %184, !tbaa !62
  %187 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %188 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %189 = inttoptr i16 %187 to ptr addrspace(2)
  %190 = addrspacecast ptr addrspace(2) %189 to ptr addrspace(1)
  %191 = getelementptr i8, ptr addrspace(1) %190, i16 %188
  %192 = getelementptr i8, ptr addrspace(1) %191, i16 2
  %193 = getelementptr i8, ptr addrspace(1) %191, i16 4
  %194 = load i16, ptr addrspace(1) %193, !tbaa !64
  store i16 %194, ptr addrspace(1) %192, !tbaa !64
  %195 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %196 = load float, ptr %195
  %197 = fmul float %196, 6.300000e+01
  %198 = tail call float @llvm.rint.f32(float %197)
  %199 = fcmp olt float %197, %198
  %200 = sext i1 %199 to i16
  %201 = sitofp i16 %200 to float
  %202 = fadd float %198, %201
  %203 = fadd float %202, 1.000000e+00
  %204 = tail call i16 @llvm.lrint.i16.f32(float %203)
  store i16 %204, ptr addrspace(1) %177, !tbaa !60
  %205 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %206 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %207 = inttoptr i16 %205 to ptr addrspace(2)
  %208 = addrspacecast ptr addrspace(2) %207 to ptr addrspace(1)
  %209 = getelementptr i8, ptr addrspace(1) %208, i16 %206
  %210 = getelementptr i8, ptr addrspace(1) %209, i16 4
  %211 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %212 = load float, ptr %211
  %213 = fmul float %212, 6.300000e+01
  %214 = tail call float @llvm.rint.f32(float %213)
  %215 = fcmp olt float %213, %214
  %216 = sext i1 %215 to i16
  %217 = sitofp i16 %216 to float
  %218 = fadd float %214, %217
  %219 = fadd float %218, 1.000000e+00
  %220 = tail call i16 @llvm.lrint.i16.f32(float %219)
  store i16 %220, ptr addrspace(1) %210, !tbaa !62
  %221 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %222 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %223 = inttoptr i16 %221 to ptr addrspace(2)
  %224 = addrspacecast ptr addrspace(2) %223 to ptr addrspace(1)
  %225 = getelementptr i8, ptr addrspace(1) %224, i16 %222
  %226 = getelementptr i8, ptr addrspace(1) %225, i16 4
  %227 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %228 = load float, ptr %227
  %229 = fmul float %228, 6.300000e+01
  %230 = tail call float @llvm.rint.f32(float %229)
  %231 = fcmp olt float %229, %230
  %232 = sext i1 %231 to i16
  %233 = sitofp i16 %232 to float
  %234 = fadd float %230, %233
  %235 = fadd float %234, 1.000000e+00
  %236 = tail call i16 @llvm.lrint.i16.f32(float %235)
  store i16 %236, ptr addrspace(1) %226, !tbaa !64
  br label %b75

b72:
  %237 = icmp eq i16 %157, %153
  %238 = icmp ugt i16 %152, 159
  %239 = icmp ult i16 %152, 184
  %240 = and i1 %238, %239
  %241 = and i1 %240, %237
  br i1 %241, label %b73, label %b75

b73:
  %242 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %243 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %244 = inttoptr i16 %242 to ptr addrspace(2)
  %245 = addrspacecast ptr addrspace(2) %244 to ptr addrspace(1)
  %246 = getelementptr i8, ptr addrspace(1) %245, i16 %243
  %247 = getelementptr i8, ptr addrspace(1) %246, i16 2
  %248 = getelementptr i8, ptr addrspace(1) %246, i16 4
  %249 = load i16, ptr addrspace(1) %248, !tbaa !60
  store i16 %249, ptr addrspace(1) %247, !tbaa !60
  %250 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %251 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %252 = inttoptr i16 %250 to ptr addrspace(2)
  %253 = addrspacecast ptr addrspace(2) %252 to ptr addrspace(1)
  %254 = getelementptr i8, ptr addrspace(1) %253, i16 %251
  %255 = getelementptr i8, ptr addrspace(1) %254, i16 2
  %256 = getelementptr i8, ptr addrspace(1) %254, i16 4
  %257 = load i16, ptr addrspace(1) %256, !tbaa !62
  store i16 %257, ptr addrspace(1) %255, !tbaa !62
  %258 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %259 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %260 = inttoptr i16 %258 to ptr addrspace(2)
  %261 = addrspacecast ptr addrspace(2) %260 to ptr addrspace(1)
  %262 = getelementptr i8, ptr addrspace(1) %261, i16 %259
  %263 = getelementptr i8, ptr addrspace(1) %262, i16 2
  %264 = getelementptr i8, ptr addrspace(1) %262, i16 4
  %265 = load i16, ptr addrspace(1) %264, !tbaa !64
  store i16 %265, ptr addrspace(1) %263, !tbaa !64
  store i16 0, ptr addrspace(1) %248, !tbaa !60
  store i16 0, ptr addrspace(1) %256, !tbaa !62
  store i16 0, ptr addrspace(1) %264, !tbaa !64
  br label %b75

b75:
  %266 = phi i16 [ 25, %b73 ], [ %153, %b72 ], [ %170, %b72.thread ]
  %267 = phi i16 [ 0, %b73 ], [ %157, %b72 ], [ 0, %b72.thread ]
  %268 = phi i16 [ 1, %b73 ], [ %155, %b72 ], [ %155, %b72.thread ]
  %269 = icmp ugt i16 %152, 184
  %270 = sub nsw i16 0, %268
  %271 = icmp slt i16 %267, %266
  %272 = xor i1 %269, true
  %273 = or i1 %271, %272
  %274 = select i1 %273, i16 0, i16 %270
  %275 = icmp ne i16 %274, 0
  %276 = icmp ugt i16 %152, 209
  %or.cond = or i1 %276, %275
  br i1 %or.cond, label %b2, label %b82

b82:
  %277 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %278 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %279 = inttoptr i16 %277 to ptr addrspace(2)
  %280 = addrspacecast ptr addrspace(2) %279 to ptr addrspace(1)
  %281 = getelementptr i8, ptr addrspace(1) %280, i16 %278
  %282 = getelementptr i8, ptr addrspace(1) %281, i16 2
  %283 = load i16, ptr addrspace(1) %282, !tbaa !60
  %284 = getelementptr i8, ptr addrspace(1) %281, i16 4
  %285 = load i16, ptr addrspace(1) %284, !tbaa !60
  %286 = sub i16 %285, %283
  %287 = sitofp i16 %286 to float
  %288 = sitofp i16 %266 to float
  %289 = fdiv float %287, %288
  %290 = sitofp i16 %267 to float
  %291 = fmul float %289, %290
  %292 = sitofp i16 %283 to float
  %293 = fadd float %291, %292
  %294 = tail call i16 @llvm.lrint.i16.f32(float %293)
  %295 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %296 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %297 = inttoptr i16 %295 to ptr addrspace(2)
  %298 = addrspacecast ptr addrspace(2) %297 to ptr addrspace(1)
  %299 = getelementptr i8, ptr addrspace(1) %298, i16 %296
  %300 = getelementptr i8, ptr addrspace(1) %299, i16 2
  %301 = load i16, ptr addrspace(1) %300, !tbaa !62
  %302 = getelementptr i8, ptr addrspace(1) %299, i16 4
  %303 = load i16, ptr addrspace(1) %302, !tbaa !62
  %304 = sub i16 %303, %301
  %305 = sitofp i16 %304 to float
  %306 = fdiv float %305, %288
  %307 = fmul float %306, %290
  %308 = sitofp i16 %301 to float
  %309 = fadd float %307, %308
  %310 = tail call i16 @llvm.lrint.i16.f32(float %309)
  %311 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %312 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %313 = inttoptr i16 %311 to ptr addrspace(2)
  %314 = addrspacecast ptr addrspace(2) %313 to ptr addrspace(1)
  %315 = getelementptr i8, ptr addrspace(1) %314, i16 %312
  %316 = getelementptr i8, ptr addrspace(1) %315, i16 2
  %317 = load i16, ptr addrspace(1) %316, !tbaa !64
  %318 = getelementptr i8, ptr addrspace(1) %315, i16 4
  %319 = load i16, ptr addrspace(1) %318, !tbaa !64
  %320 = sub i16 %319, %317
  %321 = sitofp i16 %320 to float
  %322 = fdiv float %321, %288
  %323 = fmul float %322, %290
  %324 = sitofp i16 %317 to float
  %325 = fadd float %323, %324
  %326 = tail call i16 @llvm.lrint.i16.f32(float %325)
  %327 = icmp eq i16 %294, 0
  %spec.store.select = select i1 %327, i16 1, i16 %294
  %328 = icmp eq i16 %310, 0
  %spec.store.select1 = select i1 %328, i16 1, i16 %310
  %329 = icmp eq i16 %326, 0
  %spec.store.select2 = select i1 %329, i16 1, i16 %326
  %330 = sitofp i16 %spec.store.select to float
  %331 = fdiv float 6.300000e+01, %330
  %332 = sitofp i16 %spec.store.select1 to float
  %333 = fdiv float 6.300000e+01, %332
  %334 = sitofp i16 %spec.store.select2 to float
  %335 = fdiv float 6.300000e+01, %334
  br label %b96

b96:
  %336 = phi i16 [ %353, %b100 ], [ 127, %b82 ]
  %337 = phi i16 [ 128, %b82 ], [ %352, %b100 ]
  %tmp = trunc i16 %337 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp) memory(inaccessiblemem: readwrite)
  %338 = icmp samesign ult i16 %337, 192
  br i1 %338, label %b98, label %b99

b97:
  %339 = uitofp nneg i16 %156 to float
  br label %b104

b98:
  %340 = add i16 %337, -128
  %341 = sitofp i16 %340 to float
  br label %b100

b99:
  %342 = uitofp nneg i16 %336 to float
  br label %b100

b100:
  %sink19 = phi float [ %341, %b98 ], [ %342, %b99 ]
  %343 = fdiv float %sink19, %331
  %344 = tail call i16 @llvm.lrint.i16.f32(float %343)
  %345 = trunc i16 %344 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %345) memory(inaccessiblemem: readwrite)
  %346 = fdiv float %sink19, %333
  %347 = tail call i16 @llvm.lrint.i16.f32(float %346)
  %348 = trunc i16 %347 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %348) memory(inaccessiblemem: readwrite)
  %349 = fdiv float %sink19, %335
  %350 = tail call i16 @llvm.lrint.i16.f32(float %349)
  %351 = trunc i16 %350 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %351) memory(inaccessiblemem: readwrite)
  %352 = add nuw nsw i16 %337, 1
  %353 = add nsw i16 %336, -1
  %exitcond3 = icmp eq i16 %353, -1
  br i1 %exitcond3, label %b97, label %b96

b104:
  %354 = phi i16 [ %387, %b109 ], [ -14, %b97 ]
  %355 = getelementptr i8, ptr @"DSX%", i16 %354
  %356 = getelementptr i8, ptr %355, i16 16
  %357 = load i16, ptr %356, !tbaa !2
  %358 = sitofp i16 %357 to float
  %359 = fdiv float %339, %358
  %360 = tail call float @llvm.fabs.f32(float %359)
  %361 = fcmp ogt float %360, 0x41E0000000000000
  br i1 %361, label %b106, label %b107

b106:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b107:
  %362 = tail call float @llvm.sin.f32(float %359)
  %363 = getelementptr i8, ptr @"PSX%", i16 %354
  %364 = getelementptr i8, ptr %363, i16 16
  %365 = load i16, ptr %364, !tbaa !2
  %366 = sitofp i16 %365 to float
  %367 = fmul float %362, %366
  %368 = tail call i16 @llvm.lrint.i16.f32(float %367)
  %369 = getelementptr i8, ptr @"XP1%", i16 %354
  %370 = getelementptr i8, ptr %369, i16 16
  store i16 %368, ptr %370, !tbaa !2
  %371 = getelementptr i8, ptr @"DSY%", i16 %354
  %372 = getelementptr i8, ptr %371, i16 16
  %373 = load i16, ptr %372, !tbaa !2
  %374 = sitofp i16 %373 to float
  %375 = fdiv float %339, %374
  %376 = tail call float @llvm.fabs.f32(float %375)
  %377 = fcmp ogt float %376, 0x41E0000000000000
  br i1 %377, label %b108, label %b109

b108:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b109:
  %378 = tail call float @llvm.sin.f32(float %375)
  %379 = getelementptr i8, ptr @"PSY%", i16 %354
  %380 = getelementptr i8, ptr %379, i16 16
  %381 = load i16, ptr %380, !tbaa !2
  %382 = sitofp i16 %381 to float
  %383 = fmul float %378, %382
  %384 = tail call i16 @llvm.lrint.i16.f32(float %383)
  %385 = getelementptr i8, ptr @"YP1%", i16 %354
  %386 = getelementptr i8, ptr %385, i16 16
  store i16 %384, ptr %386, !tbaa !2
  %387 = add nsw i16 %354, 2
  %exitcond2 = icmp eq i16 %387, 0
  br i1 %exitcond2, label %388, label %b104

388:
  br label %b113

b113:
  %389 = phi i16 [ %521, %b119 ], [ 0, %388 ]
  %390 = phi i16 [ %392, %b119 ], [ 0, %388 ]
  %391 = add nuw nsw i16 %390, -24576
  store i16 %391, ptr @b$seg, !tbaa !2
  %392 = add nuw nsw i16 %390, 20
  %393 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 2), !tbaa !2
  %394 = sub i16 %389, %393
  %395 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %394
  %396 = getelementptr i8, ptr %395, i16 1280
  %397 = load i16, ptr %396, !tbaa !2
  store i16 %397, ptr @"YY%.0", !tbaa !2
  %398 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 4), !tbaa !2
  %399 = sub i16 %389, %398
  %400 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %399
  %401 = getelementptr i8, ptr %400, i16 1280
  %402 = load i16, ptr %401, !tbaa !2
  store i16 %402, ptr @"YY%.1", !tbaa !2
  %403 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 6), !tbaa !2
  %404 = sub i16 %389, %403
  %405 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %404
  %406 = getelementptr i8, ptr %405, i16 1280
  %407 = load i16, ptr %406, !tbaa !2
  store i16 %407, ptr @"YY%.2", !tbaa !2
  %408 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 8), !tbaa !2
  %409 = sub i16 %389, %408
  %410 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %409
  %411 = getelementptr i8, ptr %410, i16 1280
  %412 = load i16, ptr %411, !tbaa !2
  %413 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 10), !tbaa !2
  %414 = sub i16 %389, %413
  %415 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %414
  %416 = getelementptr i8, ptr %415, i16 1280
  %417 = load i16, ptr %416, !tbaa !2
  %418 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 12), !tbaa !2
  %419 = sub i16 %389, %418
  %420 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %419
  %421 = getelementptr i8, ptr %420, i16 1280
  %422 = load i16, ptr %421, !tbaa !2
  %423 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 14), !tbaa !2
  %424 = sub i16 %389, %423
  %425 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %424
  %426 = getelementptr i8, ptr %425, i16 1280
  %427 = load i16, ptr %426, !tbaa !2
  %428 = getelementptr [2 x i8], ptr @"SP%", i16 %412
  %429 = getelementptr [2 x i8], ptr @"SP%", i16 %417
  %430 = getelementptr [2 x i8], ptr @"SP%", i16 %422
  %431 = getelementptr [2 x i8], ptr @"SP%", i16 %427
  br label %b118

b114:
  %432 = load i16, ptr @"XIT%", !tbaa !2
  %433 = icmp eq i16 %432, 0
  br i1 %433, label %522, label %434

434:
  %435 = icmp eq i16 %432, 1
  br i1 %435, label %b2, label %b68

b118:
  %436 = phi i16 [ %520, %b118 ], [ 0, %b113 ]
  %437 = load i16, ptr @"YY%.0", !tbaa !2
  %438 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 2), !tbaa !2
  %439 = mul i16 %438, -2
  %440 = shl nuw nsw i16 %436, 1
  %441 = add i16 %439, %440
  %442 = getelementptr i8, ptr @"FSIN2%", i16 %441
  %443 = getelementptr i8, ptr %442, i16 1328
  %444 = load i16, ptr %443, !tbaa !2
  %445 = getelementptr [2 x i8], ptr @"SP%", i16 %437
  %446 = getelementptr [2 x i8], ptr %445, i16 %444
  %447 = load i16, ptr %446, !tbaa !2
  %448 = load i16, ptr @"YY%.1", !tbaa !2
  %449 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 4), !tbaa !2
  %450 = mul i16 %449, -2
  %451 = shl nuw nsw i16 %436, 1
  %452 = add i16 %450, %451
  %453 = getelementptr i8, ptr @"FSIN2%", i16 %452
  %454 = getelementptr i8, ptr %453, i16 1328
  %455 = load i16, ptr %454, !tbaa !2
  %456 = getelementptr [2 x i8], ptr @"SP%", i16 %448
  %457 = getelementptr [2 x i8], ptr %456, i16 %455
  %458 = load i16, ptr %457, !tbaa !2
  %459 = load i16, ptr @"YY%.2", !tbaa !2
  %460 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 6), !tbaa !2
  %461 = mul i16 %460, -2
  %462 = shl nuw nsw i16 %436, 1
  %463 = add i16 %461, %462
  %464 = getelementptr i8, ptr @"FSIN2%", i16 %463
  %465 = getelementptr i8, ptr %464, i16 1328
  %466 = load i16, ptr %465, !tbaa !2
  %467 = getelementptr [2 x i8], ptr @"SP%", i16 %459
  %468 = getelementptr [2 x i8], ptr %467, i16 %466
  %469 = load i16, ptr %468, !tbaa !2
  %470 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 8), !tbaa !2
  %471 = mul i16 %470, -2
  %472 = shl nuw nsw i16 %436, 1
  %473 = add i16 %471, %472
  %474 = getelementptr i8, ptr @"FSIN2%", i16 %473
  %475 = getelementptr i8, ptr %474, i16 1328
  %476 = load i16, ptr %475, !tbaa !2
  %477 = getelementptr [2 x i8], ptr %428, i16 %476
  %478 = load i16, ptr %477, !tbaa !2
  %479 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 10), !tbaa !2
  %480 = mul i16 %479, -2
  %481 = shl nuw nsw i16 %436, 1
  %482 = add i16 %480, %481
  %483 = getelementptr i8, ptr @"FSIN2%", i16 %482
  %484 = getelementptr i8, ptr %483, i16 1328
  %485 = load i16, ptr %484, !tbaa !2
  %486 = getelementptr [2 x i8], ptr %429, i16 %485
  %487 = load i16, ptr %486, !tbaa !2
  %488 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 12), !tbaa !2
  %489 = mul i16 %488, -2
  %490 = shl nuw nsw i16 %436, 1
  %491 = add i16 %489, %490
  %492 = getelementptr i8, ptr @"FSIN2%", i16 %491
  %493 = getelementptr i8, ptr %492, i16 1328
  %494 = load i16, ptr %493, !tbaa !2
  %495 = getelementptr [2 x i8], ptr %430, i16 %494
  %496 = load i16, ptr %495, !tbaa !2
  %497 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 14), !tbaa !2
  %498 = mul i16 %497, -2
  %499 = shl nuw nsw i16 %436, 1
  %500 = add i16 %498, %499
  %501 = getelementptr i8, ptr @"FSIN2%", i16 %500
  %502 = getelementptr i8, ptr %501, i16 1328
  %503 = load i16, ptr %502, !tbaa !2
  %504 = getelementptr [2 x i8], ptr %431, i16 %503
  %505 = load i16, ptr %504, !tbaa !2
  %506 = getelementptr [2 x i8], ptr @"CD%", i16 %447
  %507 = getelementptr [2 x i8], ptr %506, i16 %458
  %508 = getelementptr [2 x i8], ptr %507, i16 %469
  %509 = getelementptr [2 x i8], ptr %508, i16 %478
  %510 = getelementptr [2 x i8], ptr %509, i16 %487
  %511 = getelementptr [2 x i8], ptr %510, i16 %496
  %512 = getelementptr [2 x i8], ptr %511, i16 %505
  %513 = load i16, ptr %512, !tbaa !2
  %514 = trunc i16 %513 to i8
  %515 = load i16, ptr @b$seg, !tbaa !2
  %516 = inttoptr i16 %515 to ptr addrspace(2)
  %517 = addrspacecast ptr addrspace(2) %516 to ptr addrspace(1)
  %518 = getelementptr i8, ptr addrspace(1) %517, i16 24
  %519 = getelementptr i8, ptr addrspace(1) %518, i16 %436
  store volatile i8 %514, ptr addrspace(1) %519
  %520 = add nuw nsw i16 %436, 1
  %exitcond = icmp eq i16 %520, 272
  br i1 %exitcond, label %b119, label %b118

b119:
  %521 = add nuw nsw i16 %389, 1
  %exitcond1 = icmp eq i16 %521, 200
  br i1 %exitcond1, label %b114, label %b113

522:
  %523 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %524 = icmp eq i8 %523, 1
  br i1 %524, label %b122.thread, label %b122

b122.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b2

b122:
  %525 = icmp eq i16 %432, 1
  br i1 %525, label %b2, label %b68
}

define cc 1000 void @DEEDLINESAX(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 20, i16 20)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 25, i16 80)
  %1 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %1, i16 -1, i16 0)
  %2 = load i16, ptr @"XIT%", !tbaa !2
  %3 = icmp eq i16 %2, 1
  br i1 %3, label %b2, label %b29

b2:
  ret void

b29:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 25, i16 80)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 60, i16 70)
  %4 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %4, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 60, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 55, i16 30)
  %5 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %5, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 55, i16 30)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 20, i16 20)
  %6 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %6, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 25, i16 25)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 30, i16 60)
  %7 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %7, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 30, i16 60)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 50, i16 65)
  %8 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %8, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 50, i16 65)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 25, i16 25)
  %9 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %9, i16 -1, i16 0)
  %10 = load i16, ptr %0
  %11 = icmp eq i16 %10, 15
  br i1 %11, label %b31, label %b68

b31:
  %12 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %13 = load float, ptr %12
  %14 = fmul float %13, 2.550000e+02
  %15 = tail call float @llvm.rint.f32(float %14)
  %16 = fcmp olt float %14, %15
  %17 = sext i1 %16 to i16
  %18 = sitofp i16 %17 to float
  %19 = fadd float %15, %18
  %20 = tail call i16 @llvm.lrint.i16.f32(float %19)
  br label %b68

b68:
  %21 = phi i16 [ %20, %b31 ], [ 0, %b29 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 23, i16 23)
  %22 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %21, i16 %22)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 65, i16 20)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 100, i16 24)
  %23 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %23, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 100, i16 24)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 70, i16 34)
  %24 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %24, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 70, i16 34)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 85, i16 44)
  %25 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %25, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 85, i16 44)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 75, i16 54)
  %26 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %26, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 75, i16 54)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 75, i16 64)
  %27 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %27, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 75, i16 64)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 85, i16 64)
  %28 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %28, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 85, i16 64)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 70, i16 70)
  %29 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %29, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 70, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 65, i16 20)
  %30 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %30, i16 -1, i16 0)
  %31 = load i16, ptr %0
  %32 = icmp eq i16 %31, 15
  br i1 %32, label %b70, label %b79

b70:
  %33 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %34 = load float, ptr %33
  %35 = fmul float %34, 2.550000e+02
  %36 = tail call float @llvm.rint.f32(float %35)
  %37 = fcmp olt float %35, %36
  %38 = sext i1 %37 to i16
  %39 = sitofp i16 %38 to float
  %40 = fadd float %36, %39
  %41 = tail call i16 @llvm.lrint.i16.f32(float %40)
  br label %b79

b79:
  %42 = phi i16 [ %41, %b70 ], [ 0, %b68 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 66, i16 21)
  %43 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %42, i16 %43)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 30)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 130, i16 30)
  %44 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %44, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 130, i16 30)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 40)
  %45 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %45, i16 -1, i16 0)
  %46 = load i16, ptr @"XIT%", !tbaa !2
  %47 = icmp eq i16 %46, 1
  br i1 %47, label %b2, label %b119

b119:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 40)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 50)
  %48 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %48, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 50)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 125, i16 50)
  %49 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %49, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 125, i16 50)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 125, i16 55)
  %50 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %50, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 125, i16 55)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 55)
  %51 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %51, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 55)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 65)
  %52 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %52, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 110, i16 65)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 120, i16 65)
  %53 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %53, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 120, i16 65)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 120, i16 70)
  %54 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %54, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 120, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 100, i16 70)
  %55 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %55, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 100, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 110, i16 30)
  %56 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %56, i16 -1, i16 0)
  %57 = load i16, ptr %0
  %58 = icmp eq i16 %57, 15
  br i1 %58, label %b121, label %b150

b121:
  %59 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %60 = load float, ptr %59
  %61 = fmul float %60, 2.550000e+02
  %62 = tail call float @llvm.rint.f32(float %61)
  %63 = fcmp olt float %61, %62
  %64 = sext i1 %63 to i16
  %65 = sitofp i16 %64 to float
  %66 = fadd float %62, %65
  %67 = tail call i16 @llvm.lrint.i16.f32(float %66)
  br label %b150

b150:
  %68 = phi i16 [ %67, %b121 ], [ 0, %b119 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 111, i16 31)
  %69 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %68, i16 %69)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 150, i16 40)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 130, i16 20)
  %70 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %70, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 130, i16 20)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 140, i16 70)
  %71 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %71, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 140, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 150, i16 40)
  %72 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %72, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 135, i16 30)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 140, i16 60)
  %73 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %73, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 140, i16 60)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 145, i16 40)
  %74 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %74, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 145, i16 40)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 135, i16 30)
  %75 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %75, i16 -1, i16 0)
  %76 = load i16, ptr %0
  %77 = icmp eq i16 %76, 15
  br i1 %77, label %b152, label %b154

b152:
  %78 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %79 = load float, ptr %78
  %80 = fmul float %79, 2.550000e+02
  %81 = tail call float @llvm.rint.f32(float %80)
  %82 = fcmp olt float %80, %81
  %83 = sext i1 %82 to i16
  %84 = sitofp i16 %83 to float
  %85 = fadd float %81, %84
  %86 = tail call i16 @llvm.lrint.i16.f32(float %85)
  br label %b154

b154:
  %87 = phi i16 [ %86, %b152 ], [ 0, %b150 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 133, i16 25)
  %88 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %87, i16 %88)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 160, i16 30)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 165, i16 70)
  %89 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %89, i16 -1, i16 0)
  %90 = load i16, ptr @"XIT%", !tbaa !2
  %91 = icmp eq i16 %90, 1
  br i1 %91, label %b2, label %b169

b169:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 165, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 190, i16 70)
  %92 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %92, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 190, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 175, i16 65)
  %93 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %93, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 175, i16 65)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 160, i16 30)
  %94 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %94, i16 -1, i16 0)
  %95 = load i16, ptr %0
  %96 = icmp eq i16 %95, 15
  br i1 %96, label %b171, label %b204

b171:
  %97 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %98 = load float, ptr %97
  %99 = fmul float %98, 2.550000e+02
  %100 = tail call float @llvm.rint.f32(float %99)
  %101 = fcmp olt float %99, %100
  %102 = sext i1 %101 to i16
  %103 = sitofp i16 %102 to float
  %104 = fadd float %100, %103
  %105 = tail call i16 @llvm.lrint.i16.f32(float %104)
  br label %b204

b204:
  %106 = phi i16 [ %105, %b171 ], [ 0, %b169 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 164, i16 40)
  %107 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %106, i16 %107)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 180, i16 20)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 190, i16 60)
  %108 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %108, i16 -1, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 195, i16 20)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 195, i16 70)
  %109 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %109, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 195, i16 70)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 200, i16 40)
  %110 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %110, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 200, i16 40)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 220, i16 65)
  %111 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %111, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 220, i16 65)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 220, i16 35)
  %112 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %112, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 220, i16 35)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 215, i16 55)
  %113 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %113, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 215, i16 55)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 195, i16 20)
  %114 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %114, i16 -1, i16 0)
  %115 = load i16, ptr %0
  %116 = icmp eq i16 %115, 15
  br i1 %116, label %b206, label %b243

b206:
  %117 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %118 = load float, ptr %117
  %119 = fmul float %118, 2.550000e+02
  %120 = tail call float @llvm.rint.f32(float %119)
  %121 = fcmp olt float %119, %120
  %122 = sext i1 %121 to i16
  %123 = sitofp i16 %122 to float
  %124 = fadd float %120, %123
  %125 = tail call i16 @llvm.lrint.i16.f32(float %124)
  br label %b243

b243:
  %126 = phi i16 [ %125, %b206 ], [ 0, %b204 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 196, i16 30)
  %127 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %126, i16 %127)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 245, i16 20)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 225, i16 20)
  %128 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %128, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 225, i16 20)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 225, i16 60)
  %129 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %129, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 225, i16 60)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 250, i16 60)
  %130 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %130, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 250, i16 60)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 250, i16 50)
  %131 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %131, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 250, i16 50)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 230, i16 50)
  %132 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %132, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 230, i16 50)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 230, i16 40)
  %133 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %133, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 230, i16 40)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 240, i16 40)
  %134 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %134, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 240, i16 40)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 240, i16 35)
  %135 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %135, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 240, i16 35)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 228, i16 35)
  %136 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %136, i16 -1, i16 0)
  %137 = load i16, ptr @"XIT%", !tbaa !2
  %138 = icmp eq i16 %137, 1
  br i1 %138, label %b2, label %b255

b255:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 228, i16 35)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 230, i16 25)
  %139 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %139, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 230, i16 25)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 245, i16 20)
  %140 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %140, i16 -1, i16 0)
  %141 = load i16, ptr %0
  %142 = icmp eq i16 %141, 15
  br i1 %142, label %b257, label %b286

b257:
  %143 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %144 = load float, ptr %143
  %145 = fmul float %144, 2.550000e+02
  %146 = tail call float @llvm.rint.f32(float %145)
  %147 = fcmp olt float %145, %146
  %148 = sext i1 %147 to i16
  %149 = sitofp i16 %148 to float
  %150 = fadd float %146, %149
  %151 = tail call i16 @llvm.lrint.i16.f32(float %150)
  br label %b286

b286:
  %152 = phi i16 [ %151, %b257 ], [ 0, %b255 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 227, i16 24)
  %153 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %152, i16 %153)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 300, i16 5)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 260, i16 8)
  %154 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %154, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 260, i16 8)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 290, i16 58)
  %155 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %155, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 290, i16 58)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 250, i16 68)
  %156 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %156, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 250, i16 68)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 305, i16 64)
  %157 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %157, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 305, i16 64)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 270, i16 18)
  %158 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %158, i16 -1, i16 0)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 270, i16 18)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 300, i16 5)
  %159 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %159, i16 -1, i16 0)
  %160 = load i16, ptr %0
  %161 = icmp eq i16 %160, 15
  br i1 %161, label %b288, label %b297

b288:
  %162 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %163 = load float, ptr %162
  %164 = fmul float %163, 2.550000e+02
  %165 = tail call float @llvm.rint.f32(float %164)
  %166 = fcmp olt float %164, %165
  %167 = sext i1 %166 to i16
  %168 = sitofp i16 %167 to float
  %169 = fadd float %165, %168
  %170 = tail call i16 @llvm.lrint.i16.f32(float %169)
  br label %b297

b297:
  %171 = phi i16 [ %170, %b288 ], [ 0, %b286 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 265, i16 10)
  %172 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PAIN(i16 %171, i16 %172)
  %173 = load i16, ptr %0
  %174 = sdiv i16 %173, 15
  tail call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 %174, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 1, i16 1, i16 1, i16 4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string221)
  br label %b302

b302:
  %175 = phi i16 [ 0, %b297 ], [ %186, %b308 ]
  %176 = lshr i16 %175, 3
  %177 = sitofp i16 %175 to float
  %178 = fdiv float %177, 4.500000e+01
  %179 = tail call float @llvm.fabs.f32(float %178)
  %180 = tail call float @llvm.sin.f32(float %178)
  %181 = add nuw nsw i16 %175, 100
  br label %b307

b307:
  %182 = phi i16 [ 0, %b302 ], [ %204, %b312 ]
  %183 = fcmp ogt float %179, 0x41E0000000000000
  %184 = lshr i16 %182, 3
  %185 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$PNI2(i16 %184, i16 %176)
  br i1 %183, label %b309, label %b310

b308:
  %186 = add nuw nsw i16 %175, 1
  %exitcond1 = icmp eq i16 %186, 64
  br i1 %exitcond1, label %b315, label %b302

b309:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b310:
  %187 = uitofp nneg i16 %182 to float
  %188 = fdiv float %187, 4.500000e+01
  %189 = tail call float @llvm.fabs.f32(float %188)
  %190 = fcmp ogt float %189, 0x41E0000000000000
  br i1 %190, label %b311, label %b312

b311:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b312:
  %191 = load i16, ptr %0
  %192 = sitofp i16 %191 to float
  %193 = fmul float %180, %192
  %194 = tail call float @llvm.cos.f32(float %188)
  %195 = fmul float %194, %192
  %196 = fadd float %193, %195
  %197 = shl i16 %191, 1
  %198 = sitofp i16 %197 to float
  %199 = fadd float %196, %198
  %200 = sitofp i16 %185 to float
  %201 = fmul float %199, %200
  %202 = tail call i16 @llvm.lrint.i16.f32(float %201)
  %203 = add i16 %182, 40
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %203, i16 %181)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PSTC(i16 %202)
  %204 = add i16 %203, -39
  %exitcond = icmp eq i16 %204, 256
  br i1 %exitcond, label %b308, label %b307

b315:
  %205 = load i16, ptr %0
  tail call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 %205, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 22, i16 1, i16 10, i16 4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string222)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string223)
  br label %b2
}

define cc 1000 void @DELAY(ptr dereferenceable(2) nocapture readnone %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(none) willreturn nounwind nofree nosync norecurse {
b1:
  ret void
}

define cc 1000 void @FADEFROMCOLOR(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1, ptr dereferenceable(2) nocapture readonly %2, ptr dereferenceable(2) nocapture readonly %3, ptr dereferenceable(2) nocapture readonly %4, ptr dereferenceable(2) nocapture readonly %5) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree norecurse memory(readwrite, argmem: read) {
b1:
  %6 = load i16, ptr %2
  %7 = icmp slt i16 %6, 1
  br i1 %7, label %b2, label %8

8:
  %9 = load i16, ptr %0
  %10 = trunc i16 %9 to i8
  %11 = load i16, ptr %1
  %12 = load i16, ptr %3
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %14 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %15 = inttoptr i16 %13 to ptr addrspace(2)
  %16 = addrspacecast ptr addrspace(2) %15 to ptr addrspace(1)
  %17 = uitofp nneg i16 %6 to float
  %18 = sitofp i16 %12 to float
  %19 = load i16, ptr %4
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %21 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %22 = inttoptr i16 %20 to ptr addrspace(2)
  %23 = addrspacecast ptr addrspace(2) %22 to ptr addrspace(1)
  %24 = sitofp i16 %19 to float
  %25 = load i16, ptr %5
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %28 = inttoptr i16 %26 to ptr addrspace(2)
  %29 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %30 = sitofp i16 %25 to float
  %31 = load i16, ptr @"XIT%", !tbaa !2
  %32 = add i16 %11, 1
  %33 = sub i16 %32, %9
  %34 = getelementptr i8, ptr addrspace(1) %29, i16 %27
  %35 = shl i16 %9, 1
  %36 = getelementptr i8, ptr addrspace(1) %23, i16 %21
  %37 = getelementptr i8, ptr addrspace(1) %16, i16 %14
  br label %b6

b2:
  ret void

b3:
  %38 = icmp eq i16 %41, %6
  br i1 %38, label %b2, label %39

39:
  %40 = add nuw i16 %41, 1
  br label %b6

b6:
  %41 = phi i16 [ 1, %8 ], [ %40, %39 ]
  %42 = icmp sgt i16 %9, %11
  tail call void @llrm.ia16.out.i8(i16 968, i8 %10) memory(inaccessiblemem: readwrite)
  br i1 %42, label %b12, label %43

43:
  %44 = sitofp i16 %41 to float
  br label %b11

b11:
  %45 = phi i16 [ %75, %b11 ], [ %35, %43 ]
  %46 = phi i16 [ %74, %b11 ], [ %33, %43 ]
  %47 = getelementptr i8, ptr addrspace(1) %37, i16 %45
  %48 = load i16, ptr addrspace(1) %47, !tbaa !60
  %49 = sub i16 %48, %12
  %50 = sitofp i16 %49 to float
  %51 = fdiv float %50, %17
  %52 = fmul float %51, %44
  %53 = fadd float %52, %18
  %54 = tail call i16 @llvm.lrint.i16.f32(float %53)
  %55 = trunc i16 %54 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %55) memory(inaccessiblemem: readwrite)
  %56 = getelementptr i8, ptr addrspace(1) %36, i16 %45
  %57 = load i16, ptr addrspace(1) %56, !tbaa !62
  %58 = sub i16 %57, %19
  %59 = sitofp i16 %58 to float
  %60 = fdiv float %59, %17
  %61 = fmul float %60, %44
  %62 = fadd float %61, %24
  %63 = tail call i16 @llvm.lrint.i16.f32(float %62)
  %64 = trunc i16 %63 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %64) memory(inaccessiblemem: readwrite)
  %65 = getelementptr i8, ptr addrspace(1) %34, i16 %45
  %66 = load i16, ptr addrspace(1) %65, !tbaa !64
  %67 = sub i16 %66, %25
  %68 = sitofp i16 %67 to float
  %69 = fdiv float %68, %17
  %70 = fmul float %69, %44
  %71 = fadd float %70, %30
  %72 = tail call i16 @llvm.lrint.i16.f32(float %71)
  %73 = trunc i16 %72 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %73) memory(inaccessiblemem: readwrite)
  %74 = add i16 %46, -1
  %75 = add i16 %45, 2
  %76 = icmp eq i16 %74, 0
  br i1 %76, label %b12, label %b11

b12:
  %77 = icmp eq i16 %31, 0
  br i1 %77, label %78, label %b15

78:
  %79 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %80 = icmp eq i8 %79, 1
  br i1 %80, label %b15.thread, label %b3

b15.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b2

b15:
  %81 = icmp eq i16 %31, 1
  %82 = icmp eq i16 %41, %6
  %or.cond = select i1 %81, i1 true, i1 %82
  br i1 %or.cond, label %b2, label %39
}

define cc 1000 void @FADETOCOLOR(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1, ptr dereferenceable(2) nocapture readonly %2, ptr dereferenceable(2) nocapture readonly %3, ptr dereferenceable(2) nocapture readonly %4, ptr dereferenceable(2) nocapture readonly %5) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree norecurse memory(readwrite, argmem: read) {
b1:
  %6 = load i16, ptr %2
  %7 = icmp slt i16 %6, 1
  br i1 %7, label %b2, label %8

8:
  %9 = load i16, ptr %0
  %10 = trunc i16 %9 to i8
  %11 = load i16, ptr %1
  %12 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %14 = inttoptr i16 %12 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = load i16, ptr %3
  %17 = uitofp nneg i16 %6 to float
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %20 = inttoptr i16 %18 to ptr addrspace(2)
  %21 = addrspacecast ptr addrspace(2) %20 to ptr addrspace(1)
  %22 = load i16, ptr %4
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %24 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %25 = inttoptr i16 %23 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = load i16, ptr %5
  %28 = load i16, ptr @"XIT%", !tbaa !2
  %29 = add i16 %11, 1
  %30 = sub i16 %29, %9
  %31 = getelementptr i8, ptr addrspace(1) %26, i16 %24
  %32 = shl i16 %9, 1
  %33 = getelementptr i8, ptr addrspace(1) %21, i16 %19
  %34 = getelementptr i8, ptr addrspace(1) %15, i16 %13
  br label %b6

b2:
  ret void

b3:
  %35 = icmp eq i16 %38, %6
  br i1 %35, label %b2, label %36

36:
  %37 = add nuw i16 %38, 1
  br label %b6

b6:
  %38 = phi i16 [ 1, %8 ], [ %37, %36 ]
  %39 = icmp sgt i16 %9, %11
  tail call void @llrm.ia16.out.i8(i16 968, i8 %10) memory(inaccessiblemem: readwrite)
  br i1 %39, label %b12, label %40

40:
  %41 = sitofp i16 %38 to float
  br label %b11

b11:
  %42 = phi i16 [ %75, %b11 ], [ %32, %40 ]
  %43 = phi i16 [ %74, %b11 ], [ %30, %40 ]
  %44 = getelementptr i8, ptr addrspace(1) %34, i16 %42
  %45 = load i16, ptr addrspace(1) %44, !tbaa !60
  %46 = sub i16 %16, %45
  %47 = sitofp i16 %46 to float
  %48 = fdiv float %47, %17
  %49 = fmul float %48, %41
  %50 = sitofp i16 %45 to float
  %51 = fadd float %49, %50
  %52 = tail call i16 @llvm.lrint.i16.f32(float %51)
  %53 = trunc i16 %52 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %53) memory(inaccessiblemem: readwrite)
  %54 = getelementptr i8, ptr addrspace(1) %33, i16 %42
  %55 = load i16, ptr addrspace(1) %54, !tbaa !62
  %56 = sub i16 %22, %55
  %57 = sitofp i16 %56 to float
  %58 = fdiv float %57, %17
  %59 = fmul float %58, %41
  %60 = sitofp i16 %55 to float
  %61 = fadd float %59, %60
  %62 = tail call i16 @llvm.lrint.i16.f32(float %61)
  %63 = trunc i16 %62 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %63) memory(inaccessiblemem: readwrite)
  %64 = getelementptr i8, ptr addrspace(1) %31, i16 %42
  %65 = load i16, ptr addrspace(1) %64, !tbaa !64
  %66 = sub i16 %27, %65
  %67 = sitofp i16 %66 to float
  %68 = fdiv float %67, %17
  %69 = fmul float %68, %41
  %70 = sitofp i16 %65 to float
  %71 = fadd float %69, %70
  %72 = tail call i16 @llvm.lrint.i16.f32(float %71)
  %73 = trunc i16 %72 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %73) memory(inaccessiblemem: readwrite)
  %74 = add i16 %43, -1
  %75 = add i16 %42, 2
  %76 = icmp eq i16 %74, 0
  br i1 %76, label %b12, label %b11

b12:
  %77 = icmp eq i16 %28, 0
  br i1 %77, label %78, label %b15

78:
  %79 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %80 = icmp eq i8 %79, 1
  br i1 %80, label %b15.thread, label %b3

b15.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b2

b15:
  %81 = icmp eq i16 %28, 1
  %82 = icmp eq i16 %38, %6
  %or.cond = select i1 %81, i1 true, i1 %82
  br i1 %or.cond, label %b2, label %36
}

define cc 1000 void @GETMAP(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  tail call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr nonnull %0, i16 1, i16 -1, i16 32)
  %2 = addrspacecast ptr %0 to ptr addrspace(1)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 1, ptr addrspace(1) %2, i16 0)
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 2
  %data2 = load ptr, ptr %3
  %byte2 = load i8, ptr %data2
  %4 = zext i8 %byte2 to i16
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 2, ptr addrspace(1) %2, i16 0)
  %data1 = load ptr, ptr %3
  %byte1 = load i8, ptr %data1
  %5 = zext i8 %byte1 to i16
  %6 = icmp ne i16 %5, 0
  %7 = icmp ne i16 %4, 0
  %or.cond = select i1 %6, i1 %7, i1 false
  br i1 %or.cond, label %8, label %b6

8:
  br label %b5

b5:
  %9 = phi ptr [ %24, %22 ], [ @"CD%", %8 ]
  %10 = phi i16 [ 0, %8 ], [ %23, %22 ]
  %11 = phi i16 [ 2, %8 ], [ %16, %22 ]
  br label %12

b6:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 0)
  ret void

12:
  %13 = phi ptr [ %21, %12 ], [ %9, %b5 ]
  %14 = phi i16 [ %20, %12 ], [ %4, %b5 ]
  %15 = phi i16 [ %11, %b5 ], [ %16, %12 ]
  %16 = add i16 %15, 1
  %17 = sext i16 %16 to i32
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 %17, ptr addrspace(1) %2, i16 0)
  %18 = getelementptr i8, ptr %0, i16 2
  %data = load ptr, ptr %18
  %byte = load i8, ptr %data
  %19 = zext i8 %byte to i16
  store i16 %19, ptr %13, !tbaa !2
  %20 = add nsw i16 %14, -1
  %21 = getelementptr i8, ptr %13, i16 2
  %exitcond = icmp eq i16 %20, 0
  br i1 %exitcond, label %22, label %12

22:
  %23 = add nuw nsw i16 %10, 1
  %exitcond1 = icmp eq i16 %23, %5
  %24 = getelementptr i8, ptr %9, i16 64
  br i1 %exitcond1, label %b6, label %b5
}

define cc 1000 void @GETPAL() addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: write) nounwind nofree norecurse willreturn {
b1:
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %2 = inttoptr i16 %0 to ptr addrspace(2)
  %3 = addrspacecast ptr addrspace(2) %2 to ptr addrspace(1)
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %6 = inttoptr i16 %4 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 %5
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %11 = inttoptr i16 %9 to ptr addrspace(2)
  %12 = addrspacecast ptr addrspace(2) %11 to ptr addrspace(1)
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 %10
  %14 = getelementptr i8, ptr addrspace(1) %3, i16 %1
  br label %b5

b5:
  %15 = phi i16 [ %28, %b5 ], [ -512, %b1 ]
  %16 = getelementptr i8, ptr addrspace(1) %14, i16 %15
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 512
  %18 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %19 = zext i8 %18 to i16
  store i16 %19, ptr addrspace(1) %17, !tbaa !60
  %20 = getelementptr i8, ptr addrspace(1) %8, i16 %15
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 512
  %22 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %23 = zext i8 %22 to i16
  store i16 %23, ptr addrspace(1) %21, !tbaa !62
  %24 = getelementptr i8, ptr addrspace(1) %13, i16 %15
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 512
  %26 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %27 = zext i8 %26 to i16
  store i16 %27, ptr addrspace(1) %25, !tbaa !64
  %28 = add nsw i16 %15, 2
  %exitcond = icmp eq i16 %28, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @INITCROSFADEPICS(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1) addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %3, i8 0, i16 4, i1 false)
  br label %b10

b10:
  %4 = phi i16 [ %98, %b16 ], [ 0, %b1 ]
  %5 = phi i16 [ 0, %b1 ], [ %10, %b16 ]
  %6 = load i16, ptr %0
  %7 = shl i16 %6, 2
  %8 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %9 = getelementptr i8, ptr %8, i16 %7
  %10 = add nuw nsw i16 %5, 1
  %11 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %9, i16 %10, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %11, ptr nonnull %3)
  %12 = load i16, ptr %1
  %13 = shl i16 %12, 2
  %14 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %15 = getelementptr i8, ptr %14, i16 %13
  %16 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %15, i16 %10, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %16, ptr nonnull %2)
  br label %b15

17:
  %18 = phi i16 [ 0, %99 ], [ %91, %89 ]
  %19 = phi i16 [ %25, %89 ], [ 0, %99 ]
  %20 = load i16, ptr %0
  %21 = shl i16 %20, 2
  %22 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %23 = getelementptr i8, ptr %22, i16 %21
  %24 = getelementptr i8, ptr %23, i16 160
  %25 = add nuw nsw i16 %19, 1
  %26 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %24, i16 %25, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %26, ptr nonnull %3)
  %27 = load i16, ptr %1
  %28 = shl i16 %27, 2
  %29 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @TEXT$, i16 10), !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 %28
  %31 = getelementptr i8, ptr %30, i16 160
  %32 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %31, i16 %25, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %32, ptr nonnull %2)
  br label %33

33:
  %34 = phi i16 [ %88, %85 ], [ 0, %17 ]
  %35 = phi i16 [ 0, %17 ], [ %38, %85 ]
  %36 = phi i16 [ 0, %17 ], [ %86, %85 ]
  %37 = add nuw nsw i16 %35, -21376
  store i16 %37, ptr @b$seg, !tbaa !2
  %38 = add nuw nsw i16 %35, 20
  br label %39

39:
  %40 = phi i16 [ %83, %39 ], [ %34, %33 ]
  %41 = phi i16 [ %82, %39 ], [ %18, %33 ]
  %42 = phi i16 [ %81, %39 ], [ 16, %33 ]
  %43 = getelementptr i8, ptr %3, i16 2
  %44 = load ptr, ptr %43
  %45 = load i8, ptr %44
  %46 = zext i8 %45 to i16
  %47 = shl nuw nsw i16 %46, 1
  %48 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FPOS%", i16 2), !tbaa !2
  %49 = inttoptr i16 %48 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = getelementptr i8, ptr addrspace(1) %50, i16 %47
  %52 = load i16, ptr addrspace(1) %51, !tbaa !6
  %53 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FONTS%", i16 2), !tbaa !2
  %54 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FONTS%", i16 10), !tbaa !2
  %55 = inttoptr i16 %53 to ptr addrspace(2)
  %56 = addrspacecast ptr addrspace(2) %55 to ptr addrspace(1)
  %57 = shl i16 %52, 9
  %58 = add i16 %54, %57
  %59 = getelementptr i8, ptr addrspace(1) %56, i16 %58
  %60 = getelementptr i8, ptr addrspace(1) %59, i16 %40
  %61 = load i16, ptr addrspace(1) %60, !tbaa !10
  %62 = getelementptr i8, ptr %2, i16 2
  %63 = load ptr, ptr %62
  %64 = load i8, ptr %63
  %65 = zext i8 %64 to i16
  %66 = shl nuw nsw i16 %65, 1
  %67 = getelementptr i8, ptr addrspace(1) %50, i16 %66
  %68 = load i16, ptr addrspace(1) %67, !tbaa !6
  %69 = shl i16 %68, 9
  %70 = add i16 %54, %69
  %71 = getelementptr i8, ptr addrspace(1) %56, i16 %70
  %72 = getelementptr i8, ptr addrspace(1) %71, i16 %40
  %73 = load i16, ptr addrspace(1) %72, !tbaa !10
  %74 = shl i16 %73, 4
  %75 = or i16 %74, %61
  %76 = trunc i16 %75 to i8
  %77 = load i16, ptr @b$seg, !tbaa !2
  %78 = inttoptr i16 %77 to ptr addrspace(2)
  %79 = addrspacecast ptr addrspace(2) %78 to ptr addrspace(1)
  %80 = getelementptr i8, ptr addrspace(1) %79, i16 %41
  store volatile i8 %76, ptr addrspace(1) %80
  %81 = add nsw i16 %42, -1
  %82 = add nuw nsw i16 %41, 1
  %83 = add nuw nsw i16 %40, 2
  %84 = icmp eq i16 %81, 0
  br i1 %84, label %85, label %39

85:
  %86 = add nuw nsw i16 %36, 1
  %87 = icmp eq i16 %86, 16
  %88 = add nuw nsw i16 %34, 32
  br i1 %87, label %89, label %33

89:
  %90 = icmp eq i16 %25, 20
  %91 = add nuw nsw i16 %18, 16
  br i1 %90, label %92, label %17

92:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %2)
  ret void

b15:
  %93 = phi i16 [ %142, %b21 ], [ 0, %b10 ]
  %94 = phi i16 [ 0, %b10 ], [ %97, %b21 ]
  %95 = phi i16 [ 0, %b10 ], [ %141, %b21 ]
  %96 = add nuw nsw i16 %94, -24576
  store i16 %96, ptr @b$seg, !tbaa !2
  %97 = add nuw nsw i16 %94, 20
  br label %100

b16:
  %exitcond2 = icmp eq i16 %10, 20
  %98 = add nuw nsw i16 %4, 16
  br i1 %exitcond2, label %99, label %b10

99:
  br label %17

100:
  %101 = phi i16 [ %140, %100 ], [ %93, %b15 ]
  %102 = phi i16 [ %139, %100 ], [ %4, %b15 ]
  %103 = phi i16 [ %138, %100 ], [ 16, %b15 ]
  %104 = getelementptr i8, ptr %3, i16 2
  %data1 = load ptr, ptr %104
  %byte1 = load i8, ptr %data1
  %105 = zext i8 %byte1 to i16
  %106 = shl nuw nsw i16 %105, 1
  %107 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FPOS%", i16 2), !tbaa !2
  %108 = inttoptr i16 %107 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 %106
  %111 = load i16, ptr addrspace(1) %110, !tbaa !6
  %112 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FONTS%", i16 2), !tbaa !2
  %113 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FONTS%", i16 10), !tbaa !2
  %114 = inttoptr i16 %112 to ptr addrspace(2)
  %115 = addrspacecast ptr addrspace(2) %114 to ptr addrspace(1)
  %116 = shl i16 %111, 9
  %117 = add i16 %113, %116
  %118 = getelementptr i8, ptr addrspace(1) %115, i16 %117
  %119 = getelementptr i8, ptr addrspace(1) %118, i16 %101
  %120 = load i16, ptr addrspace(1) %119, !tbaa !10
  %121 = getelementptr i8, ptr %2, i16 2
  %data = load ptr, ptr %121
  %byte = load i8, ptr %data
  %122 = zext i8 %byte to i16
  %123 = shl nuw nsw i16 %122, 1
  %124 = getelementptr i8, ptr addrspace(1) %109, i16 %123
  %125 = load i16, ptr addrspace(1) %124, !tbaa !6
  %126 = shl i16 %125, 9
  %127 = add i16 %113, %126
  %128 = getelementptr i8, ptr addrspace(1) %115, i16 %127
  %129 = getelementptr i8, ptr addrspace(1) %128, i16 %101
  %130 = load i16, ptr addrspace(1) %129, !tbaa !10
  %131 = shl i16 %130, 4
  %132 = or i16 %131, %120
  %133 = trunc i16 %132 to i8
  %134 = load i16, ptr @b$seg, !tbaa !2
  %135 = inttoptr i16 %134 to ptr addrspace(2)
  %136 = addrspacecast ptr addrspace(2) %135 to ptr addrspace(1)
  %137 = getelementptr i8, ptr addrspace(1) %136, i16 %102
  store volatile i8 %133, ptr addrspace(1) %137
  %138 = add nsw i16 %103, -1
  %139 = add nuw nsw i16 %102, 1
  %140 = add nuw nsw i16 %101, 2
  %exitcond = icmp eq i16 %138, 0
  br i1 %exitcond, label %b21, label %100

b21:
  %141 = add nuw nsw i16 %95, 1
  %exitcond1 = icmp eq i16 %141, 16
  %142 = add nuw nsw i16 %93, 32
  br i1 %exitcond1, label %b16, label %b15
}

define cc 1000 void @INTERSECTIONS() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = load i16, ptr @"PA%", !tbaa !2
  %1 = sitofp i16 %0 to float
  %2 = fadd float %1, 0x40401999A0000000
  %3 = fpround float %2 to float
  %4 = fcmp ogt float %3, 3.600000e+02
  br i1 %4, label %b5, label %b7

b2:
  %5 = phi i16 [ %68, %b63 ], [ %68, %b59 ], [ %68, %b67 ], [ %44, %b11 ], [ %68, %b27 ]
  %6 = phi i16 [ %102, %b63 ], [ %102, %b59 ], [ %102, %b67 ], [ %43, %b11 ], [ %79, %b27 ]
  %7 = phi i16 [ %109, %b67 ], [ %101, %b59 ], [ %109, %b63 ], [ %42, %b11 ], [ %68, %b27 ]
  %8 = phi i16 [ %93, %b63 ], [ %93, %b59 ], [ %93, %b67 ], [ %41, %b11 ], [ %41, %b27 ]
  %9 = icmp eq i16 %51, 90
  %10 = sext i1 %9 to i16
  %11 = icmp eq i16 %51, 270
  %12 = sext i1 %11 to i16
  %13 = or i16 %10, %12
  %14 = icmp eq i16 %13, 0
  br i1 %14, label %b83, label %b3

b3:
  %15 = phi i16 [ %5, %b2 ], [ %154, %b92 ], [ %154, %b118 ]
  %16 = phi i16 [ %6, %b2 ], [ %143, %b92 ], [ %175, %b118 ]
  %17 = phi i16 [ %7, %b2 ], [ %154, %b92 ], [ %176, %b118 ]
  %18 = phi i16 [ %8, %b2 ], [ %8, %b92 ], [ %172, %b118 ]
  %19 = fsub float %1, %50
  %20 = fdiv float %19, 0x404CA66660000000
  %21 = tail call float @llvm.fabs.f32(float %20)
  %22 = fcmp ogt float %21, 0x41E0000000000000
  br i1 %22, label %b129, label %b130

b5:
  %23 = sub i16 360, %0
  %24 = sitofp i16 %23 to float
  %25 = fsub float 0x40401999A0000000, %24
  %26 = fpround float %25 to float
  br label %b7

b7:
  %27 = phi float [ %26, %b5 ], [ %3, %b1 ]
  %28 = load float, ptr @"RA!", !tbaa !2
  %29 = load i16, ptr @"PY%", !tbaa !2
  %30 = sdiv i16 %29, 64
  %31 = shl nsw i16 %30, 6
  %32 = add i16 %31, 64
  %33 = add i16 %31, -1
  %34 = load i16, ptr @"PX%", !tbaa !2
  %35 = sitofp i16 %34 to float
  %36 = sdiv i16 %34, 64
  %37 = shl nsw i16 %36, 6
  %38 = sitofp i16 %29 to float
  %39 = load i16, ptr @"DPP%", !tbaa !2
  %40 = sitofp i16 %39 to float
  br label %b11

b11:
  %41 = phi i16 [ 0, %b7 ], [ %18, %b130 ]
  %42 = phi i16 [ 0, %b7 ], [ %17, %b130 ]
  %43 = phi i16 [ 0, %b7 ], [ %16, %b130 ]
  %44 = phi i16 [ 0, %b7 ], [ %15, %b130 ]
  %45 = phi float [ %27, %b7 ], [ %50, %b130 ]
  %46 = phi i16 [ 0, %b7 ], [ %246, %b130 ]
  %47 = fsub float %45, %28
  %48 = fpround float %47 to float
  %49 = fcmp olt float %48, 0.000000e+00
  %50 = select i1 %49, float 0x40766CCCC0000000, float %48
  %51 = tail call i16 @llvm.lrint.i16.f32(float %50)
  %52 = icmp eq i16 %51, 0
  %53 = sext i1 %52 to i16
  %54 = icmp eq i16 %51, 180
  %55 = sext i1 %54 to i16
  %56 = or i16 %53, %55
  %57 = icmp eq i16 %51, 360
  %58 = sext i1 %57 to i16
  %59 = or i16 %56, %58
  %60 = icmp eq i16 %59, 0
  br i1 %60, label %b18, label %b2

b12:
  ret void

b18:
  %61 = icmp sgt i16 %51, 0
  %62 = icmp slt i16 %51, 180
  %63 = and i1 %61, %62
  %64 = icmp sgt i16 %51, 180
  %65 = icmp slt i16 %51, 360
  %66 = and i1 %64, %65
  %67 = select i1 %66, i16 %32, i16 %44
  %68 = select i1 %63, i16 %33, i16 %67
  %69 = fdiv float %50, 0x404CA66660000000
  %70 = tail call float @llvm.fabs.f32(float %69)
  %71 = fcmp ogt float %70, 0x41E0000000000000
  br i1 %71, label %b26, label %b27

b26:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b27:
  %72 = sub i16 %29, %68
  %73 = tail call float @llvm.sin.f32(float %69)
  %74 = tail call float @llvm.cos.f32(float %69)
  %75 = fdiv float %73, %74
  %76 = sitofp i16 %72 to float
  %77 = fdiv float %76, %75
  %78 = fadd float %77, %35
  %79 = tail call i16 @llvm.lrint.i16.f32(float %78)
  %80 = sdiv i16 %79, 64
  %81 = sdiv i16 %68, 64
  %82 = tail call i16 @llvm.smax.i16(i16 %80, i16 0)
  %83 = tail call i16 @llvm.umin.i16(i16 %82, i16 31)
  %84 = tail call i16 @llvm.smax.i16(i16 %81, i16 0)
  %85 = tail call i16 @llvm.umin.i16(i16 %84, i16 31)
  %86 = getelementptr inbounds nuw [2 x i8], ptr @"MUL32%", i16 %85
  %87 = load i16, ptr %86, !tbaa !2
  %88 = getelementptr [2 x i8], ptr @"CD%", i16 %87
  %89 = getelementptr [2 x i8], ptr %88, i16 %83
  %90 = load i16, ptr %89, !tbaa !2
  %91 = icmp eq i16 %90, 1
  br i1 %91, label %b2, label %b51

b51:
  %92 = select i1 %66, i16 64, i16 %41
  %93 = select i1 %63, i16 -64, i16 %92
  %94 = sub i16 0, %93
  %95 = sitofp i16 %94 to float
  %96 = fdiv float %95, %75
  %97 = tail call i16 @llvm.lrint.i16.f32(float %96)
  %98 = tail call i16 @llvm.smin.i16(i16 %97, i16 2048)
  %99 = tail call i16 @llvm.smax.i16(i16 %98, i16 -2048)
  br label %b59

b59:
  %100 = phi i16 [ %79, %b51 ], [ %102, %b67 ]
  %101 = phi i16 [ %68, %b51 ], [ %109, %b67 ]
  %102 = add i16 %100, %99
  %103 = icmp sgt i16 %102, 2048
  %104 = sext i1 %103 to i16
  %105 = icmp slt i16 %102, -2048
  %106 = sext i1 %105 to i16
  %107 = or i16 %104, %106
  %108 = icmp eq i16 %107, 0
  br i1 %108, label %b63, label %b2

b63:
  %109 = add i16 %101, %93
  %110 = icmp sgt i16 %109, 2048
  %111 = sext i1 %110 to i16
  %112 = icmp slt i16 %109, -2048
  %113 = sext i1 %112 to i16
  %114 = or i16 %111, %113
  %115 = icmp eq i16 %114, 0
  br i1 %115, label %b67, label %b2

b67:
  %116 = sdiv i16 %102, 64
  %117 = sdiv i16 %109, 64
  %118 = tail call i16 @llvm.smax.i16(i16 %116, i16 0)
  %119 = tail call i16 @llvm.umin.i16(i16 %118, i16 31)
  %120 = tail call i16 @llvm.smax.i16(i16 %117, i16 0)
  %121 = tail call i16 @llvm.umin.i16(i16 %120, i16 31)
  %122 = getelementptr inbounds nuw [2 x i8], ptr @"MUL32%", i16 %121
  %123 = load i16, ptr %122, !tbaa !2
  %124 = getelementptr [2 x i8], ptr @"CD%", i16 %123
  %125 = getelementptr [2 x i8], ptr %124, i16 %119
  %126 = load i16, ptr %125, !tbaa !2
  %127 = icmp eq i16 %126, 1
  %128 = sext i1 %127 to i16
  %129 = icmp eq i16 %119, 31
  %130 = sext i1 %129 to i16
  %131 = icmp eq i16 %121, 31
  %132 = sext i1 %131 to i16
  %133 = or i16 %132, %130
  %134 = or i16 %133, %128
  %135 = icmp eq i16 %134, 0
  br i1 %135, label %b59, label %b2

b83:
  %136 = icmp sgt i16 %51, 270
  %137 = sext i1 %136 to i16
  %138 = icmp slt i16 %51, 90
  %139 = sext i1 %138 to i16
  %140 = or i16 %137, %139
  %141 = icmp eq i16 %140, 0
  %142 = select i1 %141, i16 -1, i16 64
  %143 = add i16 %37, %142
  %144 = fdiv float %50, 0x404CA66660000000
  %145 = tail call float @llvm.fabs.f32(float %144)
  %146 = fcmp ogt float %145, 0x41E0000000000000
  br i1 %146, label %b91, label %b92

b91:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b92:
  %147 = sub i16 %143, %34
  %148 = tail call float @llvm.sin.f32(float %144)
  %149 = tail call float @llvm.cos.f32(float %144)
  %150 = fdiv float %148, %149
  %151 = sitofp i16 %147 to float
  %152 = fmul float %150, %151
  %153 = fsub float %38, %152
  %154 = tail call i16 @llvm.lrint.i16.f32(float %153)
  %155 = sdiv i16 %143, 64
  %156 = sdiv i16 %154, 64
  %157 = tail call i16 @llvm.smax.i16(i16 %155, i16 0)
  %158 = tail call i16 @llvm.umin.i16(i16 %157, i16 31)
  %159 = tail call i16 @llvm.smax.i16(i16 %156, i16 0)
  %160 = tail call i16 @llvm.umin.i16(i16 %159, i16 31)
  %161 = getelementptr inbounds nuw [2 x i8], ptr @"MUL32%", i16 %160
  %162 = load i16, ptr %161, !tbaa !2
  %163 = getelementptr [2 x i8], ptr @"CD%", i16 %162
  %164 = getelementptr [2 x i8], ptr %163, i16 %158
  %165 = load i16, ptr %164, !tbaa !2
  %166 = icmp eq i16 %165, 1
  br i1 %166, label %b3, label %b116

b116:
  %167 = icmp eq i16 %140, 0
  %168 = select i1 %167, i16 -64, i16 64
  %169 = select i1 %167, i16 64, i16 -64
  %170 = sitofp i16 %169 to float
  %171 = fmul float %150, %170
  %172 = tail call i16 @llvm.lrint.i16.f32(float %171)
  br label %b118

b118:
  %173 = phi i16 [ %143, %b116 ], [ %175, %b118 ]
  %174 = phi i16 [ %154, %b116 ], [ %176, %b118 ]
  %175 = add i16 %173, %168
  %176 = add i16 %174, %172
  %177 = sdiv i16 %175, 64
  %178 = sdiv i16 %176, 64
  %179 = icmp slt i16 %175, -63
  %180 = sext i1 %179 to i16
  %181 = icmp sgt i16 %177, 31
  %182 = sext i1 %181 to i16
  %183 = or i16 %182, %180
  %184 = icmp eq i16 %183, 0
  %spec.store.select14 = select i1 %184, i16 %177, i16 31
  %185 = icmp slt i16 %176, -63
  %186 = sext i1 %185 to i16
  %187 = icmp sgt i16 %178, 31
  %188 = sext i1 %187 to i16
  %189 = or i16 %188, %186
  %190 = icmp eq i16 %189, 0
  %spec.store.select15 = select i1 %190, i16 %178, i16 31
  %191 = getelementptr inbounds [2 x i8], ptr @"MUL32%", i16 %spec.store.select15
  %192 = load i16, ptr %191, !tbaa !2
  %193 = getelementptr [2 x i8], ptr @"CD%", i16 %192
  %194 = getelementptr [2 x i8], ptr %193, i16 %spec.store.select14
  %195 = load i16, ptr %194, !tbaa !2
  %196 = icmp eq i16 %195, 1
  %197 = sext i1 %196 to i16
  %198 = icmp eq i16 %spec.store.select14, 31
  %199 = sext i1 %198 to i16
  %200 = or i16 %199, %197
  %201 = icmp eq i16 %spec.store.select15, 31
  %202 = sext i1 %201 to i16
  %203 = or i16 %200, %202
  %204 = icmp eq i16 %203, 0
  br i1 %204, label %b118, label %b3

b129:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b130:
  %205 = sub i16 %34, %6
  %206 = sitofp i16 %205 to float
  %207 = fpround float %206 to float
  %208 = fmul float %207, %207
  %209 = sub i16 %29, %7
  %210 = sitofp i16 %209 to float
  %211 = fpround float %210 to float
  %212 = fmul float %211, %211
  %213 = fadd float %208, %212
  %214 = tail call float @llvm.sqrt.f32(float %213)
  %215 = tail call i16 @llvm.lrint.i16.f32(float %214)
  %216 = sub i16 %34, %16
  %217 = sitofp i16 %216 to float
  %218 = fpround float %217 to float
  %219 = fmul float %218, %218
  %220 = sub i16 %29, %17
  %221 = sitofp i16 %220 to float
  %222 = fpround float %221 to float
  %223 = fmul float %222, %222
  %224 = fadd float %219, %223
  %225 = tail call float @llvm.sqrt.f32(float %224)
  %226 = tail call i16 @llvm.lrint.i16.f32(float %225)
  %227 = tail call i16 @llvm.smin.i16(i16 %215, i16 %226)
  %228 = tail call float @llvm.cos.f32(float %20)
  %229 = sitofp i16 %227 to float
  %230 = fmul float %228, %229
  %231 = tail call i16 @llvm.lrint.i16.f32(float %230)
  %232 = sitofp i16 %231 to float
  %233 = fdiv float %40, %232
  %234 = fmul float %233, 6.400000e+01
  %235 = tail call i16 @llvm.lrint.i16.f32(float %234)
  %236 = tail call i16 @llvm.smin.i16(i16 %235, i16 200)
  %237 = sdiv i16 %231, -2
  %238 = add nsw i16 %237, 254
  %239 = getelementptr inbounds nuw [2 x i8], ptr @"SC%", i16 %46
  %240 = tail call i16 @llvm.smax.i16(i16 %238, i16 0)
  store i16 %240, ptr %239
  %241 = sub i16 200, %236
  %242 = sdiv i16 %241, 2
  %243 = getelementptr inbounds nuw [2 x i8], ptr @"SLA%", i16 %46
  store i16 %242, ptr %243, !tbaa !2
  %244 = add i16 %242, %236
  %245 = getelementptr inbounds nuw [2 x i8], ptr @"SLB%", i16 %46
  store i16 %244, ptr %245, !tbaa !2
  %246 = add nuw nsw i16 %46, 1
  %exitcond = icmp eq i16 %246, 320
  br i1 %exitcond, label %b12, label %b11
}

define cc 1000 void @KEYACTION() addrspace(1) "llrm-fp-eval"="x87-extended" willreturn nounwind nofree norecurse memory(write, argmem: none, inaccessiblemem: readwrite) {
b1:
  %0 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %1 = icmp eq i8 %0, 1
  br i1 %1, label %b2, label %b4

b2:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b4

b4:
  ret void
}

define cc 1000 void @LOAD3DRECORD() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = load i16, ptr @"GGI%", !tbaa !2
  %1 = load i16, ptr @"GI%", !tbaa !2
  %2 = icmp slt i16 %0, %1
  br i1 %2, label %b2, label %4

b2:
  %3 = add nsw i16 %0, 1
  store i16 %3, ptr @"GGI%", !tbaa !2
  br label %b4

4:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GI$ to ptr addrspace(1)), i16 0)
  %data1 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GI$, i16 2)
  %byte1 = load i8, ptr %data1
  %5 = zext i8 %byte1 to i16
  store i16 %5, ptr @"GI%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GG$ to ptr addrspace(1)), i16 0)
  %data = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GG$, i16 2)
  %byte = load i8, ptr %data
  %6 = zext i8 %byte to i16
  store i16 %6, ptr @"GG%", !tbaa !2
  store i16 1, ptr @"GGI%", !tbaa !2
  %7 = load i16, ptr @"FILEI%", !tbaa !2
  %8 = add i16 %7, 2
  store i16 %8, ptr @"FILEI%", !tbaa !2
  br label %b4

b4:
  ret void
}

define cc 1000 void @LOADOBJECT(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  br label %2

2:
  %3 = phi i16 [ 0, %1 ], [ %15, %14 ]
  %4 = mul nuw nsw i16 %3, 20
  %5 = add nuw nsw i16 %4, -24576
  store i16 %5, ptr @b$seg, !tbaa !2
  br label %6

6:
  %7 = phi i16 [ %13, %6 ], [ -320, %2 ]
  %8 = load i16, ptr @b$seg, !tbaa !2
  %9 = inttoptr i16 %8 to ptr addrspace(2)
  %10 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 %7
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 320
  store volatile i8 0, ptr addrspace(1) %12
  %13 = add nsw i16 %7, 1
  %exitcond = icmp eq i16 %13, 0
  br i1 %exitcond, label %14, label %6

14:
  %15 = add nuw nsw i16 %3, 1
  %exitcond1 = icmp eq i16 %15, 200
  br i1 %exitcond1, label %16, label %2

16:
  store i16 -24576, ptr @b$seg, !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr nonnull %0, i16 1, i16 -1, i16 32)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) addrspacecast (ptr @"NDTS%" to ptr addrspace(1)), i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) addrspacecast (ptr @"NLNS%" to ptr addrspace(1)), i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) addrspacecast (ptr @"NPLS%" to ptr addrspace(1)), i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string226, ptr nonnull %0)
  %17 = load i16, ptr @"NDTS%", !tbaa !2
  %18 = add i16 %17, -1
  %19 = icmp slt i16 %18, 0
  br i1 %19, label %b6, label %20

20:
  %21 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %22

22:
  %23 = phi i16 [ %45, %22 ], [ 0, %20 ]
  %24 = phi i16 [ %44, %22 ], [ %17, %20 ]
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %21, i16 0)
  %25 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %26 = inttoptr i16 %25 to ptr addrspace(2)
  %27 = addrspacecast ptr addrspace(2) %26 to ptr addrspace(1)
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 %23
  %29 = getelementptr i8, ptr %0, i16 2
  %data2 = load ptr, ptr %29
  %byte2 = load i8, ptr %data2
  %30 = zext i8 %byte2 to i16
  %31 = add nsw i16 %30, -128
  store i16 %31, ptr addrspace(1) %28, !tbaa !16
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %21, i16 0)
  %32 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %33 = inttoptr i16 %32 to ptr addrspace(2)
  %34 = addrspacecast ptr addrspace(2) %33 to ptr addrspace(1)
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 %23
  %data1 = load ptr, ptr %29
  %byte1 = load i8, ptr %data1
  %36 = zext i8 %byte1 to i16
  %37 = add nsw i16 %36, -128
  store i16 %37, ptr addrspace(1) %35, !tbaa !20
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %21, i16 0)
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %39 = inttoptr i16 %38 to ptr addrspace(2)
  %40 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 %23
  %data = load ptr, ptr %29
  %byte = load i8, ptr %data
  %42 = zext i8 %byte to i16
  %43 = add nsw i16 %42, -128
  store i16 %43, ptr addrspace(1) %41, !tbaa !18
  %44 = add i16 %24, -1
  %45 = add i16 %23, 2
  %exitcond2 = icmp eq i16 %44, 0
  br i1 %exitcond2, label %b6, label %22

b6:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  ret void
}

define cc 1000 void @LOADQBINSIDE() addrspace(1) noinline "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull %0, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %1, i8 0, i16 4, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string227, ptr nonnull %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string228, ptr nonnull %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr nonnull %1, i16 1, i16 -1, i16 32)
  call void @llrm.ia16.out.i8(i16 968, i8 -128) memory(inaccessiblemem: readwrite)
  %2 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %3

3:
  %4 = phi i16 [ %6, %3 ], [ 384, %b1 ]
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %2, i16 0)
  %5 = getelementptr i8, ptr %0, i16 2
  %data1 = load ptr, ptr %5
  %byte1 = load i8, ptr %data1
  call void @llrm.ia16.out.i8(i16 969, i8 %byte1) memory(inaccessiblemem: readwrite)
  %6 = add nsw i16 %4, -1
  %exitcond2 = icmp eq i16 %6, 0
  br i1 %exitcond2, label %7, label %3

7:
  br label %b10

b10:
  %8 = phi i16 [ %22, %b16 ], [ 0, %7 ]
  %9 = phi i16 [ %21, %b16 ], [ 1, %7 ]
  %10 = shl i16 %8, 1
  %11 = sub i16 -100, %8
  br label %12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %0)
  ret void

12:
  %13 = phi i16 [ %19, %12 ], [ -200, %b10 ]
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %2, i16 0)
  %14 = getelementptr i8, ptr %0, i16 2
  %data = load ptr, ptr %14
  %byte = load i8, ptr %data
  %15 = zext i8 %byte to i16
  %16 = getelementptr i8, ptr @"SP%", i16 %10
  %17 = getelementptr i8, ptr %16, i16 %13
  %18 = getelementptr i8, ptr %17, i16 200
  store i16 %15, ptr %18, !tbaa !2
  %19 = add nsw i16 %13, 2
  %20 = icmp eq i16 %19, 0
  br i1 %20, label %b16, label %12

b16:
  %21 = add nuw nsw i16 %9, 1
  %exitcond1 = icmp eq i16 %21, 101
  %22 = mul i16 %11, -1
  br i1 %exitcond1, label %b11, label %b10
}

define cc 1000 void @LOADRECORD() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = load i16, ptr @"GGI%", !tbaa !2
  %1 = load i16, ptr @"GI%", !tbaa !2
  %2 = icmp slt i16 %0, %1
  br i1 %2, label %b2, label %5

b2:
  %3 = add nsw i16 %0, 1
  store i16 %3, ptr @"GGI%", !tbaa !2
  %4 = load i16, ptr @"GG%", !tbaa !2
  br label %b14

5:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GI$ to ptr addrspace(1)), i16 0)
  %data1 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GI$, i16 2)
  %byte1 = load i8, ptr %data1
  %6 = zext i8 %byte1 to i16
  store i16 %6, ptr @"GI%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 2, ptr addrspace(1) addrspacecast (ptr @GG$ to ptr addrspace(1)), i16 0)
  %data = load ptr, ptr getelementptr inbounds nuw (i8, ptr @GG$, i16 2)
  %byte = load i8, ptr %data
  %7 = zext i8 %byte to i16
  store i16 %7, ptr @"GG%", !tbaa !2
  store i16 1, ptr @"GGI%", !tbaa !2
  %8 = load i16, ptr @"FILEI%", !tbaa !2
  %9 = add i16 %8, 2
  store i16 %9, ptr @"FILEI%", !tbaa !2
  br label %b14

b14:
  %10 = phi i16 [ %4, %b2 ], [ %7, %5 ]
  %11 = tail call float @llvm.exp2.f32(float 3.000000e+00)
  %12 = sitofp i16 %10 to float
  %13 = fsub float %12, %11
  %14 = fcmp oge float %13, 0.000000e+00
  %15 = tail call i16 @llvm.lrint.i16.f32(float %13)
  %16 = zext i1 %14 to i16
  %17 = select i1 %14, i16 %15, i16 %10
  %18 = tail call float @llvm.exp2.f32(float 2.000000e+00)
  %19 = sitofp i16 %17 to float
  %20 = fsub float %19, %18
  %21 = fcmp oge float %20, 0.000000e+00
  %22 = tail call i16 @llvm.lrint.i16.f32(float %20)
  %23 = zext i1 %21 to i16
  %24 = select i1 %21, i16 %22, i16 %17
  %25 = tail call float @llvm.exp2.f32(float 1.000000e+00)
  %26 = sitofp i16 %24 to float
  %27 = fsub float %26, %25
  %28 = fcmp oge float %27, 0.000000e+00
  %29 = tail call i16 @llvm.lrint.i16.f32(float %27)
  %30 = zext i1 %28 to i16
  %31 = select i1 %28, i16 %29, i16 %24
  %32 = tail call float @llvm.exp2.f32(float 0.000000e+00)
  %33 = sitofp i16 %31 to float
  %34 = fsub float %33, %32
  %35 = fcmp oge float %34, 0.000000e+00
  %36 = zext i1 %35 to i16
  store i16 %10, ptr @"GG%", !tbaa !2
  store i16 %36, ptr @"KON%.3", !tbaa !2
  store i16 %30, ptr @"KON%.4", !tbaa !2
  store i16 %23, ptr @"KON%.2", !tbaa !2
  store i16 %16, ptr @"KON%.5", !tbaa !2
  ret void
}

define cc 1000 void @MOV3DPOS(ptr dereferenceable(4) nocapture readonly %0, ptr dereferenceable(4) nocapture readonly %1, ptr dereferenceable(4) nocapture readonly %2) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %3 = load i16, ptr @"NDTS%", !tbaa !2
  %4 = add i16 %3, -1
  %5 = icmp slt i16 %4, 0
  br i1 %5, label %b6, label %6

6:
  %7 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"X!", i16 2), !tbaa !2
  %8 = inttoptr i16 %7 to ptr addrspace(2)
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Y!", i16 2), !tbaa !2
  %10 = inttoptr i16 %9 to ptr addrspace(2)
  %11 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Z!", i16 2), !tbaa !2
  %12 = inttoptr i16 %11 to ptr addrspace(2)
  br label %b5

b5:
  %13 = phi i16 [ %34, %b5 ], [ 0, %6 ]
  %14 = phi i16 [ %33, %b5 ], [ %3, %6 ]
  %15 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %16 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %17 = addrspacecast ptr addrspace(2) %8 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 %13
  %19 = load float, ptr addrspace(1) %18, !tbaa !54
  %20 = load float, ptr %0
  %21 = fadd float %19, %20
  %22 = fpround float %21 to float
  store float %22, ptr addrspace(1) %18, !tbaa !54
  %23 = getelementptr i8, ptr addrspace(1) %16, i16 %13
  %24 = load float, ptr addrspace(1) %23, !tbaa !58
  %25 = load float, ptr %1
  %26 = fadd float %24, %25
  %27 = fpround float %26 to float
  store float %27, ptr addrspace(1) %23, !tbaa !58
  %28 = getelementptr i8, ptr addrspace(1) %15, i16 %13
  %29 = load float, ptr addrspace(1) %28, !tbaa !56
  %30 = load float, ptr %2
  %31 = fadd float %29, %30
  %32 = fpround float %31 to float
  store float %32, ptr addrspace(1) %28, !tbaa !56
  %33 = add i16 %14, -1
  %34 = add i16 %13, 4
  %exitcond = icmp eq i16 %33, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @OUTPUT0() addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree nosync norecurse willreturn {
b1:
  %0 = load i16, ptr @"MAP%", !tbaa !2
  %1 = icmp eq i16 %0, 0
  br i1 %1, label %b8, label %2

2:
  %3 = icmp eq i16 %0, 1
  br i1 %3, label %b23, label %b2

b2:
  ret void

b8:
  %4 = phi i16 [ %7, %b14 ], [ %0, %b1 ]
  %5 = phi i16 [ %19, %b14 ], [ %0, %b1 ]
  %6 = add nuw nsw i16 %4, -24576
  store i16 %6, ptr @b$seg, !tbaa !2
  %7 = add nuw nsw i16 %4, 20
  br label %b13

b13:
  %8 = phi i16 [ %33, %b17 ], [ -320, %b8 ]
  %9 = phi i16 [ %32, %b17 ], [ -640, %b8 ]
  %10 = getelementptr i8, ptr @"SLA%", i16 %9
  %11 = getelementptr i8, ptr %10, i16 640
  %12 = load i16, ptr %11, !tbaa !2
  %13 = icmp sgt i16 %5, %12
  %14 = getelementptr i8, ptr @"SLB%", i16 %9
  %15 = getelementptr i8, ptr %14, i16 640
  %16 = load i16, ptr %15, !tbaa !2
  %17 = icmp slt i16 %5, %16
  %18 = select i1 %13, i1 %17, i1 false
  br i1 %18, label %b15, label %b16

b14:
  %19 = add nuw nsw i16 %5, 1
  %exitcond1 = icmp eq i16 %19, 200
  br i1 %exitcond1, label %b24, label %b8

b15:
  %20 = getelementptr i8, ptr @"SC%", i16 %9
  %21 = getelementptr i8, ptr %20, i16 640
  %22 = load i16, ptr %21, !tbaa !2
  br label %b17

b16:
  %23 = mul i16 %5, 2
  %24 = getelementptr i8, ptr @"FS%", i16 %23
  %25 = load i16, ptr %24, !tbaa !2
  br label %b17

b17:
  %sink13 = phi i16 [ %22, %b15 ], [ %25, %b16 ]
  %26 = trunc i16 %sink13 to i8
  %27 = load i16, ptr @b$seg, !tbaa !2
  %28 = inttoptr i16 %27 to ptr addrspace(2)
  %29 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %30 = getelementptr i8, ptr addrspace(1) %29, i16 %8
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 320
  store volatile i8 %26, ptr addrspace(1) %31
  %32 = add nsw i16 %9, 2
  %33 = add nsw i16 %8, 1
  %exitcond = icmp eq i16 %32, 0
  br i1 %exitcond, label %b14, label %b13

b23:
  %34 = phi i16 [ %37, %b29 ], [ 0, %2 ]
  %35 = phi i16 [ %58, %b29 ], [ 0, %2 ]
  %36 = add nuw nsw i16 %34, -24576
  store i16 %36, ptr @b$seg, !tbaa !2
  %37 = add nuw nsw i16 %34, 20
  %38 = icmp samesign ugt i16 %35, 127
  %39 = sext i1 %38 to i16
  br label %b28

b24:
  %40 = load i16, ptr @"PY%", !tbaa !2
  %41 = sdiv i16 %40, 16
  %42 = getelementptr inbounds [2 x i8], ptr @"MUL128%", i16 %41
  %43 = load i16, ptr %42, !tbaa !2
  %44 = load i16, ptr @"PX%", !tbaa !2
  %45 = sdiv i16 %44, 16
  %46 = getelementptr [2 x i8], ptr @"SP%", i16 %43
  %47 = getelementptr [2 x i8], ptr %46, i16 %45
  store i16 24, ptr %47, !tbaa !2
  br label %b2

b28:
  %48 = phi i16 [ 0, %b23 ], [ %86, %b35 ]
  %49 = shl nuw nsw i16 %48, 1
  %50 = getelementptr i8, ptr @"SLA%", i16 %49
  %51 = load i16, ptr %50, !tbaa !2
  %52 = icmp sgt i16 %35, %51
  %53 = shl nuw nsw i16 %48, 1
  %54 = getelementptr i8, ptr @"SLB%", i16 %53
  %55 = load i16, ptr %54, !tbaa !2
  %56 = icmp slt i16 %35, %55
  %57 = select i1 %52, i1 %56, i1 false
  br i1 %57, label %b30, label %b31

b29:
  %58 = add nuw nsw i16 %35, 1
  %exitcond3 = icmp eq i16 %58, 200
  br i1 %exitcond3, label %b24, label %b23

b30:
  %59 = shl nuw nsw i16 %48, 1
  %60 = getelementptr i8, ptr @"SC%", i16 %59
  %61 = load i16, ptr %60, !tbaa !2
  br label %b32

b31:
  %62 = mul i16 %35, 2
  %63 = getelementptr i8, ptr @"FS%", i16 %62
  %64 = load i16, ptr %63, !tbaa !2
  br label %b32

b32:
  %65 = phi i16 [ %61, %b30 ], [ %64, %b31 ]
  %66 = icmp samesign ugt i16 %48, 127
  %67 = sext i1 %66 to i16
  %68 = or i16 %67, %39
  %69 = icmp eq i16 %68, 0
  br i1 %69, label %b34, label %b35

b34:
  %70 = getelementptr inbounds [2 x i8], ptr @"DIV2%", i16 %65
  %71 = load i16, ptr %70, !tbaa !2
  %72 = mul i16 %35, 2
  %73 = getelementptr i8, ptr @"MUL128%", i16 %72
  %74 = load i16, ptr %73, !tbaa !2
  %75 = shl i16 %74, 1
  %76 = shl nuw nsw i16 %48, 1
  %77 = add i16 %75, %76
  %78 = getelementptr i8, ptr @"SP%", i16 %77
  %79 = load i16, ptr %78, !tbaa !2
  %80 = add i16 %79, %71
  br label %b35

b35:
  %sink19 = phi i16 [ %80, %b34 ], [ %65, %b32 ]
  %81 = trunc i16 %sink19 to i8
  %82 = load i16, ptr @b$seg, !tbaa !2
  %83 = inttoptr i16 %82 to ptr addrspace(2)
  %84 = addrspacecast ptr addrspace(2) %83 to ptr addrspace(1)
  %85 = getelementptr i8, ptr addrspace(1) %84, i16 %48
  store volatile i8 %81, ptr addrspace(1) %85
  %86 = add nuw nsw i16 %48, 1
  %exitcond2 = icmp eq i16 %86, 320
  br i1 %exitcond2, label %b29, label %b28
}

define cc 1000 void @OUTPUT3D() addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree norecurse willreturn {
b1:
  %0 = load i16, ptr @"OBJ%", !tbaa !2
  %1 = shl i16 %0, 1
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZPO%", i16 2), !tbaa !2
  %3 = inttoptr i16 %2 to ptr addrspace(2)
  %4 = addrspacecast ptr addrspace(2) %3 to ptr addrspace(1)
  %5 = getelementptr i8, ptr addrspace(1) %4, i16 %1
  %6 = load i16, ptr addrspace(1) %5, !tbaa !14
  %7 = load i16, ptr @"FTYPE%", !tbaa !2
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %b3, label %9

9:
  br label %18

b2:
  ret void

b3:
  %10 = load i16, ptr @"NDTS%", !tbaa !2
  %11 = add i16 %10, -1
  %12 = icmp slt i16 %11, 0
  br i1 %12, label %b2, label %13

13:
  %14 = sitofp i16 %6 to float
  %15 = fdiv float 8.000000e+01, %14
  %16 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XS%", i16 2), !tbaa !2
  %17 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YS%", i16 2), !tbaa !2
  br label %b8

18:
  %19 = phi i16 [ %31, %30 ], [ 0, %9 ]
  %20 = mul nuw nsw i16 %19, 20
  %21 = add nuw nsw i16 %20, -24576
  store i16 %21, ptr @b$seg, !tbaa !2
  br label %22

22:
  %23 = phi i16 [ %29, %22 ], [ -320, %18 ]
  %24 = load i16, ptr @b$seg, !tbaa !2
  %25 = inttoptr i16 %24 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 %23
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 320
  store volatile i8 1, ptr addrspace(1) %28
  %29 = add nsw i16 %23, 1
  %exitcond = icmp eq i16 %29, 0
  br i1 %exitcond, label %30, label %22

30:
  %31 = add nuw nsw i16 %19, 1
  %exitcond1 = icmp eq i16 %31, 200
  br i1 %exitcond1, label %32, label %18

32:
  store i16 -24576, ptr @b$seg, !tbaa !2
  tail call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 63) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  br label %b2

b8:
  %33 = phi i16 [ %119, %b18 ], [ 0, %13 ]
  %34 = phi i16 [ %118, %b18 ], [ %10, %13 ]
  %35 = phi i16 [ %17, %13 ], [ %116, %b18 ]
  %36 = phi i16 [ %16, %13 ], [ %117, %b18 ]
  %37 = inttoptr i16 %36 to ptr addrspace(2)
  %38 = addrspacecast ptr addrspace(2) %37 to ptr addrspace(1)
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 8194
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 %33
  %41 = load i16, ptr addrspace(1) %40, !tbaa !66
  %42 = add i16 %41, 160
  %43 = icmp sgt i16 %42, -1
  %44 = icmp slt i16 %42, 320
  %45 = and i1 %43, %44
  %46 = inttoptr i16 %35 to ptr addrspace(2)
  %47 = addrspacecast ptr addrspace(2) %46 to ptr addrspace(1)
  %48 = getelementptr i8, ptr addrspace(1) %47, i16 8194
  %49 = getelementptr i8, ptr addrspace(1) %48, i16 %33
  %50 = load i16, ptr addrspace(1) %49, !tbaa !68
  %51 = icmp sgt i16 %50, -101
  %52 = and i1 %45, %51
  %53 = icmp slt i16 %50, 100
  %54 = and i1 %53, %52
  br i1 %54, label %b10, label %b12

b10:
  %55 = shl nsw i16 %50, 2
  %56 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YPK&", i16 2), !tbaa !2
  %57 = inttoptr i16 %56 to ptr addrspace(2)
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 %55
  %60 = getelementptr i8, ptr addrspace(1) %59, i16 400
  %61 = load i32, ptr addrspace(1) %60, !tbaa !12
  %62 = trunc i32 %61 to i16
  %63 = load i16, ptr @b$seg, !tbaa !2
  %64 = inttoptr i16 %63 to ptr addrspace(2)
  %65 = addrspacecast ptr addrspace(2) %64 to ptr addrspace(1)
  %66 = getelementptr i8, ptr addrspace(1) %65, i16 %42
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 %62
  store volatile i8 0, ptr addrspace(1) %67
  %68 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XS%", i16 2), !tbaa !2
  %69 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YS%", i16 2), !tbaa !2
  %70 = inttoptr i16 %68 to ptr addrspace(2)
  %71 = addrspacecast ptr addrspace(2) %70 to ptr addrspace(1)
  %72 = inttoptr i16 %69 to ptr addrspace(2)
  %73 = addrspacecast ptr addrspace(2) %72 to ptr addrspace(1)
  br label %b12

b12:
  %74 = phi ptr addrspace(1) [ %47, %b8 ], [ %73, %b10 ]
  %75 = phi ptr addrspace(1) [ %38, %b8 ], [ %71, %b10 ]
  %76 = phi i16 [ %35, %b8 ], [ %69, %b10 ]
  %77 = phi i16 [ %36, %b8 ], [ %68, %b10 ]
  %78 = getelementptr i8, ptr addrspace(1) %75, i16 %33
  %79 = load i16, ptr addrspace(1) %78, !tbaa !66
  %80 = add i16 %79, 160
  %81 = icmp sgt i16 %80, -1
  %82 = icmp slt i16 %80, 320
  %83 = and i1 %81, %82
  %84 = getelementptr i8, ptr addrspace(1) %74, i16 %33
  %85 = load i16, ptr addrspace(1) %84, !tbaa !68
  %86 = icmp sgt i16 %85, -101
  %87 = and i1 %83, %86
  %88 = icmp slt i16 %85, 100
  %89 = and i1 %88, %87
  br i1 %89, label %b16, label %b18

b16:
  %90 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Z!", i16 2), !tbaa !2
  %91 = inttoptr i16 %90 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = shl i16 %33, 1
  %94 = getelementptr i8, ptr addrspace(1) %92, i16 %93
  %95 = load float, ptr addrspace(1) %94, !tbaa !56
  %96 = fmul float %15, %95
  %97 = fsub float 2.560000e+02, %96
  %98 = tail call i16 @llvm.lrint.i16.f32(float %97)
  %99 = tail call i16 @llvm.smax.i16(i16 %98, i16 80)
  %100 = shl nsw i16 %85, 2
  %101 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YPK&", i16 2), !tbaa !2
  %102 = inttoptr i16 %101 to ptr addrspace(2)
  %103 = addrspacecast ptr addrspace(2) %102 to ptr addrspace(1)
  %104 = getelementptr i8, ptr addrspace(1) %103, i16 %100
  %105 = getelementptr i8, ptr addrspace(1) %104, i16 400
  %106 = load i32, ptr addrspace(1) %105, !tbaa !12
  %107 = trunc i32 %106 to i16
  %108 = trunc i16 %99 to i8
  %109 = load i16, ptr @b$seg, !tbaa !2
  %110 = inttoptr i16 %109 to ptr addrspace(2)
  %111 = addrspacecast ptr addrspace(2) %110 to ptr addrspace(1)
  %112 = getelementptr i8, ptr addrspace(1) %111, i16 %80
  %113 = getelementptr i8, ptr addrspace(1) %112, i16 %107
  store volatile i8 %108, ptr addrspace(1) %113
  %114 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XS%", i16 2), !tbaa !2
  %115 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YS%", i16 2), !tbaa !2
  br label %b18

b18:
  %116 = phi i16 [ %76, %b12 ], [ %115, %b16 ]
  %117 = phi i16 [ %77, %b12 ], [ %114, %b16 ]
  %118 = add i16 %34, -1
  %119 = add i16 %33, 2
  %exitcond2 = icmp eq i16 %118, 0
  br i1 %exitcond2, label %b2, label %b8
}

define cc 1000 void @PLASMABLOBS() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr align 2 %0, i8 0, i16 18, i1 false)
  tail call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  br label %b8

b2:
  %1 = phi i16 [ %325, %b184 ], [ %445, %b186 ], [ %445, %b193 ]
  %2 = icmp eq i16 %403, 0
  br i1 %2, label %b3, label %495

b3:
  %3 = phi i16 [ %438, %b2 ], [ %498, %b205 ]
  %4 = shl i16 %spec.store.select4, 1
  br label %b210

b4:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %0)
  ret void

b8:
  %5 = phi i16 [ %6, %b8 ], [ 128, %b1 ]
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %6 = add nsw i16 %5, -1
  %exitcond22 = icmp eq i16 %6, 0
  br i1 %exitcond22, label %7, label %b8

7:
  br label %b13

b13:
  %8 = phi i16 [ 0, %7 ], [ %9, %b13 ]
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %tmp6 = trunc i16 %8 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp6) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %9 = add nuw nsw i16 %8, 2
  %exitcond21 = icmp eq i16 %9, 64
  br i1 %exitcond21, label %10, label %b13

10:
  br label %b18

b18:
  %11 = phi i16 [ 64, %10 ], [ %13, %b18 ]
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %12 = add i16 %11, -2
  %tmp5 = trunc i16 %12 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp5) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %13 = add nsw i16 %11, -2
  %exitcond20 = icmp eq i16 %13, 0
  br i1 %exitcond20, label %14, label %b18

14:
  br label %b23

b23:
  %15 = phi i16 [ 0, %14 ], [ %16, %b23 ]
  %tmp4 = trunc i16 %15 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp4) memory(inaccessiblemem: readwrite)
  %tmp3 = trunc i16 %15 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp3) memory(inaccessiblemem: readwrite)
  %tmp2 = trunc i16 %15 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp2) memory(inaccessiblemem: readwrite)
  %16 = add nuw nsw i16 %15, 1
  %exitcond19 = icmp eq i16 %16, 64
  br i1 %exitcond19, label %17, label %b23

17:
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %20 = inttoptr i16 %18 to ptr addrspace(2)
  %21 = addrspacecast ptr addrspace(2) %20 to ptr addrspace(1)
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %24 = inttoptr i16 %22 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 %23
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %29 = inttoptr i16 %27 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %28
  %32 = getelementptr i8, ptr addrspace(1) %21, i16 %19
  br label %33

33:
  %34 = phi i16 [ %47, %33 ], [ -512, %17 ]
  %35 = getelementptr i8, ptr addrspace(1) %32, i16 %34
  %36 = getelementptr i8, ptr addrspace(1) %35, i16 512
  %37 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %38 = zext i8 %37 to i16
  store i16 %38, ptr addrspace(1) %36, !tbaa !60
  %39 = getelementptr i8, ptr addrspace(1) %26, i16 %34
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 512
  %41 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %42 = zext i8 %41 to i16
  store i16 %42, ptr addrspace(1) %40, !tbaa !62
  %43 = getelementptr i8, ptr addrspace(1) %31, i16 %34
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 512
  %45 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %46 = zext i8 %45 to i16
  store i16 %46, ptr addrspace(1) %44, !tbaa !64
  %47 = add nsw i16 %34, 2
  %exitcond1 = icmp eq i16 %47, 0
  br i1 %exitcond1, label %48, label %33

48:
  tail call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  br label %b28

b28:
  %49 = phi i16 [ %50, %b28 ], [ 256, %48 ]
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %50 = add nsw i16 %49, -1
  %exitcond18 = icmp eq i16 %50, 0
  br i1 %exitcond18, label %51, label %b28

51:
  br label %b33

b33:
  %52 = phi i16 [ -1352, %51 ], [ %78, %b44 ]
  %53 = phi i16 [ %77, %b44 ], [ -157, %51 ]
  %54 = icmp sgt i16 %53, -1
  br i1 %54, label %b35, label %b42

b35:
  %55 = uitofp nneg i16 %53 to float
  %56 = fdiv float %55, 4.500000e+01
  %57 = tail call float @llvm.fabs.f32(float %56)
  %58 = fcmp ogt float %57, 0x41E0000000000000
  br i1 %58, label %b38, label %b39

b37:
  %59 = tail call float @llvm.sin.f32(float %70)
  %60 = fmul float %59, 1.270000e+02
  %61 = tail call i16 @llvm.lrint.i16.f32(float %60)
  %62 = getelementptr i8, ptr @"FSIN2%", i16 %52
  %63 = getelementptr i8, ptr %62, i16 2318
  store i16 %61, ptr %63, !tbaa !2
  %64 = icmp samesign ult i16 %53, 476
  br i1 %64, label %b42, label %b44

b38:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b39:
  %65 = tail call float @llvm.sin.f32(float %56)
  %66 = fmul float %65, 6.300000e+01
  %67 = tail call i16 @llvm.lrint.i16.f32(float %66)
  %68 = getelementptr i8, ptr @"FSIN1%", i16 %52
  %69 = getelementptr i8, ptr %68, i16 1134
  store i16 %67, ptr %69, !tbaa !2
  %70 = fdiv float %55, 3.500000e+01
  %71 = tail call float @llvm.fabs.f32(float %70)
  %72 = fcmp ogt float %71, 0x41E0000000000000
  br i1 %72, label %b40, label %b37

b40:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b42:
  %73 = sitofp i16 %53 to float
  %74 = fdiv float %73, 2.500000e+01
  %75 = tail call float @llvm.fabs.f32(float %74)
  %76 = fcmp ogt float %75, 0x41E0000000000000
  br i1 %76, label %b45, label %b46

b44:
  %77 = add nsw i16 %53, 1
  %78 = add nsw i16 %52, 2
  %exitcond17 = icmp eq i16 %78, 0
  br i1 %exitcond17, label %79, label %b33

79:
  br label %b50

b45:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b46:
  %80 = tail call float @llvm.sin.f32(float %74)
  %81 = fmul float %80, 3.100000e+01
  %82 = tail call i16 @llvm.lrint.i16.f32(float %81)
  %83 = getelementptr i8, ptr @"FSIN3%", i16 %52
  %84 = getelementptr i8, ptr %83, i16 2318
  store i16 %82, ptr %84, !tbaa !2
  br label %b44

b50:
  %85 = phi i16 [ 64, %79 ], [ %103, %b50 ]
  %86 = phi i16 [ 0, %79 ], [ %102, %b50 ]
  %87 = phi i16 [ %spec.store.select1, %b50 ], [ -1, %79 ]
  %88 = lshr i16 %85, 6
  %89 = shl nuw nsw i16 %88, 6
  %90 = add nsw i16 %89, -1
  %91 = sub i16 %85, %89
  %92 = add i16 %86, %90
  %93 = shl nsw i16 %86, 1
  %94 = getelementptr i8, ptr @"DT100%", i16 %93
  %95 = getelementptr i8, ptr %94, i16 358
  %96 = icmp eq i16 %87, 1
  %spec.store.select5 = select i1 %96, i16 %92, i16 %91
  store i16 %spec.store.select5, ptr %95
  %97 = icmp eq i16 %spec.store.select5, 0
  %98 = and i1 %96, %97
  %spec.store.select = select i1 %98, i16 -1, i16 %87
  %99 = icmp eq i16 %spec.store.select5, 63
  %100 = icmp eq i16 %spec.store.select, -1
  %101 = select i1 %99, i1 %100, i1 false
  %spec.store.select1 = select i1 %101, i16 1, i16 %spec.store.select
  %102 = add nsw i16 %86, -1
  %103 = add nuw nsw i16 %85, 1
  %104 = icmp samesign ugt i16 %102, -156
  br i1 %104, label %b50, label %105

105:
  br label %b64

b64:
  %106 = phi i16 [ 63, %105 ], [ %133, %b68 ]
  %107 = phi i16 [ -314, %105 ], [ %132, %b68 ]
  %108 = phi i16 [ %131, %b68 ], [ 0, %105 ]
  %109 = phi i16 [ %spec.store.select3, %b68 ], [ 1, %105 ]
  %110 = lshr i16 %108, 6
  %111 = shl nuw nsw i16 %110, 6
  %112 = sub i16 %108, %111
  %113 = add i16 %106, %111
  %114 = and i16 %108, 192
  %115 = sub nsw i16 %108, %114
  %116 = getelementptr i8, ptr @"DT100%", i16 %107
  %117 = getelementptr i8, ptr %116, i16 674
  store i16 %112, ptr %117, !tbaa !2
  %118 = icmp eq i16 %109, -1
  br i1 %118, label %b66, label %b68

b65:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4095, i16 2, i16 257, ptr nonnull %0)
  %119 = getelementptr i8, ptr %0, i16 2
  %120 = load i16, ptr %119, !tbaa !2
  br label %b78

b66:
  %121 = sub nsw i16 63, %115
  %122 = getelementptr i8, ptr @"DT100%", i16 %107
  %123 = getelementptr i8, ptr %122, i16 674
  store i16 %113, ptr %123, !tbaa !2
  br label %b68

b68:
  %124 = phi i16 [ %121, %b66 ], [ %112, %b64 ]
  %125 = icmp eq i16 %124, 63
  %126 = icmp eq i16 %109, 1
  %127 = and i1 %125, %126
  %spec.store.select2 = select i1 %127, i16 -1, i16 %109
  %128 = icmp eq i16 %124, 0
  %129 = icmp eq i16 %spec.store.select2, -1
  %130 = select i1 %128, i1 %129, i1 false
  %spec.store.select3 = select i1 %130, i16 1, i16 %spec.store.select2
  %131 = add nuw nsw i16 %108, 1
  %132 = add nsw i16 %107, 2
  %133 = add nsw i16 %106, -1
  %exitcond16 = icmp eq i16 %132, 0
  br i1 %exitcond16, label %b65, label %b64

b78:
  %134 = phi i16 [ 0, %b65 ], [ %151, %b84 ]
  %135 = phi i16 [ 0, %b65 ], [ %152, %b84 ]
  %136 = inttoptr i16 %120 to ptr addrspace(2)
  %137 = addrspacecast ptr addrspace(2) %136 to ptr addrspace(1)
  %138 = sitofp i16 %134 to float
  %139 = shl i16 %135, 1
  %140 = getelementptr i8, ptr addrspace(1) %137, i16 %139
  %141 = sub i16 -64, %135
  br label %b83

b79:
  %142 = tail call i32 @llvm.lrint.i32.f32(float 0x3FF4CCCCC0000000)
  %143 = sitofp i32 %142 to float
  br label %b88

b83:
  %144 = phi i16 [ %150, %b83 ], [ -128, %b78 ]
  %IV.S. = phi float [ 0.000000e+00, %b78 ], [ %IV.S.next., %b83 ]
  %145 = getelementptr i8, ptr addrspace(1) %140, i16 %144
  %146 = getelementptr i8, ptr addrspace(1) %145, i16 128
  %147 = fdiv float %IV.S., 6.300000e+01
  %148 = fmul float %147, %138
  %149 = call i16 @llvm.lrint.i16.f32(float %148)
  store i16 %149, ptr addrspace(1) %146, !tbaa !70
  %IV.S.next. = fadd float %IV.S., 1.000000e+00
  %150 = add nsw i16 %144, 2
  %exitcond14 = icmp eq i16 %150, 0
  br i1 %exitcond14, label %b84, label %b83

b84:
  %151 = add nuw nsw i16 %134, 1
  %exitcond15 = icmp eq i16 %151, 64
  %152 = mul i16 %141, -1
  br i1 %exitcond15, label %b79, label %b78

b88:
  %153 = phi i16 [ 0, %b79 ], [ %169, %b94 ]
  %154 = phi i16 [ 0, %b79 ], [ %168, %b94 ]
  %155 = sub nuw nsw i16 99, %154
  %156 = uitofp nneg i16 %155 to float
  %157 = fpround float %156 to float
  %158 = fmul float %157, %157
  %159 = mul i16 %153, -1
  %160 = shl i16 %153, 1
  %161 = getelementptr i8, ptr @"SP%", i16 %160
  br label %b93

b93:
  %162 = phi i16 [ %212, %b117 ], [ 159, %b88 ]
  %163 = phi i16 [ %211, %b117 ], [ -320, %b88 ]
  %164 = phi i16 [ %210, %b117 ], [ %159, %b88 ]
  %165 = icmp eq i16 %154, 99
  %166 = icmp eq i16 %162, 0
  %167 = and i1 %165, %166
  br i1 %167, label %b94.thread, label %b96

b94:
  %168 = add nuw nsw i16 %154, 1
  %exitcond13 = icmp eq i16 %168, 100
  %169 = mul i16 %210, -1
  br i1 %exitcond13, label %174, label %b88

b94.thread:
  %170 = shl i16 %153, 1
  %171 = getelementptr i8, ptr @"SP%", i16 %170
  %172 = getelementptr i8, ptr %171, i16 %163
  %173 = getelementptr i8, ptr %172, i16 320
  store i16 255, ptr %173, !tbaa !2
  br label %174

174:
  br label %b121

b96:
  %175 = uitofp nneg i16 %162 to float
  %176 = fpround float %175 to float
  %177 = fmul float %176, %176
  %178 = fadd float %158, %177
  %179 = call float @llvm.sqrt.f32(float %178)
  %180 = fpround float %179 to float
  %181 = fcmp ogt float %180, 0.000000e+00
  br i1 %181, label %b98, label %b99

b97:
  %182 = sdiv i32 16384, %200
  %183 = trunc nsw i32 %182 to i16
  %184 = getelementptr i8, ptr @"SP%", i16 %160
  %185 = getelementptr i8, ptr %184, i16 %163
  %186 = getelementptr i8, ptr %185, i16 320
  store i16 %183, ptr %186, !tbaa !2
  %187 = icmp sgt i32 %182, 255
  br i1 %187, label %b115, label %b117

b98:
  %188 = call float @llvm.log2.f32(float %180)
  %189 = fmul float %188, 0x3FF4CCCCC0000000
  %190 = fcmp oge float %189, 3.276750e+04
  br i1 %190, label %b107, label %b108

b99:
  %191 = fcmp oeq float %180, 0.000000e+00
  br i1 %191, label %b106, label %b102

b102:
  %192 = fcmp oeq float %143, 0x3FF4CCCCC0000000
  br i1 %192, label %b103, label %b105

b103:
  %193 = fneg float %180
  %194 = call float @llvm.log2.f32(float %193)
  %195 = fmul float %194, 0x3FF4CCCCC0000000
  %196 = fcmp oge float %195, 3.276750e+04
  br i1 %196, label %b111, label %b112

b104:
  %197 = fneg float %207
  %198 = fpround float %197 to float
  br label %b106

b105:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b106:
  %199 = phi float [ %203, %b108 ], [ %207, %b112 ], [ %198, %b104 ], [ 0.000000e+00, %b99 ]
  %200 = call i32 @llvm.lrint.i32.f32(float %199)
  %201 = icmp eq i32 %200, 0
  br i1 %201, label %b113, label %b97

b107:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b108:
  %202 = call float @llvm.exp2.f32(float %189)
  %203 = fpround float %202 to float
  br label %b106

b111:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b112:
  %204 = and i32 %142, 1
  %205 = icmp eq i32 %204, 0
  %206 = call float @llvm.exp2.f32(float %195)
  %207 = fpround float %206 to float
  br i1 %205, label %b106, label %b104

b113:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b115:
  %208 = getelementptr i8, ptr %161, i16 %163
  %209 = getelementptr i8, ptr %208, i16 320
  store i16 255, ptr %209, !tbaa !2
  br label %b117

b117:
  %210 = add i16 %164, -1
  %211 = add nsw i16 %163, 2
  %212 = add nsw i16 %162, -1
  %exitcond12 = icmp eq i16 %211, 0
  br i1 %exitcond12, label %b94, label %b93

b121:
  %213 = phi i16 [ 479, %174 ], [ %228, %b121 ]
  %214 = phi i16 [ -1280, %174 ], [ %227, %b121 ]
  %215 = phi i16 [ %226, %b121 ], [ -160, %174 ]
  %216 = ashr i16 %215, 15
  %217 = icmp sgt i16 %215, 319
  %218 = sext i1 %217 to i16
  %219 = or i16 %216, %218
  %220 = icmp eq i16 %219, 0
  %221 = icmp slt i16 %215, 160
  %222 = select i1 %221, i16 %215, i16 %213
  %223 = select i1 %220, i16 %222, i16 0
  %224 = getelementptr i8, ptr @"FSIN4%", i16 %214
  %225 = getelementptr i8, ptr %224, i16 1598
  store i16 %223, ptr %225, !tbaa !2
  %226 = add nsw i16 %215, 1
  %227 = add nsw i16 %214, 2
  %228 = add nsw i16 %213, -1
  %exitcond11 = icmp eq i16 %227, 0
  br i1 %exitcond11, label %229, label %b121

229:
  br label %b132

b132:
  %230 = phi i16 [ 299, %229 ], [ %246, %b132 ]
  %231 = phi i16 [ -802, %229 ], [ %245, %b132 ]
  %232 = phi i16 [ %244, %b132 ], [ -100, %229 ]
  %233 = ashr i16 %232, 15
  %234 = icmp sgt i16 %232, 199
  %235 = sext i1 %234 to i16
  %236 = or i16 %233, %235
  %237 = icmp eq i16 %236, 0
  %238 = icmp sgt i16 %232, 99
  %239 = select i1 %238, i16 %230, i16 %232
  %240 = select i1 %237, i16 %239, i16 0
  %241 = getelementptr i8, ptr @"CY%", i16 %231
  %242 = getelementptr i8, ptr %241, i16 802
  %243 = mul i16 %240, 160
  store i16 %243, ptr %242, !tbaa !2
  %244 = add nsw i16 %232, 1
  %245 = add nsw i16 %231, 2
  %246 = add nsw i16 %230, -1
  %exitcond10 = icmp eq i16 %245, 0
  br i1 %exitcond10, label %247, label %b132

247:
  br label %b143

b143:
  %248 = phi i16 [ -768, %247 ], [ %255, %b143 ]
  %249 = phi i16 [ -1536, %247 ], [ %254, %b143 ]
  %250 = add i16 %248, 768
  %251 = getelementptr i8, ptr @"CD%", i16 %249
  %252 = getelementptr i8, ptr %251, i16 1536
  %253 = call i16 @llvm.umin.i16(i16 %250, i16 255)
  store i16 %253, ptr %252, !tbaa !2
  %254 = add nsw i16 %249, 2
  %255 = add nsw i16 %248, 1
  %exitcond9 = icmp eq i16 %254, 0
  br i1 %exitcond9, label %256, label %b143

256:
  br label %b151

b151:
  %257 = phi i16 [ -12288, %256 ], [ %262, %b151 ]
  %258 = phi i16 [ -512, %256 ], [ %261, %b151 ]
  %259 = getelementptr i8, ptr @"DT%", i16 %258
  %260 = getelementptr i8, ptr %259, i16 512
  store i16 %257, ptr %260, !tbaa !2
  %261 = add nsw i16 %258, 2
  %262 = add nsw i16 %257, 64
  %exitcond8 = icmp eq i16 %261, 0
  br i1 %exitcond8, label %263, label %b151

263:
  br label %b156

b156:
  %264 = phi i16 [ -6, %263 ], [ %313, %b156 ]
  %265 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %266 = load float, ptr %265
  %267 = fmul float %266, 4.500000e+01
  %268 = fadd float %267, 4.500000e+01
  %269 = call float @llvm.rint.f32(float %268)
  %270 = fcmp olt float %268, %269
  %271 = sext i1 %270 to i16
  %272 = sitofp i16 %271 to float
  %273 = fadd float %269, %272
  %274 = call i16 @llvm.lrint.i16.f32(float %273)
  %275 = getelementptr i8, ptr @"DSX%", i16 %264
  %276 = getelementptr i8, ptr %275, i16 8
  store i16 %274, ptr %276, !tbaa !2
  %277 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %278 = load float, ptr %277
  %279 = fmul float %278, 3.000000e+01
  %280 = fadd float %279, 3.000000e+01
  %281 = call float @llvm.rint.f32(float %280)
  %282 = fcmp olt float %280, %281
  %283 = sext i1 %282 to i16
  %284 = sitofp i16 %283 to float
  %285 = fadd float %281, %284
  %286 = call i16 @llvm.lrint.i16.f32(float %285)
  %287 = getelementptr i8, ptr @"DSY%", i16 %264
  %288 = getelementptr i8, ptr %287, i16 8
  store i16 %286, ptr %288, !tbaa !2
  %289 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %290 = load float, ptr %289
  %291 = fmul float %290, 8.000000e+01
  %292 = fadd float %291, 8.000000e+01
  %293 = call float @llvm.rint.f32(float %292)
  %294 = fcmp olt float %292, %293
  %295 = sext i1 %294 to i16
  %296 = sitofp i16 %295 to float
  %297 = fadd float %293, %296
  %298 = call i16 @llvm.lrint.i16.f32(float %297)
  %299 = getelementptr i8, ptr @"PSX%", i16 %264
  %300 = getelementptr i8, ptr %299, i16 8
  store i16 %298, ptr %300, !tbaa !2
  %301 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %302 = load float, ptr %301
  %303 = fmul float %302, 5.000000e+01
  %304 = fadd float %303, 5.000000e+01
  %305 = call float @llvm.rint.f32(float %304)
  %306 = fcmp olt float %304, %305
  %307 = sext i1 %306 to i16
  %308 = sitofp i16 %307 to float
  %309 = fadd float %305, %308
  %310 = call i16 @llvm.lrint.i16.f32(float %309)
  %311 = getelementptr i8, ptr @"PSY%", i16 %264
  %312 = getelementptr i8, ptr %311, i16 8
  store i16 %310, ptr %312, !tbaa !2
  %313 = add nsw i16 %264, 2
  %exitcond7 = icmp eq i16 %313, 0
  br i1 %exitcond7, label %314, label %b156

314:
  %315 = getelementptr i8, ptr @"FSIN1%", i16 96
  br label %b159

b159:
  %316 = phi i16 [ %329, %b222 ], [ %329, %575 ], [ 0, %314 ]
  %317 = phi i16 [ %443, %b222 ], [ %443, %575 ], [ 0, %314 ]
  %318 = phi i16 [ %442, %b222 ], [ %442, %575 ], [ 0, %314 ]
  %319 = phi i16 [ %441, %b222 ], [ %441, %575 ], [ 0, %314 ]
  %320 = phi i16 [ %440, %b222 ], [ %440, %575 ], [ 191, %314 ]
  %321 = phi i16 [ %439, %b222 ], [ %439, %575 ], [ 128, %314 ]
  %322 = phi i16 [ %399, %b222 ], [ %399, %575 ], [ 191, %314 ]
  %323 = phi i16 [ %400, %b222 ], [ %400, %575 ], [ 128, %314 ]
  %324 = phi i16 [ %3, %b222 ], [ %3, %575 ], [ 0, %314 ]
  %325 = phi i16 [ %1, %b222 ], [ %1, %575 ], [ 0, %314 ]
  %326 = phi i16 [ %spec.store.select4, %b222 ], [ %spec.store.select4, %575 ], [ 0, %314 ]
  %327 = phi i16 [ %332, %b222 ], [ %332, %575 ], [ 0, %314 ]
  %328 = phi i16 [ %403, %b222 ], [ %403, %575 ], [ 0, %314 ]
  %329 = add i16 %316, 1
  %330 = add i16 %326, 2
  %331 = icmp sgt i16 %330, 156
  %spec.store.select4 = select i1 %331, i16 0, i16 %330
  %332 = add i16 %327, 1
  %333 = sitofp i16 %332 to float
  %334 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSX%", i16 2), !tbaa !2
  %335 = sitofp i16 %334 to float
  %336 = fdiv float %333, %335
  %337 = call float @llvm.fabs.f32(float %336)
  %338 = fcmp ogt float %337, 0x41E0000000000000
  br i1 %338, label %b169, label %b170

b169:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b170:
  %339 = call float @llvm.sin.f32(float %336)
  %340 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSX%", i16 2), !tbaa !2
  %341 = sitofp i16 %340 to float
  %342 = fmul float %339, %341
  %343 = call i16 @llvm.lrint.i16.f32(float %342)
  store i16 %343, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 2), !tbaa !2
  %344 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSY%", i16 2), !tbaa !2
  %345 = sitofp i16 %344 to float
  %346 = fdiv float %333, %345
  %347 = call float @llvm.fabs.f32(float %346)
  %348 = fcmp ogt float %347, 0x41E0000000000000
  br i1 %348, label %b171, label %b172

b171:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b172:
  %349 = call float @llvm.sin.f32(float %346)
  %350 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSY%", i16 2), !tbaa !2
  %351 = sitofp i16 %350 to float
  %352 = fmul float %349, %351
  %353 = call i16 @llvm.lrint.i16.f32(float %352)
  store i16 %353, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 2), !tbaa !2
  %354 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSX%", i16 4), !tbaa !2
  %355 = sitofp i16 %354 to float
  %356 = fdiv float %333, %355
  %357 = call float @llvm.fabs.f32(float %356)
  %358 = fcmp ogt float %357, 0x41E0000000000000
  br i1 %358, label %b169, label %359

359:
  %360 = call float @llvm.sin.f32(float %356)
  %361 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSX%", i16 4), !tbaa !2
  %362 = sitofp i16 %361 to float
  %363 = fmul float %360, %362
  %364 = call i16 @llvm.lrint.i16.f32(float %363)
  store i16 %364, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 4), !tbaa !2
  %365 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSY%", i16 4), !tbaa !2
  %366 = sitofp i16 %365 to float
  %367 = fdiv float %333, %366
  %368 = call float @llvm.fabs.f32(float %367)
  %369 = fcmp ogt float %368, 0x41E0000000000000
  br i1 %369, label %b171, label %370

370:
  %371 = call float @llvm.sin.f32(float %367)
  %372 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSY%", i16 4), !tbaa !2
  %373 = sitofp i16 %372 to float
  %374 = fmul float %371, %373
  %375 = call i16 @llvm.lrint.i16.f32(float %374)
  store i16 %375, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 4), !tbaa !2
  %376 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSX%", i16 6), !tbaa !2
  %377 = sitofp i16 %376 to float
  %378 = fdiv float %333, %377
  %379 = call float @llvm.fabs.f32(float %378)
  %380 = fcmp ogt float %379, 0x41E0000000000000
  br i1 %380, label %b169, label %381

381:
  %382 = call float @llvm.sin.f32(float %378)
  %383 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSX%", i16 6), !tbaa !2
  %384 = sitofp i16 %383 to float
  %385 = fmul float %382, %384
  %386 = call i16 @llvm.lrint.i16.f32(float %385)
  store i16 %386, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 6), !tbaa !2
  %387 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSY%", i16 6), !tbaa !2
  %388 = sitofp i16 %387 to float
  %389 = fdiv float %333, %388
  %390 = call float @llvm.fabs.f32(float %389)
  %391 = fcmp ogt float %390, 0x41E0000000000000
  br i1 %391, label %b171, label %392

392:
  %393 = call float @llvm.sin.f32(float %389)
  %394 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSY%", i16 6), !tbaa !2
  %395 = sitofp i16 %394 to float
  %396 = fmul float %393, %395
  %397 = call i16 @llvm.lrint.i16.f32(float %396)
  store i16 %397, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 6), !tbaa !2
  %398 = icmp eq i16 %329, 51
  %399 = select i1 %398, i16 255, i16 %322
  %400 = select i1 %398, i16 192, i16 %323
  %401 = select i1 %398, i16 0, i16 %325
  %402 = icmp eq i16 %329, 151
  %403 = select i1 %402, i16 1, i16 %328
  %404 = icmp eq i16 %329, 280
  br i1 %404, label %b4, label %405

405:
  %406 = icmp eq i16 %329, 250
  br i1 %406, label %407, label %b184

407:
  call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %408 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %409 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %410 = inttoptr i16 %408 to ptr addrspace(2)
  %411 = addrspacecast ptr addrspace(2) %410 to ptr addrspace(1)
  %412 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %413 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %414 = inttoptr i16 %412 to ptr addrspace(2)
  %415 = addrspacecast ptr addrspace(2) %414 to ptr addrspace(1)
  %416 = getelementptr i8, ptr addrspace(1) %415, i16 %413
  %417 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %418 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %419 = inttoptr i16 %417 to ptr addrspace(2)
  %420 = addrspacecast ptr addrspace(2) %419 to ptr addrspace(1)
  %421 = getelementptr i8, ptr addrspace(1) %420, i16 %418
  %422 = getelementptr i8, ptr addrspace(1) %411, i16 %409
  br label %423

423:
  %424 = phi i16 [ %437, %423 ], [ -512, %407 ]
  %425 = getelementptr i8, ptr addrspace(1) %422, i16 %424
  %426 = getelementptr i8, ptr addrspace(1) %425, i16 512
  %427 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %428 = zext i8 %427 to i16
  store i16 %428, ptr addrspace(1) %426, !tbaa !60
  %429 = getelementptr i8, ptr addrspace(1) %416, i16 %424
  %430 = getelementptr i8, ptr addrspace(1) %429, i16 512
  %431 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %432 = zext i8 %431 to i16
  store i16 %432, ptr addrspace(1) %430, !tbaa !62
  %433 = getelementptr i8, ptr addrspace(1) %421, i16 %424
  %434 = getelementptr i8, ptr addrspace(1) %433, i16 512
  %435 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %436 = zext i8 %435 to i16
  store i16 %436, ptr addrspace(1) %434, !tbaa !64
  %437 = add nsw i16 %424, 2
  %exitcond = icmp eq i16 %437, 0
  br i1 %exitcond, label %b184, label %423

b184:
  %438 = phi i16 [ %324, %405 ], [ 0, %423 ]
  %439 = phi i16 [ %321, %405 ], [ 0, %423 ]
  %440 = phi i16 [ %320, %405 ], [ 255, %423 ]
  %441 = phi i16 [ %319, %405 ], [ 63, %423 ]
  %442 = phi i16 [ %318, %405 ], [ 63, %423 ]
  %443 = phi i16 [ %317, %405 ], [ 63, %423 ]
  %444 = icmp slt i16 %401, 26
  br i1 %444, label %b186, label %b2

b186:
  %445 = add nsw i16 %401, 1
  %446 = icmp ugt i16 %400, %399
  br i1 %446, label %b2, label %447

447:
  %448 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %449 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %450 = inttoptr i16 %448 to ptr addrspace(2)
  %451 = addrspacecast ptr addrspace(2) %450 to ptr addrspace(1)
  %452 = sitofp i16 %445 to float
  %453 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %454 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %455 = inttoptr i16 %453 to ptr addrspace(2)
  %456 = addrspacecast ptr addrspace(2) %455 to ptr addrspace(1)
  %457 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %458 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %459 = inttoptr i16 %457 to ptr addrspace(2)
  %460 = addrspacecast ptr addrspace(2) %459 to ptr addrspace(1)
  %461 = getelementptr i8, ptr addrspace(1) %460, i16 %458
  %462 = add nuw nsw i16 %399, 1
  %463 = getelementptr i8, ptr addrspace(1) %456, i16 %454
  %464 = getelementptr i8, ptr addrspace(1) %451, i16 %449
  br label %b193

b193:
  %465 = phi i16 [ %400, %447 ], [ %493, %b193 ]
  %tmp1 = trunc i16 %465 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %tmp1) memory(inaccessiblemem: readwrite)
  %466 = shl nuw i16 %465, 1
  %467 = getelementptr i8, ptr addrspace(1) %464, i16 %466
  %468 = load i16, ptr addrspace(1) %467, !tbaa !60
  %469 = sitofp i16 %468 to float
  %470 = fdiv float %469, 2.600000e+01
  %471 = fmul float %470, %452
  %472 = fadd float %471, 0.000000e+00
  %473 = call i16 @llvm.lrint.i16.f32(float %472)
  %474 = trunc i16 %473 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %474) memory(inaccessiblemem: readwrite)
  %475 = shl nuw i16 %465, 1
  %476 = getelementptr i8, ptr addrspace(1) %463, i16 %475
  %477 = load i16, ptr addrspace(1) %476, !tbaa !62
  %478 = sitofp i16 %477 to float
  %479 = fdiv float %478, 2.600000e+01
  %480 = fmul float %479, %452
  %481 = fadd float %480, 0.000000e+00
  %482 = call i16 @llvm.lrint.i16.f32(float %481)
  %483 = trunc i16 %482 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %483) memory(inaccessiblemem: readwrite)
  %484 = shl nuw i16 %465, 1
  %485 = getelementptr i8, ptr addrspace(1) %461, i16 %484
  %486 = load i16, ptr addrspace(1) %485, !tbaa !64
  %487 = sitofp i16 %486 to float
  %488 = fdiv float %487, 2.600000e+01
  %489 = fmul float %488, %452
  %490 = fadd float %489, 0.000000e+00
  %491 = call i16 @llvm.lrint.i16.f32(float %490)
  %492 = trunc i16 %491 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %492) memory(inaccessiblemem: readwrite)
  %493 = add nuw i16 %465, 1
  %494 = icmp eq i16 %462, %493
  br i1 %494, label %b2, label %b193

495:
  %496 = icmp slt i16 %438, 26
  %497 = zext i1 %496 to i16
  %498 = add nsw i16 %438, %497
  %499 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %500 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %501 = inttoptr i16 %499 to ptr addrspace(2)
  %502 = addrspacecast ptr addrspace(2) %501 to ptr addrspace(1)
  %503 = sitofp i16 %498 to float
  %504 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %505 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %506 = inttoptr i16 %504 to ptr addrspace(2)
  %507 = addrspacecast ptr addrspace(2) %506 to ptr addrspace(1)
  %508 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %509 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %510 = inttoptr i16 %508 to ptr addrspace(2)
  %511 = addrspacecast ptr addrspace(2) %510 to ptr addrspace(1)
  %512 = getelementptr i8, ptr addrspace(1) %511, i16 %509
  %513 = add nuw nsw i16 %440, 1
  %514 = getelementptr i8, ptr addrspace(1) %507, i16 %505
  %515 = getelementptr i8, ptr addrspace(1) %502, i16 %500
  br label %b205

b205:
  %516 = phi i16 [ %439, %495 ], [ %550, %b205 ]
  %tmp = trunc i16 %516 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %tmp) memory(inaccessiblemem: readwrite)
  %517 = shl nuw nsw i16 %516, 1
  %518 = getelementptr i8, ptr addrspace(1) %515, i16 %517
  %519 = load i16, ptr addrspace(1) %518, !tbaa !60
  %520 = sub i16 %441, %519
  %521 = sitofp i16 %520 to float
  %522 = fdiv float %521, 2.600000e+01
  %523 = fmul float %522, %503
  %524 = sitofp i16 %519 to float
  %525 = fadd float %523, %524
  %526 = call i16 @llvm.lrint.i16.f32(float %525)
  %527 = trunc i16 %526 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %527) memory(inaccessiblemem: readwrite)
  %528 = shl nuw nsw i16 %516, 1
  %529 = getelementptr i8, ptr addrspace(1) %514, i16 %528
  %530 = load i16, ptr addrspace(1) %529, !tbaa !62
  %531 = sub i16 %442, %530
  %532 = sitofp i16 %531 to float
  %533 = fdiv float %532, 2.600000e+01
  %534 = fmul float %533, %503
  %535 = sitofp i16 %530 to float
  %536 = fadd float %534, %535
  %537 = call i16 @llvm.lrint.i16.f32(float %536)
  %538 = trunc i16 %537 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %538) memory(inaccessiblemem: readwrite)
  %539 = shl nuw nsw i16 %516, 1
  %540 = getelementptr i8, ptr addrspace(1) %512, i16 %539
  %541 = load i16, ptr addrspace(1) %540, !tbaa !64
  %542 = sub i16 %443, %541
  %543 = sitofp i16 %542 to float
  %544 = fdiv float %543, 2.600000e+01
  %545 = fmul float %544, %503
  %546 = sitofp i16 %541 to float
  %547 = fadd float %545, %546
  %548 = call i16 @llvm.lrint.i16.f32(float %547)
  %549 = trunc i16 %548 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %549) memory(inaccessiblemem: readwrite)
  %550 = add nuw nsw i16 %516, 1
  %551 = icmp eq i16 %513, %550
  br i1 %551, label %b3, label %b205

b210:
  %552 = phi ptr [ %617, %b216 ], [ %315, %b3 ]
  %553 = phi i16 [ 0, %b3 ], [ %616, %b216 ]
  %554 = phi i16 [ 0, %b3 ], [ %556, %b216 ]
  %555 = add nuw nsw i16 %554, -24576
  store i16 %555, ptr @b$seg, !tbaa !2
  %556 = add nuw nsw i16 %554, 20
  %557 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 2), !tbaa !2
  %558 = sub i16 %553, %557
  %559 = getelementptr [2 x i8], ptr @"CY%", i16 %558
  %560 = getelementptr i8, ptr %559, i16 200
  %561 = load i16, ptr %560, !tbaa !2
  store i16 %561, ptr @"YY%.0", !tbaa !2
  %562 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 4), !tbaa !2
  %563 = sub i16 %553, %562
  %564 = getelementptr [2 x i8], ptr @"CY%", i16 %563
  %565 = getelementptr i8, ptr %564, i16 200
  %566 = load i16, ptr %565, !tbaa !2
  store i16 %566, ptr @"YY%.1", !tbaa !2
  %567 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 6), !tbaa !2
  %568 = sub i16 %553, %567
  %569 = getelementptr [2 x i8], ptr @"CY%", i16 %568
  %570 = getelementptr i8, ptr %569, i16 200
  %571 = load i16, ptr %570, !tbaa !2
  store i16 %571, ptr @"YY%.2", !tbaa !2
  %572 = sub i16 %553, %spec.store.select4
  br label %b215

b211:
  %573 = load i16, ptr @"XIT%", !tbaa !2
  %574 = icmp eq i16 %573, 0
  br i1 %574, label %667, label %575

575:
  %576 = icmp eq i16 %573, 1
  br i1 %576, label %b4, label %b159

b215:
  %577 = phi i16 [ 0, %b210 ], [ %666, %b219 ]
  %578 = load i16, ptr @"YY%.0", !tbaa !2
  %579 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 2), !tbaa !2
  %580 = mul i16 %579, -2
  %581 = shl nuw nsw i16 %577, 1
  %582 = add i16 %580, %581
  %583 = getelementptr i8, ptr @"FSIN4%", i16 %582
  %584 = getelementptr i8, ptr %583, i16 638
  %585 = load i16, ptr %584, !tbaa !2
  %586 = getelementptr [2 x i8], ptr @"SP%", i16 %578
  %587 = getelementptr [2 x i8], ptr %586, i16 %585
  %588 = load i16, ptr %587, !tbaa !2
  %589 = load i16, ptr @"YY%.1", !tbaa !2
  %590 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 4), !tbaa !2
  %591 = mul i16 %590, -2
  %592 = shl nuw nsw i16 %577, 1
  %593 = add i16 %591, %592
  %594 = getelementptr i8, ptr @"FSIN4%", i16 %593
  %595 = getelementptr i8, ptr %594, i16 638
  %596 = load i16, ptr %595, !tbaa !2
  %597 = getelementptr [2 x i8], ptr @"SP%", i16 %589
  %598 = getelementptr [2 x i8], ptr %597, i16 %596
  %599 = load i16, ptr %598, !tbaa !2
  %600 = load i16, ptr @"YY%.2", !tbaa !2
  %601 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 6), !tbaa !2
  %602 = mul i16 %601, -2
  %603 = shl nuw nsw i16 %577, 1
  %604 = add i16 %602, %603
  %605 = getelementptr i8, ptr @"FSIN4%", i16 %604
  %606 = getelementptr i8, ptr %605, i16 638
  %607 = load i16, ptr %606, !tbaa !2
  %608 = getelementptr [2 x i8], ptr @"SP%", i16 %600
  %609 = getelementptr [2 x i8], ptr %608, i16 %607
  %610 = load i16, ptr %609, !tbaa !2
  %611 = getelementptr [2 x i8], ptr @"CD%", i16 %588
  %612 = getelementptr [2 x i8], ptr %611, i16 %599
  %613 = getelementptr [2 x i8], ptr %612, i16 %610
  %614 = load i16, ptr %613, !tbaa !2
  %615 = icmp slt i16 %614, 192
  br i1 %615, label %b217, label %b218

b216:
  %616 = add nuw nsw i16 %553, 1
  %exitcond3 = icmp eq i16 %616, 200
  %617 = getelementptr i8, ptr %552, i16 2
  br i1 %exitcond3, label %b211, label %b210

b217:
  %618 = trunc i16 %614 to i8
  br label %b219

b218:
  %619 = shl nuw nsw i16 %577, 1
  %620 = getelementptr i8, ptr @"FSIN3%", i16 %4
  %621 = getelementptr i8, ptr %620, i16 %619
  %622 = getelementptr i8, ptr %621, i16 1280
  %623 = load i16, ptr %622, !tbaa !2
  %624 = mul i16 %553, 2
  %625 = getelementptr i8, ptr @"FSIN3%", i16 %624
  %626 = getelementptr i8, ptr %625, i16 1280
  %627 = load i16, ptr %626, !tbaa !2
  %628 = shl nuw nsw i16 %577, 1
  %629 = getelementptr i8, ptr %552, i16 %628
  %630 = load i16, ptr %629, !tbaa !2
  %631 = mul i16 %572, 2
  %632 = getelementptr i8, ptr @"FSIN3%", i16 %631
  %633 = getelementptr i8, ptr %632, i16 1280
  %634 = load i16, ptr %633, !tbaa !2
  %635 = shl nuw nsw i16 %577, 1
  %636 = getelementptr i8, ptr @"FSIN2%", i16 %635
  %637 = getelementptr i8, ptr %636, i16 1280
  %638 = load i16, ptr %637, !tbaa !2
  %639 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %634
  %640 = getelementptr [2 x i8], ptr %639, i16 %638
  %641 = getelementptr [2 x i8], ptr %640, i16 %spec.store.select4
  %642 = getelementptr i8, ptr %641, i16 1280
  %643 = load i16, ptr %642, !tbaa !2
  %644 = getelementptr [2 x i8], ptr @"DT100%", i16 %623
  %645 = getelementptr [2 x i8], ptr %644, i16 %627
  %646 = getelementptr [2 x i8], ptr %645, i16 %630
  %647 = getelementptr [2 x i8], ptr %646, i16 %643
  %648 = getelementptr i8, ptr %647, i16 360
  %649 = load i16, ptr %648, !tbaa !2
  %650 = getelementptr inbounds nuw [2 x i8], ptr @"DT%", i16 %614
  %651 = load i16, ptr %650, !tbaa !2
  %652 = add i16 %651, %649
  %653 = shl i16 %652, 1
  %654 = getelementptr i8, ptr %0, i16 2
  %655 = load i16, ptr %654, !tbaa !2
  %656 = inttoptr i16 %655 to ptr addrspace(2)
  %657 = addrspacecast ptr addrspace(2) %656 to ptr addrspace(1)
  %658 = getelementptr i8, ptr addrspace(1) %657, i16 %653
  %659 = load i16, ptr addrspace(1) %658, !tbaa !70
  %660 = trunc i16 %659 to i8
  %661 = add i8 %660, -64
  br label %b219

b219:
  %sink4 = phi i8 [ %618, %b217 ], [ %661, %b218 ]
  %662 = load i16, ptr @b$seg, !tbaa !2
  %663 = inttoptr i16 %662 to ptr addrspace(2)
  %664 = addrspacecast ptr addrspace(2) %663 to ptr addrspace(1)
  %665 = getelementptr i8, ptr addrspace(1) %664, i16 %577
  store volatile i8 %sink4, ptr addrspace(1) %665
  %666 = add nuw nsw i16 %577, 1
  %exitcond2 = icmp eq i16 %666, 320
  br i1 %exitcond2, label %b216, label %b215

667:
  %668 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %669 = icmp eq i8 %668, 1
  br i1 %669, label %b222.thread, label %b222

b222.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b4

b222:
  %670 = icmp eq i16 %573, 1
  br i1 %670, label %b4, label %b159
}

define cc 1000 void @PRECALCULATIONS() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  br label %b5

b5:
  %0 = phi i16 [ %7, %b5 ], [ -256, %b1 ]
  %1 = phi i16 [ %6, %b5 ], [ -512, %b1 ]
  %2 = add i16 %0, 256
  %3 = lshr i16 %2, 1
  %4 = getelementptr i8, ptr @"DIV2%", i16 %1
  %5 = getelementptr i8, ptr %4, i16 512
  store i16 %3, ptr %5, !tbaa !2
  %6 = add nsw i16 %1, 2
  %7 = add nsw i16 %0, 1
  %exitcond6 = icmp eq i16 %6, 0
  br i1 %exitcond6, label %8, label %b5

8:
  br label %b15

b15:
  %9 = phi ptr [ @"MUL32%", %8 ], [ %12, %b15 ]
  %10 = phi i16 [ 0, %8 ], [ %11, %b15 ]
  store i16 %10, ptr %9, !tbaa !2
  %11 = add nuw nsw i16 %10, 32
  %12 = getelementptr i8, ptr %9, i16 2
  %exitcond4 = icmp eq i16 %11, 1056
  br i1 %exitcond4, label %13, label %b15

13:
  br label %b20

b20:
  %14 = phi ptr [ @"MUL128%", %13 ], [ %17, %b20 ]
  %15 = phi i16 [ 0, %13 ], [ %16, %b20 ]
  store i16 %15, ptr %14, !tbaa !2
  %16 = add nuw nsw i16 %15, 128
  %17 = getelementptr i8, ptr %14, i16 2
  %exitcond3 = icmp eq i16 %16, 16384
  br i1 %exitcond3, label %18, label %b20

18:
  br label %b25

b25:
  %19 = phi i16 [ %38, %b31 ], [ 0, %18 ]
  %20 = phi i16 [ %37, %b31 ], [ 0, %18 ]
  %21 = shl nuw nsw i16 %20, 3
  %22 = and i16 %21, 992
  %23 = getelementptr inbounds nuw [2 x i8], ptr @"CD%", i16 %22
  %24 = shl i16 %19, 1
  %25 = sub i16 -128, %19
  br label %b30

b30:
  %26 = phi i16 [ %36, %b30 ], [ -256, %b25 ]
  %27 = phi i16 [ 0, %b25 ], [ %35, %b30 ]
  %28 = lshr i16 %27, 2
  %29 = getelementptr inbounds nuw [2 x i8], ptr %23, i16 %28
  %30 = load i16, ptr %29, !tbaa !2
  %31 = shl i16 %30, 7
  %32 = getelementptr i8, ptr @"SP%", i16 %24
  %33 = getelementptr i8, ptr %32, i16 %26
  %34 = getelementptr i8, ptr %33, i16 256
  store i16 %31, ptr %34, !tbaa !2
  %35 = add nuw nsw i16 %27, 1
  %36 = add nsw i16 %26, 2
  %exitcond1 = icmp eq i16 %36, 0
  br i1 %exitcond1, label %b31, label %b30

b31:
  %37 = add nuw nsw i16 %20, 1
  %exitcond2 = icmp eq i16 %37, 128
  %38 = mul i16 %25, -1
  br i1 %exitcond2, label %39, label %b25

39:
  %40 = getelementptr i8, ptr @"FS%", i16 200
  br label %b35

b35:
  %41 = phi ptr [ %40, %39 ], [ %62, %b41 ]
  %42 = phi i16 [ 200, %39 ], [ %61, %b41 ]
  %43 = phi float [ %60, %b41 ], [ 0.000000e+00, %39 ]
  %44 = fcmp oeq float %43, 0.000000e+00
  %45 = select i1 %44, float 0x3F847AE140000000, float %43
  %46 = fdiv float %45, 0x404CA66660000000
  %47 = tail call float @llvm.fabs.f32(float %46)
  %48 = fcmp ogt float %47, 0x41E0000000000000
  br i1 %48, label %b40, label %b41

b36:
  ret void

b40:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b41:
  %49 = tail call float @llvm.sin.f32(float %46)
  %50 = fdiv float 2.400000e+01, %49
  %51 = fpround float %50 to float
  %52 = fsub float 2.550000e+02, %51
  %53 = fpround float %52 to float
  %54 = fcmp olt float %53, 0.000000e+00
  %55 = tail call i16 @llvm.lrint.i16.f32(float %53)
  %56 = select i1 %54, i16 0, i16 %55
  %57 = getelementptr i8, ptr @"FS%", i16 %42
  %58 = getelementptr i8, ptr %57, i16 -2
  store i16 %56, ptr %58, !tbaa !2
  store i16 %56, ptr %41, !tbaa !2
  %59 = fadd float %45, 0x3FD47AE140000000
  %60 = fpround float %59 to float
  %61 = add nsw i16 %42, -2
  %62 = getelementptr i8, ptr %41, i16 2
  %exitcond = icmp eq i16 %61, 0
  br i1 %exitcond, label %b36, label %b35
}

define cc 1000 void @PREHISTORICODE() addrspace(1) noinline "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %slot = alloca i8
  %view = alloca [4 x i8]
  %slot1 = alloca i8
  %view1 = alloca [4 x i8]
  %0 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr nonnull %0, i8 0, i16 4, i1 false)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 12, i16 2)
  %1 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %2 = icmp eq i8 %1, 1
  br i1 %2, label %12, label %3

3:
  br label %4

4:
  %5 = phi i16 [ 15, %3 ], [ %48, %43 ]
  %6 = phi i16 [ 0, %3 ], [ %47, %43 ]
  %7 = phi i16 [ 0, %3 ], [ %46, %43 ]
  %8 = phi i16 [ 0, %3 ], [ %45, %43 ]
  %9 = phi i16 [ 0, %3 ], [ %44, %43 ]
  %10 = load i16, ptr @"XIT%", !tbaa !2
  %11 = icmp eq i16 %10, 1
  br i1 %11, label %b3, label %13

12:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b3

b3:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %0)
  ret void

13:
  call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 15, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 15, i16 1, i16 18, i16 4)
  store i8 34, ptr %slot1
  store i16 1, ptr %view1
  %14 = getelementptr i8, ptr %view1, i16 2
  store ptr %slot1, ptr %14
  %15 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string232, ptr nonnull %view1)
  %16 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %15, ptr @$string233)
  store i8 34, ptr %slot
  store i16 1, ptr %view
  %17 = getelementptr i8, ptr %view, i16 2
  store ptr %slot, ptr %17
  %18 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %16, ptr nonnull %view)
  %19 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %18, ptr @$string234)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %19)
  %20 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %21 = load float, ptr %20
  %22 = fmul float %21, 6.300000e+01
  %23 = fadd float %22, 1.000000e+00
  %24 = call float @llvm.rint.f32(float %23)
  %25 = fcmp olt float %23, %24
  %26 = sext i1 %25 to i16
  %27 = sitofp i16 %26 to float
  %28 = fadd float %24, %27
  %29 = call i16 @llvm.lrint.i16.f32(float %28)
  %30 = sitofp i16 %29 to float
  br label %b11

b11:
  %31 = phi i16 [ 479, %13 ], [ %37, %b11 ]
  %32 = phi i16 [ -14, %13 ], [ %36, %b11 ]
  %IV.S. = phi float [ 1.000000e+00, %13 ], [ %IV.S.next., %b11 ]
  %33 = add i16 %32, 15
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %33, i16 480)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 640, i16 %31)
  %34 = fdiv float %IV.S., %30
  %35 = call i16 @llvm.lrint.i16.f32(float %34)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %35, i16 -1, i16 0)
  %IV.S.next. = fadd float %IV.S., 1.500000e+01
  %36 = add nsw i16 %32, 15
  %37 = add nsw i16 %31, -15
  %38 = icmp samesign ult i16 %36, 626
  br i1 %38, label %b11, label %b12

b12:
  %39 = add i16 %6, 1
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %7, i16 %8)
  %40 = sitofp i16 %39 to float
  %41 = fpround float %40 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %41, i16 %5)
  %42 = icmp eq i16 %39, 30
  br i1 %42, label %43, label %b15

43:
  %44 = phi i16 [ %73, %b19 ], [ 1, %b12 ]
  %45 = phi i16 [ %74, %b19 ], [ %8, %b12 ]
  %46 = phi i16 [ %75, %b19 ], [ %7, %b12 ]
  %47 = phi i16 [ %76, %b19 ], [ 0, %b12 ]
  %48 = phi i16 [ %77, %b19 ], [ 0, %b12 ]
  %49 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %50 = icmp eq i8 %49, 1
  br i1 %50, label %12, label %4

b15:
  %51 = sub nsw i16 0, %9
  %52 = icmp eq i16 %39, 29
  %53 = select i1 %52, i16 %51, i16 0
  %54 = icmp eq i16 %53, 0
  br i1 %54, label %b19, label %b17

b17:
  %55 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %56 = load float, ptr %55
  %57 = fmul float %56, 6.400000e+02
  %58 = call float @llvm.rint.f32(float %57)
  %59 = fcmp olt float %57, %58
  %60 = sext i1 %59 to i16
  %61 = sitofp i16 %60 to float
  %62 = fadd float %58, %61
  %63 = call i16 @llvm.lrint.i16.f32(float %62)
  %64 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %65 = load float, ptr %64
  %66 = fmul float %65, 4.800000e+02
  %67 = call float @llvm.rint.f32(float %66)
  %68 = fcmp olt float %66, %67
  %69 = sext i1 %68 to i16
  %70 = sitofp i16 %69 to float
  %71 = fadd float %67, %70
  %72 = call i16 @llvm.lrint.i16.f32(float %71)
  br label %b19

b19:
  %73 = phi i16 [ 0, %b17 ], [ %9, %b15 ]
  %74 = phi i16 [ %72, %b17 ], [ %8, %b15 ]
  %75 = phi i16 [ %63, %b17 ], [ %7, %b15 ]
  %76 = phi i16 [ 0, %b17 ], [ %39, %b15 ]
  %77 = phi i16 [ 15, %b17 ], [ %5, %b15 ]
  %78 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %78, ptr nonnull %0)
  %79 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr nonnull %0, ptr @$string235)
  %80 = icmp eq i16 %79, 0
  br i1 %80, label %43, label %b22

b22:
  %81 = load i16, ptr @"XIT%", !tbaa !2
  br label %82

82:
  %83 = phi i16 [ 1, %b22 ], [ %94, %b31 ]
  %84 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %85 = icmp eq i8 %84, 1
  br i1 %85, label %12, label %86

86:
  %87 = icmp eq i16 %81, 1
  br i1 %87, label %b3, label %b31

b28:
  %88 = load i16, ptr @"XIT%", !tbaa !2
  br label %95

b31:
  %89 = uitofp nneg i16 %83 to float
  %90 = fmul float %89, 6.250000e-02
  %91 = fsub float 3.100000e+01, %90
  %92 = call i16 @llvm.lrint.i16.f32(float %91)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 320, i16 240)
  %93 = fpround float %89 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %93, i16 %92)
  %94 = add nuw nsw i16 %83, 1
  %exitcond1 = icmp eq i16 %94, 481
  br i1 %exitcond1, label %b28, label %82

95:
  %96 = phi i16 [ 1, %b28 ], [ %103, %b45 ]
  %97 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %98 = icmp eq i8 %97, 1
  br i1 %98, label %12, label %99

99:
  %100 = icmp eq i16 %88, 1
  br i1 %100, label %b3, label %b45

b45:
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 320, i16 240)
  %101 = uitofp nneg i16 %96 to float
  %102 = fpround float %101 to float
  call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %102, i16 0)
  %103 = add nuw nsw i16 %96, 1
  %exitcond = icmp eq i16 %103, 641
  br i1 %exitcond, label %b3, label %95
}

define cc 1000 void @RGBLIGHTS() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  br label %b6

b2:
  ret void

b6:
  %0 = phi i16 [ 0, %b1 ], [ %1, %b6 ]
  %tmp = trunc i16 %0 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %tmp) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %1 = add nuw nsw i16 %0, 1
  %exitcond13 = icmp eq i16 %1, 128
  br i1 %exitcond13, label %b7, label %b6

b7:
  tail call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R1%", i16 2), !tbaa !2
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R1%", i16 10), !tbaa !2
  %4 = inttoptr i16 %2 to ptr addrspace(2)
  %5 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)
  %6 = tail call i16 @llvm.lrint.i16.f32(float 0x4029333340000000)
  %7 = trunc i16 %6 to i8
  %8 = tail call i16 @llvm.lrint.i16.f32(float 0x4039333340000000)
  %9 = trunc i16 %8 to i8
  %10 = fmul float 3.000000e+00, 0x4029333340000000
  %11 = tail call i16 @llvm.lrint.i16.f32(float %10)
  %12 = trunc i16 %11 to i8
  %13 = tail call i16 @llvm.lrint.i16.f32(float 0x4049333340000000)
  %14 = trunc i16 %13 to i8
  %15 = fmul float 5.000000e+00, 0x4029333340000000
  %16 = tail call i16 @llvm.lrint.i16.f32(float %15)
  %17 = trunc i16 %16 to i8
  %18 = getelementptr i8, ptr addrspace(1) %5, i16 10
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 %3
  br label %b11

b11:
  %20 = phi i16 [ 0, %b7 ], [ %55, %b17 ]
  %21 = phi i16 [ 0, %b7 ], [ %58, %b17 ]
  %22 = sitofp i16 %21 to float
  %23 = fmul float %22, 0x4029333340000000
  %24 = tail call i16 @llvm.lrint.i16.f32(float %23)
  %25 = trunc i16 %24 to i8
  br label %b16

b12:
  store i32 393216, ptr @"EPI6%"
  store i32 2359296, ptr @"EPI36%"
  store i32 1179660, ptr getelementptr inbounds nuw (i8, ptr @"EPI6%", i16 4)
  store i32 7077960, ptr getelementptr inbounds nuw (i8, ptr @"EPI36%", i16 4)
  store i32 1966104, ptr getelementptr inbounds nuw (i8, ptr @"EPI6%", i16 8)
  store i32 11796624, ptr getelementptr inbounds nuw (i8, ptr @"EPI36%", i16 8)
  %26 = tail call i32 @llvm.lrint.i32.f32(float 0x40019999A0000000)
  %27 = sitofp i32 %26 to float
  br label %b31

b16:
  %28 = phi i16 [ %56, %b16 ], [ 6, %b11 ]
  %29 = phi i16 [ %20, %b11 ], [ %57, %b16 ]
  %IV.S. = phi float [ 0.000000e+00, %b11 ], [ %IV.S.next., %b16 ]
  %30 = fmul float %IV.S., 0x4029333340000000
  %31 = tail call i16 @llvm.lrint.i16.f32(float %30)
  %32 = trunc i16 %31 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %25) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %32) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 0) memory(inaccessiblemem: readwrite)
  %33 = shl i16 %29, 1
  %34 = getelementptr i8, ptr addrspace(1) %19, i16 %33
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 -10
  store i16 %29, ptr addrspace(1) %35, !tbaa !72
  tail call void @llrm.ia16.out.i8(i16 969, i8 %25) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %32) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %7) memory(inaccessiblemem: readwrite)
  %36 = shl i16 %29, 1
  %37 = getelementptr i8, ptr addrspace(1) %19, i16 %36
  %38 = getelementptr i8, ptr addrspace(1) %37, i16 -8
  %39 = add i16 %29, 1
  store i16 %39, ptr addrspace(1) %38, !tbaa !72
  tail call void @llrm.ia16.out.i8(i16 969, i8 %25) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %32) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %9) memory(inaccessiblemem: readwrite)
  %40 = shl i16 %29, 1
  %41 = getelementptr i8, ptr addrspace(1) %19, i16 %40
  %42 = getelementptr i8, ptr addrspace(1) %41, i16 -6
  %43 = add i16 %39, 1
  store i16 %43, ptr addrspace(1) %42, !tbaa !72
  tail call void @llrm.ia16.out.i8(i16 969, i8 %25) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %32) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %12) memory(inaccessiblemem: readwrite)
  %44 = shl i16 %29, 1
  %45 = getelementptr i8, ptr addrspace(1) %19, i16 %44
  %46 = getelementptr i8, ptr addrspace(1) %45, i16 -4
  %47 = add i16 %43, 1
  store i16 %47, ptr addrspace(1) %46, !tbaa !72
  tail call void @llrm.ia16.out.i8(i16 969, i8 %25) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %32) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %14) memory(inaccessiblemem: readwrite)
  %48 = shl i16 %29, 1
  %49 = getelementptr i8, ptr addrspace(1) %19, i16 %48
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 -2
  %51 = add i16 %47, 1
  store i16 %51, ptr addrspace(1) %50, !tbaa !72
  tail call void @llrm.ia16.out.i8(i16 969, i8 %25) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %32) memory(inaccessiblemem: readwrite)
  tail call void @llrm.ia16.out.i8(i16 969, i8 %17) memory(inaccessiblemem: readwrite)
  %52 = shl i16 %29, 1
  %53 = getelementptr i8, ptr addrspace(1) %19, i16 %52
  %54 = add i16 %51, 1
  store i16 %54, ptr addrspace(1) %53, !tbaa !72
  %55 = add i16 %29, 6
  %IV.S.next. = fadd float %IV.S., 1.000000e+00
  %56 = add nsw i16 %28, -1
  %exitcond11 = icmp eq i16 %56, 0
  %57 = add i16 %54, 1
  br i1 %exitcond11, label %b17, label %b16

b17:
  %58 = add nuw nsw i16 %21, 1
  %exitcond12 = icmp eq i16 %58, 6
  br i1 %exitcond12, label %b12, label %b11

b31:
  %59 = phi i16 [ 0, %b12 ], [ %75, %b37 ]
  %60 = phi i16 [ 0, %b12 ], [ %74, %b37 ]
  %61 = sub nuw nsw i16 99, %60
  %62 = uitofp nneg i16 %61 to float
  %63 = fpround float %62 to float
  %64 = fmul float %63, %63
  %65 = mul i16 %59, -1
  %66 = shl i16 %59, 1
  %67 = getelementptr i8, ptr @"SP%", i16 %66
  br label %b36

b36:
  %68 = phi i16 [ %118, %b60 ], [ 159, %b31 ]
  %69 = phi i16 [ %117, %b60 ], [ -320, %b31 ]
  %70 = phi i16 [ %116, %b60 ], [ %65, %b31 ]
  %71 = icmp eq i16 %60, 99
  %72 = icmp eq i16 %68, 0
  %73 = and i1 %71, %72
  br i1 %73, label %b37.thread, label %b39

b37:
  %74 = add nuw nsw i16 %60, 1
  %exitcond8 = icmp eq i16 %74, 100
  %75 = mul i16 %116, -1
  br i1 %exitcond8, label %80, label %b31

b37.thread:
  %76 = shl i16 %59, 1
  %77 = getelementptr i8, ptr @"SP%", i16 %76
  %78 = getelementptr i8, ptr %77, i16 %69
  %79 = getelementptr i8, ptr %78, i16 320
  store i16 255, ptr %79, !tbaa !2
  br label %80

80:
  br label %b64

b39:
  %81 = uitofp nneg i16 %68 to float
  %82 = fpround float %81 to float
  %83 = fmul float %82, %82
  %84 = fadd float %64, %83
  %85 = tail call float @llvm.sqrt.f32(float %84)
  %86 = fpround float %85 to float
  %87 = fcmp ogt float %86, 0.000000e+00
  br i1 %87, label %b41, label %b42

b40:
  %88 = sdiv i32 16384, %106
  %89 = trunc nsw i32 %88 to i16
  %90 = getelementptr i8, ptr @"SP%", i16 %66
  %91 = getelementptr i8, ptr %90, i16 %69
  %92 = getelementptr i8, ptr %91, i16 320
  store i16 %89, ptr %92, !tbaa !2
  %93 = icmp sgt i32 %88, 255
  br i1 %93, label %b58, label %b60

b41:
  %94 = tail call float @llvm.log2.f32(float %86)
  %95 = fmul float %94, 0x40019999A0000000
  %96 = fcmp oge float %95, 3.276750e+04
  br i1 %96, label %b50, label %b51

b42:
  %97 = fcmp oeq float %86, 0.000000e+00
  br i1 %97, label %b49, label %b45

b45:
  %98 = fcmp oeq float %27, 0x40019999A0000000
  br i1 %98, label %b46, label %b48

b46:
  %99 = fneg float %86
  %100 = tail call float @llvm.log2.f32(float %99)
  %101 = fmul float %100, 0x40019999A0000000
  %102 = fcmp oge float %101, 3.276750e+04
  br i1 %102, label %b54, label %b55

b47:
  %103 = fneg float %113
  %104 = fpround float %103 to float
  br label %b49

b48:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b49:
  %105 = phi float [ %109, %b51 ], [ %113, %b55 ], [ %104, %b47 ], [ 0.000000e+00, %b42 ]
  %106 = tail call i32 @llvm.lrint.i32.f32(float %105)
  %107 = icmp eq i32 %106, 0
  br i1 %107, label %b56, label %b40

b50:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b51:
  %108 = tail call float @llvm.exp2.f32(float %95)
  %109 = fpround float %108 to float
  br label %b49

b54:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b55:
  %110 = and i32 %26, 1
  %111 = icmp eq i32 %110, 0
  %112 = tail call float @llvm.exp2.f32(float %101)
  %113 = fpround float %112 to float
  br i1 %111, label %b49, label %b47

b56:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b58:
  %114 = getelementptr i8, ptr %67, i16 %69
  %115 = getelementptr i8, ptr %114, i16 320
  store i16 255, ptr %115, !tbaa !2
  br label %b60

b60:
  %116 = add i16 %70, -1
  %117 = add nsw i16 %69, 2
  %118 = add nsw i16 %68, -1
  %exitcond7 = icmp eq i16 %117, 0
  br i1 %exitcond7, label %b37, label %b36

b64:
  %119 = phi i16 [ 639, %80 ], [ %134, %b64 ]
  %120 = phi i16 [ -1920, %80 ], [ %133, %b64 ]
  %121 = phi i16 [ %132, %b64 ], [ -320, %80 ]
  %122 = ashr i16 %121, 15
  %123 = icmp sgt i16 %121, 319
  %124 = sext i1 %123 to i16
  %125 = or i16 %122, %124
  %126 = icmp eq i16 %125, 0
  %127 = icmp slt i16 %121, 160
  %128 = select i1 %127, i16 %121, i16 %119
  %129 = select i1 %126, i16 %128, i16 0
  %130 = getelementptr i8, ptr @"FSIN3%", i16 %120
  %131 = getelementptr i8, ptr %130, i16 2560
  store i16 %129, ptr %131, !tbaa !2
  %132 = add nsw i16 %121, 1
  %133 = add nsw i16 %120, 2
  %134 = add nsw i16 %119, -1
  %exitcond6 = icmp eq i16 %133, 0
  br i1 %exitcond6, label %135, label %b64

135:
  br label %b75

b75:
  %136 = phi i16 [ 499, %135 ], [ %152, %b75 ]
  %137 = phi i16 [ -1602, %135 ], [ %151, %b75 ]
  %138 = phi i16 [ %150, %b75 ], [ -300, %135 ]
  %139 = ashr i16 %138, 15
  %140 = icmp sgt i16 %138, 199
  %141 = sext i1 %140 to i16
  %142 = or i16 %139, %141
  %143 = icmp eq i16 %142, 0
  %144 = icmp sgt i16 %138, 99
  %145 = select i1 %144, i16 %136, i16 %138
  %146 = select i1 %143, i16 %145, i16 0
  %147 = getelementptr i8, ptr @"FSIN4%", i16 %137
  %148 = getelementptr i8, ptr %147, i16 1640
  %149 = mul i16 %146, 160
  store i16 %149, ptr %148, !tbaa !2
  %150 = add nsw i16 %138, 1
  %151 = add nsw i16 %137, 2
  %152 = add nsw i16 %136, -1
  %exitcond5 = icmp eq i16 %151, 0
  br i1 %exitcond5, label %413, label %b75

b90:
  %153 = phi i16 [ %156, %b90 ], [ -3574, %413 ]
  %154 = getelementptr i8, ptr @"CD%", i16 %153
  %155 = getelementptr i8, ptr %154, i16 3586
  store i16 5, ptr %155, !tbaa !2
  %156 = add nsw i16 %153, 2
  %exitcond4 = icmp eq i16 %156, 0
  br i1 %exitcond4, label %157, label %b90

157:
  br label %b94

b94:
  %158 = phi i16 [ -6, %157 ], [ %207, %b94 ]
  %159 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %160 = load float, ptr %159
  %161 = fmul float %160, 3.500000e+01
  %162 = fadd float %161, 1.500000e+01
  %163 = tail call float @llvm.rint.f32(float %162)
  %164 = fcmp olt float %162, %163
  %165 = sext i1 %164 to i16
  %166 = sitofp i16 %165 to float
  %167 = fadd float %163, %166
  %168 = tail call i16 @llvm.lrint.i16.f32(float %167)
  %169 = getelementptr i8, ptr @"DSX%", i16 %158
  %170 = getelementptr i8, ptr %169, i16 8
  store i16 %168, ptr %170, !tbaa !2
  %171 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %172 = load float, ptr %171
  %173 = fmul float %172, 3.500000e+01
  %174 = fadd float %173, 1.500000e+01
  %175 = tail call float @llvm.rint.f32(float %174)
  %176 = fcmp olt float %174, %175
  %177 = sext i1 %176 to i16
  %178 = sitofp i16 %177 to float
  %179 = fadd float %175, %178
  %180 = tail call i16 @llvm.lrint.i16.f32(float %179)
  %181 = getelementptr i8, ptr @"DSY%", i16 %158
  %182 = getelementptr i8, ptr %181, i16 8
  store i16 %180, ptr %182, !tbaa !2
  %183 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %184 = load float, ptr %183
  %185 = fmul float %184, 8.000000e+01
  %186 = fadd float %185, 8.000000e+01
  %187 = tail call float @llvm.rint.f32(float %186)
  %188 = fcmp olt float %186, %187
  %189 = sext i1 %188 to i16
  %190 = sitofp i16 %189 to float
  %191 = fadd float %187, %190
  %192 = tail call i16 @llvm.lrint.i16.f32(float %191)
  %193 = getelementptr i8, ptr @"PSX%", i16 %158
  %194 = getelementptr i8, ptr %193, i16 8
  store i16 %192, ptr %194, !tbaa !2
  %195 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %196 = load float, ptr %195
  %197 = fmul float %196, 5.000000e+01
  %198 = fadd float %197, 5.000000e+01
  %199 = tail call float @llvm.rint.f32(float %198)
  %200 = fcmp olt float %198, %199
  %201 = sext i1 %200 to i16
  %202 = sitofp i16 %201 to float
  %203 = fadd float %199, %202
  %204 = tail call i16 @llvm.lrint.i16.f32(float %203)
  %205 = getelementptr i8, ptr @"PSY%", i16 %158
  %206 = getelementptr i8, ptr %205, i16 8
  store i16 %204, ptr %206, !tbaa !2
  %207 = add nsw i16 %158, 2
  %exitcond3 = icmp eq i16 %207, 0
  br i1 %exitcond3, label %208, label %b94

208:
  br label %b97

b97:
  %209 = phi i16 [ %212, %b139 ], [ %212, %345 ], [ 40, %208 ]
  %210 = phi float [ %232, %b139 ], [ %232, %345 ], [ 8.000000e+00, %208 ]
  %211 = phi float [ %233, %b139 ], [ %233, %345 ], [ 8.000000e+00, %208 ]
  %212 = add nuw nsw i16 %209, 1
  %213 = fcmp ogt float %210, 1.000000e+00
  %214 = icmp ult i16 %209, 101
  %215 = sext i1 %214 to i16
  %216 = select i1 %213, i16 %215, i16 0
  %217 = icmp eq i16 %216, 0
  %218 = fsub float %210, 2.500000e-01
  %219 = fpround float %218 to float
  %220 = select i1 %217, float %210, float %219
  %221 = fcmp ogt float %211, 1.000000e+00
  %222 = select i1 %221, i16 %215, i16 0
  %223 = icmp eq i16 %222, 0
  %224 = fsub float %211, 2.500000e-01
  %225 = fpround float %224 to float
  %226 = select i1 %223, float %211, float %225
  %227 = icmp ugt i16 %209, 101
  %228 = fadd float %220, 0x3FB99999A0000000
  %229 = fpround float %228 to float
  %230 = fadd float %226, 0x3FB99999A0000000
  %231 = fpround float %230 to float
  %232 = select i1 %227, float %229, float %220
  %233 = select i1 %227, float %231, float %226
  %234 = icmp eq i16 %212, 122
  br i1 %234, label %b2, label %b110

b110:
  %235 = sitofp i16 %212 to float
  %236 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSY%", i16 2), !tbaa !2
  %237 = sitofp i16 %236 to float
  %238 = fmul float %233, %237
  %239 = tail call i16 @llvm.lrint.i16.f32(float %238)
  %240 = tail call i16 @llvm.smin.i16(i16 %239, i16 300)
  %241 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSX%", i16 2), !tbaa !2
  %242 = sitofp i16 %241 to float
  %243 = fdiv float %235, %242
  %244 = tail call float @llvm.fabs.f32(float %243)
  %245 = fcmp ogt float %244, 0x41E0000000000000
  br i1 %245, label %b123, label %b124

b123:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b124:
  %246 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSX%", i16 2), !tbaa !2
  %247 = sitofp i16 %246 to float
  %248 = fmul float %232, %247
  %249 = tail call i16 @llvm.lrint.i16.f32(float %248)
  %250 = tail call i16 @llvm.smin.i16(i16 %249, i16 320)
  %251 = tail call float @llvm.sin.f32(float %243)
  %252 = sitofp i16 %250 to float
  %253 = fmul float %251, %252
  %254 = tail call i16 @llvm.lrint.i16.f32(float %253)
  store i16 %254, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 2), !tbaa !2
  %255 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSY%", i16 2), !tbaa !2
  %256 = sitofp i16 %255 to float
  %257 = fdiv float %235, %256
  %258 = tail call float @llvm.fabs.f32(float %257)
  %259 = fcmp ogt float %258, 0x41E0000000000000
  br i1 %259, label %b125, label %b126

b125:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b126:
  %260 = tail call float @llvm.sin.f32(float %257)
  %261 = sitofp i16 %240 to float
  %262 = fmul float %260, %261
  %263 = tail call i16 @llvm.lrint.i16.f32(float %262)
  store i16 %263, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 2), !tbaa !2
  %264 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSY%", i16 4), !tbaa !2
  %265 = sitofp i16 %264 to float
  %266 = fmul float %233, %265
  %267 = tail call i16 @llvm.lrint.i16.f32(float %266)
  %268 = tail call i16 @llvm.smin.i16(i16 %267, i16 300)
  %269 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSX%", i16 4), !tbaa !2
  %270 = sitofp i16 %269 to float
  %271 = fdiv float %235, %270
  %272 = tail call float @llvm.fabs.f32(float %271)
  %273 = fcmp ogt float %272, 0x41E0000000000000
  br i1 %273, label %b123, label %274

274:
  %275 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSX%", i16 4), !tbaa !2
  %276 = sitofp i16 %275 to float
  %277 = fmul float %232, %276
  %278 = tail call i16 @llvm.lrint.i16.f32(float %277)
  %279 = tail call i16 @llvm.smin.i16(i16 %278, i16 320)
  %280 = tail call float @llvm.sin.f32(float %271)
  %281 = sitofp i16 %279 to float
  %282 = fmul float %280, %281
  %283 = tail call i16 @llvm.lrint.i16.f32(float %282)
  store i16 %283, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 4), !tbaa !2
  %284 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSY%", i16 4), !tbaa !2
  %285 = sitofp i16 %284 to float
  %286 = fdiv float %235, %285
  %287 = tail call float @llvm.fabs.f32(float %286)
  %288 = fcmp ogt float %287, 0x41E0000000000000
  br i1 %288, label %b125, label %289

289:
  %290 = tail call float @llvm.sin.f32(float %286)
  %291 = sitofp i16 %268 to float
  %292 = fmul float %290, %291
  %293 = tail call i16 @llvm.lrint.i16.f32(float %292)
  store i16 %293, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 4), !tbaa !2
  %294 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSY%", i16 6), !tbaa !2
  %295 = sitofp i16 %294 to float
  %296 = fmul float %233, %295
  %297 = tail call i16 @llvm.lrint.i16.f32(float %296)
  %298 = tail call i16 @llvm.smin.i16(i16 %297, i16 300)
  %299 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSX%", i16 6), !tbaa !2
  %300 = sitofp i16 %299 to float
  %301 = fdiv float %235, %300
  %302 = tail call float @llvm.fabs.f32(float %301)
  %303 = fcmp ogt float %302, 0x41E0000000000000
  br i1 %303, label %b123, label %304

304:
  %305 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"PSX%", i16 6), !tbaa !2
  %306 = sitofp i16 %305 to float
  %307 = fmul float %232, %306
  %308 = tail call i16 @llvm.lrint.i16.f32(float %307)
  %309 = tail call i16 @llvm.smin.i16(i16 %308, i16 320)
  %310 = tail call float @llvm.sin.f32(float %301)
  %311 = sitofp i16 %309 to float
  %312 = fmul float %310, %311
  %313 = tail call i16 @llvm.lrint.i16.f32(float %312)
  store i16 %313, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 6), !tbaa !2
  %314 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"DSY%", i16 6), !tbaa !2
  %315 = sitofp i16 %314 to float
  %316 = fdiv float %235, %315
  %317 = tail call float @llvm.fabs.f32(float %316)
  %318 = fcmp ogt float %317, 0x41E0000000000000
  br i1 %318, label %b125, label %319

319:
  %320 = tail call float @llvm.sin.f32(float %316)
  %321 = sitofp i16 %298 to float
  %322 = fmul float %320, %321
  %323 = tail call i16 @llvm.lrint.i16.f32(float %322)
  store i16 %323, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 6), !tbaa !2
  br label %b130

b130:
  %324 = phi i16 [ 0, %319 ], [ %327, %b136 ]
  %325 = phi i16 [ 0, %319 ], [ %408, %b136 ]
  %326 = add nuw nsw i16 %324, -24576
  store i16 %326, ptr @b$seg, !tbaa !2
  %327 = add nuw nsw i16 %324, 20
  %328 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 2), !tbaa !2
  %329 = sub i16 %325, %328
  %330 = getelementptr [2 x i8], ptr @"FSIN4%", i16 %329
  %331 = getelementptr i8, ptr %330, i16 638
  %332 = load i16, ptr %331, !tbaa !2
  store i16 %332, ptr @"YY%.0", !tbaa !2
  %333 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 4), !tbaa !2
  %334 = sub i16 %325, %333
  %335 = getelementptr [2 x i8], ptr @"FSIN4%", i16 %334
  %336 = getelementptr i8, ptr %335, i16 638
  %337 = load i16, ptr %336, !tbaa !2
  store i16 %337, ptr @"YY%.1", !tbaa !2
  %338 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YP1%", i16 6), !tbaa !2
  %339 = sub i16 %325, %338
  %340 = getelementptr [2 x i8], ptr @"FSIN4%", i16 %339
  %341 = getelementptr i8, ptr %340, i16 638
  %342 = load i16, ptr %341, !tbaa !2
  store i16 %342, ptr @"YY%.2", !tbaa !2
  br label %b135

b131:
  %343 = load i16, ptr @"XIT%", !tbaa !2
  %344 = icmp eq i16 %343, 0
  br i1 %344, label %409, label %345

345:
  %346 = icmp eq i16 %343, 1
  br i1 %346, label %b2, label %b97

b135:
  %347 = phi i16 [ %407, %b135 ], [ -320, %b130 ]
  %348 = load i16, ptr @"YY%.0", !tbaa !2
  %349 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 2), !tbaa !2
  %350 = mul i16 %349, -2
  %351 = shl nsw i16 %347, 1
  %352 = add i16 %350, %351
  %353 = getelementptr i8, ptr @"FSIN3%", i16 %352
  %354 = getelementptr i8, ptr %353, i16 1920
  %355 = load i16, ptr %354, !tbaa !2
  %356 = getelementptr [2 x i8], ptr @"SP%", i16 %348
  %357 = getelementptr [2 x i8], ptr %356, i16 %355
  %358 = load i16, ptr %357, !tbaa !2
  %359 = getelementptr inbounds [2 x i8], ptr @"CD%", i16 %358
  %360 = load i16, ptr %359, !tbaa !2
  %361 = load i16, ptr @"YY%.1", !tbaa !2
  %362 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 4), !tbaa !2
  %363 = mul i16 %362, -2
  %364 = shl nsw i16 %347, 1
  %365 = add i16 %363, %364
  %366 = getelementptr i8, ptr @"FSIN3%", i16 %365
  %367 = getelementptr i8, ptr %366, i16 1920
  %368 = load i16, ptr %367, !tbaa !2
  %369 = getelementptr [2 x i8], ptr @"SP%", i16 %361
  %370 = getelementptr [2 x i8], ptr %369, i16 %368
  %371 = load i16, ptr %370, !tbaa !2
  %372 = getelementptr inbounds [2 x i8], ptr @"CD%", i16 %371
  %373 = load i16, ptr %372, !tbaa !2
  %374 = load i16, ptr @"YY%.2", !tbaa !2
  %375 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XP1%", i16 6), !tbaa !2
  %376 = mul i16 %375, -2
  %377 = shl nsw i16 %347, 1
  %378 = add i16 %376, %377
  %379 = getelementptr i8, ptr @"FSIN3%", i16 %378
  %380 = getelementptr i8, ptr %379, i16 1920
  %381 = load i16, ptr %380, !tbaa !2
  %382 = getelementptr [2 x i8], ptr @"SP%", i16 %374
  %383 = getelementptr [2 x i8], ptr %382, i16 %381
  %384 = load i16, ptr %383, !tbaa !2
  %385 = getelementptr inbounds [2 x i8], ptr @"CD%", i16 %384
  %386 = load i16, ptr %385, !tbaa !2
  %387 = getelementptr inbounds [2 x i8], ptr @"EPI6%", i16 %373
  %388 = load i16, ptr %387, !tbaa !2
  %389 = add i16 %388, %360
  %390 = getelementptr inbounds [2 x i8], ptr @"EPI36%", i16 %386
  %391 = load i16, ptr %390, !tbaa !2
  %392 = add i16 %389, %391
  %393 = shl i16 %392, 1
  %394 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R1%", i16 2), !tbaa !2
  %395 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R1%", i16 10), !tbaa !2
  %396 = inttoptr i16 %394 to ptr addrspace(2)
  %397 = addrspacecast ptr addrspace(2) %396 to ptr addrspace(1)
  %398 = getelementptr i8, ptr addrspace(1) %397, i16 %395
  %399 = getelementptr i8, ptr addrspace(1) %398, i16 %393
  %400 = load i16, ptr addrspace(1) %399, !tbaa !72
  %401 = trunc i16 %400 to i8
  %402 = load i16, ptr @b$seg, !tbaa !2
  %403 = inttoptr i16 %402 to ptr addrspace(2)
  %404 = addrspacecast ptr addrspace(2) %403 to ptr addrspace(1)
  %405 = getelementptr i8, ptr addrspace(1) %404, i16 %347
  %406 = getelementptr i8, ptr addrspace(1) %405, i16 320
  store volatile i8 %401, ptr addrspace(1) %406
  %407 = add nsw i16 %347, 1
  %exitcond = icmp eq i16 %407, 0
  br i1 %exitcond, label %b136, label %b135

b136:
  %408 = add nuw nsw i16 %325, 1
  %exitcond1 = icmp eq i16 %408, 200
  br i1 %exitcond1, label %b131, label %b130

409:
  %410 = tail call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %411 = icmp eq i8 %410, 1
  br i1 %411, label %b139.thread, label %b139

b139.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b2

b139:
  %412 = icmp eq i16 %343, 1
  br i1 %412, label %b2, label %b97

413:
  store i32 65536, ptr @"CD%"
  store i32 196610, ptr getelementptr inbounds nuw (i8, ptr @"CD%", i16 4)
  store i32 327684, ptr getelementptr inbounds nuw (i8, ptr @"CD%", i16 8)
  br label %b90
}

define cc 1000 void @ROTATE3D(ptr dereferenceable(4) nocapture readonly %0, ptr dereferenceable(4) nocapture readonly %1, ptr dereferenceable(4) nocapture readonly %2) addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %3 = load float, ptr %0
  %4 = tail call float @llvm.fabs.f32(float %3)
  %5 = fcmp ogt float %4, 0x41E0000000000000
  br i1 %5, label %b2, label %b3

b2:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b3:
  %6 = tail call float @llvm.cos.f32(float %3)
  %7 = fpround float %6 to float
  %8 = load float, ptr %1
  %9 = tail call float @llvm.fabs.f32(float %8)
  %10 = fcmp ogt float %9, 0x41E0000000000000
  br i1 %10, label %b4, label %b5

b4:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b5:
  %11 = tail call float @llvm.cos.f32(float %8)
  %12 = fpround float %11 to float
  %13 = load float, ptr %2
  %14 = tail call float @llvm.fabs.f32(float %13)
  %15 = fcmp ogt float %14, 0x41E0000000000000
  br i1 %15, label %b6, label %b13

b6:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b13:
  %16 = tail call float @llvm.cos.f32(float %13)
  %17 = fpround float %16 to float
  %18 = tail call float @llvm.sin.f32(float %3)
  %19 = fpround float %18 to float
  %20 = tail call float @llvm.sin.f32(float %8)
  %21 = fpround float %20 to float
  %22 = tail call float @llvm.sin.f32(float %13)
  %23 = fpround float %22 to float
  %24 = load i16, ptr @"NDTS%", !tbaa !2
  %25 = add i16 %24, -1
  %26 = icmp slt i16 %25, 0
  br i1 %26, label %b18, label %27

27:
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"X!", i16 2), !tbaa !2
  %29 = inttoptr i16 %28 to ptr addrspace(2)
  %30 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XO%", i16 2), !tbaa !2
  %31 = inttoptr i16 %30 to ptr addrspace(2)
  %32 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ZO%", i16 2), !tbaa !2
  %33 = inttoptr i16 %32 to ptr addrspace(2)
  %34 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Z!", i16 2), !tbaa !2
  %35 = inttoptr i16 %34 to ptr addrspace(2)
  %36 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Y!", i16 2), !tbaa !2
  %37 = inttoptr i16 %36 to ptr addrspace(2)
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YO%", i16 2), !tbaa !2
  %39 = inttoptr i16 %38 to ptr addrspace(2)
  br label %b17

b17:
  %40 = phi i16 [ %88, %b17 ], [ 0, %27 ]
  %41 = phi i16 [ %87, %b17 ], [ %24, %27 ]
  %42 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %43 = addrspacecast ptr addrspace(2) %37 to ptr addrspace(1)
  %44 = addrspacecast ptr addrspace(2) %35 to ptr addrspace(1)
  %45 = addrspacecast ptr addrspace(2) %33 to ptr addrspace(1)
  %46 = addrspacecast ptr addrspace(2) %31 to ptr addrspace(1)
  %47 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %48 = shl i16 %40, 1
  %49 = getelementptr i8, ptr addrspace(1) %47, i16 %48
  %50 = getelementptr i8, ptr addrspace(1) %46, i16 %40
  %51 = load i16, ptr addrspace(1) %50, !tbaa !16
  %52 = sitofp i16 %51 to float
  %53 = fmul float %12, %52
  %54 = getelementptr i8, ptr addrspace(1) %45, i16 %40
  %55 = load i16, ptr addrspace(1) %54, !tbaa !18
  %56 = sitofp i16 %55 to float
  %57 = fmul float %21, %56
  %58 = fsub float %53, %57
  %59 = fpround float %58 to float
  %60 = shl i16 %40, 1
  %61 = getelementptr i8, ptr addrspace(1) %44, i16 %60
  %62 = fmul float %21, %52
  %63 = fmul float %12, %56
  %64 = fadd float %62, %63
  %65 = fpround float %64 to float
  %66 = shl i16 %40, 1
  %67 = getelementptr i8, ptr addrspace(1) %43, i16 %66
  %68 = getelementptr i8, ptr addrspace(1) %42, i16 %40
  %69 = load i16, ptr addrspace(1) %68, !tbaa !20
  %70 = sitofp i16 %69 to float
  %71 = fmul float %7, %70
  %72 = fmul float %19, %65
  %73 = fsub float %71, %72
  %74 = fpround float %73 to float
  %75 = fmul float %19, %70
  %76 = fmul float %7, %65
  %77 = fadd float %76, %75
  %78 = fpround float %77 to float
  store float %78, ptr addrspace(1) %61, !tbaa !56
  %79 = fmul float %17, %59
  %80 = fmul float %23, %74
  %81 = fsub float %79, %80
  %82 = fpround float %81 to float
  store float %82, ptr addrspace(1) %49, !tbaa !54
  %83 = fmul float %23, %59
  %84 = fmul float %17, %74
  %85 = fadd float %83, %84
  %86 = fpround float %85 to float
  store float %86, ptr addrspace(1) %67, !tbaa !58
  %87 = add i16 %41, -1
  %88 = add i16 %40, 2
  %exitcond = icmp eq i16 %87, 0
  br i1 %exitcond, label %b18, label %b17

b18:
  ret void
}

define cc 1000 void @SAVERECORD() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  %slot = alloca i8
  %view = alloca [4 x i8]
  %slot1 = alloca i8
  %view1 = alloca [4 x i8]
  %slot2 = alloca i8
  %view2 = alloca [4 x i8]
  %slot3 = alloca i8
  %view3 = alloca [4 x i8]
  %1 = load i16, ptr @"KON%.3", !tbaa !2
  %2 = trunc i16 %1 to i8
  store i8 %2, ptr %slot3
  store i16 1, ptr %view3
  %3 = getelementptr inbounds nuw i8, ptr %view3, i16 2
  store ptr %slot3, ptr %3
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr nonnull %view3, ptr @K$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) addrspacecast (ptr @K$ to ptr addrspace(1)), i16 0)
  %4 = load i16, ptr @"KON%.4", !tbaa !2
  %5 = trunc i16 %4 to i8
  store i8 %5, ptr %slot2
  store i16 1, ptr %view2
  %6 = getelementptr inbounds nuw i8, ptr %view2, i16 2
  store ptr %slot2, ptr %6
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr nonnull %view2, ptr @K$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) addrspacecast (ptr @K$ to ptr addrspace(1)), i16 0)
  %7 = load i16, ptr @"KON%.2", !tbaa !2
  %8 = trunc i16 %7 to i8
  store i8 %8, ptr %slot1
  store i16 1, ptr %view1
  %9 = getelementptr inbounds nuw i8, ptr %view1, i16 2
  store ptr %slot1, ptr %9
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr nonnull %view1, ptr @K$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) addrspacecast (ptr @K$ to ptr addrspace(1)), i16 0)
  %10 = load i16, ptr @"KON%.5", !tbaa !2
  %11 = trunc i16 %10 to i8
  store i8 %11, ptr %slot
  store i16 1, ptr %view
  %12 = getelementptr inbounds nuw i8, ptr %view, i16 2
  store ptr %slot, ptr %12
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr nonnull %view, ptr @K$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT3(i16 2, ptr addrspace(1) addrspacecast (ptr @K$ to ptr addrspace(1)), i16 0)
  ret void
}

define cc 1000 void @SETCROSFADEPAL(ptr dereferenceable(18) nocapture readonly %0, ptr dereferenceable(18) nocapture readonly %1, ptr dereferenceable(18) nocapture readonly %2, ptr dereferenceable(18) nocapture readonly %3, ptr dereferenceable(18) nocapture readonly %4, ptr dereferenceable(18) nocapture readonly %5) addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: readwrite, inaccessiblemem: readwrite) nounwind nofree norecurse willreturn {
b1:
  tail call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  br label %b5

b5:
  %6 = phi i16 [ %37, %b11 ], [ 0, %b1 ]
  %7 = phi i16 [ 0, %b1 ], [ %36, %b11 ]
  br label %b10

b6:
  br label %b15

b10:
  %8 = phi i16 [ %35, %b10 ], [ %6, %b5 ]
  %9 = phi i16 [ %34, %b10 ], [ 0, %b5 ]
  %tmp2 = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp2) memory(inaccessiblemem: readwrite)
  %10 = getelementptr i8, ptr %0, i16 2
  %11 = load i16, ptr %10
  %12 = getelementptr i8, ptr %0, i16 10
  %13 = load i16, ptr %12
  %14 = inttoptr i16 %11 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr addrspace(1) %15, i16 %13
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 %8
  store i16 %9, ptr addrspace(1) %17
  %tmp1 = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp1) memory(inaccessiblemem: readwrite)
  %18 = getelementptr i8, ptr %2, i16 2
  %19 = load i16, ptr %18
  %20 = getelementptr i8, ptr %2, i16 10
  %21 = load i16, ptr %20
  %22 = inttoptr i16 %19 to ptr addrspace(2)
  %23 = addrspacecast ptr addrspace(2) %22 to ptr addrspace(1)
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 %21
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 %8
  store i16 %9, ptr addrspace(1) %25
  %tmp = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %tmp) memory(inaccessiblemem: readwrite)
  %26 = getelementptr i8, ptr %4, i16 2
  %27 = load i16, ptr %26
  %28 = getelementptr i8, ptr %4, i16 10
  %29 = load i16, ptr %28
  %30 = inttoptr i16 %27 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 %8
  store i16 %9, ptr addrspace(1) %33
  %34 = add nuw nsw i16 %9, 4
  %35 = add nuw nsw i16 %8, 2
  %exitcond1 = icmp eq i16 %34, 64
  br i1 %exitcond1, label %b11, label %b10

b11:
  %36 = add nuw nsw i16 %7, 1
  %exitcond2 = icmp eq i16 %36, 16
  %37 = add nuw nsw i16 %6, 32
  br i1 %exitcond2, label %b6, label %b5

b15:
  %38 = phi i16 [ 0, %b6 ], [ %68, %b15 ]
  %39 = getelementptr i8, ptr %1, i16 2
  %40 = load i16, ptr %39
  %41 = getelementptr i8, ptr %1, i16 10
  %42 = load i16, ptr %41
  %43 = inttoptr i16 %40 to ptr addrspace(2)
  %44 = addrspacecast ptr addrspace(2) %43 to ptr addrspace(1)
  %45 = getelementptr i8, ptr addrspace(1) %44, i16 %42
  %46 = shl nuw nsw i16 %38, 1
  %47 = getelementptr i8, ptr addrspace(1) %45, i16 %46
  %48 = lshr i16 %38, 2
  %49 = and i16 %48, 60
  store i16 %49, ptr addrspace(1) %47
  %50 = getelementptr i8, ptr %3, i16 2
  %51 = load i16, ptr %50
  %52 = getelementptr i8, ptr %3, i16 10
  %53 = load i16, ptr %52
  %54 = inttoptr i16 %51 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 %53
  %57 = shl nuw nsw i16 %38, 1
  %58 = getelementptr i8, ptr addrspace(1) %56, i16 %57
  store i16 %49, ptr addrspace(1) %58
  %59 = getelementptr i8, ptr %5, i16 2
  %60 = load i16, ptr %59
  %61 = getelementptr i8, ptr %5, i16 10
  %62 = load i16, ptr %61
  %63 = inttoptr i16 %60 to ptr addrspace(2)
  %64 = addrspacecast ptr addrspace(2) %63 to ptr addrspace(1)
  %65 = getelementptr i8, ptr addrspace(1) %64, i16 %62
  %66 = shl nuw nsw i16 %38, 1
  %67 = getelementptr i8, ptr addrspace(1) %65, i16 %66
  store i16 %49, ptr addrspace(1) %67
  %68 = add nuw nsw i16 %38, 1
  %exitcond = icmp eq i16 %68, 256
  br i1 %exitcond, label %b16, label %b15

b16:
  ret void
}

define cc 1000 void @SETPAL(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1, ptr dereferenceable(2) nocapture readonly %2, ptr dereferenceable(2) nocapture readonly %3, ptr dereferenceable(2) nocapture readonly %4, ptr dereferenceable(2) nocapture readonly %5, ptr dereferenceable(2) nocapture readonly %6, ptr dereferenceable(2) nocapture readonly %7) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: read, inaccessiblemem: readwrite) nounwind nofree norecurse willreturn {
b1:
  %8 = load i16, ptr %1
  %9 = load i16, ptr %0
  %10 = load i16, ptr %2
  %11 = load i16, ptr %3
  %12 = load i16, ptr %4
  %13 = trunc i16 %9 to i8
  tail call void @llrm.ia16.out.i8(i16 968, i8 %13) memory(inaccessiblemem: readwrite)
  %14 = icmp slt i16 %8, %9
  br i1 %14, label %b6, label %15

15:
  %16 = sitofp i16 %12 to float
  %17 = sitofp i16 %11 to float
  %18 = sitofp i16 %10 to float
  %19 = sub i16 %8, %9
  %20 = load i16, ptr %5
  %21 = sub i16 %20, %10
  %22 = sitofp i16 %21 to float
  %23 = sitofp i16 %19 to float
  %24 = fdiv float %22, %23
  %25 = load i16, ptr %6
  %26 = sub i16 %25, %11
  %27 = sitofp i16 %26 to float
  %28 = fdiv float %27, %23
  %29 = load i16, ptr %7
  %30 = sub i16 %29, %12
  %31 = sitofp i16 %30 to float
  %32 = fdiv float %31, %23
  %33 = add i16 %8, 1
  %34 = sub i16 %33, %9
  br label %b5

b5:
  %35 = phi i16 [ %51, %b5 ], [ %34, %15 ]
  %36 = phi float [ %18, %15 ], [ %48, %b5 ]
  %37 = phi float [ %16, %15 ], [ %50, %b5 ]
  %38 = phi float [ %17, %15 ], [ %49, %b5 ]
  %39 = fpround float %38 to float
  %40 = fpround float %37 to float
  %41 = fpround float %36 to float
  %42 = tail call i16 @llvm.lrint.i16.f32(float %41)
  %43 = trunc i16 %42 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %43) memory(inaccessiblemem: readwrite)
  %44 = tail call i16 @llvm.lrint.i16.f32(float %39)
  %45 = trunc i16 %44 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %45) memory(inaccessiblemem: readwrite)
  %46 = tail call i16 @llvm.lrint.i16.f32(float %40)
  %47 = trunc i16 %46 to i8
  tail call void @llrm.ia16.out.i8(i16 969, i8 %47) memory(inaccessiblemem: readwrite)
  %48 = fadd float %24, %41
  %49 = fadd float %28, %39
  %50 = fadd float %32, %40
  %51 = add i16 %35, -1
  %52 = icmp eq i16 %51, 0
  br i1 %52, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @SPHEREMAPLASMA() addrspace(1) noinline "llrm-fp-eval"="x87-extended" norecurse {
  %1 = alloca [4 x i8]
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
  call void @llvm.memset.p0.i16(ptr nonnull %1, i8 0, i16 4, i1 false)
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %73
  store i16 31, ptr %72, !tbaa !2
  store i16 0, ptr %71, !tbaa !2
  store i16 0, ptr %70, !tbaa !2
  store i16 0, ptr %69, !tbaa !2
  store i16 63, ptr %68, !tbaa !2
  store i16 0, ptr %67, !tbaa !2
  store i16 0, ptr %66, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %73, ptr nonnull %72, ptr nonnull %71, ptr nonnull %70, ptr nonnull %69, ptr nonnull %68, ptr nonnull %67, ptr nonnull %66)
  store i16 32, ptr %65, !tbaa !2
  store i16 63, ptr %64, !tbaa !2
  store i16 63, ptr %63, !tbaa !2
  store i16 0, ptr %62, !tbaa !2
  store i16 0, ptr %61, !tbaa !2
  store i16 63, ptr %60, !tbaa !2
  store i16 0, ptr %59, !tbaa !2
  store i16 63, ptr %58, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %65, ptr nonnull %64, ptr nonnull %63, ptr nonnull %62, ptr nonnull %61, ptr nonnull %60, ptr nonnull %59, ptr nonnull %58)
  store i16 64, ptr %57, !tbaa !2
  store i16 95, ptr %56, !tbaa !2
  store i16 63, ptr %55, !tbaa !2
  store i16 0, ptr %54, !tbaa !2
  store i16 63, ptr %53, !tbaa !2
  store i16 0, ptr %52, !tbaa !2
  store i16 63, ptr %51, !tbaa !2
  store i16 63, ptr %50, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %57, ptr nonnull %56, ptr nonnull %55, ptr nonnull %54, ptr nonnull %53, ptr nonnull %52, ptr nonnull %51, ptr nonnull %50)
  store i16 96, ptr %49, !tbaa !2
  store i16 127, ptr %48, !tbaa !2
  store i16 0, ptr %47, !tbaa !2
  store i16 63, ptr %46, !tbaa !2
  store i16 63, ptr %45, !tbaa !2
  store i16 0, ptr %44, !tbaa !2
  store i16 0, ptr %43, !tbaa !2
  store i16 0, ptr %42, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %49, ptr nonnull %48, ptr nonnull %47, ptr nonnull %46, ptr nonnull %45, ptr nonnull %44, ptr nonnull %43, ptr nonnull %42)
  store i16 128, ptr %41, !tbaa !2
  store i16 159, ptr %40, !tbaa !2
  store i16 0, ptr %39, !tbaa !2
  store i16 0, ptr %38, !tbaa !2
  store i16 31, ptr %37, !tbaa !2
  store i16 31, ptr %36, !tbaa !2
  store i16 0, ptr %35, !tbaa !2
  store i16 31, ptr %34, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %41, ptr nonnull %40, ptr nonnull %39, ptr nonnull %38, ptr nonnull %37, ptr nonnull %36, ptr nonnull %35, ptr nonnull %34)
  store i16 160, ptr %33, !tbaa !2
  store i16 191, ptr %32, !tbaa !2
  store i16 31, ptr %31, !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  store i16 31, ptr %29, !tbaa !2
  store i16 31, ptr %28, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  store i16 63, ptr %26, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %33, ptr nonnull %32, ptr nonnull %31, ptr nonnull %30, ptr nonnull %29, ptr nonnull %28, ptr nonnull %27, ptr nonnull %26)
  store i16 192, ptr %25, !tbaa !2
  store i16 223, ptr %24, !tbaa !2
  store i16 31, ptr %23, !tbaa !2
  store i16 0, ptr %22, !tbaa !2
  store i16 63, ptr %21, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 47, ptr %19, !tbaa !2
  store i16 63, ptr %18, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %25, ptr nonnull %24, ptr nonnull %23, ptr nonnull %22, ptr nonnull %21, ptr nonnull %20, ptr nonnull %19, ptr nonnull %18)
  store i16 224, ptr %17, !tbaa !2
  store i16 254, ptr %16, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i16 47, ptr %14, !tbaa !2
  store i16 63, ptr %13, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  store i16 31, ptr %10, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %17, ptr nonnull %16, ptr nonnull %15, ptr nonnull %14, ptr nonnull %13, ptr nonnull %12, ptr nonnull %11, ptr nonnull %10)
  tail call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %74 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %75 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %76 = inttoptr i16 %74 to ptr addrspace(2)
  %77 = addrspacecast ptr addrspace(2) %76 to ptr addrspace(1)
  %78 = getelementptr i8, ptr addrspace(1) %77, i16 %75
  %79 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %80 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %81 = inttoptr i16 %79 to ptr addrspace(2)
  %82 = addrspacecast ptr addrspace(2) %81 to ptr addrspace(1)
  %83 = getelementptr i8, ptr addrspace(1) %82, i16 %80
  %84 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %85 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %86 = inttoptr i16 %84 to ptr addrspace(2)
  %87 = addrspacecast ptr addrspace(2) %86 to ptr addrspace(1)
  %88 = getelementptr i8, ptr addrspace(1) %87, i16 %85
  %89 = getelementptr i8, ptr addrspace(1) %77, i16 %75
  br label %90

90:
  %91 = phi i16 [ %104, %90 ], [ -512, %0 ]
  %92 = getelementptr i8, ptr addrspace(1) %89, i16 %91
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 512
  %94 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %95 = zext i8 %94 to i16
  store i16 %95, ptr addrspace(1) %93, !tbaa !60
  %96 = getelementptr i8, ptr addrspace(1) %83, i16 %91
  %97 = getelementptr i8, ptr addrspace(1) %96, i16 512
  %98 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %99 = zext i8 %98 to i16
  store i16 %99, ptr addrspace(1) %97, !tbaa !62
  %100 = getelementptr i8, ptr addrspace(1) %88, i16 %91
  %101 = getelementptr i8, ptr addrspace(1) %100, i16 512
  %102 = tail call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %103 = zext i8 %102 to i16
  store i16 %103, ptr addrspace(1) %101, !tbaa !64
  %104 = add nsw i16 %91, 2
  %exitcond = icmp eq i16 %104, 0
  br i1 %exitcond, label %105, label %90

105:
  %106 = addrspacecast ptr addrspace(2) %86 to ptr addrspace(1)
  %107 = addrspacecast ptr addrspace(2) %81 to ptr addrspace(1)
  %108 = addrspacecast ptr addrspace(2) %76 to ptr addrspace(1)
  %109 = getelementptr i8, ptr addrspace(1) %108, i16 %75
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 510
  store i16 0, ptr addrspace(1) %110, !tbaa !60
  %111 = getelementptr i8, ptr addrspace(1) %107, i16 %80
  %112 = getelementptr i8, ptr addrspace(1) %111, i16 510
  store i16 0, ptr addrspace(1) %112, !tbaa !62
  %113 = getelementptr i8, ptr addrspace(1) %106, i16 %85
  %114 = getelementptr i8, ptr addrspace(1) %113, i16 510
  store i16 31, ptr addrspace(1) %114, !tbaa !64
  store i16 0, ptr %9, !tbaa !2
  store i16 255, ptr %8, !tbaa !2
  store i16 63, ptr %7, !tbaa !2
  store i16 63, ptr %6, !tbaa !2
  store i16 63, ptr %5, !tbaa !2
  store i16 63, ptr %4, !tbaa !2
  store i16 63, ptr %3, !tbaa !2
  store i16 63, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %9, ptr nonnull %8, ptr nonnull %7, ptr nonnull %6, ptr nonnull %5, ptr nonnull %4, ptr nonnull %3, ptr nonnull %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string239, ptr nonnull %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr nonnull %1, i16 1, i16 -1, i16 32)
  %115 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %141

b2:
  %116 = phi i16 [ %234, %b64 ], [ %253, %b88 ]
  %117 = icmp eq i16 %251, 0
  br i1 %117, label %b3, label %b92

b3:
  %118 = phi i16 [ %233, %b2 ], [ %307, %b101 ]
  %119 = fadd float %239, %240
  %120 = fpround float %119 to float
  %121 = fcmp ogt float %120, 2.540000e+02
  %122 = sext i1 %121 to i16
  %123 = fcmp olt float %120, 6.500000e+01
  %124 = sext i1 %123 to i16
  %125 = or i16 %122, %124
  %126 = icmp eq i16 %125, 0
  %127 = fneg float %239
  %128 = fpround float %127 to float
  %129 = select i1 %126, float %239, float %128
  %130 = fadd float %237, %247
  %131 = fpround float %130 to float
  %132 = fadd float %238, %131
  %133 = fpround float %132 to float
  %134 = uitofp nneg i16 %249 to float
  %135 = fcmp ogt float %133, %134
  br i1 %135, label %b106, label %b108

b5:
  %136 = phi i16 [ %392, %b124 ], [ %458, %b134 ]
  %137 = phi i16 [ %390, %b124 ], [ %454, %b134 ]
  %138 = add nuw nsw i16 %391, 1
  %exitcond3 = icmp eq i16 %138, 200
  %139 = getelementptr i8, ptr %389, i16 -2
  %140 = getelementptr i8, ptr %388, i16 2
  br i1 %exitcond3, label %b116, label %b115

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %1)
  ret void

141:
  %142 = phi ptr [ %153, %b14 ], [ @"SP%", %105 ]
  %143 = phi i32 [ %152, %b14 ], [ 2, %105 ]
  %144 = add i32 %143, -1
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 %144, ptr addrspace(1) %115, i16 0)
  %145 = getelementptr i8, ptr %1, i16 2
  %data1 = load ptr, ptr %145
  %byte1 = load i8, ptr %data1
  %146 = zext i8 %byte1 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 1, i32 %143, ptr addrspace(1) %115, i16 0)
  %data = load ptr, ptr %145
  %byte = load i8, ptr %data
  %147 = zext i8 %byte to i16
  %148 = shl nuw nsw i16 %147, 7
  %149 = add nuw i16 %148, %146
  store i16 %149, ptr %142, !tbaa !2
  %tmp1 = trunc i32 %143 to i16
  %150 = icmp eq i16 %tmp1, 16514
  br i1 %150, label %b12, label %b14

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 0)
  br label %b18

b12:
  %151 = add nuw i16 %149, 1
  store i16 %151, ptr %142, !tbaa !2
  br label %b14

b14:
  %152 = add nuw nsw i32 %143, 2
  %tmp = trunc i32 %152 to i16
  %153 = getelementptr i8, ptr %142, i16 2
  %exitcond9 = icmp eq i16 %tmp, -32766
  br i1 %exitcond9, label %b11, label %141

b18:
  %154 = phi i16 [ %167, %b18 ], [ -278, %b11 ]
  %155 = phi i16 [ %166, %b18 ], [ -360, %b11 ]
  %156 = freeze i16 %154
  %157 = srem i16 %156, 99
  %158 = sub i16 %157, %156
  %159 = add i16 %154, %158
  %160 = add i16 %159, 98
  %161 = getelementptr i8, ptr @"DT%", i16 %155
  %162 = getelementptr i8, ptr %161, i16 384
  store i16 %160, ptr %162, !tbaa !2
  %163 = mul i16 %160, 100
  %164 = getelementptr i8, ptr @"DT100%", i16 %155
  %165 = getelementptr i8, ptr %164, i16 360
  store i16 %163, ptr %165, !tbaa !2
  %166 = add nsw i16 %155, 2
  %167 = add nuw nsw i16 %154, 1
  %exitcond8 = icmp eq i16 %166, 0
  br i1 %exitcond8, label %168, label %b18

168:
  br label %b31

b31:
  %169 = phi i16 [ -362, %168 ], [ %182, %b31 ]
  %170 = phi i16 [ %181, %b31 ], [ 0, %168 ]
  %171 = add i16 %170, -1
  %172 = freeze i16 %171
  %173 = srem i16 %172, 99
  %174 = sub i16 %173, %172
  %175 = add i16 %170, %174
  %176 = getelementptr i8, ptr @"DT%", i16 %169
  %177 = getelementptr i8, ptr %176, i16 746
  store i16 %175, ptr %177, !tbaa !2
  %178 = mul i16 %175, 100
  %179 = getelementptr i8, ptr @"DT100%", i16 %169
  %180 = getelementptr i8, ptr %179, i16 722
  store i16 %178, ptr %180, !tbaa !2
  %181 = add nuw nsw i16 %170, 1
  %182 = add nsw i16 %169, 2
  %exitcond7 = icmp eq i16 %182, 0
  br i1 %exitcond7, label %183, label %b31

183:
  br label %b44

b44:
  %184 = phi i16 [ -2318, %183 ], [ %196, %b47 ]
  %185 = phi i16 [ %195, %b47 ], [ -640, %183 ]
  %186 = sitofp i16 %185 to float
  %187 = fmul float %186, 6.250000e-02
  %188 = call float @llvm.fabs.f32(float %187)
  %189 = fcmp ogt float %188, 0x41E0000000000000
  br i1 %189, label %b46, label %b47

b46:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b47:
  %190 = call float @llvm.sin.f32(float %187)
  %191 = fmul float %190, 3.100000e+01
  %192 = call i16 @llvm.lrint.i16.f32(float %191)
  %193 = getelementptr i8, ptr @"FSIN2%", i16 %184
  %194 = getelementptr i8, ptr %193, i16 2318
  store i16 %192, ptr %194, !tbaa !2
  %195 = add nsw i16 %185, 1
  %196 = add nsw i16 %184, 2
  %exitcond6 = icmp eq i16 %196, 0
  br i1 %exitcond6, label %197, label %b44

197:
  br label %b51

b51:
  %198 = phi i16 [ -1356, %197 ], [ %211, %b54 ]
  %199 = phi i16 [ -2712, %197 ], [ %210, %b54 ]
  %200 = add i16 %198, 716
  %201 = sitofp i16 %200 to float
  %202 = fdiv float %201, 6.300000e+01
  %203 = call float @llvm.fabs.f32(float %202)
  %204 = fcmp ogt float %203, 0x41E0000000000000
  br i1 %204, label %b53, label %b54

b53:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b54:
  %205 = call float @llvm.sin.f32(float %202)
  %206 = fmul float %205, 7.500000e+01
  %207 = call i16 @llvm.lrint.i16.f32(float %206)
  %208 = getelementptr i8, ptr @"FSIN3%", i16 %199
  %209 = getelementptr i8, ptr %208, i16 2712
  store i16 %207, ptr %209, !tbaa !2
  %210 = add nsw i16 %199, 2
  %211 = add nsw i16 %198, 1
  %exitcond5 = icmp eq i16 %210, 0
  br i1 %exitcond5, label %212, label %b51

212:
  br label %b58

b58:
  %213 = phi i16 [ -337, %212 ], [ %221, %b58 ]
  %214 = phi i16 [ -674, %212 ], [ %220, %b58 ]
  %215 = add i16 %213, 169
  %216 = lshr i16 %215, 1
  %217 = and i16 %216, 127
  %218 = getelementptr i8, ptr @"MOD256128%", i16 %214
  %219 = getelementptr i8, ptr %218, i16 674
  store i16 %217, ptr %219, !tbaa !2
  %220 = add nsw i16 %214, 2
  %221 = add nsw i16 %213, 1
  %exitcond4 = icmp eq i16 %220, 0
  br i1 %exitcond4, label %222, label %b58

222:
  br label %b60

b60:
  %223 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %224 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %223, ptr @$string240)
  %225 = icmp ne i16 %224, 0
  br i1 %225, label %b60, label %226

226:
  %227 = getelementptr i8, ptr @"FSIN2%", i16 1280
  br label %b64

b64:
  %228 = phi i16 [ %136, %b140 ], [ %136, %401 ], [ 0, %226 ]
  %229 = phi i16 [ %251, %b140 ], [ %251, %401 ], [ 0, %226 ]
  %230 = phi i16 [ %spec.store.select1, %b140 ], [ %spec.store.select1, %401 ], [ 0, %226 ]
  %231 = phi i16 [ %spec.store.select, %b140 ], [ %spec.store.select, %401 ], [ 0, %226 ]
  %232 = phi i16 [ %241, %b140 ], [ %241, %401 ], [ 0, %226 ]
  %233 = phi i16 [ %118, %b140 ], [ %118, %401 ], [ 0, %226 ]
  %234 = phi i16 [ %116, %b140 ], [ %116, %401 ], [ 0, %226 ]
  %235 = phi i16 [ %249, %b140 ], [ %249, %401 ], [ 134, %226 ]
  %236 = phi float [ %247, %b140 ], [ %247, %401 ], [ 0.000000e+00, %226 ]
  %237 = phi float [ %368, %b140 ], [ %368, %401 ], [ 0.000000e+00, %226 ]
  %238 = phi float [ %369, %b140 ], [ %369, %401 ], [ -9.600000e+01, %226 ]
  %239 = phi float [ %129, %b140 ], [ %129, %401 ], [ 1.000000e+00, %226 ]
  %240 = phi float [ %120, %b140 ], [ %120, %401 ], [ 9.100000e+01, %226 ]
  %241 = add i16 %232, 1
  %242 = add i16 %231, 1
  %243 = add i16 %230, 1
  %244 = icmp eq i16 %242, 101
  %spec.store.select = select i1 %244, i16 0, i16 %242
  %245 = icmp eq i16 %243, 396
  %spec.store.select1 = select i1 %245, i16 0, i16 %243
  %246 = icmp eq i16 %241, 26
  %247 = select i1 %246, float 0x3F947AE140000000, float %236
  %248 = icmp eq i16 %241, 102
  %249 = select i1 %248, i16 654, i16 %235
  %250 = icmp eq i16 %241, 128
  %251 = select i1 %250, i16 1, i16 %229
  %252 = icmp slt i16 %234, 26
  br i1 %252, label %b81, label %b2

b81:
  %253 = add nsw i16 %234, 1
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %254 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %255 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %256 = inttoptr i16 %254 to ptr addrspace(2)
  %257 = addrspacecast ptr addrspace(2) %256 to ptr addrspace(1)
  %258 = sitofp i16 %253 to float
  %259 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %260 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %261 = inttoptr i16 %259 to ptr addrspace(2)
  %262 = addrspacecast ptr addrspace(2) %261 to ptr addrspace(1)
  %263 = getelementptr i8, ptr addrspace(1) %262, i16 %260
  %264 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %265 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %266 = inttoptr i16 %264 to ptr addrspace(2)
  %267 = addrspacecast ptr addrspace(2) %266 to ptr addrspace(1)
  %268 = getelementptr i8, ptr addrspace(1) %267, i16 %265
  %269 = getelementptr i8, ptr addrspace(1) %257, i16 %255
  br label %b88

b88:
  %270 = phi i16 [ -10, %b81 ], [ %304, %b88 ]
  %271 = shl nsw i16 %270, 1
  %272 = getelementptr i8, ptr addrspace(1) %269, i16 %271
  %273 = getelementptr i8, ptr addrspace(1) %272, i16 20
  %274 = load i16, ptr addrspace(1) %273, !tbaa !60
  %275 = add i16 %274, -63
  %276 = sitofp i16 %275 to float
  %277 = fdiv float %276, 2.600000e+01
  %278 = fmul float %277, %258
  %279 = fadd float %278, 6.300000e+01
  %280 = call i16 @llvm.lrint.i16.f32(float %279)
  %281 = trunc i16 %280 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %281) memory(inaccessiblemem: readwrite)
  %282 = shl nsw i16 %270, 1
  %283 = getelementptr i8, ptr addrspace(1) %263, i16 %282
  %284 = getelementptr i8, ptr addrspace(1) %283, i16 20
  %285 = load i16, ptr addrspace(1) %284, !tbaa !62
  %286 = add i16 %285, -63
  %287 = sitofp i16 %286 to float
  %288 = fdiv float %287, 2.600000e+01
  %289 = fmul float %288, %258
  %290 = fadd float %289, 6.300000e+01
  %291 = call i16 @llvm.lrint.i16.f32(float %290)
  %292 = trunc i16 %291 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %292) memory(inaccessiblemem: readwrite)
  %293 = shl nsw i16 %270, 1
  %294 = getelementptr i8, ptr addrspace(1) %268, i16 %293
  %295 = getelementptr i8, ptr addrspace(1) %294, i16 20
  %296 = load i16, ptr addrspace(1) %295, !tbaa !64
  %297 = add i16 %296, -63
  %298 = sitofp i16 %297 to float
  %299 = fdiv float %298, 2.600000e+01
  %300 = fmul float %299, %258
  %301 = fadd float %300, 6.300000e+01
  %302 = call i16 @llvm.lrint.i16.f32(float %301)
  %303 = trunc i16 %302 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %303) memory(inaccessiblemem: readwrite)
  %304 = add nsw i16 %270, 10
  %305 = icmp samesign ult i16 %304, 246
  br i1 %305, label %b88, label %b2

b92:
  %306 = icmp slt i16 %233, 26
  br i1 %306, label %b94, label %b6

b94:
  %307 = add nsw i16 %233, 1
  call void @llrm.ia16.out.i8(i16 968, i8 0) memory(inaccessiblemem: readwrite)
  %308 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 2), !tbaa !2
  %309 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"R%", i16 10), !tbaa !2
  %310 = inttoptr i16 %308 to ptr addrspace(2)
  %311 = addrspacecast ptr addrspace(2) %310 to ptr addrspace(1)
  %312 = sitofp i16 %307 to float
  %313 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %314 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %315 = inttoptr i16 %313 to ptr addrspace(2)
  %316 = addrspacecast ptr addrspace(2) %315 to ptr addrspace(1)
  %317 = getelementptr i8, ptr addrspace(1) %316, i16 %314
  %318 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 2), !tbaa !2
  %319 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"B%", i16 10), !tbaa !2
  %320 = inttoptr i16 %318 to ptr addrspace(2)
  %321 = addrspacecast ptr addrspace(2) %320 to ptr addrspace(1)
  %322 = getelementptr i8, ptr addrspace(1) %321, i16 %319
  %323 = getelementptr i8, ptr addrspace(1) %311, i16 %309
  br label %b101

b101:
  %324 = phi i16 [ -10, %b94 ], [ %361, %b101 ]
  %325 = shl nsw i16 %324, 1
  %326 = getelementptr i8, ptr addrspace(1) %323, i16 %325
  %327 = getelementptr i8, ptr addrspace(1) %326, i16 20
  %328 = load i16, ptr addrspace(1) %327, !tbaa !60
  %329 = sub i16 0, %328
  %330 = sitofp i16 %329 to float
  %331 = fdiv float %330, 2.600000e+01
  %332 = fmul float %331, %312
  %333 = sitofp i16 %328 to float
  %334 = fadd float %332, %333
  %335 = call i16 @llvm.lrint.i16.f32(float %334)
  %336 = trunc i16 %335 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %336) memory(inaccessiblemem: readwrite)
  %337 = shl nsw i16 %324, 1
  %338 = getelementptr i8, ptr addrspace(1) %317, i16 %337
  %339 = getelementptr i8, ptr addrspace(1) %338, i16 20
  %340 = load i16, ptr addrspace(1) %339, !tbaa !62
  %341 = sub i16 0, %340
  %342 = sitofp i16 %341 to float
  %343 = fdiv float %342, 2.600000e+01
  %344 = fmul float %343, %312
  %345 = sitofp i16 %340 to float
  %346 = fadd float %344, %345
  %347 = call i16 @llvm.lrint.i16.f32(float %346)
  %348 = trunc i16 %347 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %348) memory(inaccessiblemem: readwrite)
  %349 = shl nsw i16 %324, 1
  %350 = getelementptr i8, ptr addrspace(1) %322, i16 %349
  %351 = getelementptr i8, ptr addrspace(1) %350, i16 20
  %352 = load i16, ptr addrspace(1) %351, !tbaa !64
  %353 = sub i16 0, %352
  %354 = sitofp i16 %353 to float
  %355 = fdiv float %354, 2.600000e+01
  %356 = fmul float %355, %312
  %357 = sitofp i16 %352 to float
  %358 = fadd float %356, %357
  %359 = call i16 @llvm.lrint.i16.f32(float %358)
  %360 = trunc i16 %359 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %360) memory(inaccessiblemem: readwrite)
  %361 = add nsw i16 %324, 10
  %362 = icmp samesign ult i16 %361, 246
  br i1 %362, label %b101, label %b3

b106:
  %363 = fsub float %133, %131
  %364 = fpround float %363 to float
  %365 = fneg float %131
  %366 = fmul float %365, 0x3FE99999A0000000
  %367 = fpround float %366 to float
  br label %b108

b108:
  %368 = phi float [ %367, %b106 ], [ %131, %b3 ]
  %369 = phi float [ %364, %b106 ], [ %133, %b3 ]
  %370 = call i16 @llvm.lrint.i16.f32(float %120)
  %371 = call i16 @llvm.lrint.i16.f32(float %369)
  %372 = add i16 %371, -63
  %373 = ashr i16 %372, 15
  %374 = icmp sgt i16 %372, -128
  %375 = select i1 %374, i16 %373, i16 0
  %376 = icmp eq i16 %375, 0
  %377 = mul i16 %372, -128
  %378 = add i16 %377, -1
  %379 = select i1 %376, i16 -1, i16 %378
  %380 = add i16 %371, -64
  %381 = add i16 %371, 65
  %382 = add i16 %370, -64
  %383 = add i16 %370, 65
  %invariant.gep3 = getelementptr [2 x i8], ptr @"FSIN3%", i16 %spec.store.select1
  %384 = add i16 %370, -63
  %385 = mul i16 %spec.store.select, -2
  %386 = getelementptr i8, ptr %227, i16 %385
  %387 = shl i16 %spec.store.select1, 1
  br label %b115

b115:
  %388 = phi ptr [ %140, %b5 ], [ %227, %b108 ]
  %389 = phi ptr [ %139, %b5 ], [ %386, %b108 ]
  %390 = phi i16 [ %379, %b108 ], [ %137, %b5 ]
  %391 = phi i16 [ 0, %b108 ], [ %138, %b5 ]
  %392 = phi i16 [ %228, %b108 ], [ %136, %b5 ]
  %393 = mul nuw nsw i16 %391, 20
  %394 = add nuw nsw i16 %393, -24576
  store i16 %394, ptr @b$seg, !tbaa !2
  %395 = icmp sgt i16 %391, %380
  %396 = icmp slt i16 %391, %381
  %397 = and i1 %395, %396
  br i1 %397, label %398, label %b119

398:
  br label %b130

b116:
  %399 = load i16, ptr @"XIT%", !tbaa !2
  %400 = icmp eq i16 %399, 0
  br i1 %400, label %491, label %401

401:
  %402 = icmp eq i16 %399, 1
  br i1 %402, label %b6, label %b64

b119:
  %gep3 = getelementptr [2 x i8], ptr %invariant.gep3, i16 %391
  br label %b124

b124:
  %403 = phi i16 [ 0, %b119 ], [ %436, %b124 ]
  %404 = shl nuw nsw i16 %403, 1
  %405 = getelementptr i8, ptr @"FSIN2%", i16 %404
  %406 = getelementptr i8, ptr %405, i16 1280
  %407 = load i16, ptr %406, !tbaa !2
  %408 = getelementptr i8, ptr %gep3, i16 1280
  %409 = load i16, ptr %408, !tbaa !2
  %410 = shl nuw nsw i16 %403, 1
  %411 = getelementptr i8, ptr %388, i16 %410
  %412 = load i16, ptr %411, !tbaa !2
  %413 = shl nuw nsw i16 %403, 1
  %414 = getelementptr i8, ptr @"FSIN3%", i16 %387
  %415 = getelementptr i8, ptr %414, i16 %413
  %416 = getelementptr i8, ptr %415, i16 1280
  %417 = load i16, ptr %416, !tbaa !2
  %418 = shl nuw nsw i16 %403, 1
  %419 = getelementptr i8, ptr %389, i16 %418
  %420 = load i16, ptr %419, !tbaa !2
  %421 = sub i16 %417, %420
  %422 = getelementptr [2 x i8], ptr @"FSIN2%", i16 %421
  %423 = getelementptr i8, ptr %422, i16 1280
  %424 = load i16, ptr %423, !tbaa !2
  %425 = getelementptr [2 x i8], ptr @"MOD256128%", i16 %407
  %426 = getelementptr [2 x i8], ptr %425, i16 %409
  %427 = getelementptr [2 x i8], ptr %426, i16 %412
  %428 = getelementptr [2 x i8], ptr %427, i16 %424
  %429 = getelementptr i8, ptr %428, i16 336
  %430 = load i16, ptr %429, !tbaa !2
  %431 = trunc i16 %430 to i8
  %432 = load i16, ptr @b$seg, !tbaa !2
  %433 = inttoptr i16 %432 to ptr addrspace(2)
  %434 = addrspacecast ptr addrspace(2) %433 to ptr addrspace(1)
  %435 = getelementptr i8, ptr addrspace(1) %434, i16 %403
  store volatile i8 %431, ptr addrspace(1) %435
  %436 = add nuw nsw i16 %403, 1
  %exitcond2 = icmp eq i16 %436, 320
  br i1 %exitcond2, label %b5, label %b124

b130:
  %437 = phi i16 [ %454, %b134 ], [ %390, %398 ]
  %438 = phi i16 [ %490, %b134 ], [ 0, %398 ]
  %439 = phi i16 [ %458, %b134 ], [ %392, %398 ]
  %440 = icmp sgt i16 %438, %382
  %441 = icmp slt i16 %438, %383
  %442 = and i1 %440, %441
  br i1 %442, label %b132, label %b134

b132:
  %443 = add i16 %437, 1
  %444 = getelementptr inbounds [2 x i8], ptr @"SP%", i16 %443
  %445 = load i16, ptr %444, !tbaa !2
  %446 = sdiv i16 %445, 128
  %447 = mul i16 %446, -128
  %448 = add i16 %384, %445
  %449 = add i16 %448, %447
  %450 = add i16 %446, %372
  br label %b134

b134:
  %451 = phi i16 [ 128, %b132 ], [ %439, %b130 ]
  %452 = phi i16 [ %449, %b132 ], [ %438, %b130 ]
  %453 = phi i16 [ %450, %b132 ], [ %391, %b130 ]
  %454 = phi i16 [ %443, %b132 ], [ %437, %b130 ]
  %455 = icmp eq i16 %438, %452
  %456 = icmp eq i16 %453, %391
  %457 = and i1 %455, %456
  %458 = select i1 %457, i16 0, i16 %451
  %459 = getelementptr [2 x i8], ptr @"FSIN2%", i16 %452
  %460 = getelementptr i8, ptr %459, i16 1280
  %461 = load i16, ptr %460, !tbaa !2
  %gep1 = getelementptr [2 x i8], ptr %invariant.gep3, i16 %453
  %462 = getelementptr i8, ptr %gep1, i16 1280
  %463 = load i16, ptr %462, !tbaa !2
  %464 = getelementptr [2 x i8], ptr %459, i16 %453
  %465 = getelementptr i8, ptr %464, i16 1280
  %466 = load i16, ptr %465, !tbaa !2
  %gep2 = getelementptr [2 x i8], ptr %invariant.gep3, i16 %452
  %467 = getelementptr i8, ptr %gep2, i16 1280
  %468 = load i16, ptr %467, !tbaa !2
  %469 = add i16 %spec.store.select, %453
  %470 = sub i16 %452, %469
  %471 = getelementptr [2 x i8], ptr @"FSIN2%", i16 %470
  %472 = getelementptr i8, ptr %471, i16 1280
  %473 = load i16, ptr %472, !tbaa !2
  %474 = sub i16 %468, %473
  %475 = getelementptr [2 x i8], ptr @"FSIN2%", i16 %474
  %476 = getelementptr i8, ptr %475, i16 1280
  %477 = load i16, ptr %476, !tbaa !2
  %478 = getelementptr [2 x i8], ptr @"MOD256128%", i16 %461
  %479 = getelementptr [2 x i8], ptr %478, i16 %463
  %480 = getelementptr [2 x i8], ptr %479, i16 %466
  %481 = getelementptr [2 x i8], ptr %480, i16 %477
  %482 = getelementptr i8, ptr %481, i16 336
  %483 = load i16, ptr %482, !tbaa !2
  %484 = add i16 %483, %458
  %485 = trunc i16 %484 to i8
  %486 = load i16, ptr @b$seg, !tbaa !2
  %487 = inttoptr i16 %486 to ptr addrspace(2)
  %488 = addrspacecast ptr addrspace(2) %487 to ptr addrspace(1)
  %489 = getelementptr i8, ptr addrspace(1) %488, i16 %438
  store volatile i8 %485, ptr addrspace(1) %489
  %490 = add nuw nsw i16 %438, 1
  %exitcond1 = icmp eq i16 %490, 320
  br i1 %exitcond1, label %b5, label %b130

491:
  %492 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %493 = icmp eq i8 %492, 1
  br i1 %493, label %b140.thread, label %b140

b140.thread:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b6

b140:
  %494 = icmp eq i16 %399, 1
  br i1 %494, label %b6, label %b64
}

define cc 1000 void @SUCKING() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 15, i16 2)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 1, i16 1, i16 1, i16 4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string243)
  br label %b5

b5:
  %0 = phi i16 [ %22, %b11 ], [ 118, %b1 ]
  %1 = phi i16 [ 7, %b1 ], [ %20, %b11 ]
  %2 = add nuw nsw i16 %1, 1
  %3 = shl nuw nsw i16 %1, 4
  %4 = add nuw nsw i16 %3, 32
  %5 = sitofp i16 %4 to float
  %6 = fpround float %5 to float
  %7 = shl nuw nsw i16 %2, 4
  %8 = add nuw nsw i16 %7, 32
  %9 = sitofp i16 %8 to float
  %10 = fpround float %9 to float
  br label %b10

b6:
  ret void

b10:
  %11 = phi i16 [ %35, %b13 ], [ 0, %b5 ]
  %12 = phi i16 [ %34, %b13 ], [ 112, %b5 ]
  %13 = add i16 %11, 111
  %14 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$PNI2(i16 %13, i16 %1)
  %15 = add i16 %0, %11
  %16 = sitofp i16 %15 to float
  %17 = fdiv float %16, 4.500000e+01
  %18 = tail call float @llvm.fabs.f32(float %17)
  %19 = fcmp ogt float %18, 0x41E0000000000000
  br i1 %19, label %b12, label %b13

b11:
  %20 = add nsw i16 %1, -1
  %21 = icmp eq i16 %1, 0
  %22 = add nsw i16 %0, -1
  br i1 %21, label %b6, label %b5

b12:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b13:
  %23 = mul i16 %14, %2
  %24 = tail call float @llvm.sin.f32(float %17)
  %25 = sitofp i16 %23 to float
  %26 = fmul float %24, %25
  %27 = tail call i16 @llvm.lrint.i16.f32(float %26)
  %28 = uitofp nneg i16 %13 to float
  %29 = fmul float %28, 0x4006666660000000
  %30 = fpround float %29 to float
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N1R4(float %30, float %6)
  %31 = sitofp i16 %12 to float
  %32 = fmul float %31, 0x4006666660000000
  %33 = fpround float %32 to float
  tail call cc 1000 addrspace(1) void @llrm.qb.B$N2R4(float %33, float %10)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 %27, i16 -1, i16 2)
  %34 = add nsw i16 %12, -1
  %35 = add nsw i16 %11, -1
  %36 = icmp eq i16 %34, 0
  br i1 %36, label %b11, label %b10
}

define cc 1000 void @TRANSLATE3D() addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %0 = load i16, ptr @"NDTS%", !tbaa !2
  %1 = add i16 %0, -1
  %2 = icmp slt i16 %1, 0
  br i1 %2, label %b19, label %3

3:
  %4 = load i16, ptr @"OBJ%", !tbaa !2
  %5 = icmp sgt i16 %4, 4
  %6 = icmp eq i16 %4, 4
  %7 = icmp eq i16 %4, 2
  %8 = sext i1 %7 to i16
  %9 = icmp eq i16 %4, 3
  %10 = sext i1 %9 to i16
  %11 = or i16 %8, %10
  %12 = icmp eq i16 %11, 0
  %13 = icmp eq i16 %4, 1
  %14 = zext i1 %13 to i16
  %15 = select i1 %12, i16 %14, i16 4
  %16 = select i1 %6, i16 16, i16 %15
  %17 = select i1 %5, i16 8, i16 %16
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"XS%", i16 2), !tbaa !2
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"YS%", i16 2), !tbaa !2
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Z!", i16 2), !tbaa !2
  %21 = uitofp nneg i16 %17 to float
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"X!", i16 2), !tbaa !2
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"Y!", i16 2), !tbaa !2
  br label %b18

b2:
  %24 = add i16 %27, 2
  %25 = add i16 %26, -1
  %exitcond = icmp eq i16 %25, 0
  br i1 %exitcond, label %b19, label %b18

b18:
  %26 = phi i16 [ %25, %b2 ], [ %0, %3 ]
  %27 = phi i16 [ %24, %b2 ], [ 0, %3 ]
  %28 = inttoptr i16 %20 to ptr addrspace(2)
  %29 = inttoptr i16 %19 to ptr addrspace(2)
  %30 = inttoptr i16 %18 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %32 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %33 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %33, i16 %27
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 8194
  %36 = getelementptr i8, ptr addrspace(1) %33, i16 %27
  %37 = load i16, ptr addrspace(1) %36, !tbaa !66
  store i16 %37, ptr addrspace(1) %35, !tbaa !66
  %38 = getelementptr i8, ptr addrspace(1) %32, i16 %27
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 8194
  %40 = getelementptr i8, ptr addrspace(1) %32, i16 %27
  %41 = load i16, ptr addrspace(1) %40, !tbaa !68
  store i16 %41, ptr addrspace(1) %39, !tbaa !68
  %42 = shl i16 %27, 1
  %43 = getelementptr i8, ptr addrspace(1) %31, i16 %42
  %44 = load float, ptr addrspace(1) %43, !tbaa !56
  %45 = fcmp ole float %44, %21
  br i1 %45, label %b2, label %b22

b19:
  ret void

b22:
  %46 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %47 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %48 = inttoptr i16 %23 to ptr addrspace(2)
  %49 = inttoptr i16 %22 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %51 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %52 = shl i16 %27, 1
  %53 = getelementptr i8, ptr addrspace(1) %51, i16 %52
  %54 = load float, ptr addrspace(1) %53, !tbaa !54
  %55 = fmul float %54, 2.560000e+02
  %56 = fdiv float %55, %44
  %57 = tail call i16 @llvm.lrint.i16.f32(float %56)
  %58 = getelementptr i8, ptr addrspace(1) %46, i16 %27
  store i16 %57, ptr addrspace(1) %58, !tbaa !66
  %59 = shl i16 %27, 1
  %60 = getelementptr i8, ptr addrspace(1) %50, i16 %59
  %61 = load float, ptr addrspace(1) %60, !tbaa !58
  %62 = fmul float %61, 2.560000e+02
  %63 = fdiv float %62, %44
  %64 = tail call i16 @llvm.lrint.i16.f32(float %63)
  %65 = getelementptr i8, ptr addrspace(1) %47, i16 %27
  store i16 %64, ptr addrspace(1) %65, !tbaa !68
  br label %b2
}

define cc 1000 void @ZOOMDISTORT() addrspace(1) noinline "llrm-fp-eval"="x87-extended" norecurse {
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
  %32 = alloca [4 x i8]
  %33 = alloca [4 x i8]
  %34 = alloca [22 x i8]
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
  call void @llvm.memset.p0.i16(ptr nonnull %32, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %33, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr nonnull %34, i8 0, i16 22, i1 false)
  br label %b3

35:
  store i16 1, ptr @"XIT%", !tbaa !2
  br label %b2

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %33)
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr nonnull %32)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %34)
  ret void

b3:
  %36 = tail call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %37 = tail call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %36, ptr @$string245)
  %38 = icmp ne i16 %37, 0
  br i1 %38, label %b3, label %b5

b5:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  br label %b9

b9:
  %39 = phi i16 [ %51, %b12 ], [ -2318, %b5 ]
  %40 = phi i16 [ -640, %b5 ], [ %50, %b12 ]
  %41 = sitofp i16 %40 to float
  %42 = fmul float %41, 6.250000e-02
  %43 = tail call float @llvm.fabs.f32(float %42)
  %44 = fcmp ogt float %43, 0x41E0000000000000
  br i1 %44, label %b11, label %b12

b11:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %45 = tail call float @llvm.sin.f32(float %42)
  %46 = fmul float %45, 3.100000e+01
  %47 = tail call i16 @llvm.lrint.i16.f32(float %46)
  %48 = getelementptr i8, ptr @"FSIN2%", i16 %39
  %49 = getelementptr i8, ptr %48, i16 2318
  store i16 %47, ptr %49, !tbaa !2
  %50 = add nsw i16 %40, 1
  %51 = add nsw i16 %39, 2
  %exitcond10 = icmp eq i16 %51, 0
  br i1 %exitcond10, label %52, label %b9

52:
  br label %b16

b16:
  %53 = phi i16 [ -1356, %52 ], [ %66, %b19 ]
  %54 = phi i16 [ -2712, %52 ], [ %65, %b19 ]
  %55 = add i16 %53, 716
  %56 = sitofp i16 %55 to float
  %57 = fdiv float %56, 6.300000e+01
  %58 = tail call float @llvm.fabs.f32(float %57)
  %59 = fcmp ogt float %58, 0x41E0000000000000
  br i1 %59, label %b18, label %b19

b18:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b19:
  %60 = tail call float @llvm.sin.f32(float %57)
  %61 = fmul float %60, 7.500000e+01
  %62 = tail call i16 @llvm.lrint.i16.f32(float %61)
  %63 = getelementptr i8, ptr @"FSIN3%", i16 %54
  %64 = getelementptr i8, ptr %63, i16 2712
  store i16 %62, ptr %64, !tbaa !2
  %65 = add nsw i16 %54, 2
  %66 = add nsw i16 %53, 1
  %exitcond9 = icmp eq i16 %65, 0
  br i1 %exitcond9, label %67, label %b16

67:
  br label %b23

b23:
  %68 = phi i16 [ -588, %67 ], [ %81, %b26 ]
  %69 = phi i16 [ -1176, %67 ], [ %80, %b26 ]
  %70 = add i16 %68, 540
  %71 = sitofp i16 %70 to float
  %72 = fdiv float %71, 3.500000e+01
  %73 = tail call float @llvm.fabs.f32(float %72)
  %74 = fcmp ogt float %73, 0x41E0000000000000
  br i1 %74, label %b25, label %b26

b25:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b26:
  %75 = tail call float @llvm.sin.f32(float %72)
  %76 = fmul float %75, 2.400000e+01
  %77 = tail call i16 @llvm.lrint.i16.f32(float %76)
  %78 = getelementptr i8, ptr @"FSIN1%", i16 %69
  %79 = getelementptr i8, ptr %78, i16 1176
  store i16 %77, ptr %79, !tbaa !2
  %80 = add nsw i16 %69, 2
  %81 = add nsw i16 %68, 1
  %exitcond8 = icmp eq i16 %80, 0
  br i1 %exitcond8, label %82, label %b23

82:
  br label %b30

b30:
  %83 = phi i16 [ -922, %82 ], [ %100, %b33 ]
  %84 = phi i16 [ -1844, %82 ], [ %99, %b33 ]
  %85 = add i16 %83, 603
  %86 = sitofp i16 %85 to float
  %87 = fdiv float %86, 4.500000e+01
  %88 = tail call float @llvm.fabs.f32(float %87)
  %89 = fcmp ogt float %88, 0x41E0000000000000
  br i1 %89, label %b32, label %b33

b31:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -160, i16 160, i16 -48, i16 48, i16 2, i16 258, ptr nonnull %34)
  %90 = getelementptr inbounds nuw i8, ptr %34, i16 2
  %91 = load i16, ptr %90, !tbaa !2
  %92 = inttoptr i16 %91 to ptr addrspace(2)
  %93 = addrspacecast ptr addrspace(2) %92 to ptr addrspace(1)
  br label %b37

b32:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b33:
  %94 = tail call float @llvm.sin.f32(float %87)
  %95 = fmul float %94, 2.400000e+01
  %96 = tail call i16 @llvm.lrint.i16.f32(float %95)
  %97 = getelementptr i8, ptr @"FSIN4%", i16 %84
  %98 = getelementptr i8, ptr %97, i16 1844
  store i16 %96, ptr %98, !tbaa !2
  %99 = add nsw i16 %84, 2
  %100 = add nsw i16 %83, 1
  %exitcond7 = icmp eq i16 %99, 0
  br i1 %exitcond7, label %b31, label %b30

b37:
  %101 = phi ptr addrspace(1) [ %112, %b43 ], [ %93, %b31 ]
  %102 = phi i16 [ -48, %b31 ], [ %111, %b43 ]
  %103 = sitofp i16 %102 to float
  %104 = fmul float %103, 1.562500e-02
  %105 = fadd float %104, 2.000000e+00
  %106 = fpround float %105 to float
  br label %b42

b38:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string246, ptr nonnull %33)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string247, ptr nonnull %32)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr nonnull %33, i16 1, i16 -1, i16 32)
  call void @llrm.ia16.out.i8(i16 968, i8 -128) memory(inaccessiblemem: readwrite)
  %107 = addrspacecast ptr %32 to ptr addrspace(1)
  br label %118

b42:
  %IV.S. = phi float [ -1.600000e+02, %b37 ], [ %IV.S.next., %b46 ]
  %108 = phi i16 [ -160, %b37 ], [ %117, %b46 ]
  %109 = fcmp une float %106, 0.000000e+00
  %110 = shl nsw i16 %108, 1
  br i1 %109, label %b44, label %b46

b43:
  %111 = add nsw i16 %102, 1
  %exitcond6 = icmp eq i16 %111, 49
  %112 = getelementptr i8, ptr addrspace(1) %101, i16 642
  br i1 %exitcond6, label %b38, label %b37

b44:
  %113 = fdiv float %IV.S., %106
  %114 = call i16 @llvm.lrint.i16.f32(float %113)
  br label %b46

b46:
  %sink = phi i16 [ %114, %b44 ], [ %108, %b42 ]
  %115 = getelementptr i8, ptr addrspace(1) %101, i16 %110
  %116 = getelementptr i8, ptr addrspace(1) %115, i16 320
  store i16 %sink, ptr addrspace(1) %116, !tbaa !74
  %IV.S.next. = fadd float %IV.S., 1.000000e+00
  %117 = add nsw i16 %108, 1
  %exitcond5 = icmp eq i16 %117, 161
  br i1 %exitcond5, label %b43, label %b42

118:
  %119 = phi i16 [ %121, %118 ], [ 384, %b38 ]
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %107, i16 0)
  %120 = getelementptr i8, ptr %32, i16 2
  %data1 = load ptr, ptr %120
  %byte1 = load i8, ptr %data1
  call void @llrm.ia16.out.i8(i16 969, i8 %byte1) memory(inaccessiblemem: readwrite)
  %121 = add nsw i16 %119, -1
  %exitcond4 = icmp eq i16 %121, 0
  br i1 %exitcond4, label %122, label %118

122:
  br label %b55

b55:
  %123 = phi i16 [ %138, %b61 ], [ 0, %122 ]
  %124 = phi i16 [ %137, %b61 ], [ 1, %122 ]
  %125 = shl i16 %123, 1
  %126 = sub i16 -100, %123
  br label %128

b56:
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  store i16 0, ptr %31, !tbaa !2
  store i16 31, ptr %30, !tbaa !2
  store i16 0, ptr %29, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 31, ptr %25, !tbaa !2
  store i16 63, ptr %24, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %31, ptr nonnull %30, ptr nonnull %29, ptr nonnull %28, ptr nonnull %27, ptr nonnull %26, ptr nonnull %25, ptr nonnull %24)
  store i16 32, ptr %23, !tbaa !2
  store i16 63, ptr %22, !tbaa !2
  store i16 0, ptr %21, !tbaa !2
  store i16 31, ptr %20, !tbaa !2
  store i16 63, ptr %19, !tbaa !2
  store i16 31, ptr %18, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  store i16 63, ptr %16, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %23, ptr nonnull %22, ptr nonnull %21, ptr nonnull %20, ptr nonnull %19, ptr nonnull %18, ptr nonnull %17, ptr nonnull %16)
  store i16 64, ptr %15, !tbaa !2
  store i16 95, ptr %14, !tbaa !2
  store i16 31, ptr %13, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  store i16 63, ptr %11, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 63, ptr %9, !tbaa !2
  store i16 0, ptr %8, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %15, ptr nonnull %14, ptr nonnull %13, ptr nonnull %12, ptr nonnull %11, ptr nonnull %10, ptr nonnull %9, ptr nonnull %8)
  store i16 96, ptr %7, !tbaa !2
  store i16 127, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 63, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  call cc 1000 addrspace(1) void @SETPAL(ptr nonnull %7, ptr nonnull %6, ptr nonnull %5, ptr nonnull %4, ptr nonnull %3, ptr nonnull %2, ptr nonnull %1, ptr nonnull %0)
  %127 = load i16, ptr @"XIT%", !tbaa !2
  br label %b65

128:
  %129 = phi i16 [ %135, %128 ], [ -200, %b55 ]
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %107, i16 0)
  %130 = getelementptr i8, ptr %32, i16 2
  %data = load ptr, ptr %130
  %byte = load i8, ptr %data
  %131 = zext i8 %byte to i16
  %132 = getelementptr i8, ptr @"SP%", i16 %125
  %133 = getelementptr i8, ptr %132, i16 %129
  %134 = getelementptr i8, ptr %133, i16 200
  store i16 %131, ptr %134, !tbaa !2
  %135 = add nsw i16 %129, 2
  %136 = icmp eq i16 %135, 0
  br i1 %136, label %b61, label %128

b61:
  %137 = add nuw nsw i16 %124, 1
  %exitcond3 = icmp eq i16 %137, 101
  %138 = mul i16 %126, -1
  br i1 %exitcond3, label %b56, label %b55

b62:
  %139 = add nuw nsw i16 %141, 10
  %140 = icmp samesign ult i16 %141, 296
  br i1 %140, label %b65, label %290

b65:
  %141 = phi i16 [ 15, %b56 ], [ %139, %b62 ]
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 14, i16 6)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 %141, i16 7)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 13, i16 5)
  %142 = add nuw nsw i16 %141, 1
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 %142, i16 8)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 31, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 12, i16 4)
  %143 = add nuw nsw i16 %141, 2
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 %143, i16 9)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  %144 = icmp eq i16 %127, 1
  br i1 %144, label %b2, label %145

145:
  %146 = icmp eq i16 %127, 0
  br i1 %146, label %147, label %b62

147:
  %148 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %149 = icmp eq i8 %148, 1
  br i1 %149, label %35, label %b62

b74:
  %150 = add nuw nsw i16 %153, 10
  %151 = add i16 %150, -12
  %152 = icmp samesign ult i16 %151, 184
  br i1 %152, label %b81, label %161

b81:
  %153 = phi i16 [ 19, %298 ], [ %150, %b74 ]
  %154 = icmp eq i16 %291, 0
  %155 = add i16 %153, -2
  %156 = add i16 %153, -3
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 15, i16 %156)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 304, i16 %156)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 0, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 14, i16 6)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 305, i16 %155)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 13, i16 5)
  %157 = add i16 %153, -1
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 306, i16 %157)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 31, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 12, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 307, i16 %153)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  br i1 %154, label %158, label %b74

158:
  %159 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %160 = icmp eq i8 %159, 1
  br i1 %160, label %35, label %b74

161:
  %162 = getelementptr i8, ptr @"FSIN4%", i16 622
  br label %b90

b90:
  %163 = phi i16 [ %168, %b136 ], [ %168, %221 ], [ 0, %161 ]
  %164 = phi i16 [ %178, %b136 ], [ %178, %221 ], [ 0, %161 ]
  %165 = phi i16 [ %spec.store.select, %b136 ], [ %spec.store.select, %221 ], [ 0, %161 ]
  %166 = phi i16 [ %spec.store.select2, %b136 ], [ %spec.store.select2, %221 ], [ 0, %161 ]
  %167 = phi i16 [ %spec.store.select1, %b136 ], [ %spec.store.select1, %221 ], [ 0, %161 ]
  %168 = add i16 %163, 1
  %169 = icmp sgt i16 %168, 140
  %170 = icmp sgt i16 %164, -100
  %171 = and i1 %169, %170
  br i1 %171, label %b92, label %b93

b92:
  %172 = add nsw i16 %164, -1010
  br label %b94

b93:
  %173 = icmp slt i16 %168, 101
  %174 = icmp slt i16 %164, 10000
  %175 = and i1 %173, %174
  %176 = add i16 %164, 1000
  %177 = select i1 %175, i16 %176, i16 %164
  br label %b94

b94:
  %178 = phi i16 [ %172, %b92 ], [ %177, %b93 ]
  %179 = icmp eq i16 %178, -100
  br i1 %179, label %b2, label %b100

b100:
  %180 = add i16 %165, 1
  %181 = icmp eq i16 %180, 220
  %spec.store.select = select i1 %181, i16 0, i16 %180
  %182 = add i16 %167, 1
  %183 = icmp eq i16 %182, 283
  %spec.store.select1 = select i1 %183, i16 0, i16 %182
  %184 = add i16 %166, 1
  %185 = icmp eq i16 %184, 101
  %spec.store.select2 = select i1 %185, i16 0, i16 %184
  %186 = sitofp i16 %spec.store.select to float
  %187 = fdiv float %186, 3.500000e+01
  %188 = call float @llvm.fabs.f32(float %187)
  %189 = fcmp ogt float %188, 0x41E0000000000000
  br i1 %189, label %b114, label %b115

b114:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b115:
  %190 = sitofp i16 %spec.store.select1 to float
  %191 = fdiv float %190, 4.500000e+01
  %192 = call float @llvm.fabs.f32(float %191)
  %193 = fcmp ogt float %192, 0x41E0000000000000
  br i1 %193, label %b116, label %b117

b116:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b117:
  %194 = call float @llvm.sin.f32(float %187)
  %195 = fmul float %194, 4.800000e+01
  %196 = call i16 @llvm.lrint.i16.f32(float %195)
  %197 = call float @llvm.sin.f32(float %191)
  %198 = fmul float %197, 6.400000e+01
  %199 = call i16 @llvm.lrint.i16.f32(float %198)
  %invariant.gep4 = getelementptr [2 x i8], ptr @"FSIN4%", i16 %spec.store.select1
  %invariant.gep1 = getelementptr [2 x i8], ptr @"DT100%", i16 %199
  %invariant.gep2 = getelementptr [2 x i8], ptr @"DT%", i16 %196
  %200 = shl i16 %spec.store.select2, 1
  %201 = shl i16 %spec.store.select, 1
  br label %b121

b121:
  %202 = phi ptr [ %255, %b127 ], [ %162, %b117 ]
  %203 = phi i16 [ 8, %b117 ], [ %254, %b127 ]
  %204 = phi i16 [ 160, %b117 ], [ %206, %b127 ]
  %205 = add nuw nsw i16 %204, -24576
  store i16 %205, ptr @b$seg, !tbaa !2
  %206 = add nuw nsw i16 %204, 20
  %gep4 = getelementptr [2 x i8], ptr %invariant.gep4, i16 %203
  %207 = getelementptr i8, ptr %gep4, i16 638
  %208 = load i16, ptr %207, !tbaa !2
  %209 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %203
  %210 = getelementptr i8, ptr %209, i16 96
  %211 = load i16, ptr %210, !tbaa !2
  %212 = getelementptr [2 x i8], ptr @"FSIN1%", i16 %211
  %213 = getelementptr [2 x i8], ptr %212, i16 %208
  %214 = getelementptr i8, ptr %213, i16 96
  %215 = load i16, ptr %214, !tbaa !2
  %216 = add nsw i16 %203, -100
  %217 = mul i16 %215, 642
  %218 = mul i16 %208, 642
  br label %b126

b122:
  %219 = load i16, ptr @"XIT%", !tbaa !2
  %220 = icmp eq i16 %219, 0
  br i1 %220, label %286, label %221

221:
  %222 = icmp eq i16 %219, 1
  br i1 %222, label %b2, label %b90

b126:
  %223 = phi ptr [ %267, %b130 ], [ %202, %b121 ]
  %224 = phi i16 [ %266, %b130 ], [ 0, %b121 ]
  %225 = shl nuw nsw i16 %224, 1
  %226 = getelementptr i8, ptr @"FSIN1%", i16 %201
  %227 = getelementptr i8, ptr %226, i16 %225
  %228 = getelementptr i8, ptr %227, i16 128
  %229 = load i16, ptr %228, !tbaa !2
  %230 = load i16, ptr %223, !tbaa !2
  %231 = add i16 %230, %229
  %232 = mul i16 %231, 321
  %233 = add i16 %216, %232
  %234 = shl i16 %233, 1
  %235 = getelementptr i8, ptr %34, i16 2
  %236 = load i16, ptr %235, !tbaa !2
  %237 = inttoptr i16 %236 to ptr addrspace(2)
  %238 = addrspacecast ptr addrspace(2) %237 to ptr addrspace(1)
  %239 = getelementptr i8, ptr addrspace(1) %238, i16 %234
  %240 = getelementptr i8, ptr addrspace(1) %239, i16 31136
  %241 = load i16, ptr addrspace(1) %240, !tbaa !74
  %gep1 = getelementptr [2 x i8], ptr %invariant.gep1, i16 %241
  %242 = getelementptr i8, ptr %gep1, i16 360
  %243 = load i16, ptr %242, !tbaa !2
  %244 = add i16 %217, %218
  %245 = getelementptr i8, ptr addrspace(1) %238, i16 30848
  %246 = getelementptr i8, ptr addrspace(1) %245, i16 %244
  %247 = shl nuw nsw i16 %224, 1
  %248 = getelementptr i8, ptr addrspace(1) %246, i16 %247
  %249 = load i16, ptr addrspace(1) %248, !tbaa !74
  %gep2 = getelementptr [2 x i8], ptr %invariant.gep2, i16 %249
  %250 = getelementptr i8, ptr %gep2, i16 384
  %251 = load i16, ptr %250, !tbaa !2
  %252 = add i16 %251, %243
  %253 = icmp sgt i16 %252, %178
  br i1 %253, label %b130, label %b129

b127:
  %254 = add nuw nsw i16 %203, 1
  %exitcond1 = icmp eq i16 %254, 192
  %255 = getelementptr i8, ptr %202, i16 2
  br i1 %exitcond1, label %b122, label %b121

b129:
  %256 = getelementptr inbounds [2 x i8], ptr @"SP%", i16 %252
  %257 = load i16, ptr %256, !tbaa !2
  %258 = icmp eq i16 %257, 129
  br i1 %258, label %b131, label %b130

b130:
  %259 = phi i16 [ %285, %b131 ], [ 0, %b126 ], [ %257, %b129 ]
  %260 = trunc i16 %259 to i8
  %261 = load i16, ptr @b$seg, !tbaa !2
  %262 = inttoptr i16 %261 to ptr addrspace(2)
  %263 = addrspacecast ptr addrspace(2) %262 to ptr addrspace(1)
  %264 = getelementptr i8, ptr addrspace(1) %263, i16 16
  %265 = getelementptr i8, ptr addrspace(1) %264, i16 %224
  store volatile i8 %260, ptr addrspace(1) %265
  %266 = add nuw nsw i16 %224, 1
  %267 = getelementptr i8, ptr %223, i16 -2
  %exitcond = icmp eq i16 %266, 288
  br i1 %exitcond, label %b127, label %b126

b131:
  %268 = shl nuw nsw i16 %224, 1
  %269 = getelementptr i8, ptr @"FSIN3%", i16 %268
  %270 = getelementptr i8, ptr %269, i16 1312
  %271 = load i16, ptr %270, !tbaa !2
  %272 = shl nuw nsw i16 %224, 1
  %273 = getelementptr i8, ptr @"FSIN2%", i16 %200
  %274 = getelementptr i8, ptr %273, i16 %272
  %275 = getelementptr i8, ptr %274, i16 1312
  %276 = load i16, ptr %275, !tbaa !2
  %277 = mul i16 %203, 2
  %278 = getelementptr i8, ptr @"FSIN2%", i16 %277
  %279 = getelementptr i8, ptr %278, i16 1280
  %280 = load i16, ptr %279, !tbaa !2
  %281 = getelementptr [2 x i8], ptr @"MOD256128%", i16 %271
  %282 = getelementptr [2 x i8], ptr %281, i16 %276
  %283 = getelementptr [2 x i8], ptr %282, i16 %280
  %284 = getelementptr i8, ptr %283, i16 336
  %285 = load i16, ptr %284, !tbaa !2
  br label %b130

286:
  %287 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %288 = icmp eq i8 %287, 1
  br i1 %288, label %35, label %b136

b136:
  %289 = icmp eq i16 %219, 1
  br i1 %289, label %b2, label %b90

290:
  %291 = load i16, ptr @"XIT%", !tbaa !2
  %292 = icmp eq i16 %291, 0
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 14, i16 6)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 305, i16 7)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 13, i16 5)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 306, i16 8)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 31, i16 -1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 12, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$N2I2(i16 307, i16 9)
  call cc 1000 addrspace(1) void @llrm.qb.B$LINE(i16 27, i16 -1, i16 1)
  br i1 %292, label %293, label %296

293:
  %294 = call i8 @llrm.ia16.in.i8(i16 96) memory(inaccessiblemem: readwrite)
  %295 = icmp eq i8 %294, 1
  br i1 %295, label %35, label %298

296:
  %297 = icmp eq i16 %291, 1
  br i1 %297, label %b2, label %298

298:
  br label %b81
}

define cc 1000 void @BENCHMARK(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca [36 x i8]
  %4 = getelementptr inbounds nuw i8, ptr %3, i16 18
  call void @llvm.memset.p0.i16(ptr align 2 %3, i8 0, i16 36, i1 false)
  store i32 0, ptr %1, !tbaa !2
  store i32 0, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 767, i16 2, i16 257, ptr nonnull %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 5, i16 4, i16 257, ptr nonnull %4)
  call cc 1000 addrspace(1) void @TSCSNAP(ptr nonnull %1, ptr nonnull %2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  br label %b5

b5:
  %5 = phi ptr addrspace(1) [ %16, %b5 ], [ addrspacecast (ptr addrspace(2) inttoptr (i16 -24576 to ptr addrspace(2)) to ptr addrspace(1)), %b1 ]
  %6 = phi i32 [ %15, %b5 ], [ 64000, %b1 ]
  %7 = phi i32 [ 1, %b1 ], [ %12, %b5 ]
  %8 = phi i32 [ 0, %b1 ], [ %14, %b5 ]
  %9 = load volatile i8, ptr addrspace(1) %5
  %10 = zext i8 %9 to i32
  %11 = add i32 %7, %10
  %12 = srem i32 %11, 65521
  %13 = add i32 %12, %8
  %14 = srem i32 %13, 65521
  %15 = add nsw i32 %6, -1
  %16 = getelementptr i8, ptr addrspace(1) %5, i16 1
  %exitcond1 = icmp eq i32 %15, 0
  br i1 %exitcond1, label %b6, label %b5

b6:
  %17 = load i16, ptr %0
  %18 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %17)
  %19 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %18)
  %20 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string248, ptr %19)
  %21 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %20, ptr @$string249)
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %21, i16 0, i16 -1536)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call void @llrm.ia16.out.i8(i16 967, i8 0) memory(inaccessiblemem: readwrite)
  %22 = getelementptr i8, ptr %3, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  br label %b10

b10:
  %24 = phi i16 [ %31, %b10 ], [ -1536, %b6 ]
  %25 = inttoptr i16 %23 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 %24
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 1536
  %29 = call i8 @llrm.ia16.in.i8(i16 969) memory(inaccessiblemem: readwrite)
  %30 = zext i8 %29 to i16
  store i16 %30, ptr addrspace(1) %28, !tbaa !76
  %31 = add nsw i16 %24, 2
  %exitcond = icmp eq i16 %31, 0
  br i1 %exitcond, label %b11, label %b10

b11:
  %32 = getelementptr inbounds nuw i8, ptr %3, i16 20
  %33 = getelementptr i8, ptr %3, i16 2
  %34 = load i16, ptr %33, !tbaa !2
  %35 = inttoptr i16 %34 to ptr addrspace(2)
  %36 = addrspacecast ptr addrspace(2) %35 to ptr addrspace(1)
  %37 = addrspacecast ptr addrspace(1) %36 to ptr addrspace(2)
  %38 = ptrtoint ptr addrspace(2) %37 to i16
  store i16 %38, ptr @b$seg, !tbaa !2
  %39 = load i16, ptr %0
  %40 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %39)
  %41 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %40)
  %42 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string250, ptr %41)
  %43 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %42, ptr @$string251)
  %44 = load i16, ptr %33, !tbaa !2
  %45 = inttoptr i16 %44 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = ptrtoint ptr addrspace(1) %46 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %43, i16 %47, i16 1536)
  %48 = load i16, ptr %32, !tbaa !2
  %49 = inttoptr i16 %48 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = load i32, ptr @"BENCHHI&", !tbaa !2
  store i32 %51, ptr addrspace(1) %50, !tbaa !78
  %52 = getelementptr i8, ptr addrspace(1) %50, i16 4
  %53 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %53, ptr addrspace(1) %52, !tbaa !78
  %54 = getelementptr i8, ptr addrspace(1) %50, i16 8
  %55 = load i32, ptr %1, !tbaa !2
  store i32 %55, ptr addrspace(1) %54, !tbaa !78
  %56 = getelementptr i8, ptr addrspace(1) %50, i16 12
  %57 = load i32, ptr %2, !tbaa !2
  store i32 %57, ptr addrspace(1) %56, !tbaa !78
  %58 = getelementptr i8, ptr addrspace(1) %50, i16 16
  store i32 %14, ptr addrspace(1) %58, !tbaa !78
  %59 = getelementptr i8, ptr addrspace(1) %50, i16 20
  store i32 %12, ptr addrspace(1) %59, !tbaa !78
  %60 = addrspacecast ptr addrspace(1) %50 to ptr addrspace(2)
  %61 = ptrtoint ptr addrspace(2) %60 to i16
  store i16 %61, ptr @b$seg, !tbaa !2
  %62 = load i16, ptr %0
  %63 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %62)
  %64 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %63)
  %65 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string252, ptr %64)
  %66 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %65, ptr @$string253)
  %67 = load i16, ptr %32, !tbaa !2
  %68 = inttoptr i16 %67 to ptr addrspace(2)
  %69 = addrspacecast ptr addrspace(2) %68 to ptr addrspace(1)
  %70 = ptrtoint ptr addrspace(1) %69 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %66, i16 %70, i16 24)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc 1000 addrspace(1) void @TSCSNAP(ptr @"BENCHHI&", ptr @"BENCHLO&")
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %3)
  ret void
}

declare cc 1000 void @llrm.qb.B$RNZP(double) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$FCMD() addrspace(1) nocallback

declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1) threeway nocallback nounwind flagsresult

declare cc 1000 void @llrm.qb.B$N1I2(i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$N2I2(i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$LINE(i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$SCLS(i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$GET3(i16, ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1000 i16 @llrm.qb.B$FASC(ptr releases) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1000 void @llrm.qb.B$CLOS(i16, i16) addrspace(1) nocallback

declare float @llvm.fabs.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$STDL(ptr) addrspace(1) nocallback nounwind

declare cc 1000 ptr @llrm.qb.B$INKY() addrspace(1) nocallback

declare float @llvm.cos.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1000 void @llrm.qb.B$COLR(i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$LOCT(i16, i16, i16, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$RND0() addrspace(1) nocallback

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.sqrt.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.log2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.exp2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PAIN(i16, i16) addrspace(1) nocallback

declare cc 1000 i16 @llrm.qb.B$PNI2(i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PSTC(i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$GET4(i16, i32, ptr addrspace(1), i16) addrspace(1) nocallback

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$FMID(ptr releases, i16, i16) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1000 ptr @llrm.qb.B$FCHR(i16) addrspace(1) "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$CIRC(float, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PUT3(i16, ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$N1R4(float, float) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$N2R4(float, float) addrspace(1) nocallback

declare cc 1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1) nocallback

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1) nocallback nounwind

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i1 @llvm.umin.i1(i1, i1) nocallback nofree nosync nounwind speculatable willreturn memory(none)

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
