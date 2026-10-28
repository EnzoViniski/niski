//! Testes das rotas da aba Extensões. Saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// Estrela pela rota: a conta marcada vira a fonte, os MCPs dela chegam na outra na hora e a
/// lista da aba devolve a marca. Contas de mentira numa pasta temporária.
#[tokio::test] async fn estrela_pela_rota_leva_os_mcps_da_conta_marcada() {
    let (dir,state) = fixture();
    let (a,b) = (dir.path().join("a"),dir.path().join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(a.join(".claude.json"),json!({"mcpServers":{"so_da_a":{"type":"http","url":"https://a"}}}).to_string()).unwrap();
    std::fs::write(b.join(".claude.json"),json!({"mcpServers":{"so_da_b":{"type":"http","url":"https://b"}},"oauthAccount":{"x":1}}).to_string()).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[
        {"id":"a","provider":"claude","name":"Padrão","config_dir":a.to_string_lossy(),"builtin":false,"created_at_ms":0},
        {"id":"b","provider":"claude","name":"EmailUber","config_dir":b.to_string_lossy(),"builtin":false,"created_at_ms":0},
    ]})).unwrap();

    let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/extensoes/principal",Some(r#"{"provider":"claude","conta":"b"}"#.into()),None,vec![]).await else { panic!("não é Api") };
    assert_eq!(status,200);
    let em_a: Value = serde_json::from_slice(&std::fs::read(a.join(".claude.json")).unwrap()).unwrap();
    assert!(em_a["mcpServers"]["so_da_b"].is_object(),"a conta sem estrela recebeu o MCP da marcada");
    assert!(em_a["mcpServers"]["so_da_a"].is_object(),"e não perdeu o seu");
    let em_b: Value = serde_json::from_slice(&std::fs::read(b.join(".claude.json")).unwrap()).unwrap();
    assert!(em_b["mcpServers"].get("so_da_a").is_none(),"a principal não recebe das outras");

    let EngineResponse::Api { body, .. } = local(state.clone(),"GET","/extensoes",None,None,vec![]).await else { panic!("não é Api") };
    let contas = serde_json::from_str::<Value>(&body).unwrap()["contas"].as_array().unwrap().clone();
    let marca = |id: &str| contas.iter().find(|c| c["conta_id"] == id).unwrap()["principal"].clone();
    assert_eq!((marca("a"),marca("b")),(json!(false),json!(true)));

    let EngineResponse::Api { status, .. } = local(state,"POST","/extensoes/principal",Some(r#"{"provider":"claude","conta":"nao-existe"}"#.into()),None,vec![]).await else { panic!("não é Api") };
    assert_eq!(status,404);
}
