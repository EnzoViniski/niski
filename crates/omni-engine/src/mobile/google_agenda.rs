//! Fork (Google Agenda): liga a Rotina ao Google Agenda pessoal do usuário.
//!
//! - Login de app para computador do Google (redirecionamento para `127.0.0.1:<porta>` + PKCE), com o
//!   cliente OAuth que o próprio usuário cria no Google Cloud dele. Escopo só de eventos
//!   (`calendar.events`) da agenda principal.
//! - Credenciais em `rotina/google.json` (modo 0600): nunca em resposta, log nem tela. Fora dos
//!   JSON que o agente da rotina enxerga (`orquestracao_rotina.rs` põe a CLI fora da pasta de dados).
//! - A cada 5 minutos (e logo depois de conectar ou criar um compromisso) busca os eventos de 7 dias
//!   atrás a 120 dias à frente e grava a cópia que a Rotina mostra (`omni_core::rotina::google`).
//! - Rotas: `GET /rotina/google`, `POST /rotina/google/{conectar,desconectar}` só pelo desktop;
//!   `POST /rotina/google/{sincronizar,evento}` também pelo celular pareado.
//!
//! Erros para a tela são frases fixas: nada da resposta do Google (que pode citar o token) sai daqui.
use super::*;
use omni_core::rotina::{self as rotina_core, google::{self, AgendaDoGoogle}};
use sha2::Digest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const ARQUIVO: &str = "google.json";
/// Eventos (ler e criar) e, desde a 0.32, a lista das agendas para ler todas as visíveis.
const ESCOPO: &str = "https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/calendar.calendarlist.readonly";
const AGENDAS_MAXIMAS: usize = 25;
const PRECISA_RECONECTAR: &str = "Para ver todas as suas agendas (não só a principal), clique em Desconectar e depois em Conectar: o Niski precisa de uma permissão nova.";
const INTERVALO: std::time::Duration = std::time::Duration::from_secs(300);
const ESPERA_DO_LOGIN: std::time::Duration = std::time::Duration::from_secs(300);
const DIAS_ANTES: i64 = 7;
const DIAS_DEPOIS: i64 = 120;
const PAGINAS_MAXIMAS: usize = 10;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Credenciais {
    client_id: String,
    client_secret: String,
    refresh_token: String,
    access_token: String,
    expira_ms: u64,
}

/// Endereços do Google. Os testes trocam por um servidor local.
#[derive(Clone)]
pub(in crate::mobile) struct Enderecos {
    pub autorizar: String,
    pub token: String,
    pub api: String,
    pub revogar: String,
}

impl Default for Enderecos {
    fn default() -> Self {
        Self {
            autorizar: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            api: "https://www.googleapis.com/calendar/v3".into(),
            revogar: "https://oauth2.googleapis.com/revoke".into(),
        }
    }
}

#[derive(Default)]
struct Login {
    conectando: bool,
    erro: Option<String>,
    /// Muda a cada "Conectar"/"Desconectar": o login antigo que ainda esperava o navegador desiste.
    geracao: u64,
}

#[derive(Default)]
pub(in crate::mobile) struct Runtime {
    pub(in crate::mobile) enderecos: Mutex<Enderecos>,
    login: Mutex<Login>,
    /// Uma conversa com o Google por vez (renovar token, buscar, criar).
    operacao: tokio::sync::Mutex<()>,
}

fn runtime(state: &EngineState) -> &Runtime {
    &state.mobile.google_agenda
}

fn dir(state: &EngineState) -> std::path::PathBuf {
    state.state_file.parent().unwrap().to_path_buf()
}

fn enderecos(state: &EngineState) -> Enderecos {
    runtime(state).enderecos.lock().expect("google poisoned").clone()
}

/* ── Credenciais (0600) ────────────────────────────────────────────────────────────────────── */

fn caminho(dados: &Path) -> std::path::PathBuf {
    rotina_core::pasta(dados).join(ARQUIVO)
}

