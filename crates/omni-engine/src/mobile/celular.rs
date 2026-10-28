//! Acesso pelo celular: Tailscale (IP, MagicDNS, Serve), configurações, teste do servidor, Authy (TOTP) e pareamento.
//! Saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Roda a CLI do Tailscale e devolve (sucesso, stdout+stderr). Timeout curto: o cliente pode estar
/// travado e a tela de Configurações não pode ficar pendurada esperando por ele.
pub(super) fn tailscale(args: &[&str], segundos: u64) -> Option<(bool,String)> {
    let app = PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale"); // app da Mac App Store / site
    let path = if cfg!(windows) { PathBuf::from("C:/Program Files/Tailscale/tailscale.exe") }
        else if cfg!(target_os = "macos") && app.is_file() { app } else { PathBuf::from("tailscale") };
    let mut command = std::process::Command::new(path);
    // O binário do app do Tailscale no Mac só age como CLI se vê TERM (ou SHLVL); sem nenhum, como no
    // engine aberto pelo Finder, tenta abrir a GUI e falha com "CLIError error 3" — e a tela dizia
    // "Tailscale não detectado" com ele conectado.
    if std::env::var_os("TERM").is_none() { command.env("TERM","dumb"); }
    command.args(args).stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.spawn().ok()?;
    let started = std::time::Instant::now();
    let mut estourou = false;
    loop {
        if child.try_wait().ok()?.is_some() { break; }
        if started.elapsed() > std::time::Duration::from_secs(segundos) { let _ = child.kill(); estourou = true; break; }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    // A saída é lida **mesmo no timeout**. `tailscale serve --bg` com o Serve desabilitado imprime o
    // link de liberação e fica esperando, sem sair; antes a saída era descartada aqui e a tela só
    // dizia "o Tailscale não respondeu", escondendo justamente o link que resolvia.
    let output = child.wait_with_output().ok()?;
    let mut texto = String::from_utf8_lossy(&output.stdout).into_owned();
    texto.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success() && !estourou,texto))
}

pub(super) fn tailscale_ip() -> Option<IpAddr> {
    // The installed Tailscale client is the authority, not a guess based on a 100.x address.
    let (ok,saida) = tailscale(&["ip","-4"],2)?;
    ok.then(|| saida.trim().lines().next()?.trim().parse().ok()).flatten()
}

/// Nome MagicDNS deste PC (`meu-pc.tail1234.ts.net`). É o endereço que o `tailscale serve`
/// publica, e ele não muda quando o IP do tailnet muda.
pub(super) fn magic_dns() -> Option<String> {
    let (ok,saida) = tailscale(&["status","--json"],3)?;
    if !ok { return None }
    let valor: Value = serde_json::from_str(&saida).ok()?;
    let nome = valor["Self"]["DNSName"].as_str()?.trim_end_matches('.').to_string();
    (!nome.is_empty()).then_some(nome)
}

/// Publica a porta local pelo `tailscale serve`. Devolve `Err(link)` quando o Serve ainda não foi
/// habilitado na conta — é uma ação de uma vez só, por tailnet, e a UI mostra o link como botão.
pub(super) fn serve_start(porta: u16) -> Result<(),String> {
    // 10 s bastam: liberado, o comando volta em poucos segundos; não liberado, ele **nunca** volta
    // (imprime o link e fica esperando), então esperar mais só atrasa a tela.
    let Some((ok,saida)) = tailscale(&["serve","--bg",&porta.to_string()],10) else {
        return Err("Tailscale não encontrado neste PC.".into());
    };
    if ok { return Ok(()) }
    Err(motivo_do_serve(&saida))
}

/// Tira da saída do `tailscale serve` o que a tela precisa: o link de liberação, quando houver.
/// Separado para testar sem um Tailscale de verdade.
pub(super) fn motivo_do_serve(saida: &str) -> String {
    if let Some(link) = saida.split_whitespace().find(|palavra| palavra.starts_with("https://login.tailscale.com/")) {
        return link.to_owned();
    }
    let texto = saida.trim();
    if texto.is_empty() { "O Tailscale não respondeu.".into() } else { texto.to_owned() }
}

