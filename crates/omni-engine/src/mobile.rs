use super::*;
use axum::{Router, Json, extract::{State, Path as RoutePath, Query, DefaultBodyLimit, Request},
    http::{StatusCode, HeaderMap}, response::{IntoResponse, Response}, routing::{get,post}, middleware::{self, Next}};
use omni_protocol::{MobileConfig, PublishedAgent, PublishedProject, PublishedTheme, PublishedWorkspace, TotpAction};
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    net::{SocketAddr, IpAddr},
};
use tokio::sync::Notify;

#[cfg(mobile_assets)]
static ASSETS: include_dir::Dir<'_> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../dist-mobile");

pub struct MobileRuntime {
    config: Mutex<MobileConfig>,
    workspace: Mutex<PublishedWorkspace>,
    /// Nome MagicDNS resolvido na última leitura das configurações. Cacheado porque o
    /// `same_origin` roda em **toda** requisição e não pode disparar a CLI do Tailscale.
    magic_dns: Mutex<Option<String>>,
    /// `(url pública, link para habilitar o Serve na conta)`. Preenchido pela tarefa do servidor,
    /// nunca pela tela: `tailscale serve` pode levar dezenas de segundos e não pode segurar as
    /// Configurações esperando.
    serve_state: Mutex<(Option<String>,Option<String>)>,
    /// Tentativas de pareamento pelo Authy: instantes das falhas recentes e o último passo TOTP aceito.
    pareamento: Mutex<Pareamento>,
    status: Mutex<(Option<String>,Option<String>)>,
    changed: Notify,
    actions: Mutex<VecDeque<Action>>,
    pending: Notify,
    history: Mutex<MobileHistory>,
    /// Transcripts já lidos: a conversa aberta é consultada a cada 1,5 s e só o trecho novo é lido.
    feeds: Mutex<omni_core::feed::FeedCache>,
    /// Última busca por rollouts do Codex ainda sem caminho. Abre o começo de cada rollout da conta,
    /// então não pode rodar a cada leitura da lista.
    codex_descoberto_em: Mutex<u64>,
}

#[derive(Default)]
struct MobileHistory {
    scan: omni_core::history::ScanCache,
    entries: Vec<omni_core::history::HistoryEntry>,
    scanned_at_ms: u64,
}
#[derive(Clone, Serialize)]
struct Action {
    id: String, conversation_id: String, session_id: String, state: String,
    created_at_ms: u64, error: Option<String>,
    #[serde(skip)] key: String,
    #[serde(skip)] revision: String,
    #[serde(skip)] kind: ActionKind,
}
#[derive(Clone, PartialEq)]
enum ActionKind { Prompt(String), Approval(bool), Spawn(SpawnPlan), Modo(String), Ajustes(Ajustes), Retomar(Retomada) }

/// Uma conversa sem sessão aberta (do índice do OMNI ou só do histórico da CLI) e como reabri-la.
/// Resolvido no servidor antes da fila, como o `SpawnPlan`: o corpo do request só escolhe conta,
/// modelo e esforço.
#[derive(Clone, PartialEq)]
struct Retomada {
    conversation_id: String,
    project_id: String,
    cwd: String,
    title: String,
    provider: String,
    profile_id: Option<String>,
    session_id: String,
    /// O arquivo da conversa na conta de origem — é ele que vai junto se a conta mudar.
    transcript: Option<String>,
    ajustes: Ajustes,
}

/// Conta, modelo e esforço da conversa. `None` em modelo/esforço = o padrão da CLI.
#[derive(Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ajustes { profile_id: Option<String>, model: Option<String>, effort: Option<String> }

/// Tudo já resolvido **no servidor** antes de entrar na fila. O corpo do request não contribui com
/// nenhum caminho nem binário: o `cwd` vem do projeto conhecido, o `command` da lista de CLIs que o
/// desktop publicou, e o `env` do perfil gravado em `profiles.json`.
#[derive(Clone, PartialEq)]
struct SpawnPlan {
    project_id: String,
    cwd: String,
    title: String,
    /// O título veio do usuário (e não do rótulo padrão do agente).
    titulo_do_usuario: bool,
    provider: String,
    profile_id: Option<String>,
    command: String,
    env: Vec<(String,String)>,
    external_session_id: Option<String>,
    transcript_path: Option<String>,
}

impl MobileRuntime {
    pub fn new(dir: &Path) -> Self {
        Self { config: Mutex::new(omni_protocol::read_json_or_default(&dir.join("mobile.json"))),
            // Em disco, não só em memória: o engine reinicia sem o desktop aberto (update, crash), e
            // aí a lista sumiria justamente quando o celular é a única via de acesso.
            workspace: Mutex::new(omni_protocol::read_json_or_default(&dir.join("workspace.json"))),
            magic_dns: Mutex::new(None),
            serve_state: Mutex::new((None,None)),
            pareamento: Mutex::new(Pareamento::default()),
            status: Mutex::new((None,None)),changed: Notify::new(),actions: Mutex::new(VecDeque::new()),pending: Notify::new(),
            history: Mutex::new(MobileHistory::default()),
            feeds: Mutex::new(omni_core::feed::FeedCache::default()),
            codex_descoberto_em: Mutex::new(0) }
    }
}

/// Roda a CLI do Tailscale e devolve (sucesso, stdout+stderr). Timeout curto: o cliente pode estar
/// travado e a tela de Configurações não pode ficar pendurada esperando por ele.
fn tailscale(args: &[&str], segundos: u64) -> Option<(bool,String)> {
    let app = PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale"); // app da Mac App Store / site
    let path = if cfg!(windows) { PathBuf::from("C:/Program Files/Tailscale/tailscale.exe") }
        else if cfg!(target_os = "macos") && app.is_file() { app } else { PathBuf::from("tailscale") };
    let mut command = std::process::Command::new(path);
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

fn tailscale_ip() -> Option<IpAddr> {
    // The installed Tailscale client is the authority, not a guess based on a 100.x address.
    let (ok,saida) = tailscale(&["ip","-4"],2)?;
    ok.then(|| saida.trim().lines().next()?.trim().parse().ok()).flatten()
}

/// Nome MagicDNS deste PC (`pc-felipe.tail68f850.ts.net`). É o endereço que o `tailscale serve`
/// publica, e ele não muda quando o IP do tailnet muda.
fn magic_dns() -> Option<String> {
    let (ok,saida) = tailscale(&["status","--json"],3)?;
    if !ok { return None }
    let valor: Value = serde_json::from_str(&saida).ok()?;
    let nome = valor["Self"]["DNSName"].as_str()?.trim_end_matches('.').to_string();
    (!nome.is_empty()).then_some(nome)
}

/// Publica a porta local pelo `tailscale serve`. Devolve `Err(link)` quando o Serve ainda não foi
/// habilitado na conta — é uma ação de uma vez só, por tailnet, e a UI mostra o link como botão.
fn serve_start(porta: u16) -> Result<(),String> {
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
fn motivo_do_serve(saida: &str) -> String {
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
fn serve_e_so_o_nosso(status: &Value, porta: u16) -> bool {
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
fn serve_desfazer(porta: u16) {
    let Some((true, saida)) = tailscale(&["serve","status","--json"],5) else { return };
    let Ok(status) = serde_json::from_str::<Value>(&saida) else { return };
    if serve_e_so_o_nosso(&status, porta) { let _ = tailscale(&["serve","reset"],15); }
}

/// A publicação deste engine está de pé no Tailscale agora? `None` = não deu para perguntar.
fn serve_ativo(porta: u16) -> Option<bool> {
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
async fn publicar(state: &Arc<EngineState>, config: &MobileConfig) {
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
fn preferir_ip(config: &mut MobileConfig, tailscale: Option<IpAddr>) -> bool {
    if config.serve { return false } // o Serve exige loopback de propósito
    let Ok(endereco) = config.bind.parse::<SocketAddr>() else { return false };
    let Some(ip) = tailscale.filter(|_| endereco.ip().is_loopback()) else { return false };
    config.bind = SocketAddr::new(ip,endereco.port()).to_string();
    true
}

fn preferir_tailscale(config: &mut MobileConfig) -> bool {
    if config.serve || !config.bind.starts_with("127.") { return false }
    preferir_ip(config,tailscale_ip())
}

/// Este engine publicava pelo Serve e, com a config nova, não publica mais (trocou para o modo
/// direto ou desligou o acesso). É o único momento em que a publicação dele deve sair do ar.
fn deixou_de_publicar(anterior: &MobileConfig, nova: &MobileConfig) -> bool {
    (anterior.enabled && anterior.serve) && !(nova.enabled && nova.serve)
}

fn validate(config: &MobileConfig) -> Result<SocketAddr> {
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
        // Mesmo motivo do token: o cadastro do Authy é do engine. Uma tela de configuração que
        // mandasse `totp_confirmed: true` não pode pular a confirmação.
        {
            let guardado = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?;
            config.totp_secret = guardado.totp_secret.clone();
            config.totp_confirmed = guardado.totp_confirmed;
        }
        if config.enabled { preferir_tailscale(&mut config); validate(&config)?; }
        let anterior = state.mobile.config.lock().map_err(|_|anyhow!("config poisoned"))?.clone();
        atomic_write_json(&state.state_file.with_file_name("mobile.json"),&config)?;
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
const TOTP_PASSO: u64 = 30;
/// Janela de tolerância, em passos, para relógio do PC e do celular fora de sincronia.
const TOTP_JANELA: i64 = 1;
/// Erros de pareamento tolerados dentro de `TRAVA_MS` antes de recusar tudo.
const MAX_FALHAS: usize = 5;
const TRAVA_MS: u64 = 5 * 60_000;

#[derive(Default)]
pub struct Pareamento { falhas: VecDeque<u64>, ultimo_passo: u64 }

/// Monta o TOTP a partir do segredo guardado. `None` com segredo corrompido.
fn totp_de(secret: &str, conta: Option<&str>) -> Option<totp_rs::TOTP> {
    let bytes = totp_rs::Secret::Encoded(secret.to_owned()).to_bytes().ok()?;
    // `:` é separador no `otpauth://`: nome com dois pontos invalida o cadastro.
    let conta = conta.unwrap_or("PC").replace(':', "-");
    totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 0, TOTP_PASSO, bytes, Some("OMNI AGENTS".into()), conta).ok()
}

/// Passo aceito para `code` no instante `agora` (segundos), com janela ±`TOTP_JANELA`.
///
/// Devolve o passo, e não só `bool`, porque o pareamento recusa reuso: um código visto por cima do
/// ombro não pode ser digitado de novo nos segundos seguintes. Comparação em tempo constante.
fn totp_confere(secret: &str, code: &str, agora: u64) -> Option<u64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) { return None }
    let totp = totp_de(secret, None)?;
    let atual = (agora / TOTP_PASSO) as i64;
    (-TOTP_JANELA..=TOTP_JANELA).map(|delta| atual + delta).filter(|passo| *passo >= 0)
        .find(|passo| token_confere(code, &totp.generate(*passo as u64 * TOTP_PASSO)))
        .map(|passo| passo as u64)
}

fn agora_segundos() -> u64 { now_ms() / 1000 }

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
            }
            TotpAction::Confirm { code } => {
                if config.totp_secret.is_empty() { return Err(anyhow!("Gere o código do Authy primeiro")); }
                if totp_confere(&config.totp_secret, code.trim(), agora_segundos()).is_none() {
                    return Err(anyhow!("Código do Authy não confere. Confira se leu o QR desta tela e use o código atual."));
                }
                config.totp_confirmed = true;
            }
        }
        atomic_write_json(&state.state_file.with_file_name("mobile.json"),&*config)?;
    }
    settings(state,None,false)
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct PareamentoBody { codigo: String }

/// Troca um código atual do Authy pelo token de dispositivo.
///
/// Existe porque o app da tela inicial do iPhone **não enxerga** o armazenamento do Safari (decisão
/// da Apple, WebKit bug 181849): o token gravado pelo QR não chega nele. É rota pública — quem
/// chama é justamente quem ainda não tem token — então a trava de tentativas é o que segura força
/// bruta: 3 códigos válidos em 10^6 e 5 tentativas a cada 5 minutos.
async fn parear(State(state): State<Arc<EngineState>>, Json(body): Json<PareamentoBody>) -> Result<Json<Value>,ApiError> {
    tokio::task::spawn_blocking(move || parear_em(&state,&body.codigo,agora_segundos()))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha no pareamento"))?
}

fn parear_em(state: &EngineState, codigo: &str, agora: u64) -> Result<Json<Value>,ApiError> {
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
            Ok(Json(json!({"token":config.token})))
        }
        // Passo já usado conta como erro: é exatamente a tentativa de reaproveitar um código visto.
        _ => {
            pareamento.falhas.push_back(agora_ms);
            Err(error(StatusCode::UNAUTHORIZED,"Código do Authy não confere. Use o código que está aparecendo agora."))
        }
    }
}

pub fn start(state: Arc<EngineState>) {
    // A primeira leitura do histórico abre o começo de todo transcript de todas as contas: segundos,
    // numa máquina com muito histórico. Feita já na subida, quando ninguém está esperando, a primeira
    // abertura do celular ou da web encontra tudo pronto em vez de ficar carregando.
    if state.mobile.config.lock().map(|config| config.enabled).unwrap_or(false) {
        let aquecer = state.clone();
        tokio::task::spawn_blocking(move || { historical_entries(&aquecer); });
    }
    let worker = state.clone();
    tokio::spawn(async move {
        loop {
            worker.mobile.pending.notified().await;
            loop {
                let next = { let mut actions = worker.mobile.actions.lock().expect("actions poisoned");
                    actions.iter_mut().find(|a| a.state == "queued").map(|a| { a.state = "executing".into(); a.clone() }) };
                let Some(action) = next else { break };
                let engine = worker.clone(); let input = action.clone();
                let outcome = tokio::task::spawn_blocking(move || execute(&engine,&input)).await;
                let error = match outcome { Ok(Ok(())) => None, Ok(Err(e)) => Some(e.to_string()), Err(e) => Some(e.to_string()) };
                if let Some(record) = worker.mobile.actions.lock().expect("actions poisoned").iter_mut().find(|a| a.id == action.id) {
                    record.state = if error.is_some() { "rejected" } else { "sent" }.into(); record.error = error;
                }
            }
        }
    });
    tokio::spawn(async move {
        loop {
            let config = state.mobile.config.lock().expect("config poisoned").clone();
            *state.mobile.status.lock().expect("status poisoned") = (None,None);
            if !config.enabled { state.mobile.changed.notified().await; continue; }
            let mut checked = config.clone();
            let resultado = tokio::task::spawn_blocking(move || {
                let trocou = preferir_tailscale(&mut checked);
                (validate(&checked),checked,trocou)
            }).await;
            let (address,config) = match resultado {
                Ok((address,atualizada,trocou)) => {
                    if trocou {
                        // Configuração gravada com loopback (a padrão) se corrige sozinha na subida.
                        let _ = atomic_write_json(&state.state_file.with_file_name("mobile.json"),&atualizada);
                        *state.mobile.config.lock().expect("config poisoned") = atualizada.clone();
                    }
                    (Ok(address),atualizada)
                }
                Err(e) => (Err(e),config),
            };
            let listener = match address {
                Ok(Ok(address)) => TcpListener::bind(address).await.map_err(|e|e.to_string()),
                Ok(Err(e)) => Err(e.to_string()), Err(e) => Err(e.to_string()),
            };
            match listener {
                Ok(listener) => {
                    *state.mobile.status.lock().expect("status poisoned") = (Some(format!("http://{}",config.bind)),None);
                    publicar(&state,&config).await;
                    let shutdown = state.clone();
                    let _ = axum::serve(listener,router(state.clone())).with_graceful_shutdown(async move { shutdown.mobile.changed.notified().await; }).await;
                }
                Err(error) => { *state.mobile.status.lock().expect("status poisoned") = (None,Some(error)); state.mobile.changed.notified().await; }
            }
        }
    });
}

