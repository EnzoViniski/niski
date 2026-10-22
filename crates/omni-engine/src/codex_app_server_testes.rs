use super::*;
use omni_core::conversations::{Conversation, Segment, TitleSource};

fn falso(dir: &Path, falhar: bool) -> Arc<Cliente> {
    let mut comando = Command::new("node");
    let p = comando
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fixtures/codex-app-server-falso.mjs"))
        .env("MEDZA_FAKE_LOG", dir.join("rpc.jsonl"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if falhar {
        p.env("MEDZA_FAKE_INITIALIZE_FAIL", "1");
    }
    let mut p = p.spawn().unwrap();
    let r = p.stdout.take().unwrap();
    let w = p.stdin.take().unwrap();
    cliente(
        Estado::default(),
        Box::new(r),
        Box::new(w),
        false,
        vec![p],
        None,
    )
}
fn esperar(f: impl Fn() -> bool) {
    let limite = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < limite, "Prazo da fixture");
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn registrar(c: &Arc<Cliente>) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    CLIENTES.lock().unwrap().insert(id.clone(), c.clone());
    id
}
fn log(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join("rpc.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

// Fork: PTY e processo sintéticos permitem reproduzir parada/teclado no sandbox.
#[derive(Debug)]
struct FilhoSintetico(bool); // Fork: `kill` também torna o filho esperável, como um processo real.
impl portable_pty::ChildKiller for FilhoSintetico {
    fn kill(&mut self) -> std::io::Result<()> { self.0=true; Ok(()) }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> { Box::new(Self(self.0)) }
}
impl Child for FilhoSintetico {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> { Ok(self.0.then(|| portable_pty::ExitStatus::with_exit_code(0))) }
    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> { Ok(portable_pty::ExitStatus::with_exit_code(0)) }
    fn process_id(&self) -> Option<u32> { Some(42) }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> { None }
}
struct MestreSintetico;
impl MasterPty for MestreSintetico {
    fn resize(&self, _: PtySize) -> Result<()> { Ok(()) }
    fn get_size(&self) -> Result<PtySize> { Ok(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }) }
    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>> { Ok(Box::new(std::io::empty())) }
    fn take_writer(&self) -> Result<Box<dyn Write + Send>> { Ok(Box::new(std::io::sink())) }
    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<i32> { None }
    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> { None }
    #[cfg(unix)]
    fn tty_name(&self) -> Option<PathBuf> { None }
}
struct GravadorSintetico(Arc<Mutex<Vec<u8>>>);
impl Write for GravadorSintetico {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
fn terminal_sintetico(dir: &Path, id: &str) -> (Arc<EngineState>, Arc<Mutex<Vec<u8>>>) {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let meta = serde_json::from_value(json!({"id": id, "project_id":"p", "name":"Fixture", "cwd":"/fixture", "shell":"fake",
        "state":"answered", "pid":42, "created_at_ms":1, "last_activity_at_ms":1, "output_seq":0, "rows":24, "cols":80,
        "provider":"codex", "conversation_id":id})).unwrap();
    let session = Arc::new(LiveSession {
        interaction: Mutex::new(interaction::Interaction::new(24, 80)),
        reserved: AtomicBool::new(false), headless: AtomicBool::new(false), meta: Mutex::new(meta),
        writer: Mutex::new(Box::new(GravadorSintetico(bytes.clone()))),
        master: Mutex::new(Box::new(MestreSintetico)), child: Mutex::new(Box::new(FilhoSintetico(false))),
        output: Mutex::new(OutputBuffer { chunks: VecDeque::new(), bytes: 0, next_seq: 0 }),
    });
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dir), token: "sintetico".into(),
        state_file: dir.join("sessions.json"), sessions: Mutex::new(HashMap::from([(id.into(), SessionEntry::Live(session))])),
        id_sequence: AtomicU64::new(1),
    });
    (state, bytes)
}

