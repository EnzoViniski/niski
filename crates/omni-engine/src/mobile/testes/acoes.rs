//! Testes da fila e das rotas com uma PTY falsa (só no Windows). Fork: o antigo `mod actions` do
//! `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;
#[derive(Debug)] struct FakeChild;
impl portable_pty::ChildKiller for FakeChild {
    fn kill(&mut self) -> std::io::Result<()> { Ok(()) }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> { Box::new(FakeChild) }
}
impl Child for FakeChild {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> { Ok(None) }
    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> { Ok(portable_pty::ExitStatus::with_exit_code(0)) }
    fn process_id(&self) -> Option<u32> { Some(42) }
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> { None }
}
struct FakeMaster;
impl MasterPty for FakeMaster {
    fn resize(&self,_:PtySize) -> Result<()> { Ok(()) }
    fn get_size(&self) -> Result<PtySize> { Ok(PtySize{rows:20,cols:100,pixel_width:0,pixel_height:0}) }
    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>> { Ok(Box::new(std::io::empty())) }
    fn take_writer(&self) -> Result<Box<dyn Write + Send>> { Ok(Box::new(std::io::sink())) }
}
struct RecordingWriter(Arc<Mutex<Vec<u8>>>);
impl Write for RecordingWriter {
    fn write(&mut self,bytes:&[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(bytes); Ok(bytes.len()) }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
fn fake_session(state:&Arc<EngineState>,text:&str) -> (Arc<LiveSession>,Arc<Mutex<Vec<u8>>>,Action) {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let meta:TerminalSession = serde_json::from_value(json!({"id":"s","project_id":"p","name":"Test","cwd":"C:/test","shell":"test",
        "state":"answered","pid":42,"created_at_ms":1,"last_activity_at_ms":1,"output_seq":0,"rows":20,"cols":100,
        "provider":"claude","profile_id":"profile","conversation_id":"c"})).unwrap();
    let mut context = interaction::Interaction::new(20,100); context.parser.process(b"\x1b[?2004h"); context.parser.process(text.as_bytes());
    let session = Arc::new(LiveSession{interaction:Mutex::new(context),reserved:std::sync::atomic::AtomicBool::new(false),headless:std::sync::atomic::AtomicBool::new(false),
        meta:Mutex::new(meta),writer:Mutex::new(Box::new(RecordingWriter(bytes.clone()))),master:Mutex::new(Box::new(FakeMaster)),
        child:Mutex::new(Box::new(FakeChild)),output:Mutex::new(OutputBuffer{chunks:VecDeque::new(),bytes:0,next_seq:0})});
    state.sessions.lock().unwrap().insert("s".into(),SessionEntry::Live(session.clone()));
    atomic_write_json(&state.state_file.with_file_name("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":"C:/test","title":"Test","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"profile","terminal_session_id":"s","started_at_ms":1}]
    }]})).unwrap();
    let revision = session.interaction.lock().unwrap().revision(&session);
    let action = Action{id:"a".into(),conversation_id:"c".into(),session_id:"s".into(),state:"queued".into(),created_at_ms:now_ms(),error:None,key:"key".into(),revision,kind:ActionKind::Prompt("Olá\nmundo".into())};
    (session,bytes,action)
}
/// Mensagem com o agente no meio do turno: o campo da CLI está livre ("esc to interrupt" na
/// tela), então ela vai para a fila da CLI — sem a trava de versão da tela, que mudaria a cada
/// quadro do spinner. Prompt comum continua recusado durante o turno, e rascunho no campo
/// continua bloqueando a fila.
#[test] fn mensagem_durante_o_turno_entra_na_fila_da_cli() {
    let (_dir,state) = fixture();
    let (session,bytes,prompt) = fake_session(&state,"✻ Pensando… (esc to interrupt)\r\n❯ ");
    {
        let contexto = session.interaction.lock().unwrap();
        assert!(!interaction::ready(contexto.parser.screen(),"claude"), "prompt comum não vale no meio do turno");
        assert!(interaction::pode_enfileirar(contexto.parser.screen(),"claude"), "turno com campo livre aceita fila");
    }
    // A tela anda (o spinner conta segundos) depois que a ação entrou na fila: prompt comum
    // seria recusado pela versão; a fila cola mesmo assim, porque o campo segue livre.
    session.interaction.lock().unwrap().parser.process("\x1b[1;1H✻ Pensando… 12s (esc to interrupt)\x1b[2;3H".as_bytes());
    assert!(execute_checked(&state,&prompt,|_,_| true).is_err(), "prompt comum ignorou a versão da tela");
    let fila = Action { kind: ActionKind::Fila("e depois isso".into()), ..prompt };
    execute_checked(&state,&fila,|_,_| true).unwrap();
    assert!(String::from_utf8_lossy(&bytes.lock().unwrap()).starts_with("\x1b[200~e depois isso\x1b[201~"));

    // Rascunho digitado no PC: a fila não cola por cima.
    let (rascunho,_,_) = fake_session(&state,"✻ Pensando… (esc to interrupt)\r\n❯ meu rascunho");
    assert!(!interaction::pode_enfileirar(rascunho.interaction.lock().unwrap().parser.screen(),"claude"));
}

