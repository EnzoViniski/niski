//! Testes do protocolo local (`local`), do base64 e da tabela de rotas. Fork: saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

/// `/painel` junta lista e catálogo; com o `ETag` de volta e nada mudado, responde 304 sem corpo.
/// A visão Conversa do desktop chama as rotas do celular pelo protocolo local: sem token de
/// dispositivo nem `Origin`, que lá não existem — quem chega já mostrou o token do engine.
#[tokio::test] async fn desktop_chama_a_api_pelo_protocolo_local() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().token = "token-do-celular".into();
    let EngineResponse::Api { status, body, .. } = local(state.clone(),"GET","/conversas/c/timeline",None,None,vec![]).await else { panic!("não é Api") };
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

/// Fork (Onda 2.0): fumaça da tabela de rotas. Com o `mobile` dividido em módulos, uma rota
/// esquecida no `rotas()` só apareceria na tela. Toda rota daqui tem de chegar a um handler pelo
/// protocolo local: nem 405 (método errado) nem o 404 vazio do roteador — o 404 de um handler
/// ("Conversa não encontrada") vem com corpo JSON e conta como achado.
#[tokio::test] async fn toda_rota_da_tabela_chega_a_um_handler() {
    let (_dir,state) = fixture();
    let rotas: &[(&str, &str)] = &[
        ("GET","/conversas"), ("GET","/projetos"), ("GET","/painel"), ("GET","/atencao"), ("GET","/contas"),
        ("GET","/conversas/c/timeline"), ("GET","/conversas/c/terminal"), ("GET","/conversas/c/imagens/0/0"),
        ("GET","/conversas/c/arquivos/0/0"), ("GET","/conversas/c/opcoes"), ("GET","/conversas/c/arquivos"),
        ("POST","/conversas/c/teclas"), ("POST","/conversas/c/destravar"), ("POST","/conversas/c/prompt"),
        ("POST","/conversas/c/aprovar"), ("POST","/conversas/c/escolher"), ("POST","/conversas/c/nome"),
        ("POST","/conversas/c/ocultar"), ("POST","/conversas/c/bifurcar"), ("POST","/conversas/c/modo"),
        ("POST","/conversas/c/ajustes"), ("POST","/conversas/c/retomar"), ("POST","/conversas/c/anexos"),
        ("GET","/fluxos/execucoes"), ("POST","/fluxos/execucoes"), ("GET","/fluxos/execucoes/x"),
        ("POST","/fluxos/execucoes/x/pausar"), ("GET","/fluxos/execucoes/x/nos/n/historico"),
        ("POST","/fluxos/execucoes/x/nos/n/aprovar"), ("POST","/fluxos/execucoes/x/nos/n/conversa"),
        ("POST","/contas/x/uso"), ("POST","/historico/levar"), ("POST","/historico/buscar"),
        ("GET","/extensoes"), ("POST","/extensoes/compartilhar"), ("POST","/extensoes/principal"),
        ("GET","/terminais"), ("POST","/terminais"), ("GET","/terminais/x/saida"), ("POST","/terminais/x/teclas"),
        ("POST","/terminais/x/fechar"), ("POST","/sessoes"), ("POST","/parear"),
        ("GET","/orquestracao"), ("GET","/orquestracao/x"), ("POST","/orquestracao/x/agentes/a/parar"),
        ("GET","/antigravity/permissoes"),
    ];
    for (metodo, caminho) in rotas {
        let corpo = (*metodo == "POST").then(|| "{}".to_string());
        let EngineResponse::Api { status, body, .. } = local(state.clone(),metodo,caminho,corpo,None,vec![]).await else { panic!("não é Api") };
        assert_ne!(status, 405, "{metodo} {caminho}: método não registrado");
        assert!(!(status == 404 && body.trim().is_empty()), "{metodo} {caminho}: rota não registrada no rotas()");
    }
    // O controle: uma rota que não existe dá exatamente o 404 vazio que a tabela acima não pode dar.
    let EngineResponse::Api { status, body, .. } = local(state,"GET","/nao-existe",None,None,vec![]).await else { panic!("não é Api") };
    assert_eq!((status, body.trim().is_empty()), (404, true));
}
