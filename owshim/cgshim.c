/*
 * Open Watcom's code generator, replaced by a recorder.
 *
 * wcc's front end drives the cg API directly. Linked against this instead
 * of cgi86.lib, every call it makes becomes one line of a stream qbopt reads
 * as HIR (qbopt/cfront/stream.py). Handles are small integers; nothing here
 * generates code.
 *
 * A call this does not implement is a link error, and one it refuses is an
 * UNSUPPORTED record and a failed compile -- never a guess.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <stdint.h>
#include "watcom.h"
#include "cgstd.h"
#include "cg.h"
#include "cgdefs.h"
#include "cgswitch.h"
#include "cgaux.h"
#include "cgcli.h"
#include "cgmisc.h"
#include "cgprotos.h"
#include "feprotos.h"

static FILE *Out;
static unsigned Next = 1;
static cg_target_switches Target;
static segment_id CurrentSeg;

#define HANDLE( type )  ( (type)(uintptr_t)Next++ )
#define ID( h )         ( (unsigned)(uintptr_t)(h) )

static const char *OpNames[] = {
#define PICK( e, i, d1, d2, ot, pnum, attr ) [e] = #e,
#include "cgops.h"
#undef PICK
};

static void emit( const char *fmt, ... )
{
    va_list args;

    va_start( args, fmt );
    vfprintf( Out, fmt, args );
    va_end( args );
    fputc( '\n', Out );
}

static void refuse( const char *what )
{
    emit( "UNSUPPORTED %s", what );
    fflush( Out );
    fprintf( stderr, "cgshim: %s is not supported\n", what );
    exit( 1 );
}

static const char *op( cg_op o )
{
    return( ( o < MAX_OP && OpNames[o] != NULL ) ? OpNames[o] : "O_?" );
}

static const char *type( cg_type t )
{
    static char buf[16];

    switch( t ) {
    case TY_UINT_1: return( "TY_UINT_1" );
    case TY_INT_1: return( "TY_INT_1" );
    case TY_UINT_2: return( "TY_UINT_2" );
    case TY_INT_2: return( "TY_INT_2" );
    case TY_UINT_4: return( "TY_UINT_4" );
    case TY_INT_4: return( "TY_INT_4" );
    case TY_UINT_8: return( "TY_UINT_8" );
    case TY_INT_8: return( "TY_INT_8" );
    case TY_LONG_POINTER: return( "TY_LONG_POINTER" );
    case TY_HUGE_POINTER: return( "TY_HUGE_POINTER" );
    case TY_NEAR_POINTER: return( "TY_NEAR_POINTER" );
    case TY_LONG_CODE_PTR: return( "TY_LONG_CODE_PTR" );
    case TY_NEAR_CODE_PTR: return( "TY_NEAR_CODE_PTR" );
    case TY_SINGLE: return( "TY_SINGLE" );
    case TY_DOUBLE: return( "TY_DOUBLE" );
    case TY_LONG_DOUBLE: return( "TY_LONG_DOUBLE" );
    case TY_UNKNOWN: return( "TY_UNKNOWN" );
    case TY_DEFAULT: return( "TY_DEFAULT" );
    case TY_INTEGER: return( "TY_INTEGER" );
    case TY_UNSIGNED: return( "TY_UNSIGNED" );
    case TY_POINTER: return( "TY_POINTER" );
    case TY_CODE_PTR: return( "TY_CODE_PTR" );
    case TY_BOOLEAN: return( "TY_BOOLEAN" );
    case TY_PROC_PARM: return( "TY_PROC_PARM" );
    default:
        snprintf( buf, sizeof( buf ), "T%u", (unsigned)t );
        return( buf );
    }
}

static void quoted( char *buf, size_t size, const char *s )
{
    size_t n = 0;

    buf[n++] = '"';
    for( ; s != NULL && *s != '\0' && n + 5 < size; ++s ) {
        unsigned char c = (unsigned char)*s;
        if( c == '"' || c == '\\' || c < 0x20 || c >= 0x7f ) {
            n += snprintf( buf + n, size - n, "\\x%02x", c );
        } else {
            buf[n++] = (char)c;
        }
    }
    buf[n++] = '"';
    buf[n] = '\0';
}

/* User types, as BEDefType declared them: the front end lays out structs
 * from BETypeLength, so these have to be the real numbers. */
