use super::*;

fn preparar(fluxo: Value) -> (tempfile::TempDir, Arc<EngineState>, Viva) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: mobile::MobileRuntime::new(dir.path()),
        token: "teste".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    let fluxo: Fluxo = serde_json::from_value(fluxo).unwrap();
    let nos = fluxo
        .nos
        .iter()
        .map(|n| (n.id.clone(), EstadoDoNo::default()))
        .collect();
    let e = Execucao {
        id: format!("teste-{}", uuid::Uuid::new_v4()),
        fluxo,
        projeto: dir.path().to_string_lossy().into(),
        projeto_id: "p".into(),
        entrada: "tarefa".into(),
        pasta: dir.path().to_string_lossy().into(),
        branch: None,
        base: None,
        status: "rodando".into(),
        erro: None,
        criado_ms: now_ms(),
        atualizado_ms: now_ms(),
        nos,
        fila: vec![],
        passo_ativo: None,
        pr_url: None,
        custo_usd: 0.0,
    };
    (dir, state, Arc::new(Mutex::new(e)))
}
fn grafo() -> Value {
    json!({"id":"t","nome":"T","nos":[{"id":"e","tipo":"entrada"},{"id":"r","tipo":"repetir","config":{"vezes":3,"intervalo_minutos":0}},{"id":"a","tipo":"fim"},{"id":"f","tipo":"fim"}],"conexoes":[{"de":"r","para":"a","saida":"corpo"},{"de":"a","para":"r"},{"de":"r","para":"f","saida":"fim"}]})
}

#[test]
fn repeticoes_terminam_no_limite_e_conservam_contagem() {
    let (_dir, state, v) = preparar(grafo());
    let no = v.lock().unwrap().fluxo.nos[1].clone();
    for volta in 1..=3 {
        assert_eq!(
            processar_repetir(&state, &v, &no, &Contexto::default()).unwrap(),
            "corpo"
        );
        assert_eq!(v.lock().unwrap().nos["r"].voltas, volta);
    }
    assert_eq!(
        processar_repetir(&state, &v, &no, &Contexto::default()).unwrap(),
        "fim"
    );
    let e: Execucao = serde_json::from_slice(
        &fs::read(pasta_execucoes(&state).join(format!("{}.json", v.lock().unwrap().id))).unwrap(),
    )
    .unwrap();
    assert_eq!(e.nos["r"].voltas, 3);
}
#[test]
fn aprovado_para_antes_do_limite() {
    let (_dir, state, v) = preparar(grafo());
    let mut no = v.lock().unwrap().fluxo.nos[1].clone();
    no.config["parar_quando_aprovado"] = json!(true);
    assert_eq!(
        processar_repetir(&state, &v, &no, &Contexto::default()).unwrap(),
        "corpo"
    );
    assert_eq!(
        processar_repetir(
            &state,
            &v,
            &no,
            &Contexto {
                anterior: "aprovado".into(),
                ..Default::default()
            }
        )
        .unwrap(),
        "fim"
    );
    assert_eq!(v.lock().unwrap().nos["r"].voltas, 1);
}
#[test]
fn pausa_interrompe_espera_sem_consumir_iteracao_e_retomada_preserva_prazo() {
    let (_dir, state, v) = preparar(grafo());
    let mut no = v.lock().unwrap().fluxo.nos[1].clone();
    no.config["intervalo_minutos"] = json!(1);
    processar_repetir(&state, &v, &no, &Contexto::default()).unwrap();
    v.lock().unwrap().status = "pausado".into();
    assert_eq!(
        processar_repetir(&state, &v, &no, &Contexto::default())
            .unwrap_err()
            .to_string(),
        PAUSADO
    );
    let prazo = v.lock().unwrap().nos["r"].proxima_ms;
    assert!(prazo > now_ms());
    assert_eq!(v.lock().unwrap().nos["r"].voltas, 1);
    {
        let mut e = v.lock().unwrap();
        e.status = "rodando".into();
        e.nos.get_mut("r").unwrap().proxima_ms = now_ms().saturating_sub(1);
    }
    assert_eq!(
        processar_repetir(&state, &v, &no, &Contexto::default()).unwrap(),
        "corpo"
    );
    assert_eq!(v.lock().unwrap().nos["r"].voltas, 2);
    assert!(esperar_ate(now_ms() + 60000, || false).is_err());
}
#[test]
fn objetivo_recusa_codex_e_ganchos_desativados() {
    let dir = tempfile::tempdir().unwrap();
    assert!(conferir_goal("codex", None, dir.path().to_str().unwrap()).is_err());
    fs::create_dir(dir.path().join(".claude")).unwrap();
    fs::write(
        dir.path().join(".claude/settings.json"),
        br#"{"disableAllHooks":true}"#,
    )
    .unwrap();
    assert!(conferir_goal("claude", None, dir.path().to_str().unwrap()).is_err());
}

