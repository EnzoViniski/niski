//! Fork (Copiloto abre conversa na tela, 2026-10-22): o copiloto vive no projeto especial `copiloto`,
//! que não é projeto da lateral. Bug do Enzo: "abre uma conversa na tela com chat gpt no sol 6.1
//! medio" falhava com "Projeto desconhecido; abra o app no PC…" porque a pasta do copiloto (a pasta
//! do usuário) já era de um projeto publicado e o `known_projects` deduplica por caminho: o id
//! `copiloto` sumia da lista. CLI falsa, nada real.
use super::*;

/// Pasta do usuário com um projeto publicado nela (como no PC do Enzo) e o copiloto (Codex) lá.
fn copiloto_na_casa() -> (tempfile::TempDir, Arc<EngineState>, PathBuf, PathBuf, String) {
    let (dir, state, projeto) = fixture();
    let casa = dir.path().join("casa");
    std::fs::create_dir_all(&casa).unwrap();
    let conversa = |id: &str, projeto_id: &str, cwd: &Path, provider: &str| json!({"id": id, "project_id": projeto_id,
        "cwd": cwd.to_string_lossy(), "title": id, "created_at_ms": 1, "segments": [{"provider": provider, "profile_id": null,
        "external_session_id": null, "transcript_path": null, "terminal_session_id": null, "started_at_ms": 1, "ended_at_ms": null}]});
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version": 1, "conversations": [
        conversa("orq-a", "p", &projeto, "claude"), conversa("cop", PROJETO_DO_COPILOTO, &casa, "codex"),
    ]})).unwrap();
    let cli = cli_falsa(dir.path());
    publish(&state, vec![
        PublishedProject { id: "p".into(), name: "projeto".into(), path: projeto.to_string_lossy().into_owned() },
        PublishedProject { id: "casa".into(), name: "enzo".into(), path: casa.to_string_lossy().into_owned() },
    ], vec![PublishedAgent { id: "claude".into(), label: "Claude".into(), command: cli.clone(), resume: None },
            PublishedAgent { id: "codex".into(), label: "Codex".into(), command: cli, resume: None }], None).unwrap();
    // O copiloto nasce orquestrador no spawn (`marcar_copiloto`); aqui só liga o token dele.
    marcar_copiloto(&state, PROJETO_DO_COPILOTO, Some("cop"), Some("codex"));
    let token = ligar(&state, "cop", "codex");
    (dir, state, projeto, casa, token)
}

fn sessao_de(state: &Arc<EngineState>, conversa: &str) -> TerminalSession {
    let (_, Some(sessao)) = current(state, conversa).unwrap() else { panic!("sem sessão") };
    let meta = sessao.meta.lock().unwrap().clone();
    meta
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn copiloto_abre_conversa_na_pasta_dele_com_modelo_e_esforco() {
    let (dir, state, _projeto, casa, token) = copiloto_na_casa();
    start(state.clone());
    // O pedido do Enzo, como o Codex do copiloto chamou: sem projeto, com modelo e esforço.
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "codex", "modelo": "gpt-6.1-sol",
        "esforco": "medium", "tarefa": "diga so ok", "somente_leitura": true})).await;
    assert!(!erro, "{texto}");
    assert!(!texto.contains("abra o app"), "{texto}");
    let criado: Value = serde_json::from_str(&texto).unwrap();
    let agente = criado["agente"].as_str().unwrap().to_owned();
    let meta = sessao_de(&state, &agente);
    // Modelo e esforço chegam na linha da CLI.
    assert!(meta.initial_command.as_deref().unwrap().ends_with(" -m gpt-6.1-sol -c model_reasoning_effort=medium"), "{:?}", meta.initial_command);
    // Abre na pasta do copiloto, no projeto da lateral que é dessa pasta (aparece em "Anteriores" dele).
    assert_eq!(meta.cwd, casa.to_string_lossy());
    assert_eq!(meta.project_id, "casa");
    let (indice, _) = current(&state, &agente).unwrap();
    assert_eq!(indice.project_id, "casa");
    assert_eq!(indice.segments[0].provider, "codex");
    // O subagente não vira outro copiloto orquestrador.
    assert!(!ler_registro(&state).ativo(&agente));
    let _ = dir;
    let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": agente})).await;
    assert!(!erro, "{texto}");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn copiloto_na_tela_sem_tarefa_nao_manda_briefing_e_o_fluxo_avisa_a_tela() {
    let (dir, state, _projeto, _casa, token) = copiloto_na_casa();
    start(state.clone());
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "codex", "modelo": "gpt-6.1-sol",
        "esforco": "medium", "na_tela": true})).await;
    assert!(!erro, "{texto}");
    let criado: Value = serde_json::from_str(&texto).unwrap();
    let agente = criado["agente"].as_str().unwrap().to_owned();
    assert_eq!(criado["na_tela"], true);
    let meta = sessao_de(&state, &agente);
    assert_eq!(meta.name, "Codex · gpt-6.1-sol");
    assert!(state.mobile.orquestracao.pendentes.lock().unwrap().get(&agente).is_none(), "sem tarefa, nada na fila");

    let EngineResponse::Api { status, body, .. } = local(state.clone(), "GET", "/orquestracao/cop", None, None, vec![]).await else { panic!() };
    assert_eq!(status, 200);
    let fluxo: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(fluxo["agentes"][0]["na_tela"], true, "{body}");
    assert_eq!(fluxo["agentes"][0]["modelo"], "gpt-6.1-sol");
    assert_eq!(fluxo["agentes"][0]["esforco"], "medium");

    // Nenhum briefing digitado: a conversa espera o usuário.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(std::fs::read_to_string(dir.path().join("bin").join("recebido.txt")).unwrap_or_default().is_empty());
    let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": agente})).await;
    assert!(!erro, "{texto}");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn copiloto_escolhe_projeto_da_lista_e_id_inventado_orienta_sem_abrir_o_app() {
    let (_dir, state, projeto, _casa, token) = copiloto_na_casa();
    start(state.clone());
    let (erro, texto) = ferramenta(&state, &token, "listar_projetos", json!({})).await;
    assert!(!erro, "{texto}");
    let lista: Value = serde_json::from_str(&texto).unwrap();
    assert_eq!(lista["padrao"]["id"], "casa", "{texto}");
    let ids: Vec<&str> = lista["projetos"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["p", "casa"]);

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "projeto_id": "/etc", "na_tela": true})).await;
    assert!(erro && texto.contains("listar_projetos") && texto.contains("open -a"), "{texto}");
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "projeto_id": PROJETO_DO_COPILOTO, "na_tela": true})).await;
    assert!(erro, "o projeto especial do copiloto não é escolhível: {texto}");
    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude"})).await;
    assert!(erro && texto.contains("tarefa") && texto.contains("na_tela"), "{texto}");

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "projeto_id": "p", "na_tela": true})).await;
    assert!(!erro, "{texto}");
    let agente = serde_json::from_str::<Value>(&texto).unwrap()["agente"].as_str().unwrap().to_owned();
    let meta = sessao_de(&state, &agente);
    assert_eq!((meta.project_id.as_str(), meta.cwd.as_str()), ("p", projeto.to_string_lossy().as_ref()));
    let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": agente})).await;
    assert!(!erro, "{texto}");
}