#define MAX_TYPES 4096
static struct { cg_type t; uint length; uint align; cg_type alias; } Types[MAX_TYPES];
static unsigned TypeCount;

static unsigned_32 length( cg_type t )
{
    unsigned i;

    switch( t ) {
    case TY_UINT_1: case TY_INT_1: return( 1 );
    case TY_UINT_2: case TY_INT_2: return( 2 );
    case TY_UINT_4: case TY_INT_4: return( 4 );
    case TY_UINT_8: case TY_INT_8: return( 8 );
    case TY_NEAR_POINTER: case TY_NEAR_CODE_PTR: return( 2 );
    case TY_LONG_POINTER: case TY_HUGE_POINTER: case TY_LONG_CODE_PTR: return( 4 );
    case TY_SINGLE: return( 4 );
    case TY_DOUBLE: return( 8 );
    case TY_LONG_DOUBLE: return( 10 );
    case TY_BOOLEAN: case TY_DEFAULT: return( 0 );
    case TY_PROC_PARM: return( 4 );
    case TY_INTEGER: case TY_UNSIGNED: return( 2 );
    case TY_POINTER: return( ( Target & CGSW_X86_BIG_DATA ) ? 4 : 2 );
    case TY_CODE_PTR: return( ( Target & CGSW_X86_BIG_CODE ) ? 4 : 2 );
    default:
        for( i = 0; i < TypeCount; ++i ) {
            if( Types[i].t == t ) {
                return( Types[i].alias != TY_DEFAULT ? length( Types[i].alias ) : Types[i].length );
            }
        }
        refuse( "BETypeLength of an undeclared type" );
        return( 0 );
    }
}

/* Front-end symbols are real pointers; each gets an id and one SYM record
 * the first time anything names it. */
#define MAX_SYMS 65536
static cg_sym_handle Syms[MAX_SYMS];
static unsigned SymCount;

static void regs( char *buf, size_t size, const hw_reg_set *set )
{
    size_t n = 0;
    size_t i;
    const hw_reg_part *parts = (const hw_reg_part *)set;

    for( i = 0; i < sizeof( *set ) / sizeof( hw_reg_part ); ++i ) {
        n += snprintf( buf + n, size - n, "%s%x", i ? ":" : "", (unsigned)parts[i] );
    }
}

static unsigned sym( cg_sym_handle h );

/* An aux pragma's code -- inline assembly -- as the bytes the code generator
 * would lay down, a two-byte hole at each place it patches, and the patches:
 * the front end escapes them with FLOATING_FIXUP_BYTE (x86enc2.c reads the
 * same stream). An FPU patch mark matters only to an emulator and has no bytes. */