/// A configuração do Serve inteira é só a publicação que este app cria (`serve --bg <porta>`)?
///
/// `tailscale serve reset` apaga tudo que está publicado **no PC**, não só o que o OMNI publicou.
/// Isto é o que separa "desfazer o nosso" de "destruir o dos outros": reset só quando não há mais
/// nada lá. Separado da CLI para testar sem um Tailscale de verdade.
pub(super) fn serve_e_so_o_nosso(status: &Value, porta: u16) -> bool {
    let Some(config) = status.as_object() else { return false };
    let alvo = format!("http://127.0.0.1:{porta}");
    let vazio = |v: &Value| v.is_null() || v == &Value::Bool(false)
        || v.as_object().is_some_and(|o| o.is_empty()) || v.as_array().is_some_and(|a| a.is_empty());
    // Qualquer campo além de TCP/Web com conteúdo (Services, Foreground, Funnel…) é de outra coisa.
    if config.iter().any(|(chave, valor)| chave != "TCP" && chave != "Web" && !vazio(valor)) { return false }
    let tcp_ok = config.get("TCP").and_then(Value::as_object)
        .is_some_and(|tcp| tcp.keys().all(|porta_tcp| porta_tcp == "443"));
    let Some(web) = config.get("Web").and_then(Value::as_object) else { return false };
    let [(host, site)] = web.iter().collect::<Vec<_>>()[..] else { return false };
    let Some(handlers) = site.get("Handlers").and_then(Value::as_object) else { return false };
    let [(caminho, handler)] = handlers.iter().collect::<Vec<_>>()[..] else { return false };
    tcp_ok && host.ends_with(":443") && caminho == "/"
        && handler.get("Proxy").and_then(Value::as_str) == Some(alvo.as_str())
}

/// Tira do ar a publicação que este engine criou — e nada além dela. Com qualquer outra coisa
/// publicada no PC, deixa como está: sobrar uma rota velha é bem menos grave que apagar a de outro.
pub(super) fn serve_desfazer(porta: u16) {
    let Some((true, saida)) = tailscale(&["serve","status","--json"],5) else { return };
    let Ok(status) = serde_json::from_str::<Value>(&saida) else { return };
    if serve_e_so_o_nosso(&status, porta) { let _ = tailscale(&["serve","reset"],15); }
}

/// A publicação deste engine está de pé no Tailscale agora? `None` = não deu para perguntar.
pub(super) fn serve_ativo(porta: u16) -> Option<bool> {
    let (ok, saida) = tailscale(&["serve","status","--json"],5)?;
    if !ok { return None }
    let status: Value = serde_json::from_str(&saida).ok()?;
    let alvo = format!("http://127.0.0.1:{porta}");
    Some(status.get("Web").and_then(Value::as_object).is_some_and(|web| web.values().any(|site|
        site.get("Handlers").and_then(Value::as_object).is_some_and(|h| h.values().any(|handler|
            handler.get("Proxy").and_then(Value::as_str) == Some(alvo.as_str()))))))
}

/// Resolve MagicDNS e publica (ou despublica) pelo Serve. Roda na tarefa do servidor, fora do
/// caminho da tela, porque a CLI do Tailscale pode levar dezenas de segundos para responder.
pub(super) async fn publicar(state: &Arc<EngineState>, config: &MobileConfig) {
    // Zera antes de tentar: o aviso da tentativa anterior ("falta liberar") pararia o polling da
    // tela, e depois de a pessoa liberar e clicar em Aplicar o QR nunca apareceria.
    *state.mobile.serve_state.lock().expect("serve poisoned") = (None,None);
    let serve = config.serve;
    let porta: u16 = config.bind.rsplit(':').next().and_then(|p|p.parse().ok()).unwrap_or(0);
    let resultado = tokio::task::spawn_blocking(move || {
        let nome = magic_dns();
        // Modo direto **não mexe** no Serve. Antes isto rodava `serve reset` em toda subida — e o
        // engine de desenvolvimento (modo direto) apagava a publicação HTTPS do app instalado toda
        // vez que `npm run dev` abria: o QR do celular passava a apontar para um endereço sem nada
        // atrás. Desfazer a própria publicação é trabalho do `settings`, na troca de modo.
        if !serve { return (nome,Ok(())) }
        let publicado = serve_start(porta);
        (nome,publicado)
    }).await;
    let Ok((nome,publicado)) = resultado else { return };

    *state.mobile.magic_dns.lock().expect("magic poisoned") = nome.clone();
    *state.mobile.serve_state.lock().expect("serve poisoned") = match publicado {
        Ok(()) if serve => (nome.map(|n| format!("https://{n}")),None),
        Ok(()) => (None,None),
        Err(motivo) if motivo.starts_with("https://login.tailscale.com/") => (None,Some(motivo)),
        Err(motivo) => (None,Some(motivo)),
    };
}


