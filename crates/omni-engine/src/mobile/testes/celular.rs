//! Testes do acesso pelo celular: Tailscale/Serve, configurações, Authy e pareamento. Saíram do `mod tests` do antigo `mobile.rs` (Onda 2.0), sem mudar nenhum caso.
use super::*;

#[test] fn habilitar_gera_token_e_o_desktop_nao_escolhe_o_segredo() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().token.clear(); // primeira vez: ainda sem token
    let forjado = MobileConfig{enabled:true,bind:HOST_LOCAL.clone(),token:"escolhido-pelo-desktop".into(),serve:false,totp_secret:String::new(),totp_confirmed:false};
    settings(&state,Some(forjado),false).unwrap();
    let gerado = state.mobile.config.lock().unwrap().token.clone();
    assert_eq!(gerado.len(),64,"32 bytes em hex");
    assert_ne!(gerado,"escolhido-pelo-desktop");

    // Salvar de novo preserva o token: só `rotate` troca, senão todo "Aplicar" desparearia o celular.
    settings(&state,Some(MobileConfig{enabled:true,bind:HOST_LOCAL.clone(),token:String::new(),serve:false,totp_secret:String::new(),totp_confirmed:false}),false).unwrap();
    assert_eq!(state.mobile.config.lock().unwrap().token,gerado);
    settings(&state,Some(MobileConfig{enabled:true,bind:HOST_LOCAL.clone(),token:String::new(),serve:false,totp_secret:String::new(),totp_confirmed:false}),true).unwrap();
    assert_ne!(state.mobile.config.lock().unwrap().token,gerado);
}

#[test] fn serve_exige_loopback_no_bind() {
    // Escutar num IP roteável **e** publicar pelo Serve dobraria a superfície sem serventia.
    assert!(validate(&MobileConfig{enabled:true,bind:"100.64.0.1:47322".into(),token:String::new(),serve:true,totp_secret:String::new(),totp_confirmed:false}).is_err());
    assert!(validate(&MobileConfig{enabled:true,bind:HOST_LOCAL.clone(),token:String::new(),serve:true,totp_secret:String::new(),totp_confirmed:false}).is_ok());
}

/// O QR com `127.0.0.1` abria no PC e falhava no 4G. Com Tailscale no ar, o modo direto troca o
/// loopback pelo IP dele; nos outros casos não mexe em nada.
#[test] fn modo_direto_troca_loopback_pelo_ip_do_tailscale() {
    let ts: IpAddr = "100.94.187.72".parse().unwrap();
    let config = |bind:&str,serve:bool| MobileConfig{enabled:true,bind:bind.into(),token:String::new(),serve,totp_secret:String::new(),totp_confirmed:false};

    let mut padrao = config("127.0.0.1:47322",false);
    assert!(preferir_ip(&mut padrao,Some(ts)));
    assert_eq!(padrao.bind,"100.94.187.72:47322","troca o IP e mantém a porta");

    // Sem Tailscale não há para onde trocar: fica em loopback e a tela explica.
    let mut sem_tailscale = config("127.0.0.1:47322",false);
    assert!(!preferir_ip(&mut sem_tailscale,None));
    assert_eq!(sem_tailscale.bind,"127.0.0.1:47322");

    // O Serve exige loopback de propósito.
    let mut serve = config("127.0.0.1:47322",true);
    assert!(!preferir_ip(&mut serve,Some(ts)));
    assert_eq!(serve.bind,"127.0.0.1:47322");

    // Endereço já roteável escolhido pela pessoa não é sobrescrito.
    let mut escolhido = config("100.64.0.9:47322",false);
    assert!(!preferir_ip(&mut escolhido,Some(ts)));
    assert_eq!(escolhido.bind,"100.64.0.9:47322");
}

