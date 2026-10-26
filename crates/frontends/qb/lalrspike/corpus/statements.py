"""The statement catalog: patterns by group, expanded by expand.py."""

from expand import sentences

P = {}


def group(name, *patterns):
    P.setdefault(name, []).extend(patterns)


COORD = "[STEP]({n},{n})"

group("screen",
      "SCREEN [{n}][,[{n}][,[{n}][,{n}]]]",
      "COLOR [{c}][,[{c}][,{c}]]",
      "CLS [{i}]",
      "LOCATE [{i}][,[{i}][,[{i}][,[{i}][,{i}]]]]",
      "WIDTH {i}[,{i}]",
      "WIDTH {f},{i}",
      "WIDTH {g},{i}",
      "WIDTH LPRINT {i}",
      'WIDTH {"SCRN:"|"LPT1:"},{i}',
      "VIEW PRINT [{i} TO {i}]",
      "VIEW [[SCREEN] ({n},{n})-({n},{n})[,[{c}][,{c}]]]",
      "WINDOW [[SCREEN] ({n},{n})-({n},{n})]",
      "PALETTE [{i},{n}]",
      "PALETTE USING {a}",
      "PCOPY {i},{i}",
      )

group("graphics",
      "PSET " + COORD + "[,{c}]",
      "PRESET " + COORD + "[,{c}]",
      "LINE [" + COORD + "]-" + COORD + "[,[{c}][,[{B|BF}][,{n}]]]",
      "CIRCLE " + COORD + ",{n}[,[{c}][,[{n}][,[{n}][,{n}]]]]",
      "PAINT " + COORD + "[,[{c|s}][,[{c}][,{s}]]]",
      "DRAW {s}",
      "GET " + COORD + "-" + COORD + ",{a}",
      "PUT " + COORD + ",{a}[,{PSET|PRESET|AND|OR|XOR}]",
      )

group("sound",
      "PLAY {s}", "PLAY {ON|OFF|STOP}", "SOUND {n},{n}", "BEEP",
      )

group("machine",
      "POKE {n},{n}", "OUT {n},{n}", "WAIT {n},{n}[,{n}]", "DEF SEG[ = {n}]",
      "BLOAD {s}[,{n}]", "BSAVE {s},{n},{n}", "RANDOMIZE[ {n}]", "RANDOMIZE TIMER",
      "SLEEP[ {n}]", "TRON", "TROFF", "CLEAR[ {n}[,[{n}][,{n}]]]",
      "SHELL[ {s}]", "ENVIRON {s}", "SYSTEM", "STOP", "END",
      "DATE$ = {s}", "TIME$ = {s}", "ERROR {i}",
      )

group("files",
      "OPEN {s} FOR {INPUT|OUTPUT|APPEND|RANDOM|BINARY} AS {f}",
      "OPEN {s} FOR {INPUT|OUTPUT|APPEND|RANDOM|BINARY} AS {g}",
      "OPEN {s} AS {f}[ LEN = {i}]",
      "OPEN {s} FOR RANDOM AS {f} LEN = {i}",
      "OPEN {s} FOR BINARY ACCESS {READ|WRITE|READ WRITE} AS {f}",
      "OPEN {s} FOR INPUT SHARED AS {f}",
      "OPEN {s} FOR INPUT LOCK {READ|WRITE|READ WRITE} AS {f}",
      "OPEN {s} FOR RANDOM ACCESS READ WRITE LOCK WRITE AS {f}",
      "OPEN {q},{f},{s}[,{i}]",
      "OPEN {q}, {g}, {s}",
      "CLOSE[ {f}[,{f}]]",
      "CLOSE[ {g}]",
      "RESET",
      "SEEK {f},{n}",
      "GET {f}[,[{n}][,{v}]]",
      "PUT {f}[,[{n}][,{v}]]",
      "LOCK {f}[,{n}[ TO {n}]]",
      "UNLOCK {f}[,{n}[ TO {n}]]",
      "FIELD {f},{i} AS {t}[,{i} AS {t}]",
      "LSET {t} = {s}",
      "RSET {t} = {s}",
      "IOCTL {f},{s}",
      "NAME {s} AS {s}", "KILL {s}", "MKDIR {s}", "CHDIR {s}", "RMDIR {s}", "FILES[ {s}]",
      "CHAIN {s}", "RUN[ {s}]", "RUN lab",
      "WRITE[ {f},][{n}[,{s}]]",
      "WRITE {n};{s}",
      "INPUT [;]{v}[,{v}]",
      'INPUT [;]{q}{;|,}{v}',
      "INPUT {f},{v}[,{v}]",
      "LINE INPUT [;][{q};]{t}",
      "LINE INPUT {f},{t}",
      )

