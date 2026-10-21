//! Fork (Editor de vídeo): projetos HyperFrames com biblioteca, briefing, preview, timeline, chat
//! do agente, exportação local e edição a dois.
//!
//! - Disco: `arquivos.rs` (gravação segura), `projetos.rs` (pasta do projeto e versões),
//!   `midias.rs` (mídia por hash) e `referencias.rs` (pastas de referências).
//! - O domínio (briefing, composição, timeline) mora no front, em `src/features/video/`.

pub mod arquivos;
pub mod midias;
pub mod projetos;
pub mod protocolo;
pub mod referencias;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn raiz() -> Result<PathBuf, String> {
    Ok(crate::engine_client::engine_dir()?.join(projetos::PASTA))
}

fn agora() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) }
fn aleatorio() -> u32 { uuid::Uuid::new_v4().as_u128() as u32 }
fn carimbo() -> String { crate::backup_antes_de_atualizar::carimbo(SystemTime::now()) }

pub fn texto_do_erro(erro: std::io::Error) -> String {
    if erro.kind() == std::io::ErrorKind::InvalidInput { erro.to_string() } else { format!("Não consegui gravar o projeto: {erro}") }
}

/// Skills do HyperFrames que vão junto com o app (`skills/video/`, ver o README de lá).
fn pasta_das_skills(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    let empacotada = app.path().resource_dir().ok().map(|p| p.join("skills").join("video")).filter(|p| p.is_dir());
    #[cfg(debug_assertions)]
    let empacotada = empacotada.or_else(|| Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("skills").join("video")).filter(|p| p.is_dir()));
    empacotada
}

fn pasta(id: &str) -> Result<PathBuf, String> { projetos::pasta_do_projeto(&raiz()?, id).map_err(texto_do_erro) }

#[tauri::command(async)]
pub fn videos_listar() -> Result<Vec<projetos::ItemDaLista>, String> { Ok(projetos::listar(&raiz()?)) }

#[tauri::command(async)]
pub fn videos_criar(app: tauri::AppHandle, novo: projetos::NovoProjeto) -> Result<String, String> {
    let skills = pasta_das_skills(&app);
    let id = projetos::criar(&raiz()?, &novo, skills.as_deref(), agora(), aleatorio()).map_err(texto_do_erro)?;
    omni_core::registro::info("video_projeto_criado", &[("skills", if skills.is_some() { "sim" } else { "nao" })]);
    Ok(id)
}

/// Abrir um projeto: confere as skills (versão do HyperFrames nova → atualiza) e devolve a pasta.
#[tauri::command(async)]
pub fn videos_abrir(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let p = pasta(&id)?;
    if !p.join(projetos::PROJETO).is_file() { return Err("Esse projeto não existe.".into()); }
    if let Some(skills) = pasta_das_skills(&app) { let _ = projetos::instalar_skills(&p, &skills); }
    Ok(p.to_string_lossy().into_owned())
}

