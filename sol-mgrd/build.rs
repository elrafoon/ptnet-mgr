use std::env;
use std::path::{PathBuf};

use bindgen::callbacks;

#[derive(Debug)]
pub struct DeriveCallback {
    derives: Vec<String>,
    kind: Option<callbacks::TypeKind>,
    regex_set: bindgen::RegexSet
}

impl DeriveCallback {
    fn new(derives: Vec<String>, kind: Option<callbacks::TypeKind>, regex_set: bindgen::RegexSet) -> Self {
        Self {
            derives: derives,
            kind: kind,
            regex_set: regex_set
        }
    }
}

impl callbacks::ParseCallbacks for DeriveCallback {
    fn add_derives(&self, info: &callbacks::DeriveInfo<'_>) -> Vec<String> {
        if self.kind.map(|kind| kind == info.kind).unwrap_or(true)
           && self.regex_set.matches(info.name)
        {
            return self.derives.clone();
        }
        vec![]
    }
}

fn wrap_ptlink(component: &str) {
    let wrapper = format!("{component}_wrapper.h");

    // Tell cargo to look for shared libraries in the specified directory
    //println!("cargo:rustc-link-search=/path/to/lib");

    // Tell cargo to tell rustc to link the system bzip2
    // shared library.
    //println!("cargo:rustc-link-lib=bz2");

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed={wrapper}");

    let mut re_to_serdes = bindgen::RegexSet::new();
    re_to_serdes.insert(String::from("TI232"));
    re_to_serdes.insert(String::from("TI233"));
    re_to_serdes.insert(String::from("FW_Version_A"));
    re_to_serdes.insert(String::from("HW_Version_A"));
    re_to_serdes.build(true);

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(wrapper)
        .derive_default(true)
        .derive_debug(true)
        .derive_partialeq(true)
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .parse_callbacks(Box::new(DeriveCallback::new(
            vec![String::from("serde::Serialize"),String::from("serde::Deserialize")],
            None,
            re_to_serdes
        )))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join(format!("{component}.rs")))
        .expect("Couldn't write bindings!");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    wrap_ptlink("ptnet");
    Ok(())
}
