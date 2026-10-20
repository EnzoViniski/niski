use super::*;
use axum::body::{Body, to_bytes};
use tower::ServiceExt;

fn meta(estado: &str, seq: u64, aviso: Option<&str>) -> TerminalSession {
    serde_json::from_value(json!({"id":"s", "project_id":"p", "name":"Agente", "cwd":"/fixture/projeto", "shell":"sh",
        "state":estado, "pid":null, "created_at_ms":10, "last_activity_at_ms":100,
        "output_seq":20, "rows":24, "cols":80, "provider":"claude", "profile_id":"conta",
        "conversation_id":"c", "attention_seq":seq, "notice":aviso})).unwrap()
}

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    // Fixtures isoladas: nenhum perfil, CLI ou transcript do usuário é consultado.
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles":[{
        "id":"conta", "name":"Trabalho", "provider":"claude", "config_dir":dir.path().join("fixture"),
        "builtin":false, "created_at_ms":0}]})).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"c", "project_id":"p", "cwd":"/fixture/projeto", "title":"Conversa teste", "title_source":"manual", "created_at_ms":1,
        "segments":[{"provider":"claude", "profile_id":"conta", "terminal_session_id":"s", "started_at_ms":1}]}]})).unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
        token: "fixture".into(), state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::from([("s".into(), SessionEntry::Historical(meta("answered", 1, None)))])),
        id_sequence: AtomicU64::new(1) });
    (dir, state)
}

#[test]
fn classifica_cada_sinal_sem_confundir_ocioso_com_fim() {
    for (estado, seq, aviso, aprovar, escolha, sub, esperado) in [
        ("answered", 0, None, false, false, false, None),
        ("working", 1, None, false, false, false, None),
        ("orphan", 1, None, false, false, false, None),
        ("stopped", 1, None, false, false, false, None),
        ("approval_required", 0, None, false, false, false, Some(Tipo::PrecisaDeVoce)),
        ("working", 0, None, true, false, false, Some(Tipo::PrecisaDeVoce)),
        ("answered", 0, None, false, true, false, Some(Tipo::PrecisaDeVoce)),
        ("answered", 1, None, false, false, false, Some(Tipo::Terminou)),
        ("answered", 1, None, false, false, true, Some(Tipo::SubagenteTerminou)),
        ("answered", 1, Some("api_error"), false, false, false, Some(Tipo::Erro)),
        ("crashed", 0, None, false, false, false, Some(Tipo::Erro)),
        ("answered", 1, Some("usage_limit"), false, false, false, Some(Tipo::Limite)),
    ] { assert_eq!(classificar(&meta(estado, seq, aviso), aprovar, escolha, sub), esperado, "{estado}/{aviso:?}"); }
}

#[test]
fn inclui_identidade_e_subagente_do_orquestrador() {
    let (dir, state) = fixture();
    atomic_write_json(&dir.path().join("orquestracao.json"), &json!({"orquestradores":[{
        "orquestrador":"pai", "agentes":[{"conversationKey":"c", "plataforma":"claude", "tarefa":"fixture", "criado_em":1}]}]})).unwrap();
    let itens = coletar(&state);
    assert_eq!(itens.len(), 1);
    assert_eq!(itens[0].projeto, "projeto");
    assert_eq!(itens[0].conta, "Trabalho");
    assert_eq!(itens[0].tipo, Tipo::SubagenteTerminou);
    assert_eq!(itens[0].orquestrador.as_deref(), Some("pai"));
}

#[test]
fn redesenho_nao_muda_data_nem_reabre_lido_e_turno_novo_sim() {
    let (dir, state) = fixture();
    let mut registro = Registro::abrir(dir.path());
    registro.observar(coletar(&state));
    let id = registro.itens[0].id.clone();
    registro.marcar(dir.path(), &[id.clone()]).unwrap();
    let mut redesenho = coletar(&state);
    redesenho[0].desde_ms = 200;
    registro.observar(redesenho);
    assert_eq!(registro.itens[0].desde_ms, 100);
    assert!(registro.itens[0].visto);
    state.sessions.lock().unwrap().insert("s".into(), SessionEntry::Historical(meta("answered", 2, None)));
    registro.observar(coletar(&state));
    assert_eq!(registro.itens.len(), 2);
    assert!(registro.itens.iter().any(|i| i.id != id && !i.visto));
}

#[test]
fn vistos_persistem_so_ids_e_erros_de_escrita_nao_marcam_memoria() {
    let (dir, state) = fixture();
    let mut registro = Registro::abrir(dir.path());
    registro.observar(coletar(&state));
    let id = registro.itens[0].id.clone();
    let bloqueado = dir.path().join("arquivo");
    fs::write(&bloqueado, "fixture").unwrap();
    assert!(registro.marcar(&bloqueado, &[id.clone()]).is_err());
    assert!(!registro.itens[0].visto);
    registro.marcar(dir.path(), &[id.clone(), "nao-existe".into()]).unwrap();
    let salvo: Value = serde_json::from_slice(&fs::read(dir.path().join(ARQUIVO)).unwrap()).unwrap();
    assert_eq!(salvo, json!([id]));
    let mut reinicio = Registro::abrir(dir.path());
    reinicio.observar(coletar(&state));
    assert!(reinicio.itens[0].visto);
}

