	.386p
	.387
	ifndef	??version
?debug	macro
	endm
publicdll macro	name
	public	name
	endm
$comm	macro	name,dist,size,count
	comm	dist name:BYTE:count*size
	endm
	else
$comm	macro	name,dist,size,count
	comm	dist name[size]:BYTE:count
	endm
	endif
	?debug	V 300h
	?debug	S "pfcaller.c"
	?debug	C E992603D5D0A706663616C6C65722E63
	?debug	C E992603D5D0470662E68
	?debug	C E992603D5D0863616C6C65722E68
	?debug	C E992603D5D0763617365732E68
PFCALLER_TEXT	segment byte public use16 'CODE'
PFCALLER_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:PFCALLER_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
PFCALLER_TEXT	segment byte public use16 'CODE'
   ;	
   ;	void far call_scalars(void) {
   ;	
	assume	cs:PFCALLER_TEXT
_call_scalars	proc	far
	push	bp
	mov	bp,sp
   ;	
   ;	    SCALARS(CALL)
   ;	
	push	4369
	mov	al,byte ptr DGROUP:_in_sc
	push	ax
	push	8738
	call	far ptr P_SC
	call	far ptr R_SC
	mov	byte ptr DGROUP:_out_sc,al
	push	4369
	mov	al,byte ptr DGROUP:_in_uc
	push	ax
	push	8738
	call	far ptr P_UC
	call	far ptr R_UC
	mov	byte ptr DGROUP:_out_uc,al
	push	4369
	mov	al,byte ptr DGROUP:_in_ch
	push	ax
	push	8738
	call	far ptr P_CH
	call	far ptr R_CH
	mov	byte ptr DGROUP:_out_ch,al
	push	4369
	push	word ptr DGROUP:_in_i
	push	8738
	call	far ptr P_I
	call	far ptr R_I
	mov	word ptr DGROUP:_out_i,ax
	push	4369
	push	word ptr DGROUP:_in_u
	push	8738
	call	far ptr P_U
	call	far ptr R_U
	mov	word ptr DGROUP:_out_u,ax
	push	4369
	push	dword ptr DGROUP:_in_l
	push	8738
	call	far ptr P_L
	call	far ptr R_L
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_l,eax
	push	4369
	push	dword ptr DGROUP:_in_ul
	push	8738
	call	far ptr P_UL
	call	far ptr R_UL
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_ul,eax
	push	4369
	push	word ptr DGROUP:_in_np
	push	8738
	call	far ptr P_NP
	call	far ptr R_NP
	mov	word ptr DGROUP:_out_np,ax
	push	4369
	push	dword ptr DGROUP:_in_fp
	push	8738
	call	far ptr P_FP
	call	far ptr R_FP
	mov	word ptr DGROUP:_out_fp+2,dx
	mov	word ptr DGROUP:_out_fp,ax
	push	4369
	push	dword ptr DGROUP:_in_hp
	push	8738
	call	far ptr P_HP
	call	far ptr R_HP
	mov	word ptr DGROUP:_out_hp+2,dx
	mov	word ptr DGROUP:_out_hp,ax
	push	4369
	fld	dword ptr DGROUP:_in_f
	sub	sp,4
	fstp	dword ptr [bp-6]
	push	8738
	fwait	
	call	far ptr P_F
	call	far ptr R_F
	fstp	dword ptr DGROUP:_out_f
	push	4369
	fld	qword ptr DGROUP:_in_d
	sub	sp,8
	fstp	qword ptr [bp-10]
	push	8738
	fwait	
	call	far ptr P_D
	call	far ptr R_D
	fstp	qword ptr DGROUP:_out_d
	push	4369
	fld	tbyte ptr DGROUP:_in_ld
	sub	sp,10
	fstp	tbyte ptr [bp-12]
	push	8738
	fwait	
	call	far ptr P_LD
	call	far ptr R_LD
	fstp	tbyte ptr DGROUP:_out_ld
   ;	
   ;	    NARROW(CALL_WIDE)
   ;	
	push	4369
	fwait	
	mov	al,byte ptr DGROUP:_in_sc
	push	ax
	push	8738
	call	far ptr W_SC
	call	far ptr R_SC
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_sc,eax
	push	4369
	mov	al,byte ptr DGROUP:_in_uc
	push	ax
	push	8738
	call	far ptr W_UC
	call	far ptr R_UC
	mov	ah,0
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_uc,eax
	push	4369
	mov	al,byte ptr DGROUP:_in_ch
	push	ax
	push	8738
	call	far ptr W_CH
	call	far ptr R_CH
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_ch,eax
	push	4369
	push	word ptr DGROUP:_in_i
	push	8738
	call	far ptr W_I
	call	far ptr R_I
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_i,eax
	push	4369
	push	word ptr DGROUP:_in_u
	push	8738
	call	far ptr W_U
	call	far ptr R_U
	movzx	eax,ax
	mov	dword ptr DGROUP:_ow_u,eax
   ;	
   ;	    ARM(); r_void(); VERIFY("r_void");
   ;	
	call	far ptr R_VOID
   ;	
   ;	    ARM(); p_mixed(in_sc, in_l, in_d, in_i, in_f, in_fp, in_ld); VERIFY("p_mixed");
   ;	
	mov	al,byte ptr DGROUP:_in_sc
	push	ax
	push	dword ptr DGROUP:_in_l
	fld	qword ptr DGROUP:_in_d
	sub	sp,8
	fstp	qword ptr [bp-14]
	fwait	
	push	word ptr DGROUP:_in_i
	fld	dword ptr DGROUP:_in_f
	sub	sp,4
	fstp	dword ptr [bp-20]
	fwait	
	push	dword ptr DGROUP:_in_fp
	fld	tbyte ptr DGROUP:_in_ld
	sub	sp,10
	fstp	tbyte ptr [bp-34]
	fwait	
	call	far ptr P_MIXED
   ;	
   ;	#ifdef VARIADIC
   ;	#define VARG(n, T, P) , in_##n
   ;	    ARM(); p_variadic(0x1111 PROMOTED(VARG)); VERIFY("p_variadic");
   ;	#undef VARG
   ;	#endif
   ;	}
   ;	
	pop	bp
	ret	
