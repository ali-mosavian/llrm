; The clone names the C stack pointer c_sp; the installed cc65 (2.18) calls it sp.
        .importzp sp
        .exportzp c_sp
c_sp = sp
