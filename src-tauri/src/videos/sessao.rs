//! Fork (Editor de vídeo): a sessão a dois. Quem abre vira o host: o app faz backup do projeto, sobe
//! o servidor de `sessao_http.rs` em `127.0.0.1` e o expõe **só na tailnet** com
//! `tailscale serve --bg --http=47480` (sintaxe conferida no `tailscale serve --help` 1.104 e na
//! KB 1242; `off` com as mesmas flags desliga). Nunca Funnel. O convite leva o token e os endereços.
//! O conteúdo (Yjs) é do front; aqui só transporte, autenticação e mídia por hash.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use tauri::Emitter;

use super::sessao_http::{Aviso, Convidado, EventoDaSessao, Host};

pub const PORTA_NA_TAILNET: u16 = 47480;
pub const EVENTO: &str = "video-sessao";
const CONFIG: &str = "colaboracao.json";

enum Papel { Host(Arc<Host>), Convidado(Arc<Convidado>, String) }

fn estado() -> &'static Mutex<Option<Papel>> {
    static E: Mutex<Option<Papel>> = Mutex::new(None);
    &E
}

// ——— base64url (convite) ———

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn base64url(dados: &[u8]) -> String {
    let mut s = String::new();
    for bloco in dados.chunks(3) {
        let b = [bloco[0], *bloco.get(1).unwrap_or(&0), *bloco.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..(bloco.len() + 1) { s.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char); }
    }
    s
}

