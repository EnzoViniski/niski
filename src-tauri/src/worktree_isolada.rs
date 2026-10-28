//! "Isolar em worktree" no Novo agente e o aviso antes de apagar. A lógica de git mora em
//! `omni_core::worktree_isolada` (a mesma que o `criar_agente` do orquestrador usa no engine).

use omni_core::worktree_isolada::{self, Estado, WorktreeIsolada};
use std::path::Path;

/// Cria `<pai do repo>/.medza-worktrees/<repo>/<nome>` na branch `medza/<nome>` e devolve onde o agente roda.
#[tauri::command(async)]
pub fn criar_worktree_isolada(project_path: String, nome: String) -> Result<WorktreeIsolada, String> {
    if omni_core::targets::resolve(&project_path, &crate::ssh::connections()).remote_path().is_some() {
        return Err("Worktree isolada só funciona em projeto local".into());
    }
    let nome = if nome.trim().is_empty() { "agente".to_owned() } else { nome };
    worktree_isolada::criar(Path::new(&project_path), &nome)
}

/// Branch e mudanças não commitadas, para o aviso ao encerrar o agente. `None`: a pasta não é uma
/// worktree isolada (ou já foi apagada).
#[tauri::command(async)]
pub fn estado_da_worktree_isolada(caminho: String) -> Option<Estado> {
    worktree_isolada::estado(Path::new(&caminho)).ok()
}

/// Apaga a pasta da worktree (a branch fica). Com mudanças não commitadas, só com `forcar`.
#[tauri::command(async)]
pub fn remover_worktree_isolada(caminho: String, forcar: bool) -> Result<(), String> {
    worktree_isolada::remover(Path::new(&caminho), forcar)
}
