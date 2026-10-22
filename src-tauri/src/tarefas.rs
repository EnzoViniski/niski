//! Fork (Tarefas): comandos do app para as tarefas. A gravação mora no omni-core
//! (`omni_core::tarefas`), que o engine também usa nas ferramentas do copiloto.

use std::io;
use std::path::PathBuf;

pub use omni_core::tarefas as armazenamento;

fn padrao() -> Result<PathBuf, String> {
    Ok(crate::engine_client::engine_dir()?.join(armazenamento::PASTA_PADRAO))
}

fn texto_do_erro(erro: io::Error) -> String {
    if erro.kind() == io::ErrorKind::InvalidInput { erro.to_string() } else { format!("Não consegui acessar as tarefas: {erro}") }
}

#[tauri::command(async)]
pub fn tarefas_listar(pasta: Option<String>, tipo: String) -> Result<Vec<String>, String> {
    let raiz = armazenamento::raiz(pasta.as_deref(), &padrao()?).map_err(texto_do_erro)?;
    armazenamento::listar(&raiz, &tipo).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn tarefas_gravar(pasta: Option<String>, tipo: String, id: String, conteudo: String) -> Result<(), String> {
    let raiz = armazenamento::raiz(pasta.as_deref(), &padrao()?).map_err(texto_do_erro)?;
    armazenamento::gravar(&raiz, &tipo, &id, &conteudo).map_err(texto_do_erro)
}

/// Copia as tarefas locais para a pasta compartilhada recém-escolhida (sem sobrescrever as que já
/// estão lá: a pessoa que compartilhou primeiro manda). Devolve quantas foram copiadas.
#[tauri::command(async)]
pub fn tarefas_levar_para(pasta: String) -> Result<usize, String> {
    let origem = padrao()?;
    let destino = armazenamento::raiz(Some(&pasta), &origem).map_err(texto_do_erro)?;
    armazenamento::levar(&origem, &destino).map_err(texto_do_erro)
}
