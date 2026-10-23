//! Fork (Rotina): bot da rotina com a Bot API em memória (nada sai para a rede).
use super::*;

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

const TOKEN: &str = "777:segredo-sintetico-da-rotina-nao-usar";

fn conectado(state: &EngineState) {
    runtime(state).dados.lock().unwrap().config = Config { token: TOKEN.into(), bot: "rotina_teste_bot".into(), chat: Some(42), ativo: true };
}

#[derive(Clone)]
struct Bot {
    chamadas: Arc<Mutex<Vec<(String, Value)>>>,
    updates: Arc<Mutex<Value>>,
}
impl api::BotApi for Bot {
    async fn chamar(&self, token: &str, metodo: &str, corpo: Value) -> Result<Value, api::Falha> {
        assert_eq!(token, TOKEN);
        self.chamadas.lock().unwrap().push((metodo.into(), corpo));
        if metodo == "getUpdates" {
            return Ok(std::mem::replace(&mut *self.updates.lock().unwrap(), json!([])));
        }
        Ok(json!({"message_id": 9}))
    }
}
fn bot() -> Bot {
    Bot { chamadas: Arc::new(Mutex::new(vec![])), updates: Arc::new(Mutex::new(json!([]))) }
}
impl Bot {
    fn de(&self, metodo: &str) -> Vec<Value> {
        self.chamadas.lock().unwrap().iter().filter(|(m, _)| m == metodo).map(|(_, c)| c.clone()).collect()
    }
}

fn agora() -> operacoes::Agora {
    super::super::super::rotina_rotas::agora()
}

/// Grava um aviso como se o relógio tivesse disparado agora.
fn disparar(dir: &Path, d: lembretes::Disparo) {
    let _t = rotina_core::travar();
    let mut estado: lembretes::Estado = rotina_core::ler(dir);
    estado.registrar(&[d], now_ms() as i64);
    rotina_core::gravar(dir, &estado).unwrap();
}

fn agua(chave: &str) -> lembretes::Disparo {
    lembretes::Disparo { chave: chave.into(), tipo: "agua".into(), titulo: "Beba água".into(), texto: "0,5 L de 2,5 L hoje".into(),
        alvo: "agua".into(), data: rotina_core::texto_da_data(agora().hoje), botoes: vec!["copo".into(), "adiar".into(), "pular".into()], ..Default::default() }
}

#[test]
fn botoes_cabem_no_callback_data_e_mensagem_leva_o_texto() {
    let d = lembretes::Disparo { chave: format!("tarefa:{}:2026-10-23:15:00", "t".repeat(80)), tipo: "tarefa".into(), titulo: "Dentista".into(),
        texto: "Às 15:00".into(), botoes: vec!["feito".into(), "adiar".into(), "pular".into()], ..Default::default() };
    let m = mensagem_do_lembrete(42, &d);
    assert!(m["text"].as_str().unwrap().contains("Dentista"));
    for b in m["reply_markup"]["inline_keyboard"][0].as_array().unwrap() {
        assert!(b["callback_data"].as_str().unwrap().len() <= 64);
    }
    assert_eq!(m["reply_markup"]["inline_keyboard"][0][1]["text"], "⏰ Adiar 15 min");
}

