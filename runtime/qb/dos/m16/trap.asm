;; name: B$DIV0, B$TRAPS
;; desc: the divide fault: an integer division by zero is the error Division by
;;       zero
;;
;; args: B$TRAPS (none)                       | puts B$DIV0 at interrupt 0
;; retn: B$DIV0 does not return: the error is raised
;;
;; chng: oct/26 written [ali]
;; obs.: the CPU enters B$DIV0 with the faulting program's registers; DS is the
;;       program's own DGROUP there (compiled code keeps it), and SS is DGROUP.
;;       The shared layer's LL$SET_VECTOR keeps the old vector and puts it back
;;       when the program ends.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

                ;; error.c: qb_error, its argument in ax
                extrn   B$ERROR:far
                extrn   LL$SET_VECTOR:far

BE_DIVIDE0      equ     11
DIVIDE_VECTOR   equ     0

.code
;; B$DIV0 - the divide fault: raise the error Division by zero.
;;
;; The CPU enters with the faulting program's registers.  DS and ES are set
;; to DGROUP.  It does not return.
;;
;; Args:
;;     none.
;;
;; Saves:    none
;; Clobbers: ax ds es, flags
B$DIV0          proc    public

                mov     ax, DGROUP
                mov     ds, ax
                mov     es, ax
                sti
                mov     ax, BE_DIVIDE0
                call    B$ERROR
B$DIV0          endp

;; B$TRAPS - take interrupt 0 for B$DIV0.
;;
;; Args:
;;     none.
;;
;; Saves:    none
;; Clobbers: ax dx, flags (LL$SET_VECTOR's)
B$TRAPS         proc    public

                ;; LL$SET_VECTOR (number, handler), C's order
                push    S B$DIV0
                push    O B$DIV0
                push    DIVIDE_VECTOR
                call    LL$SET_VECTOR
                add     sp, 6
                ret
B$TRAPS         endp
                end
