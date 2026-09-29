use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalog/tools");
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    paths.sort();
    let mut generated = String::from("pub const EMBEDDED_TOOLS: &[&str] = &[\n");
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.display());
        generated.push_str(&format!("include_str!({:?}),\n", path.to_string_lossy()));
    }
    generated.push_str("];\n");
    let output = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(output.join("embedded_tools.rs"), generated).expect("write embedded tools");
}

fn walk(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read catalog tools") {
        let path = entry.expect("catalog entry").path();
        if path.is_dir() {
            walk(&path, paths);
        } else if path.extension().is_some_and(|ext| ext == "yaml") {
            paths.push(path);
        }
    }
}
