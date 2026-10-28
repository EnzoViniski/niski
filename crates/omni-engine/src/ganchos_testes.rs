//! Testes dos ganchos com **CLI falsa** (`sh` + `curl`) que faz POST nos ganchos como o Claude
//! Code faria. Nenhuma CLI real. Cobre: token errado/PTY morta (401), Host/Origin (403), corpo grande
//! (413), turno e aviso pelos ganchos, troca de transcript, permissão permitida/negada/solta/vencida,
//! "Responder no terminal", sessão sem eventos caindo na tela, desligar pelas Configurações e statusline.

use super::*;
use axum::body::{to_bytes, Body};
use tower::ServiceExt;

const PORTA_FALSA: u16 = 47_998;

struct Cena { dir: tempfile::TempDir, state: Arc<EngineState>, projeto: PathBuf, conta: PathBuf, porta: u16 }

fn cena(porta: u16) -> Cena {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dir.path()),
        token: "engine-token-de-teste".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    definir_porta(&state, porta);
    // Ganchos vêm desligados por padrão; os testes ligam explicitamente.
    omni_core::ganchos::gravar_ajustes(dir.path(), &omni_core::ganchos::Ajustes { ligado: true, ..Default::default() }).unwrap();
    let projeto = dir.path().join("projeto");
    let conta = dir.path().join("conta-claude");
    fs::create_dir_all(&projeto).unwrap();
    fs::create_dir_all(&conta).unwrap();
    Cena { dir, state, projeto, conta, porta }
}

impl Cena {
    fn bin(&self) -> PathBuf { let pasta = self.dir.path().join("bin"); fs::create_dir_all(&pasta).unwrap(); pasta }

    /// Abre uma PTY de conversa Claude com o comando dado (a conversa `conv` aponta para ela).
    fn abrir(&self, comando: &str) -> (String, Arc<LiveSession>) {
        let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
        atomic_write_json(&self.dir.path().join("conversations.json"), &json!({"version": 1, "conversations": [{
            "id": "conv", "project_id": "p", "cwd": self.projeto.to_string_lossy(), "title": "conv", "created_at_ms": 1,
            "segments": [{"provider": "claude", "profile_id": "claude-p", "external_session_id": null, "transcript_path": null,
                "terminal_session_id": terminal, "started_at_ms": 1, "ended_at_ms": null}]}]})).unwrap();
        spawn_terminal_inner(&self.state, terminal.clone(), "p".into(), "Claude".into(), self.projeto.to_string_lossy().into_owned(),
            Some("/bin/sh".into()), Some(comando.into()), 30, 100, now_ms(), self.origem()).unwrap();
        let sessao = viva(&self.state, &terminal).unwrap();
        (terminal, sessao)
    }

    fn origem(&self) -> SessionOrigin {
        SessionOrigin { provider: Some("claude".into()), profile_id: Some("claude-p".into()), conversation_id: Some("conv".into()),
            env: vec![("CLAUDE_CONFIG_DIR".into(), self.conta.to_string_lossy().into_owned())], ..SessionOrigin::default() }
    }

    fn pedido(&self, rota: &str, token: Option<&str>, corpo: Value) -> axum::http::Request<Body> {
        let mut pedido = axum::http::Request::builder().method("POST").uri(rota)
            .header("host", format!("127.0.0.1:{}", self.porta)).header("content-type", "application/json");
        if let Some(token) = token { pedido = pedido.header("authorization", format!("Bearer {token}")); }
        pedido.body(Body::from(corpo.to_string())).unwrap()
    }

    async fn chamar(&self, pedido: axum::http::Request<Body>) -> (StatusCode, String) {
        let resposta = router(self.state.clone()).oneshot(pedido).await.unwrap();
        let status = resposta.status();
        (status, String::from_utf8_lossy(&to_bytes(resposta.into_body(), usize::MAX).await.unwrap()).into_owned())
    }

    async fn api(&self, metodo: &str, rota: &str, corpo: Option<Value>) -> (u16, Value) {
        let EngineResponse::Api { status, body, .. } = mobile::local(self.state.clone(), metodo, rota, corpo.map(|c| c.to_string()), None,
            vec![("idempotency-key".into(), uuid::Uuid::new_v4().to_string())]).await else { panic!("resposta da API") };
        (status, serde_json::from_str(&body).unwrap_or(Value::Null))
    }

    fn fechar(&self, terminal: &str) { let _ = close_session(&self.state, terminal); }
}

fn token_de(terminal: &str) -> String {
    RUNTIME.tokens.lock().unwrap().iter().find(|(_, dono)| dono.as_str() == terminal).map(|(t, _)| t.clone()).expect("token da PTY")
}

/// CLI que só espera (para os testes que fazem os POSTs eles mesmos).
fn quieta(cena: &Cena) -> String {
    let script = cena.bin().join("claude");
    fs::write(&script, "#!/bin/sh\nprintf '\\033[?2004h\\r\\n\\342\\235\\257 '\nwhile IFS= read -r l; do :; done\n").unwrap();
    format!("/bin/sh {}", script.to_string_lossy())
}

