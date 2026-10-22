//! Fork: testes do orquestrador. Sem rede externa: o servidor MCP roda pelo `oneshot` do axum e os
//! subagentes são uma **CLI falsa** (`sh`) que imita o campo vazio do Claude Code.

use super::*;
use axum::body::{to_bytes, Body};
use tower::ServiceExt;
// Fork: cobertura do acesso do Jarvis, com as mesmas fixtures e CLIs falsas.
#[path = "orquestracao_jarvis_testes.rs"]
mod jarvis_testes;

const PORTA: u16 = 47_999;

fn fixture() -> (tempfile::TempDir, Arc<EngineState>, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
        token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    state.mobile.orquestracao.porta.store(PORTA, Ordering::Relaxed);
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let conversa = |id: &str, provider: &str| json!({"id": id, "project_id": "p", "cwd": projeto.to_string_lossy(), "title": id,
        "created_at_ms": 1, "segments": [{"provider": provider, "profile_id": null, "external_session_id": null,
        "transcript_path": null, "terminal_session_id": null, "started_at_ms": 1, "ended_at_ms": null}]});
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version": 1, "conversations": [
        conversa("orq-a", "claude"), conversa("orq-b", "codex"), conversa("agy", "antigravity"),
    ]})).unwrap();
    (dir, state, projeto)
}

/// `claude` de mentira: liga a colagem entre colchetes, desenha `❯ ` e grava cada linha recebida em
/// `recebido.txt`. "trabalhe" deixa o turno aberto ("esc to interrupt") por 30 s.
fn cli_falsa(dir: &Path) -> String {
    let pasta = dir.join("bin");
    std::fs::create_dir_all(&pasta).unwrap();
    let script = pasta.join("claude");
    std::fs::write(&script, r#"#!/bin/sh
LOG="$(dirname "$0")/recebido.txt"
printf '\033[?2004h'
while :; do
  printf '\r\n\342\235\257 '
  IFS= read -r linha || exit 0
  # O "\r\n" que digita o comando deixa um "\n" para trás: a CLI de verdade (modo raw) ignora.
  [ -z "$linha" ] && continue
  # Com `engolir`, a primeira mensagem some sem turno (o Codex 0.163 real fez isso subindo).
  if [ -f "$(dirname "$0")/engolir" ]; then rm "$(dirname "$0")/engolir"; while IFS= read -r -t 1 mais; do :; done; continue; fi
  printf '%s\n' "$linha" >> "$LOG"
  # Mensagem de várias linhas (o briefing) chega numa colagem só: o resto entra junto, um turno só.
  while IFS= read -r -t 1 mais; do printf '%s\n' "$mais" >> "$LOG"; linha="$linha $mais"; done
  # PEDIR_PERMISSAO/APAGAR_BUILD: pede permissão como o Claude Code (o comando aparece no diálogo) e grava a resposta.
  cmd=""
  case "$linha" in *APAGAR_BUILD*) cmd="rm -rf build" ;; *PEDIR_PERMISSAO*) cmd="ls src" ;; esac
  if [ -n "$cmd" ]; then
    printf '\r\n Bash command\r\n   %s\r\n\r\n Do you want to proceed?\r\n \342\235\257 1. Yes\r\n   2. Yes, and don'"'"'t ask again\r\n   3. No, and tell Claude what to do differently (esc)\r\n' "$cmd"
    IFS= read -r resp
    printf 'resposta %s\n' "$resp" >> "$LOG"
    printf '\033[2J\033[H ok\r\n'
    continue
  fi
  case "$linha" in
    *trabalhe*) printf '\r\n* Pensando (esc to interrupt)'; sleep 30 ;;
    *) printf '\r\n esc to interrupt'; sleep 1; printf '\r\033[2K ok\r\n' ;;
  esac
done
"#).unwrap();
    format!("/bin/sh {}", script.to_string_lossy())
}

fn publicar(state: &EngineState, projeto: &Path, comando: &str) {
    publish(state, vec![PublishedProject { id: "p".into(), name: "projeto".into(), path: projeto.to_string_lossy().into_owned() }],
        vec![PublishedAgent { id: "claude".into(), label: "Claude".into(), command: comando.into(), resume: None },
             PublishedAgent { id: "codex".into(), label: "Codex".into(), command: "codex".into(), resume: None }], None).unwrap();
}

/// Liga a orquestradora no registro e devolve o token que a PTY dela receberia.
fn ligar(state: &EngineState, conversa: &str, provider: &str) -> String {
    mudar_registro(state, |r| r.definir_ativo(conversa, true)).unwrap();
    let (comando, env) = preparar(state, Some(conversa), Some(provider), Some(provider.to_owned()));
    assert!(comando.unwrap().contains("127.0.0.1:47999/mcp"));
    env.into_iter().find(|(k, _)| k == regra::VARIAVEL_DO_TOKEN).map(|(_, v)| v).expect("token no ambiente")
}

