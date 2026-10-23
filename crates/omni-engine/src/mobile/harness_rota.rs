//! Fork (harness, A6): `GET /conversas/{id}/harness` — o estado do agente como o harness o conhece, com a
//! fonte de cada parte: `evento` (ganchos do Claude, `notify` do Codex), `app_server` (Codex pelo protocolo)
//! ou `tela` (leitura do terminal). Só metadados: nomes, números e horas; nada de texto da conversa.
//!
//! Serve à tela e ao cartão "onde parei" da troca de contexto (worktree `troca-de-contexto`), que pode ler
//! esta rota em vez de adivinhar pelo terminal.
use super::*;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/conversas/{id}/harness", get(estado))
}

async fn estado(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let (conversa, sessao) = current(&state, &id)?;
        let Some(sessao) = sessao else { return Ok(Json(json!({"conversa": conversa.id, "aberta": false}))) };
        Ok(Json(montar(&conversa.id, &sessao)))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler a conversa"))?
}

pub(crate) fn montar(conversa: &str, sessao: &Arc<LiveSession>) -> Value {
    let meta = settle(sessao);
    let fonte = if crate::codex_app_server::metadata(sessao).is_some() { "app_server" }
        else if crate::ganchos::em_turno(&meta.id).is_some() || crate::ganchos::turnos_por_evento(&meta.id).is_some() { "evento" }
        else { "tela" };
    json!({
        "conversa": conversa,
        "aberta": true,
        "provider": meta.provider,
        "estado": meta.state,
        "aviso": meta.notice,
        "fonte_do_estado": fonte,
        "eventos": crate::ganchos::estado_do_harness(&meta.id),
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sem_eventos_a_fonte_e_a_tela_e_nada_de_conteudo_sai() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
            token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
            id_sequence: AtomicU64::new(1) });
        let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
        atomic_write_json(&dir.path().join("conversations.json"), &json!({"version": 1, "conversations": [{"id": "conv", "project_id": "p",
            "cwd": dir.path().to_string_lossy(), "title": "conv", "created_at_ms": 1, "segments": [{"provider": "codex", "profile_id": null,
            "external_session_id": null, "transcript_path": null, "terminal_session_id": terminal, "started_at_ms": 1, "ended_at_ms": null}]}]})).unwrap();
        spawn_terminal_inner(&state, terminal.clone(), "p".into(), "Codex".into(), dir.path().to_string_lossy().into_owned(), Some("/bin/sh".into()),
            None, 30, 100, now_ms(), SessionOrigin { provider: Some("codex".into()), conversation_id: Some("conv".into()), ..SessionOrigin::default() }).unwrap();
        let (status, corpo) = chamar(&state, "/conversas/conv/harness").await;
        assert_eq!(status, 200);
        assert_eq!((corpo["aberta"].as_bool(), corpo["fonte_do_estado"].as_str()), (Some(true), Some("tela")), "{corpo}");
        assert!(corpo["eventos"].is_null());

        // Com o notify do Codex e um fim de turno por evento: fonte `evento`, contadores e nada de texto.
        crate::ganchos::definir_porta(&state, 47_995);
        crate::ganchos::registrar_codex(&state, &terminal, None).unwrap();
        let (_, corpo) = chamar(&state, "/conversas/conv/harness").await;
        assert_eq!(corpo["eventos"]["canal"], "notify", "{corpo}");
        assert_eq!(corpo["eventos"]["confirmado"], false);
        let (status, _) = chamar(&state, "/conversas/nao-existe/harness").await;
        assert_eq!(status, 404);
        crate::ganchos::esquecer(&terminal);
        let _ = close_session(&state, &terminal);
    }

    async fn chamar(state: &Arc<EngineState>, rota: &str) -> (u16, Value) {
        let EngineResponse::Api { status, body, .. } = local(state.clone(), "GET", rota, None, None, vec![]).await else { panic!("api") };
        (status, serde_json::from_str(&body).unwrap_or(Value::Null))
    }
}