/// As rotas da API, sem o bundle e sem a checagem de origem/token do celular — esta parte é comum
/// ao servidor HTTP (`router`) e ao desktop (`local`).
fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/conversas",get(conversations))
        .route("/projetos",get(projects))
        .route("/painel",get(painel))
        .route("/conversas/{id}/timeline",get(timeline))
        .route("/conversas/{id}/terminal",get(terminal))
        .route("/conversas/{id}/teclas",post(teclas))
        .route("/conversas/{id}/prompt",post(prompt))
        .route("/conversas/{id}/aprovar",post(approve))
        .route("/conversas/{id}/nome",post(renomear))
        .route("/conversas/{id}/modo",post(modo))
        .route("/conversas/{id}/opcoes",get(opcoes))
        .route("/conversas/{id}/ajustes",post(ajustar))
        .route("/conversas/{id}/retomar",post(retomar_rota))
        .route("/contas",get(contas))
        .route("/terminais",get(listar_terminais).post(abrir_terminal))
        .route("/terminais/{id}/saida",get(saida_do_terminal))
        .route("/terminais/{id}/teclas",post(teclas_do_terminal))
        .route("/terminais/{id}/fechar",post(fechar_terminal))
        // O limite global (32 KB) é para JSON. Anexo tem o dele, aplicado **só** nesta rota: por
        // dentro da camada global, e o extrator lê o limite mais interno.
        .route("/conversas/{id}/anexos",post(anexar).layer(DefaultBodyLimit::max(ANEXO_MAX_BYTES)))
        .route("/atencao",get(attention))
        .route("/sessoes",post(create_session))
        .route("/parear",post(parear))
}

fn router(state: Arc<EngineState>) -> Router {
    rotas()
        .fallback(get(asset)).layer(DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(),same_origin)).with_state(state)
}

/// Uma chamada da API pelo protocolo local do engine (`EngineRequest::Api`). Quem chega aqui já
/// mostrou o token do engine, que vale mais que o de dispositivo: é o próprio desktop.
pub async fn local(state: Arc<EngineState>, method: &str, path: &str, body: Option<String>, body_base64: Option<String>,
    headers: Vec<(String, String)>) -> EngineResponse {
    let app = rotas().layer(DefaultBodyLimit::max(32 * 1024)).with_state(state);
    let corpo = match body_base64 {
        Some(codificado) => match base64_decode(&codificado) {
            Some(bytes) => axum::body::Body::from(bytes),
            None => return EngineResponse::Error { code: "INVALID_REQUEST".into(), message: "corpo base64 inválido".into() },
        },
        None => axum::body::Body::from(body.unwrap_or_default()),
    };
    let mut request = axum::http::Request::builder().method(method).uri(path);
    if !headers.iter().any(|(nome, _)| nome.eq_ignore_ascii_case("content-type")) {
        request = request.header("content-type","application/json");
    }
    for (nome, valor) in &headers { request = request.header(nome.as_str(), valor.as_str()); }
    let request = request.body(corpo);
    let Ok(request) = request else {
        return EngineResponse::Error { code: "INVALID_REQUEST".into(), message: format!("requisição inválida: {method} {path}") };
    };
    let response = match tower::ServiceExt::oneshot(app, request).await { Ok(response) => response, Err(never) => match never {} };
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024).await.unwrap_or_default();
    EngineResponse::Api { status, body: String::from_utf8_lossy(&bytes).into_owned() }
}

/// Base64 padrão (com ou sem `=`). Uma função evita uma dependência para um uso só: o anexo
/// colado no chat do PC, que atravessa o protocolo JSON do engine.
fn base64_decode(texto: &str) -> Option<Vec<u8>> {
    let valor = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'), b'a'..=b'z' => Some(c - b'a' + 26), b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62), b'/' => Some(63), _ => None,
    };
    let limpo: Vec<u8> = texto.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    if limpo.len() % 4 == 1 { return None; }
    let mut saida = Vec::with_capacity(limpo.len() * 3 / 4);
    for bloco in limpo.chunks(4) {
        let mut acumulado = 0_u32;
        for (i, c) in bloco.iter().enumerate() { acumulado |= u32::from(valor(*c)?) << (18 - 6 * i); }
        let bytes = acumulado.to_be_bytes();
        saida.extend_from_slice(&bytes[1..bloco.len()]);
    }
    Some(saida)
}

type ApiError = (StatusCode, Json<Value>);
fn error(status: StatusCode, message: &str) -> ApiError { (status,Json(json!({"error":message}))) }

/// Comparação de tempo constante: `==` em string sai no primeiro byte diferente, e isso vaza o
/// prefixo correto para quem conseguir medir. Três linhas resolvem sem dependência nova.
fn token_confere(recebido: &str, esperado: &str) -> bool {
    recebido.len() == esperado.len()
        && recebido.bytes().zip(esperado.bytes()).fold(0_u8,|acumulado,(a,b)| acumulado | (a ^ b)) == 0
}

/// Hosts aceitos. No modo serve o `Host` que chega é o nome MagicDNS, não o bind de loopback —
/// mas o nome continua sendo uma lista fechada, não um curinga: o check existe para barrar CSRF e
/// DNS rebinding, e aceitar qualquer `Host` desfaria exatamente isso.
fn hosts_aceitos(state: &EngineState) -> (Vec<String>, Vec<String>) {
    let config = state.mobile.config.lock().expect("config poisoned").clone();
    let mut hosts = vec![config.bind.clone()];
    let mut origens = vec![format!("http://{}",config.bind)];
    if config.serve {
        if let Some(nome) = state.mobile.magic_dns.lock().expect("magic poisoned").clone() {
            // O serve termina TLS na 443, então o `Host` chega sem porta.
            hosts.push(nome.clone());
            origens.push(format!("https://{nome}"));
        }
    }
    (hosts,origens)
}

async fn same_origin(State(state): State<Arc<EngineState>>, request: Request, next: Next) -> Response {
    let (hosts,origens) = hosts_aceitos(&state);
    let host = request.headers().get("host").and_then(|v|v.to_str().ok()).unwrap_or("");
    if !hosts.iter().any(|aceito| aceito == host) {
        // A mensagem nomeia a causa real: o Host tem que bater com o bind literal, então abrir pelo
        // nome MagicDNS do Tailscale cai aqui. Afrouxar a checagem não é opção — ela é a defesa
        // contra CSRF e DNS rebinding.
        return error(StatusCode::FORBIDDEN,"Host não permitido; abra pelo IP do Tailscale (100.x.x.x:porta), não pelo nome MagicDNS").into_response();
    }
    if request.method() != axum::http::Method::GET {
        let origin = request.headers().get("origin").and_then(|v|v.to_str().ok()).unwrap_or("");
        if !origens.iter().any(|aceita| aceita == origin) {
            return error(StatusCode::FORBIDDEN,"Origem não permitida").into_response();
        }
    }
    // Token de dispositivo, em cima do Tailscale. Deny-by-default com uma allow-list de duas
    // entradas: o bundle precisa carregar antes de existir token na mão do celular, e todo o resto
    // é API. Rota nova nasce protegida sem ninguém lembrar de protegê-la.
    let token = state.mobile.config.lock().expect("config poisoned").token.clone();
    let path = request.uri().path();
    let publico = caminho_publico(path);
    if !publico && !token.is_empty()
        && !token_confere(request.headers().get("x-omni-token").and_then(|v|v.to_str().ok()).unwrap_or(""),&token) {
        return error(StatusCode::UNAUTHORIZED,"Dispositivo não autorizado; leia o QR de novo no PC").into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("cache-control","no-store".parse().unwrap());
    headers.insert("x-content-type-options","nosniff".parse().unwrap());
    headers.insert("referrer-policy","no-referrer".parse().unwrap());
    // `style-src 'unsafe-inline'`: o espelho do terminal (xterm) cria o próprio `<style>` com as
    // cores e o tamanho da fonte. Script continua só `'self'` — é ele que segura injeção de código;
    // estilo inline no máximo muda a aparência, e o markdown das mensagens já passa por sanitize.
    headers.insert("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'".parse().unwrap());
    response
}

fn current(state: &EngineState, id: &str) -> Result<(omni_core::conversations::Conversation, Option<Arc<LiveSession>>), ApiError> {
    let dir = state.state_file.parent().unwrap();
    let conversation = omni_core::conversations::read_index(dir).into_iter().find(|c|c.id == id)
        .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
    let session = linked_session(state,&conversation);
    Ok((conversation,session))
}

fn linked_session(state: &EngineState, conversation: &omni_core::conversations::Conversation) -> Option<Arc<LiveSession>> {
    conversation.segments.last().and_then(|segment| {
        let sessions = state.sessions.lock().ok()?;
        let SessionEntry::Live(session) = sessions.get(segment.terminal_session_id.as_deref()?)? else { return None };
        let meta = session.meta.lock().ok()?;
        (meta.conversation_id.as_deref() == Some(conversation.id.as_str()) && meta.profile_id == segment.profile_id && meta.provider.as_deref() == Some(segment.provider.as_str()))
            .then(||session.clone())
    })
}

const HISTORY_REFRESH_MS: u64 = 5_000;

fn historical_entries(state: &EngineState) -> Vec<omni_core::history::HistoryEntry> {
    let now = now_ms();
    let mut history = state.mobile.history.lock().expect("history poisoned");
    if history.scanned_at_ms != 0 && now.saturating_sub(history.scanned_at_ms) < HISTORY_REFRESH_MS {
        return history.entries.clone();
    }
    let profiles = omni_core::profiles(state.state_file.parent().unwrap());
    let mut entries = omni_core::history::scan("claude", &profiles, &mut history.scan);
    entries.extend(omni_core::history::scan("codex", &profiles, &mut history.scan));
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.updated_at_ms));
    history.entries = entries;
    history.scanned_at_ms = now;
    history.entries.clone()
}

fn stable_id(prefix: &str, parts: &[&str]) -> String {
    // FNV-1a explícito: o id precisa continuar igual depois de reiniciar ou atualizar o engine;
    // `DefaultHasher` não promete algoritmo estável entre versões do Rust.
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{prefix}-{hash:016x}")
}

fn history_id(entry: &omni_core::history::HistoryEntry) -> String {
    stable_id("history", &[&entry.provider,&entry.profile_id,&entry.session_id])
}

fn history_project_id(cwd: &str) -> String {
    stable_id("history-project", &[&normalized_path(cwd)])
}

fn normalized_path(path: &str) -> String {
    let normalized = path.replace('\\',"/").trim_end_matches('/').to_lowercase();
    normalized.strip_prefix("//wsl$/").map_or(normalized.clone(),|rest|format!("//wsl.localhost/{rest}"))
}

fn same_path(left: &str, right: &str) -> bool {
    normalized_path(left) == normalized_path(right)
}

fn history_key(provider: &str, session_id: &str) -> String {
    format!("{provider}\0{session_id}")
}

/// Resumo de uma conversa do índice: o que a lista mostra e o que o chat precisa para saber se dá
/// para responder agora. Estado e capacidades saem da sessão viva, quando há.
fn resumo(state: &EngineState, c: &omni_core::conversations::Conversation) -> Value {
    let live = linked_session(state,c);
    let capabilities = live.as_ref().map(interaction::capabilities);
    let meta = live.as_ref().map(|s| settle(s));
    let status = meta.as_ref().map(|m| m.state.clone());
    // Identidade do último evento de atenção (fim de turno, pedido de aprovação). A tela usa para
    // saber se já foi vista: muda só quando acontece algo novo — nunca com redesenho da TUI. A
    // sessão entra junto porque o contador recomeça do zero a cada sessão nova.
    let attention = meta.as_ref().map(|m| format!("{}:{}", m.id, m.attention_seq));
    // `mode` sai da tela viva (o rodapé responde na hora); sessão fechada não tem modo.
    let mode = live.as_ref().and_then(interaction::modo_da_sessao);
    json!({"id":c.id,"title":omni_core::conversations::display_title(c),"project_id":c.project_id,"provider":c.segments.last().map(|s|&s.provider),
        "profile_id":c.segments.last().and_then(|s|s.profile_id.as_ref()),"state":status,"mode":mode,"capabilities":capabilities,
        "attention":attention,"live":live.is_some(),"updated_at_ms":atividade(c,live.as_ref())})
}

/// Último sinal de vida da conversa: saída do terminal, escrita no transcript ou a criação. É o que
/// ordena a lista — a conversa em que você está mexendo fica no topo, como num app de mensagens.
fn atividade(c: &omni_core::conversations::Conversation, live: Option<&Arc<LiveSession>>) -> u64 {
    let terminal = live.and_then(|s| s.meta.lock().ok().map(|meta| meta.last_activity_at_ms)).unwrap_or(0);
    let transcript = c.segments.last().and_then(|s| s.transcript_path.as_deref())
        .and_then(|path| omni_protocol::modified_ms(Path::new(path))).unwrap_or(0);
    c.created_at_ms.max(terminal).max(transcript)
}

/// Intervalo mínimo entre buscas de rollout do Codex sem caminho conhecido.
const CODEX_DESCOBERTA_MS: u64 = 15_000;

fn summaries(state: &EngineState) -> Vec<Value> {
    let dir = state.state_file.parent().unwrap();
    let mut index = omni_core::conversations::read_index(dir);
    let profiles = omni_core::profiles(dir);
    let descobrir = {
        let mut ultima = state.mobile.codex_descoberto_em.lock().expect("codex poisoned");
        let agora = now_ms();
        let vencida = agora.saturating_sub(*ultima) >= CODEX_DESCOBERTA_MS;
        if vencida { *ultima = agora; }
        vencida
    };
    if descobrir && omni_core::conversations::discover_codex_transcripts(&mut index, &profiles) {
        let _ = omni_core::conversations::write_index(dir, &index);
    }
    let projects = known_projects(state);
    let history = historical_entries(state);
    let mut known_sessions = HashSet::new();
    let mut known_transcripts = HashSet::new();
    let mut result = Vec::new();
    for c in &index {
        for segment in &c.segments {
            if let Some(session_id) = &segment.external_session_id {
                known_sessions.insert(history_key(&segment.provider,session_id));
            }
            if let Some(path) = &segment.transcript_path {
                known_transcripts.insert(path.replace('\\',"/").to_lowercase());
            }
        }
        let resumo = resumo(state,c);
        result.push((resumo["updated_at_ms"].as_u64().unwrap_or(c.created_at_ms),resumo));
    }
    for entry in history {
        let transcript = entry.path.replace('\\',"/").to_lowercase();
        if known_sessions.contains(&history_key(&entry.provider,&entry.session_id)) || known_transcripts.contains(&transcript) {
            continue;
        }
        let project_id = entry.cwd.as_deref()
            .and_then(|cwd|projects.iter().find(|project|same_path(&project.path,cwd)).map(|project|project.id.clone()))
            .unwrap_or_else(||history_project_id(entry.cwd.as_deref().unwrap_or("")));
        let title = entry.title.clone().or(entry.first_prompt.clone()).unwrap_or_else(||"Conversa anterior".into());
        result.push((entry.updated_at_ms,json!({"id":history_id(&entry),"title":title,"project_id":project_id,"provider":entry.provider,
            "profile_id":entry.profile_id,"state":Value::Null,"mode":Value::Null,"capabilities":Value::Null,"historical":true,
            "live":false,"updated_at_ms":entry.updated_at_ms})));
    }
    result.sort_by_key(|(updated,_)|std::cmp::Reverse(*updated));
    result.into_iter().map(|(_,summary)|summary).collect()
}