fn pedido(token: Option<&str>, corpo: Value) -> axum::http::Request<Body> {
    let mut pedido = axum::http::Request::builder().method("POST").uri("/mcp")
        .header("host", format!("127.0.0.1:{PORTA}")).header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(token) = token { pedido = pedido.header("authorization", format!("Bearer {token}")); }
    pedido.body(Body::from(corpo.to_string())).unwrap()
}

async fn enviar_mcp(state: &Arc<EngineState>, pedido: axum::http::Request<Body>) -> (StatusCode, Value) {
    let resposta = router(state.clone()).oneshot(pedido).await.unwrap();
    let status = resposta.status();
    let bytes = to_bytes(resposta.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn ferramenta(state: &Arc<EngineState>, token: &str, nome: &str, argumentos: Value) -> (bool, String) {
    let (status, corpo) = enviar_mcp(state, pedido(Some(token), json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": nome, "arguments": argumentos}}))).await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    let resultado = &corpo["result"];
    (resultado["isError"] == true, resultado["content"][0]["text"].as_str().unwrap_or_default().to_owned())
}

#[tokio::test]
async fn token_errado_401_get_405_origem_estranha_403_e_handshake() {
    let (_dir, state, _) = fixture();
    let token = ligar(&state, "orq-a", "claude");
    let inicio = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}}});

    assert_eq!(enviar_mcp(&state, pedido(None, inicio.clone())).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(enviar_mcp(&state, pedido(Some("errado"), inicio.clone())).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(enviar_mcp(&state, pedido(Some(&state.token.clone()), inicio.clone())).await.0, StatusCode::UNAUTHORIZED, "engine.token não vale aqui");

    let get = axum::http::Request::builder().method("GET").uri("/mcp").header("host", format!("127.0.0.1:{PORTA}")).body(Body::empty()).unwrap();
    assert_eq!(router(state.clone()).oneshot(get).await.unwrap().status(), StatusCode::METHOD_NOT_ALLOWED);

    let mut de_fora = pedido(Some(&token), inicio.clone());
    de_fora.headers_mut().insert("origin", "https://site-malicioso.example".parse().unwrap());
    assert_eq!(enviar_mcp(&state, de_fora).await.0, StatusCode::FORBIDDEN);
    let mut outro_host = pedido(Some(&token), inicio.clone());
    outro_host.headers_mut().insert("host", "evil.example:47999".parse().unwrap());
    assert_eq!(enviar_mcp(&state, outro_host).await.0, StatusCode::FORBIDDEN);

    let (status, corpo) = enviar_mcp(&state, pedido(Some(&token), inicio)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(corpo["result"]["protocolVersion"], "2025-06-18");
    assert!(corpo["result"]["capabilities"]["tools"].is_object());
    let (status, _) = enviar_mcp(&state, pedido(Some(&token), json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (_, lista) = enviar_mcp(&state, pedido(Some(&token), json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))).await;
    let nomes: Vec<&str> = lista["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(nomes, ["listar_plataformas_e_contas", "criar_agente", "conferir_arquivos", "mandar_prompt", "ler_conversa", "esperar", "listar_agentes", "responder_pedido", "parar_agente",
        // Fork (Copiloto): ferramentas de tarefas.
        "listar_tarefas", "criar_tarefa", "editar_tarefa", "concluir_tarefa", "apagar_tarefa"]);
    let (_, desconhecido) = enviar_mcp(&state, pedido(Some(&token), json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}))).await;
    assert_eq!(desconhecido["error"]["code"], -32601);
}

#[tokio::test]
async fn token_so_enxerga_os_proprios_subagentes_e_morre_ao_desligar() {
    let (_dir, state, _) = fixture();
    let token_a = ligar(&state, "orq-a", "claude");
    let token_b = ligar(&state, "orq-b", "codex");
    assert_ne!(token_a, token_b);
    let agente = |id: &str| regra::Agente { conversation_key: id.into(), plataforma: "claude".into(), tarefa: "t".into(), criado_em: 1,
        ..regra::Agente::default() };
    mudar_registro(&state, |r| { r.adicionar("orq-a", agente("sub-a")); r.adicionar("orq-b", agente("sub-b")); }).unwrap();

    for nome in ["ler_conversa", "mandar_prompt", "esperar", "parar_agente"] {
        let (erro, texto) = ferramenta(&state, &token_a, nome, json!({"agente": "sub-b", "texto": "oi", "segundos": 1})).await;
        assert!(erro && texto.contains("não é deste orquestrador"), "{nome}: {texto}");
    }
    let (erro, texto) = ferramenta(&state, &token_a, "listar_agentes", json!({})).await;
    assert!(!erro);
    assert!(texto.contains("sub-a") && !texto.contains("sub-b"), "{texto}");

    // Reabrir a orquestradora troca o token: o antigo para de valer.
    let novo_a = ligar(&state, "orq-a", "claude");
    let lista = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    assert_eq!(enviar_mcp(&state, pedido(Some(&token_a), lista.clone())).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(enviar_mcp(&state, pedido(Some(&novo_a), lista.clone())).await.0, StatusCode::OK);

    // "Deixar de orquestrar": flags saem do comando, a PTY não recebe token e o atual é recusado.
    mudar_registro(&state, |r| r.definir_ativo("orq-a", false)).unwrap();
    assert_eq!(enviar_mcp(&state, pedido(Some(&novo_a), lista.clone())).await.0, StatusCode::UNAUTHORIZED);
    let comando = regra::ajustar_comando("claude --resume x", "claude", PORTA, true);
    let (digitado, env) = preparar(&state, Some("orq-a"), Some("claude"), Some(comando));
    assert_eq!(digitado.as_deref(), Some("claude --resume x"));
    assert!(env.is_empty());
    assert_eq!(enviar_mcp(&state, pedido(Some(&token_b), lista)).await.0, StatusCode::OK, "o outro orquestrador segue");
}

#[tokio::test]
async fn so_claude_e_codex_orquestram_e_subagente_nao_ganha_token() {
    let (_dir, state, _) = fixture();
    let ativar = |id: &str| { let state = state.clone(); let id = id.to_owned();
        async move { local(state, "POST", &format!("/orquestracao/{id}/ativar"), Some(r#"{"ativo":true}"#.into()), None, vec![]).await } };
    let EngineResponse::Api { status, .. } = ativar("agy").await else { panic!() };
    assert_eq!(status, 409, "antigravity: ainda não suportado como orquestrador");
    // Sem sessão aberta: só marca; a flag entra na próxima abertura.
    let EngineResponse::Api { status, body, .. } = ativar("orq-b").await else { panic!() };
    assert_eq!(status, 200, "{body}");
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["reaberta"], false);
    assert!(ler_registro(&state).ativo("orq-b"));

    // Conversa que não é orquestradora (um subagente) nunca recebe token nem flags.
    let (digitado, env) = preparar(&state, Some("sub"), Some("claude"), Some("claude --session-id s".into()));
    assert_eq!(digitado.as_deref(), Some("claude --session-id s"));
    assert!(env.is_empty());
    mudar_registro(&state, |r| r.adicionar("orq-b", regra::Agente { conversation_key: "orq-a".into(), plataforma: "claude".into(),
        tarefa: "t".into(), criado_em: 1, ..regra::Agente::default() })).unwrap();
    let EngineResponse::Api { status, .. } = ativar("orq-a").await else { panic!() };
    assert_eq!(status, 409, "sem recursão");

    let EngineResponse::Api { status, body, .. } = local(state.clone(), "GET", "/orquestracao/orq-b", None, None, vec![]).await else { panic!() };
    assert_eq!(status, 200);
    let fluxo: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(fluxo["ativo"], true);
    assert_eq!(fluxo["agentes"][0]["id"], "orq-a");
    assert_eq!(fluxo["agentes"][0]["estado"], "parado");
}

/// Ponta a ponta com a CLI falsa: criar (a tarefa chega sozinha), limite, mandar mais, `esperar`
/// estourando com o agente trabalhando, parar e o token e a tarefa fora do `sessions.json`.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_falsa_cria_recebe_tarefa_respeita_limite_espera_e_para() {
    let (dir, state, projeto) = fixture();
    // A fila de ações do chat (quem digita na PTY) roda na tarefa do `mobile::start`.
    start(state.clone());
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let token = ligar(&state, "orq-a", "claude");
    let EngineResponse::Api { status, .. } = local(state.clone(), "POST", "/orquestracao/limite", Some(r#"{"limite":1}"#.into()), None, vec![]).await else { panic!() };
    assert_eq!(status, 200);

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "cursor", "tarefa": "x"})).await;
    assert!(erro && texto.contains("Disponíveis: claude, codex"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "x", "modelo": "a;b"})).await;
    assert!(erro, "{texto}");

    std::fs::write(dir.path().join("bin").join("engolir"), "").unwrap();
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "modelo": "haiku", "tarefa": "diga so ok",
        "contexto": "projeto de teste", "ler": ["README.md"], "editar": ["src/a.ts"], "criar": ["src/novo/"], "criterios": ["responder ok"]})).await;
    assert!(!erro, "{texto}");
    let agente = serde_json::from_str::<Value>(&texto).unwrap()["agente"].as_str().unwrap().to_owned();
    let recebido = dir.path().join("bin").join("recebido.txt");
    let chegou = |trecho: &str| std::fs::read_to_string(&recebido).unwrap_or_default().contains(trecho);
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !chegou("diga so ok") && std::time::Instant::now() < limite { tokio::time::sleep(std::time::Duration::from_millis(200)).await; }
    if !chegou("diga so ok") {
        let (_, Some(sessao)) = current(&state, &agente).unwrap() else { panic!("sem sessão") };
        let tela = sessao.interaction.lock().unwrap().parser.screen().contents();
        let cap = interaction::capabilities(&sessao);
        panic!("a tarefa não chegou na CLI falsa; tela: {tela:?}; motivo: {:?}; pendente: {:?}", cap.reason,
            state.mobile.orquestracao.pendentes.lock().unwrap().get(&agente));
    }

    let recebida = std::fs::read_to_string(&recebido).unwrap();
    assert_eq!(recebida.matches("diga so ok").count(), 1, "reenviada só porque a primeira sumiu");
    // O briefing montado pelo engine chegou inteiro: tarefa, contexto, arquivos, critérios e regras.
    for trecho in ["## Você pode editar (só estes)", "- src/a.ts", "- src/novo/", "## Leia antes de começar", "## Pronto quando", "## Regras"] {
        assert!(recebida.contains(trecho), "faltou {trecho:?} no briefing");
    }
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "outro", "somente_leitura": true})).await;
    assert!(erro && texto.contains("Limite de 1"), "{texto}");

    // Harness: com vaga, o segundo não pode pegar arquivo do primeiro, nem nascer sem dizer o que mexe.
    let EngineResponse::Api { status, .. } = local(state.clone(), "POST", "/orquestracao/limite", Some(r#"{"limite":3}"#.into()), None, vec![]).await else { panic!() };
    assert_eq!(status, 200);
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "outro", "editar": ["src/novo/x.ts"]})).await;
    assert!(erro && texto.contains("Arquivos de outro subagente vivo") && texto.contains("src/novo/x.ts"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "outro"})).await;
    assert!(erro && texto.contains("editar"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "outro", "editar": ["../fora.ts"]})).await;
    assert!(erro && texto.contains(".."), "{texto}");

    // conferir_arquivos: git status cruzado com a posse.
    let git = |args: &[&str]| assert!(std::process::Command::new("git").args(args).current_dir(&projeto).output().unwrap().status.success());
    git(&["init", "-q"]);
    std::fs::create_dir_all(projeto.join("src/novo")).unwrap();
    std::fs::write(projeto.join("src/a.ts"), "x").unwrap();
    std::fs::write(projeto.join("src/novo/b.ts"), "x").unwrap();
    std::fs::write(projeto.join("package.json"), "{}").unwrap();
    let (erro, texto) = ferramenta(&state, &token, "conferir_arquivos", json!({})).await;
    assert!(!erro, "{texto}");
    let conferido: Value = serde_json::from_str(&texto).unwrap();
    assert_eq!(conferido["sem_dono"], json!(["package.json"]));
    let do_agente = conferido["por_agente"].as_object().unwrap().values().next().unwrap().clone();
    assert_eq!(do_agente, json!(["src/a.ts", "src/novo/b.ts"]));

    let (erro, texto) = ferramenta(&state, &token, "esperar", json!({"agente": agente, "segundos": 20})).await;
    assert!(!erro && texto.contains("\"pronto\""), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "mandar_prompt", json!({"agente": agente, "texto": "agora trabalhe"})).await;
    assert!(!erro, "{texto}");
    assert!(chegou("agora trabalhe"));
    let (erro, texto) = ferramenta(&state, &token, "esperar", json!({"agente": agente, "segundos": 2})).await;
    assert!(!erro && texto.starts_with("ainda trabalhando"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "ler_conversa", json!({"agente": agente})).await;
    assert!(!erro && texto.contains("\"trabalhando\""), "{texto}");

    let EngineResponse::Api { status, body, .. } = local(state.clone(), "GET", "/orquestracao/orq-a", None, None, vec![]).await else { panic!() };
    assert_eq!(status, 200);
    let fluxo: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(fluxo["agentes"][0]["modelo"], "haiku");
    assert_eq!(fluxo["agentes"][0]["tarefa"], "diga so ok");
    assert_eq!(fluxo["agentes"][0]["estado"], "trabalhando");

    let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": agente})).await;
    assert!(!erro, "{texto}");
    let (_, texto) = ferramenta(&state, &token, "esperar", json!({"agente": agente, "segundos": 5})).await;
    assert!(texto.contains("\"parado\""), "{texto}");

    // O token nunca vai para o disco: nem no sessions.json, nem no registro, nem na linha digitada.
    let sessoes = std::fs::read_to_string(dir.path().join("sessions.json")).unwrap();
    let registro_bruto = std::fs::read_to_string(dir.path().join(regra::ARQUIVO)).unwrap();
    assert!(!sessoes.contains(&token) && !registro_bruto.contains(&token));
    assert!(!sessoes.contains(regra::VARIAVEL_DO_TOKEN), "o ambiente extra não entra no meta");
}

