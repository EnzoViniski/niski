//! Fork (harness, A5): o orquestrador sabe que o subagente terminou o turno **pelo evento** (o `Stop` do
//! Claude, o `notify` do Codex) em vez de esperar as janelas da leitura de tela. Só acelera: sem eventos
//! (ganchos desligados, pasta sem confiança, outra CLI, Esc no meio do turno) vale a regra de antes.
use super::*;
use std::sync::LazyLock;

/// Subagente → (terminal, turnos terminados por evento na hora do envio).
static NO_ENVIO: LazyLock<Mutex<HashMap<String, (String, u64)>>> = LazyLock::new(Default::default);

/// Chamado quando a mensagem foi digitada. Sem eventos confirmados ainda, a base é zero: nenhum evento
/// chegou, então nenhum turno terminou por evento.
pub(super) fn marcar_envio(agente: &str, terminal: &str) {
    let base = crate::ganchos::turnos_por_evento(terminal).map(|(_, terminados)| terminados).unwrap_or(0);
    if let Ok(mut mapa) = NO_ENVIO.lock() { mapa.insert(agente.to_owned(), (terminal.to_owned(), base)); }
}

/// `Some(true)`: um turno terminou por evento depois do último envio. `None`: sem eventos nesta PTY (ou
/// outra PTY depois de reabrir): quem chama usa a tela.
pub(super) fn terminou_desde_envio(agente: &str, terminal: &str) -> Option<bool> {
    let (dono, base) = NO_ENVIO.lock().ok()?.get(agente).cloned()?;
    if dono != terminal { return None; }
    crate::ganchos::turnos_por_evento(terminal).map(|(_, terminados)| terminados > base)
}

pub(super) fn esquecer(agente: &str) {
    if let Ok(mut mapa) = NO_ENVIO.lock() { mapa.remove(agente); }
}

#[cfg(all(test, unix))]
mod testes {
    use super::*;
    use axum::body::Body;
    use tower::ServiceExt;

    /// Turno terminado pelo `notify` do Codex (POST na rota, como o shim faria) depois do envio: `Some(true)`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fim_do_turno_por_evento_depois_do_envio() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
            token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
            id_sequence: AtomicU64::new(1) });
        let porta = 47_996;
        crate::ganchos::definir_porta(&state, porta);
        let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
        spawn_terminal_inner(&state, terminal.clone(), "p".into(), "Sub".into(), dir.path().to_string_lossy().into_owned(), Some("/bin/sh".into()),
            None, 30, 100, now_ms(), SessionOrigin { provider: Some("codex".into()), ..SessionOrigin::default() }).unwrap();
        let (_, token) = crate::ganchos::registrar_codex(&state, &terminal, None).unwrap();
        let agente = format!("sub-{}", uuid::Uuid::new_v4());
        assert_eq!(terminou_desde_envio(&agente, &terminal), None, "sem envio, nada a dizer");
        marcar_envio(&agente, &terminal);
        assert_eq!(terminou_desde_envio(&agente, &terminal), None, "sem evento confirmado, vale a tela");
        let notificar = || {
            let pedido = axum::http::Request::builder().method("POST").uri("/ganchos/codex").header("host", format!("127.0.0.1:{porta}"))
                .header("authorization", format!("Bearer {token}")).header("content-type", "application/json")
                .body(Body::from(r#"{"type":"agent-turn-complete","thread-id":"th","turn-id":"t"}"#)).unwrap();
            crate::ganchos::router(state.clone()).oneshot(pedido)
        };
        assert_eq!(notificar().await.unwrap().status(), 200);
        assert_eq!(terminou_desde_envio(&agente, &terminal), Some(true));
        marcar_envio(&agente, &terminal);
        assert_eq!(terminou_desde_envio(&agente, &terminal), Some(false), "a base anda a cada envio");
        assert_eq!(terminou_desde_envio(&agente, "outro-terminal"), None, "PTY reaberta: vale a tela");
        notificar().await.unwrap();
        assert_eq!(terminou_desde_envio(&agente, &terminal), Some(true));
        esquecer(&agente);
        assert_eq!(terminou_desde_envio(&agente, &terminal), None);
        crate::ganchos::esquecer(&terminal);
        let _ = close_session(&state, &terminal);
    }
}