#[test]
fn token_nao_sai_no_status_e_arquivo_e_privado() {
    let (dir, state) = fixture();
    conectado(&state);
    let c = runtime(&state).dados.lock().unwrap().config.clone();
    salvar_config(dir.path(), &c).unwrap();
    let status = runtime(&state).status().to_string();
    assert!(!status.contains(TOKEN) && !status.contains("777"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(caminho(dir.path())).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert_eq!(Runtime::abrir(dir.path()).dados.lock().unwrap().config.chat, Some(42));
}

#[tokio::test]
async fn manda_lembrete_novo_uma_vez_e_nao_manda_historico() {
    let (dir, state) = fixture();
    disparar(dir.path(), agua("agua:antigo"));
    conectado(&state);
    let b = bot();
    let mut m = Memoria::default();
    passo(&state, &b, &mut m).await.ok().unwrap();
    assert!(b.de("sendMessage").is_empty(), "o que já existia não sai");
    disparar(dir.path(), agua("agua:novo"));
    passo(&state, &b, &mut m).await.ok().unwrap();
    passo(&state, &b, &mut m).await.ok().unwrap();
    let enviados = b.de("sendMessage");
    assert_eq!(enviados.len(), 1);
    assert!(enviados[0]["text"].as_str().unwrap().contains("Beba água"));
    assert_eq!(enviados[0]["chat_id"], 42);
    // Lembretes pelo Telegram desligados nos ajustes: não manda.
    operacoes::executar(dir.path(), "salvar_ajustes", &json!({"telegram_lembretes": false}), &agora()).unwrap();
    disparar(dir.path(), agua("agua:calado"));
    passo(&state, &b, &mut m).await.ok().unwrap();
    assert_eq!(b.de("sendMessage").len(), 1);
    // Bot desligado: nem lê.
    runtime(&state).dados.lock().unwrap().config.ativo = false;
    passo(&state, &b, &mut m).await.ok().unwrap();
    assert_eq!(b.de("getUpdates").len(), 4, "o passo desligado não chama a API");
}

#[tokio::test]
async fn botao_do_dono_responde_e_de_estranho_nao() {
    let (dir, state) = fixture();
    conectado(&state);
    let b = bot();
    let mut m = Memoria::default();
    passo(&state, &b, &mut m).await.ok().unwrap();
    disparar(dir.path(), agua("agua:botao"));
    let cod = codigo("agua:botao");
    *b.updates.lock().unwrap() = json!([
        {"update_id": 5, "callback_query": {"id": "q1", "data": format!("r|copo|{cod}"), "from": {"id": 99}, "message": {"message_id": 3, "chat": {"id": 42}}}},
        {"update_id": 6, "callback_query": {"id": "q2", "data": format!("r|copo|{cod}"), "from": {"id": 42}, "message": {"message_id": 3, "chat": {"id": 42}}}},
    ]);
    passo(&state, &b, &mut m).await.ok().unwrap();
    let respostas = b.de("answerCallbackQuery");
    assert_eq!(respostas.len(), 2);
    assert!(respostas[0].get("text").is_none(), "estranho não muda nada");
    assert!(respostas[1]["text"].as_str().unwrap().contains("250 ml"), "{}", respostas[1]);
    assert_eq!(b.de("editMessageReplyMarkup").len(), 1, "tira os botões");
    let dia = operacoes::executar(dir.path(), "ler_dia", &json!({}), &agora()).unwrap();
    assert_eq!(dia["agua"]["ml"], 250);
    assert_eq!(m.offset, 7);
    // Adiar gera um adiado no estado.
    disparar(dir.path(), agua("agua:adiar"));
    *b.updates.lock().unwrap() = json!([{"update_id": 7, "callback_query": {"id": "q3", "data": format!("r|adiar|{}", codigo("agua:adiar")),
        "from": {"id": 42}, "message": {"message_id": 4, "chat": {"id": 42}}}}]);
    passo(&state, &b, &mut m).await.ok().unwrap();
    let estado: lembretes::Estado = rotina_core::ler(dir.path());
    assert_eq!(estado.adiados.len(), 1);
}

#[tokio::test]
async fn comandos_e_texto_livre_so_com_a_conversa_ligada() {
    let (dir, state) = fixture();
    conectado(&state);
    let b = bot();
    let mut m = Memoria::default();
    passo(&state, &b, &mut m).await.ok().unwrap();
    let msg = |id: i64, texto: &str, de: i64, data: u64| json!({"update_id": id, "message": {"text": texto, "date": data,
        "chat": {"id": de, "type": "private"}, "from": {"id": de, "is_bot": false}}});
    let agora_s = now_ms() / 1000;
    *b.updates.lock().unwrap() = json!([msg(1, "/hoje", 42, agora_s), msg(2, "/agua", 42, agora_s), msg(3, "oi", 99, agora_s),
        msg(4, "mensagem velha", 42, 1), msg(5, "cancelaram a aula", 42, agora_s)]);
    passo(&state, &b, &mut m).await.ok().unwrap();
    let enviados: Vec<String> = b.de("sendMessage").iter().map(|c| c["text"].as_str().unwrap().to_owned()).collect();
    assert_eq!(enviados.len(), 3, "{enviados:?}");
    assert!(enviados[0].starts_with("Hoje"));
    assert!(enviados[1].contains("250 ml"));
    assert!(enviados[2].contains("Conversar pelo Telegram"), "conversa desligada por padrão");
    // Ligada: vai para o agente da rotina (aqui sem CLI publicada, a resposta é o erro).
    operacoes::executar(dir.path(), "salvar_ajustes", &json!({"telegram_conversa": true}), &agora()).unwrap();
    *b.updates.lock().unwrap() = json!([msg(8, "cancelaram a aula", 42, agora_s)]);
    passo(&state, &b, &mut m).await.ok().unwrap();
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while b.de("sendMessage").len() < 4 && std::time::Instant::now() < limite { tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    let ultima = b.de("sendMessage").last().unwrap()["text"].as_str().unwrap().to_owned();
    assert!(ultima.contains("Claude Code ou Codex"), "{ultima}");
    assert_eq!(b.de("sendChatAction").len(), 1);
}

#[tokio::test]
async fn pareia_so_pelo_desafio_no_chat_privado() {
    let (dir, state) = fixture();
    runtime(&state).dados.lock().unwrap().config = Config { token: TOKEN.into(), bot: "rotina_teste_bot".into(), ..Default::default() };
    runtime(&state).dados.lock().unwrap().pareando = Some(Pareando { codigo: "abc".into(), inicio: now_ms() - 1000, expira: now_ms() + 60_000, offset: 0 });
    let b = bot();
    *b.updates.lock().unwrap() = json!([
        {"update_id": 1, "message": {"text": "/start abc", "date": now_ms() / 1000, "chat": {"id": 7, "type": "group"}, "from": {"id": 7, "is_bot": false}}},
        {"update_id": 2, "message": {"text": "/start abc", "date": now_ms() / 1000, "chat": {"id": 42, "type": "private"}, "from": {"id": 42, "is_bot": false}}},
    ]);
    let mut m = Memoria::default();
    passo(&state, &b, &mut m).await.ok().unwrap();
    let d = Runtime::abrir(dir.path());
    let c = d.dados.lock().unwrap().config.clone();
    assert_eq!((c.chat, c.ativo), (Some(42), true));
}