/// O Enter só sai depois que a colagem aparece no campo. Junto com ela, no mesmo write, a CLI
/// trata o `\r` como parte do texto colado: com anexos (prompt de várias linhas) a mensagem
/// ficava digitada no terminal e nunca era enviada.
#[test] fn prompt_cola_primeiro_e_so_depois_manda_o_enter() {
    let (_dir,state) = fixture(); let (session,bytes,action) = fake_session(&state,"Claude Code\r\n❯ ");
    let colagem = "\x1b[200~Olá\nmundo\x1b[201~".as_bytes();

    // A CLI desenha o texto colado no campo pouco depois; é esse eco que libera o Enter.
    let eco = session.clone();
    let escrito = bytes.clone();
    let desenhou = std::thread::spawn(move || {
        let mut tentativas = 0;
        loop {
            if escrito.lock().unwrap().as_slice() == colagem { break; }
            tentativas += 1;
            assert!(tentativas < 200, "a colagem não foi escrita antes do Enter");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // Enter não pode ter saído junto com a colagem.
        assert_eq!(&*escrito.lock().unwrap(), colagem, "Enter saiu na mesma rajada da colagem");
        eco.interaction.lock().unwrap().parser.process(b"[Pasted text #1 +2 lines]");
    });

    execute_checked(&state,&action,|_,_|true).unwrap();
    desenhou.join().unwrap();
    assert_eq!(&*bytes.lock().unwrap(),"\x1b[200~Olá\nmundo\x1b[201~\r".as_bytes());
    assert!(execute_checked(&state,&action,|_,_|true).is_err());
}

/// CLI que não devolve eco nenhum não pode travar o envio para sempre.
#[test] fn sem_eco_da_colagem_o_enter_sai_no_limite() {
    let (_dir,state) = fixture(); let (_session,bytes,action) = fake_session(&state,"Claude Code\r\n❯ ");
    let comeco = std::time::Instant::now();
    execute_checked(&state,&action,|_,_|true).unwrap();
    assert!(comeco.elapsed() >= std::time::Duration::from_secs(1), "não esperou o eco");
    assert!(bytes.lock().unwrap().ends_with(b"\r"));
}
/// A TUI redesenha o diálogo de aprovação em pedaços. Um quadro parcial não pode virar
/// "pedido novo" — era o aviso já visto voltando sozinho.
#[test] fn aprovacao_redesenhada_conta_um_aviso_so() {
    let (_dir,state) = fixture(); let (session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
    let dialogo = "\x1b[2J\x1b[HDo you want to proceed?\r\n❯ 1. Yes\r\n2. Yes, always\r\n3. No";
    let seq = || session.meta.lock().unwrap().attention_seq;
    crate::append_output(&session,dialogo.as_bytes(),Some(42));
    assert_eq!(seq(),1);
    crate::append_output(&session,b"\x1b[2J\x1b[HDo you want",Some(42));
    crate::append_output(&session,dialogo.as_bytes(),Some(42));
    assert_eq!(seq(),1,"redesenho contou como pedido novo");
    // Aprovado, o agente volta a trabalhar: o próximo diálogo é outro pedido.
    crate::append_output(&session,b"\x1b[2J\x1b[H* Running (esc to interrupt)",Some(42));
    crate::append_output(&session,dialogo.as_bytes(),Some(42));
    assert_eq!(seq(),2);
    assert_eq!(summaries(&state)[0]["attention"],"s:2");
}
/// Failover reabre a conversa com `--resume`, que reexibe o aviso que fez a conta trocar.
/// Isso é histórico: a conversa não pode aparecer "interrompida" (nem disparar outro
/// failover) antes de um turno novo. Depois de um turno, o aviso volta a valer.
#[test] fn aviso_reexibido_pelo_resume_nao_conta_antes_do_primeiro_turno() {
    let (_dir,state) = fixture(); let (session,_,_) = fake_session(&state,"");
    let aviso = "[2J[H> opa
  ⎿  You've hit your session limit · resets 8:10pm
