//! Fork (Roadmaps de estudo): o celular lê e grava os mesmos arquivos que o app.
use super::*;

async fn api(state: &Arc<EngineState>, metodo: &str, caminho: &str, corpo: Option<Value>) -> (u16, Value) {
    let EngineResponse::Api { status, body } = local(state.clone(), metodo, caminho, corpo.map(|c| c.to_string()), None, vec![]).await
        else { panic!("resposta inesperada") };
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn celular_le_e_grava_o_roadmap_do_app() {
    let (dir, state) = fixture();
    let (status, vazio) = api(&state, "GET", "/roadmaps", None).await;
    assert_eq!(status, 200);
    assert_eq!(vazio, json!([]));

    // O app criou um roadmap (mesma pasta de dados).
    let raiz = dir.path().join(omni_core::roadmaps::PASTA);
    let id = omni_core::roadmaps::criar(&raiz, &json!({"versao":1,"nome":"Vendas"}), 1, 2).unwrap();
    let (_, lista) = api(&state, "GET", "/roadmaps", None).await;
    assert_eq!(lista[0]["id"], id.as_str());
    assert_eq!(lista[0]["meta"]["nome"], "Vendas");

    // Cards grandes passam do limite global de 32 KB: a rota tem o dela.
    let cards = json!({"versao":1,"cards":[],"revisoes":[{"cardId":"x","nota":3,"em":"2026-10-21T10:00:00Z"}],"folga":"ç".repeat(40_000)}).to_string();
    let (status, _) = api(&state, "POST", &format!("/roadmaps/{id}/arquivo"), Some(json!({"nome":"cards.json","conteudo":cards}))).await;
    assert_eq!(status, 200);
    let (status, lido) = api(&state, "GET", &format!("/roadmaps/{id}/arquivo?nome=cards.json"), None).await;
    assert_eq!(status, 200);
    assert_eq!(lido["conteudo"].as_str().unwrap(), cards);
    let (_, nada) = api(&state, "GET", &format!("/roadmaps/{id}/arquivo?nome=caderno.md"), None).await;
    assert_eq!(nada["conteudo"], Value::Null);

    let (status, _) = api(&state, "POST", &format!("/roadmaps/{id}/conversa"), Some(json!({"linha": "{\"de\":\"eu\",\"texto\":\"oi\"}"}))).await;
    assert_eq!(status, 200);
    assert!(std::fs::read_to_string(raiz.join(&id).join("conversa.jsonl")).unwrap().contains("\"oi\""));

    // Mesmas recusas do app.
    for (nome, conteudo) in [("../sessions.json", "{}"), ("engine.token", "x"), ("plano.json", "{ quebrado")] {
        let (status, _) = api(&state, "POST", &format!("/roadmaps/{id}/arquivo"), Some(json!({"nome":nome,"conteudo":conteudo}))).await;
        assert_eq!(status, 400, "{nome}");
    }
    let (status, _) = api(&state, "GET", "/roadmaps/..%2F..%2Fx/arquivo?nome=meta.json", None).await;
    assert_eq!(status, 400);
}