#[test]
fn regressao_7_parada_apos_checagem_do_monitor_nao_reabre() {
    let dir = tempfile::tempdir().unwrap();
    let c = falso(dir.path(), false);
    let id = registrar(&c);
    let (state, _) = terminal_sintetico(dir.path(), &id);
    assert!(!c.encerrado.load(Ordering::SeqCst)); // Monitor já passou pela checagem inicial.
    stop_session(&state, &id).unwrap();
    c.morreu.store(true, Ordering::SeqCst);
    let mut reabriu = false;
    tentar_fallback(&id, &c, || reabriu = true);
    let fallback = FALLBACK.lock().unwrap().remove(&id).is_some();
    assert!(!reabriu && !fallback, "Parada intencional não pode iniciar fallback");
}

#[test]
fn regressao_7_monitor_antigo_nao_remove_geracao_nova() {
    let dir = tempfile::tempdir().unwrap();
    let antigo = falso(dir.path(), false);
    let novo = falso(dir.path(), false);
    let id = registrar(&antigo);
    CLIENTES.lock().unwrap().insert(id.clone(), novo.clone());
    let mut reabriu = false;
    tentar_fallback(&id, &antigo, || { esquecer(&id); reabriu = true; });
    let preservou = obter(&id).is_some_and(|c| c.geracao == novo.geracao);
    FALLBACK.lock().unwrap().remove(&id);
    esquecer(&id);
    antigo.fechar();
    assert!(!reabriu && preservou, "Monitor obsoleto não pode afetar outro cliente");
}

#[test]
fn regressao_13_teclas_sem_turno_chegam_a_pty_e_turno_ativo_vai_ao_rpc() {
    let dir = tempfile::tempdir().unwrap();
    let c = falso(dir.path(), false);
    iniciar_thread(&c, "/fixture", None).unwrap();
    let id = registrar(&c);
    let (state, bytes) = terminal_sintetico(dir.path(), &id);
    for tecla in ["\x1b", "\x03"] { write_terminal(&state, &id, tecla).unwrap(); }
    let recebeu = bytes.lock().unwrap().clone();
    let rev = c.revision(&c.estado.lock().unwrap());
    enviar_prompt(&id, "esperar", &rev, false).unwrap();
    esperar(|| c.estado.lock().unwrap().turno.is_some());
    write_terminal(&state, &id, "\x03").unwrap();
    esperar(|| c.estado.lock().unwrap().turno.is_none());
    assert_eq!(*bytes.lock().unwrap(), recebeu, "Turno ativo deve ser interrompido por RPC");
    let enviados = log(dir.path());
    esquecer(&id);
    assert_eq!(recebeu, b"\x1b\x03", "Sem turno, as teclas pertencem à TUI");
    assert_eq!(enviados.iter().filter(|m| m["method"] == "turn/interrupt").count(), 1);
}

#[test]
fn regressao_7_parada_espera_fallback_em_andamento() {
    let dir = tempfile::tempdir().unwrap();
    let c = falso(dir.path(), false);
    let id = registrar(&c);
    let (entrou_tx, entrou_rx) = mpsc::channel();
    let (liberar_tx, liberar_rx) = mpsc::channel();
    let terminal = id.clone();
    let monitor = std::thread::spawn(move || tentar_fallback(&terminal, &c, || {
        entrou_tx.send(()).unwrap();
        liberar_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }));
    entrou_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (parou_tx, parou_rx) = mpsc::channel();
    let terminal = id.clone();
    let parada = std::thread::spawn(move || { esquecer(&terminal); parou_tx.send(()).unwrap(); });
    let serializou = parou_rx.recv_timeout(Duration::from_millis(100)).is_err();
    liberar_tx.send(()).unwrap();
    monitor.join().unwrap();
    parada.join().unwrap();
    FALLBACK.lock().unwrap().remove(&id);
    assert!(serializou, "Parar deve aguardar o fallback para matar também a PTY reaberta");
}