";
    let notice = || session.meta.lock().unwrap().notice.clone();
    crate::append_output(&session,aviso.as_bytes(),Some(42));
    assert_eq!(notice(),None,"histórico do resume virou aviso");
    crate::append_output(&session,b"[2J[H* Working (esc to interrupt)",Some(42));
    crate::append_output(&session,aviso.as_bytes(),Some(42));
    assert_eq!(notice().as_deref(),Some("usage_limit"));
}
/// "Toda vez que entro na conversa ele começa a pensar": abrir a aba redesenha a TUI, e
/// qualquer saída virava "trabalhando". Com o agente parado, redesenho não muda o estado.
#[test] fn redesenho_com_o_agente_parado_nao_vira_trabalhando() {
    let (_dir,state) = fixture(); let (session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
    let estado = || session.meta.lock().unwrap().state.clone();
    crate::append_output(&session,b"\x1b[2J\x1b[HClaude Code\r\n> ",Some(42));
    assert_eq!(estado(),SessionState::Answered,"redesenho virou trabalho");
    crate::append_output(&session,b"\x1b[2J\x1b[H* Pensando (esc to interrupt)",Some(42));
    assert_eq!(estado(),SessionState::Working);
}

/// Plano/Auto pelo chat do PC: o `Idempotency-Key` tem de atravessar o protocolo local.
#[tokio::test] async fn modo_pelo_pc_leva_o_idempotency_key() {
    let (_dir,state) = fixture(); let (_session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
    let EngineResponse::Api { body, .. } = local(state,"POST","/conversas/c/modo",Some(r#"{"modo":"plan"}"#.into()),None,
        vec![("Idempotency-Key".into(),"chave-1".into())]).await else { panic!("não é Api") };
    assert!(!body.contains("Idempotency-Key"), "cabeçalho se perdeu: {body}");
}
/// "Na web devo conseguir acessar os terminais sem agentes": a lista traz só shells, e as
/// teclas/saída de um id de agente são recusadas — agente se usa pela conversa.
#[tokio::test] async fn terminais_sem_agente_listam_e_recusam_sessao_de_agente() {
    let (_dir,state) = fixture(); let (_agente,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
    let lista = |state| async move {
        let EngineResponse::Api { body, .. } = local(state,"GET","/terminais",None,None,vec![]).await else { panic!("não é Api") };
        serde_json::from_str::<Value>(&body).unwrap()
    };
    assert_eq!(lista(state.clone()).await, json!([]), "sessão de agente apareceu como terminal");
    let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/terminais/s/teclas",Some(r#"{"dados":"ls\r"}"#.into()),None,vec![]).await else { panic!() };
    assert_eq!(status,404);
    let EngineResponse::Api { status, .. } = local(state,"POST","/terminais",Some(r#"{"project_id":"nao-existe"}"#.into()),None,vec![]).await else { panic!() };
    assert_eq!(status,404);
}

/// Ponta a ponta: abrir um shell no projeto pela web, digitar, ler a saída e fechar.
#[tokio::test(flavor = "multi_thread")] async fn web_abre_shell_no_projeto_roda_e_fecha() {
    let (dir,state) = fixture();
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,"segments":[]
    }]})).unwrap();
    let chamar = |metodo: &'static str, caminho: String, corpo: Option<&'static str>| {
        let state = state.clone();
        async move {
            let EngineResponse::Api { status, body } = local(state,metodo,&caminho,corpo.map(String::from),None,vec![]).await else { panic!("não é Api") };
            (status, serde_json::from_str::<Value>(&body).unwrap_or(Value::Null))
        }
    };
    let (status, aberto) = chamar("POST","/terminais".into(),Some(r#"{"project_id":"p"}"#)).await;
    assert_eq!(status,201,"{aberto}");
    let id = aberto["id"].as_str().unwrap().to_string();
    assert_eq!(chamar("GET","/terminais".into(),None).await.1[0]["id"],id.as_str());

    let (status, _) = chamar("POST",format!("/terminais/{id}/teclas"),Some(r#"{"dados":"echo OMNI_WEB_OK\r"}"#)).await;
    assert_eq!(status,200);
    let mut saida = String::new();
    for _ in 0..150 {
        saida = chamar("GET",format!("/terminais/{id}/saida"),None).await.1["data"].as_str().unwrap_or("").to_string();
        if saida.matches("OMNI_WEB_OK").count() >= 2 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(saida.matches("OMNI_WEB_OK").count() >= 2, "o shell não rodou o comando: {saida:?}");

    assert_eq!(chamar("POST",format!("/terminais/{id}/fechar"),None).await.0,200);
    assert_eq!(chamar("GET","/terminais".into(),None).await.1,json!([]));
}

/// Conta, modelo e esforço viram argumentos da linha que retoma a conversa — cada CLI com a
/// sua sintaxe.
#[test] fn comando_de_retomada_por_cli() {
    let ajustes = |model: Option<&str>, effort: Option<&str>| Ajustes { profile_id: None, model: model.map(String::from), effort: effort.map(String::from) };
    assert_eq!(comando_de_retomada("claude","claude","abc",&ajustes(Some("opus"),Some("high"))).unwrap(),
        "claude --resume abc --model opus --effort high");
    assert_eq!(comando_de_retomada("codex","codex","xyz",&ajustes(Some("gpt-5.6-sol"),Some("xhigh"))).unwrap(),
        "codex resume xyz -m gpt-5.6-sol -c model_reasoning_effort=xhigh");
    assert_eq!(comando_de_retomada("claude","claude","abc",&ajustes(None,None)).unwrap(), "claude --resume abc");
    assert!(comando_de_retomada("cursor-agent","cursor","abc",&ajustes(None,None)).is_err());
    assert_eq!(comando_de_retomada("agy","antigravity","c1",&ajustes(Some("gemini-4-argon"),Some("high"))).unwrap(),
        "agy --model gemini-4-argon --effort high --conversation c1");
    assert_eq!(comando_de_retomada("agy","antigravity","c1",&ajustes(None,None)).unwrap(), "agy --conversation c1");
}

/// O valor vira argumento de um comando digitado num shell: nada de `;`, espaço ou aspas.
#[tokio::test] async fn ajustes_recusam_valor_que_vira_comando() {
    let (_dir,state) = fixture(); let (_session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
    for corpo in [r#"{"profile_id":null,"model":"opus; rm -rf ~","effort":null}"#, r#"{"profile_id":null,"model":null,"effort":"high && calc"}"#] {
        let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/conversas/c/ajustes",Some(corpo.into()),None,
            vec![("Idempotency-Key".into(),"k".into())]).await else { panic!("não é Api") };
        assert_eq!(status,400,"{corpo}");
    }
    assert!(valor_seguro("claude-opus-5-5[1m]"));
    assert!(!valor_seguro("a b"));
}

#[tokio::test] async fn opcoes_listam_modelos_e_contas_do_mesmo_agente() {
    let (dir,state) = fixture(); let (_session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
    atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[
        {"id":"profile","provider":"claude","name":"Padrão","config_dir":dir.path().join("a").to_string_lossy(),"builtin":false,"created_at_ms":1},
        {"id":"outra","provider":"claude","name":"Trabalho","config_dir":dir.path().join("b").to_string_lossy(),"builtin":false,"created_at_ms":1},
        {"id":"cdx","provider":"codex","name":"Codex","config_dir":dir.path().join("c").to_string_lossy(),"builtin":false,"created_at_ms":1}
    ]})).unwrap();
    let EngineResponse::Api { status, body } = local(state,"GET","/conversas/c/opcoes",None,None,vec![]).await else { panic!("não é Api") };
    assert_eq!(status,200,"{body}");
    let opcoes: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(opcoes["provider"],"claude");
    let contas: Vec<&str> = opcoes["profiles"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(contas, ["Padrão","Trabalho"], "conta de outro agente apareceu");
    assert!(opcoes["models"].as_array().unwrap().iter().any(|m| m["id"] == "opus"));
}

