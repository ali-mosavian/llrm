' CURRENCY as text: INPUT, READ, STR$, MKC$, CVC, PRINT USING, WRITE.
' dialect: pds71
' flags: -O2 --isel=dag
' stdin: curtext.dat
DIM a AS CURRENCY, b AS CURRENCY
DIM s AS STRING
INPUT a
PRINT a
INPUT b, a
PRINT b; a
INPUT "p"; a
PRINT a
READ a, b
PRINT a; b
s = STR$(a) + "|" + STR$(b) + "|" + STR$(-0.5@)
PRINT s
PRINT LEN(MKC$(a));
b = CVC(MKC$(1234567.8901@))
PRINT b
PRINT USING "#######.####  "; a; b
PRINT USING "##.##"; 3.1416@
WRITE a, b, 0.0001@
DATA 99.5, -7
