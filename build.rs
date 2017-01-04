extern crate gcc;

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::process::Command;

fn main()
{
    let target = env::var("TARGET").unwrap();
    let host = env::var("HOST").unwrap();

    // If we're cross-compiling, or not compiling on and for Solaris, bail.
    if target != host || !target.contains("solaris") {
        return;
    }

    let mut dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    dst.push("config.c");

    // create and write to the file
    let mut f = File::create(&dst).unwrap();
    f.write(b"char flock(); int main() { return flock(); }\n").unwrap();
    f.flush().unwrap();

    // Compile and link; if it's successful, tell the build system we have flock().
    let compiler = gcc::Config::new().get_compiler();
    let output = Command::new(compiler.path())
        .current_dir(dst.parent().unwrap())
        .args(&["-o", "config", "config.c"])
        .output()
        .expect("failed to execute process");
    if output.status.success() {
        println!("cargo:rustc-cfg=HAVE_FLOCK");
    }
}
