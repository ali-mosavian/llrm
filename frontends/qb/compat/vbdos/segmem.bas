option explicit

dim memory_words(0 to 2) as integer
dim mapped_offset as integer

memory_words(0) = &H1234
memory_words(1) = &H5678
memory_words(2) = &H2345
mapped_offset = varptr(memory_words(1))
def seg = varseg(memory_words(1))
poke mapped_offset, 42
def seg

if memory_words(1) <> &H562A then
    print "FAIL segmented_memory poke"
    end
end if
if memory_words(0) <> &H1234 or memory_words(2) <> &H2345 then
    print "FAIL segmented_memory canary"
    end
end if

print "PASS segmented_memory"
