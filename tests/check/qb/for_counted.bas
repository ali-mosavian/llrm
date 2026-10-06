' RUN: llrm-qb %s --dialect {qb45 | pds71 | vbdos} --runtime {qb45 | pds71 | vbdos} -O2 --cpu 486 -fno-inline-functions -S -o /dev/stdout
' ownf_free's loops: a FOR with a constant step is counted, its test one compare against the limit.
' Main lost that (287e5640): the step was loaded each trip and tested for its sign, twice.
' CHECK-LABEL: $QB$MAIN proc
' CHECK: cmp {{.*}}, 3000
' CHECK: jle
' CHECK: $QB$MAIN endp
' Own frames: local dynamic arrays and strings are released at procedure exit. FRE
' reads the same after a loop of calls as before it. Numeric FRE does not build here
' (#462): 3000 calls would exhaust the array heap if one leaked.
DECLARE SUB arrays (n AS INTEGER)
DECLARE SUB strings (n AS INTEGER)
DEFINT A-Z
DIM before AS LONG, after AS LONG, i AS INTEGER
arrays 1
strings 1
FOR i = 1 TO 3000
  arrays i MOD 50
NEXT
PRINT "arrays", "survived"
before = FRE("")
FOR i = 1 TO 50
  strings i
NEXT
after = FRE("")
PRINT "strings", before = after
END
SUB arrays (n AS INTEGER)
REDIM a(300 + n) AS INTEGER
DIM b(40) AS LONG
a(n) = n: b(2) = n
IF a(n) + b(2) <> 2 * n THEN PRINT "bad"
END SUB
SUB strings (n AS INTEGER)
DIM s AS STRING
s = STRING$(100 + n, "x")
IF LEN(s) <> 100 + n THEN PRINT "bad"
END SUB