fn ler_credenciais(dados: &Path) -> Credenciais {
    omni_protocol::read_json_or_default(&caminho(dados))
}

fn salvar_credenciais(dados: &Path, c: &Credenciais) -> std::io::Result<()> {
    use std::io::Write;
    let pasta = rotina_core::pasta(dados);
    fs::create_dir_all(&pasta)?;
    let temporario = pasta.join(format!(".google-{}.tmp", uuid::Uuid::new_v4()));
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    let resultado = (|| {
        let mut arquivo = opcoes.open(&temporario)?;
        arquivo.write_all(&serde_json::to_vec(c)?)?;
        arquivo.sync_all()?;
        fs::rename(&temporario, caminho(dados))
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporario);
    }
    resultado
}

/* ── HTTP ──────────────────────────────────────────────────────────────────────────────────── */

fn cliente() -> Result<reqwest::Client, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .connect_timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Não consegui iniciar a conexão com o Google.".to_owned())
}

/// Percent-encoding de formulário/consulta (só os não reservados passam como estão).
fn codificar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            saida.push(b as char);
        } else {
            saida.push_str(&format!("%{b:02X}"));
        }
    }
    saida
}

fn decodificar(texto: &str) -> String {
    let bytes = texto.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => saida.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(v) => { saida.push(v); i += 2; }
                    None => saida.push(b'%'),
                }
            }
            b => saida.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}

fn formulario(campos: &[(&str, &str)]) -> String {
    campos.iter().map(|(k, v)| format!("{}={}", codificar(k), codificar(v))).collect::<Vec<_>>().join("&")
}

async fn postar_formulario(url: &str, corpo: String) -> Result<(u16, Value), String> {
    let resposta = cliente()?.post(url).header("content-type", "application/x-www-form-urlencoded").body(corpo).send().await
        .map_err(|_| "Sem conexão com o Google. Tento de novo em alguns minutos.".to_owned())?;
    let status = resposta.status().as_u16();
    Ok((status, resposta.json().await.unwrap_or(Value::Null)))
}

/// Token de acesso válido, renovando com o `refresh_token` quando falta menos de 1 minuto.
async fn token_de_acesso(state: &EngineState) -> Result<String, String> {
    let dados = dir(state);
    let mut c = ler_credenciais(&dados);
    if c.refresh_token.is_empty() {
        return Err("Conecte o Google Agenda nos Ajustes da Rotina.".into());
    }
    if !c.access_token.is_empty() && c.expira_ms > now_ms() + 60_000 {
        return Ok(c.access_token);
    }
    let corpo = formulario(&[("grant_type", "refresh_token"), ("refresh_token", &c.refresh_token), ("client_id", &c.client_id),
        ("client_secret", &c.client_secret)]);
    let (status, valor) = postar_formulario(&enderecos(state).token, corpo).await?;
    let token = valor["access_token"].as_str().filter(|t| !t.is_empty());
    let Some(token) = token.filter(|_| (200..300).contains(&status)) else {
        return Err(if valor["error"] == "invalid_grant" || valor["error"] == "invalid_client" {
            "O Google pediu para conectar de novo (a autorização expirou ou foi removida).".into()
        } else {
            "O Google não renovou o acesso. Tento de novo em alguns minutos.".into()
        });
    };
    c.access_token = token.to_owned();
    c.expira_ms = now_ms() + valor["expires_in"].as_u64().unwrap_or(3600) * 1000;
    salvar_credenciais(&dados, &c).map_err(|_| "Não consegui guardar o acesso ao Google.".to_owned())?;
    Ok(c.access_token)
}

/* ── Sincronizar ───────────────────────────────────────────────────────────────────────────── */

/// Uma agenda da lista do usuário.
struct Agenda {
    id: String,
    nome: String,
    cor: String,
    principal: bool,
}

