# qcport's map loader (src/model/mod.c) in the draft language.
# Everything is a plain vec: no EMS, no far memory, no regions.
# Not compilable yet.

import host.pak.Pak
import render.bsp as bsp
import game.hulls as hulls
import game.ent as ent

const GEOM_W: i32 = 8192
const GEOM_MAXREC: i32 = 20 + GEOM_MAXVTX * 6

enum LoadError:
    missing(member: &string)
    short_read(member: &string)

# ---- records: layouts match tools/mkassets.py ----

bits struct FacePlane: u16
    id: u13
    chain: u3               # surface-cache chain head, bits 0-2

bits struct FaceGeom: u16
    ofs: u14
    chain: u2               # bits 3-4

bits struct FaceTex: u16
    id: u10
    chain: u6               # bits 5-10

bits struct FaceSide: u8
    back: bool
    submodel: u7

@repr("c16", pack=1)
struct Face:                # 8 bytes
    plane: FacePlane
    geom: FaceGeom
    tex: FaceTex
    geom_row: u8
    side: FaceSide

@repr("c16", pack=1)
struct Node:                # 16 bytes
    plane_id: i16
    child0: i16
    child1: i16
    lface_id: i16
    lface_num: i16
    bound: u8[6]

@repr("c16", pack=1)
struct Plane:
    norm: f32[3]
    dist: f32

@repr("c16", pack=1)
struct Submodel:            # 32 bytes
    mins: f32[3]
    maxs: f32[3]
    head_node0: i16
    head_node1: i16
    first_face: i16
    num_faces: i16

@repr("c16", pack=1)
struct Counts:
    faces: i32
    verts: i32
    edges: i32
    ledges: i32
    leaves: i32
    planes: i32
    nodes: i32
    tex_infos: i32
    clips: i32
    textures: i32
    face_lump_bytes: i32
    models: i16

# ---- the level ----

# Owns every array. Dropping it drops each vec, which frees its buffer;
# this replaces mod_unload's thirty ifs.
struct Level:
    counts: Counts
    faces: vec[Face]
    nodes: vec[Node]
    parents: vec[i16]
    planes: vec[Plane]
    models: vec[Submodel]
    leaves: bsp.Leaves
    hulls: hulls.Hulls
    ents: ent.World
    geometry: vec[u8]           # one record per face at geom_row * GEOM_W + geom_ofs
    light_atlas: vec[u8]
    colormap: vec[u8]
    pvs: vec[u8]

fn Level.load(pak: &mut Pak) -> Result[Level, LoadError]:
    let counts = pak.read[Counts]("counts.bin")?
    let nodes = pak.read_vec[Node]("nodes.pag", counts.nodes)?
    let parents = bsp.build_parents(&nodes)
    # A failing `?` drops what is built so far.
    return .ok(Level(
        counts = counts,
        faces = pak.read_vec[Face]("faces.pag", counts.faces)?,
        nodes = nodes,
        parents = parents,
        planes = pak.read_vec[Plane]("planes.bld", counts.planes)?,
        models = pak.read_vec[Submodel]("models.bld", counts.models)?,
        leaves = bsp.load_leaves(pak, &counts)?,
        hulls = hulls.load(pak, counts.clips)?,
        ents = ent.load(pak)?,
        geometry = pak.read_whole("fgeom.bin")?,
        light_atlas = pak.read_whole("lm.bin")?,
        colormap = pak.read_whole("colmap.bin")?,
        pvs = pak.read_whole("pvs.bin")?,
    ))

# The member's own size bounds the read, so a short last row cannot take
# the next member's bytes.
fn Pak.read_whole(self: &mut Pak, member: &string) -> Result[vec[u8], LoadError]:
    let (file, size) = self.seek(member) else:
        return .err(.missing(member))
    let mut bytes = vec[u8].with_length(size)
    file.read_into(&mut bytes) else:
        return .err(.short_read(member))
    return .ok(bytes)

fn Pak.read_vec[T](self: &mut Pak, member: &string, count: i32) -> Result[vec[T], LoadError]:
    let (file, _) = self.seek(member) else:
        return .err(.missing(member))
    let mut items = vec[T].with_length(count)
    file.read_into(items.bytes_mut()) else:
        return .err(.short_read(member))
    return .ok(items)

# ---- use ----

fn main() -> Result[void, LoadError]:
    let mut pak = Pak.open("quake.qpk")?
    let mut name: string = "e1m1"
    loop:
        pak.select(&name)?
        # `with` frees this level before the next one loads.
        with level = Level.load(&mut pak)?:
            name = run(&level)?
    return .ok()

# d_faces.c copies each record out because an EMS window can move; a view
# into the vec needs no copy. A record never crosses its row.
fn face_geometry(level: &Level, f: u16) -> &[u8]:
    let face = &level.faces[f]
    let ofs = i32(face.geom.ofs)
    let row = i32(face.geom_row) * GEOM_W
    return &level.geometry[row + ofs:row + min(ofs + GEOM_MAXREC, GEOM_W)]
