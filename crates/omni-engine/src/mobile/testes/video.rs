//! Fork (vídeo na conversa): o vídeo citado na resposta do agente toca no celular em pedaços. Transcript
//! e vídeo sintéticos; nenhuma CLI real.
use super::*;

async fn api(state: &Arc<EngineState>, metodo: &str, caminho: &str) -> (u16, Value) {
    let EngineResponse::Api { status, body, .. } = local(state.clone(), metodo, caminho, None, None, vec![]).await
        else { panic!("resposta inesperada") };
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[test] fn intervalos_do_range() {
    assert_eq!(video_rota::intervalo("bytes=0-1", 10), Some((0, 1)));
    assert_eq!(video_rota::intervalo("bytes=4-", 10), Some((4, 9)));
    assert_eq!(video_rota::intervalo("bytes=-3", 10), Some((7, 9)));
    assert_eq!(video_rota::intervalo("bytes=2-99", 10), Some((2, 9)));
    for ruim in ["bytes=10-", "bytes=5-2", "items=0-1", "bytes=0-1,4-5", "bytes=x-1"] {
        assert_eq!(video_rota::intervalo(ruim, 10), None, "{ruim}");
    }
    assert_eq!(video_rota::intervalo("bytes=0-", 0), None);
}

#[tokio::test] async fn video_da_resposta_toca_por_bilhete_e_com_range() {
    let (dir, state) = fixture();
    state.mobile.config.lock().unwrap().token = "segredo".into();
    let perfil = dir.path().join("perfil-claude");
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(projeto.join("saida")).unwrap();
    let video = projeto.join("saida/final.mp4");
    let bytes: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&video, &bytes).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles":[{"id":"p","provider":"claude","name":"Fixture",
        "config_dir": perfil, "builtin": false, "created_at_ms": 0}]})).unwrap();
    let cwd = projeto.to_string_lossy().into_owned();
    let transcript = omni_core::conversations::claude_transcript_path(&perfil, &cwd, "ext");
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    std::fs::write(&transcript, [
        json!({"type":"user","uuid":"u1","message":{"role":"user","content":"faz o vídeo"}}),
        json!({"type":"assistant","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":format!("Pronto: `{}`", video.display())}]}}),
    ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":cwd,"title":"Vídeo","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"p","external_session_id":"ext","transcript_path":transcript,"started_at_ms":1}]
    }]})).unwrap();

    let (_, resposta) = api(&state, "GET", "/conversas/c/timeline").await;
    let n = resposta["timeline"]["messages"].as_array().unwrap().iter().position(|m| m["arquivos"].as_array().is_some_and(|a| !a.is_empty()))
        .expect("a resposta mostra o vídeo como arquivo");
    let (status, nada) = api(&state, "POST", &format!("/conversas/c/videos/{}/0", n - 1)).await;
    assert_eq!(status, 404, "mensagem sem vídeo: {nada}");
    let (status, bilhete) = api(&state, "POST", &format!("/conversas/c/videos/{n}/0")).await;
    assert_eq!(status, 200, "{bilhete}");
    assert_eq!((bilhete["tipo"].as_str(), bilhete["tamanho"].as_u64(), bilhete["nome"].as_str()), (Some("video/mp4"), Some(1000), Some("final.mp4")));
    let url = bilhete["url"].as_str().unwrap().to_owned();
    assert!(url.starts_with("/video/") && url.len() > 40);

    // Público só com o bilhete: o `<video>` não manda o token do aparelho.
    let app = router(state.clone());
    let pedaco = app.clone().oneshot(Request::builder().uri(&url).header("host", &*HOST_LOCAL).header("range", "bytes=10-19")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(pedaco.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(pedaco.headers()["content-range"], "bytes 10-19/1000");
    assert_eq!(pedaco.headers()["content-type"], "video/mp4");
    assert_eq!(to_bytes(pedaco.into_body(), 1 << 20).await.unwrap().as_ref(), &bytes[10..20]);
    let inteiro = app.clone().oneshot(Request::builder().uri(&url).header("host", &*HOST_LOCAL).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(inteiro.status(), StatusCode::OK);
    assert_eq!(to_bytes(inteiro.into_body(), 1 << 20).await.unwrap().len(), 1000);
    let fora = app.clone().oneshot(Request::builder().uri(&url).header("host", &*HOST_LOCAL).header("range", "bytes=5000-")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(fora.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    let falso = app.clone().oneshot(Request::builder().uri("/video/nao-existe").header("host", &*HOST_LOCAL).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(falso.status(), StatusCode::NOT_FOUND);
    // Pedir o bilhete continua exigindo o token.
    let sem_token = app.oneshot(req("POST", &format!("/conversas/c/videos/{n}/0"), &*ORIGEM_LOCAL, "")).await.unwrap();
    assert_eq!(sem_token.status(), StatusCode::UNAUTHORIZED);
}