/// Fork (0.32): as agendas marcadas como visíveis no Google Agenda (`selected`) e a principal, a principal
/// primeiro. Conexão feita antes da 0.32 não tem a permissão da lista: busca só a principal e devolve o aviso.
async fn agendas(http: &reqwest::Client, token: &str, api: &str) -> Result<(Vec<Agenda>, Option<String>), String> {
    let principal = || Agenda { id: "primary".into(), nome: String::new(), cor: String::new(), principal: true };
    let resposta = http.get(format!("{api}/users/me/calendarList?maxResults=250")).bearer_auth(token).send().await
        .map_err(|_| "Sem conexão com o Google. Tento de novo em alguns minutos.".to_owned())?;
    let status = resposta.status().as_u16();
    let valor: Value = resposta.json().await.unwrap_or(Value::Null);
    if status == 403 && recusado(status, &valor).contains("permissão da agenda") {
        return Ok((vec![principal()], Some(PRECISA_RECONECTAR.into())));
    }
    if status == 401 || status == 403 {
        return Err(recusado(status, &valor));
    }
    if !(200..300).contains(&status) {
        return Err("O Google Agenda não respondeu. Tento de novo em alguns minutos.".into());
    }
    let mut lista: Vec<Agenda> = valor["items"].as_array().into_iter().flatten()
        .filter(|a| a["selected"] == true || a["primary"] == true)
        .filter_map(|a| {
            let id = a["id"].as_str().filter(|i| !i.is_empty())?;
            let nome = a["summaryOverride"].as_str().or(a["summary"].as_str()).unwrap_or("");
            let cor = a["backgroundColor"].as_str().filter(|c| c.len() == 7 && c.starts_with('#')).unwrap_or("");
            Some(Agenda { id: id.into(), nome: nome.chars().take(80).collect(), cor: cor.into(), principal: a["primary"] == true })
        })
        .collect();
    lista.sort_by_key(|a| !a.principal);
    lista.truncate(AGENDAS_MAXIMAS);
    if !lista.iter().any(|a| a.principal) {
        lista.insert(0, principal());
    }
    Ok((lista, None))
}

/// Os eventos de uma agenda no período (com as páginas). `Ok(None)` = agenda que sumiu ou não abre mais (pula).
async fn eventos_da_agenda(http: &reqwest::Client, token: &str, api: &str, agenda: &Agenda, de: &str, ate: &str)
    -> Result<Option<(String, Vec<Value>)>, String> {
    // Aniversários e eventos do Gmail só vêm pedindo os tipos (o padrão do Google deixa os aniversários de fora).
    let tipos = if agenda.principal { "&eventTypes=default&eventTypes=birthday&eventTypes=fromGmail&eventTypes=outOfOffice&eventTypes=focusTime" } else { "" };
    let base = format!("{api}/calendars/{}/events?singleEvents=true&orderBy=startTime&maxResults=2500&timeMin={}&timeMax={}{tipos}",
        codificar(&agenda.id), codificar(de), codificar(ate));
    let (mut itens, mut nome, mut pagina) = (Vec::new(), String::new(), None::<String>);
    for _ in 0..PAGINAS_MAXIMAS {
        let url = match &pagina { Some(p) => format!("{base}&pageToken={}", codificar(p)), None => base.clone() };
        let resposta = http.get(&url).bearer_auth(token).send().await
            .map_err(|_| "Sem conexão com o Google. Tento de novo em alguns minutos.".to_owned())?;
        let status = resposta.status().as_u16();
        let valor: Value = resposta.json().await.unwrap_or(Value::Null);
        if status == 401 || (status == 403 && agenda.principal) {
            return Err(recusado(status, &valor));
        }
        if matches!(status, 403 | 404 | 410) {
            return Ok(None);
        }
        if !(200..300).contains(&status) {
            return Err("O Google Agenda não respondeu. Tento de novo em alguns minutos.".into());
        }
        if nome.is_empty() {
            nome = valor["summary"].as_str().unwrap_or("").chars().take(120).collect();
        }
        itens.extend(valor["items"].as_array().cloned().unwrap_or_default());
        pagina = valor["nextPageToken"].as_str().map(str::to_owned);
        if pagina.is_none() {
            break;
        }
    }
    Ok(Some((nome, itens)))
}

