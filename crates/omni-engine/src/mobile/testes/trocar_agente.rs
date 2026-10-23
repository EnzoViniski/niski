//! Fork (trocar agente): rota `/conversas/{id}/trocar-agente`. CLIs falsas: o comando publicado de cada
//! agente é `echo` num `/bin/sh` de verdade, então nenhuma CLI de IA roda.
use super::*;

fn perfis(dir: &Path) -> (PathBuf, PathBuf) {
    let (claude, codex) = (dir.join("conta-claude"), dir.join("conta-codex"));
    atomic_write_json(&dir.join("profiles.json"), &json!({"profiles":[
        {"id":"a","provider":"claude","name":"Pessoal","config_dir":claude.to_string_lossy(),"builtin":true,"created_at_ms":1},
        {"id":"x","provider":"codex","name":"Trabalho","config_dir":codex.to_string_lossy(),"builtin":false,"created_at_ms":1},
    ]})).unwrap();
    (claude, codex)
}

fn publicar(state: &EngineState, projeto: &Path) {
    publish(state, vec![PublishedProject { id: "p".into(), name: "projeto".into(), path: projeto.to_string_lossy().into_owned() }],
        vec![PublishedAgent { id: "claude".into(), label: "Claude".into(), command: "echo claude-falso".into(), resume: None },
             PublishedAgent { id: "codex".into(), label: "Codex".into(), command: "echo codex-falso".into(), resume: None }], None).unwrap();
}

async fn chamar(state: &Arc<EngineState>, metodo: &str, caminho: &str, corpo: Option<Value>) -> (u16, Value) {
    let EngineResponse::Api { status, body, .. } = local(state.clone(), metodo, caminho, corpo.map(|c| c.to_string()), None, Vec::new()).await
        else { panic!("não é Api") };
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

/// Conversa do Claude com transcript e sessão viva (`/bin/sh`) na PTY `t1`.
fn conversa_viva(dir: &Path, state: &Arc<EngineState>, conversa: &str) -> (String, PathBuf) {
    let (claude, _) = perfis(dir);
    let projeto = dir.join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let cwd = projeto.to_string_lossy().into_owned();
    let transcript = omni_core::conversations::claude_transcript_path(&claude, &cwd, "sessao-1");
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    std::fs::write(&transcript, [
        json!({"type":"user","uuid":"u1","cwd":cwd,"timestamp":"2026-01-02T10:00:00Z","message":{"role":"user","content":"corrige o login"}}).to_string(),
        json!({"type":"assistant","uuid":"a1","timestamp":"2026-01-02T10:01:00Z","message":{"role":"assistant","content":[{"type":"text","text":"corrigido em auth.ts"}]}}).to_string(),
    ].join("\n") + "\n").unwrap();
    publicar(state, &projeto);
    spawn_terminal_inner(state, "t1".into(), "p".into(), "Login".into(), cwd.clone(), Some("/bin/sh".into()), Some("echo claude-falso".into()), 24, 80, now_ms(),
        SessionOrigin { provider: Some("claude".into()), profile_id: Some("a".into()), conversation_id: Some(conversa.into()),
            external_session_id: Some("sessao-1".into()), env: vec![("CLAUDE_CONFIG_DIR".into(), claude.to_string_lossy().into_owned())], ..Default::default() }).unwrap();
    atomic_write_json(&dir.join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":conversa,"project_id":"p","cwd":cwd,"title":"Login","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"a","external_session_id":"sessao-1","transcript_path":transcript.to_string_lossy(),
            "terminal_session_id":"t1","started_at_ms":1}]
    }]})).unwrap();
    (cwd, projeto)
}

fn viva(state: &EngineState, id: &str) -> Arc<LiveSession> {
    match state.sessions.lock().unwrap().get(id).unwrap() { SessionEntry::Live(s) => s.clone(), _ => panic!("PTY viva") }
}

