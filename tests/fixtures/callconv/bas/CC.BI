' Shared by every module. Callees and callers keep what arrived and what
' came back in PROBE.ASM's slots (put*); the harness reads them (getB).
type Rec
    a as integer
    b as long
    c as double
    s as string * 4
end type

' PROBE.ASM
declare sub trash ()
declare sub arm ()
declare sub verify (byval k as integer)
declare sub armX ()
declare sub verifyX (byval k as integer)
declare sub badRegs ()
declare function probeMask% (byval k as integer)
declare sub putI (byval k as integer, byval v as integer)
declare sub putL (byval k as integer, byval v as long)
declare sub putF (byval k as integer, byval v as single)
declare sub putD (byval k as integer, byval v as double)
declare sub putC (byval k as integer, byval i as integer, byval c as integer)
declare function getB% (byval k as integer, byval i as integer)
declare sub recVal (byval a as integer, byval b as long, byval c as single, byval d as double)
declare sub recSeg (seg a as integer, seg b as double)
declare sub recCdl cdecl (byval a as integer, byval b as long, byval c as double)
declare function aInt% ()
declare function aLng& ()
declare function aSng! ()
declare function aDbl# ()
declare sub keepDesc (s as string)
declare function aStr$ ()
declare function xInt% cdecl ()
declare function xLng& cdecl ()
declare function xSng! cdecl ()
declare function xDbl# cdecl ()

' CE.BAS
declare sub pInt (h as integer, v as integer, t as integer)
declare sub pLng (h as integer, v as long, t as integer)
declare sub pSng (h as integer, v as single, t as integer)
declare sub pDbl (h as integer, v as double, t as integer)
declare sub pStr (h as integer, v as string, t as integer)
declare sub pFix (h as integer, v as string, t as integer)
declare sub pRec (h as integer, v as Rec, t as integer)
declare sub pArrI (h as integer, v() as integer, t as integer)
declare sub pArrS (h as integer, v() as string, t as integer)
declare sub pMix (a as integer, b as long, c as single, d as double, e as string, f as integer)
declare sub sVoid ()
declare function rInt% (h as integer)
declare function rLng& (h as integer)
declare function rSng! (h as integer)
declare function rDbl# (h as integer)
declare sub ceStr (k as integer, s as string)

' CES.BAS
declare function rStr$ (h as integer)

' CR.BAS, CRS.BAS
declare sub callAll ()
declare sub crStr (k as integer, s as string)
declare sub callStrs ()

'$include: 'CUR.BI'
'$include: 'V71.BI'