/// Bate na porta do próprio servidor e conta o que aconteceu, em português.
///
/// Existe porque `listening` só prova que o `bind` deu certo — não prova que alguém consegue
/// chegar. Era exatamente esse o buraco: a tela dizia "no ar" e o celular dava tempo esgotado, sem
/// nada para diferenciar um problema de rede de um servidor que nunca subiu.
///
/// TCP cru e um GET de uma linha em vez de um cliente HTTP: são 15 linhas contra uma dependência
/// nova no engine.
pub fn check(state: &EngineState) -> Result<EngineResponse> {
    use std::io::{Read as _, Write as _};
    let config = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?.clone();
    if !config.enabled { return Ok(EngineResponse::MobileCheck{ok:false,message:"Acesso pelo celular está desativado.".into()}); }

    let address: SocketAddr = config.bind.parse().context("Endereço inválido")?;
    let Ok(mut fluxo) = std::net::TcpStream::connect_timeout(&address,std::time::Duration::from_secs(3)) else {
        return Ok(EngineResponse::MobileCheck{ok:false,
            message:format!("Nada atendeu em {}. O servidor não chegou a subir — desative e ative de novo.",config.bind)});
    };
    fluxo.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    write!(fluxo,"GET /conversas HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",config.bind)?;
    let mut resposta = String::new();
    let _ = fluxo.take(256).read_to_string(&mut resposta);

    // No Serve, o servidor local responder não basta: o celular entra pelo Tailscale, e a
    // publicação pode ter sido apagada por fora (outro programa, `tailscale serve reset` à mão).
    if config.serve && resposta.contains(" 401") && serve_ativo(address.port()) == Some(false) {
        // Reiniciar o listener refaz a publicação (`publicar` roda na subida).
        state.mobile.changed.notify_one();
        return Ok(EngineResponse::MobileCheck{ok:false,
            message:"O servidor está no ar, mas o endereço HTTPS tinha saído do Tailscale — por isso o QR não abria. Publicando de novo; aguarde alguns segundos e teste outra vez.".into()});
    }

    // 401 é o resultado **bom**: prova que o servidor atendeu e que o token está sendo exigido.
    Ok(if resposta.contains(" 401") {
        EngineResponse::MobileCheck{ok:true,message:"Servidor no ar e exigindo o código de acesso, como deve ser.".into()}
    } else if resposta.contains(" 200") {
        EngineResponse::MobileCheck{ok:false,message:"Servidor no ar, mas respondeu sem exigir código — desative e ative de novo.".into()}
    } else {
        EngineResponse::MobileCheck{ok:false,
            message:format!("Resposta inesperada do servidor: {}",resposta.lines().next().unwrap_or("(vazia)"))}
    })
}

/// `127.0.0.1` nunca é alcançável pelo celular. No modo direto, se o Tailscale está no ar, troca o
/// loopback pelo IP dele mantendo a porta. Devolve se trocou.
///
/// Sem isto a tela gerava um QR apontando para `127.0.0.1:47322`: abria no PC, e no 4G falhava sem
/// explicação nenhuma. O IP vem injetado para o teste não depender de um Tailscale de verdade.
pub(super) fn preferir_ip(config: &mut MobileConfig, tailscale: Option<IpAddr>) -> bool {
    if config.serve { return false } // o Serve exige loopback de propósito
    let Ok(endereco) = config.bind.parse::<SocketAddr>() else { return false };
    let Some(ip) = tailscale.filter(|_| endereco.ip().is_loopback()) else { return false };
    config.bind = SocketAddr::new(ip,endereco.port()).to_string();
    true
}