static void code_record( unsigned id, const byte_seq *code )
{
    char bytes[2 * 4096 + 1], fixes[4096];
    size_t nb = 0, nf = 0;
    unsigned at = 0;
    const byte *p = code->data;
    const byte *end = code->data + code->length;

    bytes[0] = fixes[0] = '\0';
    while( p < end && nb + 5 < sizeof( bytes ) ) {
        if( code->relocs && p[0] == FLOATING_FIXUP_BYTE ) {
            byte kind = p[1];
            if( kind == FIX_SYM_OFFSET || kind == FIX_SYM_SEGMENT || kind == FIX_SYM_RELOFF ) {
                BYTE_SEQ_SYM s;
                BYTE_SEQ_OFF off;

                p += 2;
                memcpy( &s, p, sizeof( s ) );
                p += sizeof( s );
                memcpy( &off, p, sizeof( off ) );
                p += sizeof( off );
                nf += (size_t)snprintf( fixes + nf, sizeof( fixes ) - nf, "%s%u:%s:y%u:%u", nf ? "," : "", at,
                          kind == FIX_SYM_OFFSET ? "offset" : kind == FIX_SYM_SEGMENT ? "segment" : "reloff",
                          sym( (cg_sym_handle)s ), (unsigned)off );
                nb += (size_t)snprintf( bytes + nb, sizeof( bytes ) - nb, "0000" );
                at += 2;
                continue;
            }
            if( kind != FLOATING_FIXUP_BYTE ) {
                p += 2;
                continue;
            }
            ++p;    /* an escaped FLOATING_FIXUP_BYTE: the byte itself follows */
        }
        nb += (size_t)snprintf( bytes + nb, sizeof( bytes ) - nb, "%02x", *p );
        ++p;
        ++at;
    }
    if( p < end ) {
        refuse( "inline code longer than the shim holds" );
    }
    emit( "CODE y%u bytes=%s fix=%s", id, bytes, nf ? fixes : "-" );
}

static int empty( const hw_reg_set *set )
{
    size_t i;
    const hw_reg_part *parts = (const hw_reg_part *)set;

    for( i = 0; i < sizeof( *set ) / sizeof( hw_reg_part ); ++i ) {
        if( parts[i] != 0 ) {
            return( 0 );
        }
    }
    return( 1 );
}

static unsigned sym( cg_sym_handle h )
{
    unsigned i;
    unsigned id;
    fe_attr attr;
    char name[512], base[512], pattern[64];

    if( h == NULL ) {
        return( 0 );
    }
    for( i = 0; i < SymCount; ++i ) {
        if( Syms[i] == h ) {
            return( i + 1 );
        }
    }
    if( SymCount == MAX_SYMS ) {
        refuse( "more symbols than the shim holds" );
    }
    Syms[SymCount++] = h;
    id = SymCount;
    attr = FEAttr( h );
    quoted( name, sizeof( name ), FEName( h ) );
    quoted( base, sizeof( base ), FEExtName( h, EXTN_BASENAME ) );
    quoted( pattern, sizeof( pattern ), FEExtName( h, EXTN_PATTERN ) );
    emit( "SYM y%u name=%s base=%s pattern=%s attr=0x%x seg=%d",
          id, name, base, pattern, (unsigned)attr, (int)FESegID( h ) );
    if( attr & FE_PROC ) {
        aux_handle aux = FEAuxInfo( h, FEINF_AUX_LOOKUP );
        hw_reg_set *parms = FEAuxInfo( aux, FEINF_PARM_REGS );
        hw_reg_set *ret = FEAuxInfo( aux, FEINF_RETURN_REG );
        char line[1024], one[64];
        size_t n;

        n = (size_t)snprintf( line, sizeof( line ), "CALLCONV y%u class=0x%x target=0x%x parms=[",
              id, (unsigned)(pointer_uint)FEAuxInfo( aux, FEINF_CALL_CLASS ),
              (unsigned)(pointer_uint)FEAuxInfo( aux, FEINF_CALL_CLASS_TARGET ) );
        for( i = 0; parms != NULL && !empty( &parms[i] ); ++i ) {
            regs( one, sizeof( one ), &parms[i] );
            n += (size_t)snprintf( line + n, sizeof( line ) - n, "%s%s", i ? "," : "", one );
        }
        regs( one, sizeof( one ), ret );
        snprintf( line + n, sizeof( line ) - n, "] ret=%s", ret != NULL ? one : "-" );
        emit( "%s", line );
        {
            byte_seq *code = FEAuxInfo( aux, FEINF_CALL_BYTES );
            if( code != NULL ) {
                code_record( id, code );
            }
        }
    }
    return( id );
}

static long long wide( const signed_64 *v )
{
    return( (long long)v->u._64[0] );
}

/* ---- back end ---- */

bool BELoad( const char *name )
{
    (void)name;
    return( true );
}

void BEUnload( void )
{
}

