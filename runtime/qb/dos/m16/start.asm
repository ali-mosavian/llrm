;; name: start
;; desc: startup of a QB45 compiled module
;;
;; args: [in] es -> PSP, ss:sp -> the STACK segment
;; retn: none; enters the module at BC_SA:30h
;;
;; chng: oct/26 written [ali]
;; obs.: DGROUP is the static data, then STACK, then one dynamic region
;;       from qb_atopsp to the top of the group (rt/rtinit.asm:244-300).
;;       DOSSEG makes the linker put STACK last. The linker gives ss the
;;       STACK segment's own frame, so it is rebased to DGROUP, keeping
;;       the physical stack, to let a near pointer to a frame cell reach
;;       it through ds.
;;
;;       Every module with an initializer declares XIB, XI and XIE in that
;;       order (xi.h), so the first the linker meets fixes the order, and
;;       the bounds below are those segments' bases.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc
                dosseg

qb_start        proto   far c
                extrn   c llrm_os_psp:word

                public  c qb_atopsp
                public  c qb_asizds
                public  c qb_xi_begin
                public  c qb_xi_end
                public  c qb_module_segment

PAGES_64K       equ     1000h                   ;; paragraphs in 64 KB
LAST_WORD       equ     0FFFEh                  ;; the last word of 64 KB
HEADER_CODE     equ     30h                     ;; where the module's code starts

;; DGROUP's first bytes are zero: the compiled code uses address 0 as the
;; empty string's descriptor (PRINT with nothing passes it)
_NULL           segment para public 'BEGDATA'
                word    8 dup (0)
_NULL           ends

XIB             segment word public 'DATA'
XIB             ends
XI              segment word public 'DATA'
XI              ends
XIE             segment word public 'DATA'
XIE             ends

_DATA           segment word public 'DATA'
qb_asizds       word    0                       ;; the last usable word of DGROUP
qb_xi_begin     word    O DGROUP:XIB
qb_xi_end       word    O DGROUP:XIE
qb_module_segment word  0                       ;; the module's code segment
_DATA           ends

_BSS            segment word public 'BSS'
_BSS            ends

STACK           segment para stack 'STACK'
                byte    2048 dup (?)
qb_atopsp       label   byte
STACK           ends

;; the compiler's zero-length BC_SAB precedes BC_SA: a label in the same
;; public segment is BC_SA's first far address, the module header
BC_SAB          segment word public 'BC_SEGS'
bc_sa           label   byte
BC_SAB          ends

DGROUP          group   _NULL, _DATA, _BSS, XIB, XI, XIE, STACK, BC_SAB

.code
;;::::::::::::::
;; start ()
start           proc

                mov     bx, es                  ;; the PSP, before ds leaves it
                mov     ax, DGROUP
                mov     ds, ax

                mov     dx, ss                  ;; ss -> DGROUP, same stack
                sub     dx, ax
                shl     dx, 4
                cli
                mov     ss, ax
                add     sp, dx
                sti

                mov     llrm_os_psp, bx

                ;; PSP:2 is the first paragraph past the program's memory
                mov     es, bx
                mov     cx, es:[2]
                sub     cx, ax
                mov     dx, LAST_WORD
                cmp     cx, PAGES_64K
                jae     short @F
                shl     cx, 4
                lea     dx, [ecx-2]
                and     dl, 0FEh
@@:             mov     qb_asizds, dx

                ;; the linker stores no BSS: zero it, up to the stack
                mov     ax, ds
                mov     es, ax
                mov     di, O DGROUP:_BSS
                mov     cx, O DGROUP:STACK
                sub     cx, di
                mov     dx, cx
                xor     eax, eax
                cld
                shr     cx, 2
                rep     stosd
                mov     cx, dx
                and     cx, 3
                rep     stosb

                invoke  qb_start

                ;; enter the module: BC_SA's far address, code at 30h
                mov     ax, W bc_sa+2
                mov     qb_module_segment, ax
                push    cs
                push    O @@hang
                push    ax
                push    HEADER_CODE
                xor     dx, dx
                ret

@@hang:         jmp     short @@hang            ;; a module ends through B$CENP/B$CEND
start           endp
                end     start