/// Saída real do `tailscale serve --bg` com o Serve desabilitado, capturada depois de matar o
/// processo — ele não sai sozinho. O link tem de sobreviver até a tela.
#[test] fn reset_so_quando_a_config_do_serve_e_so_a_nossa() {
    let nosso = json!({"TCP":{"443":{"HTTPS":true}},
        "Web":{"pc.tail0000.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:47322"}}}}});
    assert!(serve_e_so_o_nosso(&nosso,47322));
    // Outra porta local: é de outro programa (ou de outro engine), não nosso.
    assert!(!serve_e_so_o_nosso(&nosso,47399));
    // Qualquer coisa a mais publicada no PC impede o reset.
    let com_outra_rota = json!({"TCP":{"443":{"HTTPS":true}},
        "Web":{"pc.tail0000.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:47322"},"/grafana":{"Proxy":"http://127.0.0.1:3000"}}}}});
    assert!(!serve_e_so_o_nosso(&com_outra_rota,47322));
    let com_tcp = json!({"TCP":{"443":{"HTTPS":true},"22":{"TCPForward":"127.0.0.1:22"}},
        "Web":{"pc.tail0000.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:47322"}}}}});
    assert!(!serve_e_so_o_nosso(&com_tcp,47322));
    let com_funnel = json!({"TCP":{"443":{"HTTPS":true}},"AllowFunnel":{"pc.tail0000.ts.net:443":true},
        "Web":{"pc.tail0000.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:47322"}}}}});
    assert!(!serve_e_so_o_nosso(&com_funnel,47322));
    // Config vazia ou ilegível: nada a desfazer, e reset não é chamado.
    assert!(!serve_e_so_o_nosso(&json!({}),47322));
    assert!(!serve_e_so_o_nosso(&json!(null),47322));
}

#[test] fn so_desfaz_a_publicacao_quando_este_engine_deixa_de_publicar() {
    let cfg = |enabled, serve| MobileConfig{enabled,serve,bind:HOST_LOCAL.clone(),token:String::new(),
        totp_secret:String::new(),totp_confirmed:false};
    // O caso do bug: engine em modo direto (o de dev) nunca desfaz nada, nem ligando nem salvando.
    assert!(!deixou_de_publicar(&cfg(true,false),&cfg(true,false)));
    assert!(!deixou_de_publicar(&cfg(false,false),&cfg(true,false)));
    // Continuar publicando também não.
    assert!(!deixou_de_publicar(&cfg(true,true),&cfg(true,true)));
    // Serve → direto, e Serve → desligado: aí sim.
    assert!(deixou_de_publicar(&cfg(true,true),&cfg(true,false)));
    assert!(deixou_de_publicar(&cfg(true,true),&cfg(false,true)));
    // Serve configurado mas desligado nunca publicou.
    assert!(!deixou_de_publicar(&cfg(false,true),&cfg(true,false)));
}

#[test] fn link_de_liberacao_sai_da_saida_do_serve() {
    let saida = "Serve is not enabled on your tailnet.\nTo enable, visit:\n\n         https://login.tailscale.com/f/serve?node=nEXEMPLO0000CNTRL\n";
    assert_eq!(motivo_do_serve(saida),"https://login.tailscale.com/f/serve?node=nEXEMPLO0000CNTRL");
    assert_eq!(motivo_do_serve("   "),"O Tailscale não respondeu.");
    assert_eq!(motivo_do_serve("access denied"),"access denied");
}

/// "12345678901234567890" em base32 — o segredo dos vetores oficiais do RFC 6238.
const SEGREDO_RFC: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

fn codigo_em(segundos: u64) -> String { totp_de(SEGREDO_RFC,None).unwrap().generate(segundos) }

/// Vetor oficial do RFC 6238 (SHA-1, T=59 → 94287082; 6 dígitos → 287082). Se isto falhar, o
/// Authy e o engine discordam de todo código.
#[test] fn totp_bate_com_o_vetor_do_rfc_6238() {
    assert_eq!(codigo_em(59),"287082");
    assert_eq!(totp_confere(SEGREDO_RFC,"287082",59),Some(1));
}

#[test] fn totp_tolera_um_passo_de_relogio_e_nao_dois() {
    let codigo = codigo_em(59); // passo 1
    assert_eq!(totp_confere(SEGREDO_RFC,&codigo,60),Some(1),"relógio um passo à frente ainda vale");
    assert_eq!(totp_confere(SEGREDO_RFC,&codigo,90),None,"dois passos já não vale");
    assert_eq!(totp_confere(SEGREDO_RFC,"12345",59),None);
    assert_eq!(totp_confere(SEGREDO_RFC,"28708a",59),None);
}

fn com_authy_confirmado() -> (tempfile::TempDir, Arc<EngineState>) {
    let (dir,state) = fixture();
    {
        let mut config = state.mobile.config.lock().unwrap();
        config.token = "token-do-aparelho".into();
        config.totp_secret = SEGREDO_RFC.into();
        config.totp_confirmed = true;
    }
    (dir,state)
}