pub(super) fn preferir_tailscale(config: &mut MobileConfig) -> bool {
    if config.serve || !config.bind.starts_with("127.") { return false }
    preferir_ip(config,tailscale_ip())
}

/// Este engine publicava pelo Serve e, com a config nova, não publica mais (trocou para o modo
/// direto ou desligou o acesso). É o único momento em que a publicação dele deve sair do ar.
pub(super) fn deixou_de_publicar(anterior: &MobileConfig, nova: &MobileConfig) -> bool {
    (anterior.enabled && anterior.serve) && !(nova.enabled && nova.serve)
}

pub(super) fn validate(config: &MobileConfig) -> Result<SocketAddr> {
    let address: SocketAddr = config.bind.parse().context("Use IP:porta, por exemplo 100.x.x.x:47322")?;
    if address.port() == 0 || address.port() == DEFAULT_ENGINE_PORT { return Err(anyhow!("Porta HTTP inválida")); }
    // No modo serve quem atende a rede é o tailscaled; escutar num IP roteável além disso só
    // aumentaria a superfície exposta sem serventia.
    if config.serve && address.ip() != IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) {
        return Err(anyhow!("Com o Tailscale Serve ligado, use 127.0.0.1 no endereço"));
    }
    if address.ip() == IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) { return Ok(address); }
    if address.ip().is_unspecified() || Some(address.ip()) != tailscale_ip() {
        return Err(anyhow!("Escolha 127.0.0.1 ou o IP da interface Tailscale instalada neste PC"));
    }
    Ok(address)
}

pub fn settings(state: &EngineState, config: Option<MobileConfig>, rotate: bool) -> Result<EngineResponse> {
    if let Some(mut config) = config {
        // O `token` que veio do desktop é descartado: quem guarda o segredo é o engine. Assim uma
        // tela de configuração não consegue plantar um token escolhido por ela.
        let atual = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?.token.clone();
        config.token = if rotate || (config.enabled && atual.is_empty()) { random_token() } else { atual };
        if rotate { super::video_rota::esquecer_bilhetes(); } // Fork (segurança)
        // Mesmo motivo do token: o cadastro do Authy é do engine. Uma tela de configuração que
        // mandasse `totp_confirmed: true` não pode pular a confirmação.
        {
            let guardado = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?;
            config.totp_secret = guardado.totp_secret.clone();
            config.totp_confirmed = guardado.totp_confirmed;
        }
        if config.enabled { preferir_tailscale(&mut config); validate(&config)?; }
        let anterior = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?.clone();
        omni_protocol::atomic_write_json_privado(&state.state_file.with_file_name("mobile.json"),&config)?;
        if deixou_de_publicar(&anterior,&config) {
            let porta = anterior.bind.rsplit(':').next().and_then(|p|p.parse().ok()).unwrap_or(0);
            // Fora do caminho da tela: a CLI do Tailscale pode demorar, e a resposta não depende disto.
            std::thread::spawn(move || serve_desfazer(porta));
        }
        *state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))? = config;
        // Rotacionar reinicia o listener junto: é o que derruba quem estava com o token antigo.
        state.mobile.changed.notify_one();
    }
    let config = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?.clone();
    let (listening,error) = state.mobile.status.lock().map_err(|_|anyhow!("status poisoned"))?.clone();
    let magic = state.mobile.magic_dns.lock().map_err(|_|anyhow!("magic poisoned"))?.clone();
    let (serve_url,serve_hint) = state.mobile.serve_state.lock().map_err(|_|anyhow!("serve poisoned"))?.clone();

    // No modo Serve o endereço público só existe quando o Tailscale confirmou a publicação; até
    // lá a tela mostra "ligando" em vez de um link de loopback que o celular não alcança.
    let public_url = if config.serve {
        serve_url
    } else {
        // Loopback só abre neste PC: não vira QR. Sem Tailscale, a tela explica em vez de gerar um
        // código que falha no celular.
        listening.clone().filter(|_| !config.bind.starts_with("127."))
    };

    // O link do QR só existe quando há endereço público; sem isso o celular leria um endereço morto.
    let qr = public_url.as_ref().map(|url| format!("{url}/#t={}",config.token));
    let totp_confirmed = config.totp_confirmed;
    let totp_uri = (!config.totp_confirmed && !config.totp_secret.is_empty())
        .then(|| totp_de(&config.totp_secret, magic.as_deref()).map(|t| t.get_url()))
        .flatten();
    Ok(EngineResponse::MobileSettings { config,listening,error,qr,public_url,magic_dns:magic,serve_hint,totp_confirmed,totp_uri })
}

