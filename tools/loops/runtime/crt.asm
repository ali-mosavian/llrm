; C's start-up for the loop corpus: a 16K stack in DGROUP (C's near
; pointers to locals need SS = DS), then _main. The small STACK segment is
; only for the instructions before the switch: without one, DOS starts the
; program with SS:SP inside its code, and an interrupt there writes into it.
;
; DOS does not clear memory past the image, so this zeroes what the EXE does not store: the near
; uninitialised data (_BSS, from BSS_FIRST here to zend.asm's BSS_LAST, linked last) and the far
; (class FAR_BSS, from FBSS_FIRST to zend.asm's FBSS_LAST). No .dosseg: its order puts every
; segment outside DGROUP before it, and a far one the image does not store would have its zeros
; stored. The order is the linker's, by the first appearance of each class here.
.model medium
.386

extrn _main:far
extrn BSS_LAST:byte
extrn FBSS_LAST:byte

.code
.data
        db 16384 dup (?)
stack_top label byte
.data?
BSS_FIRST label byte
.stack 256
FARDATA_ORDER segment para public 'FAR_DATA'
FARDATA_ORDER ends
FBSS_BEG segment para public 'FAR_BSS'
public FBSS_FIRST
FBSS_FIRST label byte
        db 16 dup (?)
FBSS_BEG ends

.code
start:
    mov ax, @data
    mov ds, ax
    cli
    mov ss, ax
    mov sp, offset stack_top
    sti
ifndef NOZERO                      ; the tests build one that does not, to see dirty memory
    mov es, ax
    mov di, offset DGROUP:BSS_FIRST
    mov cx, offset DGROUP:BSS_LAST
    sub cx, di
    xor al, al
    cld
    rep stosb
    ; A label's segment is its frame, which the linker shares among the far segments: its
    ; paragraph is the frame's plus the offset's sixteenths.
    mov bx, offset FBSS_FIRST
    shr bx, 4
    add bx, seg FBSS_FIRST
    mov dx, offset FBSS_LAST
    shr dx, 4
    add dx, seg FBSS_LAST
far_clear:
    cmp bx, dx
    jae far_cleared
    mov ax, dx
    sub ax, bx
    cmp ax, 1000h                  ; at most 64K a pass
    jbe far_pass
    mov ax, 1000h
far_pass:
    mov es, bx
    add bx, ax
    mov cx, ax
    shl cx, 3                      ; words: eight to a paragraph
    xor di, di
    xor ax, ax
    rep stosw
    jmp far_clear
far_cleared:
endif
    fninit
    call far ptr _main
    mov ax, 4c00h
    int 21h
end start
