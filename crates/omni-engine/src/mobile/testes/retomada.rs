//! Fork (retomar-sempre): rotas `/retomada/*` e o "Retomar" da Conversa sem beco sem saída. CLI
//! falsa: o comando publicado do agente é `echo`, então nenhuma CLI de verdade roda.
use super::*;

fn perfis(dir: &Path) -> (PathBuf, PathBuf) {
    let (a, b) = (dir.join("conta-a"), dir.join("conta-b"));
    atomic_write_json(&dir.join("profiles.json"), &json!({"profiles":[
        {"id":"a","provider":"claude","name":"Pessoal","config_dir":a.to_string_lossy(),"builtin":false,"created_at_ms":1},
        {"id":"b","provider":"claude","name":"Trabalho","config_dir":b.to_string_lossy(),"builtin":false,"created_at_ms":1},
    ]})).unwrap();
    (a, b)
}

fn transcript(conta: &Path, cwd: &str, id: &str) -> PathBuf {
    let caminho = omni_core::conversations::claude_transcript_path(conta, cwd, id);
    std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
    let linhas = [
        json!({"type":"user","cwd":cwd,"timestamp":"2026-10-01T10:00:00Z","message":{"role":"user","content":"corrige o login"}}).to_string(),
        json!({"type":"assistant","timestamp":"2026-10-01T10:01:00Z","message":{"role":"assistant","content":[{"type":"text","text":"corrigido em auth.ts"}]}}).to_string(),
    ];
    std::fs::write(&caminho, linhas.join("\n") + "\n").unwrap();
    caminho
}

async fn post(state: &Arc<EngineState>, caminho: &str, corpo: Value, chave: Option<&str>) -> (u16, Value) {
    let headers = chave.map(|k| vec![("Idempotency-Key".to_owned(), k.to_owned())]).unwrap_or_default();
    let EngineResponse::Api { status, body, .. } = local(state.clone(), "POST", caminho, Some(corpo.to_string()), None, headers).await else { panic!("não é Api") };
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

fn publicar_echo(state: &EngineState, projeto: &Path) {
    publish(state, vec![PublishedProject { id: "p".into(), name: "projeto".into(), path: projeto.to_string_lossy().into_owned() }],
        vec![PublishedAgent { id: "claude".into(), label: "Claude".into(), command: "echo".into(), resume: None }], None).unwrap();
}

/// `/exit` encerra o agente, mas deixa o shell vivo. Histórico prepara uma retomada real;
/// a rota da Conversa reinicia a mesma PTY, em vez de responder `ja_aberta` para um shell vazio.
#[tokio::test] async fn shell_vivo_sem_agente_nao_impede_retomada() {
    let (dir, state) = fixture();
    let (a, _) = perfis(dir.path());
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let cwd = projeto.to_string_lossy().into_owned();
    transcript(&a, &cwd, "sessao-1");
    spawn_terminal_inner(&state, "t1".into(), "p".into(), "Teste".into(), cwd.clone(),
        Some("/bin/sh".into()), None, 24, 80, now_ms(),
        SessionOrigin { provider: Some("claude".into()), profile_id: Some("a".into()),
            conversation_id: Some("c".into()), external_session_id: Some("sessao-1".into()),
            env: vec![("CLAUDE_CONFIG_DIR".into(), a.to_string_lossy().into_owned())], ..Default::default() }).unwrap();
    let sessao = match state.sessions.lock().unwrap().get("t1").unwrap() {
        SessionEntry::Live(s) => s.clone(), _ => panic!("PTY viva"),
    };
    let pid = sessao.meta.lock().unwrap().pid;
    assert!(pid.is_some(), "shell realmente vivo");
    // Deixa o shell desenhar o prompt; nenhuma CLI de IA é executada.
    std::thread::sleep(std::time::Duration::from_millis(200));
    {
        let mut meta = sessao.meta.lock().unwrap();
        meta.state = SessionState::Answered;
        meta.last_activity_at_ms = now_ms().saturating_sub(5_000);
        meta.initial_command = Some("echo retomada-sintetica".into());
    }
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":cwd,"title":"Teste","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"a","external_session_id":"sessao-1","terminal_session_id":"t1","started_at_ms":1}]
    }]})).unwrap();
    let (status, corpo) = post(&state, "/retomada/preparar",
        json!({"provider":"claude","session_id":"sessao-1","profile_id":"a","cwd":cwd,"preparar":true}), None).await;
    assert_eq!(status, 200, "{corpo}");
    assert!(corpo["viva"].is_null(), "PID do shell não prova que o agente está vivo: {corpo}");
    assert_eq!(corpo["pronto"], true);
    let (status, corpo) = post(&state, "/conversas/c/retomar",
        json!({"profile_id":null,"model":null,"effort":null}), Some("reabrir-shell")).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["reaberta"], true);
    assert_eq!(corpo["terminal_session_id"], "t1");
    assert_ne!(sessao.meta.lock().unwrap().pid, pid);
    let _ = stop_session(&state, "t1");
}

