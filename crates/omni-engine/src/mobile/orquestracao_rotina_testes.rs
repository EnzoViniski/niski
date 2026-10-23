//! Fork (Rotina): o agente da rotina — ferramentas MCP só da rotina e de tarefas, abrir a conversa da
//! semana com a CLI falsa (`cli_falsa`), reaproveitar a mesma e mandar a mensagem do Telegram.
use super::*;

fn rotina_conversa(dir: &Path) {
    let cwd = dir.join("rotina").join("conversa");
    std::fs::create_dir_all(&cwd).unwrap();
    atomic_write_json(&dir.join("conversations.json"), &json!({"version": 1, "conversations": [
        {"id": "rot", "project_id": rotina::PROJETO_DA_ROTINA, "cwd": cwd.to_string_lossy(), "title": "Rotina", "created_at_ms": 1,
         "segments": [{"provider": "claude", "profile_id": null, "external_session_id": null, "transcript_path": null,
            "terminal_session_id": null, "started_at_ms": 1, "ended_at_ms": null}]},
        {"id": "orq-a", "project_id": "p", "cwd": dir.join("projeto").to_string_lossy(), "title": "x", "created_at_ms": 1,
         "segments": [{"provider": "claude", "profile_id": null, "external_session_id": null, "transcript_path": null,
            "terminal_session_id": null, "started_at_ms": 1, "ended_at_ms": null}]},
    ]})).unwrap();
}

