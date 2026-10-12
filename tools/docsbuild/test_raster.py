"""PNG rendering of the figure SVG subset: size, colours, ink for text and arrowheads, reproducible bytes."""
import math
import re
import struct
import zlib

from docsbuild import figures, png, raster, ttf

WHITE = (255, 255, 255)


def decode(data):
    """(width, height, colour type, rows of (r, g, b)) of a PNG written by raster."""
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    chunks, i = {}, 8
    while i < len(data):
        (n,) = struct.unpack(">I", data[i : i + 4])
        kind = data[i + 4 : i + 8].decode()
        assert zlib.crc32(data[i + 4 : i + 8 + n]) == struct.unpack(">I", data[i + 8 + n : i + 12 + n])[0]
        chunks[kind] = chunks.get(kind, b"") + data[i + 8 : i + 8 + n]
        i += 12 + n
    w, h, depth, ctype = struct.unpack(">IIBB", chunks["IHDR"][:10])
    assert depth == 8 and list(chunks) in (["IHDR", "PLTE", "IDAT", "IEND"], ["IHDR", "IDAT", "IEND"])
    raw = zlib.decompress(chunks["IDAT"])
    bpp = 1 if ctype == 3 else 3
    rows = []
    for y in range(h):
        line = raw[y * (w * bpp + 1) : (y + 1) * (w * bpp + 1)]
        assert line[0] == 0
        px = line[1:]
        if ctype == 3:
            pal = chunks["PLTE"]
            rows.append([tuple(pal[3 * p : 3 * p + 3]) for p in px])
        else:
            rows.append([tuple(px[3 * x : 3 * x + 3]) for x in range(w)])
    return w, h, ctype, rows


def svg(body, w=40, h=20):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">{body}</svg>'


def px(rows, x, y):
    return rows[round(y * raster.SCALE)][round(x * raster.SCALE)]


def ink(rows, x0, y0, x1, y1):
    s = raster.SCALE
    return sum(
        1 for y in range(int(y0 * s), int(y1 * s)) for x in range(int(x0 * s), int(x1 * s)) if rows[y][x] != WHITE
    )


def test_png_size_is_the_viewbox_times_the_scale():
    w, h, _, _ = decode(raster.render_png(svg("", 37.3, 11.1)))
    assert (w, h) == (math.ceil(37.3 * raster.SCALE), math.ceil(11.1 * raster.SCALE))


def test_the_same_svg_gives_the_same_bytes_with_a_cold_or_warm_glyph_cache():
    s = figures.render('kind: graph\nnode A "Hello, W"\nedge A -> A "again"')
    first = raster.render_png(s)
    raster._glyphs.clear()
    assert raster.render_png(s) == first == raster.render_png(s)


def test_the_compressed_stream_is_ours_not_zlibs():
    """zlib's deflate differs between builds; ours is a pure function of the pixels."""
    rows = [bytes([i % 3] * 50) for i in range(30)]
    framed = [b"\0" + r for r in rows]
    stream = png.deflate(framed)
    assert zlib.decompress(stream, -15) == b"".join(framed) and png.deflate(framed) == stream


def test_a_filled_and_stroked_rect_has_its_colours_at_known_points():
    rows = decode(raster.render_png(svg('<rect class="nd f-a" x="4" y="4" width="20" height="10" rx="0"/>')))[3]
    assert px(rows, 15, 9) == (0xDB, 0xEA, 0xFE)  # fill: --a
    assert px(rows, 4, 9) == (0x25, 0x63, 0xEB)  # the stroke is centred on the edge: --as
    assert px(rows, 30, 9) == WHITE and px(rows, 15, 18) == WHITE


def test_background_is_opaque_white():
    rows = decode(raster.render_png(svg("")))[3]
    assert {p for r in rows for p in r} == {WHITE}


def test_rounded_corner_is_not_filled():
    rows = decode(raster.render_png(svg('<rect class="nd f-a" x="4" y="4" width="20" height="12" rx="6"/>')))[3]
    assert px(rows, 4.3, 4.3) == WHITE and px(rows, 14, 10) == (0xDB, 0xEA, 0xFE)


