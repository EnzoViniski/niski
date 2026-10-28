//! CLI/PTY totalmente sintéticas, sem processos, TCP ou credenciais reais.
use super::*;
use std::{cell::Cell, sync::atomic::AtomicBool};
use tower::ServiceExt;
use axum::body::Body;

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
        token:"test-only".into(),state_file:dir.path().join("sessions.json"),sessions:Mutex::new(HashMap::new()),id_sequence:AtomicU64::new(1) });
    (dir,state)
}
#[derive(Debug)] struct Filho;
impl portable_pty::ChildKiller for Filho {
    fn kill(&mut self) -> std::io::Result<()> { Ok(()) }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> { Box::new(Filho) }
}
impl Child for Filho {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> { Ok(None) }
    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> { Ok(portable_pty::ExitStatus::with_exit_code(0)) }
    fn process_id(&self) -> Option<u32> { Some(42) }
    #[cfg(windows)] fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> { None }
}
struct Mestre;
impl MasterPty for Mestre {
    fn resize(&self,_:PtySize) -> Result<()> { Ok(()) }
    fn get_size(&self) -> Result<PtySize> { Ok(PtySize{rows:24,cols:100,pixel_width:0,pixel_height:0}) }
    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>> { Ok(Box::new(std::io::empty())) }
    fn take_writer(&self) -> Result<Box<dyn Write + Send>> { Ok(Box::new(std::io::sink())) }
    #[cfg(unix)] fn process_group_leader(&self) -> Option<i32> { None }
    #[cfg(unix)] fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> { None }
    #[cfg(unix)] fn tty_name(&self) -> Option<PathBuf> { None }
}
struct Gravador(Arc<Mutex<Vec<u8>>>);
impl Write for Gravador {
    fn write(&mut self,bytes:&[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(bytes); Ok(bytes.len()) }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
fn sessao(state: &Arc<EngineState>, tela: &str) -> (Arc<LiveSession>, Arc<Mutex<Vec<u8>>>, Action) {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let meta = serde_json::from_value(json!({"id":"s","project_id":"p","name":"Teste","cwd":"/fixture","shell":"fake",
        "state":"answered","pid":42,"created_at_ms":1,"last_activity_at_ms":1,"output_seq":0,"rows":24,"cols":100,
        "provider":"claude","profile_id":"p","conversation_id":"c","external_session_id":"ext"})).unwrap();
    let mut context = interaction::Interaction::new(24,100);
    context.parser.process(b"\x1b[?2004h"); context.parser.process(tela.as_bytes());
    let session = Arc::new(LiveSession { interaction:Mutex::new(context),reserved:AtomicBool::new(false),headless:AtomicBool::new(false),
        meta:Mutex::new(meta),writer:Mutex::new(Box::new(Gravador(bytes.clone()))),master:Mutex::new(Box::new(Mestre)),
        child:Mutex::new(Box::new(Filho)),output:Mutex::new(OutputBuffer { chunks:VecDeque::new(),bytes:0,next_seq:0 }) });
    state.sessions.lock().unwrap().insert("s".into(),SessionEntry::Live(session.clone()));
    let dir = state.state_file.parent().unwrap();
    atomic_write_json(&dir.join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":"/fixture","title":"Teste","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"p","external_session_id":"ext","terminal_session_id":"s","started_at_ms":1}]
    }]})).unwrap();
    let perfil = dir.join("perfil");
    atomic_write_json(&dir.join("profiles.json"),&json!({"profiles":[{"id":"p","provider":"claude","name":"Fixture","config_dir":perfil,"builtin":false,"created_at_ms":0}]})).unwrap();
    let transcript = omni_core::conversations::claude_transcript_path(&perfil,"/fixture","ext");
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    std::fs::write(transcript,json!({"type":"assistant","message":{"model":"claude-opus-4-8","role":"assistant","content":[]}}).to_string()).unwrap();
    let revision = session.interaction.lock().unwrap().revision(&session);
    let action = Action { id:"a".into(),conversation_id:"c".into(),session_id:"s".into(),state:"queued".into(),created_at_ms:now_ms(),error:None,key:"k".into(),revision,
        kind:ActionKind::Comando(Pedido::Slash(Comando { comando:"fast".into(),argumento:Some("on".into()) })) };
    (session,bytes,action)
}