#[test]
fn fallback_prepara_pty_sob_a_mesma_trava_sem_reabrir_servidor() {
    let dir = tempfile::tempdir().unwrap();
    let c = falso(dir.path(), false);
    let id = registrar(&c);
    let (state, _) = terminal_sintetico(dir.path(), &id);
    let (tx, rx) = mpsc::channel();
    let monitor = std::thread::spawn(move || {
        tentar_fallback(&id, &c, || {
            remover_cliente(&id);
            let comando = preparar_com(&state, &id, &SessionOrigin::default(), "/fixture", Some("codex".into()), &[],
                |_, _, _, _| panic!("Fallback não pode criar outro app-server"));
            assert!(!FALLBACK.lock().unwrap().contains_key(&id));
            tx.send(comando).unwrap();
        });
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).expect("Preparação não pode disputar sua própria trava").as_deref(), Some("codex"));
    monitor.join().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn regressao_12_compactar_usa_rpc_oficial_sem_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let c = falso(dir.path(), false);
    iniciar_thread(&c, "/fixture", None).unwrap();
    let id = registrar(&c);
    let (state, bytes) = terminal_sintetico(dir.path(), &id);
    mobile::start(state.clone());
    // Índice inteiramente sintético, somente para passar pela rota pública da Conversa.
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":id,"project_id":"p","cwd":"/fixture","title":"Fixture","created_at_ms":1,
        "segments":[{"provider":"codex","terminal_session_id":id,"started_at_ms":1}]
    }]})).unwrap();
    let rev = c.revision(&c.estado.lock().unwrap());
    let EngineResponse::Api { status, body, .. } = mobile::local(state.clone(), "POST", &format!("/conversas/{id}/comando"),
        Some(json!({"comando":"compact"}).to_string()), None, vec![("if-match".into(), rev), ("idempotency-key".into(), id.clone())]).await else { panic!() };
    assert_eq!(status, 202, "{body}");
    esperar(|| log(dir.path()).iter().any(|m| m["method"] == "turn/start" || m["method"] == "thread/compact/start"));
    let enviados = log(dir.path());
    // A fixture de thread não emite progresso; exercitamos aqui os eventos oficiais de compactação.
    assert_eq!(estado_da_sessao(&c.estado.lock().unwrap()), SessionState::Working);
    for (metodo, turno) in [("turn/started", json!({"id":"compactacao"})), ("turn/completed", json!({"id":"compactacao","status":"completed"}))] {
        c.despachar(protocolo::ler(&serde_json::to_vec(&json!({"method":metodo,"params":{"threadId":"thread-falso","turn":turno}})).unwrap()).unwrap()).unwrap();
    }
    assert_eq!(estado_da_sessao(&c.estado.lock().unwrap()), SessionState::Answered);
    esquecer(&id);
    assert!(bytes.lock().unwrap().is_empty());
    assert!(!enviados.iter().any(|m| m["method"] == "turn/start" || m["method"] == "turn/steer"), "Compactar não é prompt");
    let rpc = enviados.iter().find(|m| m["method"] == "thread/compact/start").expect("RPC de compactação");
    assert_eq!(rpc["params"], json!({"threadId":"thread-falso"}));
}

#[test]
fn turno_plano_contexto_e_fim_uma_vez() {
    let dir = tempfile::tempdir().unwrap();
    let c = falso(dir.path(), false);
    iniciar_thread(&c, "/tmp", None).unwrap();
    let id = registrar(&c);
    let rev = c.revision(&c.estado.lock().unwrap());
    enviar_prompt(&id, "concluir", &rev, false).unwrap();
    esperar(|| c.estado.lock().unwrap().fins == 1);
    let e = c.estado.lock().unwrap();
    assert_eq!(e.tarefas.as_ref().unwrap()[0].texto, "Testar fixture");
    assert_eq!(e.tokens.as_ref().unwrap()["last"]["totalTokens"], 123);
    assert_eq!(estado_da_sessao(&e), SessionState::Answered);
    drop(e);
    assert_eq!(resumo(&id).unwrap()["contexto_pct"], 12.3);
    assert!(log(dir.path()).iter().any(|m| m["method"] == "initialized"));
    esquecer(&id);
}

