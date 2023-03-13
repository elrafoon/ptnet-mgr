use std::collections::HashMap;
use std::env;
use std::path::{PathBuf, Path};
use std::fs;

use bindgen::callbacks;
use schema_resolve::resolve;
//use typify::{TypeSpace, TypeSpaceSettings};

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

fn resolve_sol_core_schema(schema_file_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let schema_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("dep")
        .join("sol-core")
        .join("schema");

    let mut schema_index: HashMap<String, serde_json::Value> = HashMap::new();

    for dir in fs::read_dir(schema_dir)? {
        let entry = dir?;
        let os_fname = entry.file_name();
        let fname = os_fname.as_os_str().to_str().unwrap();
        if fname.ends_with(".json") {
            let file = fs::File::open(entry.path())?;
            let schema = serde_json::from_reader(file)?;
            schema_index.insert(String::from(fname), schema);
        }
    }

    let schema = schema_index.get(schema_file_name).unwrap();

    let resolved_schema = resolve(&schema, &schema_index, true)?;

    let mut out_path = Path::new(&env::var("OUT_DIR").unwrap()).to_path_buf();
    out_path.push("schema");
    fs::create_dir_all(&out_path)?;

    out_path.push(schema_file_name);
    let out_file = fs::File::create(&out_path)?;
    serde_json::to_writer(out_file, &resolved_schema)?;

    Ok(out_path)
}

fn wrap_sol_core_schema(schema_path: &PathBuf, root_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    let contents = format!("
schemafy::schemafy!(
    root: {}
    \"{}\"
);",
        root_type,
        schema_path.as_os_str().to_str().unwrap()
    );

    let mut out_file = schema_path.clone();
    out_file.set_extension("rs");
    fs::write(out_file, contents).unwrap();

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    wrap_ptlink("ptnet");
    wrap_ptlink("ptlink_connection");

    println!("cargo:rerun-if-changed=dep/sol-core/schema/*.json");
    let res_sol_model_json = resolve_sol_core_schema("sol.model.json")?;
    wrap_sol_core_schema(&res_sol_model_json, "Solmodel")?;

    let res_sol_user_json = resolve_sol_core_schema("sol.user.json")?;
    wrap_sol_core_schema(&res_sol_user_json, "Soluser")?;

    Ok(())
}
