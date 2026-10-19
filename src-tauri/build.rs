fn main() {
    // Triple do alvo para achar `binaries/omni-engine-<triple>` em dev (convenção de sidecar do Tauri).
    println!("cargo:rustc-env=OMNI_TARGET_TRIPLE={}", std::env::var("TARGET").unwrap());
    // Fork: o backend Metal do whisper.cpp usa `@available`, que chama `__isPlatformVersionAtLeast`
    // do runtime do clang. O linker do Rust não traz essa lib sozinho; sem ela o link falha.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        if let Ok(saida) = std::process::Command::new("clang").arg("--print-resource-dir").output() {
            let base = String::from_utf8_lossy(&saida.stdout).trim().to_string();
            println!("cargo:rustc-link-search=native={base}/lib/darwin");
            println!("cargo:rustc-link-lib=static=clang_rt.osx");
        }
    }
    tauri_build::build()
}
