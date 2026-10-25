' flags: -O2 -march=i486
' data: @signal
' BASIC -m32 hangs at -O1 and above (#1467): add `| -O2 -march=i486 -m32` to the flags when it is fixed.
' Short-time Fourier transform in Q9 fixed point, as stft_fixed.c: LONG arithmetic; a product is floored, as Nib's fixed '*' does: (p - (p AND mask)) \ scale.
DEFINT A-Z
CONST N = 256
CONST HOP = 128
CONST BINS = 129
CONST ONE = 512&
DECLARE FUNCTION BenchStftFixed& ()

DIM SHARED twRe(HOP - 1) AS LONG
DIM SHARED twIm(HOP - 1) AS LONG
DIM SHARED hann(N - 1) AS LONG
DIM SHARED re(N - 1) AS LONG
DIM SHARED im(N - 1) AS LONG
DIM SHARED rev(N - 1) AS INTEGER
DIM SHARED blk(0 TO 1) AS STRING
DIM SHARED rowBytes AS STRING

PRINT LTRIM$(STR$(BenchStftFixed&))
END

FUNCTION BenchStftFixed&
    DIM c AS LONG, s AS LONG, nxt AS LONG, ar AS LONG, ai AS LONG, br AS LONG, bi AS LONG
    DIM tr AS LONG, ti AS LONG, pr AS LONG, pi AS LONG, p1 AS LONG, p2 AS LONG, p3 AS LONG, p4 AS LONG
    DIM sum AS LONG, index AS LONG, q AS LONG, remaining AS LONG, v AS LONG
    c = 32768
    s = 0
    FOR k = 0 TO HOP - 1
        twRe(k) = c \ 64
        twIm(k) = -(s \ 64)
        p1 = c * 32758: p2 = s * 804: p3 = s * 32758: p4 = c * 804
        nxt = (p1 - (p1 AND 32767)) \ 32768 - (p2 - (p2 AND 32767)) \ 32768
        s = (p3 - (p3 AND 32767)) \ 32768 + (p4 - (p4 AND 32767)) \ 32768
        c = nxt
    NEXT
    FOR k = 0 TO HOP - 1
        hann(k) = (ONE - twRe(k)) \ 2
        hann(k + HOP) = ONE - hann(k)
    NEXT
    FOR i = 0 TO N - 1
        r = 0
        t = i
        FOR bit = 1 TO 8
            r = r * 2 + (t AND 1)
            t = t \ 2
        NEXT
        rev(i) = r
    NEXT
    blk(0) = SPACE$(2 * HOP)
    blk(1) = SPACE$(2 * HOP)
    rowBytes = SPACE$(2 * BINS)
    OPEN "SIGNAL" FOR BINARY AS #1
    OPEN "SPECTRA" FOR OUTPUT AS #2
    CLOSE #2
    OPEN "SPECTRA" FOR BINARY AS #2
    remaining = LOF(1) - 2 * HOP
    GET #1, , blk(0)
    tnow = 0
    sum = 0
    index = 0
    DO WHILE remaining >= 2 * HOP
        GET #1, , blk(1 - tnow)
        remaining = remaining - 2 * HOP
        FOR smp = 0 TO HOP - 1
            re(rev(smp)) = (CVI(MID$(blk(tnow), 2 * smp + 1, 2)) \ 4) * hann(smp)
            re(rev(smp + HOP)) = (CVI(MID$(blk(1 - tnow), 2 * smp + 1, 2)) \ 4) * hann(smp + HOP)
            im(rev(smp)) = 0
            im(rev(smp + HOP)) = 0
        NEXT
        m = 2
        DO WHILE m <= N
            half = m \ 2
            stride = N \ m
            FOR i = 0 TO N - 1 STEP m
                FOR j = 0 TO half - 1
                    ar = re(i + j)
                    ai = im(i + j)
                    br = re(i + j + half)
                    bi = im(i + j + half)
                    p1 = twRe(j * stride) * br
                    p2 = twIm(j * stride) * bi
                    p3 = twRe(j * stride) * bi
                    p4 = twIm(j * stride) * br
                    tr = (p1 - (p1 AND 511)) \ ONE - (p2 - (p2 AND 511)) \ ONE
                    ti = (p3 - (p3 AND 511)) \ ONE + (p4 - (p4 AND 511)) \ ONE
                    re(i + j) = (ar + tr) \ 2
                    im(i + j) = (ai + ti) \ 2
                    re(i + j + half) = (ar - tr) \ 2
                    im(i + j + half) = (ai - ti) \ 2
                NEXT
            NEXT
            m = m * 2
        LOOP
        FOR k = 0 TO BINS - 1
            pr = re(k) \ 128
            pi = im(k) \ 128
            q = (pr * pr + pi * pi) \ 256
            IF q > 65535 THEN q = 65535
            v = q
            IF v > 32767 THEN v = v - 65536
            MID$(rowBytes, 2 * k + 1, 2) = MKI$(v)
            sum = ((sum * 3) + q + index) AND &H1FFFFFFF
            index = index + 1
        NEXT
        PUT #2, , rowBytes
        tnow = 1 - tnow
    LOOP
    CLOSE #1
    CLOSE #2
    BenchStftFixed& = sum
END FUNCTION
