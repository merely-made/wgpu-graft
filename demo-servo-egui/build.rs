fn main() {
    // Only needed on Windows — Servo forces ANGLE (libEGL/libGLESv2) which must
    // be present next to the executable at runtime.
    #[cfg(target_os = "windows")]
    copy_angle_dlls_to_binary_dir();
}

#[cfg(target_os = "windows")]
fn copy_angle_dlls_to_binary_dir() {
    use std::path::Path;

    // OUT_DIR = {target}/{profile}/build/{crate}-{hash}/out
    // Binary  = {target}/{profile}/
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let binary_dir = Path::new(&out_dir)
        .parent()
        .unwrap() // strip "out"
        .parent()
        .unwrap() // strip "{crate}-{hash}"
        .parent()
        .unwrap(); // strip "build"

    let build_dir = binary_dir.join("build");

    let dlls = ["libEGL.dll", "libGLESv2.dll"];
    let newest_out_dir = std::fs::read_dir(&build_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("mozangle-"))
        .map(|entry| entry.path().join("out"))
        .filter(|out_dir| dlls.iter().all(|dll| out_dir.join(dll).is_file()))
        .max_by_key(|out_dir| {
            dlls.iter()
                .filter_map(|dll| std::fs::metadata(out_dir.join(dll)).ok()?.modified().ok())
                .max()
                .unwrap_or(std::time::UNIX_EPOCH)
        });

    if let Some(out_dir) = newest_out_dir {
        for dll in dlls {
            let src = out_dir.join(dll);
            let dest = binary_dir.join(dll);
            if let Err(e) = std::fs::copy(&src, &dest) {
                println!("cargo:warning=Failed to copy {dll} to binary dir: {e}");
            }
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
}
