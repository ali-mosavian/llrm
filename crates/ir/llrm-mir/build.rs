//! Records in the library how it was built: a binary that carries
//! `llrm-build:incremental` was built with incremental compilation, which moves
//! the inlining of unrelated code by half a point an edit, so no instruction or
//! cycle count of it compares with another's. Cargo does not tell a build
//! script its profile's `incremental`, so it is read as the tools' rule says:
//! `CARGO_INCREMENTAL=0` is clean, any other value incremental, and with none
//! the `dist` profile is clean and every other incremental
//! (`[profile.release]`).

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_INCREMENTAL");
    let profile = std::env::var_os("OUT_DIR")
        .map(std::path::PathBuf::from)
        .and_then(|out| {
            out.ancestors().nth(3).and_then(|dir| dir.file_name().map(|name| name.to_string_lossy().into_owned()))
        })
        .unwrap_or_default();
    let incremental = match std::env::var("CARGO_INCREMENTAL").as_deref() {
        Ok("0") => false,
        Ok(_) => true,
        Err(_) => profile != "dist",
    };
    println!("cargo:rustc-env=LLRM_BUILD_KIND=profile={profile};incremental={}", u8::from(incremental));
}