/// Põe uma ação na fila do chat com a versão da tela atual (o que o usuário faria clicando).
#[cfg(unix)]
fn acao(state: &Arc<EngineState>, conversa: &str, kind: ActionKind) -> bool {
    let Ok((_, Some(sessao))) = current(state, conversa) else { return false };
    let mut headers = HeaderMap::new();
    headers.insert("idempotency-key", format!("teste-{}", uuid::Uuid::new_v4()).parse().unwrap());
    headers.insert("if-match", interaction::capabilities(&sessao).revision.parse().unwrap());
    enqueue(state, conversa, &headers, kind).is_ok()
}

/// **CLI real, uma vez, à mão** (perfil Padrão: `~/.claude` e `~/.codex`, sem `profiles.json`):
/// orquestrador Claude Haiku numa pasta de rascunho pede 2 subagentes (Claude Haiku + Codex
/// `gpt-6-luna`, o menor da lista da conta) com "diga só 'ok'". Este teste faz o papel do usuário:
/// aprova o pedido de permissão da 1ª chamada de cada ferramenta (nada pré-autorizado) e responde o
/// "confiar na pasta?" do Codex com a opção que pede aprovação. Para tudo no fim.
///
/// `cargo test -p omni-engine cli_real_orquestrador -- --ignored --nocapture`
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "CLI real: gasta assento; rodar uma vez, à mão"]
async fn cli_real_orquestrador() {
    let dados = tempfile::tempdir().unwrap();
    let rascunho = tempfile::tempdir().unwrap();
    // Caminho real (/private/var/…): é o que a CLI grava no transcript.
    let pasta = std::fs::canonicalize(rascunho.path()).unwrap().to_string_lossy().into_owned();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dados.path()),
        token: "test-only".into(), state_file: dados.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    start(state.clone());
    iniciar(state.clone(), 47_590);
    // Perfil Padrão como o app registra (builtin = pasta nativa, sem variável de conta).
    let casa = PathBuf::from(std::env::var("HOME").unwrap());
    atomic_write_json(&dados.path().join("profiles.json"), &json!({"profiles": [
        {"id": "claude-padrao", "provider": "claude", "name": "Padrão", "config_dir": casa.join(".claude").to_string_lossy(), "builtin": true, "created_at_ms": 0},
        {"id": "codex-padrao", "provider": "codex", "name": "Padrão", "config_dir": casa.join(".codex").to_string_lossy(), "builtin": true, "created_at_ms": 0},
    ]})).unwrap();
    publish(&state, vec![PublishedProject { id: "p".into(), name: "rascunho".into(), path: pasta.clone() }],
        vec![PublishedAgent { id: "claude".into(), label: "Claude".into(), command: "claude".into(), resume: None },
             PublishedAgent { id: "codex".into(), label: "Codex".into(), command: "codex".into(), resume: None }], None).unwrap();
    let sid = uuid::Uuid::new_v4().to_string();
    let transcript = omni_core::conversations::claude_transcript_path(&PathBuf::from(std::env::var("HOME").unwrap()).join(".claude"), &pasta, &sid);
    let segmento = |terminal: Option<String>| omni_core::conversations::Segment { provider: "claude".into(), profile_id: Some("claude-padrao".into()),
        external_session_id: Some(sid.clone()), transcript_path: Some(transcript.to_string_lossy().into_owned()),
        terminal_session_id: terminal, started_at_ms: now_ms(), ended_at_ms: None };
    omni_core::conversations::write_index(dados.path(), &[omni_core::conversations::Conversation { id: "orq".into(), project_id: "p".into(),
        cwd: pasta.clone(), title: "Orquestrador (teste)".into(), title_source: omni_core::conversations::TitleSource::Manual,
        created_at_ms: now_ms(), segments: vec![segmento(None)] }]).unwrap();
    mudar_registro(&state, |r| r.definir_ativo("orq", true)).unwrap();
    let sessao = spawn_terminal_inner(&state, state.next_id(), "p".into(), "Orquestrador".into(), pasta.clone(), None,
        Some(format!("claude --session-id {sid} --model haiku")), 40, 140, now_ms(),
        SessionOrigin { headless: true, env: vec![], provider: Some("claude".into()), profile_id: Some("claude-padrao".into()),
            conversation_id: Some("orq".into()), external_session_id: Some(sid.clone()) }).unwrap();
    omni_core::conversations::write_index(dados.path(), &[omni_core::conversations::Conversation { id: "orq".into(), project_id: "p".into(),
        cwd: pasta.clone(), title: "Orquestrador (teste)".into(), title_source: omni_core::conversations::TitleSource::Manual,
        created_at_ms: now_ms(), segments: vec![segmento(Some(sessao.id.clone()))] }]).unwrap();
    passar_pela_confianca(&state, &sessao.id);

    let espera = |segundos: u64| tokio::time::sleep(std::time::Duration::from_secs(segundos));
    let mut pronto = false;
    for _ in 0..90 {
        espera(1).await;
        if estado_de(&state, "orq").0 == regra::Estado::Pronto { pronto = true; break; }
    }
    assert!(pronto, "o orquestrador não ficou pronto");
    println!("orquestrador pronto; mandando o pedido");
    assert!(acao(&state, "orq", ActionKind::Prompt("Use as ferramentas medza: crie 2 subagentes com criar_agente (1: plataforma claude, modelo haiku; 2: plataforma codex, modelo gpt-6-luna, esforco low), os dois com a tarefa \"diga só 'ok'\" e somente_leitura true. Depois chame esperar em cada um e responda numa linha o que cada um disse.".into())));

    let inicio = std::time::Instant::now();
    let mut pronto_seguido = 0;
    let mut visto_trabalhando = false;
    while inicio.elapsed() < std::time::Duration::from_secs(480) {
        espera(1).await;
        let registro = ler_registro(&state);
        let agentes: Vec<String> = registro.orquestrador("orq").map(|o| o.agentes.iter().map(|a| a.conversation_key.clone()).collect()).unwrap_or_default();
        for conversa in std::iter::once("orq".to_owned()).chain(agentes.iter().cloned()) {
            let (estado, _, cap) = estado_de(&state, &conversa);
            let Some(cap) = cap else { continue };
            if cap.approve {
                println!("[{:>3}s] {conversa}: aprovando pedido: {}", inicio.elapsed().as_secs(),
                    cap.approval_text.as_deref().unwrap_or("").lines().next().unwrap_or(""));
                acao(&state, &conversa, ActionKind::Approval(true));
                espera(2).await;
            } else if let Some(menu) = &cap.escolha {
                let opcao = menu.opcoes.iter().find(|o| o.rotulo.to_lowercase().contains("ask")).or(menu.opcoes.first());
                println!("[{:>3}s] {conversa}: menu \"{}\" → {:?}", inicio.elapsed().as_secs(), menu.pergunta, opcao.map(|o| &o.rotulo));
                if let Some(opcao) = opcao { acao(&state, &conversa, ActionKind::Escolha(Resposta::Opcao(opcao.numero))); }
                espera(2).await;
            }
            if conversa == "orq" {
                visto_trabalhando |= estado == regra::Estado::Trabalhando;
                pronto_seguido = if estado == regra::Estado::Pronto && visto_trabalhando && agentes.len() >= 2 { pronto_seguido + 1 } else { 0 };
            }
        }
        if pronto_seguido >= 5 { break; }
    }
    let resumo = ferramentas::listar_agentes(&state, "orq");
    println!("subagentes: {}", serde_json::to_string_pretty(&resumo).unwrap());
    println!("orquestrador disse: {:?}", ultima_do_agente(&state, "orq").map(|t| regra::resumir(&t, 400)));
    // Encerra tudo, terminado ou não.
    let ids: Vec<String> = state.sessions.lock().unwrap().keys().cloned().collect();
    for id in ids { let _ = stop_session(&state, &id); }
    assert!(pronto_seguido >= 5, "não terminou em 8 min");
    assert_eq!(resumo["agentes"].as_array().map(|a| a.len()), Some(2));
}