/// Devolve (conta, eventos de todas as agendas visíveis, aviso).
async fn buscar(state: &EngineState) -> Result<(String, Vec<google::Evento>, Option<String>), String> {
    let token = token_de_acesso(state).await?;
    let api = enderecos(state).api;
    let agora = chrono::Local::now();
    let (de, ate) = ((agora - chrono::Duration::days(DIAS_ANTES)).to_rfc3339(), (agora + chrono::Duration::days(DIAS_DEPOIS)).to_rfc3339());
    let http = cliente()?;
    let (lista, aviso) = agendas(&http, &token, &api).await?;
    let (mut conta, mut eventos) = (String::new(), Vec::new());
    let mut vistos = std::collections::HashSet::new();
    for agenda in &lista {
        let Some((nome, itens)) = eventos_da_agenda(&http, &token, &api, agenda, &de, &ate).await? else { continue };
        if agenda.principal && conta.is_empty() {
            conta = nome;
        }
        for mut e in google::eventos_de(&itens, &chrono::Local) {
            // O mesmo convite aparece em mais de uma agenda (a sua e a compartilhada): fica o primeiro.
            if !vistos.insert((e.titulo.clone(), e.data.clone(), e.inicio.clone())) {
                continue;
            }
            if !agenda.principal {
                e.agenda = agenda.nome.clone();
            }
            e.cor = agenda.cor.clone();
            eventos.push(e);
        }
    }
    eventos.sort_by(|a, b| (a.data.as_str(), a.inicio.as_str()).cmp(&(b.data.as_str(), b.inicio.as_str())));
    Ok((conta, eventos, aviso))
}

/// Por que o Google recusou (401/403), numa frase que diz o que fazer. Lê só os códigos de motivo da
/// resposta (`errors[].reason`, `details[].reason`), nunca a mensagem dela.
fn recusado(status: u16, valor: &Value) -> String {
    let motivos: Vec<&str> = valor["error"]["errors"].as_array().into_iter().flatten().chain(valor["error"]["details"].as_array().into_iter().flatten())
        .filter_map(|e| e["reason"].as_str()).collect();
    let tem = |nomes: &[&str]| motivos.iter().any(|m| nomes.contains(m));
    if tem(&["accessNotConfigured", "SERVICE_DISABLED"]) {
        "A Google Calendar API não está ativada no seu projeto do Google Cloud. Ative-a (passo 2 do guia) e clique em Atualizar agora.".into()
    } else if tem(&["insufficientPermissions", "ACCESS_TOKEN_SCOPE_INSUFFICIENT"]) {
        "Faltou marcar a permissão da agenda no login do Google. Desconecte e conecte de novo, marcando o acesso ao Google Agenda.".into()
    } else if status == 401 {
        "O Google pediu para conectar de novo. Clique em Desconectar e depois em Conectar.".into()
    } else {
        "O Google recusou o acesso à agenda. Desconecte e conecte de novo nos Ajustes da Rotina.".into()
    }
}

async fn sincronizar_dentro(state: &EngineState) -> Result<usize, String> {
    let resultado = buscar(state).await;
    let dados = dir(state);
    let para_gravar = resultado.clone();
    tokio::task::spawn_blocking(move || {
        let _trava = rotina_core::travar();
        rotina_core::mudar::<AgendaDoGoogle, _>(&dados, |a| {
            match para_gravar {
                Ok((conta, eventos, aviso)) => {
                    if !conta.is_empty() { a.conta = conta; }
                    a.eventos = eventos;
                    a.erro = aviso.unwrap_or_default();
                    a.sincronizado_ms = now_ms() as i64;
                }
                Err(e) => a.erro = e, // os eventos anteriores continuam na tela
            }
            Ok(())
        })
    }).await.map_err(|_| "Falha interna".to_owned())??;
    resultado.map(|(_, eventos, _)| eventos.len())
}

