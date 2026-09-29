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
	?debug	S "pfcallee.c"
	?debug	C E992603D5D0A706663616C6C65652E63
	?debug	C E992603D5D0470662E68
	?debug	C E992603D5D0863616C6C65652E68
	?debug	C E992603D5D0763617365732E68
PFCALLEE_TEXT	segment byte public use16 'CODE'
PFCALLEE_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:PFCALLEE_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
PFCALLEE_TEXT	segment byte public use16 'CODE'
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_SC	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_sc,ax
	mov	al,byte ptr [bp+8]
	mov	byte ptr DGROUP:_gv_sc,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_sc,ax
	pop	bp
	ret	6
P_SC	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_UC	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_uc,ax
	mov	al,byte ptr [bp+8]
	mov	byte ptr DGROUP:_gv_uc,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_uc,ax
	pop	bp
	ret	6
P_UC	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_CH	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_ch,ax
	mov	al,byte ptr [bp+8]
	mov	byte ptr DGROUP:_gv_ch,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_ch,ax
	pop	bp
	ret	6
P_CH	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_I	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_i,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_i,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_i,ax
	pop	bp
	ret	6
P_I	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_U	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_u,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_u,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_u,ax
	pop	bp
	ret	6
P_U	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_L	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_l,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_l,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_l,ax
	pop	bp
	ret	8
P_L	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_UL	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_ul,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_ul,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_ul,ax
	pop	bp
	ret	8
P_UL	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_NP	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_np,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_np,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_np,ax
	pop	bp
	ret	6
P_NP	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_FP	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_fp,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_fp,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_fp,ax
	pop	bp
	ret	8
P_FP	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_HP	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_hp,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_hp,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_hp,ax
	pop	bp
	ret	8
P_HP	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_F	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_f,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_f,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_f,ax
	pop	bp
	ret	8
P_F	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_D	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+16]
	mov	word ptr DGROUP:_gh_d,ax
	fld	qword ptr [bp+8]
	fstp	qword ptr DGROUP:_gv_d
	fwait	
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_d,ax
	pop	bp
	ret	12
P_D	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
P_LD	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+18]
	mov	word ptr DGROUP:_gh_ld,ax
	fld	tbyte ptr [bp+8]
	fstp	tbyte ptr DGROUP:_gv_ld
	fwait	
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_ld,ax
	pop	bp
	ret	14
P_LD	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_SC	proc	far
	call	far ptr _trash
	mov	al,byte ptr DGROUP:_in_sc
	ret	
R_SC	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_UC	proc	far
	call	far ptr _trash
	mov	al,byte ptr DGROUP:_in_uc
	ret	
R_UC	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_CH	proc	far
	call	far ptr _trash
	mov	al,byte ptr DGROUP:_in_ch
	ret	
R_CH	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_I	proc	far
	call	far ptr _trash
	mov	ax,word ptr DGROUP:_in_i
	ret	
R_I	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_U	proc	far
	call	far ptr _trash
	mov	ax,word ptr DGROUP:_in_u
	ret	
R_U	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_L	proc	far
	call	far ptr _trash
	mov	eax,dword ptr DGROUP:_in_l
	shld	edx,eax,16
	ret	
R_L	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_UL	proc	far
	call	far ptr _trash
	mov	eax,dword ptr DGROUP:_in_ul
	shld	edx,eax,16
	ret	
R_UL	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_NP	proc	far
	call	far ptr _trash
	mov	ax,word ptr DGROUP:_in_np
	ret	
R_NP	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_FP	proc	far
	call	far ptr _trash
	mov	dx,word ptr DGROUP:_in_fp+2
	mov	ax,word ptr DGROUP:_in_fp
	ret	
R_FP	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_HP	proc	far
	call	far ptr _trash
	mov	dx,word ptr DGROUP:_in_hp+2
	mov	ax,word ptr DGROUP:_in_hp
	ret	
R_HP	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_F	proc	far
	call	far ptr _trash
	fld	dword ptr DGROUP:_in_f
	ret	
