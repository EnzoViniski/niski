//! Testes da fila de ações. Fork: saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// O modo é `plan` ou `auto` e nada mais; sem sessão viva a fila recusa, como qualquer ação.
#[tokio::test] async fn modo_aceita_so_plan_e_auto() {
    let (_dir,state) = fixture();
    let app = router(state);
    assert_eq!(app.clone().oneshot(req("POST","/conversas/c/modo",&*ORIGEM_LOCAL,r#"{"modo":"bypassPermissions"}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
    assert_eq!(app.clone().oneshot(req("POST","/conversas/c/modo",&*ORIGEM_LOCAL,r#"{"modo":"plan"}"#)).await.unwrap().status(),StatusCode::CONFLICT);
    assert_eq!(app.oneshot(req("POST","/conversas/c/modo",&*ORIGEM_LOCAL,r#"{"modo":"auto"}"#)).await.unwrap().status(),StatusCode::CONFLICT);
}

#[test] fn idempotent_retry_does_not_need_a_live_session_or_enqueue_again() {
    let (_dir,state) = fixture();
    state.mobile.actions.lock().unwrap().push_back(Action { id:"a".into(),conversation_id:"c".into(),session_id:"gone".into(),state:"sent".into(),created_at_ms:now_ms(),error:None,key:"test-key".into(),revision:"old".into(),kind:ActionKind::Prompt("oi".into()) });
    let headers = req("POST","/conversas/c/prompt",&*ORIGEM_LOCAL,"").headers().clone();
    assert_eq!(enqueue(&state,"c",&headers,ActionKind::Prompt("oi".into())).unwrap().0,StatusCode::ACCEPTED);
    assert_eq!(state.mobile.actions.lock().unwrap().len(),1);
    assert_eq!(enqueue(&state,"c",&headers,ActionKind::Prompt("outro".into())).unwrap_err().0,StatusCode::CONFLICT);
    let mut action = state.mobile.actions.lock().unwrap()[0].clone();
    action.created_at_ms = now_ms().saturating_sub(60_001);
    assert!(execute(&state,&action).unwrap_err().to_string().contains("expirada"));
}