/// Histórico: conversa gravada na conta B, retomada pedida na conta A → o engine acha e copia para
/// onde a CLI da conta A procura. Conversa inexistente: `transcript: false` (a tela oferece conversa nova).
#[tokio::test] async fn preparar_acha_a_conversa_em_outra_conta_e_copia() {
    let (dir, state) = fixture();
    let (a, b) = perfis(dir.path());
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let cwd = projeto.to_string_lossy().into_owned();
    transcript(&b, &cwd, "sessao-1");
    let (status, corpo) = post(&state, "/retomada/preparar",
        json!({"provider":"claude","session_id":"sessao-1","profile_id":"a","cwd":cwd,"preparar":true}), None).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["transcript"], true);
    assert_eq!(corpo["pronto"], true);
    assert_eq!(corpo["conta"], "a");
    assert!(corpo["viva"].is_null());
    assert!(omni_core::conversations::claude_transcript_path(&a, &cwd, "sessao-1").is_file(), "copiado para a conta A");

    let (_, nada) = post(&state, "/retomada/preparar", json!({"provider":"claude","session_id":"nao-existe","cwd":cwd}), None).await;
    assert_eq!(nada["transcript"], false);
    assert_eq!(post(&state, "/retomada/preparar", json!({"provider":"claude","session_id":"../x","cwd":cwd}), None).await.0, 400);
}

#[tokio::test] async fn preparar_preserva_o_servidor_local_da_sessao_registrada() {
    let (dir, state) = fixture();
    let meta: TerminalSession = serde_json::from_value(json!({
        "id":"local","project_id":"p","name":"Codex","cwd":dir.path().to_string_lossy(),"shell":"/bin/sh",
        "state":"stopped","pid":null,"created_at_ms":1,"last_activity_at_ms":2,"output_seq":0,"rows":24,"cols":80,
        "provider":"codex","external_session_id":"local-123",
        "initial_command":"codex --oss --local-provider ollama --model empresa/modelo:local"
    })).unwrap();
    state.sessions.lock().unwrap().insert("local".into(), SessionEntry::Historical(meta));
    let (status, corpo) = post(&state, "/retomada/preparar", json!({"provider":"codex","session_id":"local-123","cwd":dir.path().to_string_lossy()}), None).await;
    assert_eq!(status, 200);
    assert_eq!(corpo["modelo_local"], json!({"provedor":"ollama","modelo":"empresa/modelo:local"}));
}

/// Worktree apagada: a pasta não existe, e a sugestão é o repositório de onde ela saiu.
#[tokio::test] async fn preparar_sugere_o_repositorio_principal_da_worktree_apagada() {
    let (dir, state) = fixture();
    perfis(dir.path());
    let repo = dir.path().join("app");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let worktree = dir.path().join(".medza-worktrees").join("app").join("tarefa");
    let (_, corpo) = post(&state, "/retomada/preparar",
        json!({"provider":"claude","session_id":"s","cwd":worktree.to_string_lossy()}), None).await;
    assert_eq!(corpo["pasta_existe"], false);
    assert_eq!(corpo["pasta_principal"], repo.to_string_lossy().as_ref());
}

/// "Retomar" na Conversa com a pasta sumida: 409 com código e sugestão; com a pasta escolhida, segue.
#[tokio::test] async fn retomar_conversa_de_pasta_apagada_oferece_outra_pasta() {
    let (dir, state) = fixture();
    perfis(dir.path());
    let repo = dir.path().join("app");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let worktree = dir.path().join(".medza-worktrees").join("app").join("tarefa");
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":worktree.to_string_lossy(),"title":"Teste","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"a","external_session_id":"abc","transcript_path":null,"terminal_session_id":null,"started_at_ms":1}]
    }]})).unwrap();
    let (status, corpo) = post(&state, "/conversas/c/retomar", json!({"profile_id":null,"model":null,"effort":null}), Some("k1")).await;
    assert_eq!(status, 409, "{corpo}");
    assert_eq!(corpo["codigo"], "pasta_sumiu");
    assert_eq!(corpo["pasta_principal"], repo.to_string_lossy().as_ref());
    let (status, corpo) = post(&state, "/conversas/c/retomar",
        json!({"profile_id":null,"model":null,"effort":null,"pasta":repo.to_string_lossy()}), Some("k2")).await;
    assert_eq!(status, 202, "{corpo}");
    assert_eq!(corpo["conversation_id"], "c");
}