R_F	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_D	proc	far
	call	far ptr _trash
	fld	qword ptr DGROUP:_in_d
	ret	
R_D	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
R_LD	proc	far
	call	far ptr _trash
	fld	tbyte ptr DGROUP:_in_ld
	ret	
R_LD	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
W_SC	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	al,byte ptr [bp+8]
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_gw_sc,eax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_sc,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_sc,ax
	pop	bp
	ret	6
W_SC	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
W_UC	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	al,byte ptr [bp+8]
	mov	ah,0
	movsx	eax,ax
	mov	dword ptr DGROUP:_gw_uc,eax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_uc,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_uc,ax
	pop	bp
	ret	6
W_UC	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
W_CH	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	al,byte ptr [bp+8]
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_gw_ch,eax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_ch,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_ch,ax
	pop	bp
	ret	6
W_CH	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
W_I	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	movsx	eax,word ptr [bp+8]
	mov	dword ptr DGROUP:_gw_i,eax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_i,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_i,ax
	pop	bp
	ret	6
W_I	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:PFCALLEE_TEXT
W_U	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	movzx	eax,word ptr [bp+8]
	mov	dword ptr DGROUP:_gw_u,eax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_u,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_u,ax
	pop	bp
	ret	6
W_U	endp
   ;	
   ;	void CONV DIST r_void(void) { trash(); }
   ;	
	assume	cs:PFCALLEE_TEXT
R_VOID	proc	far
	call	far ptr _trash
	ret	
R_VOID	endp
   ;	
   ;	void CONV DIST p_mixed(signed char a, long b, double c, int d, float e, char far *f, long double h) {
   ;	
	assume	cs:PFCALLEE_TEXT
P_MIXED	proc	far
	push	bp
	mov	bp,sp
   ;	
   ;	    trash(); gm_a = a; gm_b = b; gm_c = c; gm_d = d; gm_e = e; gm_f = f; gm_h = h;
   ;	
	call	far ptr _trash
	mov	al,byte ptr [bp+38]
	mov	byte ptr DGROUP:_gm_a,al
	mov	eax,dword ptr [bp+34]
	mov	dword ptr DGROUP:_gm_b,eax
	fld	qword ptr [bp+26]
	fstp	qword ptr DGROUP:_gm_c
	fwait	
	mov	ax,word ptr [bp+24]
	mov	word ptr DGROUP:_gm_d,ax
	mov	eax,dword ptr [bp+20]
	mov	dword ptr DGROUP:_gm_e,eax
	mov	eax,dword ptr [bp+16]
	mov	dword ptr DGROUP:_gm_f,eax
	fld	tbyte ptr [bp+6]
	fstp	tbyte ptr DGROUP:_gm_h
	fwait	
   ;	
   ;	}
   ;	
	pop	bp
	ret	34
P_MIXED	endp
PFCALLEE_TEXT	ends
_BSS	segment word public use16 'BSS'
_gm_h	label	qword
	db	10 dup (?)
_gm_f	label	dword
	db	4 dup (?)
_gm_e	label	dword
	db	4 dup (?)
_gm_d	label	word
	db	2 dup (?)
_gm_c	label	qword
	db	8 dup (?)
_gm_b	label	word
	db	4 dup (?)
_gm_a	label	byte
	db	1 dup (?)
_gw_u	label	word
	db	4 dup (?)
_gw_i	label	word
	db	4 dup (?)
_gw_ch	label	word
	db	4 dup (?)
_gw_uc	label	word
	db	4 dup (?)
_gw_sc	label	word
	db	4 dup (?)
_gt_ld	label	word
	db	2 dup (?)
_gh_ld	label	word
	db	2 dup (?)
_gv_ld	label	qword
	db	10 dup (?)
_gt_d	label	word
	db	2 dup (?)
_gh_d	label	word
	db	2 dup (?)
_gv_d	label	qword
	db	8 dup (?)
_gt_f	label	word
	db	2 dup (?)
