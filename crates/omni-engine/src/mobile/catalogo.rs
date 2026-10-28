//! Workspace publicado pelo desktop (projetos, agentes, tema) e o catálogo que o celular lê.
//! Saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

pub fn publish(state: &EngineState, projects: Vec<PublishedProject>, agents: Vec<PublishedAgent>, theme: Option<PublishedTheme>) -> Result<EngineResponse> {
    // Desktop antigo publica sem tema: mantém o último conhecido em vez de apagar.
    let theme = theme.or_else(|| state.mobile.workspace.lock().ok().and_then(|w| w.theme.clone()));
    let published = PublishedWorkspace { projects, agents, theme, published_at_ms: now_ms() };
    atomic_write_json(&state.state_file.with_file_name("workspace.json"),&published)?;
    *state.mobile.workspace.lock().map_err(|_|anyhow!("workspace poisoned"))? = published;
    Ok(EngineResponse::Ok)
}

/// Projetos que o celular pode escolher: os publicados pelo desktop, os do índice do OMNI e os
/// encontrados nos históricos nativos do Claude/Codex.
pub(super) fn known_projects(state: &EngineState) -> Vec<PublishedProject> {
    let dir = state.state_file.parent().unwrap();
    let mut projects = state.mobile.workspace.lock().expect("workspace poisoned").projects.clone();
    for conversation in omni_core::conversations::read_index(dir) {
        if projects.iter().any(|p| p.id == conversation.project_id || same_path(&p.path,&conversation.cwd)) { continue }
        let name = Path::new(&conversation.cwd).file_name().map(|n|n.to_string_lossy().into_owned())
            .unwrap_or_else(|| conversation.cwd.clone());
        projects.push(PublishedProject { id: conversation.project_id.clone(), name, path: conversation.cwd.clone() });
    }
    for entry in historical_entries(state) {
        let cwd = entry.cwd.unwrap_or_default();
        if projects.iter().any(|project|same_path(&project.path,&cwd)) { continue }
        let name = Path::new(&cwd).file_name().filter(|name|!name.is_empty()).map(|name|name.to_string_lossy().into_owned())
            .unwrap_or_else(||"Histórico".into());
        projects.push(PublishedProject { id: history_project_id(&cwd), name, path: cwd });
    }
    projects
}

pub(super) fn catalogo(state: &EngineState) -> Value {
    let dir = state.state_file.parent().unwrap();
    let (agents,published_at_ms,theme) = {
        let workspace = state.mobile.workspace.lock().expect("workspace poisoned");
        (workspace.agents.clone(),workspace.published_at_ms,workspace.theme.clone())
    };
    // `config_dir` fica de fora de propósito: é caminho de credencial e o celular não precisa.
    let profiles: Vec<_> = omni_core::profiles(dir).into_iter()
        .map(|p| json!({"id":p.id,"provider":p.provider,"name":p.name})).collect();
    json!({"published_at_ms":published_at_ms,"projects":known_projects(state),"agents":agents,"profiles":profiles,"theme":theme})
}

pub(super) async fn projects(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || catalogo(&state))
        .await.unwrap_or(json!({"published_at_ms":0,"projects":[],"agents":[],"profiles":[]})))
}