pub fn publish(state: &EngineState, projects: Vec<PublishedProject>, agents: Vec<PublishedAgent>, theme: Option<PublishedTheme>) -> Result<EngineResponse> {
    // Desktop antigo publica sem tema: mantém o último conhecido em vez de apagar.
    let theme = theme.or_else(|| state.mobile.workspace.lock().ok().and_then(|w| w.theme.clone()));
    let published = PublishedWorkspace { projects, agents, theme, published_at_ms: now_ms() };
    atomic_write_json(&state.state_file.with_file_name("workspace.json"),&published)?;
    *state.mobile.workspace.lock().map_err(|_|anyhow!("workspace poisoned"))? = published;
    Ok(EngineResponse::Ok)
}

/// Projetos que o celular pode escolher: os publicados pelo desktop, os do índice do OMNI e os
/// encontrados nos históricos nativos do Claude/Codex.
fn known_projects(state: &EngineState) -> Vec<PublishedProject> {
    let dir = state.state_file.parent().unwrap();
    let mut projects = state.mobile.workspace.lock().expect("workspace poisoned").projects.clone();
    for conversation in omni_core::conversations::read_index(dir) {
        if projects.iter().any(|p| p.id == conversation.project_id || same_path(&p.path,&conversation.cwd)) { continue }
        let name = Path::new(&conversation.cwd).file_name().map(|n|n.to_string_lossy().into_owned())
            .unwrap_or_else(|| conversation.cwd.clone());
        projects.push(PublishedProject { id: conversation.project_id.clone(), name, path: conversation.cwd.clone() });
    }
    for entry in historical_entries(state) {
        let cwd = entry.cwd.unwrap_or_default();
        if projects.iter().any(|project|same_path(&project.path,&cwd)) { continue }
        let name = Path::new(&cwd).file_name().filter(|name|!name.is_empty()).map(|name|name.to_string_lossy().into_owned())
            .unwrap_or_else(||"Histórico".into());
        projects.push(PublishedProject { id: history_project_id(&cwd), name, path: cwd });
    }
    projects
}

fn catalogo(state: &EngineState) -> Value {
    let dir = state.state_file.parent().unwrap();
    let (agents,published_at_ms,theme) = {
        let workspace = state.mobile.workspace.lock().expect("workspace poisoned");
        (workspace.agents.clone(),workspace.published_at_ms,workspace.theme.clone())
    };
    // `config_dir` fica de fora de propósito: é caminho de credencial e o celular não precisa.
    let profiles: Vec<_> = omni_core::profiles(dir).into_iter()
        .map(|p| json!({"id":p.id,"provider":p.provider,"name":p.name})).collect();
    json!({"published_at_ms":published_at_ms,"projects":known_projects(state),"agents":agents,"profiles":profiles,"theme":theme})
}

async fn projects(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || catalogo(&state))
        .await.unwrap_or(json!({"published_at_ms":0,"projects":[],"agents":[],"profiles":[]})))
}

async fn conversations(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || json!(summaries(&state))).await.unwrap_or(json!([])))
}
async fn attention(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || json!(summaries(&state).into_iter().filter(|c|
        c["capabilities"]["approve"] == true || c["capabilities"]["prompt"] == true || c["state"] == "approval_required").collect::<Vec<_>>())).await.unwrap_or(json!([])))
}

/// Resposta JSON com `ETag`. Se o cliente já tem exatamente esse conteúdo (`If-None-Match`), volta
/// `304` sem corpo: a lista e a conversa aberta são consultadas a cada poucos segundos e, quase
/// sempre, nada mudou — sem isto cada consulta remontava a tela inteira à toa.
fn com_etag(headers: &HeaderMap, corpo: &Value) -> Response {
    let texto = corpo.to_string();
    let etag = format!("\"{:016x}\"", texto.bytes().fold(0xcbf29ce484222325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3)));
    if headers.get("if-none-match").and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return (StatusCode::NOT_MODIFIED, [("etag", etag)]).into_response();
    }
    (StatusCode::OK, [("etag", etag), ("content-type", "application/json".into())], texto).into_response()
}

/// Lista de conversas e catálogo numa chamada só. Antes eram três (`/conversas`, `/atencao` e
/// `/projetos`), cada uma montando a lista inteira de novo — o "precisa de você" agora é calculado
/// no cliente a partir da mesma lista.
async fn painel(State(state): State<Arc<EngineState>>, headers: HeaderMap) -> Response {
    let corpo = tokio::task::spawn_blocking(move || json!({"conversations":summaries(&state),"catalog":catalogo(&state)}))
        .await.unwrap_or(json!({"conversations":[],"catalog":{"published_at_ms":0,"projects":[],"agents":[],"profiles":[]}}));
    com_etag(&headers, &corpo)
}

/// Sem `cursor` a timeline abre no fim da conversa, como um chat.
#[derive(Deserialize)] struct Page { #[serde(default)] cursor: Option<usize> }
/// Itens por página. Ações do agente contam como item, então é maior que uma página só de texto.
const TIMELINE_PAGINA: usize = 150;
async fn timeline(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(page): Query<Page>, headers: HeaderMap) -> Response {
    let resultado = tokio::task::spawn_blocking(move || -> Result<Value, ApiError> {
        let dir = state.state_file.parent().unwrap();
        let profiles = omni_core::profiles(dir);
        if let Ok((conversation,_)) = current(&state,&id) {
            let timeline = {
                let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
                omni_core::conversations::timeline_cached(&conversation,&profiles,page.cursor,TIMELINE_PAGINA,&mut feeds)
            };
            let actions: Vec<_> = state.mobile.actions.lock().expect("actions poisoned").iter().filter(|a|a.conversation_id == id).cloned().collect();
            // O estado da conversa vem junto: o chat aberto não precisa recarregar a lista inteira a
            // cada 1,5 s só para saber se o agente terminou.
            return Ok(json!({"timeline":timeline,"actions":actions,"conversation":resumo(&state,&conversation)}));
        }
        let entry = historical_entries(&state).into_iter().find(|entry|history_id(entry) == id)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
        let caminho = omni_core::history::validated(Path::new(&entry.path),&entry.provider,&profiles)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Histórico não encontrado"))?;
        let itens: Vec<_> = {
            let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
            feeds.read(&caminho,&entry.provider).map_err(|_|error(StatusCode::NOT_FOUND,"Histórico não encontrado"))?
                .iter().map(|e|(entry.provider.clone(),e.item.clone())).collect()
        };
        let timeline = omni_core::conversations::page(&id,itens,page.cursor,TIMELINE_PAGINA);
        Ok(json!({"timeline":timeline,"actions":[]}))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler timeline"));
    match resultado {
        Ok(Ok(corpo)) => com_etag(&headers,&corpo),
        Ok(Err(erro)) | Err(erro) => erro.into_response(),
    }
}

/// Quanto do histórico do terminal vai na primeira leitura. O engine guarda até 5 MB; mandar tudo
/// pela rede para desenhar uma tela é desperdício — 512 KB cobrem bem mais que a tela visível.
const TERMINAL_INICIAL_BYTES: usize = 512 * 1024;
#[derive(Deserialize)] struct Desde { #[serde(default)] desde: Option<u64> }

/// O terminal da conversa como o PC desenha: a saída bruta da PTY a partir de `desde`.
///
/// Só leitura de propósito — **não** marca a sessão como exibida (`headless`): quem responde às
/// perguntas do terminal continua sendo o xterm do desktop ou o próprio engine. Se a página também
/// respondesse, a resposta chegaria em dobro e vazaria como texto digitado dentro da CLI.
async fn terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(desde): Query<Desde>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let (_, sessao) = current(&state,&id)?;
        let Some(sessao) = sessao else {
            return Ok(Json(json!({"live":false,"data":"","from_seq":0,"next_seq":0,"cols":0,"rows":0})));
        };
        ler_saida(&sessao, desde.desde)
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler o terminal"))?
}

/// A saída bruta da PTY a partir de `desde` — comum ao terminal de uma conversa e a um shell.
fn ler_saida(sessao: &Arc<LiveSession>, desde: Option<u64>) -> Result<Json<Value>, ApiError> {
    {
        let (cols, rows) = sessao.meta.lock().map(|meta| (meta.cols, meta.rows)).unwrap_or((120, 40));
        let saida = sessao.output.lock().map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Terminal indisponível"))?;
        let pedido = desde.unwrap_or(0);
        // Primeira leitura (ou cliente que ficou para trás do buffer): só o fim, começando num bloco
        // inteiro. Daí em diante vem só o que é novo.
        let mut inicio = saida.chunks.front().map(|(seq,_)| *seq).unwrap_or(saida.next_seq).max(pedido);
        if pedido == 0 || saida.chunks.front().is_some_and(|(seq,_)| *seq > pedido) {
            let mut bytes = 0;
            inicio = saida.next_seq;
            for (seq, bloco) in saida.chunks.iter().rev() {
                if bytes + bloco.len() > TERMINAL_INICIAL_BYTES && bytes > 0 { break; }
                bytes += bloco.len();
                inicio = *seq;
            }
        }
        let data: String = saida.chunks.iter().filter(|(seq,_)| *seq >= inicio).map(|(_,bloco)| bloco.as_str()).collect();
        Ok(Json(json!({"live":true,"data":data,"from_seq":inicio,"next_seq":saida.next_seq,"cols":cols,"rows":rows,
            "reset":inicio != pedido})))
    }
}

/* ── Terminais sem agente (shells) ───────────────────────────────────────────────────────────
   Para quem está no celular ou no outro computador conseguir rodar um `npm run dev` num projeto.
   A pasta sai **sempre** da lista de projetos: o navegador escolhe o projeto, nunca um caminho. */

/// Um shell vivo pelo id. Sessão de agente não serve: aquela se lê pela conversa.
fn shell_vivo(state: &EngineState, id: &str) -> Result<Arc<LiveSession>, ApiError> {
    let sessoes = state.sessions.lock().map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Sessões indisponíveis"))?;
    match sessoes.get(id) {
        Some(SessionEntry::Live(sessao)) if sessao.meta.lock().map(|meta| meta.provider.is_none()).unwrap_or(false) => Ok(sessao.clone()),
        _ => Err(error(StatusCode::NOT_FOUND,"Terminal não está aberto no PC")),
    }
}

async fn listar_terminais(State(state): State<Arc<EngineState>>) -> Json<Value> {
    let lista: Vec<Value> = state.sessions.lock().map(|sessoes| sessoes.values().filter_map(|entrada| match entrada {
        SessionEntry::Live(sessao) => {
            let meta = settle(sessao);
            meta.provider.is_none().then(|| json!({"id":meta.id,"name":meta.name,"project_id":meta.project_id,"cwd":meta.cwd,
                "state":meta.state,"created_at_ms":meta.created_at_ms,"updated_at_ms":meta.last_activity_at_ms}))
        }
        SessionEntry::Historical(_) => None,
    }).collect()).unwrap_or_default();
    Json(json!(lista))
}

async fn saida_do_terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(desde): Query<Desde>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || ler_saida(&shell_vivo(&state,&id)?, desde.desde))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler o terminal"))?
}

