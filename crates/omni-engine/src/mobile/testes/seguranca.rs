//! Testes de Host/Origin, token de dispositivo e caminhos públicos. Saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// A página de computador é servida em `/pc`, com a mesma casca pública do celular — os dados
/// continuam atrás do código de acesso.
#[tokio::test] async fn pagina_de_computador_abre_sem_codigo_mas_os_dados_nao() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().token = "segredo".into();
    let app = router(state);
    let pagina = app.clone().oneshot(req("GET","/pc","","")).await.unwrap();
    assert_eq!(pagina.status(),StatusCode::OK,"/pc precisa servir a página de computador");
    assert_eq!(pagina.headers()["content-type"],"text/html; charset=utf-8");
    let corpo = to_bytes(pagina.into_body(),64*1024).await.unwrap();
    assert!(String::from_utf8_lossy(&corpo).contains("/src/web/main.tsx") || String::from_utf8_lossy(&corpo).contains("assets/pc"),
        "a página servida em /pc tem de ser a do computador, não a do celular");
    assert_eq!(app.oneshot(req("GET","/conversas","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
}

#[tokio::test] async fn concurrent_readers_and_no_privileged_http_surface() {
    let (_dir,state) = fixture();
    let app = router(state.clone());
    let (a,b) = tokio::join!(app.clone().oneshot(req("GET","/conversas","","")),app.clone().oneshot(req("GET","/atencao","","")));
    assert_eq!(a.unwrap().status(),StatusCode::OK); assert_eq!(b.unwrap().status(),StatusCode::OK);
    let response = app.clone().oneshot(req("GET","/conversas/c/timeline","","")).await.unwrap();
    let data:Value = serde_json::from_slice(&to_bytes(response.into_body(),1_000_000).await.unwrap()).unwrap();
    assert_eq!(data["timeline"]["messages"],json!([]));
    let response = app.clone().oneshot(req("POST","/conversas/c/prompt",&*ORIGEM_LOCAL,r#"{"texto":"oi"}"#)).await.unwrap();
    assert_eq!(response.status(),StatusCode::CONFLICT);
    assert!(state.sessions.lock().unwrap().is_empty(),"HTTP must not spawn a terminal");
    assert_ne!(app.oneshot(req("POST","/spawn_terminal",&*ORIGEM_LOCAL,"{}")).await.unwrap().status(),StatusCode::OK);
}
#[tokio::test] async fn foreign_origin_unknown_conversation_and_invalid_prompt_are_rejected() {
    let (_dir,state) = fixture(); let app = router(state);
    assert_eq!(app.clone().oneshot(req("POST","/conversas/c/prompt","https://evil.example",r#"{"texto":"oi"}"#)).await.unwrap().status(),StatusCode::FORBIDDEN);
    assert_eq!(app.clone().oneshot(req("GET","/conversas/missing/timeline","","")).await.unwrap().status(),StatusCode::NOT_FOUND);
    assert_eq!(app.oneshot(req("POST","/conversas/c/prompt",&*ORIGEM_LOCAL,r#"{"texto":"/exit"}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
}

/// O bundle tem de carregar antes de o celular ter token; a API, não. Se esta asserção inverter,
/// ou o celular não consegue nem abrir a página, ou qualquer um na tailnet lê as conversas.
#[tokio::test] async fn token_de_dispositivo_protege_a_api_e_libera_o_bundle() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().token = "segredo".into();
    let app = router(state);
    let com_token = |valor:&str| {
        Request::builder().method("GET").uri("/conversas").header("host",&*HOST_LOCAL)
            .header("x-omni-token",valor).body(Body::empty()).unwrap()
    };
    assert_eq!(app.clone().oneshot(req("GET","/conversas","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
    assert_eq!(app.clone().oneshot(com_token("segred")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
    assert_eq!(app.clone().oneshot(com_token("segredx")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
    assert_eq!(app.clone().oneshot(com_token("segredo")).await.unwrap().status(),StatusCode::OK);
    assert_ne!(app.oneshot(req("GET","/","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
}

/// Com o Serve ligado o `Host` que chega é o nome MagicDNS, não o bind. Aceitar esse nome não
/// pode virar "aceitar qualquer Host": a checagem existe para barrar CSRF e DNS rebinding.
#[tokio::test] async fn serve_aceita_o_nome_magicdns_e_mais_nenhum() {
    let (_dir,state) = fixture();
    *state.mobile.config.lock().unwrap() =
        MobileConfig{enabled:true,bind:HOST_LOCAL.clone(),token:TOKEN_DE_TESTE.into(),serve:true,totp_secret:String::new(),totp_confirmed:false};
    *state.mobile.magic_dns.lock().unwrap() = Some("pc.tail0000.ts.net".into());
    let app = router(state);
    let get = |host:&str| Request::builder().method("GET").uri("/conversas")
        .header("host",host).header("x-omni-token",TOKEN_DE_TESTE).body(Body::empty()).unwrap();

    assert_eq!(app.clone().oneshot(get("pc.tail0000.ts.net")).await.unwrap().status(),StatusCode::OK);
    assert_eq!(app.clone().oneshot(get(&*HOST_LOCAL)).await.unwrap().status(),StatusCode::OK);
    assert_eq!(app.clone().oneshot(get("evil.example")).await.unwrap().status(),StatusCode::FORBIDDEN);
    // Sufixo parecido não basta: a comparação é igualdade, não "termina com".
    assert_eq!(app.oneshot(get("mal.pc.tail0000.ts.net")).await.unwrap().status(),StatusCode::FORBIDDEN);
}

/// O celular busca ícone e manifest sem o código de acesso. Se caírem no 401, o app fica sem ícone.
#[tokio::test] async fn icone_e_manifest_carregam_sem_codigo_mas_a_api_nao() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().token = "segredo".into();
    let app = router(state);
    for publico in ["/manifest.webmanifest","/apple-touch-icon.png","/icon-192.png","/icon-512.png","/favicon.png"] {
        assert_ne!(app.clone().oneshot(req("GET",publico,"","")).await.unwrap().status(),StatusCode::UNAUTHORIZED,
            "{publico} precisa carregar sem código");
    }
    // Lista fechada: nome parecido não passa, e a API segue protegida.
    assert_eq!(app.clone().oneshot(req("GET","/icon-192.png/../conversas","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
    assert_eq!(app.oneshot(req("GET","/conversas","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
}

#[test] fn tipos_de_arquivo_que_o_celular_exige() {
    assert_eq!(tipo_do_arquivo("apple-touch-icon.png"),"image/png");
    assert_eq!(tipo_do_arquivo("manifest.webmanifest"),"application/manifest+json");
    assert_eq!(tipo_do_arquivo("assets/index-abc.js"),"text/javascript; charset=utf-8");
}

/// Fork (segurança): `mobile.json` ligado e sem token (versão antiga, restaurado) abria a API inteira.
#[tokio::test] async fn sem_token_a_api_recusa_tudo() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().token.clear();
    let app = router(state);
    let sem_cabecalho = Request::builder().method("GET").uri("/conversas").header("host",&*HOST_LOCAL).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(sem_cabecalho).await.unwrap().status(),StatusCode::UNAUTHORIZED);
    let vazio = Request::builder().method("GET").uri("/terminais").header("host",&*HOST_LOCAL).header("x-omni-token","").body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(vazio).await.unwrap().status(),StatusCode::UNAUTHORIZED);
    assert_ne!(app.oneshot(req("GET","/","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED,"a casca continua pública");
}
