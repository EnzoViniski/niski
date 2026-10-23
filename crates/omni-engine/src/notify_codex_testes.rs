//! Fork (harness, A2): Codex **falso** (`sh`) que lê o `-c notify=[…]` da linha, como o Codex leria, e
//! chama o programa no fim do turno; shim falso (`sh` + `curl`) faz o POST que o `omni-shim` faria.
//! Nenhuma CLI real.

use super::*;
use std::sync::{atomic::AtomicU64, Mutex};

struct Cena { dir: tempfile::TempDir, state: Arc<EngineState>, projeto: PathBuf, codex_home: PathBuf }

fn cena() -> Cena {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dir.path()),
        token: "engine-token-de-teste".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    let projeto = dir.path().join("projeto");
    let codex_home = dir.path().join("codex-home");
    fs::create_dir_all(&projeto).unwrap();
    fs::create_dir_all(&codex_home).unwrap();
    Cena { dir, state, projeto, codex_home }
}

impl Cena {
    fn origem(&self) -> SessionOrigin {
        SessionOrigin { provider: Some("codex".into()), profile_id: Some("codex-p".into()), conversation_id: Some("conv".into()),
            env: vec![("CODEX_HOME".into(), self.codex_home.to_string_lossy().into_owned())], ..SessionOrigin::default() }
    }