def test_dashed_stroke_has_gaps():
    rows = decode(raster.render_png(svg('<path class="e dash" d="M2 10H38"/>')))[3]
    line = [rows[round(10 * raster.SCALE)][x] != WHITE for x in range(round(2 * raster.SCALE), round(38 * raster.SCALE))]
    assert any(line) and not all(line)


def test_text_draws_ink_inside_its_box_and_nowhere_else():
    text = "Wave"
    width = ttf.font().width(text, raster.FONT_SIZE)
    rows = decode(raster.render_png(svg(f'<text class="t" x="5" y="15">{text}</text>', 60, 24)))[3]
    assert ink(rows, 5, 15 - 11, 5 + width, 15 + 3) > 40
    assert ink(rows, 0, 0, 60, 15 - 11) == 0 and ink(rows, 5 + width + 1, 0, 60, 24) == 0 and ink(rows, 0, 0, 4, 24) == 0


def test_text_is_centred_and_right_aligned_on_its_anchor():
    width = ttf.font().width("Wave", raster.FONT_SIZE)
    mid = decode(raster.render_png(svg('<text class="t m" x="20" y="15">Wave</text>')))[3]
    end = decode(raster.render_png(svg('<text class="t x" x="38" y="15">Wave</text>')))[3]
    assert ink(mid, 20 - width / 2 - 1, 0, 20 + width / 2 + 1, 20) > 40 and ink(mid, 0, 0, 20 - width / 2 - 1, 20) == 0
    assert ink(end, 38 - width - 1, 0, 40, 20) > 40 and ink(end, 0, 0, 38 - width - 1, 20) == 0


def test_a_label_halo_clears_the_line_behind_the_text():
    body = '<path class="e" d="M2 10H38"/><text class="t lbl m" x="20" y="14">M</text>'
    rows = decode(raster.render_png(svg(body)))[3]
    assert px(rows, 14.8, 10) == WHITE  # the halo wipes the line beside the letter
    assert px(rows, 4, 10) != WHITE


def test_unknown_character_draws_the_box_glyph_and_takes_its_advance():
    f = ttf.font()
    ch = "\U0010ffff"
    assert f.glyph(ch) == 0 and f.width(ch, 13) == f.advance(0) * 13 / f.units
    rows = decode(raster.render_png(svg(f'<text class="t" x="5" y="15">{ch}</text>')))[3]
    assert ink(rows, 5, 4, 5 + f.width(ch, 13), 16) > 30


def test_arrowhead_is_wider_than_its_line():
    body = '<path class="e" d="M20 2V18" marker-end="url(#a)"/>'
    rows = decode(raster.render_png(svg(body, 40, 24)))[3]
    assert px(rows, 20, 6) != WHITE and px(rows, 17, 6) == WHITE  # the bare line is two pixels wide
    assert px(rows, 17, 11) != WHITE and px(rows, 23, 11) != WHITE  # the head spreads about the line


def test_few_colours_make_an_indexed_png_and_many_make_rgb(monkeypatch):
    body = '<circle class="nd f-b" cx="20" cy="10" r="8"/>'
    assert decode(raster.render_png(svg(body)))[2] == 3
    monkeypatch.setattr(raster, "LEVELS", 256)
    discs = "".join(f'<circle class="nd f-{c}" cx="{40 + 30 * i}" cy="{40 + 7 * i}" r="{30 + i}"/>' for i, c in enumerate("abcde"))
    assert decode(raster.render_png(svg(discs, 240, 120)))[2] == 2


def test_advance_widths_come_from_the_font_not_from_a_constant():
    f = ttf.font()
    assert f.advance(f.glyph("i")) < f.advance(f.glyph("n")) < f.advance(f.glyph("W"))
    assert f.glyph("é") != 0 and len(f.outline(f.glyph("é"))) >= 2  # a composite glyph


def test_figure_widths_use_the_same_metrics_as_the_drawn_text():
    fig = figures.build('kind: graph\nnode A "Instruction selection"')
    node = next(b for b in fig.boxes if b.kind == "node")
    assert abs(node.w - (ttf.font().width("Instruction selection", 13) + 2 * figures.PAD_X)) < 0.5
    assert re.search(r"Instruction selection", fig.svg)
