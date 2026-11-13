use std::env;

fn main() {
    println!("cargo:rerun-if-changed=src/getauxval.c");
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("arm") {
        cc::Build::new()
            .file("src/getauxval.c")
            .warnings(false)
            .compile("getauxval");
        let out = env::var("OUT_DIR").expect("OUT_DIR");
        println!("cargo:rustc-link-arg={out}/libgetauxval.a");
    }
}