#[test]
fn falha_no_spawn_ou_initialize_preserva_pty_e_sucesso_prepara_remote() {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: mobile::MobileRuntime::new(dir.path()),
        token: "sintetico".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    gravar(dir.path(), &HashSet::from(["c".into()])).unwrap();
    let origem = SessionOrigin {
        provider: Some("codex".into()),
        conversation_id: Some("c".into()),
        env: vec![("CODEX_HOME".into(), "/perfil-sintetico".into())],
        ..Default::default()
    };
    let original = Some("codex -m modelo-sintetico".into());
    assert_eq!(
        preparar_com(
            &state,
            "spawn-falso",
            &origem,
            "/tmp",
            original.clone(),
            &[],
            |_, _, _, _| Err(anyhow!("Falha sintética"))
        ),
        original
    );
    let c = falso(dir.path(), true);
    assert_eq!(
        preparar_com(
            &state,
            "init-falso",
            &origem,
            "/tmp",
            original.clone(),
            &[],
            |_, _, _, _| Ok((c.clone(), "unix:///fake".into()))
        ),
        original
    );
    assert!(c.encerrado.load(Ordering::SeqCst));
    let c = falso(dir.path(), false);
    let id = uuid::Uuid::new_v4().to_string();
    let preparado = preparar_com(
        &state,
        &id,
        &origem,
        "/tmp",
        original,
        &[("MEDZA_MCP_TOKEN".into(), "fixture".into())],
        |_, flags, _, env| {
            assert!(flags.iter().any(|s| s.contains("modelo-sintetico")));
            assert!(env
                .iter()
                .any(|(k, v)| k == "CODEX_HOME" && v == "/perfil-sintetico"));
            assert!(env.iter().any(|(k, _)| k == "MEDZA_MCP_TOKEN"));
            Ok((c.clone(), "unix:///fake".into()))
        },
    )
    .unwrap();
    assert!(preparado.contains("--remote 'unix:///fake' resume 'thread-falso'"));
    esquecer(&id);
}

