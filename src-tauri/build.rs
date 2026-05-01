fn main() {
    println!("cargo:rerun-if-env-changed=SHINDEN_BUILD_PROJECT_ROOT");
    if let Ok(project_root) = std::env::var("SHINDEN_BUILD_PROJECT_ROOT") {
        println!("cargo:rustc-env=SHINDEN_BUILD_PROJECT_ROOT={project_root}");
    }

    tauri_build::build()
}