pub fn de_base64url(texto: &str) -> Option<Vec<u8>> {
    let mut saida = Vec::new();
    let valores: Option<Vec<u32>> = texto.trim().bytes().filter(|b| *b != b'=').map(|c| B64.iter().position(|x| *x == c).map(|p| p as u32)).collect();
    for bloco in valores?.chunks(4) {
        if bloco.len() < 2 { return None; }
        let n = bloco.iter().enumerate().fold(0u32, |acc, (i, v)| acc | v << (18 - 6 * i));
        saida.push((n >> 16) as u8);
        if bloco.len() > 2 { saida.push((n >> 8) as u8); }
        if bloco.len() > 3 { saida.push(n as u8); }
    }
    Some(saida)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Convite { pub v: u32, pub bases: Vec<String>, pub t: String, pub p: String, pub n: String }

pub fn montar_convite(c: &Convite) -> String { format!("medza-video:{}", base64url(serde_json::to_string(c).unwrap_or_default().as_bytes())) }

pub fn ler_convite(texto: &str) -> Result<Convite, String> {
    let corpo = texto.trim().strip_prefix("medza-video:").ok_or("Esse não é um convite de edição a dois do Niski.")?;
    let bytes = de_base64url(corpo).ok_or("Convite quebrado (copie inteiro).")?;
    let c: Convite = serde_json::from_slice(&bytes).map_err(|_| "Convite quebrado (copie inteiro).")?;
    if c.v != 1 || c.bases.is_empty() || c.t.len() < 16 || !super::arquivos::id_valido(&c.p) { return Err("Convite inválido.".into()); }
    if c.bases.iter().any(|b| !b.starts_with("http://")) { return Err("Convite inválido.".into()); }
    Ok(c)
}

// ——— Tailscale ———

fn tailscale() -> Option<PathBuf> {
    omni_core::cli_path::ampliar_path();
    crate::engine_client::resolve_on_path("tailscale")
        .or_else(|| Some(PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale")).filter(|p| p.is_file()))
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct EstadoDoTailscale { pub pronto: bool, pub login: Option<String>, pub dns: Option<String>, pub ips: Vec<String>, pub erro: Option<String> }

/// Só o que a tela precisa: rodando?, login (para a lista), nome na tailnet e IPs. Nada vai para o log.
pub fn ler_tailscale() -> EstadoDoTailscale {
    let Some(exe) = tailscale() else { return EstadoDoTailscale { erro: Some("Tailscale não encontrado. Instale e entre na sua conta.".into()), ..Default::default() } };
    let Ok(saida) = crate::git_client::hidden(&mut Command::new(&exe)).args(["status", "--json"]).output() else {
        return EstadoDoTailscale { erro: Some("Não consegui rodar o Tailscale.".into()), ..Default::default() };
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&saida.stdout) else { return EstadoDoTailscale { erro: Some("O Tailscale não respondeu.".into()), ..Default::default() } };
    if v["BackendState"] != "Running" { return EstadoDoTailscale { erro: Some("O Tailscale está desligado. Ligue e entre na conta.".into()), ..Default::default() }; }
    let eu = &v["Self"];
    let usuario = eu["UserID"].as_u64().map(|id| id.to_string());
    let login = usuario.and_then(|id| v["User"][&id]["LoginName"].as_str().map(str::to_owned));
    EstadoDoTailscale {
        pronto: true, login,
        dns: eu["DNSName"].as_str().map(|d| d.trim_end_matches('.').to_owned()).filter(|d| !d.is_empty()),
        ips: eu["TailscaleIPs"].as_array().into_iter().flatten().filter_map(|i| i.as_str()).filter(|i| !i.contains(':')).map(str::to_owned).collect(),
        erro: None,
    }
}

fn rodar_tailscale(args: &[String]) -> Result<(), String> {
    let exe = tailscale().ok_or("Tailscale não encontrado.")?;
    let saida = crate::git_client::hidden(&mut Command::new(exe)).args(args).output().map_err(|e| format!("Não consegui rodar o Tailscale: {e}"))?;
    if saida.status.success() { return Ok(()); }
    let erro = String::from_utf8_lossy(&saida.stderr).lines().find(|l| !l.trim().is_empty()).unwrap_or("sem detalhe").chars().take(300).collect::<String>();
    Err(format!("O Tailscale recusou: {erro}"))
}

pub fn argumentos_do_serve(porta_local: u16) -> Vec<String> {
    vec!["serve".into(), "--bg".into(), "--yes".into(), format!("--http={PORTA_NA_TAILNET}"), format!("http://127.0.0.1:{porta_local}")]
}
pub fn argumentos_do_serve_off() -> Vec<String> { vec!["serve".into(), format!("--http={PORTA_NA_TAILNET}"), "off".into()] }

// ——— configuração (nome e permitidos) ———

fn caminho_da_config() -> Result<PathBuf, String> { Ok(super::raiz()?.join(CONFIG)) }

pub fn ler_config() -> serde_json::Value {
    caminho_da_config().ok().and_then(|c| super::arquivos::ler_json(&c)).filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({ "versao": 1, "nome": "", "permitidos": [] }))
}

fn limpar_login(l: &str) -> Option<String> {
    let l = l.trim().to_ascii_lowercase();
    (!l.is_empty() && l.len() <= 200 && !l.contains(char::is_whitespace)).then_some(l)
}

fn gravar_config(nome: &str, permitidos: &[String]) -> Result<(), String> {
    let lista: Vec<String> = permitidos.iter().filter_map(|l| limpar_login(l)).collect();
    let _trava = super::arquivos::travar();
    super::arquivos::gravar_json(&caminho_da_config()?, &serde_json::json!({ "versao": 1, "nome": nome.chars().take(60).collect::<String>(), "permitidos": lista })).map_err(|e| e.to_string())
}

fn permitidos_da_config() -> Vec<String> {
    ler_config()["permitidos"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect()
}

fn aviso_da_tela(app: tauri::AppHandle) -> Aviso {
    Arc::new(move |e: EventoDaSessao| {
        // Só metadados no log: o tipo do evento, nunca login nem conteúdo.
        if !matches!(e, EventoDaSessao::Mensagem { .. }) {
            let tipo = serde_json::to_value(&e).ok().and_then(|v| v["tipo"].as_str().map(str::to_owned)).unwrap_or_default();
            omni_core::registro::info("video_sessao", &[("evento", &tipo)]);
        }
        let _ = app.emit(EVENTO, e);
    })
}

// ——— comandos ———

#[derive(serde::Serialize)]
pub struct Situacao { tailscale: EstadoDoTailscale, config: serde_json::Value, papel: Option<&'static str>, projeto: Option<String>, convidados: Vec<(String, String)> }

#[tauri::command(async)]
pub fn videos_sessao_situacao() -> Situacao {
    let (papel, projeto, convidados) = match &*estado().lock().unwrap_or_else(|e| e.into_inner()) {
        Some(Papel::Host(h)) => (Some("host"), Some(h.projeto.clone()), h.convidados()),
        Some(Papel::Convidado(_, p)) => (Some("convidado"), Some(p.clone()), Vec::new()),
        None => (None, None, Vec::new()),
    };
    Situacao { tailscale: ler_tailscale(), config: ler_config(), papel, projeto, convidados }
}

#[tauri::command(async)]
pub fn videos_sessao_config_gravar(nome: String, permitidos: Vec<String>) -> Result<(), String> {
    gravar_config(&nome, &permitidos)?;
    if let Some(Papel::Host(h)) = &*estado().lock().unwrap_or_else(|e| e.into_inner()) {
        let mut p = h.permitidos.lock().unwrap_or_else(|e| e.into_inner());
        *p = permitidos.iter().filter_map(|l| limpar_login(l)).collect();
    }
    Ok(())
}

/// Abre a sessão a dois deste projeto. Devolve o convite (para mandar ao outro pelo WhatsApp etc.).
#[tauri::command(async)]
pub fn videos_sessao_iniciar(app: tauri::AppHandle, projeto: String) -> Result<String, String> {
    if estado().lock().unwrap_or_else(|e| e.into_inner()).is_some() { return Err("Já tem uma sessão a dois aberta. Encerre antes.".into()); }
    let ts = ler_tailscale();
    if !ts.pronto { return Err(ts.erro.unwrap_or_else(|| "Tailscale indisponível.".into())); }
    let raiz = super::raiz()?;
    let pasta = super::projetos::pasta_do_projeto(&raiz, &projeto).map_err(super::texto_do_erro)?;
    if !pasta.join(super::projetos::PROJETO).is_file() { return Err("Esse projeto não existe.".into()); }
    // Antes de qualquer pessoa mexer junto: cópia do texto do projeto em ~/omni-backups/videos/.
    let casa = std::env::var_os("HOME").map(PathBuf::from).ok_or("Pasta do usuário indisponível")?;
    let carimbo = crate::backup_antes_de_atualizar::carimbo(std::time::SystemTime::now());
    super::projetos::copiar_para_backups(&raiz, &projeto, "sessao-a-dois", &casa.join("omni-backups"), &carimbo).map_err(super::texto_do_erro)?;
    let token = format!("{:032x}{:032x}", uuid::Uuid::new_v4().as_u128(), uuid::Uuid::new_v4().as_u128());
    let config = ler_config();
    let nome = config["nome"].as_str().filter(|n| !n.trim().is_empty()).map(str::to_owned).or_else(|| ts.login.clone()).unwrap_or_else(|| "Host".into());
    let host = Host::iniciar(&projeto, &nome, &pasta, &token, permitidos_da_config(), aviso_da_tela(app)).map_err(|e| format!("Não consegui abrir a sessão: {e}"))?;
    if let Err(e) = rodar_tailscale(&argumentos_do_serve(host.porta)) { host.parar(); return Err(e); }
    let mut bases: Vec<String> = ts.ips.iter().map(|ip| format!("http://{ip}:{PORTA_NA_TAILNET}")).collect();
    if let Some(dns) = &ts.dns { bases.push(format!("http://{dns}:{PORTA_NA_TAILNET}")); }
    let convite = montar_convite(&Convite { v: 1, bases, t: token, p: projeto, n: nome });
    *estado().lock().unwrap_or_else(|e| e.into_inner()) = Some(Papel::Host(host));
    omni_core::registro::info("video_sessao", &[("evento", "iniciou")]);
    Ok(convite)
}

#[tauri::command(async)]
pub fn videos_sessao_parar() -> Result<(), String> {
    let papel = estado().lock().unwrap_or_else(|e| e.into_inner()).take();
    match papel {
        Some(Papel::Host(h)) => { h.parar(); rodar_tailscale(&argumentos_do_serve_off()) }
        Some(Papel::Convidado(c, _)) => { c.sair(); Ok(()) }
        None => Ok(()),
    }
}

#[tauri::command(async)]
pub fn videos_sessao_permitir(login: String) -> Result<(), String> {
    let login = limpar_login(&login).ok_or("Login inválido.")?;
    let config = ler_config();
    let mut lista = permitidos_da_config();
    if !lista.contains(&login) { lista.push(login.clone()); }
    gravar_config(config["nome"].as_str().unwrap_or_default(), &lista)?;
    if let Some(Papel::Host(h)) = &*estado().lock().unwrap_or_else(|e| e.into_inner()) { h.permitir(&login); }
    Ok(())
}

/// Convidado: confere o convite, fala com o host e fica ouvindo. Devolve o `ola` do host.
#[tauri::command(async)]
pub fn videos_sessao_entrar(app: tauri::AppHandle, convite: String, projeto_local: String) -> Result<serde_json::Value, String> {
    if estado().lock().unwrap_or_else(|e| e.into_inner()).is_some() { return Err("Já tem uma sessão a dois aberta. Saia antes.".into()); }
    let c = ler_convite(&convite)?;
    let cliente = format!("c{:x}", uuid::Uuid::new_v4().as_u128() as u64);
    let mut ultimo_erro = String::new();
    for base in &c.bases {
        let conv = Convidado::new(base, &c.t, &cliente);
        match conv.ola() {
            Ok(ola) => {
                conv.ouvir(aviso_da_tela(app));
                *estado().lock().unwrap_or_else(|e| e.into_inner()) = Some(Papel::Convidado(conv, projeto_local));
                omni_core::registro::info("video_sessao", &[("evento", "entrou")]);
                return Ok(serde_json::json!({ "ola": ola, "cliente": cliente, "projetoRemoto": c.p, "nomeDoHost": c.n }));
            }
            Err(e) => ultimo_erro = e,
        }
    }
    Err(ultimo_erro)
}

/// Convidado: lê o convite sem entrar (a tela mostra de quem é antes de criar a cópia local).
#[tauri::command]
pub fn videos_sessao_ler_convite(convite: String) -> Result<Convite, String> { ler_convite(&convite).map(|mut c| { c.t = String::new(); c }) }

/// Manda uma mensagem (Yjs, presença, trava) para os outros da sessão.
#[tauri::command(async)]
pub fn videos_sessao_enviar(corpo: String) -> Result<(), String> {
    if corpo.len() > super::sessao_http::MENSAGEM_MAXIMA { return Err("Mensagem grande demais.".into()); }
    let papel = match &*estado().lock().unwrap_or_else(|e| e.into_inner()) {
        Some(Papel::Host(h)) => Some(Ok(h.clone())),
        Some(Papel::Convidado(c, _)) => Some(Err(c.clone())),
        None => None,
    };
    match papel {
        Some(Ok(h)) => { h.difundir("host", None, &corpo, None); Ok(()) }
        Some(Err(c)) => c.enviar(&corpo),
        None => Err("Nenhuma sessão a dois aberta.".into()),
    }
}

fn assets_do(projeto: &str) -> Result<PathBuf, String> {
    let raiz = super::raiz()?;
    Ok(super::projetos::pasta_do_projeto(&raiz, projeto).map_err(super::texto_do_erro)?.join("assets"))
}

fn convidado() -> Result<Arc<Convidado>, String> {
    match &*estado().lock().unwrap_or_else(|e| e.into_inner()) {
        Some(Papel::Convidado(c, _)) => Ok(c.clone()),
        _ => Err("Só quem entrou numa sessão baixa ou sobe mídia.".into()),
    }
}

#[tauri::command(async)]
pub fn videos_sessao_baixar(projeto: String, arquivo: String, hash: String) -> Result<(), String> {
    convidado()?.baixar(&arquivo, &hash, &assets_do(&projeto)?)
}

#[tauri::command(async)]
pub fn videos_sessao_subir(projeto: String, arquivo: String, hash: String) -> Result<(), String> {
    convidado()?.subir(&arquivo, &hash, &assets_do(&projeto)?)
}

#[cfg(test)]
#[path = "sessao_testes.rs"]
mod testes;