group("print",
      "PRINT",
      "PRINT {n}", "PRINT {s}", "PRINT {s};{n}", "PRINT {s},{n}", "PRINT {n};", "PRINT ;", "PRINT ,",
      "PRINT {n};;{n}", "PRINT {n},,{n}",
      "PRINT {f},{n}", "PRINT {f};{n}", "PRINT {f}",
      "PRINT USING {s};{n}", "PRINT USING {s};{n};{s}", "PRINT USING {s};", "PRINT {f},USING {s};{n}",
      "PRINT USING {n};{n}",
      "PRINT TAB({i});{s}", "PRINT SPC({i});{s}", "PRINT {s};TAB({i});{s}", "PRINT TAB({i})", "PRINT SPC(3)",
      "PRINT {s} {n}", "PRINT {s}{s}", "PRINT {n} {n}",
      "LPRINT {n}", "LPRINT USING {s};{n}", "LPRINT", "LPRINT {s};{n};",
      "? {n}", "? {s};{n}",
      )

group("data",
      "DIM {v}", "x% = {n}", "s$ = {s}", "s$ = {n}", "x% = {s}",
      "SWAP {v},{v}", "SWAP s$,t$", "SWAP x%,s$",
      "LET x% = {n}", "MID$(s$, {i}[,{i}]) = {s}", "MID$(s$, {s}) = {s}", "MID$(x%, 1) = {s}",
      "ERASE {a}", "ERASE x%",
      "arr%({i}) = {n}", "arr%({i},{i}) = {n}", "m%({i},{i}) = {n}", "m%({i}) = {n}",
      "READ {v}[,{v}]", "RESTORE[ lab]",
      "OPTION BASE {0|1}", "DEFINT A-Z", "DEFSTR s", "DEFLNG a-c", "DEFSNG x", "DEFDBL d",
      "CONST k1 = {n}", "CONST k2 = {s}", "CONST k3 = {v}",
      "r.a = {n}", "r.s = {s}", "r.a = {s}", "r.zz = 1", "r = r",
      "x% = c%", "c% = 1",
      )

group("control",
      "IF {n} THEN x% = 1[ ELSE y% = 2]",
      "IF {s} THEN x% = 1",
      "IF {n} THEN lab[ ELSE lab]",
      "IF {n} GOTO lab",
      "IF {n} THEN 10",
      "FOR i% = {n} TO {n}[ STEP {n}]: NEXT",
      "FOR i% = {n} TO {n}: NEXT i%",
      "FOR s$ = 1 TO 2: NEXT",
      "FOR i% = {s} TO 2: NEXT",
      "FOR arr%(1) = 1 TO 2: NEXT",
      "FOR i% = 1 TO 2: NEXT j%",
      "WHILE {n}: WEND",
      "WHILE {s}: WEND",
      "DO[ {WHILE|UNTIL} {n}]: LOOP",
      "DO: LOOP {WHILE|UNTIL} {n}",
      "DO WHILE {n}: LOOP UNTIL {n}",
      "SELECT CASE {n}: CASE 1: x% = 1: CASE ELSE: y% = 2: END SELECT",
      "SELECT CASE {n}: CASE IS > {n}, 5 TO 7, 9: x% = 1: END SELECT",
      "SELECT CASE {s}: CASE \"a\": x% = 1: END SELECT",
      "SELECT CASE {s}: CASE 1: x% = 1: END SELECT",
      "SELECT CASE {n}: CASE \"a\": x% = 1: END SELECT",
      "SELECT CASE r: CASE 1: END SELECT",
      "GOSUB lab", "GOTO lab", "GOTO lab2", "GOTO 10", "RETURN", "RETURN lab",
      "ON {n} GOTO lab[,lab]", "ON {n} GOSUB lab[,lab]", "ON {s} GOTO lab", "ON x% GOTO",
      "ON ERROR GOTO lab", "ON ERROR GOTO 0", "ON ERROR RESUME NEXT", "ON LOCAL ERROR GOTO lab",
      "RESUME", "RESUME NEXT", "RESUME lab", "RESUME 0",
      "EXIT SUB", "EXIT FUNCTION", "EXIT DO", "EXIT FOR", "EXIT WHILE",
      "CALL mysub(x%, s$)", "CALL mysub", "CALL mysub(s$, x%)", "CALL mysub(x%)", "CALL mysub(x%, s$, 1)",
      "mysub x%, s$", "mysub(x%, s$)", "mysub (x%), s$", "mysub x%", "mysub",
      "CALL nosuch", "nosuch 1, 2",
      "x% = myfn%({n})", "x% = myfn%(s$)", "x% = myfn%()", "x% = myfn%(1, 2)", "s$ = myfn%(1)",
      "CALL myfn%(1)", "myfn% 1",
      "CALL arrsub(arr%())", "CALL arrsub(arl&())", "CALL arrsub(x%)", "arrsub arr%()",
      "DEF FNa%(z%) = z% * 2", "x% = FNa%(1)", "DEF FNb$(z$) = z$", "DEF FNa%(z%) = z%",
      )

