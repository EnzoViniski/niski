//! Testes do protocolo local (`local`), do base64 e da tabela de rotas. Fork: saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// `/painel` junta lista e catálogo; com o `ETag` de volta e nada mudado, responde 304 sem corpo.
/// A visão Conversa do desktop chama as rotas do celular pelo protocolo local: sem token de
/// dispositivo nem `Origin`, que lá não existem — quem chega já mostrou o token do engine.
#[tokio::test] async fn desktop_chama_a_api_pelo_protocolo_local() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().token = "token-do-celular".into();
    let EngineResponse::Api { status, body } = local(state.clone(),"GET","/conversas/c/timeline",None,None,vec![]).await else { panic!("não é Api") };
    assert_eq!(status,200);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["conversation"]["id"],"c");
    let EngineResponse::Api { status, .. } = local(state,"POST","/conversas/c/nome",Some(r#"{"nome":"Novo"}"#.into()),None,vec![]).await else { panic!("não é Api") };
    assert!(status != 401 && status != 403, "barrado como se fosse o celular: {status}");
}

#[test] fn base64_decode_confere_com_os_casos_do_rfc() {
    for (texto, esperado) in [("", ""), ("Zg==", "f"), ("Zm8=", "fo"), ("Zm9v", "foo"), ("Zm9vYg==", "foob"), ("Zm9vYmFy", "foobar")] {
        assert_eq!(base64_decode(texto).unwrap(), esperado.as_bytes(), "{texto}");
    }
    assert_eq!(base64_decode("/+8A").unwrap(), vec![0xff, 0xef, 0x00]);
    assert!(base64_decode("Z").is_none());
    assert!(base64_decode("Zm9*").is_none());
}