pub(in crate::mobile) async fn sincronizar(state: &EngineState) -> Result<usize, String> {
    let _operacao = runtime(state).operacao.lock().await;
    sincronizar_dentro(state).await
}

fn conectado(state: &EngineState) -> bool {
    !ler_credenciais(&dir(state)).refresh_token.is_empty()
}

pub(in crate::mobile) fn iniciar(state: Arc<EngineState>) {
    tokio::spawn(async move {
        let mut intervalo = tokio::time::interval(INTERVALO);
        intervalo.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            intervalo.tick().await;
            if conectado(&state) && sincronizar(&state).await.is_err() {
                registro::aviso("google_agenda_nao_sincronizou", &[]);
            }
        }
    });
}

/* ── Criar compromisso ─────────────────────────────────────────────────────────────────────── */

#[derive(Default, Deserialize)]
#[serde(default)]
pub(in crate::mobile) struct NovoCompromisso {
    titulo: String,
    /// `AAAA-MM-DD`.
    data: String,
    /// `HH:MM`; vazio = dia inteiro.
    inicio: String,
    /// `HH:MM`; vazio = 1 hora depois do início.
    fim: String,
    local: String,
    notas: String,
}

fn quando(data: chrono::NaiveDate, hora: chrono::NaiveTime) -> Result<String, String> {
    use chrono::TimeZone;
    chrono::Local.from_local_datetime(&data.and_time(hora)).earliest().map(|d| d.to_rfc3339())
        .ok_or_else(|| "Esse horário não existe no fuso do computador.".to_owned())
}

fn corpo_do_evento(p: &NovoCompromisso) -> Result<Value, String> {
    let titulo = p.titulo.trim();
    if titulo.is_empty() || titulo.chars().count() > 200 {
        return Err("Dê um título ao compromisso (até 200 letras).".into());
    }
    let data = rotina_core::data(&p.data).ok_or("Escolha a data do compromisso.")?;
    let (inicio, fim) = if p.inicio.trim().is_empty() {
        (json!({"date": rotina_core::texto_da_data(data)}), json!({"date": rotina_core::texto_da_data(data + chrono::Duration::days(1))}))
    } else {
        let ini = rotina_core::hora(&p.inicio).ok_or("Hora de início inválida.")?;
        let fim = if p.fim.trim().is_empty() {
            // 1 hora, sem passar da meia-noite.
            match ini.overflowing_add_signed(chrono::Duration::hours(1)) {
                (f, 0) => f,
                _ => chrono::NaiveTime::from_hms_opt(23, 59, 0).unwrap_or(ini),
            }
        } else {
            rotina_core::hora(&p.fim).ok_or("Hora de fim inválida.")?
        };
        if fim <= ini {
            return Err("O fim precisa ser depois do início.".into());
        }
        (json!({"dateTime": quando(data, ini)?}), json!({"dateTime": quando(data, fim)?}))
    };
    let mut corpo = json!({"summary": titulo, "start": inicio, "end": fim});
    let local: String = p.local.trim().chars().take(300).collect();
    if !local.is_empty() { corpo["location"] = json!(local); }
    let notas: String = p.notas.trim().chars().take(4000).collect();
    if !notas.is_empty() { corpo["description"] = json!(notas); }
    Ok(corpo)
}