/// O orquestrador responde o pedido de permissão do subagente; o arriscado fica para o usuário.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn orquestrador_responde_permissao_do_subagente_e_arriscado_fica_com_o_usuario() {
    let (dir, state, projeto) = fixture();
    start(state.clone());
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let token = ligar(&state, "orq-a", "claude");
    let recebido = dir.path().join("bin").join("recebido.txt");
    let chegou = |trecho: &str| std::fs::read_to_string(&recebido).unwrap_or_default().contains(trecho);

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "PEDIR_PERMISSAO para listar", "somente_leitura": true})).await;
    assert!(!erro, "{texto}");
    let agente = serde_json::from_str::<Value>(&texto).unwrap()["agente"].as_str().unwrap().to_owned();
    let (_, texto) = ferramenta(&state, &token, "esperar", json!({"agente": agente, "segundos": 40})).await;
    let espera: Value = serde_json::from_str(&texto).unwrap_or(Value::Null);
    assert_eq!(espera["estado"], "esperando você", "{texto}");
    assert_eq!(espera["pedido"]["tipo"], "permissao");
    assert!(espera["pedido"]["texto"].as_str().unwrap().contains("ls src"));
    assert!(espera["pedido"]["arriscado"].is_null());

    let (erro, texto) = ferramenta(&state, &token, "responder_pedido", json!({"agente": agente, "opcao": 2})).await;
    assert!(erro && texto.contains("permitir"), "permissão não se responde com opção: {texto}");
    let (erro, texto) = ferramenta(&state, &token, "responder_pedido", json!({"agente": agente, "permitir": true})).await;
    assert!(!erro, "{texto}");
    assert!(chegou("resposta 1"), "a CLI recebeu o 'Yes' de uma vez só");
    let (_, texto) = ferramenta(&state, &token, "esperar", json!({"agente": agente, "segundos": 20})).await;
    assert!(texto.contains("\"pronto\""), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "responder_pedido", json!({"agente": agente, "permitir": true})).await;
    assert!(erro && texto.contains("Não há pedido"), "{texto}");

    // Arriscado: aprovar é recusado (fica para o usuário no fluxo); negar pode.
    let (erro, texto) = ferramenta(&state, &token, "mandar_prompt", json!({"agente": agente, "texto": "agora APAGAR_BUILD"})).await;
    assert!(!erro, "{texto}");
    let (_, texto) = ferramenta(&state, &token, "esperar", json!({"agente": agente, "segundos": 20})).await;
    let espera: Value = serde_json::from_str(&texto).unwrap_or(Value::Null);
    assert_eq!(espera["pedido"]["arriscado"], "apaga arquivos em massa", "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "responder_pedido", json!({"agente": agente, "permitir": true})).await;
    assert!(erro && texto.contains("só o usuário aprova"), "{texto}");
    assert!(!chegou("resposta 3") && std::fs::read_to_string(&recebido).unwrap().matches("resposta").count() == 1);
    let (erro, texto) = ferramenta(&state, &token, "responder_pedido", json!({"agente": agente, "permitir": false})).await;
    assert!(!erro, "{texto}");
    assert!(chegou("resposta 3"), "negar manda a opção No");
    let _ = ferramenta(&state, &token, "parar_agente", json!({"agente": agente})).await;
}