bool TBreak( void )
{
    return( false );
}

void CauseTBreak( void )
{
}

cg_init_info BEInit( cg_switches sw, cg_target_switches tsw, uint size, proc_revision rev )
{
    cg_init_info info;
    const char *path = getenv( "QBOPT_CG_STREAM" );

    Out = fopen( path != NULL ? path : "cg.stream", "w" );
    if( Out == NULL ) {
        fprintf( stderr, "cgshim: cannot write %s\n", path != NULL ? path : "cg.stream" );
        exit( 1 );
    }
    Target = tsw;
    emit( "INIT sw=0x%x target=0x%x size=%u rev=0x%x", (unsigned)sw, (unsigned)tsw, size, (unsigned)rev );
    info.revision = II_REVISION;
    info.target = II_TARG_8086;
    return( info );
}

void BEStart( void )
{
    emit( "START" );
}

void BEStop( void )
{
    emit( "STOP" );
}

void BEAbort( void )
{
    emit( "ABORT" );
}

void BEFini( void )
{
    emit( "FINI" );
    if( Out != NULL ) {
        fclose( Out );
        Out = NULL;
    }
}

segment_id BESetSeg( segment_id seg )
{
    segment_id old = CurrentSeg;

    CurrentSeg = seg;
    emit( "SETSEG %d", (int)seg );
    return( old );
}

void BEDefSeg( segment_id id, seg_attr attr, cchar_ptr name, uint align )
{
    char q[256];

    quoted( q, sizeof( q ), name );
    emit( "SEG %d attr=0x%x name=%s align=%u", (int)id, (unsigned)attr, q, align );
}

back_handle BENewBack( cg_sym_handle s )
{
    unsigned y = sym( s );
    back_handle b = HANDLE( back_handle );

    emit( "b%u BENewBack y%u", ID( b ), y );
    return( b );
}

void BEFiniBack( back_handle b )
{
    (void)b;
}

void BEFreeBack( back_handle b )
{
    (void)b;
}

void BEDefType( cg_type t, uint align, unsigned_32 size )
{
    if( TypeCount == MAX_TYPES ) {
        refuse( "more types than the shim holds" );
    }
    Types[TypeCount].t = t;
    Types[TypeCount].length = size;
    Types[TypeCount].align = align;
    Types[TypeCount].alias = TY_DEFAULT;
    ++TypeCount;
    emit( "TYPE %s size=%u align=%u", type( t ), (unsigned)size, align );
}

void BEAliasType( cg_type t, cg_type to )
{
    char a[16];

    if( TypeCount == MAX_TYPES ) {
        refuse( "more types than the shim holds" );
    }
    Types[TypeCount].t = t;
    Types[TypeCount].alias = to;
    ++TypeCount;
    snprintf( a, sizeof( a ), "%s", type( t ) );
    emit( "ALIAS %s %s", a, type( to ) );
}

unsigned_32 BETypeLength( cg_type t )
{
    return( length( t ) );
}

uint BETypeAlign( cg_type t )
{
    unsigned i;

    for( i = 0; i < TypeCount; ++i ) {
        if( Types[i].t == t ) {
            return( Types[i].align );
        }
    }
    return( 1 );
}

unsigned_32 BEUnrollCount( unsigned_32 count )
{
    return( count );
}

label_handle BENewLabel( void )
{
    label_handle l = HANDLE( label_handle );

    emit( "l%u BENewLabel", ID( l ) );
    return( l );
}

void BEFiniLabel( label_handle l )
{
    (void)l;
}

/* ---- trees ---- */

cg_name CGInteger( signed_32 v, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGInteger %ld %s", ID( n ), (long)v, type( t ) );
    return( n );
}

cg_name CGInt64( signed_64 v, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGInt64 %lld %s", ID( n ), wide( &v ), type( t ) );
    return( n );
}

