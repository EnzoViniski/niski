//! Rotas da aba Extensões: MCPs/plugins por conta, compartilhar e conta principal.
//! Fork: saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// MCPs, plugins e conectores por IA e conta (a aba Extensões, na web). Já vem sem segredos (ver
/// `omni_core::extensoes`). O projeto chega por id, nunca por caminho: o engine resolve pela lista
/// publicada pelo desktop.
#[derive(Deserialize)] pub(super) struct ExtensoesQuery { #[serde(default)] projeto: Option<String> }
pub(super) async fn extensoes(State(state): State<Arc<EngineState>>, Query(q): Query<ExtensoesQuery>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap().to_path_buf();
        let pasta = q.projeto.and_then(|id| known_projects(&state).into_iter().find(|p| p.id == id)).map(|p| PathBuf::from(p.path));
        Json(json!({
            "contas": omni_core::extensoes::listar(&dir, pasta.as_deref()),
            "compartilhar": omni_core::compartilhar::ativado(&dir),
        }))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler as extensões"))
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct CompartilharBody { ligado: bool }
pub(super) async fn compartilhar_extensoes(State(state): State<Arc<EngineState>>, Json(body): Json<CompartilharBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap().to_path_buf();
        omni_core::compartilhar::definir_e_levar(&dir, body.ligado)
            .map(|levado| Json(json!({"compartilhar": body.ligado, "levado": levado})))
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao salvar"))?
}

/// Estrela da aba Extensões: qual conta da IA é a principal. Só aceita conta cadastrada.
#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct PrincipalBody { provider: String, conta: String }
pub(super) async fn definir_principal(State(state): State<Arc<EngineState>>, Json(body): Json<PrincipalBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap().to_path_buf();
        omni_core::compartilhar::definir_principal(&dir, &body.provider, &body.conta)
            .map(|levado| Json(json!({"ok": true, "levado": levado})))
            .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao salvar"))?
}