/// `claude` de mentira **com ganchos**: lê o `--settings`, tira as URLs com `sed` e, a cada linha
/// recebida, faz `UserPromptSubmit` … `Stop` com `curl`. "PEDIR" pede permissão (bloqueia até a
/// resposta, que vai para `respostas.txt`); "trabalhe" fica 3 s no turno **sem** rodapé de turno (como
/// com uma statusline própria).
fn com_ganchos(cena: &Cena) -> String {
    let script = cena.bin().join("claude");
    fs::write(&script, r#"#!/bin/sh
ARQ=""
while [ $# -gt 0 ]; do [ "$1" = "--settings" ] && ARQ="$2"; shift; done
DIR="$(dirname "$0")"
EVENTOS=$(sed -n 's/.*"url": *"\(http[^"]*\/ganchos\)".*/\1/p' "$ARQ" | head -n 1)
PERMISSAO=$(sed -n 's/.*"url": *"\(http[^"]*\/ganchos\/permissao\)".*/\1/p' "$ARQ" | head -n 1)
post() { curl -s -m 20 -H "Authorization: Bearer $MEDZA_GANCHO_TOKEN" -H 'Content-Type: application/json' --data "$2" "$1"; }
evento() { post "$EVENTOS" "{\"hook_event_name\":\"$1\",\"session_id\":\"s-falsa\",\"transcript_path\":\"/tmp/s-falsa.jsonl\"$2}" >/dev/null; }
printf '\033[?2004h'
while :; do
  printf '\r\n\342\235\257 '
  IFS= read -r linha || exit 0
  [ -z "$linha" ] && continue
  evento UserPromptSubmit
  case "$linha" in
    *PEDIR*)
      evento PreToolUse ',"tool_name":"Bash","tool_input":{"command":"ls src"}'
      R=$(post "$PERMISSAO" '{"hook_event_name":"PermissionRequest","session_id":"s-falsa","tool_name":"Bash","tool_input":{"command":"ls src"}}')
      [ -z "$R" ] && R=vazio
      printf '%s\n' "$R" >> "$DIR/respostas.txt" ;;
    *trabalhe*) printf '\r\n pensando'; sleep 3 ;;
  esac
  evento Stop
done
"#).unwrap();
    format!("/bin/sh {}", script.to_string_lossy())
}