cg_name CGFloat( cchar_ptr text, cg_type t )
{
    cg_name n = HANDLE( cg_name );
    char q[128];

    quoted( q, sizeof( q ), text );
    emit( "n%u CGFloat %s %s", ID( n ), q, type( t ) );
    return( n );
}

cg_name CGFEName( cg_sym_handle s, cg_type t )
{
    unsigned y = sym( s );
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGFEName y%u %s", ID( n ), y, type( t ) );
    return( n );
}

cg_name CGBackName( back_handle b, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGBackName b%u %s", ID( n ), ID( b ), type( t ) );
    return( n );
}

temp_handle CGTemp( cg_type t )
{
    temp_handle h = HANDLE( temp_handle );

    emit( "t%u CGTemp %s", ID( h ), type( t ) );
    return( h );
}

cg_name CGTempName( temp_handle h, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGTempName t%u %s", ID( n ), ID( h ), type( t ) );
    return( n );
}

cg_name CGUnary( cg_op o, cg_name a, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGUnary %s n%u %s", ID( n ), op( o ), ID( a ), type( t ) );
    return( n );
}

cg_name CGBinary( cg_op o, cg_name a, cg_name b, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGBinary %s n%u n%u %s", ID( n ), op( o ), ID( a ), ID( b ), type( t ) );
    return( n );
}

cg_name CGCompare( cg_op o, cg_name a, cg_name b, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGCompare %s n%u n%u %s", ID( n ), op( o ), ID( a ), ID( b ), type( t ) );
    return( n );
}

cg_name CGFlow( cg_op o, cg_name a, cg_name b )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGFlow %s n%u n%u", ID( n ), op( o ), ID( a ), ID( b ) );
    return( n );
}

cg_name CGChoose( cg_name c, cg_name a, cg_name b, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGChoose n%u n%u n%u %s", ID( n ), ID( c ), ID( a ), ID( b ), type( t ) );
    return( n );
}

cg_name CGAssign( cg_name dst, cg_name src, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGAssign n%u n%u %s", ID( n ), ID( dst ), ID( src ), type( t ) );
    return( n );
}

cg_name CGLVAssign( cg_name dst, cg_name src, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGLVAssign n%u n%u %s", ID( n ), ID( dst ), ID( src ), type( t ) );
    return( n );
}

cg_name CGPreGets( cg_op o, cg_name dst, cg_name src, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGPreGets %s n%u n%u %s", ID( n ), op( o ), ID( dst ), ID( src ), type( t ) );
    return( n );
}

cg_name CGPostGets( cg_op o, cg_name dst, cg_name src, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGPostGets %s n%u n%u %s", ID( n ), op( o ), ID( dst ), ID( src ), type( t ) );
    return( n );
}

cg_name CGBitMask( cg_name a, byte start, byte len, cg_type t )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGBitMask n%u %u %u %s", ID( n ), ID( a ), start, len, type( t ) );
    return( n );
}

cg_name CGEval( cg_name a )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGEval n%u", ID( n ), ID( a ) );
    return( n );
}

cg_name CGVolatile( cg_name a )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGVolatile n%u", ID( n ), ID( a ) );
    return( n );
}

cg_name CGAttr( cg_name a, cg_sym_attr attr )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGAttr n%u %u", ID( n ), ID( a ), (unsigned)attr );
    return( n );
}

cg_name CGVarargsBasePtr( cg_type t )
{
    (void)t;
    refuse( "CGVarargsBasePtr" );
    return( NULL );
}

void CGDone( cg_name a )
{
    emit( "- CGDone n%u", ID( a ) );
}

void CGTrash( cg_name a )
{
    emit( "- CGTrash n%u", ID( a ) );
}

/* ---- procedures and calls ---- */

void CGProcDecl( cg_sym_handle s, cg_type t )
{
    unsigned y = sym( s );

    emit( "- CGProcDecl y%u %s", y, type( t ) );
}

void CGParmDecl( cg_sym_handle s, cg_type t )
{
    unsigned y = sym( s );

    emit( "- CGParmDecl y%u %s", y, type( t ) );
}