pub(in crate::mobile) async fn criar(state: &EngineState, p: NovoCompromisso) -> Result<Value, String> {
    let corpo = corpo_do_evento(&p)?;
    let _operacao = runtime(state).operacao.lock().await;
    let token = token_de_acesso(state).await?;
    let resposta = cliente()?.post(format!("{}/calendars/primary/events", enderecos(state).api)).bearer_auth(&token).json(&corpo).send().await
        .map_err(|_| "Sem conexão com o Google. O compromisso não foi criado.".to_owned())?;
    let status = resposta.status().as_u16();
    let valor: Value = resposta.json().await.unwrap_or(Value::Null);
    if !(200..300).contains(&status) {
        return Err(if status == 401 || status == 403 {
            recusado(status, &valor)
        } else {
            "O Google Agenda não aceitou o compromisso. Tente de novo.".into()
        });
    }
    let _ = sincronizar_dentro(state).await; // já aparece na Rotina
    Ok(json!({"ok": true, "id": valor["id"].as_str().unwrap_or(""), "titulo": p.titulo.trim(), "data": p.data}))
}

/// Ferramenta `criar_compromisso_google` do agente da rotina.
pub(in crate::mobile) async fn criar_pelo_agente(state: &EngineState, args: &Value) -> Result<String, String> {
    let p: NovoCompromisso = serde_json::from_value(args.clone()).map_err(|_| "Campos: titulo, data (AAAA-MM-DD), inicio e fim (HH:MM), local, notas".to_owned())?;
    criar(state, p).await.map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
}

/* ── Conectar (login no navegador) ─────────────────────────────────────────────────────────── */

fn aleatorio(n: usize) -> String {
    use rand::Rng;
    rand::rng().sample_iter(rand::distr::Alphanumeric).take(n).map(char::from).collect()
}

fn desafio_pkce(verificador: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verificador.as_bytes()))
}

struct Pendente {
    client_id: String,
    client_secret: String,
    verificador: String,
    estado: String,
    redirecionar: String,
    geracao: u64,
}

fn pagina(titulo: &str, texto: &str) -> String {
    let corpo = format!("<!doctype html><meta charset=utf-8><title>{titulo}</title><body style=\"font-family:-apple-system,sans-serif;\
        max-width:32rem;margin:4rem auto;padding:0 1rem;color:#222\"><h1 style=\"font-size:1.3rem\">{titulo}</h1><p>{texto}</p></body>");
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}", corpo.len())
}

/// Os parâmetros da linha `GET /?a=b HTTP/1.1` (só do caminho `/`).
fn parametros(cabecalho: &str) -> Option<HashMap<String, String>> {
    let alvo = cabecalho.lines().next()?.strip_prefix("GET ")?.split(' ').next()?;
    let (caminho, consulta) = alvo.split_once('?').unwrap_or((alvo, ""));
    if caminho != "/" {
        return None;
    }
    Some(consulta.split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (decodificar(k), decodificar(v))).collect())
}

async fn ler_pedido(conexao: &mut tokio::net::TcpStream) -> Option<String> {
    let mut buffer = Vec::new();
    let mut pedaco = [0u8; 2048];
    let leitura = async {
        while buffer.len() < 16 * 1024 {
            let n = conexao.read(&mut pedaco).await.ok()?;
            if n == 0 { break; }
            buffer.extend_from_slice(&pedaco[..n]);
            if buffer.windows(4).any(|w| w == b"\r\n\r\n") { break; }
        }
        Some(())
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), leitura).await.ok()??;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

fn terminar_login(state: &EngineState, geracao: u64, erro: Option<String>) {
    let mut login = runtime(state).login.lock().expect("google poisoned");
    if login.geracao == geracao {
        login.conectando = false;
        login.erro = erro;
    }
}

