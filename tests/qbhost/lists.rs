//! The source lists the host tests share (tests/qb_headless.rs,
//! tests/qb_arm64.rs).

/// The headless device and the host video it draws on.
pub const DEVICE: [&str; 4] = [
    "runtime/qb/dev/headless/headless.c",
    "runtime/qb/dev/host/video.c",
    "runtime/qb/dev/host/vgadata.c",
    "runtime/qb/dev/host/tick.c",
];

/// The portable runtime, the host glue and the OS layer: what a program that
/// drives the runtime's entries links.
pub const RUNTIME: [&str; 36] = [
    "runtime/qb/console.c",
    "runtime/qb/cntext.c",
    "runtime/qb/cngfx.c",
    "runtime/qb/gfx.c",
    "runtime/qb/gline.c",
    "runtime/qb/gcirc.c",
    "runtime/qb/gpaint.c",
    "runtime/qb/gputget.c",
    "runtime/qb/gdraw.c",
    "runtime/qb/play.c",
    "runtime/qb/inkey.c",
    "runtime/qb/sleep.c",
    "runtime/qb/timer.c",
    "runtime/qb/datetime.c",
    "runtime/qb/events.c",
    "runtime/qb/screen.c",
    "runtime/qb/varptr.c",
    "runtime/qb/fout.c",
    "runtime/qb/i8out.c",
    "runtime/qb/bigint.c",
    "runtime/qb/ifout.c",
    "runtime/qb/fin.c",
    "runtime/qb/rtinit.c",
    "runtime/qb/rtterm.c",
    "runtime/qb/error.c",
    "runtime/qb/nhstutil.c",
    "runtime/qb/stcore.c",
    "runtime/qb/nheap.c",
    "runtime/qb/strcase.c",
    "runtime/qb/string.c",
    "runtime/qb/posix/startup.c",
    "runtime/qb/posix/block.c",
    "runtime/qb/posix/ad.c",
    "runtime/qb/posix/mathops.c",
    "runtime/qb/posix/paintq.c",
    "runtime/shared/posix/os.c",
];
