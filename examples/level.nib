# A map saved as packed records, loaded back with raw reads, and walked:
# which BSP leaf holds each point, and which faces its last split has.

import std.io as io

const MAGIC: u16 = 0x564C              # "LV" in the file

bits struct FaceSide: u8
    back: bool
    texture: u7

@repr("c16", pack=1)
struct Header:
    magic: u16
    planes: u16
    nodes: u16
    faces: u16

# The line normal . (x, y) == distance.
@repr("c16", pack=1)
struct Plane:
    normal: i8[2]
    distance: i16

# A child below zero is leaf -1 - child.
@repr("c16", pack=1)
struct Node:
    plane: u16
    children: i16[2]
    first_face: u16
    faces: u8

@repr("c16", pack=1)
struct Face:
    plane: u16
    side: FaceSide

enum LoadError:
    io(error: io.IoError)
    not_a_level
    short(missing: u16)

struct Level:
    planes: vec[Plane]
    nodes: vec[Node]
    faces: vec[Face]

fn read_exact(file: &mut io.File, data: *far mut u8, bytes: u16) -> Result[void, LoadError]:
    let got = file.read_raw(data, bytes)?
    if got < bytes:
        return .err(.short(bytes - got))
    return .ok()

# `count` records, each read over `blank`.
fn records[T](file: &mut io.File, count: u16, blank: T) -> Result[vec[T], LoadError]:
    let mut items: vec[T] = []
    let mut item = blank
    for _ in 0..count:
        unsafe:
            let data: *far mut T = &mut item
            read_exact(file, data.cast[u8](), size_of[T]())?
        items.push(item)
    return .ok(items)

fn written[T](file: &mut io.File, items: &vec[T]) -> Result[void, LoadError]:
    unsafe:
        let data: *far T = &items
        file.write_raw(data.cast[u8](), items.len * size_of[T]())?
    return .ok()

fn Level.save(self: &Level, path: &string) -> Result[void, LoadError]:
    let mut file = io.File.create(path)?
    let header = Header(magic=MAGIC, planes=self.planes.len, nodes=self.nodes.len, faces=self.faces.len)
    unsafe:
        let data: *far Header = &header
        file.write_raw(data.cast[u8](), size_of[Header]())?
    written(&mut file, &self.planes)?
    written(&mut file, &self.nodes)?
    written(&mut file, &self.faces)?
    return .ok()

fn Level.load(path: &string) -> Result[Level, LoadError]:
    let mut file = io.File.open(path)?
    let mut header = Header(magic=0, planes=0, nodes=0, faces=0)
    unsafe:
        let data: *far mut Header = &mut header
        read_exact(&mut file, data.cast[u8](), size_of[Header]())?
    if header.magic != MAGIC:
        return .err(.not_a_level)
    let planes = records(&mut file, header.planes, Plane(normal=[0, 0], distance=0))?
    let nodes = records(&mut file, header.nodes, Node(plane=0, children=[0, 0], first_face=0, faces=0))?
    let faces = records(&mut file, header.faces, Face(plane=0, side=FaceSide(back=false, texture=0)))?
    return .ok(Level(planes=planes, nodes=nodes, faces=faces))

# The leaf holding (x, y), and the node that split it off.
fn Level.leaf(self: &Level, x: i16, y: i16) -> (u16, u16):
    let mut child: i16 = 0
    let mut last: u16 = 0
    while child >= 0:
        last = u16(child)
        let node = &self.nodes[last]
        let plane = &self.planes[node.plane]
        let side = i16(plane.normal[0]) * x + i16(plane.normal[1]) * y - plane.distance
        child = node.children[side >= 0 ? 0 : 1]
    return (u16(-1 - child), last)

# A 64x64 room: split at x = 32, then the east half at y = 32 and the
# west half at y = 16.
fn built() -> Level:
    let planes: vec[Plane] = [
        Plane(normal=[1, 0], distance=32),
        Plane(normal=[0, 1], distance=32),
        Plane(normal=[0, 1], distance=16),
    ]
    let nodes: vec[Node] = [
        Node(plane=0, children=[1, 2], first_face=0, faces=1),
        Node(plane=1, children=[-1, -2], first_face=1, faces=2),
        Node(plane=2, children=[-3, -4], first_face=3, faces=2),
    ]
    let faces: vec[Face] = [
        Face(plane=0, side=FaceSide(back=false, texture=1)),
        Face(plane=1, side=FaceSide(back=false, texture=2)),
        Face(plane=1, side=FaceSide(back=true, texture=2)),
        Face(plane=2, side=FaceSide(back=false, texture=3)),
        Face(plane=2, side=FaceSide(back=true, texture=4)),
    ]
    return Level(planes=planes, nodes=nodes, faces=faces)

fn run() -> Result[void, LoadError]:
    built().save("LEVEL.DAT")?
    let level = Level.load("LEVEL.DAT")?
    print(f"{level.planes.len} planes {level.nodes.len} nodes {level.faces.len} faces, {size_of[Header]() + level.planes.len * size_of[Plane]() + level.nodes.len * size_of[Node]() + level.faces.len * size_of[Face]()} bytes")
    let points: vec[(i16, i16)] = [(40, 50), (40, 10), (8, 20), (8, 4)]
    for (x, y) in points:
        let (leaf, split) = level.leaf(x, y)
        let node = &level.nodes[split]
        let mut seen: string = ""
        for at in node.first_face..node.first_face + u16(node.faces):
            let face = &level.faces[at]
            seen += f" {face.side.back ? 'b' : 'f'}{face.side.texture}"
        print(f"({x},{y}) leaf {leaf} node {split}:{seen}")
    return .ok()

fn main() -> i16:
    match run():
        .ok(_):
            return 0
        .err(.io(_)):
            print("cannot read or write LEVEL.DAT")
        .err(.not_a_level):
            print("LEVEL.DAT is not a level")
        .err(.short(missing)):
            print(f"LEVEL.DAT is {missing} bytes short")
    return 1
