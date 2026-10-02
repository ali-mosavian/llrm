option explicit

dim pixel_offset as integer
dim pixel_value as integer

screen 13
def seg = &HA000
for pixel_offset = 0 to 15
    poke pixel_offset, pixel_offset
next pixel_offset
for pixel_offset = 0 to 15
    pixel_value = peek(pixel_offset)
    if pixel_value <> pixel_offset then
        print "FAIL vga_framebuffer_bsave pixel"
        def seg
        screen 0
        end
    end if
next pixel_offset
bsave "VBGFX.BIN", 0, 16
def seg
screen 0

print "PASS vga_framebuffer_bsave"
