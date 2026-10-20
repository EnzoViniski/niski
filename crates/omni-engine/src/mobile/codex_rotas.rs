//! Fork 3A: ajustes por agente e adaptação da fila existente ao protocolo estruturado.
use super::*;
use crate::codex_app_server as servidor;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/codex/app-server/ajustes", get(ler).post(gravar))
}
fn ajustes(state: &EngineState) -> Value {
    let dir = state.state_file.parent().unwrap();
    let ligados = servidor::habilitados(dir);
    let agentes:Vec<_>=omni_core::conversations::read_index(dir).iter()
        .filter(|c|c.segments.last().is_some_and(|s|s.provider=="codex"))
        .map(|c|json!({"id":c.id,"nome":omni_core::conversations::display_title(c),"ligado":ligados.contains(&c.id),
            "ativo":c.segments.last().and_then(|s|s.terminal_session_id.as_deref()).is_some_and(servidor::ativo)})).collect();
    json!({"versao_conferida":omni_core::codex_protocolo::VERSAO_CONFERIDA,"suportado":cfg!(unix),"agentes":agentes})
}
async fn ler(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(
        tokio::task::spawn_blocking(move || ajustes(&state))
            .await
            .unwrap_or(json!({})),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    agente: String,
    ligado: bool,
}
async fn gravar(
    State(state): State<Arc<EngineState>>,
    Json(body): Json<Body>,
) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        if body.ligado && !cfg!(unix) {
            return Err(error(
                StatusCode::CONFLICT,
                "App-server disponível apenas em macOS/Linux",
            ));
        }
        let dir = state.state_file.parent().unwrap();
        if !omni_core::conversations::read_index(dir).iter().any(|c| {
            c.id == body.agente && c.segments.last().is_some_and(|s| s.provider == "codex")
        }) {
            return Err(error(StatusCode::NOT_FOUND, "Agente Codex não encontrado"));
        }
        let mut ligados = servidor::habilitados(dir);
        if body.ligado {
            ligados.insert(body.agente);
        } else {
            ligados.remove(&body.agente);
        }
        servidor::gravar(dir, &ligados)
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui salvar"))?;
        Ok(Json(ajustes(&state)))
    })
    .await
    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao salvar"))?
}

pub(super) fn executar(
    state: &Arc<EngineState>,
    action: &Action,
    session: &Arc<LiveSession>,
) -> Option<Result<()>> {
    if !servidor::ativo(&action.session_id) {
        return None;
    }
    Some((|| {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions poisoned"))?;
        anyhow::ensure!(
            matches!(sessions.get(&action.session_id),Some(SessionEntry::Live(s)) if Arc::ptr_eq(s,session)),
            "Sessão trocada"
        );
        anyhow::ensure!(!session.reserved.load(Ordering::SeqCst), "Sessão reservada");
        // Não segura o registro durante uma resposta RPC: o leitor e o fallback precisam dele.
        drop(sessions);
        match &action.kind {
            ActionKind::Prompt(texto) => {
                servidor::enviar_prompt(&action.session_id, texto, &action.revision, false)
            }
            ActionKind::Fila(texto) => {
                servidor::enviar_prompt(&action.session_id, texto, &action.revision, true)
            }
            _ => Err(anyhow!(
                "Use o terminal para este comando do Codex app-server"
            )),
        }
    })())
}

pub(super) async fn aprovar(
    state: &Arc<EngineState>,
    id: &str,
    permitir: bool,
    pedido: Option<&str>,
) -> Option<Result<(StatusCode, Json<Value>), ApiError>> {
    // Prefixo distingue um pedido obsoleto do Codex de um pedido do Claude, inclusive após fallback.
    let ativo = state.sessions.lock().ok().is_some_and(|sessoes| {
        sessoes.values().any(|s| match s {
            SessionEntry::Live(s) => s.meta.lock().ok().is_some_and(|m| {
                m.conversation_id.as_deref() == Some(id) && servidor::ativo(&m.id)
            }),
            _ => false,
        })
    });
    if !ativo && !pedido.is_some_and(|p| p.starts_with("codex-")) {
        return None;
    }
    if !pedido.is_some_and(|p| p.starts_with("codex-")) {
        return Some(Err(error(
            StatusCode::PRECONDITION_FAILED,
            "Confira o pedido atual antes de responder",
        )));
    }
    let state = state.clone();
    let id = id.to_owned();
    let pedido = pedido.unwrap().to_owned();
    Some(
        tokio::task::spawn_blocking(move || {
            let (_, sessao) = current(&state, &id)?;
            let terminal = sessao
                .and_then(|s| s.meta.lock().ok().map(|m| m.id.clone()))
                .ok_or_else(|| error(StatusCode::CONFLICT, "Sessão encerrada"))?;
            servidor::responder(&terminal, &pedido, permitir).map_err(|_| {
                error(
                    StatusCode::PRECONDITION_FAILED,
                    "O pedido mudou ou já foi respondido",
                )
            })?;
            Ok((
                StatusCode::OK,
                Json(json!({"state":"sent","via":"codex_app_server"})),
            ))
        })
        .await
        .unwrap_or_else(|_| {
            Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Falha ao responder",
            ))
        }),
    )
}
