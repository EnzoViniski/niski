use super::*;
use std::sync::atomic::AtomicBool;
use tower::ServiceExt;

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: MobileRuntime::new(dir.path()),
        token: "fixture".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    (dir, state)
}
fn config() -> Config {
    Config {
        token: "123:segredo-sintetico-nao-usar-em-rede".into(),
        bot: "medza_teste_bot".into(),
        chat: Some(42),
        ativo: true,
        tipos: TIPOS.iter().map(|t| (*t).into()).collect(),
        incluir_titulos: false,
    }
}
fn item(id: &str) -> Value {
    json!({"id":id,"conversa":"c","sessao":"s","projeto_id":"p","projeto":"PROJETO PRIVADO", "titulo":"TITULO PRIVADO", "plataforma":"codex",
    "conta_id":null,"conta":"Conta", "tipo":"precisa_de_voce", "desde_ms":1, "visto":false,"resumo":"Precisa de você", "orquestrador":null})
}

#[test]
fn destinatario_so_pareia_por_desafio_fresco_em_chat_privado() {
    let mut p = Pareando {
        codigo: "desafio".into(),
        inicio: now_ms() - 1_000,
        expira: now_ms() + 60_000,
        offset: 0,
    };
    let mut u = json!({"message":{"text":"/start desafio", "date":now_ms()/1000,"chat":{"id":42,"type":"private"},"from":{"id":42,"is_bot":false}}});
    assert_eq!(destinatario(&u, &p), Some(42));
    u["message"]["chat"]["type"] = json!("group");
    assert_eq!(destinatario(&u, &p), None);
    u["message"]["chat"]["type"] = json!("private");
    u["message"]["from"]["id"] = json!(43);
    assert_eq!(destinatario(&u, &p), None);
    u["message"]["from"]["id"] = json!(42);
    u["message"]["text"] = json!("/start outro");
    assert_eq!(destinatario(&u, &p), None);
    u["message"]["text"] = json!("/start desafio");
    u["message"]["date"] = json!(1);
    assert_eq!(destinatario(&u, &p), None);
    u["message"]["date"] = json!(now_ms() / 1000);
    p.expira = 1;
    assert_eq!(destinatario(&u, &p), None);
}

