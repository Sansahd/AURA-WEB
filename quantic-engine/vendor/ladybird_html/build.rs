use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=Entities.json");
    let raw = fs::read_to_string("Entities.json").expect("read Ladybird Entities.json");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("parse entities");
    let object = value.as_object().expect("entity map");

    let mut entries = Vec::with_capacity(object.len());
    for (name, data) in object {
        let clean = name.strip_prefix('&').unwrap_or(name);
        let cps = data
            .get("codepoints")
            .and_then(|v| v.as_array())
            .expect("codepoints array");
        let first = cps.first().and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let second = cps.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        entries.push((clean.to_string(), first, second));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::from("pub static ENTITIES: &[(&str, u32, u32)] = &[\n");
    for (name, first, second) in entries {
        out.push_str(&format!("    ({:?}, {}, {}),\n", name, first, second));
    }
    out.push_str("];\n");

    let path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("entities_generated.rs");
    fs::write(path, out).expect("write entity table");
}