#[tauri::command(async)]
pub fn videos_ler(id: String, arquivo: String) -> Result<Option<String>, String> {
    projetos::ler(&raiz()?, &id, &arquivo).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_gravar(id: String, arquivo: String, conteudo: String, motivo: Option<String>) -> Result<Option<u64>, String> {
    projetos::gravar(&raiz()?, &id, &arquivo, &conteudo, motivo.as_deref(), agora()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_anexar_conversa(id: String, linha: String) -> Result<(), String> {
    projetos::anexar_na_conversa(&raiz()?, &id, &linha).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_ordenar(ordem: Vec<String>) -> Result<(), String> { projetos::ordenar(&raiz()?, &ordem).map_err(texto_do_erro) }

#[tauri::command(async)]
pub fn videos_composicoes(id: String) -> Result<Vec<String>, String> { projetos::composicoes(&raiz()?, &id).map_err(texto_do_erro) }

#[tauri::command(async)]
pub fn videos_versoes(id: String) -> Result<Vec<serde_json::Value>, String> { projetos::listar_versoes(&raiz()?, &id).map_err(texto_do_erro) }

#[tauri::command(async)]
pub fn videos_criar_versao(id: String, motivo: String) -> Result<Option<u64>, String> {
    projetos::criar_versao(&raiz()?, &id, &motivo, agora()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_restaurar_versao(id: String, n: u64) -> Result<Option<u64>, String> {
    projetos::restaurar_versao(&raiz()?, &id, n, agora()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_apagar(id: String) -> Result<(), String> {
    projetos::mandar_para_a_lixeira(&raiz()?, &id, &carimbo()).map(|_| ()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_importar_midia(id: String, caminho: String, papel: String, nota: Option<String>) -> Result<serde_json::Value, String> {
    let p = pasta(&id)?;
    if !p.join(projetos::PROJETO).is_file() { return Err("Esse projeto não existe.".into()); }
    let item = midias::importar(&p.join(projetos::MIDIAS), &p.join("assets"), "assets/", Path::new(&caminho), &papel, nota.as_deref(), agora()).map_err(texto_do_erro)?;
    omni_core::registro::info("video_midia_importada", &[("tipo", item["tipo"].as_str().unwrap_or("outro"))]);
    Ok(item)
}

#[tauri::command(async)]
pub fn videos_remover_midia(id: String, item: String) -> Result<bool, String> {
    let r = raiz()?;
    let p = pasta(&id)?;
    midias::remover(&p.join(projetos::MIDIAS), &p.join("assets"), "assets/", &item, &r.join(projetos::LIXEIRA).join("midias"), &carimbo()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_midias_faltando(id: String) -> Result<Vec<serde_json::Value>, String> {
    let p = pasta(&id)?;
    Ok(midias::faltando(&p.join(projetos::MIDIAS), &p.join("assets"), "assets/"))
}

/// Antes de o front migrar o formato de um projeto: cópia em `~/omni-backups/videos/`.
#[tauri::command(async)]
pub fn videos_backup(id: String, motivo: String) -> Result<String, String> {
    let casa = std::env::var_os("HOME").map(PathBuf::from).ok_or("Pasta do usuário indisponível")?;
    let destino = projetos::copiar_para_backups(&raiz()?, &id, &motivo, &casa.join("omni-backups"), &carimbo()).map_err(texto_do_erro)?;
    omni_core::registro::info("video_backup", &[("motivo", &motivo)]);
    Ok(destino.to_string_lossy().into_owned())
}

#[tauri::command(async)]
pub fn videos_ref_listar() -> Result<Vec<serde_json::Value>, String> { Ok(referencias::listar(&raiz()?)) }

#[tauri::command(async)]
pub fn videos_ref_criar(dados: serde_json::Value) -> Result<serde_json::Value, String> {
    referencias::criar(&raiz()?, &dados, agora(), aleatorio()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_ref_editar(id: String, dados: serde_json::Value) -> Result<serde_json::Value, String> {
    referencias::editar(&raiz()?, &id, &dados, agora()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_ref_ler(id: String) -> Result<serde_json::Value, String> { referencias::ler(&raiz()?, &id).map_err(texto_do_erro) }

#[tauri::command(async)]
pub fn videos_ref_importar(id: String, caminho: String, papel: String, nota: Option<String>) -> Result<serde_json::Value, String> {
    referencias::importar(&raiz()?, &id, Path::new(&caminho), &papel, nota.as_deref(), agora()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_ref_remover_item(id: String, item: String) -> Result<bool, String> {
    referencias::remover_item(&raiz()?, &id, &item, &carimbo()).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn videos_ref_apagar(id: String) -> Result<(), String> {
    referencias::mandar_para_a_lixeira(&raiz()?, &id, &carimbo()).map(|_| ()).map_err(texto_do_erro)
}

/// Põe tudo de uma pasta de referências no projeto (arquivos e links). Devolve os itens copiados.
#[tauri::command(async)]
pub fn videos_ref_aplicar(ref_id: String, projeto: String) -> Result<Vec<serde_json::Value>, String> {
    let r = raiz()?;
    let origem = referencias::existente(&r, &ref_id).map_err(texto_do_erro)?;
    let destino = pasta(&projeto)?;
    if !destino.join(projetos::PROJETO).is_file() { return Err("Esse projeto não existe.".into()); }
    midias::copiar_itens(&origem.join(referencias::PASTA_JSON), &origem.join(referencias::ARQUIVOS), referencias::PREFIXO,
        &destino.join(projetos::MIDIAS), &destino.join("assets"), "assets/", None, agora()).map_err(texto_do_erro)
}

/// "Salvar como pasta de referências": cria uma pasta nova com as mídias e links do projeto.
#[tauri::command(async)]
pub fn videos_ref_de_projeto(projeto: String, dados: serde_json::Value) -> Result<serde_json::Value, String> {
    let r = raiz()?;
    let origem = pasta(&projeto)?;
    let nova = referencias::criar(&r, &dados, agora(), aleatorio()).map_err(texto_do_erro)?;
    let id = nova["id"].as_str().unwrap_or_default().to_owned();
    let destino = referencias::existente(&r, &id).map_err(texto_do_erro)?;
    midias::copiar_itens(&origem.join(projetos::MIDIAS), &origem.join("assets"), "assets/",
        &destino.join(referencias::PASTA_JSON), &destino.join(referencias::ARQUIVOS), referencias::PREFIXO, None, agora()).map_err(texto_do_erro)?;
    referencias::ler(&r, &id).map_err(texto_do_erro)
}

#[cfg(test)]
mod testes;
