use std::env;
use std::path::PathBuf;
use std::process::Command;

fn run(mut cmd: Command) {
    let st = cmd.status().unwrap_or_else(|e| panic!("{}: {}", cmd.get_program().to_string_lossy(), e));
    if !st.success() {
        panic!("{} failed", cmd.get_program().to_string_lossy());
    }
}

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let c = root.join("c");
    let src = root.join("src");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let version = "2024-01-13";
    let cc = env::var("CC").unwrap_or_else(|_| "cc".into());
    let mut objs: Vec<PathBuf> = Vec::new();
    for f in [
        "cutils.c",
        "libregexp.c",
        "libunicode.c",
        "libbf.c",
        "quickjs.c",
        "shim.c",
    ] {
        let dir = if f == "shim.c" { &src } else { &c };
        let obj = out.join(format!("{}.o", f));
        let mut cmd = Command::new(&cc);
        cmd.args([
            "-O2",
            "-std=c99",
            "-fno-strict-aliasing",
            "-w",
            "-D_GNU_SOURCE",
            &format!("-DCONFIG_VERSION=\"{}\"", version),
            "-I",
        ])
        .arg(&c)
        .arg("-c")
        .arg(dir.join(f))
        .arg("-o")
        .arg(&obj);
        run(cmd);
        objs.push(obj);
    }
    let lib = out.join("libbreakyqjs.a");
    let mut cmd = Command::new("ar");
    cmd.arg("rcs").arg(&lib);
    for o in &objs {
        cmd.arg(o);
    }
    run(cmd);
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=breakyqjs");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rerun-if-changed=c/");
    println!("cargo:rerun-if-changed=src/shim.c");
}
