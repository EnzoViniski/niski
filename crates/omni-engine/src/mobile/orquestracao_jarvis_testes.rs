//! Fork: ferramentas reais do Jarvis com estado isolado e CLI falsa.
use super::*;

#[tokio::test]
async fn chave_do_jarvis_so_sai_pelo_ipc_desktop_e_e_revogavel() {
    let (_dir, state, _) = fixture();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/orquestracao/jarvis")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resposta = rotas()
        .with_state(state.clone())
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(resposta.status(), StatusCode::FORBIDDEN);
    let Json(primeira) =
        jarvis::abrir(State(state.clone()), Some(axum::Extension(jarvis::Desktop)))
            .await
            .unwrap();
    let Json(segunda) = jarvis::abrir(State(state.clone()), Some(axum::Extension(jarvis::Desktop)))
        .await
        .unwrap();
    let a = primeira["token"].as_str().unwrap();
    let b = segunda["token"].as_str().unwrap();
    assert_ne!(a, b);
    let (status, _) = enviar_mcp(
        &state,
        pedido(Some(a), json!({"id":1,"method":"tools/list"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, lista) = enviar_mcp(
        &state,
        pedido(Some(b), json!({"id":1,"method":"tools/list"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tools = lista["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "listar_projetos"));
    assert!(
        tools.iter().find(|t| t["name"] == "criar_agente").unwrap()["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("projeto_id"))
    );
    let _ = jarvis::fechar(State(state.clone()), Some(axum::Extension(jarvis::Desktop)))
        .await
        .unwrap();
    assert_eq!(
        enviar_mcp(
            &state,
            pedido(Some(b), json!({"id":1,"method":"tools/list"}))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert!(!omni_core::conversations::read_index(dir(&state))
        .iter()
        .any(|c| c.id == jarvis::DONO));
}

#[tokio::test]
async fn jarvis_consulta_app_mas_so_comanda_seus_agentes() {
    let (dir, state, projeto) = fixture();
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let Json(acesso) = jarvis::abrir(State(state.clone()), Some(axum::Extension(jarvis::Desktop)))
        .await
        .unwrap();
    let token = acesso["token"].as_str().unwrap();
    let (erro, projetos) = ferramenta(&state, token, "listar_projetos", json!({})).await;
    assert!(!erro && projetos.contains("projeto"));
    let (erro, conversas) = ferramenta(&state, token, "listar_conversas", json!({})).await;
    assert!(!erro && conversas.contains("orq-a"));
    let (erro, _) = ferramenta(
        &state,
        token,
        "consultar_conversa",
        json!({"conversa":"orq-a"}),
    )
    .await;
    assert!(!erro);
    for nome in ["mandar_prompt", "responder_pedido", "parar_agente"] {
        let (erro, _) = ferramenta(
            &state,
            token,
            nome,
            json!({"agente":"orq-a","texto":"oi","permitir":true}),
        )
        .await;
        assert!(erro, "{nome} não deve comandar conversa alheia");
    }
    for projeto in [None, Some("inexistente")] {
        let mut args = json!({"plataforma":"claude","tarefa":"analise","somente_leitura":true});
        if let Some(p) = projeto {
            args["projeto_id"] = json!(p);
        }
        assert!(ferramenta(&state, token, "criar_agente", args).await.0);
    }
    assert!(state.sessions.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn jarvis_abre_agentes_paralelos_reais_no_projeto_escolhido_com_cli_falsa() {
    let (dir, state, projeto) = fixture();
    start(state.clone());
    publicar(&state, &projeto, &cli_falsa(dir.path()));
    let Json(acesso) = jarvis::abrir(State(state.clone()), Some(axum::Extension(jarvis::Desktop)))
        .await
        .unwrap();
    let token = acesso["token"].as_str().unwrap();
    let criar = |tarefa| {
        ferramenta(
            &state,
            token,
            "criar_agente",
            json!({"plataforma":"claude","tarefa":tarefa,"projeto_id":"p","somente_leitura":true}),
        )
    };
    let (a, b) = tokio::join!(criar("inspecione documentação"), criar("inspecione testes"));
    for (erro, retorno) in [a, b] {
        assert!(!erro, "{retorno}");
        let criado: Value = serde_json::from_str(&retorno).unwrap();
        let id = criado["agente"].as_str().unwrap();
        let (conversa, Some(_)) = current(&state, id).unwrap() else {
            panic!("subagente sem sessão")
        };
        assert_eq!(conversa.project_id, "p");
        assert_eq!(Path::new(&conversa.cwd), projeto);
        assert!(
            !ferramenta(&state, token, "parar_agente", json!({"agente":id}))
                .await
                .0
        );
    }
}

#[cfg(unix)]
#[test]
fn pasta_canonica_distingue_projetos_e_identifica_symlink() {
    let pasta = tempfile::tempdir().unwrap();
    let a = pasta.path().join("a");
    let b = pasta.path().join("b");
    let link = pasta.path().join("link");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    std::os::unix::fs::symlink(&a, &link).unwrap();
    assert!(!jarvis::mesma_pasta(
        a.to_str().unwrap(),
        b.to_str().unwrap()
    ));
    assert!(jarvis::mesma_pasta(
        a.to_str().unwrap(),
        link.to_str().unwrap()
    ));
}