async fn teclas_do_terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<TeclasBody>) -> Result<Json<Value>, ApiError> {
    if body.dados.is_empty() || body.dados.len() > TECLAS_MAX {
        return Err(error(StatusCode::BAD_REQUEST,"Entrada vazia ou grande demais"));
    }
    tokio::task::spawn_blocking(move || {
        shell_vivo(&state,&id)?;
        write_terminal(&state,&id,&body.dados).map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao enviar teclas"))?
}

async fn fechar_terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        shell_vivo(&state,&id)?;
        close_session(&state,&id).map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao fechar o terminal"))?
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct AbrirTerminalBody { project_id: String }
async fn abrir_terminal(State(state): State<Arc<EngineState>>, Json(body): Json<AbrirTerminalBody>) -> Result<(StatusCode,Json<Value>), ApiError> {
    tokio::task::spawn_blocking(move || {
        let projeto = known_projects(&state).into_iter().find(|projeto| projeto.id == body.project_id && !projeto.path.is_empty())
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Projeto não encontrado neste PC"))?;
        if !Path::new(&projeto.path).is_dir() { return Err(error(StatusCode::CONFLICT,"A pasta do projeto não existe mais neste PC")); }
        if state.sessions.lock().expect("sessions poisoned").values()
            .filter(|entry| matches!(entry,SessionEntry::Live(_))).count() >= MAX_SESSOES {
            return Err(error(StatusCode::TOO_MANY_REQUESTS,"Muitas sessões abertas neste PC; feche alguma antes"));
        }
        // Headless: ninguém exibe esta PTY num terminal de verdade até o desktop abrir a aba.
        let resposta = spawn_terminal(&state, projeto.id, "Terminal".into(), projeto.path, None, None, 32, 120,
            SessionOrigin { headless: true, ..SessionOrigin::default() })
            .map_err(|erro|error(StatusCode::INTERNAL_SERVER_ERROR,&erro.to_string()))?;
        let EngineResponse::Session { session } = resposta else { return Err(error(StatusCode::INTERNAL_SERVER_ERROR,"Resposta inesperada")); };
        Ok((StatusCode::CREATED,Json(json!({"id":session.id,"name":session.name,"project_id":session.project_id}))))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao abrir o terminal"))?
}

/// Teclas mandadas pelo terminal da página — o mesmo que digitar no terminal do PC (Esc para
/// interromper, setas e Enter num menu da CLI). Limite pequeno: é teclado, não colagem de arquivo.
#[derive(Deserialize)] struct TeclasBody { dados: String }
const TECLAS_MAX: usize = 8 * 1024;
async fn teclas(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<TeclasBody>) -> Result<Json<Value>, ApiError> {
    if body.dados.is_empty() || body.dados.len() > TECLAS_MAX {
        return Err(error(StatusCode::BAD_REQUEST,"Entrada vazia ou grande demais"));
    }
    tokio::task::spawn_blocking(move || {
        let (_, sessao) = current(&state,&id)?;
        let sessao = sessao.ok_or_else(||error(StatusCode::CONFLICT,"Sessão não está aberta no PC"))?;
        let sessao_id = sessao.meta.lock().map(|meta| meta.id.clone()).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Sessão indisponível"))?;
        write_terminal(&state,&sessao_id,&body.dados).map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao enviar teclas"))?
}
/// Renomear a conversa. Vale para o celular e para a web de computador; o desktop chega aqui pelo
/// `EngineRequest::RenameConversation`, para o índice ter **um** escritor só.
#[derive(Deserialize)] struct NomeBody { nome: String }
async fn renomear(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<NomeBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap();
        let conversa = omni_core::conversations::rename(dir,&id,&body.nome)
            .map_err(|erro| error(StatusCode::BAD_REQUEST,&erro))?;
        Ok(Json(json!({"id":conversa.id,"title":conversa.title})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao renomear"))?
}

/// Tamanho máximo de um anexo vindo do celular. Cobre foto de câmera, PDF e print com folga, e
/// ainda cabe na memória sem susto — o corpo é lido inteiro antes de gravar.
const ANEXO_MAX_BYTES: usize = 25 * 1024 * 1024;
/// Pasta dos anexos, dentro do projeto da conversa. Dentro do projeto de propósito: a CLI lê sem
/// pedir permissão extra, e é a mesma família do `.omni-agents/pasted/` que o desktop já usa.
const PASTA_ANEXOS: [&str; 2] = [".omni-agents", "anexos"];

/// Nome seguro para gravar: só o último componente, sem nada que o shell ou o Windows interpretem.
/// O caminho vai cru no prompt, então espaço e vírgula também saem — a CLI leria só o pedaço.
fn nome_seguro(bruto: &str) -> String {
    let base = bruto.rsplit(['/', '\\']).next().unwrap_or("");
    let limpo: String = base.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let limpo = limpo.trim_matches(|c| c == '.' || c == '-');
    // Nome reservado do Windows (`CON`, `NUL`…) ou vazio vira genérico em vez de falhar na escrita.
    let tronco = limpo.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reservado = matches!(tronco.as_str(), "CON"|"PRN"|"AUX"|"NUL") || ((tronco.starts_with("COM") || tronco.starts_with("LPT"))
        && tronco.len() == 4 && tronco.as_bytes()[3].is_ascii_digit());
    let limpo = if limpo.is_empty() || reservado { "anexo" } else { limpo };
    // Nome longo quebra caminho no Windows (MAX_PATH) sem ganho nenhum: corta, preservando a extensão.
    match limpo.rsplit_once('.') {
        Some((tronco, ext)) if !tronco.is_empty() && ext.len() <= 10 => format!("{}.{}", &tronco[..tronco.len().min(60)], ext),
        _ => limpo[..limpo.len().min(70)].to_owned(),
    }
}

async fn anexar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, corpo: axum::body::Bytes) -> Result<Json<Value>,ApiError> {
    if corpo.is_empty() { return Err(error(StatusCode::BAD_REQUEST,"Arquivo vazio")); }
    let nome = headers.get("x-omni-nome").and_then(|v| v.to_str().ok())
        .map(|v| percent_decode(v)).unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        // A pasta sai da conversa registrada no servidor — o celular nunca manda caminho.
        let (conversation,_) = current(&state,&id)?;
        let mut pasta = std::path::PathBuf::from(&conversation.cwd);
        if !pasta.is_dir() { return Err(error(StatusCode::CONFLICT,"A pasta do projeto não existe mais neste PC")); }
        for parte in PASTA_ANEXOS { pasta.push(parte); }
        std::fs::create_dir_all(&pasta).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR,"Não foi possível criar a pasta de anexos"))?;
        // Prefixo com data e sequência: dois "image.png" colados seguidos não se sobrescrevem.
        let seguro = nome_seguro(&nome);
        let destino = pasta.join(format!("{}-{:04x}-{}", now_ms(), uuid::Uuid::new_v4().as_u128() as u16, seguro));
        let mut arquivo = std::fs::OpenOptions::new().write(true).create_new(true).open(&destino)
            .map_err(|_| error(StatusCode::CONFLICT,"Já existe um anexo com esse nome; tente de novo"))?;
        use std::io::Write as _;
        if arquivo.write_all(&corpo).is_err() {
            drop(arquivo);
            let _ = std::fs::remove_file(&destino);
            return Err(error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao gravar o anexo no PC"));
        }
        Ok(Json(json!({"caminho":destino.to_string_lossy(),"nome":seguro,"bytes":corpo.len()})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao gravar o anexo"))?
}

/// `X-Omni-Nome` chega com `encodeURIComponent` (cabeçalho HTTP não carrega acento). Sequência
/// inválida vira o próprio texto: o nome ainda passa pelo `nome_seguro` depois.
fn percent_decode(valor: &str) -> String {
    let bytes = valor.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // Bytes, não `&valor[..]`: fatiar a `str` no meio de um caractere acentuado dá pânico.
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let par = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|hex| u8::from_str_radix(hex, 16).ok());
            if let Some(byte) = par { saida.push(byte); i += 3; continue; }
        }
        saida.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct PromptBody { texto: String }
#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct ApprovalBody { permitir: bool }
async fn prompt(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<PromptBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    if body.texto.trim().is_empty() || body.texto.len() > 16_000 || body.texto.chars().any(|c|c.is_control() && c != '\n' && c != '\t') || body.texto.trim_start().starts_with('/') {
        return Err(error(StatusCode::BAD_REQUEST,"Envie texto de até 16000 bytes, sem comandos slash ou caracteres de controle"));
    }
    enqueue_async(state,id,headers,ActionKind::Prompt(body.texto)).await
}
async fn approve(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<ApprovalBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    enqueue_async(state,id,headers,ActionKind::Approval(body.permitir)).await
}

/// Modo de permissão, o que o `shift+tab` faz no terminal. Só `plan` e `auto`: são os dois que o
/// usuário troca no dia a dia, e um seletor de quatro estados numa tela de celular seria pior.
#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct ModoBody { modo: String }
async fn modo(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<ModoBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    if body.modo != "plan" && body.modo != "auto" {
        return Err(error(StatusCode::BAD_REQUEST,"Modo deve ser plan ou auto"));
    }
    enqueue_async(state,id,headers,ActionKind::Modo(body.modo)).await
}
/// `deny_unknown_fields` é peça de segurança, não capricho: um `cwd` contrabandeado no corpo vira
/// **400**, em vez de um campo ignorado em silêncio que dá a falsa impressão de ter sido aceito.
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct SpawnBody { project_id: String, provider: String, profile_id: Option<String>, #[serde(default)] titulo: Option<String> }

/// Teto de sessões vivas abertas pelo celular somadas às do desktop. Sem ele, um toque repetido no
/// botão enche o PC de PTYs.
/// ponytail: número fixo; se incomodar, vira configuração na tela de Celular.
const MAX_SESSOES: usize = 16;

/// Resolve o pedido do celular contra o que o servidor já conhece. Nada aqui sai do corpo do
/// request além de escolhas dentro de listas fechadas.
fn resolve_spawn(state: &Arc<EngineState>, body: &SpawnBody) -> Result<SpawnPlan, ApiError> {
    let dir = state.state_file.parent().unwrap();

    let project = known_projects(state).into_iter().find(|p| p.id == body.project_id)
        .ok_or_else(||error(StatusCode::CONFLICT,"Projeto desconhecido; abra o app no PC uma vez para publicar a lista"))?;
    if !Path::new(&project.path).is_dir() {
        return Err(error(StatusCode::CONFLICT,"A pasta do projeto não existe mais neste PC"));
    }

    // O comando vem da lista publicada: o celular escolhe um id, nunca nomeia um binário.
    let agent = state.mobile.workspace.lock().expect("workspace poisoned").agents.iter()
        .find(|a| a.id == body.provider).cloned()
        .ok_or_else(||error(StatusCode::CONFLICT,"CLI não publicada pelo desktop; abra o app no PC uma vez"))?;

    let profiles = omni_core::profiles(dir);
    let profile = match &body.profile_id {
        Some(id) => Some(profiles.iter().find(|p| &p.id == id && p.provider == body.provider)
            .ok_or_else(||error(StatusCode::CONFLICT,"Conta desconhecida para este agente"))?.clone()),
        None => profiles.iter().find(|p| p.provider == body.provider && p.builtin).cloned(),
    };

    let titulo_do_usuario = body.titulo.is_some();
    let titulo = body.titulo.clone().unwrap_or_else(|| format!("{} · celular",agent.label));
    if titulo.len() > 120 || titulo.chars().any(|c| c.is_control()) {
        return Err(error(StatusCode::BAD_REQUEST,"Título inválido"));
    }

    if state.sessions.lock().expect("sessions poisoned").values()
        .filter(|entry| matches!(entry,SessionEntry::Live(_))).count() >= MAX_SESSOES {
        return Err(error(StatusCode::TOO_MANY_REQUESTS,"Muitas sessões abertas neste PC; feche alguma antes"));
    }

    // Fixar o `--session-id` do Claude aqui é o que torna o caminho do transcript conhecido de
    // imediato — sem isso a timeline do celular ficaria vazia até alguém descobrir o arquivo.
    let (command,external_session_id,transcript_path) = if body.provider == "claude" {
        let session_id = uuid::Uuid::new_v4().to_string();
        let path = profile.as_ref().map(|p| omni_core::conversations::claude_transcript_path(
            Path::new(&p.config_dir),&project.path,&session_id).to_string_lossy().into_owned());
        (format!("{} --session-id {session_id}",agent.command),Some(session_id),path)
    } else {
        (agent.command.clone(),None,None)
    };

    Ok(SpawnPlan {
        project_id: project.id, cwd: project.path, title: titulo, titulo_do_usuario, provider: body.provider.clone(),
        profile_id: profile.as_ref().map(|p| p.id.clone()),
        env: profile.as_ref().map(omni_core::env_for).unwrap_or_default(),
        command, external_session_id, transcript_path,
    })
}

async fn create_session(State(state): State<Arc<EngineState>>, headers: HeaderMap, Json(body): Json<SpawnBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    tokio::task::spawn_blocking(move || {
        let plan = resolve_spawn(&state,&body)?;
        enqueue(&state,"",&headers,ActionKind::Spawn(plan))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha na fila"))?
}

async fn enqueue_async(state: Arc<EngineState>,id:String,headers:HeaderMap,kind:ActionKind) -> Result<(StatusCode,Json<Value>),ApiError> {
    tokio::task::spawn_blocking(move || enqueue(&state,&id,&headers,kind)).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha na fila"))?
}
fn enqueue(state:&Arc<EngineState>,id:&str,headers:&HeaderMap,kind:ActionKind) -> Result<(StatusCode,Json<Value>),ApiError> {
    let key = headers.get("idempotency-key").and_then(|h|h.to_str().ok()).filter(|s|!s.is_empty() && s.len() <= 128)
        .ok_or_else(||error(StatusCode::BAD_REQUEST,"Idempotency-Key obrigatório"))?;
    let mut actions = state.mobile.actions.lock().expect("actions poisoned");
    if let Some(action) = actions.iter().find(|a| a.key == key) {
        if action.conversation_id != id || action.kind != kind { return Err(error(StatusCode::CONFLICT,"Chave já usada para outra ação")); }
        return Ok((StatusCode::ACCEPTED,Json(json!(action))));
    }
    actions.retain(|a| now_ms().saturating_sub(a.created_at_ms) < 600_000 || a.state == "queued" || a.state == "executing");
    if actions.len() >= 1024 || actions.iter().filter(|a|a.state == "queued").count() >= 128 {
        return Err(error(StatusCode::TOO_MANY_REQUESTS,"Fila cheia; aguarde"));
    }
    // Spawn não tem tela viva para ficar obsoleta: pula `capabilities` e `If-Match`, mas mantém
    // idempotência, limite de fila e expiração — o que impede um toque repetido de abrir duas PTYs.
    if matches!(kind, ActionKind::Spawn(_) | ActionKind::Retomar(_)) {
        // Retomar devolve a conversa que vai nascer (ou voltar): a tela navega para ela.
        let conversation_id = if matches!(kind, ActionKind::Retomar(_)) { id.to_string() } else { String::new() };
        let action = Action { id: state.next_id(),conversation_id,session_id:String::new(),
            state:"queued".into(),created_at_ms:now_ms(),error:None,key:key.into(),revision:String::new(),
            kind:kind.clone() };
        let response = json!(action); actions.push_back(action); drop(actions);
        state.mobile.pending.notify_one();
        return Ok((StatusCode::ACCEPTED,Json(response)));
    }
    let (_,session) = current(state,id)?;
    let session = session.ok_or_else(||error(StatusCode::CONFLICT,"Nenhuma sessão existente para esta conversa"))?;
    let capability = interaction::capabilities(&session);
    let supported = match kind {
        // Trocar de modo é tecla no composer: vale a mesma porta do prompt (CLI ocioso e tela
        // reconhecida). Com o agente no meio de um turno, o shift+tab não é para ele.
        // Reabrir no meio de um turno mataria o trabalho em andamento.
        ActionKind::Prompt(_) | ActionKind::Modo(_) | ActionKind::Ajustes(_) => capability.prompt,
        ActionKind::Approval(_) => capability.approve,
        ActionKind::Spawn(_) | ActionKind::Retomar(_) => false,
    };
    if !supported { return Err(error(StatusCode::CONFLICT,"Sessão ocupada ou tela não reconhecida; atualize o status")); }
    if headers.get("if-match").and_then(|h|h.to_str().ok()) != Some(capability.revision.as_str()) {
        return Err(error(StatusCode::PRECONDITION_FAILED,"O contexto mudou; atualize antes de responder"));
    }
    let session_id = session.meta.lock().expect("metadata poisoned").id.clone();
    let action = Action { id: state.next_id(),conversation_id:id.into(),session_id,state:"queued".into(),created_at_ms:now_ms(),error:None,key:key.into(),revision:capability.revision,kind };
    let response = json!(action); actions.push_back(action); drop(actions);
    state.mobile.pending.notify_one();
    Ok((StatusCode::ACCEPTED,Json(response)))
}

fn execute(state:&Arc<EngineState>, action:&Action) -> Result<()> {
    match &action.kind {
        ActionKind::Spawn(plan) => spawn_from_mobile(state,action,plan),
        // O modo tem laço próprio: aperta, relê a tela e repete, então não pode segurar os locks
        // como o caminho de prompt/aprovação segura.
        ActionKind::Modo(alvo) => {
            let (_,session) = current(state,&action.conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
            let session = session.ok_or_else(||anyhow!("Sessão encerrada"))?;
            interaction::alternar_modo(state,&session,alvo).map(|_|())
        }
        ActionKind::Ajustes(ajustes) => reabrir_com(state,&action.conversation_id,ajustes),
        ActionKind::Retomar(plano) => retomar(state,action,plano),
        _ => execute_checked(state,action,interaction::provider_running),
    }
}

/* ── Conta, modelo e esforço ─────────────────────────────────────────────────────────────────
   Trocar qualquer um dos três é a mesma operação: reabrir a **mesma** sessão (mesmo id — a aba do
   PC continua ligada nela) com `--resume` e os parâmetros novos. As duas CLIs aceitam modelo e
   esforço na linha de comando; trocar por comando interno (`/model`) muda de versão para versão.
   Trocar de conta leva o transcript junto para o config dir da conta nova, como o failover. */

const MODELOS_CLAUDE: [(&str, &str); 4] = [("fable","Fable"),("opus","Opus"),("sonnet","Sonnet"),("haiku","Haiku")];
const ESFORCOS_CLAUDE: [&str; 5] = ["low","medium","high","xhigh","max"];

/// Só letras, números e `._-[]`: o valor vira argumento de um comando digitado num shell.
fn valor_seguro(valor: &str) -> bool {
    !valor.is_empty() && valor.len() <= 64 && valor.chars().all(|c| c.is_ascii_alphanumeric() || "._-[]".contains(c))
}

/// Modelos do Codex que a conta enxerga (`models_cache.json` do CODEX_HOME), com os esforços de cada um.
fn modelos_codex(config_dir: &Path) -> Vec<Value> {
    let Ok(bytes) = std::fs::read(config_dir.join("models_cache.json")) else { return Vec::new() };
    let Ok(cache) = serde_json::from_slice::<Value>(&bytes) else { return Vec::new() };
    cache["models"].as_array().into_iter().flatten()
        .filter(|modelo| modelo["visibility"].as_str() != Some("hide"))
        .filter_map(|modelo| {
            let id = modelo["slug"].as_str()?;
            let esforcos: Vec<&str> = modelo["supported_reasoning_levels"].as_array().into_iter().flatten()
                .filter_map(|nivel| nivel["effort"].as_str().or_else(|| nivel.as_str())).collect();
            Some(json!({"id":id,"label":modelo["display_name"].as_str().unwrap_or(id),"efforts":esforcos}))
        }).collect()
}

/// Agente, conta, pasta e id nativo da última sessão — do índice do OMNI ou do histórico da CLI.
fn ultima_sessao(state: &EngineState, id: &str) -> Result<omni_core::conversations::Segment, ApiError> {
    if let Ok((conversa,_)) = current(state,id) {
        return conversa.segments.last().cloned().ok_or_else(||error(StatusCode::CONFLICT,"Conversa sem sessão"));
    }
    let entrada = historical_entries(state).into_iter().find(|entrada| history_id(entrada) == id)
        .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
    Ok(omni_core::conversations::Segment {
        provider: entrada.provider, profile_id: Some(entrada.profile_id), external_session_id: Some(entrada.session_id),
        transcript_path: Some(entrada.path), terminal_session_id: None, started_at_ms: 0, ended_at_ms: None,
    })
}

fn pasta_da_conversa(state: &EngineState, id: &str) -> Option<String> {
    current(state,id).map(|(c,_)| c.cwd).ok()
        .or_else(|| historical_entries(state).into_iter().find(|e| history_id(e) == id).and_then(|e| e.cwd))
}

fn opcoes_da_conversa(state: &EngineState, id: &str) -> Result<Value, ApiError> {
    let dir = state.state_file.parent().unwrap();
    let segmento = ultima_sessao(state,id)?;
    let cwd = pasta_da_conversa(state,id).unwrap_or_default();
    let segmento = &segmento;
    let perfis: Vec<_> = omni_core::profiles(dir).into_iter().filter(|p| p.provider == segmento.provider).collect();
    let atual = perfis.iter().find(|p| Some(&p.id) == segmento.profile_id.as_ref());
    let runtime = match (segmento.provider.as_str(), atual) {
        ("claude", Some(perfil)) => segmento.external_session_id.as_deref()
            .and_then(|sid| omni_core::runtime::claude_runtime(Path::new(&perfil.config_dir),&cwd,sid)),
        ("codex", Some(perfil)) => omni_core::runtime::codex_runtime(Path::new(&perfil.config_dir),&cwd),
        _ => None,
    };
    let modelos: Vec<Value> = match segmento.provider.as_str() {
        "claude" => MODELOS_CLAUDE.iter().map(|(id,label)| json!({"id":id,"label":label,"efforts":ESFORCOS_CLAUDE})).collect(),
        "codex" => atual.map(|perfil| modelos_codex(Path::new(&perfil.config_dir))).unwrap_or_default(),
        _ => Vec::new(),
    };
    Ok(json!({
        "provider": segmento.provider,
        "profile_id": segmento.profile_id,
        "model": runtime.as_ref().and_then(|r| r.model.clone()),
        "effort": runtime.as_ref().and_then(|r| r.effort.clone()),
        "profiles": perfis.iter().map(|p| json!({"id":p.id,"name":p.name})).collect::<Vec<_>>(),
        "models": modelos,
        "can_resume": segmento.external_session_id.is_some(),
    }))
}

async fn opcoes(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || opcoes_da_conversa(&state,&id).map(Json))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler as opções"))?
}

async fn ajustar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<Ajustes>) -> Result<(StatusCode,Json<Value>),ApiError> {
    for valor in [&body.model, &body.effort, &body.profile_id].into_iter().flatten() {
        if !valor_seguro(valor) { return Err(error(StatusCode::BAD_REQUEST,"Valor inválido")); }
    }
    enqueue_async(state,id,headers,ActionKind::Ajustes(body)).await
}

/// Uso das contas como o rodapé do PC mostra (janela de 5 h e semanal). Só o que já foi lido —
/// consultar de novo digitaria `/usage` num agente, e isso fica com o PC.
async fn contas(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap().to_path_buf();
        let agora = now_ms();
        let lista: Vec<Value> = omni_core::profiles(&dir).into_iter().map(|perfil| {
            let uso = match perfil.provider.as_str() {
                "codex" => Some(omni_core::usage::read_codex(&perfil.id,Path::new(&perfil.config_dir),agora)),
                "claude" => omni_core::usage::read_claude_cache(&perfil.id,&omni_core::usage::claude_state_path(&perfil),agora),
                _ => None,
            };
            json!({"id":perfil.id,"provider":perfil.provider,"name":perfil.name,"usage":uso})
        }).collect();
        json!(lista)
    }).await.unwrap_or(json!([])))
}

/// O comando da CLI como o desktop publicou (`claude`, `codex`, ou um caminho), ou o id do agente.
pub(crate) fn comando_do_agente(state: &EngineState, provider: &str) -> String {
    state.mobile.workspace.lock().ok()
        .and_then(|w| w.agents.iter().find(|agente| agente.id == provider).map(|agente| agente.command.clone()))
        .unwrap_or_else(|| provider.to_string())
}

/// Linha de comando que retoma a conversa com o modelo e o esforço pedidos.
fn comando_de_retomada(base: &str, provider: &str, sessao: &str, ajustes: &Ajustes) -> Result<String> {
    let mut comando = match provider {
        "claude" => format!("{base} --resume {sessao}"),
        "codex" => format!("{base} resume {sessao}"),
        _ => return Err(anyhow!("Este agente não permite trocar conta, modelo ou esforço")),
    };
    if let Some(modelo) = &ajustes.model {
        comando += &if provider == "claude" { format!(" --model {modelo}") } else { format!(" -m {modelo}") };
    }
    if let Some(esforco) = &ajustes.effort {
        // Sem aspas de propósito: o `-c` do Codex lê o valor como TOML e, se não for TOML válido,
        // usa o texto cru — e aspas mudariam de sentido entre bash e PowerShell.
        comando += &if provider == "claude" { format!(" --effort {esforco}") } else { format!(" -c model_reasoning_effort={esforco}") };
    }
    Ok(comando)
}

/// Copia o arquivo da conversa para onde a CLI da conta de destino procura: Claude por pasta do
/// projeto + id; Codex na mesma posição relativa dentro do CODEX_HOME. Devolve o caminho novo.
fn levar_transcript(perfis: &[omni_core::Profile], provider: &str, conta_origem: Option<&str>, origem: &Path,
    alvo: &omni_core::Profile, cwd: &str, sessao: &str) -> Result<String> {
    let destino = match provider {
        "claude" => omni_core::conversations::claude_transcript_path(Path::new(&alvo.config_dir),cwd,sessao),
        _ => {
            let raiz = perfis.iter().find(|p| Some(p.id.as_str()) == conta_origem)
                .and_then(|p| Path::new(&p.config_dir).canonicalize().ok()).ok_or_else(||anyhow!("Conta de origem não encontrada"))?;
            let relativo = origem.canonicalize()?.strip_prefix(&raiz).map(Path::to_path_buf)
                .map_err(|_|anyhow!("Transcript fora da conta de origem"))?;
            Path::new(&alvo.config_dir).join(relativo)
        }
    };
    if destino != origem {
        if let Some(pasta) = destino.parent() { std::fs::create_dir_all(pasta)?; }
        std::fs::copy(origem,&destino)?;
    }
    Ok(destino.to_string_lossy().into_owned())
}

/* ── Retomar uma conversa sem sessão aberta ──────────────────────────────────────────────────
   Conversa do histórico (ou do OMNI, com a sessão já fechada) não tem o que "reabrir": abre uma
   sessão nova na pasta do projeto com `--resume`, já com a conta, o modelo e o esforço escolhidos. */

fn plano_de_retomada(state: &EngineState, id: &str, ajustes: Ajustes) -> Result<Retomada, ApiError> {
    let dir = state.state_file.parent().unwrap();
    if let Ok((conversa, viva)) = current(state,id) {
        if viva.is_some() { return Err(error(StatusCode::CONFLICT,"Esta conversa já está aberta no PC")); }
        let segmento = conversa.segments.last().ok_or_else(||error(StatusCode::CONFLICT,"Conversa sem sessão"))?;
        let session_id = segmento.external_session_id.clone().ok_or_else(||error(StatusCode::CONFLICT,"Esta conversa não chegou a ter mensagens para retomar"))?;
        let transcript = omni_core::conversations::resolve_transcript(segmento,&conversa,&omni_core::profiles(dir))
            .map(|p| p.to_string_lossy().into_owned()).or(segmento.transcript_path.clone());
        return Ok(Retomada { conversation_id: conversa.id.clone(), project_id: conversa.project_id.clone(), cwd: conversa.cwd.clone(),
            title: omni_core::conversations::display_title(&conversa), provider: segmento.provider.clone(),
            profile_id: segmento.profile_id.clone(), session_id, transcript, ajustes });
    }
    let entrada = historical_entries(state).into_iter().find(|entrada| history_id(entrada) == id)
        .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
    let cwd = entrada.cwd.clone().filter(|cwd| Path::new(cwd).is_dir())
        .ok_or_else(||error(StatusCode::CONFLICT,"A pasta desta conversa não existe mais neste PC"))?;
    let project_id = known_projects(state).into_iter().find(|p| same_path(&p.path,&cwd)).map(|p| p.id)
        .unwrap_or_else(|| history_project_id(&cwd));
    Ok(Retomada {
        // Id estável: um segundo toque (mesma chave) acha a mesma conversa em vez de criar outra.
        conversation_id: stable_id("conv",&[id]), project_id, cwd,
        title: entrada.title.clone().or(entrada.first_prompt.clone()).unwrap_or_else(||"Conversa retomada".into()),
        provider: entrada.provider.clone(), profile_id: Some(entrada.profile_id.clone()), session_id: entrada.session_id.clone(),
        transcript: Some(entrada.path.clone()), ajustes,
    })
}

async fn retomar_rota(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<Ajustes>) -> Result<(StatusCode,Json<Value>),ApiError> {
    for valor in [&body.model, &body.effort, &body.profile_id].into_iter().flatten() {
        if !valor_seguro(valor) { return Err(error(StatusCode::BAD_REQUEST,"Valor inválido")); }
    }
    tokio::task::spawn_blocking(move || {
        if state.sessions.lock().expect("sessions poisoned").values()
            .filter(|entry| matches!(entry,SessionEntry::Live(_))).count() >= MAX_SESSOES {
            return Err(error(StatusCode::TOO_MANY_REQUESTS,"Muitas sessões abertas neste PC; feche alguma antes"));
        }
        let plano = plano_de_retomada(&state,&id,body)?;
        let conversa = plano.conversation_id.clone();
        enqueue(&state,&conversa,&headers,ActionKind::Retomar(plano))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha na fila"))?
}

fn retomar(state: &Arc<EngineState>, action: &Action, plano: &Retomada) -> Result<()> {
    if now_ms().saturating_sub(action.created_at_ms) >= 60_000 { return Err(anyhow!("Ação expirada")); }
    let dir = state.state_file.parent().unwrap().to_path_buf();
    let perfis = omni_core::profiles(&dir);
    let alvo_id = plano.ajustes.profile_id.clone().or(plano.profile_id.clone());
    let alvo = perfis.iter().find(|p| Some(&p.id) == alvo_id.as_ref() && p.provider == plano.provider);
    if alvo_id.is_some() && alvo.is_none() { return Err(anyhow!("Conta não encontrada para este agente")); }

    let mut transcript = plano.transcript.clone();
    if alvo_id != plano.profile_id {
        let alvo = alvo.ok_or_else(||anyhow!("Conta não encontrada"))?;
        let origem = plano.transcript.as_deref().map(PathBuf::from).filter(|p| p.is_file())
            .ok_or_else(||anyhow!("Não achei o transcript desta conversa para levar à outra conta"))?;
        transcript = Some(levar_transcript(&perfis,&plano.provider,plano.profile_id.as_deref(),&origem,alvo,&plano.cwd,&plano.session_id)?);
    }
    let base = state.mobile.workspace.lock().map_err(|_|anyhow!("workspace poisoned"))?.agents.iter()
        .find(|agente| agente.id == plano.provider).map(|agente| agente.command.clone())
        .unwrap_or_else(|| plano.provider.clone());
    let comando = comando_de_retomada(&base,&plano.provider,&plano.session_id,&plano.ajustes)?;

    let agora = now_ms();
    let sessao = spawn_terminal_inner(state, state.next_id(), plano.project_id.clone(), plano.title.clone(), plano.cwd.clone(),
        None, Some(comando), 30, 120, agora,
        SessionOrigin {
            headless: true,
            env: alvo.map(omni_core::env_for).unwrap_or_default(),
            provider: Some(plano.provider.clone()),
            profile_id: alvo_id.clone(),
            conversation_id: Some(plano.conversation_id.clone()),
            external_session_id: Some(plano.session_id.clone()),
        })?;
    let segmento = omni_core::conversations::Segment {
        provider: plano.provider.clone(), profile_id: alvo_id, external_session_id: Some(plano.session_id.clone()),
        transcript_path: transcript, terminal_session_id: Some(sessao.id.clone()), started_at_ms: agora, ended_at_ms: None,
    };
    let mut indice = omni_core::conversations::read_index(&dir);
    match indice.iter_mut().find(|c| c.id == plano.conversation_id) {
        Some(conversa) => {
            if let Some(ultimo) = conversa.segments.last_mut() { ultimo.ended_at_ms.get_or_insert(agora); }
            conversa.segments.push(segmento);
        }
        None => indice.push(omni_core::conversations::Conversation {
            id: plano.conversation_id.clone(), project_id: plano.project_id.clone(), cwd: plano.cwd.clone(),
            title: plano.title.clone(), title_source: omni_core::conversations::TitleSource::Auto,
            created_at_ms: agora, segments: vec![segmento],
        }),
    }
    omni_core::conversations::write_index(&dir,&indice)?;
    if plano.provider == "claude" { passar_pela_confianca(state,&sessao.id); }
    Ok(())
}

fn reabrir_com(state: &Arc<EngineState>, conversation_id: &str, ajustes: &Ajustes) -> Result<()> {
    let dir = state.state_file.parent().unwrap().to_path_buf();
    let (_, sessao) = current(state,conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
    let sessao = sessao.ok_or_else(||anyhow!("Sessão encerrada"))?;
    let mut indice = omni_core::conversations::read_index(&dir);
    let conversa = indice.iter_mut().find(|c| c.id == conversation_id).ok_or_else(||anyhow!("Conversa removida"))?;
    let segmento = conversa.segments.last().cloned().ok_or_else(||anyhow!("Conversa sem sessão"))?;
    let sessao_externa = segmento.external_session_id.clone()
        .ok_or_else(||anyhow!("Mande a primeira mensagem antes: ainda não há conversa para retomar"))?;
    let perfis = omni_core::profiles(&dir);
    let alvo_id = ajustes.profile_id.clone().or(segmento.profile_id.clone());
    let alvo = perfis.iter().find(|p| Some(&p.id) == alvo_id.as_ref() && p.provider == segmento.provider);
    if alvo_id.is_some() && alvo.is_none() { return Err(anyhow!("Conta não encontrada para este agente")); }

    // Outra conta: o transcript vai junto, no lugar onde a CLI da conta nova procura.
    let trocou_conta = alvo_id != segmento.profile_id;
    let mut transcript = segmento.transcript_path.clone();
    if trocou_conta {
        let alvo = alvo.ok_or_else(||anyhow!("Conta não encontrada"))?;
        let origem = omni_core::conversations::resolve_transcript(&segmento,conversa,&perfis)
            .ok_or_else(||anyhow!("Não achei o transcript desta conversa para levar à outra conta"))?;
        transcript = Some(levar_transcript(&perfis,&segmento.provider,segmento.profile_id.as_deref(),&origem,alvo,&conversa.cwd,&sessao_externa)?);
    }

    let base = state.mobile.workspace.lock().map_err(|_|anyhow!("workspace poisoned"))?.agents.iter()
        .find(|agente| agente.id == segmento.provider).map(|agente| agente.command.clone())
        .unwrap_or_else(|| segmento.provider.clone());
    let comando = comando_de_retomada(&base,&segmento.provider,&sessao_externa,ajustes)?;
    let sessao_id = {
        let mut meta = sessao.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?;
        // Só as variáveis de conta mudam; o resto do ambiente da sessão fica como estava.
        meta.env.retain(|(chave,_)| chave != "CLAUDE_CONFIG_DIR" && chave != "CODEX_HOME");
        if let Some(alvo) = alvo { meta.env.extend(omni_core::env_for(alvo)); }
        meta.profile_id = alvo_id.clone();
        meta.initial_command = Some(comando);
        meta.external_session_id = Some(sessao_externa.clone());
        meta.id.clone()
    };
    if trocou_conta {
        let agora = now_ms();
        if let Some(ultimo) = conversa.segments.last_mut() { ultimo.ended_at_ms = Some(agora); }
        conversa.segments.push(omni_core::conversations::Segment {
            provider: segmento.provider.clone(), profile_id: alvo_id, external_session_id: Some(sessao_externa),
            transcript_path: transcript, terminal_session_id: Some(sessao_id.clone()), started_at_ms: agora, ended_at_ms: None,
        });
        omni_core::conversations::write_index(&dir,&indice)?;
    }
    restart_session(state,&sessao_id)?;
    // Conta nova numa pasta que ela nunca abriu: o Claude pergunta se confia nela.
    if segmento.provider == "claude" { passar_pela_confianca(state,&sessao_id); }
    Ok(())
}

/// Abre a sessão que o celular pediu e registra a conversa que vai dar a timeline dela.
///
/// A conversa é criada **aqui** e não no desktop porque a janela pode estar fechada — é justamente
/// esse o caso de uso. Sem a conversa, a sessão existiria mas o celular não teria o que ler.
fn spawn_from_mobile(state:&Arc<EngineState>, action:&Action, plan:&SpawnPlan) -> Result<()> {
    if now_ms().saturating_sub(action.created_at_ms) >= 60_000 { return Err(anyhow!("Ação expirada")); }
    let dir = state.state_file.parent().unwrap().to_path_buf();

    let conversation = omni_core::conversations::Conversation {
        id: format!("conv-{}",uuid::Uuid::new_v4()),
        project_id: plan.project_id.clone(),
        cwd: plan.cwd.clone(),
        title: plan.title.clone(),
        // Nome escolhido pelo usuário no celular conta como manual; sem título, o padrão automático
        // deixa o primeiro prompt nomear a conversa depois.
        title_source: if plan.titulo_do_usuario {
            omni_core::conversations::TitleSource::Manual
        } else {
            omni_core::conversations::TitleSource::Auto
        },
        created_at_ms: now_ms(),
        segments: vec![omni_core::conversations::Segment {
            provider: plan.provider.clone(),
            profile_id: plan.profile_id.clone(),
            external_session_id: plan.external_session_id.clone(),
            transcript_path: plan.transcript_path.clone(),
            terminal_session_id: None,
            started_at_ms: now_ms(),
            ended_at_ms: None,
        }],
    };

    let session = spawn_terminal_inner(
        state,
        state.next_id(),
        plan.project_id.clone(),
        plan.title.clone(),
        plan.cwd.clone(),
        None,
        Some(plan.command.clone()),
        // A PTY nasce sem ninguém olhando; o desktop redimensiona quando anexar a aba.
        // ponytail: tamanho fixo, suficiente até alguém reclamar.
        30,
        120,
        now_ms(),
        SessionOrigin {
            // Aberta sem nenhum terminal exibindo: o engine responde o `ESC[6n` do ConPTY.
            headless: true,
            env: plan.env.clone(),
            provider: Some(plan.provider.clone()),
            profile_id: plan.profile_id.clone(),
            conversation_id: Some(conversation.id.clone()),
            external_session_id: plan.external_session_id.clone(),
        },
    )?;

    // Grava a conversa só depois do spawn dar certo: falhou, não fica conversa órfã na lista.
    let mut index = omni_core::conversations::read_index(&dir);
    let mut conversation = conversation;
    conversation.segments[0].terminal_session_id = Some(session.id.clone());
    index.push(conversation);
    omni_core::conversations::write_index(&dir,&index)?;
    if plan.provider == "claude" { passar_pela_confianca(state,&session.id); }
    Ok(())
}

/// Pasta que a conta nunca abriu: o Claude para no "confiar nesta pasta?" e o celular só via
/// "entrada não reconhecida", sem saída. A pasta é um projeto da lista do usuário — o mesmo que o app
/// do PC pré-aprova (`ensure_agent_trust`). Em thread à parte: a fila de ações não espera o Claude subir.
fn passar_pela_confianca(state: &Arc<EngineState>, session_id: &str) {
    let viva = state.sessions.lock().ok().and_then(|sessoes| match sessoes.get(session_id) {
        Some(SessionEntry::Live(sessao)) => Some(sessao.clone()),
        _ => None,
    });
    if let Some(viva) = viva {
        std::thread::spawn(move || { interaction::wait_claude_ready(&viva,std::time::Duration::from_secs(45)); });
    }
}

fn execute_checked(state:&Arc<EngineState>, action:&Action, is_running: impl Fn(Option<u32>, &str) -> bool) -> Result<()> {
    if now_ms().saturating_sub(action.created_at_ms) >= 60_000 { return Err(anyhow!("Ação expirada")); }
    let (_,current_session) = current(state,&action.conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
    let session = current_session.ok_or_else(||anyhow!("Sessão encerrada"))?;
    // Hold the sessions registry through validation/write so close cannot replace this session.
    let sessions = state.sessions.lock().map_err(|_|anyhow!("sessions poisoned"))?;
    if !matches!(sessions.get(&action.session_id),Some(SessionEntry::Live(s)) if Arc::ptr_eq(s,&session)) { return Err(anyhow!("Sessão trocada")); }
    let mut context = session.interaction.lock().map_err(|_|anyhow!("screen poisoned"))?;
    if session.reserved.load(Ordering::SeqCst) || context.revision(&session) != action.revision { return Err(anyhow!("Contexto mudou antes da execução")); }
    let meta = session.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?.clone();
    let provider = meta.provider.as_deref().unwrap_or("");
    if !is_running(meta.pid,provider) { return Err(anyhow!("CLI não está mais ativo")); }
    // Prompt vai em duas partes: a colagem agora, o Enter depois que ela aparece no campo (ver
    // `interaction::submeter_colagem`). Aprovação é tecla de menu e continua num write só.
    let (data, submeter) = match &action.kind {
        ActionKind::Prompt(text) if interaction::ready(context.parser.screen(),provider) => (format!("\x1b[200~{text}\x1b[201~"), true),
        ActionKind::Approval(allow) => { let dialog = interaction::approval(&context.parser.screen().contents(),provider).ok_or_else(||anyhow!("Aprovação não reconhecida"))?; (if *allow { dialog.0 } else { dialog.1 }, false) },
        _ => return Err(anyhow!("Entrada não está disponível")),
    };
    context.input_revision += 1;
    {
        let mut writer = session.writer.lock().map_err(|_|anyhow!("writer poisoned"))?;
        writer.write_all(data.as_bytes())?; writer.flush()?;
    }
    if !submeter { return Ok(()); }
    // Solta os locks antes de esperar: o leitor da PTY precisa deles para a tela andar.
    drop(context);
    drop(sessions);
    interaction::submeter_colagem(&session)
}

/// Caminhos servidos sem código de acesso. Lista **fechada**, não prefixo: todo o resto é API e nasce
/// protegido.
///
/// Ícones e manifest precisam estar aqui porque o navegador os busca **sozinho**, sem o
/// `X-Omni-Token` — ao adicionar à tela inicial, e antes mesmo de o JavaScript rodar. Antes eles
/// caíam no 401 e o celular ficava sem ícone.
// `/parear` é pública porque quem chama ainda não tem token; quem a protege é a trava de tentativas.
const ARQUIVOS_PUBLICOS: [&str; 6] =
    ["/manifest.webmanifest", "/apple-touch-icon.png", "/icon-192.png", "/icon-512.png", "/favicon.png", "/parear"];

fn caminho_publico(path: &str) -> bool {
    // A casca das duas páginas é pública (só HTML estático); quem protege os dados é a API.
    matches!(path, "/" | "/pc" | "/pc/" | "/pc.html")
        || path.starts_with("/assets/")
        || ARQUIVOS_PUBLICOS.contains(&path)
}

/// `nosniff` está ligado em toda resposta, então o tipo tem de estar certo: ícone ou manifest
/// servidos como `application/octet-stream` são ignorados pelo celular.
fn tipo_do_arquivo(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

async fn asset(request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    // `/` é a página do celular; `/pc` é a de computador — mesmas rotas de API, layout largo.
    let path = match path {
        "" => "index.html",
        "pc" | "pc/" => "pc.html",
        outro => outro,
    };
    #[cfg(mobile_assets)]
    if let Some(file) = ASSETS.get_file(path) {
        return ([("content-type",tipo_do_arquivo(path))],file.contents()).into_response();
    }
    let _ = path;
    (StatusCode::NOT_FOUND,"Página não encontrada; em desenvolvimento, execute npm run build:mobile antes de compilar o engine").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;
    use axum::body::{Body,to_bytes};

    fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(EngineState { persist_lock:Mutex::new(()),mobile:MobileRuntime::new(dir.path()),
            token:"test-only".into(),state_file:dir.path().join("sessions.json"),sessions:Mutex::new(HashMap::new()),id_sequence:AtomicU64::new(1) });
        atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
            "id":"c","project_id":"p","cwd":"C:/test","title":"Teste","created_at_ms":1,"segments":[]
        }]})).unwrap();
        (dir,state)
    }

    /// Local visual harness: no real profiles, credentials, sessions or user data.
    #[tokio::test]
    #[ignore = "Run explicitly while checking the mobile UI in a browser (10 minutes)"]
    async fn browser_fixture() {
        let (dir,state) = fixture();
        *state.mobile.config.lock().unwrap() = MobileConfig{enabled:true,bind:"127.0.0.1:47329".into(),token:"fixture-token".into(),serve:false,totp_secret:String::new(),totp_confirmed:false};

        // Conteúdo de mentira, mas com cara de uso real: é para julgar o visual, não a lógica.
        let conta = dir.path().join("claude");
        let transcript = conta.join("projects").join("fixture").join("sessao.jsonl");
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        let linha = |i: usize, role: &str, texto: &str| json!({"uuid":format!("m{i}"),"type":role,"timestamp":format!("2026-09-17T13:0{i}:00Z"),
            "message":{"role":role,"content":texto}}).to_string();
        let mensagens = [
            linha(0,"user","O login está lento no celular. Consegue ver o que está acontecendo?"),
            linha(1,"assistant","Achei a causa. O **token é validado duas vezes** a cada requisição:\n\n1. no middleware de sessão\n2. de novo em `authGuard`\n\n```ts\nexport function authGuard(req: Request) {\n  return validar(req.token); // duplicado\n}\n```\n\n| Etapa | Antes | Depois |\n|---|---|---|\n| Login | 1,8 s | 0,6 s |\n| Refresh | 900 ms | 310 ms |"),
            linha(2,"user","Perfeito. Segue o print do erro que aparece e o log do servidor.\n\n[Anexos enviados pelo celular]\n- C:\\dev\\omni-agents\\.omni-agents\\anexos\\1726000000000-ab12-print-erro.png\n- C:\\dev\\omni-agents\\.omni-agents\\anexos\\1726000000001-cd34-servidor.log"),
            linha(3,"assistant","Vi os dois. O erro do print é o `401` que o segundo `authGuard` devolve quando o token acabou de ser renovado. Removi a validação duplicada e os testes passaram."),
        ];
        std::fs::write(&transcript,mensagens.join("\n")).unwrap();
        atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[
            {"id":"claude-p","provider":"claude","name":"Pessoal","config_dir":conta.to_string_lossy(),"builtin":false,"created_at_ms":0},
            {"id":"codex-p","provider":"codex","name":"Trabalho","config_dir":dir.path().join("codex").to_string_lossy(),"builtin":false,"created_at_ms":0},
        ]})).unwrap();
        let segmento = |provider: &str, perfil: &str, caminho: Option<String>| json!({"provider":provider,"profile_id":perfil,
            "external_session_id":null,"transcript_path":caminho,"terminal_session_id":null,"started_at_ms":1,"ended_at_ms":null});
        atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[
            {"id":"c","project_id":"p","cwd":"C:/dev/omni-agents","title":"Login lento no celular","created_at_ms":3,
                "segments":[segmento("claude","claude-p",Some(transcript.to_string_lossy().into_owned()))]},
            {"id":"c2","project_id":"p","cwd":"C:/dev/omni-agents","title":"Revisar PR de pagamentos","created_at_ms":2,
                "segments":[segmento("codex","codex-p",None)]},
            {"id":"c3","project_id":"loja","cwd":"C:/dev/loja","title":"Migrar para Next 16","created_at_ms":1,"segments":[]},
        ]})).unwrap();
        publish(&state,
            vec![PublishedProject{id:"p".into(),name:"omni-agents".into(),path:"C:/dev/omni-agents".into()},
                 PublishedProject{id:"loja".into(),name:"loja-virtual".into(),path:"C:/dev/loja".into()}],
            vec![PublishedAgent{id:"claude".into(),label:"Claude".into(),command:"claude".into(),resume:Some("--continue".into())},
                 PublishedAgent{id:"codex".into(),label:"Codex".into(),command:"codex".into(),resume:None}],
            Some(PublishedTheme{accent:std::env::var("OMNI_FIXTURE_ACCENT").unwrap_or_else(|_|"roxo".into()),
                theme:std::env::var("OMNI_FIXTURE_THEME").unwrap_or_else(|_|"escuro".into()),
                background:"gradient-waves".into(),glass:"forte".into()}),
        ).unwrap();

        let listener = TcpListener::bind("127.0.0.1:47329").await.unwrap();
        let stop = Arc::new(Notify::new());
        let notify = stop.clone();
        let app = router(state).route("/__test_stop",post(move || { let notify = notify.clone(); async move { notify.notify_one(); StatusCode::OK } }));
        axum::serve(listener,app).with_graceful_shutdown(async move {
            tokio::select! { _ = stop.notified() => {}, _ = tokio::time::sleep(std::time::Duration::from_secs(600)) => {} }
        }).await.unwrap();
    }
    fn req(method:&str, uri:&str, origin:&str, body:&str) -> Request {
        Request::builder().method(method).uri(uri).header("host","127.0.0.1:47322").header("origin",origin)
            .header("content-type","application/json").header("idempotency-key","test-key").body(Body::from(body.to_owned())).unwrap()
    }

    /// `/painel` junta lista e catálogo; com o `ETag` de volta e nada mudado, responde 304 sem corpo.
    /// A visão Conversa do desktop chama as rotas do celular pelo protocolo local: sem token de
    /// dispositivo nem `Origin`, que lá não existem — quem chega já mostrou o token do engine.
    #[tokio::test] async fn desktop_chama_a_api_pelo_protocolo_local() {
        let (_dir,state) = fixture();
        state.mobile.config.lock().unwrap().token = "token-do-celular".into();
        let EngineResponse::Api { status, body } = local(state.clone(),"GET","/conversas/c/timeline",None,None,vec![]).await else { panic!("não é Api") };
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

    /// Imagem colada no chat do PC: binário em base64 e o nome no cabeçalho, como o celular manda.
    #[tokio::test] async fn anexo_colado_no_pc_chega_pelo_protocolo_local() {
        let (dir,state) = fixture();
        let projeto = dir.path().join("projeto");
        std::fs::create_dir_all(&projeto).unwrap();
        atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
            "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,"segments":[]
        }]})).unwrap();
        let EngineResponse::Api { status, body } = local(state,"POST","/conversas/c/anexos",None,Some("iVBORw0KGgo=".into()),
            vec![("x-omni-nome".into(),"print.png".into()),("content-type".into(),"image/png".into())]).await else { panic!("não é Api") };
        assert_eq!(status,200,"{body}");
        let caminho = serde_json::from_str::<Value>(&body).unwrap()["caminho"].as_str().unwrap().to_string();
        assert_eq!(std::fs::read(&caminho).unwrap(), b"\x89PNG\r\n\x1a\n");
        assert!(caminho.ends_with("print.png"));
    }
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

    /// Sem sessão aberta no PC não há terminal para espelhar nem para receber teclas.
    #[tokio::test] async fn terminal_e_teclas_exigem_sessao_viva() {
        let (_dir,state) = fixture();
        let app = router(state);
        let resposta = app.clone().oneshot(req("GET","/conversas/c/terminal","","")).await.unwrap();
        let corpo: Value = serde_json::from_slice(&axum::body::to_bytes(resposta.into_body(),usize::MAX).await.unwrap()).unwrap();
        assert_eq!(corpo["live"],false);
        let origem = "http://127.0.0.1:47322";
        assert_eq!(app.clone().oneshot(req("POST","/conversas/c/teclas",origem,r#"{"dados":"\u001b"}"#)).await.unwrap().status(),StatusCode::CONFLICT);
        assert_eq!(app.clone().oneshot(req("POST","/conversas/c/teclas",origem,r#"{"dados":""}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
        assert_eq!(app.oneshot(req("GET","/conversas/nao-existe/terminal","","")).await.unwrap().status(),StatusCode::NOT_FOUND);
    }

    /// Renomear vale para celular e para a web de computador, e é o engine que grava — o índice de
    /// conversas tem um escritor só.
    #[tokio::test] async fn renomear_conversa_pela_api_grava_o_nome_e_recusa_lixo() {
        let (dir,state) = fixture();
        let app = router(state.clone());

        let resposta = app.clone().oneshot(req("POST","/conversas/c/nome","http://127.0.0.1:47322",r#"{"nome":"Refatorar login"}"#)).await.unwrap();
        assert_eq!(resposta.status(),StatusCode::OK);
        let conversas = omni_core::conversations::read_index(dir.path());
        assert_eq!(conversas[0].title,"Refatorar login");
        assert_eq!(conversas[0].title_source,omni_core::conversations::TitleSource::Manual);
        // Nome manual aparece na lista que o celular lê.
        let lista = summaries(&state);
        assert_eq!(lista[0]["title"],"Refatorar login");

        for lixo in [r#"{"nome":"   "}"#, r#"{"nome":"com
quebra"}"#] {
            assert_eq!(app.clone().oneshot(req("POST","/conversas/c/nome","http://127.0.0.1:47322",lixo)).await.unwrap().status(),StatusCode::BAD_REQUEST);
        }
        assert_eq!(app.oneshot(req("POST","/conversas/desconhecida/nome","http://127.0.0.1:47322",r#"{"nome":"x"}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
    }

    /// O modo é `plan` ou `auto` e nada mais; sem sessão viva a fila recusa, como qualquer ação.
    #[tokio::test] async fn modo_aceita_so_plan_e_auto() {
        let (_dir,state) = fixture();
        let app = router(state);
        assert_eq!(app.clone().oneshot(req("POST","/conversas/c/modo","http://127.0.0.1:47322",r#"{"modo":"bypassPermissions"}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
        assert_eq!(app.clone().oneshot(req("POST","/conversas/c/modo","http://127.0.0.1:47322",r#"{"modo":"plan"}"#)).await.unwrap().status(),StatusCode::CONFLICT);
        assert_eq!(app.oneshot(req("POST","/conversas/c/modo","http://127.0.0.1:47322",r#"{"modo":"auto"}"#)).await.unwrap().status(),StatusCode::CONFLICT);
    }

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
        let post = |corpo:&str| req("POST","/sessoes","http://127.0.0.1:47322",corpo);

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

    #[tokio::test] async fn concurrent_readers_and_no_privileged_http_surface() {
        let (_dir,state) = fixture();
        let app = router(state.clone());
        let (a,b) = tokio::join!(app.clone().oneshot(req("GET","/conversas","","")),app.clone().oneshot(req("GET","/atencao","","")));
        assert_eq!(a.unwrap().status(),StatusCode::OK); assert_eq!(b.unwrap().status(),StatusCode::OK);
        let response = app.clone().oneshot(req("GET","/conversas/c/timeline","","")).await.unwrap();
        let data:Value = serde_json::from_slice(&to_bytes(response.into_body(),1_000_000).await.unwrap()).unwrap();
        assert_eq!(data["timeline"]["messages"],json!([]));
        let response = app.clone().oneshot(req("POST","/conversas/c/prompt","http://127.0.0.1:47322",r#"{"texto":"oi"}"#)).await.unwrap();
        assert_eq!(response.status(),StatusCode::CONFLICT);
        assert!(state.sessions.lock().unwrap().is_empty(),"HTTP must not spawn a terminal");
        assert_ne!(app.oneshot(req("POST","/spawn_terminal","http://127.0.0.1:47322","{}")).await.unwrap().status(),StatusCode::OK);
    }
    #[tokio::test] async fn foreign_origin_unknown_conversation_and_invalid_prompt_are_rejected() {
        let (_dir,state) = fixture(); let app = router(state);
        assert_eq!(app.clone().oneshot(req("POST","/conversas/c/prompt","https://evil.example",r#"{"texto":"oi"}"#)).await.unwrap().status(),StatusCode::FORBIDDEN);
        assert_eq!(app.clone().oneshot(req("GET","/conversas/missing/timeline","","")).await.unwrap().status(),StatusCode::NOT_FOUND);
        assert_eq!(app.oneshot(req("POST","/conversas/c/prompt","http://127.0.0.1:47322",r#"{"texto":"/exit"}"#)).await.unwrap().status(),StatusCode::BAD_REQUEST);
    }
    #[test] fn idempotent_retry_does_not_need_a_live_session_or_enqueue_again() {
        let (_dir,state) = fixture();
        state.mobile.actions.lock().unwrap().push_back(Action { id:"a".into(),conversation_id:"c".into(),session_id:"gone".into(),state:"sent".into(),created_at_ms:now_ms(),error:None,key:"test-key".into(),revision:"old".into(),kind:ActionKind::Prompt("oi".into()) });
        let headers = req("POST","/conversas/c/prompt","http://127.0.0.1:47322","").headers().clone();
        assert_eq!(enqueue(&state,"c",&headers,ActionKind::Prompt("oi".into())).unwrap().0,StatusCode::ACCEPTED);
        assert_eq!(state.mobile.actions.lock().unwrap().len(),1);
        assert_eq!(enqueue(&state,"c",&headers,ActionKind::Prompt("outro".into())).unwrap_err().0,StatusCode::CONFLICT);
        let mut action = state.mobile.actions.lock().unwrap()[0].clone();
        action.created_at_ms = now_ms().saturating_sub(60_001);
        assert!(execute(&state,&action).unwrap_err().to_string().contains("expirada"));
    }
    /// O bundle tem de carregar antes de o celular ter token; a API, não. Se esta asserção inverter,
    /// ou o celular não consegue nem abrir a página, ou qualquer um na tailnet lê as conversas.
    #[tokio::test] async fn token_de_dispositivo_protege_a_api_e_libera_o_bundle() {
        let (_dir,state) = fixture();
        state.mobile.config.lock().unwrap().token = "segredo".into();
        let app = router(state);
        let com_token = |valor:&str| {
            Request::builder().method("GET").uri("/conversas").header("host","127.0.0.1:47322")
                .header("x-omni-token",valor).body(Body::empty()).unwrap()
        };
        assert_eq!(app.clone().oneshot(req("GET","/conversas","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
        assert_eq!(app.clone().oneshot(com_token("segred")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
        assert_eq!(app.clone().oneshot(com_token("segredx")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
        assert_eq!(app.clone().oneshot(com_token("segredo")).await.unwrap().status(),StatusCode::OK);
        assert_ne!(app.oneshot(req("GET","/","","")).await.unwrap().status(),StatusCode::UNAUTHORIZED);
    }

    #[test] fn habilitar_gera_token_e_o_desktop_nao_escolhe_o_segredo() {
        let (_dir,state) = fixture();
        let forjado = MobileConfig{enabled:true,bind:"127.0.0.1:47322".into(),token:"escolhido-pelo-desktop".into(),serve:false,totp_secret:String::new(),totp_confirmed:false};
        settings(&state,Some(forjado),false).unwrap();
        let gerado = state.mobile.config.lock().unwrap().token.clone();
        assert_eq!(gerado.len(),64,"32 bytes em hex");
        assert_ne!(gerado,"escolhido-pelo-desktop");

        // Salvar de novo preserva o token: só `rotate` troca, senão todo "Aplicar" desparearia o celular.
        settings(&state,Some(MobileConfig{enabled:true,bind:"127.0.0.1:47322".into(),token:String::new(),serve:false,totp_secret:String::new(),totp_confirmed:false}),false).unwrap();
        assert_eq!(state.mobile.config.lock().unwrap().token,gerado);
        settings(&state,Some(MobileConfig{enabled:true,bind:"127.0.0.1:47322".into(),token:String::new(),serve:false,totp_secret:String::new(),totp_confirmed:false}),true).unwrap();
        assert_ne!(state.mobile.config.lock().unwrap().token,gerado);
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

    /// Com o Serve ligado o `Host` que chega é o nome MagicDNS, não o bind. Aceitar esse nome não
    /// pode virar "aceitar qualquer Host": a checagem existe para barrar CSRF e DNS rebinding.
    #[tokio::test] async fn serve_aceita_o_nome_magicdns_e_mais_nenhum() {
        let (_dir,state) = fixture();
        *state.mobile.config.lock().unwrap() =
            MobileConfig{enabled:true,bind:"127.0.0.1:47322".into(),token:String::new(),serve:true,totp_secret:String::new(),totp_confirmed:false};
        *state.mobile.magic_dns.lock().unwrap() = Some("pc.tail0000.ts.net".into());
        let app = router(state);
        let get = |host:&str| Request::builder().method("GET").uri("/conversas")
            .header("host",host).body(Body::empty()).unwrap();

        assert_eq!(app.clone().oneshot(get("pc.tail0000.ts.net")).await.unwrap().status(),StatusCode::OK);
        assert_eq!(app.clone().oneshot(get("127.0.0.1:47322")).await.unwrap().status(),StatusCode::OK);
        assert_eq!(app.clone().oneshot(get("evil.example")).await.unwrap().status(),StatusCode::FORBIDDEN);
        // Sufixo parecido não basta: a comparação é igualdade, não "termina com".
        assert_eq!(app.oneshot(get("mal.pc.tail0000.ts.net")).await.unwrap().status(),StatusCode::FORBIDDEN);
    }

    #[test] fn serve_exige_loopback_no_bind() {
        // Escutar num IP roteável **e** publicar pelo Serve dobraria a superfície sem serventia.
        assert!(validate(&MobileConfig{enabled:true,bind:"100.64.0.1:47322".into(),token:String::new(),serve:true,totp_secret:String::new(),totp_confirmed:false}).is_err());
        assert!(validate(&MobileConfig{enabled:true,bind:"127.0.0.1:47322".into(),token:String::new(),serve:true,totp_secret:String::new(),totp_confirmed:false}).is_ok());
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
        let cfg = |enabled, serve| MobileConfig{enabled,serve,bind:"127.0.0.1:47322".into(),token:String::new(),
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
        *state.mobile.config.lock().unwrap() = MobileConfig{enabled:true,bind:"127.0.0.1:47322".into(),token:"segredo".into(),
            serve:false,totp_secret:String::new(),totp_confirmed:false};
        let app = router(state.clone());
        // Maior que o limite global de 32 KB: prova que a rota tem o limite dela.
        let conteudo = vec![7_u8; 200 * 1024];
        let pedido = |token: &str| Request::builder().method("POST").uri("/conversas/c/anexos")
            .header("host","127.0.0.1:47322").header("origin","http://127.0.0.1:47322")
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
            .header("host","127.0.0.1:47322").header("origin","http://127.0.0.1:47322")
            .header("x-omni-token","segredo").body(Body::from(vec![1_u8])).unwrap();
        assert_eq!(app.clone().oneshot(outra).await.unwrap().status(),StatusCode::NOT_FOUND);

        // Acima do limite da rota: recusado.
        let grande = Request::builder().method("POST").uri("/conversas/c/anexos")
            .header("host","127.0.0.1:47322").header("origin","http://127.0.0.1:47322")
            .header("x-omni-token","segredo").body(Body::from(vec![0_u8; ANEXO_MAX_BYTES + 1])).unwrap();
        assert_eq!(app.oneshot(grande).await.unwrap().status(),StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test] fn tema_publicado_vai_para_o_celular_e_desktop_antigo_nao_apaga() {
        let (_dir,state) = fixture();
        let tema = PublishedTheme{accent:"roxo".into(),theme:"claro".into(),background:"pixel-grid".into(),glass:"forte".into()};
        publish(&state,vec![],vec![],Some(tema.clone())).unwrap();
        publish(&state,vec![],vec![],None).unwrap();
        assert_eq!(state.mobile.workspace.lock().unwrap().theme.as_ref(),Some(&tema));
    }

    #[test] fn link_de_liberacao_sai_da_saida_do_serve() {
        let saida = "Serve is not enabled on your tailnet.\nTo enable, visit:\n\n         https://login.tailscale.com/f/serve?node=nEXEMPLO0000CNTRL\n";
        assert_eq!(motivo_do_serve(saida),"https://login.tailscale.com/f/serve?node=nEXEMPLO0000CNTRL");
        assert_eq!(motivo_do_serve("   "),"O Tailscale não respondeu.");
        assert_eq!(motivo_do_serve("access denied"),"access denied");
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
        let status = app.clone().oneshot(req("POST","/parear","http://127.0.0.1:47322",corpo)).await.unwrap().status();
        assert_eq!(status,StatusCode::UNAUTHORIZED,"401 aqui é do código errado, não do token ausente");
        let sem_origem = app.oneshot(req("POST","/parear","https://evil.example",corpo)).await.unwrap().status();
        assert_eq!(sem_origem,StatusCode::FORBIDDEN);
    }

    #[test] fn desktop_nao_confirma_o_authy_nem_troca_o_segredo() {
        let (_dir,state) = fixture();
        totp(&state,TotpAction::Reset).unwrap();
        let segredo = state.mobile.config.lock().unwrap().totp_secret.clone();
        assert!(!segredo.is_empty());

        let forjado = MobileConfig{enabled:false,bind:"127.0.0.1:47322".into(),token:String::new(),serve:false,
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
        assert!(validate(&MobileConfig{enabled:true,bind:"127.0.0.1:47321".into(),token:String::new(),serve:false,totp_secret:String::new(),totp_confirmed:false}).is_err());
    }

    #[cfg(windows)]
    mod actions {
        use super::*;
        #[derive(Debug)] struct FakeChild;
        impl portable_pty::ChildKiller for FakeChild {
            fn kill(&mut self) -> std::io::Result<()> { Ok(()) }
            fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> { Box::new(FakeChild) }
        }
        impl Child for FakeChild {
            fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> { Ok(None) }
            fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> { Ok(portable_pty::ExitStatus::with_exit_code(0)) }
            fn process_id(&self) -> Option<u32> { Some(42) }
            fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> { None }
        }
        struct FakeMaster;
        impl MasterPty for FakeMaster {
            fn resize(&self,_:PtySize) -> Result<()> { Ok(()) }
            fn get_size(&self) -> Result<PtySize> { Ok(PtySize{rows:20,cols:100,pixel_width:0,pixel_height:0}) }
            fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>> { Ok(Box::new(std::io::empty())) }
            fn take_writer(&self) -> Result<Box<dyn Write + Send>> { Ok(Box::new(std::io::sink())) }
        }
        struct RecordingWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for RecordingWriter {
            fn write(&mut self,bytes:&[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(bytes); Ok(bytes.len()) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        fn fake_session(state:&Arc<EngineState>,text:&str) -> (Arc<LiveSession>,Arc<Mutex<Vec<u8>>>,Action) {
            let bytes = Arc::new(Mutex::new(Vec::new()));
            let meta:TerminalSession = serde_json::from_value(json!({"id":"s","project_id":"p","name":"Test","cwd":"C:/test","shell":"test",
                "state":"answered","pid":42,"created_at_ms":1,"last_activity_at_ms":1,"output_seq":0,"rows":20,"cols":100,
                "provider":"claude","profile_id":"profile","conversation_id":"c"})).unwrap();
            let mut context = interaction::Interaction::new(20,100); context.parser.process(b"\x1b[?2004h"); context.parser.process(text.as_bytes());
            let session = Arc::new(LiveSession{interaction:Mutex::new(context),reserved:std::sync::atomic::AtomicBool::new(false),headless:std::sync::atomic::AtomicBool::new(false),
                meta:Mutex::new(meta),writer:Mutex::new(Box::new(RecordingWriter(bytes.clone()))),master:Mutex::new(Box::new(FakeMaster)),
                child:Mutex::new(Box::new(FakeChild)),output:Mutex::new(OutputBuffer{chunks:VecDeque::new(),bytes:0,next_seq:0})});
            state.sessions.lock().unwrap().insert("s".into(),SessionEntry::Live(session.clone()));
            atomic_write_json(&state.state_file.with_file_name("conversations.json"),&json!({"version":1,"conversations":[{
                "id":"c","project_id":"p","cwd":"C:/test","title":"Test","created_at_ms":1,
                "segments":[{"provider":"claude","profile_id":"profile","terminal_session_id":"s","started_at_ms":1}]
            }]})).unwrap();
            let revision = session.interaction.lock().unwrap().revision(&session);
            let action = Action{id:"a".into(),conversation_id:"c".into(),session_id:"s".into(),state:"queued".into(),created_at_ms:now_ms(),error:None,key:"key".into(),revision,kind:ActionKind::Prompt("Olá\nmundo".into())};
            (session,bytes,action)
        }
        /// O Enter só sai depois que a colagem aparece no campo. Junto com ela, no mesmo write, a CLI
        /// trata o `\r` como parte do texto colado: com anexos (prompt de várias linhas) a mensagem
        /// ficava digitada no terminal e nunca era enviada.
        #[test] fn prompt_cola_primeiro_e_so_depois_manda_o_enter() {
            let (_dir,state) = fixture(); let (session,bytes,action) = fake_session(&state,"Claude Code\r\n❯ ");
            let colagem = "\x1b[200~Olá\nmundo\x1b[201~".as_bytes();

            // A CLI desenha o texto colado no campo pouco depois; é esse eco que libera o Enter.
            let eco = session.clone();
            let escrito = bytes.clone();
            let desenhou = std::thread::spawn(move || {
                let mut tentativas = 0;
                loop {
                    if escrito.lock().unwrap().as_slice() == colagem { break; }
                    tentativas += 1;
                    assert!(tentativas < 200, "a colagem não foi escrita antes do Enter");
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                // Enter não pode ter saído junto com a colagem.
                assert_eq!(&*escrito.lock().unwrap(), colagem, "Enter saiu na mesma rajada da colagem");
                eco.interaction.lock().unwrap().parser.process(b"[Pasted text #1 +2 lines]");
            });

            execute_checked(&state,&action,|_,_|true).unwrap();
            desenhou.join().unwrap();
            assert_eq!(&*bytes.lock().unwrap(),"\x1b[200~Olá\nmundo\x1b[201~\r".as_bytes());
            assert!(execute_checked(&state,&action,|_,_|true).is_err());
        }

        /// CLI que não devolve eco nenhum não pode travar o envio para sempre.
        #[test] fn sem_eco_da_colagem_o_enter_sai_no_limite() {
            let (_dir,state) = fixture(); let (_session,bytes,action) = fake_session(&state,"Claude Code\r\n❯ ");
            let comeco = std::time::Instant::now();
            execute_checked(&state,&action,|_,_|true).unwrap();
            assert!(comeco.elapsed() >= std::time::Duration::from_secs(1), "não esperou o eco");
            assert!(bytes.lock().unwrap().ends_with(b"\r"));
        }
        /// A TUI redesenha o diálogo de aprovação em pedaços. Um quadro parcial não pode virar
        /// "pedido novo" — era o aviso já visto voltando sozinho.
        #[test] fn aprovacao_redesenhada_conta_um_aviso_so() {
            let (_dir,state) = fixture(); let (session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
            let dialogo = "\x1b[2J\x1b[HDo you want to proceed?\r\n❯ 1. Yes\r\n2. Yes, always\r\n3. No";
            let seq = || session.meta.lock().unwrap().attention_seq;
            crate::append_output(&session,dialogo.as_bytes(),Some(42));
            assert_eq!(seq(),1);
            crate::append_output(&session,b"\x1b[2J\x1b[HDo you want",Some(42));
            crate::append_output(&session,dialogo.as_bytes(),Some(42));
            assert_eq!(seq(),1,"redesenho contou como pedido novo");
            // Aprovado, o agente volta a trabalhar: o próximo diálogo é outro pedido.
            crate::append_output(&session,b"\x1b[2J\x1b[H* Running (esc to interrupt)",Some(42));
            crate::append_output(&session,dialogo.as_bytes(),Some(42));
            assert_eq!(seq(),2);
            assert_eq!(summaries(&state)[0]["attention"],"s:2");
        }
        /// Failover reabre a conversa com `--resume`, que reexibe o aviso que fez a conta trocar.
        /// Isso é histórico: a conversa não pode aparecer "interrompida" (nem disparar outro
        /// failover) antes de um turno novo. Depois de um turno, o aviso volta a valer.
        #[test] fn aviso_reexibido_pelo_resume_nao_conta_antes_do_primeiro_turno() {
            let (_dir,state) = fixture(); let (session,_,_) = fake_session(&state,"");
            let aviso = "[2J[H> opa
  ⎿  You've hit your session limit · resets 8:10pm
";
            let notice = || session.meta.lock().unwrap().notice.clone();
            crate::append_output(&session,aviso.as_bytes(),Some(42));
            assert_eq!(notice(),None,"histórico do resume virou aviso");
            crate::append_output(&session,b"[2J[H* Working (esc to interrupt)",Some(42));
            crate::append_output(&session,aviso.as_bytes(),Some(42));
            assert_eq!(notice().as_deref(),Some("usage_limit"));
        }
        /// "Toda vez que entro na conversa ele começa a pensar": abrir a aba redesenha a TUI, e
        /// qualquer saída virava "trabalhando". Com o agente parado, redesenho não muda o estado.
        #[test] fn redesenho_com_o_agente_parado_nao_vira_trabalhando() {
            let (_dir,state) = fixture(); let (session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
            let estado = || session.meta.lock().unwrap().state.clone();
            crate::append_output(&session,b"\x1b[2J\x1b[HClaude Code\r\n> ",Some(42));
            assert_eq!(estado(),SessionState::Answered,"redesenho virou trabalho");
            crate::append_output(&session,b"\x1b[2J\x1b[H* Pensando (esc to interrupt)",Some(42));
            assert_eq!(estado(),SessionState::Working);
        }

        /// Plano/Auto pelo chat do PC: o `Idempotency-Key` tem de atravessar o protocolo local.
        #[tokio::test] async fn modo_pelo_pc_leva_o_idempotency_key() {
            let (_dir,state) = fixture(); let (_session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
            let EngineResponse::Api { body, .. } = local(state,"POST","/conversas/c/modo",Some(r#"{"modo":"plan"}"#.into()),None,
                vec![("Idempotency-Key".into(),"chave-1".into())]).await else { panic!("não é Api") };
            assert!(!body.contains("Idempotency-Key"), "cabeçalho se perdeu: {body}");
        }
        /// "Na web devo conseguir acessar os terminais sem agentes": a lista traz só shells, e as
        /// teclas/saída de um id de agente são recusadas — agente se usa pela conversa.
        #[tokio::test] async fn terminais_sem_agente_listam_e_recusam_sessao_de_agente() {
            let (_dir,state) = fixture(); let (_agente,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
            let lista = |state| async move {
                let EngineResponse::Api { body, .. } = local(state,"GET","/terminais",None,None,vec![]).await else { panic!("não é Api") };
                serde_json::from_str::<Value>(&body).unwrap()
            };
            assert_eq!(lista(state.clone()).await, json!([]), "sessão de agente apareceu como terminal");
            let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/terminais/s/teclas",Some(r#"{"dados":"ls\r"}"#.into()),None,vec![]).await else { panic!() };
            assert_eq!(status,404);
            let EngineResponse::Api { status, .. } = local(state,"POST","/terminais",Some(r#"{"project_id":"nao-existe"}"#.into()),None,vec![]).await else { panic!() };
            assert_eq!(status,404);
        }

        /// Ponta a ponta: abrir um shell no projeto pela web, digitar, ler a saída e fechar.
        #[tokio::test(flavor = "multi_thread")] async fn web_abre_shell_no_projeto_roda_e_fecha() {
            let (dir,state) = fixture();
            let projeto = dir.path().join("projeto");
            std::fs::create_dir_all(&projeto).unwrap();
            atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
                "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,"segments":[]
            }]})).unwrap();
            let chamar = |metodo: &'static str, caminho: String, corpo: Option<&'static str>| {
                let state = state.clone();
                async move {
                    let EngineResponse::Api { status, body } = local(state,metodo,&caminho,corpo.map(String::from),None,vec![]).await else { panic!("não é Api") };
                    (status, serde_json::from_str::<Value>(&body).unwrap_or(Value::Null))
                }
            };
            let (status, aberto) = chamar("POST","/terminais".into(),Some(r#"{"project_id":"p"}"#)).await;
            assert_eq!(status,201,"{aberto}");
            let id = aberto["id"].as_str().unwrap().to_string();
            assert_eq!(chamar("GET","/terminais".into(),None).await.1[0]["id"],id.as_str());

            let (status, _) = chamar("POST",format!("/terminais/{id}/teclas"),Some(r#"{"dados":"echo OMNI_WEB_OK\r"}"#)).await;
            assert_eq!(status,200);
            let mut saida = String::new();
            for _ in 0..150 {
                saida = chamar("GET",format!("/terminais/{id}/saida"),None).await.1["data"].as_str().unwrap_or("").to_string();
                if saida.matches("OMNI_WEB_OK").count() >= 2 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            assert!(saida.matches("OMNI_WEB_OK").count() >= 2, "o shell não rodou o comando: {saida:?}");

            assert_eq!(chamar("POST",format!("/terminais/{id}/fechar"),None).await.0,200);
            assert_eq!(chamar("GET","/terminais".into(),None).await.1,json!([]));
        }

        /// Conta, modelo e esforço viram argumentos da linha que retoma a conversa — cada CLI com a
        /// sua sintaxe.
        #[test] fn comando_de_retomada_por_cli() {
            let ajustes = |model: Option<&str>, effort: Option<&str>| Ajustes { profile_id: None, model: model.map(String::from), effort: effort.map(String::from) };
            assert_eq!(comando_de_retomada("claude","claude","abc",&ajustes(Some("opus"),Some("high"))).unwrap(),
                "claude --resume abc --model opus --effort high");
            assert_eq!(comando_de_retomada("codex","codex","xyz",&ajustes(Some("gpt-5.6-sol"),Some("xhigh"))).unwrap(),
                "codex resume xyz -m gpt-5.6-sol -c model_reasoning_effort=xhigh");
            assert_eq!(comando_de_retomada("claude","claude","abc",&ajustes(None,None)).unwrap(), "claude --resume abc");
            assert!(comando_de_retomada("cursor-agent","cursor","abc",&ajustes(None,None)).is_err());
        }

        /// O valor vira argumento de um comando digitado num shell: nada de `;`, espaço ou aspas.
        #[tokio::test] async fn ajustes_recusam_valor_que_vira_comando() {
            let (_dir,state) = fixture(); let (_session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
            for corpo in [r#"{"profile_id":null,"model":"opus; rm -rf ~","effort":null}"#, r#"{"profile_id":null,"model":null,"effort":"high && calc"}"#] {
                let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/conversas/c/ajustes",Some(corpo.into()),None,
                    vec![("Idempotency-Key".into(),"k".into())]).await else { panic!("não é Api") };
                assert_eq!(status,400,"{corpo}");
            }
            assert!(valor_seguro("claude-opus-5-5[1m]"));
            assert!(!valor_seguro("a b"));
        }

        #[tokio::test] async fn opcoes_listam_modelos_e_contas_do_mesmo_agente() {
            let (dir,state) = fixture(); let (_session,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
            atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[
                {"id":"profile","provider":"claude","name":"Padrão","config_dir":dir.path().join("a").to_string_lossy(),"builtin":false,"created_at_ms":1},
                {"id":"outra","provider":"claude","name":"Trabalho","config_dir":dir.path().join("b").to_string_lossy(),"builtin":false,"created_at_ms":1},
                {"id":"cdx","provider":"codex","name":"Codex","config_dir":dir.path().join("c").to_string_lossy(),"builtin":false,"created_at_ms":1}
            ]})).unwrap();
            let EngineResponse::Api { status, body } = local(state,"GET","/conversas/c/opcoes",None,None,vec![]).await else { panic!("não é Api") };
            assert_eq!(status,200,"{body}");
            let opcoes: Value = serde_json::from_str(&body).unwrap();
            assert_eq!(opcoes["provider"],"claude");
            let contas: Vec<&str> = opcoes["profiles"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
            assert_eq!(contas, ["Padrão","Trabalho"], "conta de outro agente apareceu");
            assert!(opcoes["models"].as_array().unwrap().iter().any(|m| m["id"] == "opus"));
        }

        /// Conversa sem sessão aberta (o caso do print: "Histórico") se retoma com conta, modelo e
        /// esforço; uma que já está aberta não — ali o caminho é "ajustes".
        #[tokio::test] async fn retomar_conversa_sem_sessao_e_recusar_a_aberta() {
            let (dir,state) = fixture();
            let projeto = dir.path().join("projeto");
            std::fs::create_dir_all(&projeto).unwrap();
            atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
                "id":"c","project_id":"p","cwd":projeto.to_string_lossy(),"title":"Teste","created_at_ms":1,
                "segments":[{"provider":"claude","profile_id":null,"external_session_id":"abc","transcript_path":null,"terminal_session_id":null,"started_at_ms":1}]
            }]})).unwrap();
            let pedir = |state: Arc<EngineState>, chave: &'static str| async move {
                let EngineResponse::Api { status, body } = local(state,"POST","/conversas/c/retomar",
                    Some(r#"{"profile_id":null,"model":"opus","effort":"high"}"#.into()),None,
                    vec![("Idempotency-Key".into(),chave.into())]).await else { panic!("não é Api") };
                (status, serde_json::from_str::<Value>(&body).unwrap_or(Value::Null))
            };
            let (status, corpo) = pedir(state.clone(),"k1").await;
            assert_eq!(status,202,"{corpo}");
            assert_eq!(corpo["conversation_id"],"c");
            let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/conversas/c/retomar",
                Some(r#"{"profile_id":null,"model":"opus; calc","effort":null}"#.into()),None,vec![("Idempotency-Key".into(),"k2".into())]).await else { panic!() };
            assert_eq!(status,400,"valor perigoso passou");

            let (_sessao,_,_) = fake_session(&state,"Claude Code\r\n❯ ");
            assert_eq!(pedir(state,"k3").await.0,409,"retomou uma conversa que já está aberta");
        }

        #[test] fn rejects_desktop_input_closed_cli_and_stale_dialog_without_writing() {
            let (_dir,state) = fixture(); let (session,bytes,mut action) = fake_session(&state,"Claude Code\r\nDo you want to proceed?\r\n❯ 1. Yes\r\n2. Yes, always\r\n3. No");
            action.kind = ActionKind::Approval(true);
            assert!(execute_checked(&state,&action,|_,_|false).is_err());
            session.interaction.lock().unwrap().input_revision += 1;
            assert!(execute_checked(&state,&action,|_,_|true).is_err());
            state.sessions.lock().unwrap().clear();
            assert!(execute_checked(&state,&action,|_,_|true).is_err());
            assert!(bytes.lock().unwrap().is_empty());
        }
        #[test] fn single_use_approval_and_denial_do_not_select_persistent_permission() {
            for (allow,expected) in [(true,"1\r"),(false,"3\r")] {
                let (_dir,state) = fixture(); let (_,bytes,mut action) = fake_session(&state,"Claude Code\r\nDo you want to proceed?\r\n❯ 1. Yes\r\n2. Yes, always\r\n3. No");
                action.kind = ActionKind::Approval(allow); execute_checked(&state,&action,|_,_|true).unwrap();
                assert_eq!(&*bytes.lock().unwrap(),expected.as_bytes());
            }
        }
    }
}