/// Fork: `criar_agente` com `worktree` — cada subagente numa worktree isolada (branch `medza/<nome>`,
/// `.worktreeinclude` copiado), sem disputa de arquivo entre eles, e a pasta principal intacta.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn criar_agente_em_worktree_isolada() {
    let (dir, state, projeto) = fixture();
    start(state.clone());
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let token = ligar(&state, "orq-a", "claude");

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "x", "editar": ["src/a.ts"], "worktree": true})).await;
    assert!(erro && texto.contains("não é um repositório git"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "x", "editar": ["src/a.ts"], "worktree": 3})).await;
    assert!(erro && texto.contains("true/false"), "{texto}");

    let git = |args: &[&str]| assert!(std::process::Command::new("git").args(args).current_dir(&projeto).output().unwrap().status.success(), "{args:?}");
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "t@t"]);
    git(&["config", "user.name", "t"]);
    std::fs::write(projeto.join(".gitignore"), ".env\n").unwrap();
    std::fs::write(projeto.join(".worktreeinclude"), ".env\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "inicio"]);
    std::fs::write(projeto.join(".env"), "X=1\n").unwrap();

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "Tela de login nova",
        "editar": ["src/a.ts"], "worktree": true})).await;
    assert!(!erro, "{texto}");
    let criado: Value = serde_json::from_str(&texto).unwrap();
    let caminho = PathBuf::from(criado["worktree"].as_str().unwrap());
    assert!(caminho.ends_with(".medza-worktrees/projeto/tela-de-login-nova"), "{caminho:?}");
    assert_eq!(criado["branch"], "medza/tela-de-login-nova");
    assert_eq!(std::fs::read_to_string(caminho.join(".env")).unwrap(), "X=1\n");
    let agente = criado["agente"].as_str().unwrap().to_owned();
    let (_, Some(sessao)) = current(&state, &agente).unwrap() else { panic!("sem sessão") };
    assert_eq!(PathBuf::from(sessao.meta.lock().unwrap().cwd.clone()), caminho);

    // O mesmo arquivo noutro subagente isolado não é disputa: cada um tem a sua cópia.
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "y",
        "editar": ["src/a.ts"], "worktree": "Outra Parte"})).await;
    assert!(!erro, "{texto}");
    assert_eq!(serde_json::from_str::<Value>(&texto).unwrap()["branch"], "medza/outra-parte");

    // O briefing diz onde ele está e pede commit na branch dele.
    let recebido = dir.path().join("bin").join("recebido.txt");
    let chegou = |trecho: &str| std::fs::read_to_string(&recebido).unwrap_or_default().contains(trecho);
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !chegou("medza/tela-de-login-nova") && std::time::Instant::now() < limite { tokio::time::sleep(std::time::Duration::from_millis(200)).await; }
    assert!(chegou("## Sua pasta (worktree isolada)") && chegou("medza/tela-de-login-nova"));

    let (_, texto) = ferramenta(&state, &token, "listar_agentes", json!({})).await;
    assert!(texto.contains("medza/outra-parte"), "{texto}");
    std::fs::write(caminho.join("rascunho.txt"), "x").unwrap();
    let (erro, texto) = ferramenta(&state, &token, "conferir_arquivos", json!({})).await;
    assert!(!erro, "{texto}");
    let conferido: Value = serde_json::from_str(&texto).unwrap();
    assert_eq!(conferido["em_worktrees"].as_array().unwrap().len(), 2);
    assert!(conferido["em_worktrees"].as_array().unwrap().iter().any(|w| w["nao_commitadas"] == 1), "{texto}");
    // A pasta do orquestrador continua em main, sem nada novo.
    let status = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(&projeto).output().unwrap();
    assert!(String::from_utf8_lossy(&status.stdout).trim().is_empty());

    for id in [agente, serde_json::from_str::<Value>(&texto).unwrap()["em_worktrees"][1]["agente"].as_str().unwrap().to_owned()] {
        let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": id})).await;
        assert!(!erro, "{texto}");
    }
}

