//! Testes da lista, do painel, da timeline e do catálogo publicado. Fork: saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

#[tokio::test] async fn painel_junta_lista_e_catalogo_e_responde_304_sem_mudanca() {
    let (_dir,state) = fixture();
    let app = router(state);
    let primeira = app.clone().oneshot(req("GET","/painel","","")).await.unwrap();
    assert_eq!(primeira.status(),StatusCode::OK);
    let etag = primeira.headers().get("etag").unwrap().to_str().unwrap().to_owned();
    let corpo: Value = serde_json::from_slice(&axum::body::to_bytes(primeira.into_body(),usize::MAX).await.unwrap()).unwrap();
    assert_eq!(corpo["conversations"][0]["id"],"c");
    assert!(corpo["catalog"]["projects"].is_array());

    let mut repetida = req("GET","/painel","","");
    repetida.headers_mut().insert("if-none-match",etag.parse().unwrap());
    let resposta = app.oneshot(repetida).await.unwrap();
    assert_eq!(resposta.status(),StatusCode::NOT_MODIFIED);
    assert!(axum::body::to_bytes(resposta.into_body(),usize::MAX).await.unwrap().is_empty());
}

/// A timeline traz o estado da própria conversa, para o chat não precisar recarregar a lista.
#[tokio::test] async fn timeline_traz_o_estado_da_conversa() {
    let (_dir,state) = fixture();
    let resposta = router(state).oneshot(req("GET","/conversas/c/timeline","","")).await.unwrap();
    assert_eq!(resposta.status(),StatusCode::OK);
    let corpo: Value = serde_json::from_slice(&axum::body::to_bytes(resposta.into_body(),usize::MAX).await.unwrap()).unwrap();
    assert_eq!(corpo["conversation"]["id"],"c");
    assert_eq!(corpo["conversation"]["live"],false);
    assert!(corpo["timeline"]["messages"].as_array().unwrap().is_empty());
}

/// A união com o `conversations.json` é o que mantém o celular útil quando o desktop nunca
/// publicou ou está fechado há muito tempo. Sem ela, um app recém-instalado mostraria zero
/// projetos até alguém abrir a janela no PC.
#[tokio::test] async fn projetos_publicados_se_unem_aos_das_conversas() {
    let (_dir,state) = fixture();
    publish(&state,
        vec![PublishedProject{id:"publicado".into(),name:"Publicado".into(),path:"C:/pub".into()}],
        vec![PublishedAgent{id:"claude".into(),label:"Claude".into(),command:"claude".into(),resume:Some("--continue".into())}],
        None,
    ).unwrap();

    let response = router(state).oneshot(req("GET","/projetos","","")).await.unwrap();
    assert_eq!(response.status(),StatusCode::OK);
    let data:Value = serde_json::from_slice(&to_bytes(response.into_body(),1_000_000).await.unwrap()).unwrap();

    let ids:Vec<_> = data["projects"].as_array().unwrap().iter().map(|p|p["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"publicado"));
    assert!(ids.contains(&"p"),"o projeto da conversa do fixture tem de entrar: {ids:?}");
    assert!(data["published_at_ms"].as_u64().unwrap() > 0);
    assert_eq!(data["agents"][0]["resume"],"--continue");
}

#[tokio::test] async fn historico_nativo_entra_na_lista_mobile_e_abre_a_timeline() {
    let (dir,state) = fixture();
    let profile_dir = dir.path().join("claude-history");
    let transcript = profile_dir.join("projects").join("test").join("old-session.jsonl");
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    std::fs::write(&transcript,[
        json!({"type":"user","cwd":"C:/test","timestamp":"2026-09-20T10:00:00Z",
            "message":{"role":"user","content":"Encontre esta conversa antiga"}}).to_string(),
        json!({"type":"assistant","timestamp":"2026-09-20T10:01:00Z",
            "message":{"role":"assistant","content":"Histórico encontrado"}}).to_string(),
    ].join("\n")).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[{
        "id":"claude-history","provider":"claude","name":"Pessoal","config_dir":profile_dir,
        "builtin":false,"created_at_ms":0
    }]})).unwrap();

    let list = summaries(&state);
    let historical = list.iter().find(|item|item["historical"] == true)
        .expect("o histórico do CLI precisa aparecer junto das conversas do OMNI");
    assert_eq!(historical["title"],"Encontre esta conversa antiga");
    assert_eq!(historical["project_id"],"p","a pasta conhecida deve reutilizar o projeto existente");
    assert!(historical["capabilities"].is_null(),"histórico sem PTY é somente leitura");

    let id = historical["id"].as_str().unwrap();
    let response = router(state).oneshot(req("GET",&format!("/conversas/{id}/timeline"),"","")).await.unwrap();
    assert_eq!(response.status(),StatusCode::OK);
    let data:Value = serde_json::from_slice(&to_bytes(response.into_body(),1_000_000).await.unwrap()).unwrap();
    assert_eq!(data["timeline"]["messages"][0]["text"],"Encontre esta conversa antiga");
    assert_eq!(data["timeline"]["messages"][1]["text"],"Histórico encontrado");
}

#[test] fn publicacao_sobrevive_ao_reinicio_do_engine() {
    let (dir,state) = fixture();
    publish(&state,vec![PublishedProject{id:"p1".into(),name:"Um".into(),path:"C:/um".into()}],vec![],None).unwrap();
    // Um `MobileRuntime` novo é o que o engine monta ao subir de novo.
    let renascido = MobileRuntime::new(dir.path());
    assert_eq!(renascido.workspace.lock().unwrap().projects[0].id,"p1");
}

#[test] fn tema_publicado_vai_para_o_celular_e_desktop_antigo_nao_apaga() {
    let (_dir,state) = fixture();
    let tema = PublishedTheme{accent:"roxo".into(),theme:"claro".into(),background:"pixel-grid".into(),glass:"forte".into()};
    publish(&state,vec![],vec![],Some(tema.clone())).unwrap();
    publish(&state,vec![],vec![],None).unwrap();
    assert_eq!(state.mobile.workspace.lock().unwrap().theme.as_ref(),Some(&tema));
}
