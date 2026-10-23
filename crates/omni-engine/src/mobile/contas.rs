//! Contas e uso (janela de 5 h e semanal) e o comando publicado de cada agente.
//! Fork: saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Uso das contas como o rodapé do PC mostra (janela de 5 h e semanal). Só o que já foi lido —
/// consultar de novo digitaria `/usage` num agente, e isso fica com o PC.
pub(super) async fn contas(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap().to_path_buf();
        let agora = now_ms();
        let lista: Vec<Value> = omni_core::profiles(&dir).into_iter().map(|perfil| {
            let uso = match perfil.provider.as_str() {
                "codex" => Some(crate::codex_app_server::uso_da_conta(&state,&perfil.id).unwrap_or_else(||omni_core::usage::read_codex(&perfil.id,Path::new(&perfil.config_dir),agora))), // Fork: 3A.
                // Fork (harness, C5): só a statusline; o `.claude.json` não é mais lido.
                "claude" => crate::ganchos::uso_da_statusline(&dir,&perfil.id),
                _ => None,
            };
            json!({"id":perfil.id,"provider":perfil.provider,"name":perfil.name,"usage":uso})
        }).collect();
        json!(lista)
    }).await.unwrap_or(json!([])))
}

/// O comando da CLI como o desktop publicou (`claude`, `codex`, ou um caminho), ou o id do agente.
pub(crate) fn comando_do_agente(state: &EngineState, provider: &str) -> String {
    state.mobile.workspace.lock().ok()
        .and_then(|w| w.agents.iter().find(|agente| agente.id == provider).map(|agente| agente.command.clone()))
        .unwrap_or_else(|| provider.to_string())
}

/// Busca o uso da conta de novo, como o botão de atualizar do rodapé do PC. Claude: a última statusline
/// (Fork harness, C5: nada é digitado). Codex: relê o rollout.
pub(super) async fn atualizar_uso(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap().to_path_buf();
        let perfil = omni_core::profiles(&dir).into_iter().find(|p| p.id == id)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Conta não encontrada"))?;
        let uso = match perfil.provider.as_str() {
            "claude" => match interaction::account_usage(&state,&perfil.id,true) {
                Ok(EngineResponse::AccountUsage { usage }) => usage,
                Ok(EngineResponse::Error { message, .. }) => return Err(error(StatusCode::CONFLICT,&message)),
                Ok(_) => return Err(error(StatusCode::INTERNAL_SERVER_ERROR,"Resposta inesperada da consulta de uso")),
                Err(erro) => return Err(error(StatusCode::CONFLICT,&erro.to_string())),
            },
            "codex" => crate::codex_app_server::uso_da_conta(&state,&perfil.id).unwrap_or_else(||omni_core::usage::read_codex(&perfil.id,Path::new(&perfil.config_dir),now_ms())), // Fork: 3A.
            _ => return Err(error(StatusCode::BAD_REQUEST,"Este agente não informa uso")),
        };
        Ok(Json(json!({"id":perfil.id,"provider":perfil.provider,"name":perfil.name,"usage":uso})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao consultar o uso"))?
}