label_handle CGLastParm( void )
{
    label_handle l = HANDLE( label_handle );

    emit( "l%u CGLastParm", ID( l ) );
    return( l );
}

void CGAutoDecl( cg_sym_handle s, cg_type t )
{
    unsigned y = sym( s );

    emit( "- CGAutoDecl y%u %s", y, type( t ) );
}

call_handle CGInitCall( cg_name target, cg_type t, cg_sym_handle aux )
{
    unsigned y = sym( aux );
    call_handle c = HANDLE( call_handle );

    emit( "c%u CGInitCall n%u %s y%u", ID( c ), ID( target ), type( t ), y );
    return( c );
}

void CGAddParm( call_handle c, cg_name a, cg_type t )
{
    emit( "- CGAddParm c%u n%u %s", ID( c ), ID( a ), type( t ) );
}

cg_name CGCall( call_handle c )
{
    cg_name n = HANDLE( cg_name );

    emit( "n%u CGCall c%u", ID( n ), ID( c ) );
    return( n );
}

void CGReturn( cg_name a, cg_type t )
{
    emit( "- CGReturn n%u %s", ID( a ), type( t ) );
}

void CGControl( cg_op o, cg_name a, label_handle l )
{
    emit( "- CGControl %s n%u l%u", op( o ), ID( a ), ID( l ) );
}

void CGBigLabel( back_handle b )
{
    emit( "- CGBigLabel b%u", ID( b ) );
}

sel_handle CGSelInit( void )
{
    sel_handle s = HANDLE( sel_handle );

    emit( "s%u CGSelInit", ID( s ) );
    return( s );
}

void CGSelCase( sel_handle s, label_handle l, signed_64 v )
{
    emit( "- CGSelCase s%u l%u %lld", ID( s ), ID( l ), wide( &v ) );
}

void CGSelRange( sel_handle s, signed_64 lo, signed_64 hi, label_handle l )
{
    emit( "- CGSelRange s%u %lld %lld l%u", ID( s ), wide( &lo ), wide( &hi ), ID( l ) );
}

void CGSelOther( sel_handle s, label_handle l )
{
    emit( "- CGSelOther s%u l%u", ID( s ), ID( l ) );
}

void CGSelect( sel_handle s, cg_name a )
{
    emit( "- CGSelect s%u n%u", ID( s ), ID( a ) );
}

/* ---- data ---- */

void DGLabel( back_handle b )
{
    emit( "- DGLabel b%u", ID( b ) );
}

void DGBackPtr( back_handle b, segment_id seg, signed_32 offset, cg_type t )
{
    emit( "- DGBackPtr b%u %d %ld %s", ID( b ), (int)seg, (long)offset, type( t ) );
}

void DGFEPtr( cg_sym_handle s, cg_type t, signed_32 offset )
{
    unsigned y = sym( s );

    emit( "- DGFEPtr y%u %s %ld", y, type( t ), (long)offset );
}

void DGInteger( unsigned_32 v, cg_type t )
{
    emit( "- DGInteger %lu %s", (unsigned long)v, type( t ) );
}

void DGInteger64( unsigned_64 v, cg_type t )
{
    emit( "- DGInteger64 %lld %s", wide( &v ), type( t ) );
}

void DGFloat( cchar_ptr text, cg_type t )
{
    char q[128];

    quoted( q, sizeof( q ), text );
    emit( "- DGFloat %s %s", q, type( t ) );
}

void DGBytes( unsigned_32 len, const void *data )
{
    const unsigned char *p = data;
    unsigned_32 i;

    fprintf( Out, "- DGBytes %lu ", (unsigned long)len );
    for( i = 0; i < len; ++i ) {
        fprintf( Out, "%02x", p[i] );
    }
    fputc( '\n', Out );
}

void DGIBytes( unsigned_32 len, byte b )
{
    emit( "- DGIBytes %lu %u", (unsigned long)len, b );
}

