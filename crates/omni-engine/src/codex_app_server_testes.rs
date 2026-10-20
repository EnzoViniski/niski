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
    let EngineResponse::Api { status, body } = mobile::local(
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
        let EngineResponse::Api { status, body } = mobile::local(
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
    esperar(
        || matches!(state.sessions.lock().unwrap().get(&id),Some(SessionEntry::Live(s)) if s.meta.lock().unwrap().pid!=meta.pid),
    );
    let enviados = log(dir.path());
    assert_eq!(
        enviados
            .iter()
            .filter(|m| m["method"] == "turn/start")
            .count(),
        1
    );
    assert!(!FALLBACK.lock().unwrap().contains(&id));
    close_session(&state, &id).unwrap();
}
