//! Testes do terminal da conversa. Saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// Sem sessão aberta no PC não há terminal para espelhar nem para receber teclas.
#[tokio::test] async fn terminal_e_teclas_exigem_sessao_viva() {
    let (_dir,state) = fixture();
    let app = router(state);
    let resposta = app.clone().oneshot(req("GET","/conversas/c/terminal","","")).await.unwrap();
    let corpo: Value = serde_json::from_slice(&axum::body::to_bytes(resposta.into_body(),usize::MAX).await.unwrap()).unwrap();
    assert_eq!(corpo["live"],false);
    let origem = &*ORIGEM_LOCAL;
    assert_eq!(app.clone().oneshot(req("POST","/conversas/c/teclas",origem,r#"{"dados":"\u001b"}"#)).await.unwrap().status(),StatusCode::CONFLICT);
    assert_eq!(app.clone().oneshot(req("POST","/conversas/c/teclas",origem,r#"{"dados":""}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
    assert_eq!(app.oneshot(req("GET","/conversas/nao-existe/terminal","","")).await.unwrap().status(),StatusCode::NOT_FOUND);
}
