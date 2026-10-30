' flags: -O2 -march=i486
' data: @signal
' BASIC -m32 hangs at -O1 and above (#1467): add `| -O2 -march=i486 -m32` to the flags when it is fixed.
' Short-time Fourier transform in SINGLE, as stft_single.c: a 256-point Hann-windowed FFT of every 128-sample hop of SIGNAL,
' the 129 bin powers of each frame written to SPECTRA. A hop is read as a 256-byte string, CVI'd; a row is built by MID$ and MKI$.
DEFINT A-Z
CONST N = 256
CONST HOP = 128
CONST BINS = 129
DECLARE FUNCTION BenchStftSingle& ()

DIM SHARED twRe(HOP - 1) AS SINGLE
DIM SHARED twIm(HOP - 1) AS SINGLE
DIM SHARED hann(N - 1) AS SINGLE
DIM SHARED re(N - 1) AS SINGLE
DIM SHARED im(N - 1) AS SINGLE
DIM SHARED rev(N - 1) AS INTEGER
DIM SHARED blk(0 TO 1) AS STRING
DIM SHARED rowBytes AS STRING

PRINT LTRIM$(STR$(BenchStftSingle&))
END

FUNCTION BenchStftSingle&
    DIM c AS SINGLE, s AS SINGLE, nxt AS SINGLE, ar AS SINGLE, ai AS SINGLE, br AS SINGLE, bi AS SINGLE
    DIM tr AS SINGLE, ti AS SINGLE, p AS SINGLE
    DIM sum AS LONG, index AS LONG, q AS LONG, remaining AS LONG, v AS LONG
    c = 1!
    s = 0!
    FOR k = 0 TO HOP - 1
        twRe(k) = c
        twIm(k) = -s
        nxt = c * .99969882! - s * .024541229!
        s = s * .99969882! + c * .024541229!
        c = nxt
    NEXT
    FOR k = 0 TO HOP - 1
        hann(k) = (1! - twRe(k)) * .5!
        hann(k + HOP) = 1! - hann(k)
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
            re(rev(smp)) = CSNG(CVI(MID$(blk(tnow), 2 * smp + 1, 2))) * .25! * hann(smp)
            re(rev(smp + HOP)) = CSNG(CVI(MID$(blk(1 - tnow), 2 * smp + 1, 2))) * .25! * hann(smp + HOP)
            im(rev(smp)) = 0!
            im(rev(smp + HOP)) = 0!
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
                    tr = twRe(j * stride) * br - twIm(j * stride) * bi
                    ti = twRe(j * stride) * bi + twIm(j * stride) * br
                    re(i + j) = (ar + tr) * .5!
                    im(i + j) = (ai + ti) * .5!
                    re(i + j + half) = (ar - tr) * .5!
                    im(i + j + half) = (ai - ti) * .5!
                NEXT
            NEXT
            m = m * 2
        LOOP
        FOR k = 0 TO BINS - 1
            p = (re(k) * re(k) + im(k) * im(k)) * (1! / 16!)
            IF p >= 65535! THEN q = 65535 ELSE q = FIX(p)
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
    BenchStftSingle& = sum
END FUNCTION
