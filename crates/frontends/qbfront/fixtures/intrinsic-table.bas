dim angle as single
dim wave as single
dim root as single
dim entry as string
dim nextFile as integer

wave = sin(angle) + cos(angle)
root = sqr(abs(wave))
entry = dir$("")
nextFile = freefile