#[test]
fn guarda_historico_observado_ao_voltar_a_trabalhar_e_limita_memoria() {
    let (_dir, state) = fixture();
    atualizar(&state);
    state.sessions.lock().unwrap().insert("s".into(), SessionEntry::Historical(meta("working", 1, None)));
    atualizar(&state);
    assert_eq!(state.mobile.atividade.lock().unwrap().itens.len(), 1);
    let mut registro = state.mobile.atividade.lock().unwrap();
    let modelo = registro.itens[0].clone();
    registro.observar((0..600).map(|n| Item { id: format!("evento-{n}"), conversa: format!("c-{n}"), desde_ms: n, ..modelo.clone() }).collect());
    assert_eq!(registro.itens.len(), MAX_ITENS);
    registro.observar(vec![Item { id: "evento-0".into(), conversa: "c-0".into(), desde_ms: 900, ..modelo }]);
    assert!(!registro.itens.iter().any(|item| item.id == "evento-0"), "evento fora da janela não pode reaparecer num redesenho");
}

#[tokio::test]
async fn rotas_etag_vistos_compartilhados_e_transporte_local() {
    let (_dir, state) = fixture();
    let app = super::super::rotas().with_state(state.clone());
    let resposta = app.clone().oneshot(axum::http::Request::builder().uri("/atividade").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resposta.status(), StatusCode::OK);
    let etag = resposta.headers()["etag"].clone();
    let itens: Value = serde_json::from_slice(&to_bytes(resposta.into_body(), 100_000).await.unwrap()).unwrap();
    let repetida = axum::http::Request::builder().uri("/atividade").header("if-none-match", &etag).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(repetida).await.unwrap().status(), StatusCode::NOT_MODIFIED);
    let corpo = json!({"ids":[itens[0]["id"]]}).to_string();
    let resposta = super::super::local(state.clone(), "POST", "/atividade/vistos", Some(corpo), None, vec![]).await;
    assert!(matches!(resposta, EngineResponse::Api { status: 200, .. }));
    let depois = app.oneshot(axum::http::Request::builder().uri("/atividade").header("if-none-match", &etag).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(depois.status(), StatusCode::OK);
    assert_ne!(depois.headers()["etag"], etag);
    let corpo: Value = serde_json::from_slice(&to_bytes(depois.into_body(), 100_000).await.unwrap()).unwrap();
    assert_eq!(corpo[0]["visto"], true);
}

#[tokio::test]
async fn recusa_lote_invalido_e_sessao_ligada_a_outra_conta() {
    let (_dir, state) = fixture();
    let corpo = json!({"ids":vec!["x"; MAX_ITENS + 1]}).to_string();
    let resposta = super::super::local(state.clone(), "POST", "/atividade/vistos", Some(corpo), None, vec![]).await;
    assert!(matches!(resposta, EngineResponse::Api { status: 400, .. }));
    let mut outra = meta("answered", 1, None); outra.profile_id = Some("outra".into());
    state.sessions.lock().unwrap().insert("s".into(), SessionEntry::Historical(outra));
    assert!(coletar(&state).is_empty());
}

/// Fork (pergunta-da-cli): menu da própria CLI e espera no terminal viram "precisa de você" com o
/// resumo dizendo que é a CLI; pergunta do agente e aprovação mantêm o resumo de sempre.
#[test]
fn pergunta_da_cli_e_espera_dizem_que_e_no_terminal() {
    let cap = |escolha: Option<interaction::Escolha>, espera: bool| interaction::Capabilities {
        prompt: false, approve: false, revision: String::new(), approval_text: None, reason: None, enfileirar: false, escolha, pedido: None,
        espera: espera.then(|| crate::pergunta_da_cli::Espera { texto: "Continuing automatically at 3:45pm · esc to cancel".into(), dica: None }),
    };
    let menu = |da_cli| interaction::Escolha { pergunta: "Enable full access?".into(), detalhe: None, opcoes: Vec::new(), multipla: false,
        etapas: Vec::new(), previa: false, da_cli, teclas: None };
    // A espera conta como sinal de "precisa de você" (o `coletar` passa `escolha || espera`).
    assert_eq!(classificar(&meta("answered", 1, None), false, true, false), Some(Tipo::PrecisaDeVoce));
    assert_eq!(resumo(Tipo::PrecisaDeVoce, Some(&cap(Some(menu(true)), false))), "A CLI está perguntando no terminal");
    assert_eq!(resumo(Tipo::PrecisaDeVoce, Some(&cap(None, true))), "Esperando algo no terminal");
    assert_eq!(resumo(Tipo::PrecisaDeVoce, Some(&cap(Some(menu(false)), false))), "Precisa de você");
    assert_eq!(resumo(Tipo::Terminou, Some(&cap(None, true))), "Terminou o turno");
}
