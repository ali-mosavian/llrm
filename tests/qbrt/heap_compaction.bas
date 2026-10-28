' CLOSE of a file compacts the local heap (B$LHDALC_CPCT): closing the older of two files returns its room to string
' space at once. The C runtime never compacted, so FRE("") grew only when the newer file was closed too.
DEFLNG A-Z
a = FRE("")
OPEN "FR1.DAT" FOR RANDOM AS #1 LEN = 64
OPEN "FR2.DAT" FOR RANDOM AS #2 LEN = 64
b = FRE("")
CLOSE #1
c = FRE("")
CLOSE #2
d = FRE("")
PRINT (a > b); (c > b); (d > c); (a = d)
KILL "FR1.DAT": KILL "FR2.DAT"
