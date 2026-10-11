;; name: B$ATAN2, B$SINCOS
;; desc: the FPU's arctangent and sine
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed;
;;       the QB runtime's device code (dev*.c, gfxdev.c) is C around these.
;;       Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include qb.inc

.code
;; B$ATAN2 - store atan(y / x), in (-pi, pi], at `out`.
;;
;; The FPU stack is left empty.
;;
;; Args:
;;     y   (ax): ptr to a double.
;;     x   (dx): ptr to a double.
;;     out (cx): ptr to a double.
;;
;; Returns:
;;     *out: the angle.
;;
;; Saves:    si
;; Clobbers: bx, flags, the FPU's status
B$ATAN2         proc    public uses si

                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                mov     bx, dx
                fld     qword ptr [bx]
                fpatan                          ;; atan(st1 / st0), in (-pi, pi]
                fstp    qword ptr [si]
                fwait
                ret
B$ATAN2         endp

;; B$SINCOS - store the sine and the cosine of an angle.
;;
;; The FPU stack is left empty.
;;
;; Args:
;;     angle  (ax): ptr to a double.
;;     sine   (dx): ptr to a double.
;;     cosine (cx): ptr to a double.
;;
;; Returns:
;;     *sine, *cosine.
;;
;; Saves:    si
;; Clobbers: bx, flags, the FPU's status
B$SINCOS        proc    public uses si

                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                fsincos                         ;; st0 the cosine, st1 the sine
                fstp    qword ptr [si]
                mov     bx, dx
                fstp    qword ptr [bx]
                fwait
                ret
B$SINCOS        endp
                end
