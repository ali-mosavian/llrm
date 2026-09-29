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
	?debug	S "pncaller.c"
	?debug	C E9996A3D5D0A706E63616C6C65722E63
	?debug	C E9996A3D5D04706E2E68
	?debug	C E9996A3D5D0863616C6C65722E68
	?debug	C E9996A3D5D0763617365732E68
PNCALLER_TEXT	segment byte public use16 'CODE'
PNCALLER_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:PNCALLER_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
PNCALLER_TEXT	segment byte public use16 'CODE'
   ;	
   ;	void far call_scalars(void) {
   ;	
	assume	cs:PNCALLER_TEXT
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
	call	near ptr P_SC
	call	near ptr R_SC
	mov	byte ptr DGROUP:_out_sc,al
	push	4369
	mov	al,byte ptr DGROUP:_in_uc
	push	ax
	push	8738
	call	near ptr P_UC
	call	near ptr R_UC
	mov	byte ptr DGROUP:_out_uc,al
	push	4369
	mov	al,byte ptr DGROUP:_in_ch
	push	ax
	push	8738
	call	near ptr P_CH
	call	near ptr R_CH
	mov	byte ptr DGROUP:_out_ch,al
	push	4369
	push	word ptr DGROUP:_in_i
	push	8738
	call	near ptr P_I
	call	near ptr R_I
	mov	word ptr DGROUP:_out_i,ax
	push	4369
	push	word ptr DGROUP:_in_u
	push	8738
	call	near ptr P_U
	call	near ptr R_U
	mov	word ptr DGROUP:_out_u,ax
	push	4369
	push	dword ptr DGROUP:_in_l
	push	8738
	call	near ptr P_L
	call	near ptr R_L
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_l,eax
	push	4369
	push	dword ptr DGROUP:_in_ul
	push	8738
	call	near ptr P_UL
	call	near ptr R_UL
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_ul,eax
	push	4369
	push	word ptr DGROUP:_in_np
	push	8738
	call	near ptr P_NP
	call	near ptr R_NP
	mov	word ptr DGROUP:_out_np,ax
	push	4369
	push	dword ptr DGROUP:_in_fp
	push	8738
	call	near ptr P_FP
	call	near ptr R_FP
	mov	word ptr DGROUP:_out_fp+2,dx
	mov	word ptr DGROUP:_out_fp,ax
	push	4369
	push	dword ptr DGROUP:_in_hp
	push	8738
	call	near ptr P_HP
	call	near ptr R_HP
	mov	word ptr DGROUP:_out_hp+2,dx
	mov	word ptr DGROUP:_out_hp,ax
	push	4369
	fld	dword ptr DGROUP:_in_f
	sub	sp,4
	fstp	dword ptr [bp-6]
	push	8738
	fwait	
	call	near ptr P_F
	call	near ptr R_F
	fstp	dword ptr DGROUP:_out_f
	push	4369
	fld	qword ptr DGROUP:_in_d
	sub	sp,8
	fstp	qword ptr [bp-10]
	push	8738
	fwait	
	call	near ptr P_D
	call	near ptr R_D
	fstp	qword ptr DGROUP:_out_d
	push	4369
	fld	tbyte ptr DGROUP:_in_ld
	sub	sp,10
	fstp	tbyte ptr [bp-12]
	push	8738
	fwait	
	call	near ptr P_LD
	call	near ptr R_LD
	fstp	tbyte ptr DGROUP:_out_ld
   ;	
   ;	    NARROW(CALL_WIDE)
   ;	
	push	4369
	fwait	
	mov	al,byte ptr DGROUP:_in_sc
	push	ax
	push	8738
	call	near ptr W_SC
	call	near ptr R_SC
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_sc,eax
	push	4369
	mov	al,byte ptr DGROUP:_in_uc
	push	ax
	push	8738
	call	near ptr W_UC
	call	near ptr R_UC
	mov	ah,0
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_uc,eax
	push	4369
	mov	al,byte ptr DGROUP:_in_ch
	push	ax
	push	8738
	call	near ptr W_CH
	call	near ptr R_CH
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_ch,eax
	push	4369
	push	word ptr DGROUP:_in_i
	push	8738
	call	near ptr W_I
	call	near ptr R_I
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_i,eax
	push	4369
	push	word ptr DGROUP:_in_u
	push	8738
	call	near ptr W_U
	call	near ptr R_U
	movzx	eax,ax
	mov	dword ptr DGROUP:_ow_u,eax
   ;	
   ;	    ARM(); r_void(); VERIFY("r_void");
   ;	
	call	near ptr R_VOID
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
	call	near ptr P_MIXED
   ;	
   ;	#ifdef VARIADIC
   ;	#define VARG(n, T, P) , in_##n
   ;	    ARM(); p_variadic(0x1111 PROMOTED(VARG)); VERIFY("p_variadic");
   ;	#undef VARG
   ;	#endif
   ;	#ifdef INTERRUPTS
   ;	    /* Last: after calling an interrupt function BCC counts its pushf as still
   ;	       on the stack, and every BP-relative temporary it makes is 2 bytes off. */
   ;	    ARM(); i_plain(); VERIFY("i_plain");
   ;	#endif
   ;	}
   ;	
	pop	bp
	ret	
_call_scalars	endp
PNCALLER_TEXT	ends
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
PNCALLER_TEXT	segment byte public use16 'CODE'
PNCALLER_TEXT	ends
	public	_call_scalars
	extrn	P_MIXED:near
	extrn	R_VOID:near
	public	_ow_u
	extrn	W_U:near
	public	_ow_i
	extrn	W_I:near
	public	_ow_ch
	extrn	W_CH:near
	public	_ow_uc
	extrn	W_UC:near
	public	_ow_sc
	extrn	W_SC:near
	public	_out_ld
	extrn	R_LD:near
	extrn	P_LD:near
	public	_out_d
	extrn	R_D:near
	extrn	P_D:near
	public	_out_f
	extrn	R_F:near
	extrn	P_F:near
	public	_out_hp
	extrn	R_HP:near
	extrn	P_HP:near
	public	_out_fp
	extrn	R_FP:near
	extrn	P_FP:near
	public	_out_np
	extrn	R_NP:near
	extrn	P_NP:near
	public	_out_ul
	extrn	R_UL:near
	extrn	P_UL:near
	public	_out_l
	extrn	R_L:near
	extrn	P_L:near
	public	_out_u
	extrn	R_U:near
	extrn	P_U:near
	public	_out_i
	extrn	R_I:near
	extrn	P_I:near
	public	_out_ch
	extrn	R_CH:near
	extrn	P_CH:near
	public	_out_uc
	extrn	R_UC:near
	extrn	P_UC:near
	public	_out_sc
	extrn	R_SC:near
	extrn	P_SC:near
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