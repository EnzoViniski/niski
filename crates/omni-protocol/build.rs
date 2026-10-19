//! Lê `identidade.json` (raiz do repositório) e expõe cada campo como variável de compilação
//! (`OMNI_ID_*`), lida pelas constantes de `lib.rs`. Um lugar só para nome, bundle id e portas: um
//! fork troca o arquivo, não o código.

use std::{env, fs, path::PathBuf};

fn main() {
    let arquivo = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../identidade.json");
    println!("cargo:rerun-if-changed={}", arquivo.display());
    let texto = fs::read_to_string(&arquivo).unwrap_or_else(|erro| panic!("{}: {erro}", arquivo.display()));
    let id: serde_json::Value = serde_json::from_str(&texto).expect("identidade.json inválido");
    let texto_de = |campo: &str| id[campo].as_str().unwrap_or_else(|| panic!("identidade.json: `{campo}` precisa ser texto")).to_owned();
    let porta_de = |campo: &str| match id[campo].as_u64() {
        Some(porta @ 1..=65_535) => porta.to_string(),
        _ => panic!("identidade.json: `{campo}` precisa ser uma porta (1–65535)"),
    };
    for (variavel, valor) in [
        ("OMNI_ID_NOME", texto_de("nome")),
        ("OMNI_ID_BUNDLE", texto_de("bundleId")),
        ("OMNI_ID_PERFIS", texto_de("pastaDePerfis")),
        ("OMNI_ID_PORTA", porta_de("portaDoEngine")),
        ("OMNI_ID_PORTA_DEV", porta_de("portaDeDev")),
        ("OMNI_ID_PORTA_CELULAR", porta_de("portaDoCelular")),
    ] {
        println!("cargo:rustc-env={variavel}={valor}");
    }
}
