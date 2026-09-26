defint a-z
p = &H3C8: f! = 41.6
out p, f!
poke 0, f!
a = inp(p + 1)
