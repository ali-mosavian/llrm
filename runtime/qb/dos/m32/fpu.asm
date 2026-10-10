;; name: B$ATAN2, B$SINCOS
;; desc: the FPU's arctangent and sine
;;
;; args: eax:ptr double y, edx:ptr double x, ebx:ptr double out; or eax:ptr angle, edx:ptr sine, ebx:ptr cosine
;; retn: none
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (the target's own convention: eax, edx, ebx, ecx).

                .386
                .387
                .model  flat

                public  B$ATAN2
                public  B$SINCOS

.code
;;::::::::::::::
;; B$ATAN2 (eax: y, edx: x, ebx: out)
B$ATAN2         proc

                fld     qword ptr [eax]
                fld     qword ptr [edx]
                fpatan                          ;; atan(st1 / st0), in (-pi, pi]
                fstp    qword ptr [ebx]
                fwait
                ret
B$ATAN2         endp

;;::::::::::::::
;; B$SINCOS (eax: angle, edx: sine, ebx: cosine)
B$SINCOS        proc

                fld     qword ptr [eax]
                fsincos                         ;; st0 the cosine, st1 the sine
                fstp    qword ptr [ebx]
                fstp    qword ptr [edx]
                fwait
                ret
B$SINCOS        endp
                end