/// Conversa sem sessão aberta (o caso do print: "Histórico") se retoma com conta, modelo e
/// esforço; uma que já está aberta não — ali o caminho é "ajustes".
#[tokio::test] async fn retomar_conversa_sem_sessao_e_recusar_a_aberta() {
    let (dir,state) = fixture();
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":null,"external_session_id":"abc","transcript_path":null,"terminal_session_id":null,"started_at_ms":1}]
    }]})).unwrap();
    let pedir = |state: Arc<EngineState>, chave: &'static str| async move {
        let EngineResponse::Api { status, body } = local(state,"POST","/conversas/c/retomar",
            Some(r#"{"profile_id":null,"model":"opus","effort":"high"}"#.into()),None,
            vec![("Idempotency-Key".into(),chave.into())]).await else { panic!("não é Api") };
        (status, serde_json::from_str::<Value>(&body).unwrap_or(Value::Null))
    };
    let (status, corpo) = pedir(state.clone(),"k1").await;
    assert_eq!(status,202,"{corpo}");
    assert_eq!(corpo["conversation_id"],"c");
    let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/conversas/c/retomar",
        Some(r#"{"profile_id":null,"model":"opus; calc","effort":null}"#.into()),None,vec![("Idempotency-Key".into(),"k2".into())]).await else { panic!() };
    assert_eq!(status,400,"valor perigoso passou");

    let (_sessao,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
    assert_eq!(pedir(state,"k3").await.0,409,"retomou uma conversa que já está aberta");
}

/// Retomar do histórico noutra conta: o transcript vai para a pasta da conta escolhida, e um
/// pedido que não bate com nada do histórico (ou conta de outro agente) é recusado.
#[tokio::test] async fn historico_leva_a_conversa_para_outra_conta() {
    let (dir,state) = fixture();
    let (conta_a, conta_b) = (dir.path().join("a"), dir.path().join("b"));
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let origem = omni_core::conversations::claude_transcript_path(&conta_a,&projeto.to_string_lossy(),"sessao-1");
    std::fs::create_dir_all(origem.parent().unwrap()).unwrap();
    std::fs::write(&origem,json!({"type":"user","cwd":projeto.to_string_lossy(),"timestamp":"2026-10-01T10:00:00Z",
        "message":{"role":"user","content":"oi"}}).to_string()).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[
        {"id":"a","provider":"claude","name":"Pessoal","config_dir":conta_a.to_string_lossy(),"builtin":false,"created_at_ms":1},
        {"id":"b","provider":"claude","name":"Trabalho","config_dir":conta_b.to_string_lossy(),"builtin":false,"created_at_ms":1},
        {"id":"x","provider":"codex","name":"Codex","config_dir":dir.path().join("x").to_string_lossy(),"builtin":false,"created_at_ms":1}
    ]})).unwrap();
    let pedir = |state: Arc<EngineState>, corpo: &'static str| async move {
        let EngineResponse::Api { status, body } = local(state,"POST","/historico/levar",Some(corpo.into()),None,vec![]).await else { panic!() };
        (status, body)
    };
    let (status, corpo) = pedir(state.clone(),r#"{"provider":"claude","profile_id":"a","session_id":"sessao-1","destino":"b"}"#).await;
    assert_eq!(status,200,"{corpo}");
    let destino = omni_core::conversations::claude_transcript_path(&conta_b,&projeto.to_string_lossy(),"sessao-1");
    assert_eq!(std::fs::read(&destino).unwrap(), std::fs::read(&origem).unwrap());
    assert_eq!(pedir(state.clone(),r#"{"provider":"claude","profile_id":"a","session_id":"outra","destino":"b"}"#).await.0,404);
    assert_eq!(pedir(state.clone(),r#"{"provider":"claude","profile_id":"a","session_id":"sessao-1","destino":"x"}"#).await.0,404);
    assert_eq!(pedir(state,r#"{"provider":"claude","profile_id":"a","session_id":"sessao-1","destino":"b","path":"C:/x"}"#).await.0,422);
}

#[test] fn melhores_arquivos_priorizam_o_nome() {
    let lista: Vec<String> = ["src/app/page.tsx","src/components/AppShell.tsx","docs/app.md","README.md"].map(String::from).to_vec();
    assert_eq!(melhores_arquivos(&lista,"app",3), ["docs/app.md","src/components/AppShell.tsx","src/app/page.tsx"]);
    assert_eq!(melhores_arquivos(&lista,"readme",5), ["README.md"]);
    assert_eq!(melhores_arquivos(&lista,"zzz",5), Vec::<String>::new());
}

/// O `@` da visão Conversa do app: a busca chega pelo protocolo local (`engine_api`), com a
/// busca na query string — o mesmo caminho do Tauri, sem navegador.
#[tokio::test] async fn app_busca_arquivos_pelo_protocolo_local() {
    let (dir,state) = fixture();
    let projeto = dir.path().join("projeto");
    for caminho in ["src/landing-page.ts","src/outro.ts","node_modules/x/landing.js"] {
        let alvo = projeto.join(caminho);
        std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
        std::fs::write(alvo,"x").unwrap();
    }
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,"segments":[]
    }]})).unwrap();
    let EngineResponse::Api { status, body } = local(state,"GET","/conversas/c/arquivos?q=landing",None,None,vec![]).await else { panic!("não é Api") };
    assert_eq!(status,200,"{body}");
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["arquivos"], json!(["src/landing-page.ts"]));
}