group("events",
      "ON {KEY|TIMER|PLAY|STRIG|COM|PEN|UEVENT|SIGNAL}({i}) GOSUB lab",
      "ON {PEN|UEVENT} GOSUB lab",
      "{KEY|TIMER|PLAY|STRIG|COM|PEN|UEVENT|SIGNAL}({i}) {ON|OFF|STOP}",
      "{TIMER|PEN|UEVENT|PLAY} {ON|OFF|STOP}",
      "KEY {ON|OFF|LIST}", "KEY {k}, {s}", "KEY {k}, CHR$(1) + CHR$(2)",
      "STRIG {ON|OFF}",
      )

FUNCS_N = ["ABS", "SGN", "INT", "FIX", "CINT", "CLNG", "CSNG", "CDBL", "SQR", "SIN", "COS", "TAN", "ATN", "LOG", "EXP"]
group("functions",
      *[f"x% = {f}({{n}})" for f in FUNCS_N],
      *[f"s! = {f}({{s}})" for f in FUNCS_N[:3]],
      "s! = RND", "s! = RND({n})", "s! = RND(1,2)",
      "x% = LEN({s})", "x% = LEN({n})", "x% = LEN(r)", "x% = ASC({s})", "x% = ASC(1)", "s! = VAL({s})", "s! = VAL({n})",
      "x% = INSTR({s},{s})", "x% = INSTR({i},{s},{s})", "x% = INSTR({s},{s},{s})", "x% = INSTR(1)",
      "x% = CVI({s})", "l& = CVL({s})", "s! = CVS({s})", "d# = CVD({s})", "s! = CVSMBF({s})", "d# = CVDMBF({s})",
      "s$ = MKI$({i})", "s$ = MKL$({n})", "s$ = MKS$({n})", "s$ = MKD$({n})", "s$ = MKSMBF$({n})", "s$ = MKDMBF$({n})",
      "s$ = LEFT$({s},{i})", "s$ = RIGHT$({s},{i})", "s$ = MID$({s},{i}[,{i}])", "s$ = LEFT$({s})", "s$ = LEFT$({i},{i})",
      "s$ = STR$({n})", "s$ = STR$({s})", "s$ = CHR$({i})", "s$ = SPACE$({i})", "s$ = STRING$({i},{i})", "s$ = STRING$({i},{s})",
      "s$ = HEX$({n})", "s$ = OCT$({n})", "s$ = LTRIM$({s})", "s$ = RTRIM$({s})", "s$ = UCASE$({s})", "s$ = LCASE$({s})",
      "x% = PEEK({n})", "x% = INP({n})", "x% = POS({i})", "x% = CSRLIN", "s! = TIMER", "s$ = DATE$", "s$ = TIME$",
      "s$ = COMMAND$", "s$ = ENVIRON$({s})", "s$ = ENVIRON$({i})", "s$ = INKEY$", "s$ = INPUT$({i})", "s$ = INPUT$({i},{f})",
      "x% = EOF({g})", "l& = LOF({g})", "l& = LOC({g})", "l& = SEEK({g})", "x% = FILEATTR({g},{i})", "x% = FREEFILE",
      "l& = FRE({i})", "l& = FRE({s})", "x% = LBOUND({a})", "x% = LBOUND({a},{i})", "x% = UBOUND({a})", "x% = UBOUND({a},{i})",
      "x% = UBOUND(x%)",
      "x% = VARPTR({v})", "x% = VARSEG({v})", "s$ = VARPTR$({v})", "x% = SADD({t})", "x% = SADD(1)",
      "x% = POINT({n},{n})", "x% = POINT({i})", "x% = PMAP({n},{i})", "x% = SCREEN({i},{i})", "x% = SCREEN({i},{i},{i})",
      "x% = STICK({i})", "x% = STRIG({i})", "x% = PEN({i})", "x% = PLAY({i})", "x% = ERR", "x% = ERL", "x% = ERDEV", "s$ = ERDEV$",
      "x% = LPOS({i})", "x% = SETMEM({n})",
      "s$ = TAB(3)", "x% = FOO(1)",
      "x% = {n} + {n}", "s$ = {s} + {s}", "s$ = {s} + {n}", "x% = {s} + {n}", "x% = {s} < {s}", "x% = {s} < {n}", "x% = {s} AND {n}",
      "x% = NOT {n}", "x% = NOT {s}", "x% = -{s}", "x% = {n} MOD {n}", "x% = {n} \\ {n}", "s! = {n} / {n}", "d# = {n} ^ {n}",
      "x% = {s} * 2", "s$ = {s} - {s}", "x% = {n} XOR {n}", "x% = {n} EQV {n}", "x% = {n} IMP {n}",
      "x% = (1 + 2", "x% = 1 +", "x% = ()", "x% = 1 2",
      "x% = 32768", "x% = 40000", "l& = 2147483648", "x% = 1E40", "s! = 1D+40", "x% = &HFFFF", "x% = &H10000", "l& = &HFFFFFFFF&",
      "x% = 1..2", "x% = .", "s$ = \"abc",
      )