#[test]
fn token_nao_sai_no_status_e_arquivo_e_privado() {
    let (dir, state) = fixture();
    let c = config();
    salvar(dir.path(), &c).unwrap();
    state.mobile.telegram.dados.lock().unwrap().config = c.clone();
    let status = state.mobile.telegram.status().to_string();
    assert!(!status.contains(&c.token) && !status.contains("chat"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dir.path().join(ARQUIVO))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    assert_eq!(
        Runtime::abrir(dir.path()).dados.lock().unwrap().config.chat,
        Some(42)
    );
}

#[test]
fn aviso_sem_conteudo_e_link_sem_token() {
    let (_dir, state) = fixture();
    let mut c = config();
    let corpo = aviso(&item("um"), &c, None).to_string();
    assert!(
        corpo.contains("Precisa de você")
            && !corpo.contains("PRIVADO")
            && !corpo.contains(&c.token)
    );
    state.mobile.config.lock().unwrap().enabled = true;
    state.mobile.config.lock().unwrap().serve = true;
    *state.mobile.serve_state.lock().unwrap() = (Some("https://pc.tail123.ts.net".into()), None);
    let url = url_remota(&state, "c/duas partes").unwrap();
    assert_eq!(url, "https://pc.tail123.ts.net/#/c/c%2Fduas%20partes");
    assert_eq!(
        aviso(&item("um"), &c, Some(url))["reply_markup"]["inline_keyboard"][0][0]["text"],
        "Abrir conversa no Medza"
    );
    c.incluir_titulos = true;
    assert!(aviso(&item("um"), &c, None)
        .to_string()
        .contains("PROJETO PRIVADO"));
    state.mobile.config.lock().unwrap().enabled = false;
    assert!(url_remota(&state, "c").is_none());
}

#[tokio::test]
async fn rotas_so_no_desktop_e_configuracao_nao_expoe_token() {
    let (dir, state) = fixture();
    for (metodo, path, corpo) in [
        ("GET", "/telegram", ""),
        (
            "POST",
            "/telegram/conectar",
            r#"{"token":"123:xxxxxxxxxxxxxxxxxxxxxxxxx"}"#,
        ),
        ("POST", "/telegram/desconectar", "{}"),
        ("POST", "/telegram/testar", "{}"),
        (
            "POST",
            "/telegram",
            r#"{"ativo":false,"tipos":[],"incluir_titulos":false}"#,
        ),
    ] {
        let r = axum::http::Request::builder()
            .method(metodo)
            .uri(path)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(corpo))
            .unwrap();
        assert_eq!(
            crate::mobile::rotas()
                .with_state(state.clone())
                .oneshot(r)
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let c = config();
    state.mobile.telegram.dados.lock().unwrap().config = c.clone();
    let r = local(state.clone(), "GET", "/telegram", None, None, vec![]).await;
    let EngineResponse::Api { status, body } = r else {
        panic!()
    };
    assert_eq!(status, 200);
    assert!(!body.contains(&c.token));
    let r = local(
        state.clone(),
        "POST",
        "/telegram",
        Some(json!({"ativo":false,"tipos":["limite"],"incluir_titulos":true}).to_string()),
        None,
        vec![],
    )
    .await;
    assert!(matches!(r, EngineResponse::Api { status: 200, .. }));
    let salva = Runtime::abrir(dir.path());
    let d = salva.dados.lock().unwrap();
    assert!(!d.config.ativo && d.config.incluir_titulos && d.config.tipos.contains("limite"));
    drop(d);
    local(
        state,
        "POST",
        "/telegram/desconectar",
        Some("{}".into()),
        None,
        vec![],
    )
    .await;
    assert!(Runtime::abrir(dir.path())
        .dados
        .lock()
        .unwrap()
        .config
        .token
        .is_empty());
}

#[tokio::test]
async fn cliente_envia_json_e_redige_erros_que_podem_conter_segredo() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app=Router::new().route("/botfixture/sendMessage",post(|Json(c):Json<Value>| async move { assert_eq!(c["chat_id"],42); Json(json!({"ok":true,"result":{"message_id":1}})) }))
        .route("/botfixture/getMe",post(|| async { (StatusCode::TOO_MANY_REQUESTS,Json(json!({"ok":false,"error_code":429,"description":"fixture segredo privado","parameters":{"retry_after":42}}))) }));
    let servidor = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let cliente = api::Cliente::local(base);
    assert_eq!(
        cliente
            .chamar(
                "fixture",
                "sendMessage",
                json!({"chat_id":42,"text":"Teste"})
            )
            .await
            .ok()
            .unwrap()["message_id"],
        1
    );
    let e = cliente
        .chamar("fixture", "getMe", json!({}))
        .await
        .err()
        .unwrap();
    assert_eq!(e.espera_ms, 42_000);
    assert!(!e.texto.contains("segredo") && !e.texto.contains("fixture"));
    servidor.abort();
}

#[tokio::test]
async fn worker_avisa_sem_janela_sem_repetir_evento_nem_enviar_historico() {
    let (_dir, state) = fixture();
    state.mobile.telegram.dados.lock().unwrap().config = config();
    let chamadas = Arc::new(Mutex::new(Vec::<Value>::new()));
    let capturadas = chamadas.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().route(
        "/bot123:segredo-sintetico-nao-usar-em-rede/sendMessage",
        post(move |Json(c): Json<Value>| {
            let capturadas = capturadas.clone();
            async move {
                capturadas.lock().unwrap().push(c);
                Json(json!({"ok":true,"result":{}}))
            }
        }),
    );
    let servidor = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    caixa_de_atividade::registrar_fixture(&state, item("antigo"));
    let trabalhador = tokio::spawn(observar(
        state.clone(),
        api::Cliente::local(base),
        std::time::Duration::from_secs(3),
    ));
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(chamadas.lock().unwrap().is_empty());
    caixa_de_atividade::registrar_fixture(&state, item("novo"));
    tokio::time::sleep(std::time::Duration::from_millis(3_200)).await;
    assert_eq!(chamadas.lock().unwrap().len(), 1);
    tokio::time::sleep(std::time::Duration::from_millis(3_200)).await;
    assert_eq!(chamadas.lock().unwrap().len(), 1);
    state.mobile.telegram.dados.lock().unwrap().config.ativo = false;
    caixa_de_atividade::registrar_fixture(&state, item("desligado"));
    tokio::time::sleep(std::time::Duration::from_millis(3_200)).await;
    assert_eq!(chamadas.lock().unwrap().len(), 1);
    trabalhador.abort();
    servidor.abort();
}

/// Transporte em memória complementar aos testes HTTP acima: roda a fila completa sem exigir socket.
#[derive(Clone)]
struct BotEmMemoria {
    chamadas: Arc<Mutex<Vec<(String, Value)>>>,
    falhar: Arc<AtomicBool>,
    updates: Value,
}
impl api::BotApi for BotEmMemoria {
    async fn chamar(&self, _: &str, metodo: &str, corpo: Value) -> Result<Value, api::Falha> {
        self.chamadas.lock().unwrap().push((metodo.into(), corpo));
        if metodo == "getUpdates" {
            return Ok(self.updates.clone());
        }
        if self.falhar.swap(false, Ordering::SeqCst) {
            return Err(api::Falha {
                texto: "Falha sintética",
                espera_ms: 20,
            });
        }
        Ok(json!({"message_id":1}))
    }
}
fn bot() -> BotEmMemoria {
    BotEmMemoria {
        chamadas: Arc::new(Mutex::new(vec![])),
        falhar: Arc::new(AtomicBool::new(false)),
        updates: json!([]),
    }
}

#[tokio::test]
async fn pareamento_em_memoria_guarda_so_o_destinatario_do_desafio() {
    let (dir, state) = fixture();
    let mut bot = bot();
    let mut c = config();
    c.chat = None;
    c.ativo = false;
    *state.mobile.telegram.dados.lock().unwrap() = Dados {
        config: c,
        erro: None,
        pareando: Some(Pareando {
            codigo: "desafio".into(),
            inicio: now_ms() - 1_000,
            expira: now_ms() + 30_000,
            offset: 0,
        }),
    };
    bot.updates = json!([
        {"update_id":1,"message":{"text":"/start outro","date":now_ms()/1000,"chat":{"id":99,"type":"private"},"from":{"id":99,"is_bot":false}}},
        {"update_id":2,"message":{"text":"/start desafio","date":now_ms()/1000,"chat":{"id":42,"type":"private"},"from":{"id":42,"is_bot":false}}}
    ]);
    assert!(parear(&state, &bot).await.is_ok());
    let salvo = Runtime::abrir(dir.path());
    let d = salvo.dados.lock().unwrap();
    assert_eq!(d.config.chat, Some(42));
    assert!(d.config.ativo);
    assert!(state
        .mobile
        .telegram
        .dados
        .lock()
        .unwrap()
        .pareando
        .is_none());
    assert_eq!(bot.chamadas.lock().unwrap()[0].0, "getUpdates");
}

#[tokio::test]
async fn worker_em_memoria_reenvia_falha_sem_duplicar_e_respeita_desligado() {
    let (_dir, state) = fixture();
    let bot = bot();
    bot.falhar.store(true, Ordering::SeqCst);
    state.mobile.telegram.dados.lock().unwrap().config = config();
    caixa_de_atividade::registrar_fixture(&state, item("antigo"));
    let trabalhador = tokio::spawn(observar(
        state.clone(),
        bot.clone(),
        std::time::Duration::from_millis(10),
    ));
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(
        bot.chamadas.lock().unwrap().is_empty(),
        "não envia o histórico ao ligar"
    );
    caixa_de_atividade::registrar_fixture(&state, item("novo"));
    let prazo = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while bot.chamadas.lock().unwrap().len() < 2 {
        assert!(std::time::Instant::now() < prazo);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(
        bot.chamadas.lock().unwrap().len(),
        2,
        "uma falha e um reenvio, sem duplicar após sucesso"
    );
    assert!(state.mobile.telegram.dados.lock().unwrap().erro.is_none());
    state.mobile.telegram.dados.lock().unwrap().config.ativo = false;
    caixa_de_atividade::registrar_fixture(&state, item("desligado"));
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    state.mobile.telegram.dados.lock().unwrap().config.ativo = true;
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert_eq!(
        bot.chamadas.lock().unwrap().len(),
        2,
        "reativar não envia eventos do período desligado"
    );
    state
        .mobile
        .telegram
        .dados
        .lock()
        .unwrap()
        .config
        .tipos
        .clear();
    caixa_de_atividade::registrar_fixture(&state, item("filtrado"));
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert_eq!(
        bot.chamadas.lock().unwrap().len(),
        2,
        "respeita o tipo de aviso desligado"
    );
    trabalhador.abort();
}