_call_scalars	endp
PFCALLER_TEXT	ends
_BSS	segment word public use16 'BSS'
_ow_u	label	word
	db	4 dup (?)
_ow_i	label	word
	db	4 dup (?)
_ow_ch	label	word
	db	4 dup (?)
_ow_uc	label	word
	db	4 dup (?)
_ow_sc	label	word
	db	4 dup (?)
_out_ld	label	qword
	db	10 dup (?)
_out_d	label	qword
	db	8 dup (?)
_out_f	label	dword
	db	4 dup (?)
_out_hp	label	dword
	db	4 dup (?)
_out_fp	label	dword
	db	4 dup (?)
_out_np	label	word
	db	2 dup (?)
_out_ul	label	word
	db	4 dup (?)
_out_l	label	word
	db	4 dup (?)
_out_u	label	word
	db	2 dup (?)
_out_i	label	word
	db	2 dup (?)
_out_ch	label	byte
	db	1 dup (?)
_out_uc	label	byte
	db	1 dup (?)
_out_sc	label	byte
	db	1 dup (?)
	?debug	C E9
	?debug	C FA00000000
_BSS	ends
_DATA	segment word public use16 'DATA'
s@	label	byte
_DATA	ends
PFCALLER_TEXT	segment byte public use16 'CODE'
PFCALLER_TEXT	ends
	public	_call_scalars
	extrn	P_MIXED:far
	extrn	R_VOID:far
	public	_ow_u
	extrn	W_U:far
	public	_ow_i
	extrn	W_I:far
	public	_ow_ch
	extrn	W_CH:far
	public	_ow_uc
	extrn	W_UC:far
	public	_ow_sc
	extrn	W_SC:far
	public	_out_ld
	extrn	R_LD:far
	extrn	P_LD:far
	public	_out_d
	extrn	R_D:far
	extrn	P_D:far
	public	_out_f
	extrn	R_F:far
	extrn	P_F:far
	public	_out_hp
	extrn	R_HP:far
	extrn	P_HP:far
	public	_out_fp
	extrn	R_FP:far
	extrn	P_FP:far
	public	_out_np
	extrn	R_NP:far
	extrn	P_NP:far
	public	_out_ul
	extrn	R_UL:far
	extrn	P_UL:far
	public	_out_l
	extrn	R_L:far
	extrn	P_L:far
	public	_out_u
	extrn	R_U:far
	extrn	P_U:far
	public	_out_i
	extrn	R_I:far
	extrn	P_I:far
	public	_out_ch
	extrn	R_CH:far
	extrn	P_CH:far
	public	_out_uc
	extrn	R_UC:far
	extrn	P_UC:far
	public	_out_sc
	extrn	R_SC:far
	extrn	P_SC:far
	extrn	_in_ld:qword
	extrn	_in_d:qword
	extrn	_in_f:dword
	extrn	_in_hp:dword
	extrn	_in_fp:dword
	extrn	_in_np:word
	extrn	_in_ul:word
	extrn	_in_l:word
	extrn	_in_u:word
	extrn	_in_i:word
	extrn	_in_ch:byte
	extrn	_in_uc:byte
	extrn	_in_sc:byte
_s@	equ	s@
	end