/// Passo do TOTP (RFC 6238). Padrão do Authy e do Google Authenticator: 30 s, 6 dígitos, SHA-1.
pub(super) const TOTP_PASSO: u64 = 30;
/// Janela de tolerância, em passos, para relógio do PC e do celular fora de sincronia.
pub(super) const TOTP_JANELA: i64 = 1;
/// Erros de pareamento tolerados dentro de `TRAVA_MS` antes de recusar tudo.
pub(super) const MAX_FALHAS: usize = 5;
pub(super) const TRAVA_MS: u64 = 5 * 60_000;

/// Fork (segurança): erros seguidos (contados também entre reinícios do engine) que desligam o
/// pareamento pelo Authy até o PC confirmar o Authy de novo. Só a janela de 5 min deixava tentar
/// para sempre (~1 por minuto: ~12% de acerto em um mês); com o teto, a chance total fica ~5×10⁻⁵.
pub(super) const MAX_FALHAS_SEGUIDAS: u32 = 15;
const ARQUIVO_DE_FALHAS: &str = "pareamento-falhas.json";

#[derive(Default)]
pub struct Pareamento { falhas: VecDeque<u64>, ultimo_passo: u64, seguidas: u32 }

impl Pareamento {
    pub(super) fn abrir(dir: &Path) -> Self {
        let seguidas = omni_protocol::read_json_or_default::<u32>(&dir.join(ARQUIVO_DE_FALHAS));
        Self { seguidas, ..Self::default() }
    }
}

fn gravar_falhas_seguidas(state: &EngineState, seguidas: u32) {
    let caminho = state.state_file.with_file_name(ARQUIVO_DE_FALHAS);
    if seguidas == 0 { let _ = std::fs::remove_file(caminho); } else { let _ = atomic_write_json(&caminho, &seguidas); }
}

/// Monta o TOTP a partir do segredo guardado. `None` com segredo corrompido.
pub(super) fn totp_de(secret: &str, conta: Option<&str>) -> Option<totp_rs::TOTP> {
    let bytes = totp_rs::Secret::Encoded(secret.to_owned()).to_bytes().ok()?;
    // `:` é separador no `otpauth://`: nome com dois pontos invalida o cadastro.
    let conta = conta.unwrap_or("PC").replace(':', "-");
    totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 0, TOTP_PASSO, bytes, Some(omni_protocol::NOME_DO_PRODUTO.into()), conta).ok()
}

/// Passo aceito para `code` no instante `agora` (segundos), com janela ±`TOTP_JANELA`.
///
/// Devolve o passo, e não só `bool`, porque o pareamento recusa reuso: um código visto por cima do
/// ombro não pode ser digitado de novo nos segundos seguintes. Comparação em tempo constante.
pub(super) fn totp_confere(secret: &str, code: &str, agora: u64) -> Option<u64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) { return None }
    let totp = totp_de(secret, None)?;
    let atual = (agora / TOTP_PASSO) as i64;
    (-TOTP_JANELA..=TOTP_JANELA).map(|delta| atual + delta).filter(|passo| *passo >= 0)
        .find(|passo| token_confere(code, &totp.generate(*passo as u64 * TOTP_PASSO)))
        .map(|passo| passo as u64)
}

pub(super) fn agora_segundos() -> u64 { now_ms() / 1000 }

