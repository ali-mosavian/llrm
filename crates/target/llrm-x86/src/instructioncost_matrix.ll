target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"
define void @arith_i8(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v1 = add i8 %i8, %i8
  %v2 = add i8 %i8, 8
  %v3 = add i8 %i8, 7
  %v4 = add i8 %i8, -1
  %v5 = sub i8 %i8, %i8
  %v6 = sub i8 %i8, 8
  %v7 = sub i8 %i8, 7
  %v8 = sub i8 %i8, -1
  %v9 = mul i8 %i8, %i8
  %v10 = mul i8 %i8, 8
  %v11 = mul i8 %i8, 7
  %v12 = mul i8 %i8, -1
  %v13 = sdiv i8 %i8, %i8
  %v14 = sdiv i8 %i8, 8
  %v15 = sdiv i8 %i8, 7
  %v16 = sdiv i8 %i8, -1
  %v17 = udiv i8 %i8, %i8
  %v18 = udiv i8 %i8, 8
  %v19 = udiv i8 %i8, 7
  %v20 = udiv i8 %i8, -1
  %v21 = srem i8 %i8, %i8
  %v22 = srem i8 %i8, 8
  %v23 = srem i8 %i8, 7
  %v24 = srem i8 %i8, -1
  %v25 = urem i8 %i8, %i8
  %v26 = urem i8 %i8, 8
  %v27 = urem i8 %i8, 7
  %v28 = urem i8 %i8, -1
  %v29 = shl i8 %i8, %i8
  %v30 = shl i8 %i8, 8
  %v31 = shl i8 %i8, 7
  %v32 = shl i8 %i8, -1
  %v33 = lshr i8 %i8, %i8
  %v34 = lshr i8 %i8, 8
  %v35 = lshr i8 %i8, 7
  %v36 = lshr i8 %i8, -1
  %v37 = ashr i8 %i8, %i8
  %v38 = ashr i8 %i8, 8
  %v39 = ashr i8 %i8, 7
  %v40 = ashr i8 %i8, -1
  %v41 = and i8 %i8, %i8
  %v42 = and i8 %i8, 8
  %v43 = and i8 %i8, 7
  %v44 = and i8 %i8, -1
  %v45 = or i8 %i8, %i8
  %v46 = or i8 %i8, 8
  %v47 = or i8 %i8, 7
  %v48 = or i8 %i8, -1
  %v49 = xor i8 %i8, %i8
  %v50 = xor i8 %i8, 8
  %v51 = xor i8 %i8, 7
  %v52 = xor i8 %i8, -1
  ret void
}
define void @arith_i16(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v53 = add i16 %i16, %i16
  %v54 = add i16 %i16, 8
  %v55 = add i16 %i16, 7
  %v56 = add i16 %i16, -1
  %v57 = sub i16 %i16, %i16
  %v58 = sub i16 %i16, 8
  %v59 = sub i16 %i16, 7
  %v60 = sub i16 %i16, -1
  %v61 = mul i16 %i16, %i16
  %v62 = mul i16 %i16, 8
  %v63 = mul i16 %i16, 7
  %v64 = mul i16 %i16, -1
  %v65 = sdiv i16 %i16, %i16
  %v66 = sdiv i16 %i16, 8
  %v67 = sdiv i16 %i16, 7
  %v68 = sdiv i16 %i16, -1
  %v69 = udiv i16 %i16, %i16
  %v70 = udiv i16 %i16, 8
  %v71 = udiv i16 %i16, 7
  %v72 = udiv i16 %i16, -1
  %v73 = srem i16 %i16, %i16
  %v74 = srem i16 %i16, 8
  %v75 = srem i16 %i16, 7
  %v76 = srem i16 %i16, -1
  %v77 = urem i16 %i16, %i16
  %v78 = urem i16 %i16, 8
  %v79 = urem i16 %i16, 7
  %v80 = urem i16 %i16, -1
  %v81 = shl i16 %i16, %i16
  %v82 = shl i16 %i16, 8
  %v83 = shl i16 %i16, 7
  %v84 = shl i16 %i16, -1
  %v85 = lshr i16 %i16, %i16
  %v86 = lshr i16 %i16, 8
  %v87 = lshr i16 %i16, 7
  %v88 = lshr i16 %i16, -1
  %v89 = ashr i16 %i16, %i16
  %v90 = ashr i16 %i16, 8
  %v91 = ashr i16 %i16, 7
  %v92 = ashr i16 %i16, -1
  %v93 = and i16 %i16, %i16
  %v94 = and i16 %i16, 8
  %v95 = and i16 %i16, 7
  %v96 = and i16 %i16, -1
  %v97 = or i16 %i16, %i16
  %v98 = or i16 %i16, 8
  %v99 = or i16 %i16, 7
  %v100 = or i16 %i16, -1
  %v101 = xor i16 %i16, %i16
  %v102 = xor i16 %i16, 8
  %v103 = xor i16 %i16, 7
  %v104 = xor i16 %i16, -1
  ret void
}
define void @arith_i32(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v105 = add i32 %i32, %i32
  %v106 = add i32 %i32, 8
  %v107 = add i32 %i32, 7
  %v108 = add i32 %i32, -1
  %v109 = sub i32 %i32, %i32
  %v110 = sub i32 %i32, 8
  %v111 = sub i32 %i32, 7
  %v112 = sub i32 %i32, -1
  %v113 = mul i32 %i32, %i32
  %v114 = mul i32 %i32, 8
  %v115 = mul i32 %i32, 7
  %v116 = mul i32 %i32, -1
  %v117 = sdiv i32 %i32, %i32
  %v118 = sdiv i32 %i32, 8
  %v119 = sdiv i32 %i32, 7
  %v120 = sdiv i32 %i32, -1
  %v121 = udiv i32 %i32, %i32
  %v122 = udiv i32 %i32, 8
  %v123 = udiv i32 %i32, 7
  %v124 = udiv i32 %i32, -1
  %v125 = srem i32 %i32, %i32
  %v126 = srem i32 %i32, 8
  %v127 = srem i32 %i32, 7
  %v128 = srem i32 %i32, -1
  %v129 = urem i32 %i32, %i32
  %v130 = urem i32 %i32, 8
  %v131 = urem i32 %i32, 7
  %v132 = urem i32 %i32, -1
  %v133 = shl i32 %i32, %i32
  %v134 = shl i32 %i32, 8
  %v135 = shl i32 %i32, 7
  %v136 = shl i32 %i32, -1
  %v137 = lshr i32 %i32, %i32
  %v138 = lshr i32 %i32, 8
  %v139 = lshr i32 %i32, 7
  %v140 = lshr i32 %i32, -1
  %v141 = ashr i32 %i32, %i32
  %v142 = ashr i32 %i32, 8
  %v143 = ashr i32 %i32, 7
  %v144 = ashr i32 %i32, -1
  %v145 = and i32 %i32, %i32
  %v146 = and i32 %i32, 8
  %v147 = and i32 %i32, 7
  %v148 = and i32 %i32, -1
  %v149 = or i32 %i32, %i32
  %v150 = or i32 %i32, 8
  %v151 = or i32 %i32, 7
  %v152 = or i32 %i32, -1
  %v153 = xor i32 %i32, %i32
  %v154 = xor i32 %i32, 8
  %v155 = xor i32 %i32, 7
  %v156 = xor i32 %i32, -1
  ret void
}
define void @arith_i64(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v157 = add i64 %i64, %i64
  %v158 = add i64 %i64, 8
  %v159 = add i64 %i64, 7
  %v160 = add i64 %i64, -1
  %v161 = sub i64 %i64, %i64
  %v162 = sub i64 %i64, 8
  %v163 = sub i64 %i64, 7
  %v164 = sub i64 %i64, -1
  %v165 = mul i64 %i64, %i64
  %v166 = mul i64 %i64, 8
  %v167 = mul i64 %i64, 7
  %v168 = mul i64 %i64, -1
  %v169 = sdiv i64 %i64, %i64
  %v170 = sdiv i64 %i64, 8
  %v171 = sdiv i64 %i64, 7
  %v172 = sdiv i64 %i64, -1
  %v173 = udiv i64 %i64, %i64
  %v174 = udiv i64 %i64, 8
  %v175 = udiv i64 %i64, 7
  %v176 = udiv i64 %i64, -1
  %v177 = srem i64 %i64, %i64
  %v178 = srem i64 %i64, 8
  %v179 = srem i64 %i64, 7
  %v180 = srem i64 %i64, -1
  %v181 = urem i64 %i64, %i64
  %v182 = urem i64 %i64, 8
  %v183 = urem i64 %i64, 7
  %v184 = urem i64 %i64, -1
  %v185 = shl i64 %i64, %i64
  %v186 = shl i64 %i64, 8
  %v187 = shl i64 %i64, 7
  %v188 = shl i64 %i64, -1
  %v189 = lshr i64 %i64, %i64
  %v190 = lshr i64 %i64, 8
  %v191 = lshr i64 %i64, 7
  %v192 = lshr i64 %i64, -1
  %v193 = ashr i64 %i64, %i64
  %v194 = ashr i64 %i64, 8
  %v195 = ashr i64 %i64, 7
  %v196 = ashr i64 %i64, -1
  %v197 = and i64 %i64, %i64
  %v198 = and i64 %i64, 8
  %v199 = and i64 %i64, 7
  %v200 = and i64 %i64, -1
  %v201 = or i64 %i64, %i64
  %v202 = or i64 %i64, 8
  %v203 = or i64 %i64, 7
  %v204 = or i64 %i64, -1
  %v205 = xor i64 %i64, %i64
  %v206 = xor i64 %i64, 8
  %v207 = xor i64 %i64, 7
  %v208 = xor i64 %i64, -1
  ret void
}
define void @farith_float(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v209 = fadd float %float, %float
  %v210 = fadd float %float, 2.0
  %v211 = fadd float %float, 3.0
  %v212 = fsub float %float, %float
  %v213 = fsub float %float, 2.0
  %v214 = fsub float %float, 3.0
  %v215 = fmul float %float, %float
  %v216 = fmul float %float, 2.0
  %v217 = fmul float %float, 3.0
  %v218 = fdiv float %float, %float
  %v219 = fdiv float %float, 2.0
  %v220 = fdiv float %float, 3.0
  %v221 = frem float %float, %float
  %v222 = frem float %float, 2.0
  %v223 = frem float %float, 3.0
  %v224 = fneg float %float
  ret void
}
define void @farith_double(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v225 = fadd double %double, %double
  %v226 = fadd double %double, 2.0
  %v227 = fadd double %double, 3.0
  %v228 = fsub double %double, %double
  %v229 = fsub double %double, 2.0
  %v230 = fsub double %double, 3.0
  %v231 = fmul double %double, %double
  %v232 = fmul double %double, 2.0
  %v233 = fmul double %double, 3.0
  %v234 = fdiv double %double, %double
  %v235 = fdiv double %double, 2.0
  %v236 = fdiv double %double, 3.0
  %v237 = frem double %double, %double
  %v238 = frem double %double, 2.0
  %v239 = frem double %double, 3.0
  %v240 = fneg double %double
  ret void
}
define void @farith_x86_fp80(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v241 = fadd x86_fp80 %x86_fp80, %x86_fp80
  %v242 = fadd x86_fp80 %x86_fp80, 2.0
  %v243 = fadd x86_fp80 %x86_fp80, 3.0
  %v244 = fsub x86_fp80 %x86_fp80, %x86_fp80
  %v245 = fsub x86_fp80 %x86_fp80, 2.0
  %v246 = fsub x86_fp80 %x86_fp80, 3.0
  %v247 = fmul x86_fp80 %x86_fp80, %x86_fp80
  %v248 = fmul x86_fp80 %x86_fp80, 2.0
  %v249 = fmul x86_fp80 %x86_fp80, 3.0
  %v250 = fdiv x86_fp80 %x86_fp80, %x86_fp80
  %v251 = fdiv x86_fp80 %x86_fp80, 2.0
  %v252 = fdiv x86_fp80 %x86_fp80, 3.0
  %v253 = frem x86_fp80 %x86_fp80, %x86_fp80
  %v254 = frem x86_fp80 %x86_fp80, 2.0
  %v255 = frem x86_fp80 %x86_fp80, 3.0
  %v256 = fneg x86_fp80 %x86_fp80
  ret void
}
define void @icmp_i8(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v257 = icmp eq i8 %i8, %i8
  %v258 = icmp eq i8 %i8, 0
  %v259 = icmp eq i8 %i8, 5
  %v260 = icmp ne i8 %i8, %i8
  %v261 = icmp ne i8 %i8, 0
  %v262 = icmp ne i8 %i8, 5
  %v263 = icmp ugt i8 %i8, %i8
  %v264 = icmp ugt i8 %i8, 0
  %v265 = icmp ugt i8 %i8, 5
  %v266 = icmp uge i8 %i8, %i8
  %v267 = icmp uge i8 %i8, 0
  %v268 = icmp uge i8 %i8, 5
  %v269 = icmp ult i8 %i8, %i8
  %v270 = icmp ult i8 %i8, 0
  %v271 = icmp ult i8 %i8, 5
  %v272 = icmp ule i8 %i8, %i8
  %v273 = icmp ule i8 %i8, 0
  %v274 = icmp ule i8 %i8, 5
  %v275 = icmp sgt i8 %i8, %i8
  %v276 = icmp sgt i8 %i8, 0
  %v277 = icmp sgt i8 %i8, 5
  %v278 = icmp sge i8 %i8, %i8
  %v279 = icmp sge i8 %i8, 0
  %v280 = icmp sge i8 %i8, 5
  %v281 = icmp slt i8 %i8, %i8
  %v282 = icmp slt i8 %i8, 0
  %v283 = icmp slt i8 %i8, 5
  %v284 = icmp sle i8 %i8, %i8
  %v285 = icmp sle i8 %i8, 0
  %v286 = icmp sle i8 %i8, 5
  ret void
}
define void @icmp_i16(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v287 = icmp eq i16 %i16, %i16
  %v288 = icmp eq i16 %i16, 0
  %v289 = icmp eq i16 %i16, 5
  %v290 = icmp ne i16 %i16, %i16
  %v291 = icmp ne i16 %i16, 0
  %v292 = icmp ne i16 %i16, 5
  %v293 = icmp ugt i16 %i16, %i16
  %v294 = icmp ugt i16 %i16, 0
  %v295 = icmp ugt i16 %i16, 5
  %v296 = icmp uge i16 %i16, %i16
  %v297 = icmp uge i16 %i16, 0
  %v298 = icmp uge i16 %i16, 5
  %v299 = icmp ult i16 %i16, %i16
  %v300 = icmp ult i16 %i16, 0
  %v301 = icmp ult i16 %i16, 5
  %v302 = icmp ule i16 %i16, %i16
  %v303 = icmp ule i16 %i16, 0
  %v304 = icmp ule i16 %i16, 5
  %v305 = icmp sgt i16 %i16, %i16
  %v306 = icmp sgt i16 %i16, 0
  %v307 = icmp sgt i16 %i16, 5
  %v308 = icmp sge i16 %i16, %i16
  %v309 = icmp sge i16 %i16, 0
  %v310 = icmp sge i16 %i16, 5
  %v311 = icmp slt i16 %i16, %i16
  %v312 = icmp slt i16 %i16, 0
  %v313 = icmp slt i16 %i16, 5
  %v314 = icmp sle i16 %i16, %i16
  %v315 = icmp sle i16 %i16, 0
  %v316 = icmp sle i16 %i16, 5
  ret void
}
define void @icmp_i32(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v317 = icmp eq i32 %i32, %i32
  %v318 = icmp eq i32 %i32, 0
  %v319 = icmp eq i32 %i32, 5
  %v320 = icmp ne i32 %i32, %i32
  %v321 = icmp ne i32 %i32, 0
  %v322 = icmp ne i32 %i32, 5
  %v323 = icmp ugt i32 %i32, %i32
  %v324 = icmp ugt i32 %i32, 0
  %v325 = icmp ugt i32 %i32, 5
  %v326 = icmp uge i32 %i32, %i32
  %v327 = icmp uge i32 %i32, 0
  %v328 = icmp uge i32 %i32, 5
  %v329 = icmp ult i32 %i32, %i32
  %v330 = icmp ult i32 %i32, 0
  %v331 = icmp ult i32 %i32, 5
  %v332 = icmp ule i32 %i32, %i32
  %v333 = icmp ule i32 %i32, 0
  %v334 = icmp ule i32 %i32, 5
  %v335 = icmp sgt i32 %i32, %i32
  %v336 = icmp sgt i32 %i32, 0
  %v337 = icmp sgt i32 %i32, 5
  %v338 = icmp sge i32 %i32, %i32
  %v339 = icmp sge i32 %i32, 0
  %v340 = icmp sge i32 %i32, 5
  %v341 = icmp slt i32 %i32, %i32
  %v342 = icmp slt i32 %i32, 0
  %v343 = icmp slt i32 %i32, 5
  %v344 = icmp sle i32 %i32, %i32
  %v345 = icmp sle i32 %i32, 0
  %v346 = icmp sle i32 %i32, 5
  ret void
}
define void @icmp_i64(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v347 = icmp eq i64 %i64, %i64
  %v348 = icmp eq i64 %i64, 0
  %v349 = icmp eq i64 %i64, 5
  %v350 = icmp ne i64 %i64, %i64
  %v351 = icmp ne i64 %i64, 0
  %v352 = icmp ne i64 %i64, 5
  %v353 = icmp ugt i64 %i64, %i64
  %v354 = icmp ugt i64 %i64, 0
  %v355 = icmp ugt i64 %i64, 5
  %v356 = icmp uge i64 %i64, %i64
  %v357 = icmp uge i64 %i64, 0
  %v358 = icmp uge i64 %i64, 5
  %v359 = icmp ult i64 %i64, %i64
  %v360 = icmp ult i64 %i64, 0
  %v361 = icmp ult i64 %i64, 5
  %v362 = icmp ule i64 %i64, %i64
  %v363 = icmp ule i64 %i64, 0
  %v364 = icmp ule i64 %i64, 5
  %v365 = icmp sgt i64 %i64, %i64
  %v366 = icmp sgt i64 %i64, 0
  %v367 = icmp sgt i64 %i64, 5
  %v368 = icmp sge i64 %i64, %i64
  %v369 = icmp sge i64 %i64, 0
  %v370 = icmp sge i64 %i64, 5
  %v371 = icmp slt i64 %i64, %i64
  %v372 = icmp slt i64 %i64, 0
  %v373 = icmp slt i64 %i64, 5
  %v374 = icmp sle i64 %i64, %i64
  %v375 = icmp sle i64 %i64, 0
  %v376 = icmp sle i64 %i64, 5
  ret void
}
define void @fcmp_float(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v377 = fcmp oeq float %float, %float
  %v378 = fcmp ogt float %float, %float
  %v379 = fcmp oge float %float, %float
  %v380 = fcmp olt float %float, %float
  %v381 = fcmp ole float %float, %float
  %v382 = fcmp one float %float, %float
  %v383 = fcmp ord float %float, %float
  %v384 = fcmp ueq float %float, %float
  %v385 = fcmp ugt float %float, %float
  %v386 = fcmp uge float %float, %float
  %v387 = fcmp ult float %float, %float
  %v388 = fcmp ule float %float, %float
  %v389 = fcmp une float %float, %float
  %v390 = fcmp uno float %float, %float
  ret void
}
define void @fcmp_double(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v391 = fcmp oeq double %double, %double
  %v392 = fcmp ogt double %double, %double
  %v393 = fcmp oge double %double, %double
  %v394 = fcmp olt double %double, %double
  %v395 = fcmp ole double %double, %double
  %v396 = fcmp one double %double, %double
  %v397 = fcmp ord double %double, %double
  %v398 = fcmp ueq double %double, %double
  %v399 = fcmp ugt double %double, %double
  %v400 = fcmp uge double %double, %double
  %v401 = fcmp ult double %double, %double
  %v402 = fcmp ule double %double, %double
  %v403 = fcmp une double %double, %double
  %v404 = fcmp uno double %double, %double
  ret void
}
define void @fcmp_x86_fp80(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v405 = fcmp oeq x86_fp80 %x86_fp80, %x86_fp80
  %v406 = fcmp ogt x86_fp80 %x86_fp80, %x86_fp80
  %v407 = fcmp oge x86_fp80 %x86_fp80, %x86_fp80
  %v408 = fcmp olt x86_fp80 %x86_fp80, %x86_fp80
  %v409 = fcmp ole x86_fp80 %x86_fp80, %x86_fp80
  %v410 = fcmp one x86_fp80 %x86_fp80, %x86_fp80
  %v411 = fcmp ord x86_fp80 %x86_fp80, %x86_fp80
  %v412 = fcmp ueq x86_fp80 %x86_fp80, %x86_fp80
  %v413 = fcmp ugt x86_fp80 %x86_fp80, %x86_fp80
  %v414 = fcmp uge x86_fp80 %x86_fp80, %x86_fp80
  %v415 = fcmp ult x86_fp80 %x86_fp80, %x86_fp80
  %v416 = fcmp ule x86_fp80 %x86_fp80, %x86_fp80
  %v417 = fcmp une x86_fp80 %x86_fp80, %x86_fp80
  %v418 = fcmp uno x86_fp80 %x86_fp80, %x86_fp80
  ret void
}
define void @selects(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v419 = select i1 %c, i8 %i8, i8 %i8
  %v420 = icmp slt i32 %i32, %n
  %v421 = select i1 %v420, i8 %i8, i8 %i8
  %v422 = select i1 %c, i16 %i16, i16 %i16
  %v423 = icmp slt i32 %i32, %n
  %v424 = select i1 %v423, i16 %i16, i16 %i16
  %v425 = select i1 %c, i32 %i32, i32 %i32
  %v426 = icmp slt i32 %i32, %n
  %v427 = select i1 %v426, i32 %i32, i32 %i32
  %v428 = select i1 %c, i64 %i64, i64 %i64
  %v429 = icmp slt i32 %i32, %n
  %v430 = select i1 %v429, i64 %i64, i64 %i64
  %v431 = select i1 %c, float %float, float %float
  %v432 = icmp slt i32 %i32, %n
  %v433 = select i1 %v432, float %float, float %float
  %v434 = select i1 %c, double %double, double %double
  %v435 = icmp slt i32 %i32, %n
  %v436 = select i1 %v435, double %double, double %double
  %v437 = select i1 %c, x86_fp80 %x86_fp80, x86_fp80 %x86_fp80
  %v438 = icmp slt i32 %i32, %n
  %v439 = select i1 %v438, x86_fp80 %x86_fp80, x86_fp80 %x86_fp80
  %v440 = select i1 %c, ptr %p, ptr %p
  %v441 = icmp slt i32 %i32, %n
  %v442 = select i1 %v441, ptr %p, ptr %p
  ret void
}
define void @intcasts(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v443 = zext i1 %c to i8
  %v444 = sext i1 %c to i8
  %v445 = zext i1 %c to i16
  %v446 = sext i1 %c to i16
  %v447 = zext i1 %c to i32
  %v448 = sext i1 %c to i32
  %v449 = zext i1 %c to i64
  %v450 = sext i1 %c to i64
  %v451 = trunc i8 %i8 to i1
  %v452 = load i8, ptr %p
  %v453 = trunc i8 %v452 to i1
  %v454 = zext i8 %i8 to i16
  %v455 = load i8, ptr %p
  %v456 = zext i8 %v455 to i16
  %v457 = sext i8 %i8 to i16
  %v458 = load i8, ptr %p
  %v459 = sext i8 %v458 to i16
  %v460 = zext i8 %i8 to i32
  %v461 = load i8, ptr %p
  %v462 = zext i8 %v461 to i32
  %v463 = sext i8 %i8 to i32
  %v464 = load i8, ptr %p
  %v465 = sext i8 %v464 to i32
  %v466 = zext i8 %i8 to i64
  %v467 = load i8, ptr %p
  %v468 = zext i8 %v467 to i64
  %v469 = sext i8 %i8 to i64
  %v470 = load i8, ptr %p
  %v471 = sext i8 %v470 to i64
  %v472 = trunc i16 %i16 to i1
  %v473 = load i16, ptr %p
  %v474 = trunc i16 %v473 to i1
  %v475 = trunc i16 %i16 to i8
  %v476 = load i16, ptr %p
  %v477 = trunc i16 %v476 to i8
  %v478 = zext i16 %i16 to i32
  %v479 = load i16, ptr %p
  %v480 = zext i16 %v479 to i32
  %v481 = sext i16 %i16 to i32
  %v482 = load i16, ptr %p
  %v483 = sext i16 %v482 to i32
  %v484 = zext i16 %i16 to i64
  %v485 = load i16, ptr %p
  %v486 = zext i16 %v485 to i64
  %v487 = sext i16 %i16 to i64
  %v488 = load i16, ptr %p
  %v489 = sext i16 %v488 to i64
  %v490 = trunc i16 %i16 to i8
  store i8 %v490, ptr %q
  %v491 = trunc i32 %i32 to i1
  %v492 = load i32, ptr %p
  %v493 = trunc i32 %v492 to i1
  %v494 = trunc i32 %i32 to i8
  %v495 = load i32, ptr %p
  %v496 = trunc i32 %v495 to i8
  %v497 = trunc i32 %i32 to i16
  %v498 = load i32, ptr %p
  %v499 = trunc i32 %v498 to i16
  %v500 = zext i32 %i32 to i64
  %v501 = load i32, ptr %p
  %v502 = zext i32 %v501 to i64
  %v503 = sext i32 %i32 to i64
  %v504 = load i32, ptr %p
  %v505 = sext i32 %v504 to i64
  %v506 = trunc i32 %i32 to i8
  store i8 %v506, ptr %q
  %v507 = trunc i32 %i32 to i16
  store i16 %v507, ptr %q
  %v508 = trunc i64 %i64 to i1
  %v509 = load i64, ptr %p
  %v510 = trunc i64 %v509 to i1
  %v511 = trunc i64 %i64 to i8
  %v512 = load i64, ptr %p
  %v513 = trunc i64 %v512 to i8
  %v514 = trunc i64 %i64 to i16
  %v515 = load i64, ptr %p
  %v516 = trunc i64 %v515 to i16
  %v517 = trunc i64 %i64 to i32
  %v518 = load i64, ptr %p
  %v519 = trunc i64 %v518 to i32
  %v520 = trunc i64 %i64 to i8
  store i8 %v520, ptr %q
  %v521 = trunc i64 %i64 to i16
  store i16 %v521, ptr %q
  %v522 = trunc i64 %i64 to i32
  store i32 %v522, ptr %q
  ret void
}
define void @fpcasts(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v523 = sitofp i8 %i8 to float
  %v524 = load i8, ptr %p
  %v525 = sitofp i8 %v524 to float
  %v526 = uitofp i8 %i8 to float
  %v527 = load i8, ptr %p
  %v528 = uitofp i8 %v527 to float
  %v529 = sitofp i8 %i8 to double
  %v530 = load i8, ptr %p
  %v531 = sitofp i8 %v530 to double
  %v532 = uitofp i8 %i8 to double
  %v533 = load i8, ptr %p
  %v534 = uitofp i8 %v533 to double
  %v535 = sitofp i8 %i8 to x86_fp80
  %v536 = load i8, ptr %p
  %v537 = sitofp i8 %v536 to x86_fp80
  %v538 = uitofp i8 %i8 to x86_fp80
  %v539 = load i8, ptr %p
  %v540 = uitofp i8 %v539 to x86_fp80
  %v541 = sitofp i16 %i16 to float
  %v542 = load i16, ptr %p
  %v543 = sitofp i16 %v542 to float
  %v544 = uitofp i16 %i16 to float
  %v545 = load i16, ptr %p
  %v546 = uitofp i16 %v545 to float
  %v547 = sitofp i16 %i16 to double
  %v548 = load i16, ptr %p
  %v549 = sitofp i16 %v548 to double
  %v550 = uitofp i16 %i16 to double
  %v551 = load i16, ptr %p
  %v552 = uitofp i16 %v551 to double
  %v553 = sitofp i16 %i16 to x86_fp80
  %v554 = load i16, ptr %p
  %v555 = sitofp i16 %v554 to x86_fp80
  %v556 = uitofp i16 %i16 to x86_fp80
  %v557 = load i16, ptr %p
  %v558 = uitofp i16 %v557 to x86_fp80
  %v559 = sitofp i32 %i32 to float
  %v560 = load i32, ptr %p
  %v561 = sitofp i32 %v560 to float
  %v562 = uitofp i32 %i32 to float
  %v563 = load i32, ptr %p
  %v564 = uitofp i32 %v563 to float
  %v565 = sitofp i32 %i32 to double
  %v566 = load i32, ptr %p
  %v567 = sitofp i32 %v566 to double
  %v568 = uitofp i32 %i32 to double
  %v569 = load i32, ptr %p
  %v570 = uitofp i32 %v569 to double
  %v571 = sitofp i32 %i32 to x86_fp80
  %v572 = load i32, ptr %p
  %v573 = sitofp i32 %v572 to x86_fp80
  %v574 = uitofp i32 %i32 to x86_fp80
  %v575 = load i32, ptr %p
  %v576 = uitofp i32 %v575 to x86_fp80
  %v577 = sitofp i64 %i64 to float
  %v578 = load i64, ptr %p
  %v579 = sitofp i64 %v578 to float
  %v580 = uitofp i64 %i64 to float
  %v581 = load i64, ptr %p
  %v582 = uitofp i64 %v581 to float
  %v583 = sitofp i64 %i64 to double
  %v584 = load i64, ptr %p
  %v585 = sitofp i64 %v584 to double
  %v586 = uitofp i64 %i64 to double
  %v587 = load i64, ptr %p
  %v588 = uitofp i64 %v587 to double
  %v589 = sitofp i64 %i64 to x86_fp80
  %v590 = load i64, ptr %p
  %v591 = sitofp i64 %v590 to x86_fp80
  %v592 = uitofp i64 %i64 to x86_fp80
  %v593 = load i64, ptr %p
  %v594 = uitofp i64 %v593 to x86_fp80
  %v595 = fptosi float %float to i8
  %v596 = fptoui float %float to i8
  %v597 = fptosi float %float to i16
  %v598 = fptoui float %float to i16
  %v599 = fptosi float %float to i32
  %v600 = fptoui float %float to i32
  %v601 = fptosi float %float to i64
  %v602 = fptoui float %float to i64
  %v603 = fptosi double %double to i8
  %v604 = fptoui double %double to i8
  %v605 = fptosi double %double to i16
  %v606 = fptoui double %double to i16
  %v607 = fptosi double %double to i32
  %v608 = fptoui double %double to i32
  %v609 = fptosi double %double to i64
  %v610 = fptoui double %double to i64
  %v611 = fptosi x86_fp80 %x86_fp80 to i8
  %v612 = fptoui x86_fp80 %x86_fp80 to i8
  %v613 = fptosi x86_fp80 %x86_fp80 to i16
  %v614 = fptoui x86_fp80 %x86_fp80 to i16
  %v615 = fptosi x86_fp80 %x86_fp80 to i32
  %v616 = fptoui x86_fp80 %x86_fp80 to i32
  %v617 = fptosi x86_fp80 %x86_fp80 to i64
  %v618 = fptoui x86_fp80 %x86_fp80 to i64
  %v619 = fpext float %float to double
  %v620 = fpext float %float to x86_fp80
  %v621 = fpext double %double to x86_fp80
  %v622 = fptrunc double %double to float
  %v623 = fptrunc x86_fp80 %x86_fp80 to float
  %v624 = fptrunc x86_fp80 %x86_fp80 to double
  %v625 = bitcast i32 %i32 to float
  %v626 = bitcast float %float to i32
  %v627 = bitcast i64 %i64 to double
  %v628 = bitcast double %double to i64
  %v629 = ptrtoint ptr %p to i16
  %v630 = inttoptr i16 %i16 to ptr
  %v631 = ptrtoint ptr %p to i32
  %v632 = inttoptr i32 %i32 to ptr
  %v633 = ptrtoint ptr %p to i64
  %v634 = inttoptr i64 %i64 to ptr
  ret void
}
define void @memops(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v635 = load i8, ptr %p
  store i8 %i8, ptr %q
  %v636 = getelementptr inbounds i8, ptr %p, i32 3
  %v637 = load i8, ptr %v636
  store i8 %i8, ptr %v636
  %v638 = getelementptr inbounds i8, ptr %p, i32 %i32
  %v639 = load i8, ptr %v638
  store i8 %i8, ptr %v638
  %v640 = getelementptr inbounds i8, ptr %p, i16 %i16
  %v641 = load i8, ptr %v640
  store i8 %i8, ptr %v640
  %v642 = getelementptr inbounds i8, ptr %p, i64 %i64
  %v643 = load i8, ptr %v642
  store i8 %i8, ptr %v642
  %v644 = load i16, ptr %p
  store i16 %i16, ptr %q
  %v645 = getelementptr inbounds i16, ptr %p, i32 3
  %v646 = load i16, ptr %v645
  store i16 %i16, ptr %v645
  %v647 = getelementptr inbounds i16, ptr %p, i32 %i32
  %v648 = load i16, ptr %v647
  store i16 %i16, ptr %v647
  %v649 = getelementptr inbounds i16, ptr %p, i16 %i16
  %v650 = load i16, ptr %v649
  store i16 %i16, ptr %v649
  %v651 = getelementptr inbounds i16, ptr %p, i64 %i64
  %v652 = load i16, ptr %v651
  store i16 %i16, ptr %v651
  %v653 = load i32, ptr %p
  store i32 %i32, ptr %q
  %v654 = getelementptr inbounds i32, ptr %p, i32 3
  %v655 = load i32, ptr %v654
  store i32 %i32, ptr %v654
  %v656 = getelementptr inbounds i32, ptr %p, i32 %i32
  %v657 = load i32, ptr %v656
  store i32 %i32, ptr %v656
  %v658 = getelementptr inbounds i32, ptr %p, i16 %i16
  %v659 = load i32, ptr %v658
  store i32 %i32, ptr %v658
  %v660 = getelementptr inbounds i32, ptr %p, i64 %i64
  %v661 = load i32, ptr %v660
  store i32 %i32, ptr %v660
  %v662 = load i64, ptr %p
  store i64 %i64, ptr %q
  %v663 = getelementptr inbounds i64, ptr %p, i32 3
  %v664 = load i64, ptr %v663
  store i64 %i64, ptr %v663
  %v665 = getelementptr inbounds i64, ptr %p, i32 %i32
  %v666 = load i64, ptr %v665
  store i64 %i64, ptr %v665
  %v667 = getelementptr inbounds i64, ptr %p, i16 %i16
  %v668 = load i64, ptr %v667
  store i64 %i64, ptr %v667
  %v669 = getelementptr inbounds i64, ptr %p, i64 %i64
  %v670 = load i64, ptr %v669
  store i64 %i64, ptr %v669
  %v671 = load float, ptr %p
  store float %float, ptr %q
  %v672 = getelementptr inbounds float, ptr %p, i32 3
  %v673 = load float, ptr %v672
  store float %float, ptr %v672
  %v674 = getelementptr inbounds float, ptr %p, i32 %i32
  %v675 = load float, ptr %v674
  store float %float, ptr %v674
  %v676 = getelementptr inbounds float, ptr %p, i16 %i16
  %v677 = load float, ptr %v676
  store float %float, ptr %v676
  %v678 = getelementptr inbounds float, ptr %p, i64 %i64
  %v679 = load float, ptr %v678
  store float %float, ptr %v678
  %v680 = load double, ptr %p
  store double %double, ptr %q
  %v681 = getelementptr inbounds double, ptr %p, i32 3
  %v682 = load double, ptr %v681
  store double %double, ptr %v681
  %v683 = getelementptr inbounds double, ptr %p, i32 %i32
  %v684 = load double, ptr %v683
  store double %double, ptr %v683
  %v685 = getelementptr inbounds double, ptr %p, i16 %i16
  %v686 = load double, ptr %v685
  store double %double, ptr %v685
  %v687 = getelementptr inbounds double, ptr %p, i64 %i64
  %v688 = load double, ptr %v687
  store double %double, ptr %v687
  %v689 = load x86_fp80, ptr %p
  store x86_fp80 %x86_fp80, ptr %q
  %v690 = getelementptr inbounds x86_fp80, ptr %p, i32 3
  %v691 = load x86_fp80, ptr %v690
  store x86_fp80 %x86_fp80, ptr %v690
  %v692 = getelementptr inbounds x86_fp80, ptr %p, i32 %i32
  %v693 = load x86_fp80, ptr %v692
  store x86_fp80 %x86_fp80, ptr %v692
  %v694 = getelementptr inbounds x86_fp80, ptr %p, i16 %i16
  %v695 = load x86_fp80, ptr %v694
  store x86_fp80 %x86_fp80, ptr %v694
  %v696 = getelementptr inbounds x86_fp80, ptr %p, i64 %i64
  %v697 = load x86_fp80, ptr %v696
  store x86_fp80 %x86_fp80, ptr %v696
  %v698 = load ptr, ptr %p
  store ptr %p, ptr %q
  %v699 = getelementptr inbounds ptr, ptr %p, i32 3
  %v700 = load ptr, ptr %v699
  store ptr %p, ptr %v699
  %v701 = getelementptr inbounds ptr, ptr %p, i32 %i32
  %v702 = load ptr, ptr %v701
  store ptr %p, ptr %v701
  %v703 = getelementptr inbounds ptr, ptr %p, i16 %i16
  %v704 = load ptr, ptr %v703
  store ptr %p, ptr %v703
  %v705 = getelementptr inbounds ptr, ptr %p, i64 %i64
  %v706 = load ptr, ptr %v705
  store ptr %p, ptr %v705
  store i8 0, ptr %q
  store i8 1, ptr %q
  store i16 0, ptr %q
  store i16 1, ptr %q
  store i32 0, ptr %q
  store i32 1, ptr %q
  %v707 = getelementptr [8 x i32], ptr %p, i32 %i32, i32 %n
  %v708 = getelementptr [8 x i32], ptr %p, i32 1, i32 2
  %v709 = getelementptr { i8, i32, [4 x i16] }, ptr %p, i32 %i32, i32 2, i32 %n
  %v710 = getelementptr i8, ptr %p, i32 %i32
  ret void
}
define void @misc(i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n) {
  %v711 = freeze i32 %i32
  %v712 = alloca i32
  %v713 = alloca i32, i32 %n
  %v714 = alloca [16 x i8]
  %v715 = call i32 @ext(i32 %i32)
  call void @ext0()
  %v716 = call double @ext_d(double %double)
  %v717 = call i32 @llvm.ctpop.i32(i32 %i32)
  %v718 = call i16 @llvm.bswap.i16(i16 %i16)
  %v719 = call float @llvm.fabs.f32(float %float)
  %v720 = call double @llvm.sqrt.f64(double %double)
  %v721 = call i32 @llvm.smax.i32(i32 %i32, i32 %n)
  %v722 = call i64 @llvm.umin.i64(i64 %i64, i64 %i64)
  ret void
}
declare i32 @ext(i32)
declare void @ext0()
declare double @ext_d(double)
declare i32 @llvm.ctpop.i32(i32)
declare i16 @llvm.bswap.i16(i16)
declare float @llvm.fabs.f32(float)
declare double @llvm.sqrt.f64(double)
declare i32 @llvm.smax.i32(i32, i32)
declare i64 @llvm.umin.i64(i64, i64)
