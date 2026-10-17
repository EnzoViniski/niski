use serde::Serialize;
use std::sync::Mutex;

/// Transcripts já lidos, por (caminho, mtime, tamanho): reabrir a página só relê o que mudou.
static CACHE: Mutex<Option<omni_core::relatorio::Cache>> = Mutex::new(None);

#[derive(Serialize)]
pub struct CommitsDoProjeto {
    caminho: String,
    commits: Vec<crate::git_client::CommitResumo>,
}

#[derive(Serialize)]
pub struct RelatorioIa {
    uso: omni_core::relatorio::Relatorio,
    commits: Vec<CommitsDoProjeto>,
}

/// Página Relatórios: uso de IA por dia (dos transcripts das contas) e commits dos projetos abertos
/// no período. Tudo local; nenhum texto de mensagem ou commit sai daqui, só contagens.
#[tauri::command(async)]
pub fn relatorio_ia(dias: u32, fuso_min: i32, projetos: Vec<String>) -> Result<RelatorioIa, String> {
    let dias = dias.clamp(1, 90);
    let profiles = omni_core::profiles(&crate::engine_client::engine_dir()?);
    let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let uso = {
        let mut cache = CACHE.lock().map_err(|_| "cache do relatório indisponível".to_string())?;
        omni_core::relatorio::montar(&profiles, dias, fuso_min, agora, cache.get_or_insert_with(Default::default))
    };
    let mut vistos = std::collections::HashSet::new();
    let commits = projetos.into_iter().take(50)
        .filter(|caminho| vistos.insert(caminho.replace('\\', "/").to_lowercase()))
        .map(|caminho| CommitsDoProjeto { commits: crate::git_client::commits_recentes(&caminho, dias), caminho })
        .collect();
    Ok(RelatorioIa { uso, commits })
}