#[test] fn varredura_pula_pastas_pesadas() {
    let pasta = tempfile::tempdir().unwrap();
    for caminho in ["src/a.ts","node_modules/x/b.js",".git/HEAD","docs/c.md"] {
        let alvo = pasta.path().join(caminho);
        std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
        std::fs::write(alvo,"x").unwrap();
    }
    assert_eq!(varrer_arquivos(pasta.path()), ["docs/c.md","src/a.ts"]);
}

#[test] fn rejects_desktop_input_closed_cli_and_stale_dialog_without_writing() {
    let (_dir,state) = fixture(); let (session,bytes,mut action) = fake_session(&state,"Claude Code\r\nDo you want to proceed?\r\n❯ 1. Yes\r\n2. Yes, always\r\n3. No");
    action.kind = ActionKind::Approval(true);
    assert!(execute_checked(&state,&action,|_,_|false).is_err());
    session.interaction.lock().unwrap().input_revision += 1;
    assert!(execute_checked(&state,&action,|_,_|true).is_err());
    state.sessions.lock().unwrap().clear();
    assert!(execute_checked(&state,&action,|_,_|true).is_err());
    assert!(bytes.lock().unwrap().is_empty());
}
#[test] fn single_use_approval_and_denial_do_not_select_persistent_permission() {
    for (allow,expected) in [(true,"1\r"),(false,"3\r")] {
        let (_dir,state) = fixture(); let (_,bytes,mut action) = fake_session(&state,"Claude Code\r\nDo you want to proceed?\r\n❯ 1. Yes\r\n2. Yes, always\r\n3. No");
        action.kind = ActionKind::Approval(allow); execute_checked(&state,&action,|_,_|true).unwrap();
        assert_eq!(&*bytes.lock().unwrap(),expected.as_bytes());
    }
}
