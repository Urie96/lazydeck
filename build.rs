use std::{env, fs, path::PathBuf};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // LuaJIT (via mlua's vendored luajit-src) calls the compiler runtime
    // builtin `__clear_cache` when syncing generated machine code.  On Termux
    // this symbol lives in Clang's compiler-rt builtins archive, but Cargo does
    // not automatically link that archive for C objects bundled in Rust crates.
    if target_os == "android" && target_arch == "aarch64" {
        if let Some(clang_rt_dir) = find_termux_clang_rt_dir() {
            println!("cargo:rustc-link-search=native={}", clang_rt_dir.display());
            println!("cargo:rustc-link-lib=static=clang_rt.builtins-aarch64-android");
        }
    }
}

fn find_termux_clang_rt_dir() -> Option<PathBuf> {
    let prefixes = [
        env::var_os("TERMUX_PREFIX").map(PathBuf::from),
        env::var_os("PREFIX").map(PathBuf::from),
        Some(PathBuf::from("/data/data/com.termux/files/usr")),
    ];

    for prefix in prefixes.into_iter().flatten() {
        let clang_dir = prefix.join("lib/clang");
        let Ok(entries) = fs::read_dir(&clang_dir) else {
            continue;
        };

        let mut versions = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("lib/linux"))
            .filter(|dir| dir.join("libclang_rt.builtins-aarch64-android.a").exists())
            .collect::<Vec<_>>();

        versions.sort();
        if let Some(dir) = versions.pop() {
            return Some(dir);
        }
    }

    None
}
