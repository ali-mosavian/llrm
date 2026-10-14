//! Where a stage dump may go. A dump is hundreds of small files for each
//! function; on tmpfs it ran /tmp out of inodes and stopped every session's
//! shell. The command-line tools refuse a dump directory in memory.

use std::path::{Path, PathBuf};

/// The dump directories a command line asks for: `--dump DIR` and
/// `LLRM_MIR_STAGES`.
fn asked(
    args: &[String],
    stages: Option<&std::ffi::OsStr>,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> =
        args.windows(2).filter(|pair| pair[0] == "--dump").map(|pair| PathBuf::from(&pair[1])).collect();
    out.extend(stages.map(PathBuf::from));
    out
}

/// Why the tool should stop, if a dump of `args` (or `LLRM_MIR_STAGES`) would
/// be written on tmpfs.
pub fn refuse(args: &[String]) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if let Ok(mounts) = std::fs::read_to_string("/proc/mounts") {
        for directory in asked(args, std::env::var_os("LLRM_MIR_STAGES").as_deref()) {
            if let Some(kind) = memory_filesystem(&mounts, &existing(&directory)) {
                return Err(format!(
                    "{} is on {kind}: a dump is thousands of small files and exhausts its inodes. Use a directory on disk",
                    directory.display()
                ));
            }
        }
    }
    let _ = args;
    Ok(())
}

/// The real path of `directory`, or of its nearest ancestor that exists: the
/// dump has not made it yet.
#[cfg(target_os = "linux")]
fn existing(directory: &Path) -> PathBuf {
    let absolute = std::path::absolute(directory).unwrap_or_else(|_| directory.to_path_buf());
    absolute
        .ancestors()
        .find_map(|one| {
            one.canonicalize().ok().map(|real| real.join(absolute.strip_prefix(one).unwrap_or(Path::new(""))))
        })
        .unwrap_or(absolute)
}

/// The kind of the filesystem that holds `path`, if it lives in memory:
/// the longest mount point in `/proc/mounts` text that `path` is under.
fn memory_filesystem<'a>(
    mounts: &'a str,
    path: &Path,
) -> Option<&'a str> {
    let (_, kind) = mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (_device, point, kind) = (fields.next()?, fields.next()?, fields.next()?);
            let point = point.replace("\\040", " ");
            path.starts_with(&point).then_some((point.len(), kind))
        })
        .max_by_key(|(length, _)| *length)?;
    ["tmpfs", "ramfs"].contains(&kind).then_some(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "/dev/nvme0n1p4 / ext4 rw 0 0\ntmpfs /tmp tmpfs rw,nr_inodes=1048576 0 0\n\
                          tmpfs /dev/shm tmpfs rw 0 0\n/dev/sdb1 /tmp/disk ext4 rw 0 0\n";

    /// A dump into /tmp ran it out of inodes (nr_inodes=1048576) and no shell
    /// could start; the directory is refused where it is in memory.
    #[test]
    fn a_dump_directory_on_tmpfs_is_refused() {
        assert_eq!(memory_filesystem(MOUNTS, Path::new("/tmp/dump")), Some("tmpfs"));
        assert_eq!(memory_filesystem(MOUNTS, Path::new("/dev/shm/x/y")), Some("tmpfs"));
    }

    /// The longest mount point decides: a disk mounted under /tmp is disk.
    #[test]
    fn a_disk_under_tmpfs_and_the_root_are_fine() {
        assert_eq!(memory_filesystem(MOUNTS, Path::new("/tmp/disk/dump")), None);
        assert_eq!(memory_filesystem(MOUNTS, Path::new("/home/alim/dump")), None);
    }

    /// Both ways to ask for a dump are read: the option and the environment.
    #[test]
    fn the_option_and_the_variable_both_name_a_dump() {
        let args = ["x.c", "--dump", "/tmp/a", "-O2"].map(String::from);
        let stages = std::ffi::OsString::from("/dev/shm/b");
        assert_eq!(asked(&args, Some(&stages)), [PathBuf::from("/tmp/a"), PathBuf::from("/dev/shm/b")]);
        assert!(asked(&["x.c".to_owned()], None).is_empty());
    }
}