_gh_f	label	word
	db	2 dup (?)
_gv_f	label	dword
	db	4 dup (?)
_gt_hp	label	word
	db	2 dup (?)
_gh_hp	label	word
	db	2 dup (?)
_gv_hp	label	dword
	db	4 dup (?)
_gt_fp	label	word
	db	2 dup (?)
_gh_fp	label	word
	db	2 dup (?)
_gv_fp	label	dword
	db	4 dup (?)
_gt_np	label	word
	db	2 dup (?)
_gh_np	label	word
	db	2 dup (?)
_gv_np	label	word
	db	2 dup (?)
_gt_ul	label	word
	db	2 dup (?)
_gh_ul	label	word
	db	2 dup (?)
_gv_ul	label	word
	db	4 dup (?)
_gt_l	label	word
	db	2 dup (?)
_gh_l	label	word
	db	2 dup (?)
_gv_l	label	word
	db	4 dup (?)
_gt_u	label	word
	db	2 dup (?)
_gh_u	label	word
	db	2 dup (?)
_gv_u	label	word
	db	2 dup (?)
_gt_i	label	word
	db	2 dup (?)
_gh_i	label	word
	db	2 dup (?)
_gv_i	label	word
	db	2 dup (?)
_gt_ch	label	word
	db	2 dup (?)
_gh_ch	label	word
	db	2 dup (?)
_gv_ch	label	byte
	db	1 dup (?)
_gt_uc	label	word
	db	2 dup (?)
_gh_uc	label	word
	db	2 dup (?)
_gv_uc	label	byte
	db	1 dup (?)
_gt_sc	label	word
	db	2 dup (?)
_gh_sc	label	word
	db	2 dup (?)
_gv_sc	label	byte
	db	1 dup (?)
	?debug	C E9
	?debug	C FA00000000
_BSS	ends
_DATA	segment word public use16 'DATA'
s@	label	byte
_DATA	ends
PFCALLEE_TEXT	segment byte public use16 'CODE'
PFCALLEE_TEXT	ends
	public	P_MIXED
	public	_gm_h
	public	_gm_f
	public	_gm_e
	public	_gm_d
	public	_gm_c
	public	_gm_b
	public	_gm_a
	public	R_VOID
	public	W_U
	public	_gw_u
	public	W_I
	public	_gw_i
	public	W_CH
	public	_gw_ch
	public	W_UC
	public	_gw_uc
	public	W_SC
	public	_gw_sc
	public	R_LD
	public	R_D
	public	R_F
	public	R_HP
	public	R_FP
	public	R_NP
	public	R_UL
	public	R_L
	public	R_U
	public	R_I
	public	R_CH
	public	R_UC
	public	R_SC
	public	P_LD
	public	_gt_ld
	public	_gh_ld
	public	_gv_ld
	public	P_D
	public	_gt_d
	public	_gh_d
	public	_gv_d
	public	P_F
	public	_gt_f
	public	_gh_f
	public	_gv_f
	public	P_HP
	public	_gt_hp
	public	_gh_hp
	public	_gv_hp
	public	P_FP
	public	_gt_fp
	public	_gh_fp
	public	_gv_fp
	public	P_NP
	public	_gt_np
	public	_gh_np
	public	_gv_np
	public	P_UL
	public	_gt_ul
	public	_gh_ul
	public	_gv_ul
	public	P_L
	public	_gt_l
	public	_gh_l
	public	_gv_l
	public	P_U
	public	_gt_u
	public	_gh_u
	public	_gv_u
	public	P_I
	public	_gt_i
	public	_gh_i
	public	_gv_i
	public	P_CH
	public	_gt_ch
	public	_gh_ch
	public	_gv_ch
	public	P_UC
	public	_gt_uc
	public	_gh_uc
	public	_gv_uc
	public	P_SC
	public	_gt_sc
	public	_gh_sc
	public	_gv_sc
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
	extrn	_trash:far
_s@	equ	s@
	end