#[test] fn pareamento_entrega_o_token_uma_vez_por_codigo() {
    let (_dir,state) = com_authy_confirmado();
    let agora = 30 * 1_000_000;
    let codigo = codigo_em(agora);
    let resposta = parear_em(&state,&codigo,agora).expect("código atual deveria parear");
    assert_eq!(resposta.0["token"],"token-do-aparelho");
    // O mesmo código de novo é o cenário "vi por cima do ombro": recusado.
    assert_eq!(parear_em(&state,&codigo,agora).unwrap_err().0,StatusCode::UNAUTHORIZED);
}

#[test] fn pareamento_trava_depois_de_cinco_erros() {
    let (_dir,state) = com_authy_confirmado();
    let agora = 30 * 2_000_000;
    for _ in 0..MAX_FALHAS {
        assert_eq!(parear_em(&state,"000000",agora).unwrap_err().0,StatusCode::UNAUTHORIZED);
    }
    // Travado: nem o código certo passa até a janela acabar.
    assert_eq!(parear_em(&state,&codigo_em(agora),agora).unwrap_err().0,StatusCode::TOO_MANY_REQUESTS);
    let depois = agora + TRAVA_MS / 1000 + 30;
    assert!(parear_em(&state,&codigo_em(depois),depois).is_ok(),"a trava tem de acabar sozinha");
}

#[test] fn pareamento_recusa_sem_authy_confirmado() {
    let (_dir,state) = fixture();
    state.mobile.config.lock().unwrap().totp_secret = SEGREDO_RFC.into();
    assert_eq!(parear_em(&state,&codigo_em(59),59).unwrap_err().0,StatusCode::CONFLICT);
}

/// Rota pública (quem pareia ainda não tem token), mas sob a mesma checagem de Host e Origin.
#[tokio::test] async fn parear_nao_exige_token_mas_exige_origem() {
    let (_dir,state) = com_authy_confirmado();
    let app = router(state);
    let corpo = r#"{"codigo":"000000"}"#;
    let status = app.clone().oneshot(req("POST","/parear",&*ORIGEM_LOCAL,corpo)).await.unwrap().status();
    assert_eq!(status,StatusCode::UNAUTHORIZED,"401 aqui é do código errado, não do token ausente");
    let sem_origem = app.oneshot(req("POST","/parear","https://evil.example",corpo)).await.unwrap().status();
    assert_eq!(sem_origem,StatusCode::FORBIDDEN);
}

#[test] fn desktop_nao_confirma_o_authy_nem_troca_o_segredo() {
    let (_dir,state) = fixture();
    totp(&state,TotpAction::Reset).unwrap();
    let segredo = state.mobile.config.lock().unwrap().totp_secret.clone();
    assert!(!segredo.is_empty());

    let forjado = MobileConfig{enabled:false,bind:HOST_LOCAL.clone(),token:String::new(),serve:false,
        totp_secret:"FORJADO".into(),totp_confirmed:true};
    let EngineResponse::MobileSettings{totp_confirmed,totp_uri,..} = settings(&state,Some(forjado),false).unwrap() else { panic!() };
    assert!(!totp_confirmed,"confirmar só com código do Authy");
    assert!(totp_uri.is_some_and(|uri| uri.starts_with("otpauth://totp/")),"antes de confirmar o QR do Authy aparece");
    assert_eq!(state.mobile.config.lock().unwrap().totp_secret,segredo);
}

#[test] fn qr_do_authy_some_depois_de_confirmado() {
    let (_dir,state) = fixture();
    totp(&state,TotpAction::Reset).unwrap();
    let segredo = state.mobile.config.lock().unwrap().totp_secret.clone();
    let codigo = totp_de(&segredo,None).unwrap().generate(agora_segundos());
    let EngineResponse::MobileSettings{totp_confirmed,totp_uri,..} = totp(&state,TotpAction::Confirm{code:codigo}).unwrap() else { panic!() };
    assert!(totp_confirmed);
    assert!(totp_uri.is_none(),"depois de confirmado o segredo não sai mais do engine");
}

#[test] fn loopback_default_and_no_wildcard() {
    let config = MobileConfig::default(); assert!(!config.enabled); assert!(validate(&config).is_ok());
    assert!(validate(&MobileConfig{enabled:true,bind:"0.0.0.0:47322".into(),token:String::new(),serve:false,totp_secret:String::new(),totp_confirmed:false}).is_err());
    assert!(validate(&MobileConfig{enabled:true,bind:format!("127.0.0.1:{}", DEFAULT_ENGINE_PORT),token:String::new(),serve:false,totp_secret:String::new(),totp_confirmed:false}).is_err());
}
