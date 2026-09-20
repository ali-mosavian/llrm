option explicit

dim renderer_tick as long
dim frame_index as integer

renderer_tick = 1000
for frame_index = 1 to 3
    renderer_tick = renderer_tick + frame_index
next frame_index

if renderer_tick <> 1006 then
    print "FAIL underscore_identifiers arithmetic"
    end
end if

print "PASS underscore_identifiers"
