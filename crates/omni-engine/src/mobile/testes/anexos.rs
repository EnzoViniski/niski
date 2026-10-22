//! Testes dos anexos. Fork: saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// Imagem colada no chat do PC: binário em base64 e o nome no cabeçalho, como o celular manda.
#[tokio::test] async fn anexo_colado_no_pc_chega_pelo_protocolo_local() {
    let (dir,state) = fixture();
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,"segments":[]
    }]})).unwrap();
    let EngineResponse::Api { status, body, .. } = local(state,"POST","/conversas/c/anexos",None,Some("iVBORw0KGgo=".into()),
        vec![("x-omni-nome".into(),"print.png".into()),("content-type".into(),"image/png".into())]).await else { panic!("não é Api") };
    assert_eq!(status,200,"{body}");
    let caminho = serde_json::from_str::<Value>(&body).unwrap()["caminho"].as_str().unwrap().to_string();
    assert_eq!(std::fs::read(&caminho).unwrap(), b"\x89PNG\r\n\x1a\n");
    assert!(caminho.ends_with("print.png"));
}

#[test] fn nome_de_anexo_nao_escapa_da_pasta_nem_quebra_o_prompt() {
    assert_eq!(nome_seguro("foto.png"),"foto.png");
    assert_eq!(nome_seguro("../../Windows/System32/evil.dll"),"evil.dll");
    assert_eq!(nome_seguro("C:\\Users\\x\\segredo.txt"),"segredo.txt");
    assert_eq!(nome_seguro("Captura de Tela 2026-09-17, às 10.00.png"),"Captura-de-Tela-2026-09-17---s-10.00.png");
    assert_eq!(nome_seguro(""),"anexo");
    assert_eq!(nome_seguro(".."),"anexo");
    assert_eq!(nome_seguro("CON.txt"),"anexo");
    assert_eq!(nome_seguro("com1"),"anexo");
    assert!(nome_seguro(&"a".repeat(300)).len() <= 70);
    assert_eq!(nome_seguro(&format!("{}.pdf","b".repeat(300))).len(),64);
}

#[test] fn nome_do_cabecalho_chega_com_acento() {
    assert_eq!(percent_decode("relat%C3%B3rio%20final.pdf"),"relatório final.pdf");
    assert_eq!(percent_decode("100%"),"100%");
    assert_eq!(percent_decode("%zz"),"%zz");
    assert_eq!(percent_decode("%é"),"%é");
}

#[tokio::test] async fn anexo_grava_na_pasta_do_projeto_e_exige_token() {
    let (dir,state) = fixture();
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,"segments":[]
    }]})).unwrap();
    *state.mobile.config.lock().unwrap() = MobileConfig{enabled:true,bind:HOST_LOCAL.clone(),token:"segredo".into(),
        serve:false,totp_secret:String::new(),totp_confirmed:false};
    let app = router(state.clone());
    // Maior que o limite global de 32 KB: prova que a rota tem o limite dela.
    let conteudo = vec![7_u8; 200 * 1024];
    let pedido = |token: &str| Request::builder().method("POST").uri("/conversas/c/anexos")
        .header("host",&*HOST_LOCAL).header("origin",&*ORIGEM_LOCAL)
        .header("x-omni-token",token).header("x-omni-nome","..%2F..%2Ffoto%20nova.png")
        .body(Body::from(conteudo.clone())).unwrap();

    assert_eq!(app.clone().oneshot(pedido("errado")).await.unwrap().status(),StatusCode::UNAUTHORIZED);

    let resposta = app.clone().oneshot(pedido("segredo")).await.unwrap();
    assert_eq!(resposta.status(),StatusCode::OK);
    let valor: Value = serde_json::from_slice(&to_bytes(resposta.into_body(),usize::MAX).await.unwrap()).unwrap();
    let caminho = std::path::PathBuf::from(valor["caminho"].as_str().unwrap());
    assert!(caminho.starts_with(projeto.join(".omni-agents").join("anexos")),"{caminho:?}");
    assert!(caminho.file_name().unwrap().to_string_lossy().ends_with("-foto-nova.png"));
    assert_eq!(std::fs::read(&caminho).unwrap(),conteudo);

    // Conversa desconhecida não grava nada em lugar nenhum.
    let outra = Request::builder().method("POST").uri("/conversas/nao-existe/anexos")
        .header("host",&*HOST_LOCAL).header("origin",&*ORIGEM_LOCAL)
        .header("x-omni-token","segredo").body(Body::from(vec![1_u8])).unwrap();
    assert_eq!(app.clone().oneshot(outra).await.unwrap().status(),StatusCode::NOT_FOUND);

    // Acima do limite da rota: recusado.
    let grande = Request::builder().method("POST").uri("/conversas/c/anexos")
        .header("host",&*HOST_LOCAL).header("origin",&*ORIGEM_LOCAL)
        .header("x-omni-token","segredo").body(Body::from(vec![0_u8; ANEXO_MAX_BYTES + 1])).unwrap();
    assert_eq!(app.oneshot(grande).await.unwrap().status(),StatusCode::PAYLOAD_TOO_LARGE);
}
