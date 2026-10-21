//! Fork (Roadmaps de estudo): trilhas de estudo criadas por entrevista com a IA.
//! Arquivos em `armazenamento.rs`; a IA (Claude/Codex sem tela, na conta escolhida) em `ia.rs`.
//! O domínio (plano, progresso, cards FSRS, prompts) mora no front, em `src/features/roadmaps/`.

pub mod armazenamento;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn raiz() -> Result<PathBuf, String> {
    Ok(crate::engine_client::engine_dir()?.join(armazenamento::PASTA))
}

fn texto_do_erro(erro: std::io::Error) -> String {
    if erro.kind() == std::io::ErrorKind::InvalidInput { erro.to_string() } else { format!("Não consegui gravar o roadmap: {erro}") }
}

#[tauri::command(async)]
pub fn roadmaps_listar() -> Result<Vec<armazenamento::ItemDaLista>, String> {
    Ok(armazenamento::listar(&raiz()?))
}

#[tauri::command(async)]
pub fn roadmaps_criar(meta: serde_json::Value) -> Result<String, String> {
    let agora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let aleatorio = uuid::Uuid::new_v4().as_u128() as u32;
    armazenamento::criar(&raiz()?, &meta, agora, aleatorio).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn roadmaps_ler(id: String, arquivo: String) -> Result<Option<String>, String> {
    armazenamento::ler(&raiz()?, &id, &arquivo).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn roadmaps_gravar(id: String, arquivo: String, conteudo: String) -> Result<(), String> {
    armazenamento::gravar(&raiz()?, &id, &arquivo, &conteudo).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn roadmaps_anexar_conversa(id: String, linha: String) -> Result<(), String> {
    armazenamento::anexar_na_conversa(&raiz()?, &id, &linha).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn roadmaps_ordenar(ordem: Vec<String>) -> Result<(), String> {
    armazenamento::ordenar(&raiz()?, &ordem).map_err(texto_do_erro)
}

/// Antes de o front migrar o formato de um roadmap: cópia em `~/omni-backups/roadmaps/`.
#[tauri::command(async)]
pub fn roadmaps_backup_antes_de_migrar(id: String, versao_antiga: u32) -> Result<String, String> {
    let casa = std::env::var_os("HOME").map(PathBuf::from).ok_or("Pasta do usuário indisponível")?;
    let carimbo = crate::backup_antes_de_atualizar::carimbo(SystemTime::now());
    let destino = armazenamento::copiar_antes_de_migrar(&raiz()?, &id, versao_antiga, &casa.join("omni-backups"), &carimbo).map_err(texto_do_erro)?;
    omni_core::registro::info("roadmap_backup_de_migracao", &[("versao", &versao_antiga.to_string())]);
    Ok(destino.to_string_lossy().into_owned())
}
