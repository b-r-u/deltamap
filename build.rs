extern crate gl_generator;

use gl_generator::{Registry, Api, Profile, Fallbacks};
use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap()).join("gles_bindings.rs");
    let mut file = File::create(&dest).unwrap();
    Registry::new(Api::Gles2, (3, 0), Profile::Core, Fallbacks::All, [])
            .write_bindings(gl_generator::StructGenerator, &mut file).unwrap();
}