async fn trocar_codigo(state: &EngineState, p: &Pendente, codigo: &str) -> Result<(), String> {
    let corpo = formulario(&[("grant_type", "authorization_code"), ("code", codigo), ("client_id", &p.client_id),
        ("client_secret", &p.client_secret), ("code_verifier", &p.verificador), ("redirect_uri", &p.redirecionar)]);
    let (status, valor) = postar_formulario(&enderecos(state).token, corpo).await?;
    if !(200..300).contains(&status) {
        return Err(if valor["error"] == "invalid_client" || valor["error"] == "unauthorized_client" {
            "O Google não reconheceu o ID ou a chave secreta do cliente. Confira e cole de novo.".into()
        } else {
            "O Google não concluiu o login. Tente conectar de novo.".into()
        });
    }
    let refresh = valor["refresh_token"].as_str().unwrap_or("");
    if refresh.is_empty() {
        return Err("O Google não mandou a autorização permanente. Remova o acesso do app em myaccount.google.com/permissions e conecte de novo.".into());
    }
    let c = Credenciais { client_id: p.client_id.clone(), client_secret: p.client_secret.clone(), refresh_token: refresh.into(),
        access_token: valor["access_token"].as_str().unwrap_or("").into(),
        expira_ms: now_ms() + valor["expires_in"].as_u64().unwrap_or(3600) * 1000 };
    salvar_credenciais(&dir(state), &c).map_err(|_| "Não consegui guardar o acesso ao Google.".to_owned())
}

async fn esperar_login(state: Arc<EngineState>, ouvinte: tokio::net::TcpListener, p: Pendente) {
    let limite = tokio::time::Instant::now() + ESPERA_DO_LOGIN;
    loop {
        let Ok(Ok((mut conexao, _))) = tokio::time::timeout_at(limite, ouvinte.accept()).await else {
            terminar_login(&state, p.geracao, Some("O tempo para entrar na conta Google acabou. Clique em Conectar de novo.".into()));
            return;
        };
        if runtime(&state).login.lock().expect("google poisoned").geracao != p.geracao {
            return; // outro "Conectar" ou "Desconectar" veio depois
        }
        let Some(pedido) = ler_pedido(&mut conexao).await else { continue };
        let Some(campos) = parametros(&pedido) else {
            let _ = conexao.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            continue;
        };
        if campos.get("state") != Some(&p.estado) {
            let _ = conexao.write_all(pagina("Pedido inválido", "Volte ao Niski e clique em Conectar de novo.").as_bytes()).await;
            continue;
        }
        let resultado = match campos.get("code") {
            Some(codigo) => trocar_codigo(&state, &p, codigo).await,
            None => Err("O acesso ao Google Agenda não foi autorizado.".into()),
        };
        let resposta = match &resultado {
            Ok(()) => pagina("Google Agenda conectado", "Pronto! Pode fechar esta aba e voltar ao Niski."),
            Err(e) => pagina("Não deu certo", e),
        };
        let _ = conexao.write_all(resposta.as_bytes()).await;
        let _ = conexao.shutdown().await;
        let erro = resultado.err();
        let ok = erro.is_none();
        terminar_login(&state, p.geracao, erro);
        registro::info("google_agenda_login", &[("ok", if ok { "sim" } else { "nao" })]);
        if ok {
            let _ = sincronizar(&state).await;
        }
        return;
    }
}

/* ── Rotas ─────────────────────────────────────────────────────────────────────────────────── */

type Desktop = Option<axum::Extension<orquestracao::jarvis::Desktop>>;
fn desktop(acesso: &Desktop) -> Result<(), ApiError> {
    if acesso.is_some() { Ok(()) } else { Err(error(StatusCode::FORBIDDEN, "Configure o Google Agenda pelo app do computador")) }
}

pub(in crate::mobile) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/rotina/google", get(status))
        .route("/rotina/google/conectar", post(conectar))
        .route("/rotina/google/desconectar", post(desconectar))
        .route("/rotina/google/sincronizar", post(sincronizar_rota))
        .route("/rotina/google/evento", post(evento))
}

fn situacao(state: &EngineState) -> Value {
    let dados = dir(state);
    let c = ler_credenciais(&dados);
    let agenda: AgendaDoGoogle = rotina_core::ler(&dados);
    let login = runtime(state).login.lock().expect("google poisoned");
    let erro = login.erro.clone().or_else(|| Some(agenda.erro.clone()).filter(|e| !e.is_empty() && !c.refresh_token.is_empty()));
    json!({"conectado": !c.refresh_token.is_empty(), "conectando": login.conectando, "conta": agenda.conta,
        "sincronizado_ms": agenda.sincronizado_ms, "eventos": agenda.eventos.len(), "erro": erro})
}