#[test] fn retomada_com_os_quatro_modos_e_sem_flag_no_codex() {
    let a = Ajustes { profile_id:None,model:Some("opus".into()),effort:Some("high".into()) };
    for modo in ["default","acceptEdits","plan","auto"] {
        assert_eq!(comando_de_retomada("claude","claude","ext",&a,Some(modo)).unwrap(),format!("claude --resume ext --model opus --effort high --permission-mode {modo}"));
        assert_eq!(com_modo("claude --permission-mode plan --resume ext",modo).unwrap(),format!("claude --resume ext --permission-mode {modo}"));
    }
    assert!(comando_de_retomada("claude","claude","ext",&a,Some("plan;exit")).is_err());
    assert!(!comando_de_retomada("codex","codex","ext",&a,Some("plan")).unwrap().contains("permission-mode"));
}
#[test] fn modo_confirmado_persiste_por_conversa() {
    let (dir,state) = fixture();
    guardar_modo(&state,"a","auto").unwrap(); guardar_modo(&state,"b","plan").unwrap();
    assert_eq!(modo_salvo(dir.path(),"a").as_deref(),Some("auto"));
    assert_eq!(modo_salvo(dir.path(),"b").as_deref(),Some("plan"));
    assert_eq!(modo_salvo(dir.path(),"c"),None);
}
#[test] fn ciclo_rele_selo_ate_os_quatro_alvos() {
    let modos = ["default","acceptEdits","plan","auto"];
    for alvo in modos {
        let pos = Cell::new(0usize);
        ciclar_modo(alvo,||Some(modos[pos.get()]),|tecla| {
            assert_eq!(tecla,b"\x1b[Z"); pos.set((pos.get()+1)%4); Ok(())
        },||{}).unwrap();
        assert_eq!(modos[pos.get()],alvo);
    }
}
#[test] fn modo_indisponivel_nao_finge_confirmacao() {
    assert!(ciclar_modo("auto",||Some("plan"),|_|Ok(()),||{}).is_err());
}
#[test] fn tela_sem_selo_nao_chuta_default() {
    let (_dir,state) = fixture();
    let (session,_,_) = sessao(&state,"nada aqui");
    assert_eq!(modo_da_sessao(&session),None);
    let mut c = session.interaction.lock().unwrap();
    c.parser.process("\x1b[2J\x1b[H? for shortcuts\r\n❯ ".as_bytes());
    assert_eq!(modo_na_tela(c.parser.screen()),Some("default"));
}
#[test] fn fast_so_chega_com_campo_vazio_e_mesma_revisao() {
    let (_dir,state) = fixture();
    let (session,bytes,action) = sessao(&state,"❯ ");
    let pedido = Pedido::Slash(Comando{comando:"fast".into(),argumento:Some("on".into())});
    executar_com(&state,&action,&pedido,|_,_|true).unwrap();
    assert_eq!(&*bytes.lock().unwrap(),b"\x1b[200~/fast on\x1b[201~\r");
    bytes.lock().unwrap().clear();
    assert!(executar_com(&state,&action,&pedido,|_,_|true).is_err(),"revisão antiga");
    let mut c = session.interaction.lock().unwrap(); c.parser.process(b"rascunho");
    let action = Action{revision:c.revision(&session),..action}; drop(c);
    assert!(executar_com(&state,&action,&pedido,|_,_|true).is_err(),"campo ocupado");
    assert!(bytes.lock().unwrap().is_empty());
}
#[tokio::test] async fn rotas_rejeitam_slash_livre_e_aceitam_quatro_modos() {
    let (_dir,state) = fixture(); let (session,_,_) = sessao(&state,"❯ ");
    let revision = session.interaction.lock().unwrap().revision(&session);
    let app = rotas().with_state(state.clone());
    for comando in ["model","/model","config","exit"] {
        let req = Request::builder().method("POST").uri("/conversas/c/comando").header("content-type","application/json")
            .body(Body::from(json!({"comando":comando}).to_string())).unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(),StatusCode::BAD_REQUEST);
    }
    // Idempotência já é coberta pela fila; aqui provamos validação dos quatro estados sem ps.
    for modo in ["default","acceptEdits","plan","auto"] {
        let kind = ActionKind::Comando(Pedido::Modo(modo.into()));
        state.mobile.actions.lock().unwrap().push_back(Action{id:modo.into(),conversation_id:"c".into(),session_id:"s".into(),state:"sent".into(),created_at_ms:now_ms(),error:None,key:modo.into(),revision:revision.clone(),kind});
        let req = Request::builder().method("POST").uri("/conversas/c/modo").header("content-type","application/json").header("idempotency-key",modo)
            .body(Body::from(json!({"modo":modo}).to_string())).unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(),StatusCode::ACCEPTED);
    }
}