#[tokio::test] async fn claude_para_codex_na_mesma_pty_com_passagem_e_trecho_novo() {
    let (dir, state) = fixture();
    let (_, projeto) = conversa_viva(dir.path(), &state, "c");
    let sessao = viva(&state, "t1");
    let pid = sessao.meta.lock().unwrap().pid;

    let (status, opcoes) = chamar(&state, "GET", "/conversas/c/trocar-agente", None).await;
    assert_eq!(status, 200, "{opcoes}");
    assert_eq!(opcoes["provider"], "claude");
    assert_eq!(opcoes["agentes"].as_array().unwrap().len(), 2);
    assert_eq!(opcoes["agentes"][1]["profiles"][0]["name"], "Trabalho");
    assert_eq!(opcoes["pode_trocar"], true, "{opcoes}");

    let (status, corpo) = chamar(&state, "POST", "/conversas/c/trocar-agente",
        Some(json!({"provider":"codex","profile_id":"x","mensagem":"agora mede no celular"}))).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["modo"], "passagem");
    assert_eq!(corpo["terminal_session_id"], "t1", "mesma PTY: a aba do PC continua ligada");
    assert_eq!(corpo["relativo"], ".omni-agents/passagens/c-2.md");
    let passagem = std::fs::read_to_string(projeto.join(".omni-agents/passagens/c-2.md")).unwrap();
    assert!(passagem.contains("> corrige o login") && passagem.contains("corrigido em auth.ts") && passagem.contains("**Codex · conta Trabalho**"), "{passagem}");

    // Trecho novo no índice, ligado à mesma PTY; o anterior fecha.
    let indice = omni_core::conversations::read_index(dir.path());
    let segmentos = &indice[0].segments;
    assert_eq!(segmentos.len(), 2);
    assert!(segmentos[0].ended_at_ms.is_some());
    assert_eq!((segmentos[1].provider.as_str(), segmentos[1].profile_id.as_deref(), segmentos[1].terminal_session_id.as_deref()), ("codex", Some("x"), Some("t1")));

    // A PTY agora é do Codex: conta, ambiente e comando trocados, processo novo.
    let meta = sessao.meta.lock().unwrap().clone();
    assert_eq!(meta.provider.as_deref(), Some("codex"));
    assert_eq!(meta.profile_id.as_deref(), Some("x"));
    assert!(meta.env.iter().any(|(k, _)| k == "CODEX_HOME") && !meta.env.iter().any(|(k, _)| k == "CLAUDE_CONFIG_DIR"));
    assert_eq!(meta.initial_command.as_deref(), Some("echo codex-falso"));
    assert_ne!(meta.pid, pid);

    // A conversa na tela: o separador no ponto da troca.
    let (status, linha) = chamar(&state, "GET", "/conversas/c/timeline", None).await;
    assert_eq!(status, 200, "{linha}");
    let textos: Vec<&str> = linha["timeline"]["messages"].as_array().unwrap().iter().filter_map(|m| m["text"].as_str()).collect();
    assert_eq!(textos, ["corrige o login", "corrigido em auth.ts", "Continuou com o Codex · conta Trabalho"]);
    assert_eq!(linha["conversation"]["provider"], "codex", "{linha}");

    // De novo para o mesmo: recusa. Agente que o PC não publicou: recusa.
    assert_eq!(chamar(&state, "POST", "/conversas/c/trocar-agente", Some(json!({"provider":"codex","profile_id":"x"}))).await.0, 409);
    assert_eq!(chamar(&state, "POST", "/conversas/c/trocar-agente", Some(json!({"provider":"cursor"}))).await.0, 409);
    assert_eq!(chamar(&state, "POST", "/conversas/c/trocar-agente", Some(json!({"provider":"claude","mensagem":"/clear"}))).await.0, 400);

    // E volta para o Claude: sessão nova com id escolhido (`--session-id`), passagem 3 com os dois trechos.
    let (status, corpo) = chamar(&state, "POST", "/conversas/c/trocar-agente", Some(json!({"provider":"claude"}))).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["profile_id"], "a", "sem conta escolhida vai a principal do agente");
    let segmentos = omni_core::conversations::read_index(dir.path())[0].segments.clone();
    assert_eq!(segmentos.len(), 3);
    let novo = segmentos[2].external_session_id.clone().expect("id do Claude escolhido na largada");
    assert_ne!(novo, "sessao-1", "conversa nova do Claude, não a antiga");
    assert_eq!(sessao.meta.lock().unwrap().initial_command.as_deref(), Some(format!("echo claude-falso --session-id {novo}").as_str()));
    assert!(projeto.join(".omni-agents/passagens/c-3.md").is_file());
    let _ = stop_session(&state, "t1");
}

/// Conversa sem sessão aberta: a troca abre uma PTY nova já com o agente novo.
#[tokio::test] async fn conversa_fechada_abre_sessao_nova_com_o_agente_novo() {
    let (dir, state) = fixture();
    // Fork: id próprio para as fixtures paralelas não disputarem a trava global de troca.
    conversa_viva(dir.path(), &state, "fechada");
    let _ = close_session(&state, "t1");
    let (status, corpo) = chamar(&state, "POST", "/conversas/fechada/trocar-agente", Some(json!({"provider":"codex"}))).await;
    assert_eq!(status, 200, "{corpo}");
    let terminal = corpo["terminal_session_id"].as_str().unwrap().to_owned();
    assert_ne!(terminal, "t1");
    let segmentos = omni_core::conversations::read_index(dir.path())[0].segments.clone();
    assert_eq!(segmentos.last().unwrap().terminal_session_id.as_deref(), Some(terminal.as_str()));
    assert_eq!(viva(&state, &terminal).meta.lock().unwrap().provider.as_deref(), Some("codex"));
    let _ = stop_session(&state, &terminal);
}
