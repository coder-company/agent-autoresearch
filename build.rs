fn main() {
    if std::env::var_os("CARGO_CFG_TARGET_OS").as_deref() == Some("windows".as_ref()) {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