    /// Shim de mentira: o que o `omni-shim --medza-notify` faz (POST só com os ids).
    fn shim(&self) -> PathBuf {
        let shim = self.dir.path().join("shim-falso");
        fs::write(&shim, r#"#!/bin/sh
[ "$1" = "--medza-notify" ] || exit 2
curl -s -m 5 -H "Host: 127.0.0.1:$2" -H "Authorization: Bearer $MEDZA_GANCHO_TOKEN" -H 'Content-Type: application/json' \
  --data '{"type":"agent-turn-complete","thread-id":"th-falso","turn-id":"t-1"}' "http://127.0.0.1:$2/ganchos/codex" >/dev/null
[ -n "$MEDZA_NOTIFY_DO_USUARIO" ] && printf '%s' "$MEDZA_NOTIFY_DO_USUARIO" > "$(dirname "$0")/usuario.txt"
exit 0
"#).unwrap();
        shim
    }

    /// Codex de mentira: acha o `-c notify=[…]`, mostra o campo e, a cada linha, faz um "turno" de 1 s
    /// com o rodapé de trabalho e chama o programa do notify com o JSON do turno.
    fn codex(&self) -> String {
        let script = self.dir.path().join("codex");
        fs::write(&script, r#"#!/bin/sh
N=""
while [ $# -gt 0 ]; do case "$1" in -c) shift; case "$1" in notify=*) N="$1";; esac;; esac; shift; done
printf '%s' "$N" > "$(dirname "$0")/notify.txt"
V=${N#notify=[}; V=${V%]}
PROG=$(printf '%s' "$V" | sed 's/^"\([^"]*\)".*/\1/')
A1=$(printf '%s' "$V" | sed 's/^"[^"]*","\([^"]*\)".*/\1/')
A2=$(printf '%s' "$V" | sed 's/^"[^"]*","[^"]*","\([^"]*\)".*/\1/')
printf '\033[?2004h'
while :; do
  printf '\r\n OpenAI Codex\r\n\342\200\272 '
  IFS= read -r linha || exit 0
  [ -z "$linha" ] && continue
  printf '\r\n esc to interrupt'; sleep 1; printf '\r\033[2K feito\r\n'
  [ -n "$PROG" ] && /bin/sh "$PROG" "$A1" "$A2" '{"type":"agent-turn-complete","thread-id":"th-falso","turn-id":"t-1","last-assistant-message":"feito"}'
done
"#).unwrap();
        format!("/bin/sh {}", script.to_string_lossy())
    }
}

async fn ate(segundos: u64, mut condicao: impl FnMut() -> bool) -> bool {
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(segundos);
    while std::time::Instant::now() < limite {
        if condicao() { return true; }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    condicao()
}

async fn servidor(state: &Arc<EngineState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    ganchos::definir_porta(state, listener.local_addr().unwrap().port());
    let app = ganchos::router(state.clone());
    tokio::spawn(async move { let _ = axum::serve(listener, app).await; });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn codex_falso_avisa_o_fim_do_turno_e_encadeia_o_notify_do_usuario() {
    let c = cena();
    servidor(&c.state).await;
    fs::write(c.codex_home.join("config.toml"), "notify = [\"/bin/echo\", \"oi\"]\n").unwrap();
    let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
    let comando = c.codex();
    // A preparação é a mesma do spawn, mas com o shim falso no lugar do `omni-shim`.
    let (digitado, ambiente) = preparar_com(&c.state, &terminal, &c.origem(), Some(comando.clone()), Some(c.shim()));
    let digitado = digitado.unwrap();
    assert_eq!(digitado, format!("{comando} -c \"$MEDZA_CODEX_NOTIFY\""));
    assert_eq!(preparar_com(&c.state, &terminal, &c.origem(), Some(digitado.clone()), Some(c.shim())).0.unwrap(), digitado, "idempotente");
    let (mut sessao_ambiente, env) = (c.origem().env, ambiente);
    sessao_ambiente.extend(env);
    let origem = SessionOrigin { env: sessao_ambiente, ..c.origem() };
    // Abre a PTY já com a linha preparada (o `preparo_da_sessao` não acha `omni-shim` ao lado do binário de teste).
    let (pid, writer, master, child, reader) = open_pty_and_spawn(&c.projeto.to_string_lossy(), "/bin/sh", 30, 100, &origem.env, true).unwrap();
    let meta = TerminalSession { id: terminal.clone(), project_id: "p".into(), name: "Codex".into(), cwd: c.projeto.to_string_lossy().into_owned(),
        shell: "/bin/sh".into(), state: SessionState::Working, pid, created_at_ms: now_ms(), last_activity_at_ms: now_ms(), output_seq: 0,
        rows: 30, cols: 100, initial_command: Some(comando.clone()), input_locked: false, notice: None, attention_seq: 0, env: c.origem().env,
        provider: Some("codex".into()), profile_id: Some("codex-p".into()), conversation_id: Some("conv".into()), external_session_id: None,
        retoma_ao_abrir: false };
    let sessao = Arc::new(LiveSession { interaction: Mutex::new(interaction::Interaction::new(30, 100)), reserved: std::sync::atomic::AtomicBool::new(false),
        headless: std::sync::atomic::AtomicBool::new(false), meta: Mutex::new(meta), writer: Mutex::new(writer), master: Mutex::new(master),
        child: Mutex::new(child), output: Mutex::new(OutputBuffer { chunks: VecDeque::new(), bytes: 0, next_seq: 0 }) });
    c.state.sessions.lock().unwrap().insert(terminal.clone(), SessionEntry::Live(sessao.clone()));
    spawn_initial_command_thread(sessao.clone(), Some(digitado));
    spawn_reader_thread(c.state.clone(), sessao.clone(), reader, pid);

    assert!(ate(15, || sessao.interaction.lock().unwrap().parser.screen().contents().contains('›')).await, "o Codex falso não subiu");
    let notify = fs::read_to_string(c.dir.path().join("notify.txt")).unwrap();
    assert!(notify.starts_with("notify=[\"") && notify.contains("--medza-notify"), "o Codex recebeu o notify: {notify}");
    assert_eq!(ganchos::turnos_por_evento(&terminal), None, "antes do 1º turno a tela manda");
    let antes = sessao.meta.lock().unwrap().attention_seq;
    write_terminal(&c.state, &terminal, "oi\r").unwrap();
    assert!(ate(15, || ganchos::turnos_por_evento(&terminal) == Some((0, 1))).await, "fim do turno pelo notify");
    assert!(ate(5, || { settle(&sessao); sessao.meta.lock().unwrap().attention_seq == antes + 1 }).await, "um aviso por turno");
    assert_eq!(sessao.meta.lock().unwrap().state, SessionState::Answered);
    assert_eq!(ganchos::resumo(&terminal)["estado"], "ativos");
    // Depois da janela do fim, a tela não conta o mesmo turno de novo.
    std::thread::sleep(std::time::Duration::from_millis(2_500));
    settle(&sessao);
    assert_eq!(sessao.meta.lock().unwrap().attention_seq, antes + 1);
    assert!(ate(5, || fs::read_to_string(c.dir.path().join("usuario.txt")).is_ok_and(|u| u == r#"["/bin/echo","oi"]"#)).await,
        "o notify da conta segue encadeado");
    let persistido = fs::read_to_string(c.dir.path().join("sessions.json")).unwrap_or_default();
    assert!(!persistido.contains("MEDZA_CODEX_NOTIFY") && !persistido.contains("--medza-notify"));
    let _ = close_session(&c.state, &terminal);
}

#[test]
fn so_codex_com_conversa_na_pty_e_sem_remote() {
    let c = cena();
    ganchos::definir_porta(&c.state, 47_990);
    let shim = Some(PathBuf::from("/x/omni-shim"));
    let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
    let claude = SessionOrigin { provider: Some("claude".into()), ..c.origem() };
    assert_eq!(preparar_com(&c.state, &terminal, &claude, Some("claude".into()), shim.clone()), (Some("claude".into()), vec![]));
    let sem_conversa = SessionOrigin { conversation_id: None, ..c.origem() };
    assert_eq!(preparar_com(&c.state, &terminal, &sem_conversa, Some("codex".into()), shim.clone()).1, vec![]);
    let remoto = "codex --remote unix:///tmp/s resume x".to_owned();
    assert_eq!(preparar_com(&c.state, &terminal, &c.origem(), Some(remoto.clone()), shim.clone()).0, Some(remoto));
    assert_eq!(preparar_com(&c.state, &terminal, &c.origem(), Some("codex".into()), None).0.as_deref(), Some("codex"), "sem shim, nada");
    // Eventos desligados nas Configurações: nada.
    omni_core::ganchos::gravar_ajustes(c.dir.path(), &omni_core::ganchos::Ajustes { eventos: false, ..Default::default() }).unwrap();
    assert_eq!(preparar_com(&c.state, &terminal, &c.origem(), Some("codex".into()), shim.clone()).0.as_deref(), Some("codex"));
    omni_core::ganchos::gravar_ajustes(c.dir.path(), &omni_core::ganchos::Ajustes::default()).unwrap();
    let (comando, ambiente) = preparar_com(&c.state, &terminal, &c.origem(), Some("codex".into()), shim);
    assert_eq!(comando.as_deref(), Some("codex -c \"$MEDZA_CODEX_NOTIFY\""));
    assert!(ambiente.iter().any(|(k, v)| k == "MEDZA_CODEX_NOTIFY" && v == "notify=[\"/x/omni-shim\",\"--medza-notify\",\"47990\"]"));
    assert!(ambiente.iter().any(|(k, _)| k == omni_core::ganchos::VARIAVEL_DO_TOKEN));
    assert!(ambiente.iter().all(|(k, _)| k != "MEDZA_NOTIFY_DO_USUARIO"), "a conta não tinha notify");
    ganchos::esquecer(&terminal);
}
