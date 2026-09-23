dim keyHits as integer
dim penHits as integer
dim playHits as integer

on key(1) gosub keyHandler
on pen gosub penHandler
on play(1) gosub playHandler
key(1) on
pen on
play on
print "REFERENCE ONLY pds-key-pen-play-events"
key(1) off
pen off
play off
end

keyHandler:
keyHits = keyHits + 1
return

penHandler:
penHits = penHits + 1
return

playHandler:
playHits = playHits + 1
return
