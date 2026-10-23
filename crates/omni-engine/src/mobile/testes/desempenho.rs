//! Fork (desempenho, 2026-10-22): resumo da conversa com ETag e o ETag atravessando o IPC do desktop.
use super::*;

#[tokio::test] async fn resumo_traz_so_a_conversa_e_responde_304_sem_mudanca() {
    let (_dir,state) = fixture();
    let app = router(state);
    let primeira = app.clone().oneshot(req("GET","/conversas/c/resumo","","")).await.unwrap();
    assert_eq!(primeira.status(),StatusCode::OK);
    let etag = primeira.headers().get("etag").unwrap().to_str().unwrap().to_owned();
    let corpo: Value = serde_json::from_slice(&axum::body::to_bytes(primeira.into_body(),usize::MAX).await.unwrap()).unwrap();
    assert_eq!(corpo["id"],"c");
    assert_eq!(corpo["live"],false);
    assert!(corpo.get("timeline").is_none(),"só o resumo, sem as mensagens");

    let mut repetida = req("GET","/conversas/c/resumo","","");
    repetida.headers_mut().insert("if-none-match",etag.parse().unwrap());
    assert_eq!(app.clone().oneshot(repetida).await.unwrap().status(),StatusCode::NOT_MODIFIED);
    assert_eq!(app.oneshot(req("GET","/conversas/nao-existe/resumo","","")).await.unwrap().status(),StatusCode::NOT_FOUND);
}

#[tokio::test] async fn etag_atravessa_o_protocolo_local_e_volta_304() {
    let (_dir,state) = fixture();
    let EngineResponse::Api { status, body, etag } = local(state.clone(),"GET","/conversas/c/timeline",None,None,vec![]).await else { panic!() };
    assert_eq!(status,200);
    assert!(!body.is_empty());
    let etag = etag.expect("o ETag da rota tem de chegar ao desktop");
    let EngineResponse::Api { status, body, .. } = local(state.clone(),"GET","/conversas/c/timeline",None,None,
        vec![("If-None-Match".into(),etag.clone())]).await else { panic!() };
    assert_eq!(status,304);
    assert!(body.is_empty());
    // Rota sem ETag segue sem ele (e o JSON do protocolo nem leva o campo).
    let resposta = local(state,"GET","/contas",None,None,vec![]).await;
    let EngineResponse::Api { etag: None, .. } = &resposta else { panic!("rota sem ETag: {resposta:?}") };
    assert!(!serde_json::to_string(&resposta).unwrap().contains("etag"));
}
