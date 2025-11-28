use std::{env, fs};

fn main() {
    fs::write(
        format!("{}/target.txt", env::var("OUT_DIR").unwrap()),
        env::var("TARGET").unwrap(),
    )
    .unwrap();
}
