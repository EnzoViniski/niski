//! Testes de abrir sessão, renomear e ocultar conversas. Saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// Renomear vale para celular e para a web de computador, e é o engine que grava — o índice de
/// conversas tem um escritor só.
#[tokio::test] async fn renomear_conversa_pela_api_grava_o_nome_e_recusa_lixo() {
    let (dir,state) = fixture();
    let app = router(state.clone());

    let resposta = app.clone().oneshot(req("POST","/conversas/c/nome",&*ORIGEM_LOCAL,r#"{"nome":"Refatorar login"}"#)).await.unwrap();
    assert_eq!(resposta.status(),StatusCode::OK);
    let conversas = omni_core::conversations::read_index(dir.path());
    assert_eq!(conversas[0].title,"Refatorar login");
    assert_eq!(conversas[0].title_source,omni_core::conversations::TitleSource::Manual);
    // Nome manual aparece na lista que o celular lê.
    let lista = summaries(&state);
    assert_eq!(lista[0]["title"],"Refatorar login");

    for lixo in [r#"{"nome":"   "}"#, r#"{"nome":"com
quebra"}"#] {
        assert_eq!(app.clone().oneshot(req("POST","/conversas/c/nome",&*ORIGEM_LOCAL,lixo)).await.unwrap().status(),StatusCode::BAD_REQUEST);
    }
    assert_eq!(app.oneshot(req("POST","/conversas/desconhecida/nome",&*ORIGEM_LOCAL,r#"{"nome":"x"}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
}

/// Ocultar tira da lista (marca `hidden`), reabrir devolve; id estranho nem chega ao disco.
#[tokio::test] async fn ocultar_e_reabrir_conversa() {
    let (dir,state) = fixture();
    let app = router(state.clone());
    let origem = &*ORIGEM_LOCAL;
    assert_eq!(app.clone().oneshot(req("POST","/conversas/c/ocultar",origem,r#"{"oculta":true}"#)).await.unwrap().status(),StatusCode::OK);
    assert_eq!(summaries(&state)[0]["hidden"],true);
    assert_eq!(app.clone().oneshot(req("POST","/conversas/c/ocultar",origem,r#"{"oculta":false}"#)).await.unwrap().status(),StatusCode::OK);
    assert!(summaries(&state)[0].get("hidden").is_none());
    assert_eq!(app.clone().oneshot(req("POST","/conversas/..%2Fx/ocultar",origem,r#"{"oculta":true}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
    assert_eq!(std::fs::read_to_string(dir.path().join(OCULTAS)).unwrap().trim(),"[]");
    // Bifurcar sem sessão aberta explica em vez de falhar calado.
    assert_eq!(app.oneshot(req("POST","/conversas/c/bifurcar",origem,"")).await.unwrap().status(),StatusCode::CONFLICT);
}

/// O celular pode abrir sessão, mas **só** escolhendo dentro de listas que o servidor já
/// conhece. Cada asserção aqui corresponde a uma forma de tentar escapar disso.
#[tokio::test] async fn http_so_abre_sessao_dentro_do_que_ja_e_conhecido() {
    let (_dir,state) = fixture();
    publish(&state,
        vec![PublishedProject{id:"p".into(),name:"Teste".into(),path:"C:/test".into()}],
        vec![PublishedAgent{id:"claude".into(),label:"Claude".into(),command:"claude".into(),resume:None}],
        None,
    ).unwrap();
    let app = router(state.clone());
    let post = |corpo:&str| req("POST","/sessoes",&*ORIGEM_LOCAL,corpo);

    // Caminho vindo do celular: recusado como campo desconhecido, não ignorado em silêncio.
    // 422 é o que o axum devolve quando o corpo não desserializa — o que importa é que
    // `deny_unknown_fields` transforma o `cwd` contrabandeado em recusa, não em campo ignorado.
    assert_eq!(app.clone().oneshot(post(r#"{"project_id":"p","provider":"claude","cwd":"C:/Windows"}"#)).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY);
    // Binário escolhido pelo celular: só vale o que o desktop publicou.
    assert_eq!(app.clone().oneshot(post(r#"{"project_id":"p","provider":"bash"}"#)).await.unwrap().status(),
        StatusCode::CONFLICT);
    // Projeto que o servidor não conhece.
    assert_eq!(app.clone().oneshot(post(r#"{"project_id":"inventado","provider":"claude"}"#)).await.unwrap().status(),
        StatusCode::CONFLICT);
    // Conta que não existe para esse agente.
    assert_eq!(app.clone().oneshot(post(r#"{"project_id":"p","provider":"claude","profile_id":"nao-existe"}"#)).await.unwrap().status(),
        StatusCode::CONFLICT);

    assert!(state.sessions.lock().unwrap().is_empty(),"nenhuma recusa pode ter aberto PTY");
}

#[test] fn o_plano_de_spawn_tira_o_caminho_do_registro_do_servidor() {
    let (_dir,state) = fixture();
    publish(&state,
        vec![PublishedProject{id:"p".into(),name:"Teste".into(),path:".".into()}],
        vec![PublishedAgent{id:"claude".into(),label:"Claude".into(),command:"claude".into(),resume:None}],
        None,
    ).unwrap();
    let plan = resolve_spawn(&state,&SpawnBody{
        project_id:"p".into(),provider:"claude".into(),profile_id:None,titulo:None,
    }).expect("deveria resolver");
    assert_eq!(plan.cwd,".","o cwd sai do projeto publicado");
    assert!(plan.command.starts_with("claude --session-id "),
        "o comando sai da CLI publicada e ja fixa o id da sessao: {}",plan.command);
    assert!(plan.external_session_id.is_some(),"sem id fixo a timeline do celular nasceria vazia");
}
