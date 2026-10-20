use super::*;

#[tokio::test]
async fn memoria_persiste_rele_desativa_e_rejeita_texto_invalido_pela_api_local() {
    let (dir, state) = fixture();
    let EngineResponse::Api { status, body } = local(state.clone(), "GET", "/memoria", None, None, vec![]).await else { panic!() };
    assert_eq!(status, 200);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), json!({"ativa":false,"texto":""}));
    for ativa in [true, false] {
        let texto = json!({"ativa":ativa,"texto":"Perfil sintético. Respostas curtas."}).to_string();
        let EngineResponse::Api { status, .. } = local(state.clone(), "POST", "/memoria", Some(texto), None, vec![]).await else { panic!() };
        assert_eq!(status, 200);
        let memoria = omni_core::memoria_compartilhada::ler(dir.path()).unwrap();
        assert_eq!(memoria.ativa, ativa);
        assert_eq!(crate::memoria_compartilhada::texto(&state).is_some(), ativa);
    }
    let EngineResponse::Api { status, .. } = local(state.clone(), "POST", "/memoria", Some(json!({"ativa":true,"texto":"x".repeat(12001)}).to_string()), None, vec![]).await else { panic!() };
    assert_eq!(status, 400);
    assert!(!omni_core::memoria_compartilhada::ler(dir.path()).unwrap().ativa);
}
