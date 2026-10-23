//! Fork (Tarefas no celular): o celular lê e grava os mesmos arquivos que o app do PC.
use super::*;

async fn api(state: &Arc<EngineState>, metodo: &str, caminho: &str, corpo: Option<Value>) -> (u16, Value) {
    let EngineResponse::Api { status, body, .. } = local(state.clone(), metodo, caminho, corpo.map(|c| c.to_string()), None, vec![]).await
        else { panic!("resposta inesperada") };
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[tokio::test] async fn celular_le_e_grava_as_tarefas_da_pasta_do_pc() {
    let (dir, state) = fixture();
    let (status, vazio) = api(&state, "GET", "/tarefas/listar/tarefas", None).await;
    assert_eq!((status, vazio), (200, json!({"itens": []})));

    // O PC escreveu o espelho com o "eu"; sem pasta compartilhada, vale `<dados>/tarefas`.
    let ajustes = json!({"eu": {"id": "enzo", "nome": "Enzo", "cor": "#fff", "atualizadaEm": ""}, "pastaCompartilhada": ""}).to_string();
    atomic_write_json(&dir.path().join("estado-da-interface.json"), &json!({"chaves": {"medza-tarefas-ajustes": ajustes}})).unwrap();
    let (_, contexto) = api(&state, "GET", "/tarefas/contexto", None).await;
    assert_eq!((contexto["eu"]["id"].as_str(), contexto["compartilhada"].as_bool()), (Some("enzo"), Some(false)));

    let tarefa = json!({"id": "t-1", "titulo": "Comprar café"}).to_string();
    let (status, _) = api(&state, "POST", "/tarefas/gravar", Some(json!({"tipo": "tarefas", "id": "t-1", "conteudo": tarefa}))).await;
    assert_eq!(status, 200);
    assert!(dir.path().join("tarefas/tarefas/t-1.json").is_file(), "mesma pasta que o app do PC usa");
    let (_, lidas) = api(&state, "GET", "/tarefas/listar/tarefas", None).await;
    assert_eq!(lidas["itens"], json!([tarefa]));

    // Mesmas recusas do armazenamento: tipo, id e conteúdo.
    for corpo in [json!({"tipo": "../x", "id": "a", "conteudo": "{}"}), json!({"tipo": "tarefas", "id": "../a", "conteudo": "{}"}),
        json!({"tipo": "tarefas", "id": "a", "conteudo": "não é json"})] {
        let (status, _) = api(&state, "POST", "/tarefas/gravar", Some(corpo.clone())).await;
        assert_eq!(status, 400, "{corpo}");
    }
    let (status, _) = api(&state, "GET", "/tarefas/listar/segredos", None).await;
    assert_eq!(status, 400);
}