#[cfg(unix)]
#[tokio::test]
async fn mesma_rota_aprovar_e_ajustes_por_agente() {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: mobile::MobileRuntime::new(dir.path()),
        token: "sintetico".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    let id = uuid::Uuid::new_v4().to_string();
    let conv = uuid::Uuid::new_v4().to_string();
    spawn_terminal_inner(
        &state,
        id.clone(),
        "p".into(),
        "Fixture".into(),
        "/tmp".into(),
        Some("/bin/sh".into()),
        None,
        24,
        80,
        now_ms(),
        SessionOrigin {
            provider: Some("codex".into()),
            conversation_id: Some(conv.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    omni_core::conversations::write_index(
        dir.path(),
        &[Conversation {
            id: conv.clone(),
            project_id: "p".into(),
            cwd: "/tmp".into(),
            title: "Fixture".into(),
            title_source: TitleSource::Auto,
            created_at_ms: now_ms(),
            segments: vec![Segment {
                provider: "codex".into(),
                profile_id: None,
                external_session_id: None,
                transcript_path: None,
                terminal_session_id: Some(id.clone()),
                started_at_ms: 0,
                ended_at_ms: None,
            }],
        }],
    )
    .unwrap();
    let EngineResponse::Api { status, body, .. } = mobile::local(
        state.clone(),
        "GET",
        "/codex/app-server/ajustes",
        None,
        None,
        vec![],
    )
    .await
    else {
        panic!()
    };
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["agentes"][0]["ligado"],
        false
    );
    let EngineResponse::Api { status, .. } = mobile::local(
        state.clone(),
        "POST",
        "/codex/app-server/ajustes",
        Some(json!({"agente":conv,"ligado":true}).to_string()),
        None,
        vec![],
    )
    .await
    else {
        panic!()
    };
    assert_eq!(status, 200);
    assert!(habilitados(dir.path()).contains(&conv));
    let c = falso(dir.path(), false);
    iniciar_thread(&c, "/tmp", None).unwrap();
    CLIENTES.lock().unwrap().insert(id.clone(), c.clone());
    for permitir in [true, false] {
        let rev = c.revision(&c.estado.lock().unwrap());
        enviar_prompt(&id, "aprovar", &rev, false).unwrap();
        esperar(|| !c.estado.lock().unwrap().pedidos.is_empty());
        let pedido = c.estado.lock().unwrap().pedidos[0].visivel.id.clone();
        let rota = format!("/conversas/{conv}/aprovar");
        let EngineResponse::Api { status, .. } = mobile::local(
            state.clone(),
            "POST",
            &rota,
            Some(json!({"permitir":permitir}).to_string()),
            None,
            vec![],
        )
        .await
        else {
            panic!()
        };
        assert_eq!(status, 412, "sem id não pode aprovar às cegas");
        let EngineResponse::Api { status, .. } = mobile::local(
            state.clone(),
            "POST",
            &rota,
            Some(json!({"permitir":permitir,"pedido":"codex-obsoleto"}).to_string()),
            None,
            vec![],
        )
        .await
        else {
            panic!()
        };
        assert_eq!(status, 412);
        let EngineResponse::Api { status, body, .. } = mobile::local(
            state.clone(),
            "POST",
            &rota,
            Some(json!({"permitir":permitir,"pedido":pedido}).to_string()),
            None,
            vec![],
        )
        .await
        else {
            panic!()
        };
        assert_eq!(status, 200);
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap()["via"],
            "codex_app_server"
        );
        esperar(|| c.estado.lock().unwrap().turno.is_none());
    }
    if let Some(SessionEntry::Live(s)) = state.sessions.lock().unwrap().get(&id) {
        s.meta.lock().unwrap().profile_id = Some("perfil-sintetico".into());
    }
    let uso = uso_da_conta(&state, "perfil-sintetico").unwrap();
    assert_eq!(uso.source, "codex_app_server");
    assert_eq!(uso.primary.unwrap().used_percent, 12.0);
    close_session(&state, &id).unwrap();
}

#[test]
fn permitir_e_negar_com_ids_originais_e_pedido_obsoleto_recusado() {
    for (texto, permitir) in [("aprovar", true), ("arquivo", false)] {
        let dir = tempfile::tempdir().unwrap();
        let c = falso(dir.path(), false);
        iniciar_thread(&c, "/tmp", None).unwrap();
        let id = registrar(&c);
        let rev = c.revision(&c.estado.lock().unwrap());
        enviar_prompt(&id, texto, &rev, false).unwrap();
        esperar(|| !c.estado.lock().unwrap().pedidos.is_empty());
        assert!(responder(&id, "outro-pedido", true).is_err());
        let pedido = c.estado.lock().unwrap().pedidos[0].visivel.clone();
        responder(&id, &pedido.id, permitir).unwrap();
        assert!(responder(&id, &pedido.id, permitir).is_err());
        esperar(|| c.estado.lock().unwrap().fins == 1);
        let enviados = log(dir.path());
        let resposta = enviados.iter().find(|m| m.get("result").is_some()).unwrap();
        assert_eq!(resposta["id"], "pedido-1");
        assert_eq!(
            resposta["result"]["decision"],
            if permitir { "accept" } else { "decline" }
        );
        esquecer(&id);
    }
}

#[test]
fn resume_fork_steer_interrupt_e_initialize_recusado() {
    let dir = tempfile::tempdir().unwrap();
    let c = falso(dir.path(), true);
    assert!(iniciar_thread(&c, "/tmp", None).is_err());
    c.fechar();
    for operacao in ["resume", "fork"] {
        let dir = tempfile::tempdir().unwrap();
        let c = falso(dir.path(), false);
        iniciar_thread(&c, "/tmp", Some((operacao.into(), "original".into()))).unwrap();
        assert_eq!(
            c.estado.lock().unwrap().thread,
            if operacao == "fork" {
                "fork-original"
            } else {
                "original"
            }
        );
        let id = registrar(&c);
        let rev = c.revision(&c.estado.lock().unwrap());
        enviar_prompt(&id, "esperar", &rev, false).unwrap();
        esperar(|| c.estado.lock().unwrap().turno.is_some());
        enviar_prompt(&id, "ajuste sintético", "ignorada-na-fila", true).unwrap();
        interromper(&id).unwrap().unwrap();
        esperar(|| c.estado.lock().unwrap().fins == 1);
        let enviados = log(dir.path());
        let steer = enviados
            .iter()
            .find(|m| m["method"] == "turn/steer")
            .unwrap();
        assert_eq!(steer["params"]["expectedTurnId"], "turn-1");
        esquecer(&id);
    }
}

#[test]
fn opcao_desligada_e_comando_personalizado_preservado() {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: mobile::MobileRuntime::new(dir.path()),
        token: "sintetico".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    let origem = SessionOrigin {
        provider: Some("codex".into()),
        conversation_id: Some("c".into()),
        ..Default::default()
    };
    assert_eq!(
        preparar(&state, "off", &origem, "/tmp", Some("codex".into()), &[]).as_deref(),
        Some("codex")
    );
    gravar(dir.path(), &HashSet::from(["c".into()])).unwrap();
    assert_eq!(
        preparar(
            &state,
            "custom",
            &origem,
            "/tmp",
            Some("codex && echo x".into()),
            &[]
        )
        .as_deref(),
        Some("codex && echo x")
    );
    assert!(argumentos("codex 'prompt'").is_none());
    let (_,flags,alvo)=argumentos("codex -m modelo-sintetico -c 'mcp_servers.medza.url=\"http://127.0.0.1/mcp\"' fork original").unwrap();
    assert!(flags
        .iter()
        .any(|v| v == "mcp_servers.medza.url=\"http://127.0.0.1/mcp\""));
    assert_eq!(alvo.unwrap().0, "fork");
    let retomada = comando_de_retomada(
        "codex fork original -c 'model_reasoning_effort=\"high\"'",
        "copia",
    )
    .unwrap();
    assert!(retomada.starts_with("codex resume copia "));
    assert!(!retomada.contains("fork original"));
}

#[cfg(unix)]
#[test]
fn queda_do_servidor_reabre_pty_sem_repetir_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: mobile::MobileRuntime::new(dir.path()),
        token: "sintetico".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    let id = uuid::Uuid::new_v4().to_string();
    let conv = uuid::Uuid::new_v4().to_string();
    let meta = spawn_terminal_inner(
        &state,
        id.clone(),
        "p".into(),
        "Fixture".into(),
        "/tmp".into(),
        Some("/bin/sh".into()),
        Some("printf 'PTY-FALLBACK\\n'".into()),
        24,
        80,
        now_ms(),
        SessionOrigin {
            provider: Some("codex".into()),
            conversation_id: Some(conv.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    omni_core::conversations::write_index(
        dir.path(),
        &[Conversation {
            id: conv,
            project_id: "p".into(),
            cwd: "/tmp".into(),
            title: "Fixture".into(),
            title_source: TitleSource::Auto,
            created_at_ms: now_ms(),
            segments: vec![Segment {
                provider: "codex".into(),
                profile_id: None,
                external_session_id: None,
                transcript_path: None,
                terminal_session_id: Some(id.clone()),
                started_at_ms: 0,
                ended_at_ms: None,
            }],
        }],
    )
    .unwrap();
    let c = falso(dir.path(), false);
    iniciar_thread(&c, "/tmp", None).unwrap();
    CLIENTES.lock().unwrap().insert(id.clone(), c.clone());
    acompanhar(Arc::downgrade(&state), id.clone(), c.clone());
    let rev = c.revision(&c.estado.lock().unwrap());
    enviar_prompt(&id, "cair", &rev, false).unwrap();
    esperar(|| !ativo(&id));
    // Fork: o reinício zera o pid (`None`) antes de preparar a PTY nova e consumir a marca do
    // fallback; esperar só "pid diferente" via o `None` no meio do caminho e, sob carga, conferia a
    // marca cedo demais. Espera o pid novo de verdade.
    esperar(
        || matches!(state.sessions.lock().unwrap().get(&id),Some(SessionEntry::Live(s)) if s.meta.lock().unwrap().pid.is_some_and(|p| Some(p)!=meta.pid)),
    );
    let enviados = log(dir.path());
    assert_eq!(
        enviados
            .iter()
            .filter(|m| m["method"] == "turn/start")
            .count(),
        1
    );
    assert!(!FALLBACK.lock().unwrap().contains_key(&id));
    close_session(&state, &id).unwrap();
}


// Fork (revisão): o risco do pedido olha o comando inteiro, não só os 500 caracteres da tela.
#[test]
fn risco_do_pedido_olha_o_comando_inteiro() {
    let dir = tempfile::tempdir().unwrap();
    let terminal = format!("terminal-risco-{}", uuid::Uuid::new_v4());
    falso_com_pedido(&terminal, dir.path(), "longo");
    let c = CLIENTES.lock().unwrap().get(&terminal).cloned().unwrap();
    let e = c.estado.lock().unwrap();
    let pedido = &e.pedidos.front().unwrap().visivel;
    assert!(pedido.resumo.chars().count() <= 500 && !pedido.resumo.contains("rm -rf"));
    assert!(pedido.arriscado.is_some(), "rm -rf depois do corte tem que contar como arriscado");
}