async fn status(State(state): State<Arc<EngineState>>, acesso: Desktop) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    Ok(Json(situacao(&state)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conectar { client_id: String, client_secret: String }

fn id_valido(id: &str) -> bool {
    id.len() < 300 && id.ends_with(".apps.googleusercontent.com") && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
}

async fn conectar(State(state): State<Arc<EngineState>>, acesso: Desktop, Json(p): Json<Conectar>) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let (client_id, client_secret) = (p.client_id.trim().to_owned(), p.client_secret.trim().to_owned());
    if !id_valido(&client_id) {
        return Err(error(StatusCode::BAD_REQUEST, "Cole o ID do cliente inteiro (termina em .apps.googleusercontent.com)"));
    }
    if client_secret.is_empty() || client_secret.len() > 200 || client_secret.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(error(StatusCode::BAD_REQUEST, "Cole a chave secreta do cliente"));
    }
    let ouvinte = tokio::net::TcpListener::bind("127.0.0.1:0").await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui abrir a porta local para o login"))?;
    let porta = ouvinte.local_addr().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui abrir a porta local para o login"))?.port();
    let redirecionar = format!("http://127.0.0.1:{porta}");
    let (verificador, estado) = (aleatorio(64), aleatorio(32));
    let url = format!("{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        enderecos(&state).autorizar, codificar(&client_id), codificar(&redirecionar), codificar(ESCOPO), desafio_pkce(&verificador), estado);
    let geracao = {
        let mut login = runtime(&state).login.lock().expect("google poisoned");
        login.geracao += 1;
        login.conectando = true;
        login.erro = None;
        login.geracao
    };
    tokio::spawn(esperar_login(state.clone(), ouvinte, Pendente { client_id, client_secret, verificador, estado, redirecionar, geracao }));
    let mut resposta = situacao(&state);
    resposta["url"] = json!(url);
    Ok(Json(resposta))
}

async fn desconectar(State(state): State<Arc<EngineState>>, acesso: Desktop) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let _operacao = runtime(&state).operacao.lock().await;
    let dados = dir(&state);
    let c = ler_credenciais(&dados);
    if !c.refresh_token.is_empty() {
        // Melhor esforço: tira o acesso no Google também. Sem rede, a credencial some daqui mesmo assim.
        let _ = postar_formulario(&enderecos(&state).revogar, formulario(&[("token", &c.refresh_token)])).await;
    }
    salvar_credenciais(&dados, &Credenciais::default()).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui apagar o acesso ao Google"))?;
    let limpar = dados.clone();
    let _ = tokio::task::spawn_blocking(move || { let _t = rotina_core::travar(); rotina_core::gravar(&limpar, &AgendaDoGoogle::default()) }).await;
    {
        let mut login = runtime(&state).login.lock().expect("google poisoned");
        login.geracao += 1;
        login.conectando = false;
        login.erro = None;
    }
    Ok(Json(situacao(&state)))
}

async fn sincronizar_rota(State(state): State<Arc<EngineState>>) -> Result<Json<Value>, ApiError> {
    let eventos = sincronizar(&state).await.map_err(|e| error(StatusCode::BAD_GATEWAY, &e))?;
    Ok(Json(json!({"ok": true, "eventos": eventos})))
}

async fn evento(State(state): State<Arc<EngineState>>, Json(p): Json<NovoCompromisso>) -> Result<Json<Value>, ApiError> {
    criar(&state, p).await.map(Json).map_err(|e| error(StatusCode::BAD_REQUEST, &e))
}

#[cfg(test)]
#[path = "google_agenda_testes.rs"]
mod testes;
