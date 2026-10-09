' RTRIM$ and STR$ of an INTEGER make temporaries and leave their argument alone.
s$ = "  x  "
PRINT "[" + RTRIM$(s$) + "]"
PRINT "[" + LTRIM$(s$) + "]"
PRINT LEN(s$)
PRINT STR$(-5)
PRINT STR$(12345)