#[tokio::test]
async fn conversa_da_rotina_ve_so_ferramentas_da_rotina_e_tarefas() {
    let (dir, state, _) = fixture();
    rotina_conversa(dir.path());
    let token = ligar(&state, "rot", "claude");
    let (status, corpo) = enviar_mcp(&state, pedido(Some(&token), json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))).await;
    assert_eq!(status, StatusCode::OK);
    let nomes: Vec<&str> = corpo["result"]["tools"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
    for esperado in ["ler_dia", "planejar_semana", "ajustar_dia", "registrar_serie", "registrar_agua", "salvar_lembrete", "criar_tarefa", "listar_tarefas"] {
        assert!(nomes.contains(&esperado), "faltou {esperado}");
    }
    assert!(!nomes.contains(&"criar_agente") && !nomes.contains(&"responder_aviso") && !nomes.contains(&"salvar_ajustes"));
    let (_, inicio) = enviar_mcp(&state, pedido(Some(&token), json!({"jsonrpc": "2.0", "id": 2, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18"}}))).await;
    assert!(inicio["result"]["instructions"].as_str().unwrap().contains("rotina"));

    let (erro, texto) = ferramenta(&state, &token, "registrar_agua", json!({"copos": 2})).await;
    assert!(!erro, "{texto}");
    assert!(texto.contains("\"agua_ml\": 500"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "criar_tarefa", json!({"titulo": "Prova de Cálculo", "categoria": "faculdade",
        "data": "2026-10-29", "hora": "19:00"})).await;
    assert!(!erro, "{texto}");
    let (erro, _) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "tarefa": "x"})).await;
    assert!(erro, "a rotina não cria subagentes");
    let (erro, _) = ferramenta(&state, &token, "salvar_ajustes", json!({"plataforma": "codex"})).await;
    assert!(erro, "ajustes só pela tela");

    // Orquestradora comum continua sem as ferramentas da rotina.
    let outro = ligar(&state, "orq-a", "claude");
    let (_, corpo) = enviar_mcp(&state, pedido(Some(&outro), json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}))).await;
    let nomes: Vec<&str> = corpo["result"]["tools"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert!(nomes.contains(&"criar_agente") && !nomes.contains(&"registrar_serie"));
}

#[test]
fn semana_alvo_no_fim_de_semana_e_a_seguinte() {
    let mut a = omni_core::rotina::ajustes::Ajustes::default();
    let d = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    assert_eq!(rotina::semana_alvo(&a, d("2026-10-25"), true), d("2026-10-26"), "domingo planeja a próxima");
    assert_eq!(rotina::semana_alvo(&a, d("2026-10-25"), false), d("2026-10-19"), "conversar sem plano novo fica na atual");
    a.definir_conversa("2026-10-26", "conv-x");
    assert_eq!(rotina::semana_alvo(&a, d("2026-10-25"), false), d("2026-10-26"), "depois de planejar, segue na nova");
    assert_eq!(rotina::semana_alvo(&a, d("2026-10-21"), true), d("2026-10-19"), "quarta replaneja a atual");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn planejar_abre_conversa_da_semana_com_briefing_e_reaproveita() {
    let (dir, state, projeto) = fixture();
    // A pasta da conversa mora ao lado da pasta de dados (fora dela): apagar no fim do teste.
    struct Limpar(std::path::PathBuf);
    impl Drop for Limpar { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let _limpar = Limpar(rotina::pasta_da_conversa_em(dir.path()).parent().unwrap().to_path_buf());
    start(state.clone());
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let EngineResponse::Api { status, body, .. } = local(state.clone(), "POST", "/rotina/conversa",
        Some(r#"{"modo":"planejar"}"#.into()), None, vec![]).await else { panic!() };
    assert_eq!(status, 200, "{body}");
    let aberta: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(aberta["nova"], true);
    let conversa = aberta["conversa"].as_str().unwrap().to_owned();
    assert!(aberta["sessao"].as_str().is_some());
    let (indice, Some(sessao)) = current(&state, &conversa).unwrap() else { panic!("sem sessão") };
    assert_eq!(indice.project_id, rotina::PROJETO_DA_ROTINA);
    let cwd = sessao.meta.lock().unwrap().cwd.clone();
    assert!(!Path::new(&cwd).starts_with(dir.path()), "a CLI da rotina não roda dentro da pasta de dados (tokens)");
    assert_eq!(Path::new(&cwd).file_name().unwrap(), "Rotina");
    assert!(std::fs::read_to_string(Path::new(&cwd).join("CLAUDE.md")).unwrap().contains("Planejamento de domingo"));
    assert!(Path::new(&cwd).join("AGENTS.md").exists());
    assert!(ler_registro(&state).ativo(&conversa), "nasce com o MCP da rotina");
    let ajustes: omni_core::rotina::ajustes::Ajustes = omni_core::rotina::ler(dir.path());
    assert!(ajustes.conversas.values().any(|c| c == &conversa));

    let recebido = dir.path().join("bin").join("recebido.txt");
    let chegou = |trecho: &str| std::fs::read_to_string(&recebido).unwrap_or_default().contains(trecho);
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !chegou("Vamos planejar a semana") && std::time::Instant::now() < limite { tokio::time::sleep(std::time::Duration::from_millis(200)).await; }
    assert!(chegou("Vamos planejar a semana"), "a primeira mensagem não chegou");

    // Conversar de novo: a mesma conversa, sem abrir outra.
    let EngineResponse::Api { status, body, .. } = local(state.clone(), "POST", "/rotina/conversa",
        Some(r#"{"modo":"conversar"}"#.into()), None, vec![]).await else { panic!() };
    assert_eq!(status, 200, "{body}");
    let de_novo: Value = serde_json::from_str(&body).unwrap();
    // No fim de semana "conversar" segue na conversa planejada; no meio da semana, planejar = semana atual.
    assert_eq!(de_novo["nova"], false);
    assert_eq!(de_novo["conversa"], conversa);
    let EngineResponse::Api { body, .. } = local(state.clone(), "GET", "/rotina/conversa", None, None, vec![]).await else { panic!() };
    assert!(body.contains(&conversa));

    // Mensagem pelo Telegram chega à mesma conversa com a marca de origem.
    let (s, c) = (state.clone(), conversa.clone());
    let resposta = tokio::task::spawn_blocking(move || rotina::conversar(&s, "Cancelaram a aula de hoje", std::time::Duration::from_secs(8))).await.unwrap();
    assert!(resposta.is_ok(), "{resposta:?}");
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !chegou("[Pelo Telegram] Cancelaram a aula de hoje") && std::time::Instant::now() < limite { tokio::time::sleep(std::time::Duration::from_millis(200)).await; }
    assert!(chegou("[Pelo Telegram] Cancelaram a aula de hoje"));
    let _ = c;
    let EngineResponse::Api { status, .. } = local(state.clone(), "POST", "/rotina/conversa", Some(r#"{"modo":"outro"}"#.into()), None, vec![]).await else { panic!() };
    assert_eq!(status, 400);
}