/// Conversa cuja aba o engine anterior deixou (sessão histórica): "Retomar" reabre a mesma sessão
/// em vez de abrir outra ao lado (duas CLIs na mesma conversa).
#[tokio::test] async fn retomar_conversa_com_aba_historica_reabre_a_mesma_sessao() {
    let (dir, state) = fixture();
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let meta: TerminalSession = serde_json::from_value(json!({"id":"t1","project_id":"p","name":"Claude","cwd":projeto.to_string_lossy(),
        "shell":"/bin/sh","state":"stopped","pid":null,"created_at_ms":1,"last_activity_at_ms":1,"output_seq":0,"rows":24,"cols":80,
        "provider":"claude","conversation_id":"c","initial_command":"echo retomada"})).unwrap();
    state.sessions.lock().unwrap().insert("t1".into(), SessionEntry::Historical(meta));
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":null,"external_session_id":null,"transcript_path":null,"terminal_session_id":"t1","started_at_ms":1}]
    }]})).unwrap();
    let (status, corpo) = post(&state, "/conversas/c/retomar", json!({"profile_id":null,"model":null,"effort":null}), Some("k")).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["reaberta"], true);
    assert_eq!(corpo["terminal_session_id"], "t1");
    assert!(matches!(state.sessions.lock().unwrap().get("t1"), Some(SessionEntry::Live(_))), "mesma sessão, agora viva");
    let _ = stop_session(&state, "t1");
}

/// A CLI não retoma: conversa nova na mesma pasta, com as últimas mensagens da antiga como contexto.
#[tokio::test] async fn continuar_em_conversa_nova_com_o_historico() {
    let (dir, state) = fixture();
    let (_, b) = perfis(dir.path());
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let cwd = projeto.to_string_lossy().into_owned();
    publicar_echo(&state, &projeto);
    transcript(&b, &cwd, "sessao-1");
    let (status, corpo) = post(&state, "/retomada/continuar-nova",
        json!({"provider":"claude","session_id":"sessao-1","cwd":cwd,"titulo":"Login"}), None).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["contexto"], true);
    let conversa = corpo["conversation_id"].as_str().unwrap().to_owned();
    let indice = omni_core::conversations::read_index(dir.path());
    let nova = indice.iter().find(|c| c.id == conversa).expect("conversa nova no índice");
    assert_eq!(nova.title, "Login (continuação)");
    assert_eq!(nova.segments[0].profile_id.as_deref(), Some("b"), "abre na conta onde a conversa estava");
    let terminal = corpo["terminal_session_id"].as_str().unwrap().to_owned();
    let _ = stop_session(&state, &terminal);

    let (status, corpo) = post(&state, "/retomada/continuar-nova",
        json!({"provider":"claude","session_id":"sessao-1","cwd":dir.path().join("sumiu").to_string_lossy()}), None).await;
    assert_eq!((status, corpo["codigo"].as_str()), (409, Some("pasta_sumiu")));
}

/// Aba cuja pasta sumiu reabre em outra pasta, com o transcript levado para lá.
#[tokio::test] async fn mudar_pasta_reabre_a_sessao_com_o_transcript_na_pasta_nova() {
    let (dir, state) = fixture();
    let (a, _) = perfis(dir.path());
    let sumiu = dir.path().join("sumiu").to_string_lossy().into_owned();
    let nova = dir.path().join("repo");
    std::fs::create_dir_all(&nova).unwrap();
    transcript(&a, &sumiu, "sessao-1");
    let meta: TerminalSession = serde_json::from_value(json!({"id":"t1","project_id":"p","name":"Claude","cwd":sumiu,
        "shell":"/bin/sh","state":"orphan","pid":null,"created_at_ms":1,"last_activity_at_ms":1,"output_seq":0,"rows":24,"cols":80,
        "provider":"claude","profile_id":"a","conversation_id":"c","external_session_id":"sessao-1","initial_command":"echo retomada",
        "env":[["CLAUDE_CONFIG_DIR", a.to_string_lossy()]]})).unwrap();
    state.sessions.lock().unwrap().insert("t1".into(), SessionEntry::Historical(meta));
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":sumiu,"title":"Teste","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"a","external_session_id":"sessao-1","transcript_path":null,"terminal_session_id":"t1","started_at_ms":1}]
    }]})).unwrap();
    let (status, corpo) = post(&state, "/retomada/mudar-pasta", json!({"terminal_session_id":"t1","cwd":nova.to_string_lossy()}), None).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["session"]["cwd"], nova.to_string_lossy().as_ref());
    assert!(omni_core::conversations::claude_transcript_path(&a, &nova.to_string_lossy(), "sessao-1").is_file(), "transcript na pasta nova");
    let indice = omni_core::conversations::read_index(dir.path());
    assert_eq!(indice[0].cwd, nova.to_string_lossy());
    let _ = stop_session(&state, "t1");
    assert_eq!(post(&state, "/retomada/mudar-pasta", json!({"terminal_session_id":"t1","cwd":"/nao/existe"}), None).await.0, 400);
}