#[cfg(unix)]
#[test]
fn goal_usa_cli_falsa_stream_json_teto_e_sessao() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, state, v) = preparar(grafo());
    let script = dir.path().join("claude-falso");
    fs::write(&script,r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/args"
cat > "$(dirname "$0")/entrada"
printf '%s\n' '{"type":"assistant","session_id":"goal-falso","message":{"role":"assistant","content":[{"type":"text","text":"conferindo"}]}}'
printf '%s\n' '{"type":"result","session_id":"goal-falso","result":"objetivo atingido","total_cost_usd":0.01,"num_turns":2,"is_error":false}'
"#).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let modelo = omni_core::agentes::ModeloDeAgente {
        cli: "claude".into(),
        instrucoes: "fixture".into(),
        ..Default::default()
    };
    let resultado = rodar_modelo(
        &state,
        &v,
        "a",
        modelo.clone(),
        &script,
        "/goal testes passam ou pare após 4 turnos",
        None,
        dir.path().to_str().unwrap(),
        &Ajuste {
            max_turnos: Some(4),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(resultado, "objetivo atingido");
    let args = fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(args.contains("--max-turns\n4\n") && args.contains("stream-json"));
    assert_eq!(
        fs::read_to_string(dir.path().join("entrada")).unwrap(),
        "/goal testes passam ou pare após 4 turnos"
    );
    assert_eq!(
        v.lock().unwrap().nos["a"].session_id.as_deref(),
        Some("goal-falso")
    );
    assert!(historico(&state, &v.lock().unwrap().id, "a")
        .unwrap()
        .iter()
        .any(|ev| ev["text"] == "conferindo"));
    fs::write(&script,"#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"type\":\"result\",\"result\":\"limite atingido\",\"is_error\":true}'\nexit 1\n").unwrap();
    assert!(rodar_modelo(
        &state,
        &v,
        "a",
        modelo,
        &script,
        "/goal teste",
        None,
        dir.path().to_str().unwrap(),
        &Ajuste {
            max_turnos: Some(4),
            ..Default::default()
        }
    )
    .unwrap_err()
    .to_string()
    .contains("limite atingido"));
}

#[test]
fn recuperar_passo_em_espera_depois_de_reiniciar_engine() {
    let (_dir, state, v) = preparar(grafo());
    {
        let mut e = v.lock().unwrap();
        e.passo_ativo = Some(Passo {
            no: "r".into(),
            de: Some("a".into()),
        });
        e.nos.get_mut("r").unwrap().proxima_ms = now_ms() + 60000;
        gravar(&state, &e);
    }
    carregar(&state);
    let e = obter(&v.lock().unwrap().id).unwrap();
    assert_eq!(e.status, "pausado");
    assert_eq!(e.fila[0].no, "r");
    assert_eq!(e.fila[0].de.as_deref(), Some("a"));
    assert!(e.nos["r"].proxima_ms > now_ms());
}

#[test]
fn grafo_repetir_executa_ramo_duas_vezes_e_segue_fim() {
    let (dir, state, _v) = preparar(grafo());
    let fluxo: Fluxo = serde_json::from_value(json!({"id":"repeticao","nome":"Repetição","nos":[{"id":"e","tipo":"entrada"},{"id":"r","tipo":"repetir","config":{"vezes":2,"intervalo_minutos":0}},{"id":"a","tipo":"aprovacao"},{"id":"f","tipo":"fim"}],"conexoes":[{"de":"e","para":"r"},{"de":"r","para":"a","saida":"corpo"},{"de":"a","para":"r","saida":"sim"},{"de":"a","para":"r","saida":"nao"},{"de":"r","para":"f","saida":"fim"}]})).unwrap();
    let e = iniciar(
        &state,
        fluxo,
        dir.path().to_string_lossy().into(),
        "p".into(),
        "Rever".into(),
    )
    .unwrap();
    let esperar = |status: &str| {
        for _ in 0..100 {
            if obter(&e.id).unwrap().status == status {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("não chegou a {status}");
    };
    esperar("aguardando");
    assert_eq!(obter(&e.id).unwrap().nos["r"].voltas, 1);
    aprovar(&state, &e.id, "a", true).unwrap();
    esperar("aguardando");
    assert_eq!(obter(&e.id).unwrap().nos["r"].voltas, 2);
    aprovar(&state, &e.id, "a", true).unwrap();
    esperar("concluido");
    assert_eq!(
        obter(&e.id).unwrap().nos["r"].caminho.as_deref(),
        Some("fim")
    );
}

#[cfg(unix)]
#[test]
fn cancelar_mata_cli_falsa_e_descendentes() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, state, v) = preparar(grafo());
    let script = dir.path().join("claude-espera");
    fs::write(&script,"#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{\"type\":\"assistant\",\"session_id\":\"fake\",\"message\":{\"role\":\"assistant\",\"content\":[]}}'\nsleep 60\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let id = v.lock().unwrap().id.clone();
    execucoes().lock().unwrap().insert(id.clone(), v.clone());
    let (envio, recepcao) = std::sync::mpsc::channel();
    let (s, viva, pasta) = (
        state.clone(),
        v.clone(),
        dir.path().to_string_lossy().into_owned(),
    );
    std::thread::spawn(move || {
        let modelo = omni_core::agentes::ModeloDeAgente {
            cli: "claude".into(),
            ..Default::default()
        };
        let r = rodar_modelo(
            &s,
            &viva,
            "a",
            modelo,
            &script,
            "fixture",
            None,
            &pasta,
            &Ajuste::default(),
        );
        let _ = envio.send(r.map_err(|e| e.to_string()));
    });
    for _ in 0..100 {
        if processos().lock().unwrap().contains_key(&id) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    cancelar(&state, &id).unwrap();
    assert_eq!(
        recepcao
            .recv_timeout(std::time::Duration::from_secs(3))
            .expect("CLI e descendentes deveriam parar")
            .unwrap_err(),
        PAUSADO
    );
    assert_eq!(obter(&id).unwrap().status, "cancelado");
    assert!(!processos().lock().unwrap().contains_key(&id));
}