/// Cadastro do Authy, pedido pelo desktop.
pub fn totp(state: &EngineState, action: TotpAction) -> Result<EngineResponse> {
    {
        let mut config = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?;
        match action {
            TotpAction::Reset => {
                let totp_rs::Secret::Encoded(novo) = totp_rs::Secret::generate_secret().to_encoded() else {
                    return Err(anyhow!("Falha ao gerar o segredo do Authy"));
                };
                config.totp_secret = novo;
                config.totp_confirmed = false;
                zerar_falhas(state);
            }
            TotpAction::Confirm { code } => {
                if config.totp_secret.is_empty() { return Err(anyhow!("Gere o código do Authy primeiro")); }
                if totp_confere(&config.totp_secret, code.trim(), agora_segundos()).is_none() {
                    return Err(anyhow!("Código do Authy não confere. Confira se leu o QR desta tela e use o código atual."));
                }
                config.totp_confirmed = true;
                zerar_falhas(state);
            }
        }
        omni_protocol::atomic_write_json_privado(&state.state_file.with_file_name("mobile.json"),&*config)?;
    }
    settings(state,None,false)
}

fn zerar_falhas(state: &EngineState) {
    if let Ok(mut pareamento) = state.mobile.pareamento.lock() { pareamento.seguidas = 0; pareamento.falhas.clear(); }
    gravar_falhas_seguidas(state, 0);
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct PareamentoBody { codigo: String }

/// Troca um código atual do Authy pelo token de dispositivo.
///
/// Existe porque o app da tela inicial do iPhone **não enxerga** o armazenamento do Safari (decisão
/// da Apple, WebKit bug 181849): o token gravado pelo QR não chega nele. É rota pública — quem
/// chama é justamente quem ainda não tem token — então a trava de tentativas é o que segura força
/// bruta: 3 códigos válidos em 10^6 e 5 tentativas a cada 5 minutos.
pub(super) async fn parear(State(state): State<Arc<EngineState>>, Json(body): Json<PareamentoBody>) -> Result<Json<Value>,ApiError> {
    tokio::task::spawn_blocking(move || parear_em(&state,&body.codigo,agora_segundos()))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha no pareamento"))?
}

pub(super) fn parear_em(state: &EngineState, codigo: &str, agora: u64) -> Result<Json<Value>,ApiError> {
    let config = state.mobile.config.lock().expect("config poisoned").clone();
    if !config.totp_confirmed || config.totp_secret.is_empty() {
        return Err(error(StatusCode::CONFLICT,"Configure o Authy na aba Celular do PC primeiro"));
    }
    let mut pareamento = state.mobile.pareamento.lock().expect("pareamento poisoned");
    let agora_ms = agora * 1000;
    pareamento.falhas.retain(|instante| agora_ms.saturating_sub(*instante) < TRAVA_MS);
    if pareamento.falhas.len() >= MAX_FALHAS {
        return Err(error(StatusCode::TOO_MANY_REQUESTS,"Muitas tentativas erradas. Espere alguns minutos e tente de novo."));
    }
    match totp_confere(&config.totp_secret, codigo.trim(), agora) {
        Some(passo) if passo > pareamento.ultimo_passo => {
            pareamento.ultimo_passo = passo;
            pareamento.falhas.clear();
            if pareamento.seguidas > 0 { pareamento.seguidas = 0; gravar_falhas_seguidas(state, 0); }
            Ok(Json(json!({"token":config.token})))
        }
        // Passo já usado conta como erro: é exatamente a tentativa de reaproveitar um código visto.
        _ => {
            pareamento.falhas.push_back(agora_ms);
            pareamento.seguidas += 1;
            gravar_falhas_seguidas(state, pareamento.seguidas);
            if pareamento.seguidas >= MAX_FALHAS_SEGUIDAS {
                // Fork (segurança): desliga o Authy até o PC confirmar de novo (o QR continua valendo).
                drop(pareamento);
                if let Ok(mut guardada) = state.mobile.config.lock() {
                    guardada.totp_confirmed = false;
                    let _ = omni_protocol::atomic_write_json_privado(&state.state_file.with_file_name("mobile.json"),&*guardada);
                }
                registro::aviso("pareamento_desligado", &[("motivo", "tentativas")]);
                return Err(error(StatusCode::CONFLICT,"Muitas tentativas erradas: o pareamento pelo Authy foi desligado. Confirme o Authy de novo na aba Celular do PC."));
            }
            Err(error(StatusCode::UNAUTHORIZED,"Código do Authy não confere. Use o código que está aparecendo agora."))
        }
    }
}