/// Mensagem comum não cai por redesenho da tela (linha de status, indicador girando do Codex), mas
/// aprovação continua presa à tela exata que o usuário viu.
#[test] fn mensagem_comum_nao_cai_por_redesenho_e_aprovacao_cai() {
    let (_dir,state) = fixture();
    let (session,bytes,action) = sessao(&state,"❯ ");
    // A CLI redesenha algo fora do campo (status) depois que o chat leu a versão.
    { let mut c = session.interaction.lock().unwrap(); c.parser.process(b"\x1b7\x1b[20;1HGPT-6-Luna low \xc2\xb7 ~/omni\x1b8"); }
    let prompt = Action { kind: ActionKind::Prompt("oi".into()), ..action.clone() };
    super::super::fila::execute_checked(&state,&prompt,|_,_|true).unwrap();
    assert!(bytes.lock().unwrap().starts_with(b"\x1b[200~oi\x1b[201~"), "mensagem colada mesmo com versão antiga");
    let aprovar = Action { kind: ActionKind::Approval(true), ..action };
    assert!(super::super::fila::execute_checked(&state,&aprovar,|_,_|true).is_err(), "aprovação exige a tela exata");
}

/// Fork (modo do Codex na Conversa): a rota aceita os quatro modos do Codex e recusa os do Claude; a
/// troca só reabre com o Codex parado (nenhuma CLI real: os casos que reabririam não rodam aqui).
#[tokio::test] async fn codex_aceita_os_modos_dele_pela_mesma_rota() {
    let (dir,state) = fixture(); let (session,_,_) = sessao(&state,"❯ ");
    session.meta.lock().unwrap().provider = Some("codex".into());
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":"/fixture","title":"Teste","created_at_ms":1,
        "segments":[{"provider":"codex","profile_id":"p","terminal_session_id":"s","started_at_ms":1}]}]})).unwrap();
    let revision = session.interaction.lock().unwrap().revision(&session);
    let app = rotas().with_state(state.clone());
    for modo in ["auto","plan","acceptEdits"] {
        let req = Request::builder().method("POST").uri("/conversas/c/modo").header("content-type","application/json").header("idempotency-key",modo)
            .body(Body::from(json!({"modo":modo}).to_string())).unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(),StatusCode::BAD_REQUEST,"{modo} é do Claude");
    }
    for modo in omni_core::modo_do_codex::MODOS {
        let kind = ActionKind::Comando(Pedido::ModoCodex(modo.into()));
        state.mobile.actions.lock().unwrap().push_back(Action{id:modo.into(),conversation_id:"c".into(),session_id:"s".into(),state:"sent".into(),created_at_ms:now_ms(),error:None,key:modo.into(),revision:revision.clone(),kind});
        let req = Request::builder().method("POST").uri("/conversas/c/modo").header("content-type","application/json").header("idempotency-key",modo)
            .body(Body::from(json!({"modo":modo}).to_string())).unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(),StatusCode::ACCEPTED,"{modo}");
    }
}