/// Fork (3B): `criar_agente` com `conta: "auto"` abre na conta que a Fila de contas escolhe e diz por
/// quê; `listar_plataformas_e_contas` mostra a escolha antes.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn criar_agente_com_conta_automatica_usa_a_fila() {
    let (dir, state, projeto) = fixture();
    start(state.clone());
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let perfil = |id: &str, nome: &str| json!({"id": id, "provider": "claude", "name": nome,
        "config_dir": dir.path().join("contas").join(id), "builtin": false, "created_at_ms": 0});
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles": [perfil("a", "Pessoal"), perfil("b", "Empresa")]})).unwrap();
    atomic_write_json(&dir.path().join("routing.json"), &json!({"modo": "fila", "etapas": ["b", "a"]})).unwrap();
    let token = ligar(&state, "orq-a", "claude");

    let (erro, texto) = ferramenta(&state, &token, "listar_plataformas_e_contas", json!({})).await;
    assert!(!erro, "{texto}");
    let catalogo: Value = serde_json::from_str(&texto).unwrap();
    assert_eq!(catalogo["plataformas"][0]["conta_automatica"]["nome"], "Empresa", "{texto}");

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "conta": "auto", "tarefa": "diga so ok",
        "somente_leitura": true})).await;
    assert!(!erro, "{texto}");
    let criado: Value = serde_json::from_str(&texto).unwrap();
    assert_eq!(criado["conta"], "Empresa");
    assert!(criado["por_que_esta_conta"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("primeira da fila")), "{texto}");
    let agente = criado["agente"].as_str().unwrap().to_owned();
    let (_, Some(sessao)) = current(&state, &agente).unwrap() else { panic!("sem sessão") };
    assert_eq!(sessao.meta.lock().unwrap().profile_id.as_deref(), Some("b"));

    // Fila sem conta apta (confidencial e nenhuma liberada): erro com a explicação, nada abre.
    atomic_write_json(&dir.path().join("routing.json"), &json!({"etapas": ["b", "a"],
        "projetos_confidenciais": [projeto.to_string_lossy()]})).unwrap();
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "conta": "auto", "tarefa": "x"})).await;
    assert!(erro && texto.contains("A Fila de contas não achou conta") && texto.contains("confidencial"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": agente})).await;
    assert!(!erro, "{texto}");
}

/// Fork (fix-orquestracao, achado 8): duas `criar_agente` com `conta: "auto"` ao mesmo tempo não caem
/// as duas na conta de teto 1: escolher a conta e ocupar a vaga ficam na mesma seção crítica.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn criacoes_simultaneas_com_conta_automatica_respeitam_o_teto() {
    let (dir, state, projeto) = fixture();
    start(state.clone());
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let perfil = |id: &str, nome: &str| json!({"id": id, "provider": "claude", "name": nome,
        "config_dir": dir.path().join("contas").join(id), "builtin": false, "created_at_ms": 0});
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles": [perfil("a", "Pessoal"), perfil("b", "Empresa")]})).unwrap();
    atomic_write_json(&dir.path().join("routing.json"), &json!({"modo": "fila", "etapas": ["b", "a"],
        "contas": {"b": {"max_concorrencia": 1}}})).unwrap();
    let token = ligar(&state, "orq-a", "claude");
    let criar = |tarefa: &'static str| ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "conta": "auto",
        "tarefa": tarefa, "somente_leitura": true}));
    let ((erro_1, texto_1), (erro_2, texto_2)) = tokio::join!(criar("primeira"), criar("segunda"));
    assert!(!erro_1 && !erro_2, "{texto_1}\n{texto_2}");
    let criados: Vec<Value> = [texto_1, texto_2].iter().map(|t| serde_json::from_str(t).unwrap()).collect();
    let mut contas: Vec<&str> = criados.iter().map(|c| c["conta"].as_str().unwrap()).collect();
    contas.sort();
    assert_eq!(contas, ["Empresa", "Pessoal"], "a segunda não pode passar do teto da Empresa");
    for criado in &criados {
        let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": criado["agente"]})).await;
        assert!(!erro, "{texto}");
    }
}

// Fork (Copiloto): a conversa do copiloto nasce orquestradora; as outras não mudam.
#[test]
fn copiloto_nasce_orquestrador_e_os_outros_projetos_nao() {
    let (_dir, state, _) = fixture();
    marcar_copiloto(&state, "p", Some("orq-a"), Some("claude"));
    assert!(!ler_registro(&state).ativo("orq-a"));
    marcar_copiloto(&state, PROJETO_DO_COPILOTO, Some("agy"), Some("antigravity"));
    assert!(!ler_registro(&state).ativo("agy"));
    marcar_copiloto(&state, PROJETO_DO_COPILOTO, Some("orq-b"), Some("codex"));
    assert!(ler_registro(&state).ativo("orq-b"));
    let (comando, ambiente) = preparar(&state, Some("orq-b"), Some("codex"), Some("codex".into()));
    assert!(comando.unwrap().contains("mcp"));
    assert_eq!(ambiente.len(), 1);
}