# Every reserved word of the three families, used as a name where a name stands.
WORDS = "ABS|ACCESS|ADDITEM|ALIAS|ALL|AND|ANY|APPEND|AS|ASC|ATN|BASE|BEEP|BEGINTRANS|BINARY|BLOAD|BOF|BSAVE|BYVAL|CALL|CALLS|CASE|CCUR|CDBL|CDECL|CHAIN|CHDIR|CHDRIVE|CHECKPOINT|CHR$|CINT|CIRCLE|CLEAR|CLIPBOARD|CLNG|CLOSE|CLS|COLOR|COM|COMMAND$|COMMITTRANS|COMMON|CONST|CONTROL|COS|CREATEINDEX|CSNG|CSRLIN|CURDIR$|CURRENCY|CVC|CVD|CVDMBF|CVI|CVL|CVS|CVSMBF|DATA|DATE$|DATESERIAL|DATEVALUE|DAY|DECLARE|DEF|DEFCUR|DEFDBL|DEFINT|DEFLNG|DEFSNG|DEFSTR|DELETE|DELETEINDEX|DELETETABLE|DIM|DIR$|DO|DOEVENTS|DOUBLE|DRAG|DRAW|ELSE|ELSEIF|END|ENDDOC|ENDIF|ENVIRON|ENVIRON$|EOF|EQV|ERASE|ERDEV|ERDEV$|ERL|ERR|ERROR|ERROR$|EVENT|EXIT|EXP|EXPLICIT|FIELD|FILEATTR|FILES|FIX|FOR|FORM|FORMAT$|FRE|FREEFILE|FUNCTION|GET|GETINDEX$|GETTEXT|GOSUB|GOTO|HEX$|HIDE|HOUR|IF|IMP|INKEY$|INP|INPUT|INPUT$|INPUTBOX$|INSERT|INSTR|INT|INTEGER|IOCTL|IOCTL$|IS|ISAM|KEY|KILL|LBOUND|LCASE$|LEFT$|LEN|LET|LINE|LIST|LOAD|LOC|LOCAL|LOCATE|LOCK|LOF|LOG|LONG|LOOP|LPOS|LPRINT|LSET|LTRIM$|MID$|MINUTE|MKC$|MKD$|MKDIR|MKDMBF$|MKI$|MKL$|MKS$|MKSMBF$|MOD|MONTH|MOVE|MOVEFIRST|MOVELAST|MOVENEXT|MOVEPREVIOUS|MSGBOX|NAME|NEWPAGE|NEXT|NOT|NOW|OCT$|OFF|ON|OPEN|OPTION|OR|OUT|OUTPUT|PAINT|PALETTE|PCOPY|PEEK|PEN|PLAY|PMAP|POINT|POKE|POS|PRESERVE|PRESET|PRINT|PRINTER|PRINTFORM|PSET|PUT|QBCOLOR|RANDOM|RANDOMIZE|READ|REDIM|REFRESH|REM|REMOVEITEM|RESET|RESTORE|RESUME|RETRIEVE|RETURN|RGB|RIGHT$|RMDIR|RND|ROLLBACK|RSET|RTRIM$|RUN|SADD|SAVEPOINT|SCREEN|SECOND|SEEK|SEEKEQ|SEEKGE|SEEKGT|SEG|SELECT|SETFOCUS|SETINDEX|SETMEM|SETTEXT|SGN|SHARED|SHELL|SHOW|SIGNAL|SIN|SINGLE|SLEEP|SOUND|SPACE$|SPC|SQR|SSEG|SSEGADD|STACK|STATIC|STEP|STICK|STOP|STR$|STRIG|STRING|STRING$|SUB|SWAP|SYSTEM|TAB|TAN|TEXTCOMP|TEXTHEIGHT|TEXTWIDTH|THEN|TIME$|TIMER|TIMESERIAL|TIMEVALUE|TO|TROFF|TRON|TYPE|UBOUND|UCASE$|UEVENT|UNLOAD|UNLOCK|UNTIL|UPDATE|USING|VAL|VARPTR|VARPTR$|VARSEG|VIEW|WAIT|WEEKDAY|WEND|WHILE|WIDTH|WINDOW|WRITE|XOR|YEAR".split("|")
WORD_TEMPLATES = [
    "{w} = 1", "{w}% = 1", '{w}$ = ""', "DIM {w} AS INTEGER", "DIM {w}", "DIM {w}(5)", "x% = {w}", "CONST {w} = 1",
    "FOR {w} = 1 TO 2: NEXT", "x% = {w}(1)", "CALL {w}", "DECLARE SUB {w}", "DIM x AS {w}", "DEF FN{w} = 1",
    "TYPE tt: {w} AS INTEGER: END TYPE", "r.{w} = 1", "{w}:", "TYPE {w}: a AS INTEGER: END TYPE", "SUB q ({w}): END SUB",
    "x% = r.{w}", "SUB {w}: END SUB", "FUNCTION {w}: END FUNCTION",
]


def catalog(only=None):
    out = []
    for name, patterns in P.items():
        if only and name not in only:
            continue
        for pattern in patterns:
            for text, why in sentences(pattern):
                out.append({"group": name, "pattern": pattern, "text": text, "why": why})
    if not only or "words" in only:
        for template in WORD_TEMPLATES:
            for w in WORDS:
                out.append({"group": "words", "pattern": template, "text": template.replace("{w}", w), "why": "word"})
    # drop duplicates by text
    seen, uniq = set(), []
    for item in out:
        if item["text"] not in seen:
            seen.add(item["text"])
            uniq.append(item)
    for i, item in enumerate(uniq):
        item["id"] = i
    return uniq


if __name__ == "__main__":
    import json
    import sys
    items = catalog(sys.argv[1:] or None)
    print(len(items), file=sys.stderr)
    for item in items:
        print(json.dumps(item))


def tsv(items):
    return "".join(f"{i['id']}\t{i['text']}\n" for i in items)
