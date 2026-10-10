;; name: B$ATAN2, B$SINCOS
;; desc: the FPU's arctangent and sine
;;
;; args: ax:ptr double y, dx:ptr double x, cx:ptr double out; or ax:ptr angle, dx:ptr sine, cx:ptr cosine
;; retn: none
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed; the QB
;;       runtime's device code (dev*.c, gfxdev.c) is C around these. Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include qb.inc

.code
;;::::::::::::::
;; B$ATAN2 (ax: y, dx: x, cx: out)
B$ATAN2         proc    public

                push    si
                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                mov     bx, dx
                fld     qword ptr [bx]
                fpatan                          ;; atan(st1 / st0), in (-pi, pi]
                fstp    qword ptr [si]
                fwait
                pop     si
                ret
B$ATAN2         endp

;;::::::::::::::
;; B$SINCOS (ax: angle, dx: sine, cx: cosine)
B$SINCOS        proc    public

                push    si
                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                fsincos                         ;; st0 the cosine, st1 the sine
                fstp    qword ptr [si]
                mov     bx, dx
                fstp    qword ptr [bx]
                fwait
                pop     si
                ret
B$SINCOS        endp
                end