#[test] fn codex_nao_reabre_trabalhando_nem_sendo_outro_agente() {
    let (dir,state) = fixture();
    let (session,bytes,action) = sessao(&state,"\r\n OpenAI Codex\r\n esc to interrupt\r\n› ");
    session.meta.lock().unwrap().provider = Some("codex".into());
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":"/fixture","title":"Teste","created_at_ms":1,
        "segments":[{"provider":"codex","profile_id":"p","terminal_session_id":"s","started_at_ms":1}]}]})).unwrap();
    session.meta.lock().unwrap().initial_command = Some("codex -m x".into());
    let erro = executar_com(&state,&action,&Pedido::ModoCodex("semPerguntar".into()),|_,_|true).unwrap_err().to_string();
    assert!(erro.contains("trabalhando"),"{erro}");
    session.meta.lock().unwrap().provider = Some("claude".into());
    assert!(executar_com(&state,&action,&Pedido::ModoCodex("semPerguntar".into()),|_,_|true).is_err());
    assert!(executar_com(&state,&action,&Pedido::ModoCodex("auto".into()),|_,_|true).is_err(),"modo do Claude");
    assert!(bytes.lock().unwrap().is_empty(),"nada digitado na CLI");
    assert_eq!(session.meta.lock().unwrap().initial_command.as_deref(),Some("codex -m x"),"comando intacto");
}

#[test] fn retomar_codex_leva_o_modo_guardado() {
    let a = Ajustes { profile_id:None, model:None, effort:None };
    assert_eq!(comando_de_retomada("codex","codex","th-1",&a,Some("semPerguntar")).unwrap(),"codex resume th-1 -a never -s workspace-write");
    assert_eq!(comando_de_retomada("codex","codex","th-1",&a,Some("plan")).unwrap(),"codex resume th-1","modo do Claude não vai para o Codex");
    // Conversa que passou do Codex para o Claude: o modo do Codex guardado não derruba a retomada.
    assert_eq!(comando_de_retomada("claude","claude","ext",&a,Some("acessoTotal")).unwrap(),"claude --resume ext");
    let (dir,state) = fixture();
    guardar_modo(&state,"c","somenteLeitura").unwrap();
    assert_eq!(modo_salvo(dir.path(),"c").as_deref(),Some("somenteLeitura"));
}

/// Fork (perguntas na Conversa): a terceira opção do diálogo de permissão chega à CLI pela fila, presa à
/// mesma tela que o cartão mostrou.
#[test] fn opcao_extra_da_permissao_vai_pela_fila_com_a_tecla_dela() {
    let (_dir,state) = fixture();
    let tela = "Do you want to make this edit to a.ts?\r\n❯ 1. Yes\r\n  2. Yes, allow all edits during this session (shift+tab)\r\n  3. No, and tell Claude what to do differently (esc)\r\n";
    let (session,bytes,action) = sessao(&state,tela);
    let opcoes = crate::opcoes_da_permissao::opcoes(&session.interaction.lock().unwrap().parser.screen().contents(),"claude");
    assert_eq!(opcoes.iter().map(|o| o.papel).collect::<Vec<_>>(), ["permitir","outra","negar"]);
    let acao = Action { kind: ActionKind::OpcaoDaPermissao(2), ..action };
    execute_checked(&state,&acao,|_,_|true).unwrap();
    assert_eq!(&*bytes.lock().unwrap(),b"2\r");
    bytes.lock().unwrap().clear();
    let sumiu = Action { kind: ActionKind::OpcaoDaPermissao(7), ..acao };
    assert!(execute_checked(&state,&sumiu,|_,_|true).is_err(),"opção que a tela não tem");
    assert!(bytes.lock().unwrap().is_empty());
}