/// Versão "antiga": nunca manda evento; o turno aparece só na tela ("esc to interrupt").
fn sem_ganchos(cena: &Cena) -> String {
    let script = cena.bin().join("claude");
    fs::write(&script, r#"#!/bin/sh
printf '\033[?2004h'
while :; do
  printf '\r\n\342\235\257 '
  IFS= read -r linha || exit 0
  [ -z "$linha" ] && continue
  printf '\r\n esc to interrupt'; sleep 2; printf '\r\033[2K ok\r\n'
done
"#).unwrap();
    format!("/bin/sh {}", script.to_string_lossy())
}

async fn ate(segundos: u64, mut condicao: impl FnMut() -> bool) -> bool {
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(segundos);
    while std::time::Instant::now() < limite {
        if condicao() { return true; }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    condicao()
}

/// Sobe o servidor de verdade (TCP) numa porta livre, para a CLI falsa alcançar com `curl`.
async fn servidor(state: &Arc<EngineState>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = listener.local_addr().unwrap().port();
    definir_porta(state, porta);
    let app = router(state.clone());
    tokio::spawn(async move { let _ = axum::serve(listener, app).await; });
    porta
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn token_errado_e_pty_morta_401_origem_403_corpo_grande_413() {
    let c = cena(PORTA_FALSA);
    let (terminal, _sessao) = c.abrir(&quieta(&c));
    let token = token_de(&terminal);
    let corpo = json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"});

    assert_eq!(c.chamar(c.pedido("/ganchos", None, corpo.clone())).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(c.chamar(c.pedido("/ganchos", Some("errado"), corpo.clone())).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(c.chamar(c.pedido("/ganchos", Some("engine-token-de-teste"), corpo.clone())).await.0, StatusCode::UNAUTHORIZED,
        "o engine.token não vale aqui");
    let mut de_fora = c.pedido("/ganchos", Some(&token), corpo.clone());
    de_fora.headers_mut().insert("origin", "https://site.example".parse().unwrap());
    assert_eq!(c.chamar(de_fora).await.0, StatusCode::FORBIDDEN);
    let mut outro_host = c.pedido("/ganchos", Some(&token), corpo.clone());
    outro_host.headers_mut().insert("host", "evil.example:47998".parse().unwrap());
    assert_eq!(c.chamar(outro_host).await.0, StatusCode::FORBIDDEN);
    let grande = json!({"hook_event_name": "PreToolUse", "tool_input": {"content": "x".repeat(regra::LIMITE_DO_CORPO + 10)}});
    assert_eq!(c.chamar(c.pedido("/ganchos", Some(&token), grande)).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(c.chamar(c.pedido("/ganchos", Some(&token), json!({"sem": "nome"}))).await.0, StatusCode::BAD_REQUEST);
    let (status, corpo_da_resposta) = c.chamar(c.pedido("/ganchos", Some(&token), corpo.clone())).await;
    assert_eq!((status, corpo_da_resposta.as_str()), (StatusCode::OK, ""), "2xx vazio: sem decisão");

    // O arquivo da sessão não tem segredo e a linha digitada também não.
    let arquivo = c.dir.path().join(regra::PASTA_DOS_ARQUIVOS).join(format!("{terminal}.json"));
    let conteudo = fs::read_to_string(&arquivo).unwrap();
    assert!(conteudo.contains("${MEDZA_GANCHO_TOKEN}") && !conteudo.contains(&token));
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; assert_eq!(fs::metadata(&arquivo).unwrap().permissions().mode() & 0o777, 0o600); }
    let sessoes = fs::read_to_string(c.dir.path().join("sessions.json")).unwrap();
    assert!(!sessoes.contains(&token) && !sessoes.contains("--settings"), "token e flag fora do sessions.json");

    // PTY parada: o token morre junto.
    stop_session(&c.state, &terminal).unwrap();
    assert_eq!(c.chamar(c.pedido("/ganchos", Some(&token), corpo)).await.0, StatusCode::UNAUTHORIZED);
    c.fechar(&terminal);
    assert!(!arquivo.exists(), "fechar a sessão apaga o arquivo");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn turno_pelos_ganchos_aviso_uma_vez_e_troca_de_transcript() {
    let c = cena(PORTA_FALSA + 1);
    let (terminal, sessao) = c.abrir(&quieta(&c));
    assert!(ate(10, || viva(&c.state, &terminal).is_some_and(|s| s.interaction.lock().unwrap().parser.screen().contents().contains('❯'))).await);
    let token = token_de(&terminal);
    let evento = |nome: &str, extra: Value| { let mut v = json!({"hook_event_name": nome, "session_id": "sess-1",
        "transcript_path": c.conta.join("sess-1.jsonl").to_string_lossy()}); for (k, x) in extra.as_object().unwrap() { v[k] = x.clone(); } v };
    assert_eq!(resumo(&terminal)["estado"], "tela", "antes do 1º evento vale a tela");
    assert_eq!(em_turno(&terminal), None);

    let antes = sessao.meta.lock().unwrap().attention_seq;
    assert_eq!(c.chamar(c.pedido("/ganchos", Some(&token), evento("UserPromptSubmit", json!({"permission_mode": "plan"})))).await.0, StatusCode::OK);
    assert_eq!(em_turno(&terminal), Some(true));
    assert_eq!(resumo(&terminal)["estado"], "ativos");
    assert_eq!(resumo(&terminal)["modo"], "plan");
    std::thread::sleep(std::time::Duration::from_millis(1_700));
    assert_eq!(settle(&sessao).state, SessionState::Working, "sem rodapé na tela, o turno vem do gancho");
    assert!(!interaction::capabilities(&sessao).prompt, "no meio do turno não é prompt");
    c.chamar(c.pedido("/ganchos", Some(&token), evento("PreToolUse", json!({"tool_name": "Bash", "tool_input": {"command": "npm test"}})))).await;
    assert_eq!(resumo(&terminal)["ferramenta"], "Bash · npm test");
    // Fork (harness, A6): a rota do harness leva só o nome da ferramenta, sem o comando.
    assert_eq!(estado_do_harness(&terminal).unwrap()["ferramenta"], "Bash");

    c.chamar(c.pedido("/ganchos", Some(&token), evento("Stop", json!({"last_assistant_message": "não guardamos"})))).await;
    assert_eq!(sessao.meta.lock().unwrap().state, SessionState::Answered);
    assert_eq!(sessao.meta.lock().unwrap().attention_seq, antes + 1);
    assert_eq!(tipo(&terminal), Some("terminou"));
    c.chamar(c.pedido("/ganchos", Some(&token), evento("Stop", json!({})))).await;
    std::thread::sleep(std::time::Duration::from_millis(1_700));
    settle(&sessao);
    assert_eq!(sessao.meta.lock().unwrap().attention_seq, antes + 1, "um aviso por turno");

    // StopFailure com limite: o aviso vale mesmo sem a linha na tela.
    c.chamar(c.pedido("/ganchos", Some(&token), evento("UserPromptSubmit", json!({})))).await;
    c.chamar(c.pedido("/ganchos", Some(&token), evento("StopFailure", json!({"error": "rate_limit"})))).await;
    assert_eq!(sessao.meta.lock().unwrap().notice.as_deref(), Some("usage_limit"));
    assert_eq!(tipo(&terminal), Some("limite"));
    assert_eq!(sessao.meta.lock().unwrap().attention_seq, antes + 2);

    // Fork (harness, A6): o estado do harness conta turnos, compactações e o último erro, sem texto.
    c.chamar(c.pedido("/ganchos", Some(&token), evento("PostCompact", json!({"trigger": "auto", "compact_summary": "resumo secreto"})))).await;
    let harness = estado_do_harness(&terminal).unwrap();
    assert_eq!((harness["canal"].as_str(), harness["confirmado"].as_bool()), (Some("ganchos"), Some(true)));
    assert_eq!(harness["turnos"], json!({"iniciados": 2, "terminados": 3}), "{harness}");
    assert_eq!(harness["compactacoes"], 1);
    assert_eq!(harness["ultima_compactacao"]["gatilho"], "auto");
    assert_eq!(harness["ultimo_erro"]["codigo"], "rate_limit");
    assert_eq!(harness["modo_de_permissao"], "plan");
    let texto = harness.to_string();
    assert!(!texto.contains("secreto") && !texto.contains("npm test") && !texto.contains("não guardamos"), "{texto}");

    // Outra sessão da CLI (`/clear`): o segmento da conversa passa a apontar para ela.
    let indice = omni_core::conversations::read_index(c.dir.path());
    assert_eq!(indice[0].segments[0].external_session_id.as_deref(), Some("sess-1"));
    let novo = c.conta.join("sess-2.jsonl").to_string_lossy().into_owned();
    c.chamar(c.pedido("/ganchos", Some(&token), json!({"hook_event_name": "UserPromptSubmit", "session_id": "sess-2", "transcript_path": novo}))).await;
    let indice = omni_core::conversations::read_index(c.dir.path());
    assert_eq!((indice[0].segments[0].external_session_id.as_deref(), indice[0].segments[0].transcript_path.as_deref()), (Some("sess-2"), Some(novo.as_str())));

    // Esc no meio do turno: o Stop não vem, a tela volta a valer.
    assert_eq!(em_turno(&terminal), Some(true));
    write_terminal(&c.state, &terminal, "\x1b").unwrap();
    assert_eq!(em_turno(&terminal), None);
    c.fechar(&terminal);
}

/// Espera o pedido aparecer na conversa e devolve o id.
async fn pedido_pendente(terminal: &str) -> PedidoVisivel {
    assert!(ate(10, || pedido(terminal).is_some()).await, "o pedido não chegou");
    pedido(terminal).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn permissao_permitida_negada_solta_vencida_e_pergunta_da_tela() {
    let c = cena(PORTA_FALSA + 2);
    let (terminal, sessao) = c.abrir(&quieta(&c));
    let token = token_de(&terminal);
    let corpo = json!({"hook_event_name": "PermissionRequest", "session_id": "s", "tool_name": "Bash", "tool_input": {"command": "rm -rf build"}});
    let pedir = |corpo: Value| { let (pedido, app) = (c.pedido("/ganchos/permissao", Some(&token), corpo), router(c.state.clone()));
        tokio::spawn(async move { let r = app.oneshot(pedido).await.unwrap(); let s = r.status();
            (s, String::from_utf8_lossy(&to_bytes(r.into_body(), usize::MAX).await.unwrap()).into_owned()) }) };

    // Permitir pela conversa (com o id que a tela mostrou).
    let antes = sessao.meta.lock().unwrap().attention_seq;
    let espera = pedir(corpo.clone());
    let visto = pedido_pendente(&terminal).await;
    assert_eq!((visto.origem, visto.ferramenta.as_str(), visto.resumo.as_str()), ("gancho", "Bash", "Bash · rm -rf build"));
    assert_eq!(visto.arriscado, Some("apaga arquivos em massa"));
    let cap = interaction::capabilities(&sessao);
    assert!(cap.approve && !cap.prompt && cap.approval_text.as_deref() == Some("Bash · rm -rf build"));
    assert_eq!(sessao.meta.lock().unwrap().state, SessionState::ApprovalRequired);
    assert_eq!(sessao.meta.lock().unwrap().attention_seq, antes + 1);
    assert_eq!(tipo(&terminal), Some("precisa_de_voce"));
    let (status, _) = c.api("POST", "/conversas/conv/aprovar", Some(json!({"permitir": true}))).await;
    assert_eq!(status, 412, "sem o id do pedido, ninguém aprova às cegas");
    let (status, _) = c.api("POST", "/conversas/conv/aprovar", Some(json!({"permitir": true, "pedido": "outro"}))).await;
    assert_eq!(status, 412);
    let (status, corpo_api) = c.api("POST", "/conversas/conv/aprovar", Some(json!({"permitir": true, "pedido": visto.id}))).await;
    assert_eq!(status, 200, "{corpo_api}");
    let (status, saida) = espera.await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_str::<Value>(&saida).unwrap(),
        json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}}));
    assert!(pedido(&terminal).is_none());
    assert_eq!(sessao.meta.lock().unwrap().state, SessionState::Working);
    let (status, _) = c.api("POST", "/conversas/conv/aprovar", Some(json!({"permitir": true, "pedido": visto.id}))).await;
    assert_eq!(status, 409, "já respondido");

    // Negar com "o que fazer".
    let espera = pedir(corpo.clone());
    let visto = pedido_pendente(&terminal).await;
    let (status, _) = c.api("POST", "/conversas/conv/aprovar", Some(json!({"permitir": false, "pedido": visto.id, "mensagem": "use git clean"}))).await;
    assert_eq!(status, 200);
    let saida: Value = serde_json::from_str(&espera.await.unwrap().1).unwrap();
    assert_eq!(saida["hookSpecificOutput"]["decision"], json!({"behavior": "deny", "message": "Negado no Niski: use git clean"}));
    assert!(saida["hookSpecificOutput"]["decision"].get("updatedPermissions").is_none());

    // "Responder no terminal": resposta vazia, a CLI mostra o diálogo dela.
    let espera = pedir(corpo.clone());
    let visto = pedido_pendente(&terminal).await;
    let (status, _) = c.api("POST", "/conversas/conv/pedido/terminal", Some(json!({"pedido": "outro"}))).await;
    assert_eq!(status, 412);
    let (status, _) = c.api("POST", "/conversas/conv/pedido/terminal", Some(json!({"pedido": visto.id}))).await;
    assert_eq!(status, 200);
    assert_eq!(espera.await.unwrap(), (StatusCode::OK, String::new()));

    // Prazo: sem resposta, vazio.
    PRAZOS_DE_TESTE.lock().unwrap().insert(dir_de(&c.state), 400);
    let espera = pedir(corpo.clone());
    assert_eq!(espera.await.unwrap(), (StatusCode::OK, String::new()));
    assert!(pedido(&terminal).is_none(), "vencido sai da tela");

    // Pergunta e plano: soltos na hora (o cartão da tela responde).
    let pergunta = json!({"hook_event_name": "PermissionRequest", "tool_name": "AskUserQuestion", "tool_input": {"questions": []}});
    assert_eq!(c.chamar(c.pedido("/ganchos/permissao", Some(&token), pergunta)).await, (StatusCode::OK, String::new()));
    c.fechar(&terminal);
}

/// Ponta a ponta: CLI falsa que faz POST de verdade (TCP + `curl`) nos ganchos.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_falsa_com_ganchos_permite_nega_solta_e_segue_sem_rodape() {
    let c = cena(0);
    servidor(&c.state).await;
    let (terminal, sessao) = c.abrir(&com_ganchos(&c));
    let respostas = c.bin().join("respostas.txt");
    let lidas = || fs::read_to_string(&respostas).unwrap_or_default();
    assert!(ate(15, || sessao.interaction.lock().unwrap().parser.screen().contents().contains('❯')).await, "a CLI falsa não subiu");

    let antes = sessao.meta.lock().unwrap().attention_seq;
    write_terminal(&c.state, &terminal, "oi\r").unwrap();
    assert!(ate(15, || sessao.meta.lock().unwrap().attention_seq > antes).await, "Stop não virou aviso");
    assert_eq!(resumo(&terminal)["estado"], "ativos");

    // Turno sem "esc to interrupt" na tela (statusline própria): o estado vem dos ganchos.
    write_terminal(&c.state, &terminal, "trabalhe\r").unwrap();
    assert!(ate(5, || em_turno(&terminal) == Some(true)).await);
    std::thread::sleep(std::time::Duration::from_millis(1_800));
    assert_eq!(settle(&sessao).state, SessionState::Working);
    assert!(ate(10, || em_turno(&terminal) != Some(true)).await);

    write_terminal(&c.state, &terminal, "PEDIR\r").unwrap();
    let visto = pedido_pendente(&terminal).await;
    assert_eq!(visto.resumo, "Bash · ls src");
    let (status, _) = c.api("POST", "/conversas/conv/aprovar", Some(json!({"permitir": true, "pedido": visto.id}))).await;
    assert_eq!(status, 200);
    assert!(ate(10, || lidas().contains("\"allow\"")).await, "a CLI falsa recebeu: {}", lidas());

    write_terminal(&c.state, &terminal, "PEDIR\r").unwrap();
    let visto = pedido_pendente(&terminal).await;
    c.api("POST", "/conversas/conv/aprovar", Some(json!({"permitir": false, "pedido": visto.id, "mensagem": "não liste"}))).await;
    assert!(ate(10, || lidas().contains("Negado no Niski: não liste")).await, "{}", lidas());

    write_terminal(&c.state, &terminal, "PEDIR\r").unwrap();
    let visto = pedido_pendente(&terminal).await;
    c.api("POST", "/conversas/conv/pedido/terminal", Some(json!({"pedido": visto.id}))).await;
    assert!(ate(10, || lidas().lines().any(|l| l == "vazio")).await, "{}", lidas());

    let token = token_de(&terminal);
    let sessoes = fs::read_to_string(c.dir.path().join("sessions.json")).unwrap();
    assert!(!sessoes.contains(&token) && !sessoes.contains(regra::VARIAVEL_DO_TOKEN));
    c.fechar(&terminal);
}

/// Versão antiga (ou pasta sem confiança): nenhum evento. A sessão segue pela tela e fica `sem_eventos`.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sessao_sem_eventos_cai_na_tela() {
    let c = cena(0);
    servidor(&c.state).await;
    let (terminal, sessao) = c.abrir(&sem_ganchos(&c));
    assert!(ate(15, || sessao.interaction.lock().unwrap().parser.screen().contents().contains('❯')).await);
    let antes = sessao.meta.lock().unwrap().attention_seq;
    write_terminal(&c.state, &terminal, "oi\r").unwrap();
    assert!(ate(5, || sessao.interaction.lock().unwrap().turn_active).await, "a tela viu o turno");
    assert_eq!(em_turno(&terminal), None);
    assert!(ate(15, || { settle(&sessao); sessao.meta.lock().unwrap().attention_seq > antes }).await, "fim do turno pela tela");
    assert_eq!(sessao.meta.lock().unwrap().state, SessionState::Answered);
    assert!(ate(5, || resumo(&terminal)["estado"] == "sem_eventos").await, "{}", resumo(&terminal));
    c.fechar(&terminal);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn desligado_pelas_configuracoes_ou_pelo_usuario_nao_injeta() {
    let c = cena(PORTA_FALSA + 3);
    let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
    let cwd = c.projeto.to_string_lossy().into_owned();
    let (comando, ambiente) = preparar(&c.state, &terminal, &c.origem(), &cwd, Some("claude --resume x".into()));
    assert!(comando.as_deref().unwrap().contains(" --settings '") && ambiente.iter().any(|(k, _)| k == regra::VARIAVEL_DO_TOKEN));
    assert!(ambiente.iter().all(|(k, _)| k != regra::VARIAVEL_DA_STATUSLINE_DO_USUARIO), "statusline só com a conta ligada");

    let arquivo = c.dir.path().join(regra::PASTA_DOS_ARQUIVOS).join(format!("{terminal}.json"));
    assert!(fs::read_to_string(&arquivo).unwrap().contains("PermissionRequest"), "permissões ligadas na cena");

    // Fork (harness, A1): permissões pelo app desligadas = só observar; a CLI mostra o diálogo dela.
    let (status, ajustes) = c.api("POST", "/ganchos/ajustes", Some(json!({"ligado": false}))).await;
    assert_eq!(status, 200);
    assert_eq!((ajustes["eventos"].as_bool(), ajustes["ligado"].as_bool()), (Some(true), Some(false)));
    let (comando, ambiente) = preparar(&c.state, &terminal, &c.origem(), &cwd, Some("claude --resume x".into()));
    assert!(comando.as_deref().unwrap().contains(" --settings '") && ambiente.iter().any(|(k, _)| k == regra::VARIAVEL_DO_TOKEN));
    let settings = fs::read_to_string(&arquivo).unwrap();
    assert!(!settings.contains("PermissionRequest") && settings.contains("UserPromptSubmit") && settings.contains("StopFailure"));

    // Configurações → eventos desligados: nada de ganchos.
    let (status, _) = c.api("POST", "/ganchos/ajustes", Some(json!({"eventos": false}))).await;
    assert_eq!(status, 200);
    let (comando, ambiente) = preparar(&c.state, &terminal, &c.origem(), &cwd, Some("claude --resume x".into()));
    assert_eq!(comando.as_deref(), Some("claude --resume x"));
    assert!(ambiente.is_empty());
    assert_eq!(resumo(&terminal)["estado"], "desligados");
    assert!(RUNTIME.tokens.lock().unwrap().values().all(|t| t != &terminal), "o token velho morre");

    // `disableAllHooks` do usuário: respeitado (nunca sobrescrevemos com false).
    c.api("POST", "/ganchos/ajustes", Some(json!({"eventos": true}))).await;
    fs::write(c.conta.join("settings.json"), r#"{"disableAllHooks": true}"#).unwrap();
    let (comando, _) = preparar(&c.state, &terminal, &c.origem(), &cwd, Some("claude".into()));
    assert_eq!(comando.as_deref(), Some("claude"));
    assert_eq!(resumo(&terminal)["motivo"], "usuario");
    fs::remove_file(c.conta.join("settings.json")).unwrap();

    // Outra plataforma ou sessão sem conversa (consulta de uso oculta): nada.
    let codex = SessionOrigin { provider: Some("codex".into()), ..c.origem() };
    assert_eq!(preparar(&c.state, &terminal, &codex, &cwd, Some("codex".into())).0.as_deref(), Some("codex"));
    let oculta = SessionOrigin { conversation_id: None, ..c.origem() };
    assert_eq!(preparar(&c.state, &terminal, &oculta, &cwd, Some("claude".into())).0.as_deref(), Some("claude"));

    // Prazo inválido recusado; ajustes listam a conta Claude.
    let (status, _) = c.api("POST", "/ganchos/ajustes", Some(json!({"espera_s": 7}))).await;
    assert_eq!(status, 400);
    let (status, _) = c.api("POST", "/ganchos/ajustes", Some(json!({"statusline": {"conta": "nao-existe", "ligado": true}}))).await;
    assert_eq!(status, 404);
    esquecer(&terminal);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn statusline_vira_uso_da_conta_e_o_claude_json_nao_e_aberto() {
    let c = cena(PORTA_FALSA + 4);
    atomic_write_json(&c.dir.path().join("profiles.json"), &json!({"profiles": [{"id": "claude-p", "provider": "claude", "name": "Trabalho",
        "config_dir": c.conta.to_string_lossy(), "builtin": false, "created_at_ms": 0}]})).unwrap();
    let (terminal, _sessao) = c.abrir(&quieta(&c));
    let token = token_de(&terminal);
    let entrada = json!({"session_id": "s", "model": {"display_name": "Opus"}, "context_window": {"used_percentage": 34},
        "cost": {"total_cost_usd": 0.82}, "rate_limits": {"five_hour": {"used_percentage": 12, "resets_at": 4_000_000_000_i64},
        "seven_day": {"used_percentage": 40, "resets_at": 4_100_000_000_i64}}});
    assert_eq!(c.chamar(c.pedido("/statusline", None, entrada.clone())).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(c.chamar(c.pedido("/statusline", Some(&token), entrada)).await.0, StatusCode::OK);
    assert_eq!(resumo(&terminal)["contexto"], json!({"pct": 34.0, "custo_usd": 0.82, "modelo": "Opus"}));
    let uso = uso_da_statusline(c.dir.path(), "claude-p").unwrap();
    assert_eq!((uso.source.as_str(), uso.primary.as_ref().unwrap().used_percent), ("claude_statusline", 12.0));
    let gravado = fs::read_to_string(c.dir.path().join(ARQUIVO_DE_USO)).unwrap();
    assert!(!gravado.contains("Opus") && !gravado.contains("0.82"), "só os números de uso vão para o disco");

    // Fork (harness, C5): o `.claude.json` não é mais aberto. Aqui ele é um FIFO: qualquer leitura
    // travaria no `open` (ninguém escreve do outro lado) e o prazo abaixo estouraria.
    let fifo = c.conta.join(".claude.json");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let (status, contas) = tokio::time::timeout(std::time::Duration::from_secs(10), c.api("GET", "/contas", None)).await
        .expect("GET /contas abriu o .claude.json");
    assert_eq!(status, 200);
    assert_eq!(contas[0]["usage"]["source"], "claude_statusline", "{contas}");
    let estado = c.state.clone();
    let consultas = tokio::time::timeout(std::time::Duration::from_secs(10), tokio::task::spawn_blocking(move || {
        [false, true].map(|refresh| match interaction::account_usage(&estado, "claude-p", refresh).unwrap() {
            EngineResponse::AccountUsage { usage } => usage, _ => panic!() })
    })).await.expect("account_usage abriu o .claude.json").unwrap();
    assert!(consultas.iter().all(|u| u.source == "claude_statusline"));
    let (status, _) = tokio::time::timeout(std::time::Duration::from_secs(10), c.api("POST", "/contas/claude-p/uso", None)).await
        .expect("atualizar o uso abriu o .claude.json");
    assert_eq!(status, 200);
    let sessoes = c.state.sessions.lock().unwrap().len();
    assert_eq!(sessoes, 1, "atualizar o uso não abre Claude escondido (só a sessão do teste)");
    // Sem statusline gravada: sem número, com o motivo, e ainda sem abrir o arquivo.
    fs::remove_file(c.dir.path().join(ARQUIVO_DE_USO)).unwrap();
    let estado = c.state.clone();
    let usage = tokio::time::timeout(std::time::Duration::from_secs(10), tokio::task::spawn_blocking(move ||
        match interaction::account_usage(&estado, "claude-p", true).unwrap() { EngineResponse::AccountUsage { usage } => usage, _ => panic!() }))
        .await.expect("sem statusline abriu o .claude.json").unwrap();
    assert!(usage.primary.is_none() && usage.reason.as_deref().is_some_and(|m| m.contains("primeira resposta")), "{usage:?}");
    fs::remove_file(&fifo).unwrap();
    c.fechar(&terminal);
}

/// Fork (fix-orquestracao, achado 1): o risco sai do comando inteiro, não do resumo cortado em 2.000
/// caracteres — um `rm -rf` depois do corte continuava aprovável pelo orquestrador. O corte aparece (…).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn risco_do_pedido_olha_o_comando_inteiro_mesmo_cortado_na_tela() {
    let c = cena(PORTA_FALSA + 5);
    let (terminal, _sessao) = c.abrir(&quieta(&c));
    let token = token_de(&terminal);
    let comando = format!("echo {} && rm -rf ~/projeto", "a".repeat(2_100));
    let corpo = json!({"hook_event_name": "PermissionRequest", "session_id": "s", "tool_name": "Bash", "tool_input": {"command": comando}});
    let (pedido_http, app) = (c.pedido("/ganchos/permissao", Some(&token), corpo), router(c.state.clone()));
    let espera = tokio::spawn(async move { app.oneshot(pedido_http).await.unwrap().status() });
    let visto = pedido_pendente(&terminal).await;
    assert!(visto.resumo.chars().count() <= 2_000 && visto.resumo.ends_with('…'), "a tela mostra que cortou");
    assert!(!visto.resumo.contains("rm -rf"), "o trecho perigoso ficou depois do corte");
    assert_eq!(visto.arriscado, Some("apaga arquivos em massa"));
    responder(&terminal, Some(&visto.id), Decisao::Negar(None)).unwrap();
    assert_eq!(espera.await.unwrap(), StatusCode::OK);
    c.fechar(&terminal);
}

/// Fork (fix-orquestracao, achado 5): depois de `/clear` ou `/branch` a CLI roda outra sessão. Além do
/// índice, a identidade da sessão (meta) e o comando de reinício passam a apontar para ela: reiniciar
/// retoma a nova e bifurcar copia a nova, não a anterior.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn troca_de_sessao_da_cli_atualiza_meta_e_comando_de_reinicio() {
    let c = cena(PORTA_FALSA + 6);
    let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
    atomic_write_json(&c.dir.path().join("conversations.json"), &json!({"version": 1, "conversations": [{
        "id": "conv", "project_id": "p", "cwd": c.projeto.to_string_lossy(), "title": "conv", "created_at_ms": 1,
        "segments": [{"provider": "claude", "profile_id": "claude-p", "external_session_id": "sess-1", "transcript_path": null,
            "terminal_session_id": terminal, "started_at_ms": 1, "ended_at_ms": null}]}]})).unwrap();
    let comando = format!("{} --model opus --session-id sess-1", quieta(&c));
    spawn_terminal_inner(&c.state, terminal.clone(), "p".into(), "Claude".into(), c.projeto.to_string_lossy().into_owned(),
        Some("/bin/sh".into()), Some(comando.clone()), 30, 100, now_ms(),
        SessionOrigin { external_session_id: Some("sess-1".into()), ..c.origem() }).unwrap();
    let sessao = viva(&c.state, &terminal).unwrap();
    let token = token_de(&terminal);
    let evento = |sid: &str| json!({"hook_event_name": "UserPromptSubmit", "session_id": sid,
        "transcript_path": c.conta.join(format!("{sid}.jsonl")).to_string_lossy()});

    // A mesma sessão com que a CLI abriu: nada muda.
    c.chamar(c.pedido("/ganchos", Some(&token), evento("sess-1"))).await;
    assert_eq!(sessao.meta.lock().unwrap().initial_command.as_deref(), Some(comando.as_str()));

    // `/clear`: sessão nova.
    c.chamar(c.pedido("/ganchos", Some(&token), evento("sess-2"))).await;
    let meta = sessao.meta.lock().unwrap().clone();
    assert_eq!(meta.external_session_id.as_deref(), Some("sess-2"));
    let reaberto = meta.initial_command.clone().unwrap();
    assert!(reaberto.contains("--session-id sess-2") && !reaberto.contains("sess-1") && reaberto.contains("--model opus"), "{reaberto}");
    let indice = omni_core::conversations::read_index(c.dir.path());
    assert_eq!(indice[0].segments[0].external_session_id.as_deref(), Some("sess-2"));
    assert!(fs::read_to_string(c.dir.path().join("sessions.json")).unwrap().contains("sess-2"), "a troca fica gravada");

    // Com a sessão nova já gravada pela CLI, reiniciar retoma ela.
    let transcript = omni_core::conversations::claude_transcript_path(&c.conta, &meta.cwd, "sess-2");
    fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    fs::write(&transcript, "{}\n").unwrap();
    let reinicio = comando_ao_reiniciar(&meta).unwrap();
    assert!(reinicio.contains("--resume sess-2") && !reinicio.contains("--session-id"), "{reinicio}");
    c.fechar(&terminal);
}

/// Fork (harness, A1): com os ajustes padrão (sem `ganchos.json`), a sessão ganha só os ganchos de
/// observação. Pasta ainda sem confiança (a CLI suspende os hooks) = nenhum evento: tudo segue pela tela,
/// exatamente como antes, e o diálogo de permissão é o da CLI.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn padrao_so_observa_e_pasta_sem_confianca_segue_pela_tela() {
    let c = cena(0);
    fs::remove_file(c.dir.path().join(regra::ARQUIVO_DE_AJUSTES)).unwrap();
    servidor(&c.state).await;
    let (terminal, sessao) = c.abrir(&sem_ganchos(&c));
    let arquivo = c.dir.path().join(regra::PASTA_DOS_ARQUIVOS).join(format!("{terminal}.json"));
    let settings = fs::read_to_string(&arquivo).expect("padrão injeta os eventos");
    assert!(settings.contains("UserPromptSubmit") && !settings.contains("PermissionRequest"));
    assert!(ate(15, || sessao.interaction.lock().unwrap().parser.screen().contents().contains('❯')).await);
    let antes = sessao.meta.lock().unwrap().attention_seq;
    write_terminal(&c.state, &terminal, "oi\r").unwrap();
    assert!(ate(5, || sessao.interaction.lock().unwrap().turn_active).await, "a tela viu o turno");
    assert_eq!(em_turno(&terminal), None, "sem evento, a tela manda");
    assert!(ate(15, || { settle(&sessao); sessao.meta.lock().unwrap().attention_seq > antes }).await, "fim do turno pela tela");
    assert_eq!(sessao.meta.lock().unwrap().state, SessionState::Answered);
    assert!(pedido(&terminal).is_none());
    c.fechar(&terminal);
}