/// Copiloto numa pasta que não é de projeto nenhum (📁): abre lá mesmo, com o id estável da pasta.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn copiloto_em_pasta_sem_projeto_abre_nela() {
    let (dir, state, _projeto, _casa, token) = copiloto_na_casa();
    start(state.clone());
    let solta = dir.path().join("solta");
    std::fs::create_dir_all(&solta).unwrap();
    let mut indice = omni_core::conversations::read_index(dir.path());
    indice.iter_mut().find(|c| c.id == "cop").unwrap().cwd = solta.to_string_lossy().into_owned();
    omni_core::conversations::write_index(dir.path(), &indice).unwrap();

    let (erro, texto) = ferramenta(&state, &token, "criar_agente", json!({"plataforma": "claude", "na_tela": true})).await;
    assert!(!erro, "{texto}");
    let agente = serde_json::from_str::<Value>(&texto).unwrap()["agente"].as_str().unwrap().to_owned();
    let meta = sessao_de(&state, &agente);
    assert_eq!(meta.cwd, solta.to_string_lossy());
    assert_ne!(meta.project_id, PROJETO_DO_COPILOTO);
    assert!(!ler_registro(&state).ativo(&agente));
    let (erro, texto) = ferramenta(&state, &token, "parar_agente", json!({"agente": agente})).await;
    assert!(!erro, "{texto}");
}

/// Só o copiloto ganha `listar_projetos`, `projeto_id` e `na_tela`; o orquestrador comum não muda.
#[tokio::test]
async fn so_o_copiloto_ganha_projetos_e_na_tela() {
    let (_dir, state, _projeto, _casa, token) = copiloto_na_casa();
    let (_, lista) = enviar_mcp(&state, pedido(Some(&token), json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))).await;
    let tools = lista["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "listar_projetos"));
    let criar = tools.iter().find(|t| t["name"] == "criar_agente").unwrap();
    assert!(criar["inputSchema"]["properties"]["na_tela"].is_object() && criar["inputSchema"]["properties"]["projeto_id"].is_object());
    assert_eq!(criar["inputSchema"]["required"], json!(["plataforma"]));
    assert!(criar["description"].as_str().unwrap().contains("open -a"));

    let comum = ligar(&state, "orq-a", "claude");
    let (_, lista) = enviar_mcp(&state, pedido(Some(&comum), json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))).await;
    let tools = lista["result"]["tools"].as_array().unwrap();
    assert!(!tools.iter().any(|t| t["name"] == "listar_projetos"));
    let (erro, texto) = ferramenta(&state, &comum, "listar_projetos", json!({})).await;
    assert!(erro, "{texto}");
}
