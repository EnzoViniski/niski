//! A conferência única do teto de sessões (antes eram quatro cópias de `MAX_SESSOES = 16`
//! espalhadas pelo `mobile.rs`, contando até shell parado no prompt). Regra e número em
//! `omni_core::limite_de_sessoes`; aqui só a contagem das PTYs vivas.
//!
//! Filho de `mobile` (mora em `mobile/`) para usar `ApiError` e o estado sem torná-los públicos.

use super::*;

/// Shell sem agente com um comando em primeiro plano. Na PTY, o grupo de processos da frente é o do
/// próprio shell quando ele está parado no prompt; rodando `npm run dev`, é o do comando.
fn terminal_ocupado(sessao: &LiveSession) -> bool {
    #[cfg(unix)]
    {
        // Trava ocupada (redimensionando, encerrando): conta, para nunca deixar passar do limite.
        let (Ok(master), Ok(mut filho)) = (sessao.master.try_lock(), sessao.child.try_lock()) else { return true };
        if !matches!(filho.try_wait(), Ok(None)) { return false; }
        match (master.process_group_leader(), filho.process_id()) {
            (Some(frente), Some(shell)) => frente as u32 != shell,
            _ => false,
        }
    }
    // Sem como ler o grupo da frente: conta como antes.
    #[cfg(not(unix))]
    { let _ = sessao; true }
}

pub(crate) fn contar(state: &EngineState) -> omni_core::limite_de_sessoes::Contagem {
    // Copia as sessões e solta a trava do mapa antes de olhar cada uma.
    let vivas: Vec<Arc<LiveSession>> = state.sessions.lock().expect("sessions poisoned").values()
        .filter_map(|entrada| match entrada { SessionEntry::Live(s) => Some(s.clone()), SessionEntry::Historical(_) => None })
        .collect();
    let mut contagem = omni_core::limite_de_sessoes::Contagem::default();
    for sessao in vivas {
        let agente = sessao.meta.lock().map(|meta| meta.provider.is_some()).unwrap_or(true);
        if agente { contagem.agentes += 1 } else if terminal_ocupado(&sessao) { contagem.terminais_ocupados += 1 }
    }
    contagem
}

/// `429` com quantas há e como liberar, se abrir mais uma passaria do limite.
pub(crate) fn conferir(state: &EngineState) -> Result<(), ApiError> {
    let dir = state.state_file.parent().unwrap();
    let limite = omni_core::limite_de_sessoes::ler(dir);
    let contagem = contar(state);
    omni_core::limite_de_sessoes::conferir(contagem, limite).map_err(|mensagem| {
        registro::aviso("limite_de_sessoes", &[("limite", &limite.to_string()), ("agentes", &contagem.agentes.to_string()),
            ("terminais", &contagem.terminais_ocupados.to_string())]);
        error(StatusCode::TOO_MANY_REQUESTS, &mensagem)
    })
}

#[cfg(all(test, unix))]
mod testes {
    use super::*;

    fn esperar(ate: impl Fn() -> bool) -> bool {
        let fim = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < fim {
            if ate() { return true; }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    #[test]
    fn shell_parado_nao_conta_comando_rodando_e_agente_contam() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
            token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
            id_sequence: AtomicU64::new(1) });
        let cwd = dir.path().to_string_lossy().into_owned();
        let abrir = |comando: Option<&str>, provider: Option<&str>| {
            spawn_terminal(&state, "p".into(), "t".into(), cwd.clone(), Some("/bin/sh".into()), comando.map(str::to_owned), 24, 80,
                SessionOrigin { headless: true, provider: provider.map(str::to_owned), ..SessionOrigin::default() }).unwrap();
        };
        abrir(None, None);
        // Dá tempo do shell subir e assumir o terminal.
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert_eq!(contar(&state).total(), 0, "shell parado no prompt não ocupa vaga");

        abrir(Some("sleep 30"), None);
        assert!(esperar(|| contar(&state).terminais_ocupados == 1), "comando rodando ocupa vaga");
        abrir(None, Some("claude"));
        assert_eq!(contar(&state).agentes, 1);

        omni_core::limite_de_sessoes::gravar(dir.path(), 4).unwrap();
        assert!(conferir(&state).is_ok());
        abrir(Some("sleep 30"), None);
        abrir(None, Some("codex"));
        assert!(esperar(|| contar(&state).total() == 4));
        let (status, Json(corpo)) = conferir(&state).unwrap_err();
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let texto = corpo["error"].as_str().unwrap();
        assert!(texto.contains("Limite de 4") && texto.contains("2 agentes e 2 terminais"), "{texto}");

        let ids: Vec<String> = state.sessions.lock().unwrap().keys().cloned().collect();
        for id in ids { let _ = close_session(&state, &id); }
    }
}