void DGUBytes( unsigned_32 len )
{
    emit( "- DGUBytes %lu", (unsigned long)len );
}

void DGAlign( uint align )
{
    emit( "- DGAlign %u", align );
}

/* ---- debug information: only the line numbers are kept ---- */

uint DBSrcFile( cchar_ptr name )
{
    char q[512];
    unsigned id = Next++;

    quoted( q, sizeof( q ), name );
    emit( "f%u DBSrcFile %s", id, q );
    return( id );
}

void DBSrcCue( uint file, uint line, uint col )
{
    emit( "- DBSrcCue f%u %u %u", file, line, col );
}

void DBModSym( cg_sym_handle s, cg_type t ) { (void)s; (void)t; }
void DBLocalSym( cg_sym_handle s, cg_type t ) { (void)s; (void)t; }
void DBTypeDef( cchar_ptr n, dbg_type t ) { (void)n; (void)t; }
dbg_type DBScalar( cchar_ptr n, cg_type t ) { (void)n; (void)t; return( DBG_NIL_TYPE ); }
dbg_type DBScope( cchar_ptr n ) { (void)n; return( DBG_NIL_TYPE ); }
dbg_name DBBegName( cchar_ptr n, dbg_type t ) { (void)n; (void)t; return( NULL ); }
dbg_type DBForward( dbg_name n ) { (void)n; return( DBG_NIL_TYPE ); }
dbg_type DBEndName( dbg_name n, dbg_type t ) { (void)n; (void)t; return( DBG_NIL_TYPE ); }
dbg_type DBIntArrayCG( cg_type t, unsigned_32 hi, dbg_type b ) { (void)t; (void)hi; (void)b; return( DBG_NIL_TYPE ); }
dbg_type DBPtr( cg_type t, dbg_type b ) { (void)t; (void)b; return( DBG_NIL_TYPE ); }
dbg_struct DBBegNameStruct( cchar_ptr n, cg_type t, bool c ) { (void)n; (void)t; (void)c; return( NULL ); }
dbg_type DBStructForward( dbg_struct s ) { (void)s; return( DBG_NIL_TYPE ); }
void DBAddField( dbg_struct s, unsigned_32 o, cchar_ptr n, dbg_type t ) { (void)s; (void)o; (void)n; (void)t; }
void DBAddBitField( dbg_struct s, unsigned_32 o, byte st, byte l, cchar_ptr n, dbg_type t )
{ (void)s; (void)o; (void)st; (void)l; (void)n; (void)t; }
dbg_type DBEndStruct( dbg_struct s ) { (void)s; return( DBG_NIL_TYPE ); }
dbg_enum DBBegEnum( cg_type t ) { (void)t; return( NULL ); }
void DBAddConst64( dbg_enum e, cchar_ptr n, signed_64 v ) { (void)e; (void)n; (void)v; }
dbg_type DBEndEnum( dbg_enum e ) { (void)e; return( DBG_NIL_TYPE ); }
dbg_proc DBBegProc( cg_type t, dbg_type r ) { (void)t; (void)r; return( NULL ); }
void DBAddParm( dbg_proc p, dbg_type t ) { (void)p; (void)t; }
dbg_type DBEndProc( dbg_proc p ) { (void)p; return( DBG_NIL_TYPE ); }
dbg_type DBBasedPtr( cg_type t, dbg_type b, dbg_loc l ) { (void)t; (void)b; (void)l; return( DBG_NIL_TYPE ); }
dbg_loc DBLocInit( void ) { return( NULL ); }
dbg_loc DBLocSym( dbg_loc l, cg_sym_handle s ) { (void)s; return( l ); }
dbg_loc DBLocConst( dbg_loc l, unsigned_32 v ) { (void)v; return( l ); }
dbg_loc DBLocOp( dbg_loc l, dbg_loc_op o, unsigned n ) { (void)o; (void)n; return( l ); }
void DBLocFini( dbg_loc l ) { (void)l; }
void DBBegBlock( void ) { }
void DBEndBlock( void ) { }
