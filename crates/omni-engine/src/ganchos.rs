//! Fork (Onda 2, itens 1–3): servidor dos ganchos HTTP do Claude Code, pedidos de permissão e
//! statusline.
//!
//! - Porta própria: `127.0.0.1:<porta do engine + 4>` (`POST /ganchos`, `POST /ganchos/permissao`,
//!   `POST /statusline`). Só loopback com a porta certa no `Host` (a mesma defesa do MCP do orquestrador).
//! - Token **por PTY**, novo a cada spawn/reinício, só em memória e no ambiente da PTY
//!   (`MEDZA_GANCHO_TOKEN`); nunca o `engine.token`, nunca na linha digitada nem no `sessions.json`.
//!   PTY morta invalida o token (conferido a cada chamada contra `state.sessions`).
//! - Fonte de estado por sessão: a tela continua rodando sempre; os ganchos só **vencem** onde dizem
//!   algo (turno, pedido de permissão, fim com erro). Sem evento nenhum, tudo segue como antes.
//! - Log em disco só com nome de evento, ids e números (`gancho_confirmado`, `gancho_401`,
//!   `gancho_413`, `ganchos_sem_eventos`…). Nunca o corpo, `tool_input`, mensagem nem a statusline.
//!
//! O estado mora num `static` (só memória), indexado pelo id do terminal; a porta é por pasta de dados
//! do engine, então cada `EngineState` (inclusive os dos testes) só vê a sua.

use super::*;
use axum::{
    body::Bytes,
    extract::{rejection::BytesRejection, DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use omni_core::ganchos as regra;
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::HashSet, sync::LazyLock};
use tokio::sync::oneshot;

/// O que o usuário (ou o orquestrador) decidiu sobre um pedido de permissão.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Decisao {
    Permitir,
    Negar(Option<String>),
    /// "Responder no terminal": resposta vazia, a CLI mostra o diálogo dela.
    Soltar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ErroDeResposta {
    /// Não há pedido pendente (já respondido, solto ou vencido).
    Nenhum,
    /// O pedido pendente não é o que a tela mostrou: ninguém aprova às cegas.
    Outro,
}

struct Pedido {
    id: String,
    ferramenta: String,
    texto: String,
    servidor_mcp: Option<String>,
    arriscado: Option<&'static str>,
    expira_em_ms: u64,
    resposta: Option<oneshot::Sender<Decisao>>,
}

/// O pedido como a tela, o celular e o orquestrador veem (`capabilities.pedido`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PedidoVisivel {
    pub id: String,
    pub origem: &'static str,
    pub ferramenta: String,
    pub resumo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servidor_mcp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arriscado: Option<&'static str>,
    pub expira_em_ms: u64,
}

/// Uma linha do anel de eventos (só nomes e números).
#[derive(Clone, Debug, Serialize)]
struct Ocorrido { evento: String, ferramenta: Option<String>, em_ms: u64, agente: Option<String> }

const TAMANHO_DO_ANEL: usize = 200;
/// Turno que a tela viu começar sem nenhum evento depois disto: a sessão fica `sem_eventos`.
const SEM_EVENTOS_MS: u64 = if cfg!(test) { 800 } else { 5_000 };
/// Logo depois do `Stop`, a tela ainda pode mostrar o rodapé do turno por alguns quadros.
const DEPOIS_DO_FIM_MS: u64 = 2_000;

#[derive(Default)]
struct Estado {
    /// Motivo de não ter injetado ("configuracao", "usuario", "versao", "caminho").
    desligado: Option<&'static str>,
    injetado: bool,
    statusline: bool,
    arquivo: Option<PathBuf>,
    chave: String,
    perfil: Option<String>,
    confirmado: bool,
    turno_visto_em_ms: Option<u64>,
    sem_eventos_registrado: bool,
    em_turno: bool,
    fim_do_turno_ms: Option<u64>,
    /// Esc/Ctrl+C no meio do turno: o `Stop` não vem, então volta a valer a tela até o próximo prompt.
    interrompido: bool,
    aviso: Option<&'static str>,
    tipo: Option<&'static str>,
    ultimo_evento: Option<(String, u64)>,
    ferramenta_atual: Option<String>,
    modo_de_permissao: Option<String>,
    esforco: Option<String>,
    subagentes: HashMap<String, String>,
    session_id: Option<String>,
    pedidos: VecDeque<Pedido>,
    anel: VecDeque<Ocorrido>,
    medicao: Option<regra::Medicao>,
}

#[derive(Default)]
struct Runtime {
    /// Pasta de dados do engine → porta do servidor de ganchos.
    portas: Mutex<HashMap<PathBuf, u16>>,
    /// token → terminal.
    tokens: Mutex<HashMap<String, String>>,
    sessoes: Mutex<HashMap<String, Estado>>,
    /// "perfil|pasta" que já confirmaram ganchos: só aí a statusline pode entrar (ela esconde o rodapé
    /// "esc to interrupt", e sem ganchos o estado se perderia).
    confirmados: Mutex<HashSet<String>>,
    /// Versão da CLI por executável (caminho + data).
    versoes: Mutex<HashMap<(PathBuf, u64), Option<(u32, u32, u32)>>>,
    /// Ler-mudar-gravar do `uso-statusline.json`.
    trava_do_uso: Mutex<()>,
}

static RUNTIME: LazyLock<Runtime> = LazyLock::new(Runtime::default);

fn dir_de(state: &EngineState) -> PathBuf { state.state_file.parent().map(Path::to_path_buf).unwrap_or_default() }

fn porta_de(state: &EngineState) -> u16 {
    RUNTIME.portas.lock().ok().and_then(|p| p.get(&dir_de(state)).copied()).unwrap_or(0)
}

fn com_estado<T>(terminal: &str, f: impl FnOnce(&mut Estado) -> T) -> Option<T> {
    RUNTIME.sessoes.lock().ok()?.get_mut(terminal).map(f)
}

const ARQUIVO_DE_CONFIRMADOS: &str = "ganchos-confirmados.json";
const ARQUIVO_DE_USO: &str = "uso-statusline.json";

/* ── Subida ──────────────────────────────────────────────────────────────────────────────────── */

/// Sobe o servidor. Porta ocupada: nenhuma sessão ganha ganchos (fica no log), o resto segue.
pub(crate) fn iniciar(state: Arc<EngineState>, porta_do_engine: u16) {
    let porta = regra::porta_dos_ganchos(porta_do_engine);
    let dir = dir_de(&state);
    // Arquivos de sessões de um engine anterior: nenhuma PTY sobrevive a ele.
    if let Ok(entradas) = fs::read_dir(dir.join(regra::PASTA_DOS_ARQUIVOS)) {
        for entrada in entradas.flatten() { let _ = fs::remove_file(entrada.path()); }
    }
    if let Ok(lidos) = fs::read(dir.join(ARQUIVO_DE_CONFIRMADOS)).map(|b| serde_json::from_slice::<Vec<String>>(&b).unwrap_or_default()) {
        if let Ok(mut confirmados) = RUNTIME.confirmados.lock() { confirmados.extend(lidos); }
    }
    tokio::spawn(async move {
        match TcpListener::bind(("127.0.0.1", porta)).await {
            Ok(listener) => {
                definir_porta(&state, porta);
                registro::info("ganchos_iniciou", &[("porta", &porta.to_string())]);
                let _ = axum::serve(listener, router(state)).await;
            }
            Err(erro) => registro::erro("ganchos_porta_ocupada", &[("porta", &porta.to_string()), ("erro", &erro.to_string())]),
        }
    });
}

pub(crate) fn definir_porta(state: &EngineState, porta: u16) {
    if let Ok(mut portas) = RUNTIME.portas.lock() { portas.insert(dir_de(state), porta); }
}

pub(crate) fn router(state: Arc<EngineState>) -> Router {
    Router::new()
        .route("/ganchos", post(evento))
        .route("/ganchos/permissao", post(permissao))
        .route("/statusline", post(statusline))
        .layer(DefaultBodyLimit::max(regra::LIMITE_DO_CORPO))
        .with_state(state)
}

/* ── Preparar a PTY ──────────────────────────────────────────────────────────────────────────── */

/// Chamado a cada spawn/reinício de PTY (por `preparo_da_sessao`): o comando a digitar (com
/// ` --settings '<arquivo>'` quando cabem ganchos) e o ambiente extra (o token). O comando gravado no
/// `sessions.json` continua o original.
pub(crate) fn preparar(state: &EngineState, terminal: &str, origem: &SessionOrigin, cwd: &str, comando: Option<String>)
    -> (Option<String>, Vec<(String, String)>) {
    esquecer_token(terminal);
    let limpo = comando.as_deref().map(regra::sem_settings);
    let (Some(_), Some("claude"), Some(comando)) = (origem.conversation_id.as_deref(), origem.provider.as_deref(), limpo) else {
        esquecer(terminal);
        return (comando, Vec::new());
    };
    let porta = porta_de(state);
    if porta == 0 {
        esquecer(terminal);
        return (Some(comando), Vec::new());
    }
    let dir = dir_de(state);
    let desligar = |motivo: &'static str, comando: String| {
        novo_estado(terminal, Estado { desligado: Some(motivo), ..Estado::default() });
        registro::info("ganchos_desligados", &[("terminal", terminal), ("motivo", motivo)]);
        (Some(comando), Vec::new())
    };
    let ajustes = regra::ler_ajustes(&dir);
    if !ajustes.ligado { return desligar("configuracao", comando); }
    let pasta_da_conta = origem.env.iter().find(|(k, _)| k == "CLAUDE_CONFIG_DIR").map(|(_, v)| PathBuf::from(v))
        .or_else(|| env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")).map(|h| PathBuf::from(h).join(".claude")))
        .unwrap_or_default();
    let config = regra::ler_config_do_usuario(&regra::Arquivos::de(&pasta_da_conta, Path::new(cwd)));
    if config.ganchos_desligados || config.so_ganchos_gerenciados { return desligar("usuario", comando); }
    if versao_da_cli(&comando).is_some_and(|v| !regra::suporta_ganchos_http(v)) { return desligar("versao", comando); }

    let perfil = origem.profile_id.clone();
    let chave = format!("{}|{cwd}", perfil.as_deref().unwrap_or("-"));
    let confirmada = RUNTIME.confirmados.lock().is_ok_and(|c| c.contains(&chave));
    let statusline = perfil.as_ref().filter(|p| ajustes.statusline.contains(p)).filter(|_| confirmada && !config.statusline_gerenciada)
        .and_then(|_| caminho_do_shim()).and_then(|shim| regra::comando_da_statusline(&shim.to_string_lossy(), porta));
    let arquivo = dir.join(regra::PASTA_DOS_ARQUIVOS).join(format!("{terminal}.json"));
    let Some(digitado) = regra::com_settings(&comando, &arquivo) else { return desligar("caminho", comando) };
    let settings = regra::montar_settings(porta, ajustes.espera(), &config, statusline.as_deref());
    if let Err(erro) = gravar_privado(&arquivo, &settings) {
        registro::aviso("ganchos_arquivo_falhou", &[("terminal", terminal), ("erro", &erro.to_string())]);
        esquecer(terminal);
        return (Some(comando), Vec::new());
    }
    let token = random_token();
    if let Ok(mut tokens) = RUNTIME.tokens.lock() { tokens.insert(token.clone(), terminal.to_owned()); }
    let mut ambiente = vec![(regra::VARIAVEL_DO_TOKEN.to_owned(), token)];
    if statusline.is_some() {
        if let Some(dele) = regra::statusline_do_usuario(&config) { ambiente.push((regra::VARIAVEL_DA_STATUSLINE_DO_USUARIO.to_owned(), dele)); }
    }
    novo_estado(terminal, Estado { injetado: true, statusline: statusline.is_some(), arquivo: Some(arquivo), chave, perfil, ..Estado::default() });
    registro::info("ganchos_injetados", &[("terminal", terminal), ("statusline", if statusline.is_some() { "sim" } else { "nao" })]);
    (Some(digitado), ambiente)
}

fn novo_estado(terminal: &str, estado: Estado) {
    if let Ok(mut sessoes) = RUNTIME.sessoes.lock() { sessoes.insert(terminal.to_owned(), estado); }
}

/// Grava o arquivo da sessão só para o dono (0600). Não tem segredo, mas também não é de mais ninguém.
fn gravar_privado(arquivo: &Path, valor: &Value) -> std::io::Result<()> {
    if let Some(pasta) = arquivo.parent() { fs::create_dir_all(pasta)?; }
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create(true).truncate(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    opcoes.open(arquivo)?.write_all(&serde_json::to_vec_pretty(valor).map_err(std::io::Error::other)?)
}

/// Versão da CLI quando o comando começa por um `claude` achável no PATH (uma vez por executável).
/// Comando personalizado ou não achado: `None` — tenta e confirma em tempo de execução.
fn versao_da_cli(comando: &str) -> Option<(u32, u32, u32)> {
    if cfg!(test) || cfg!(windows) { return None; }
    let primeiro = comando.split_whitespace().next()?;
    if Path::new(primeiro).file_name()?.to_str()? != "claude" { return None; }
    let caminho = if primeiro.contains('/') { PathBuf::from(primeiro) } else {
        env::split_paths(&env::var_os("PATH")?).map(|d| d.join(primeiro)).find(|c| c.is_file())?
    };
    let data = omni_protocol::modified_ms(&caminho)?;
    let chave = (caminho.clone(), data);
    if let Some(v) = RUNTIME.versoes.lock().ok()?.get(&chave) { return *v; }
    let versao = saida_com_prazo(&caminho, std::time::Duration::from_secs(3)).and_then(|s| regra::versao_de(&s));
    RUNTIME.versoes.lock().ok()?.insert(chave, versao);
    versao
}

/// `<cli> --version` com prazo: uma CLI travada não pode segurar a abertura da sessão.
fn saida_com_prazo(caminho: &Path, prazo: std::time::Duration) -> Option<String> {
    let mut filho = std::process::Command::new(caminho).arg("--version").stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;
    let limite = std::time::Instant::now() + prazo;
    loop {
        match filho.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < limite => std::thread::sleep(std::time::Duration::from_millis(50)),
            _ => { let _ = filho.kill(); let _ = filho.wait(); return None; }
        }
    }
    let mut saida = String::new();
    filho.stdout.take()?.read_to_string(&mut saida).ok()?;
    Some(saida)
}

fn esquecer_token(terminal: &str) {
    if let Ok(mut tokens) = RUNTIME.tokens.lock() { tokens.retain(|_, dono| dono != terminal); }
}

/// Sessão fechada: token, estado e pedidos somem (quem esperava um pedido recebe resposta vazia), e o
/// arquivo é apagado.
pub(crate) fn esquecer(terminal: &str) {
    esquecer_token(terminal);
    let estado = RUNTIME.sessoes.lock().ok().and_then(|mut s| s.remove(terminal));
    if let Some(arquivo) = estado.and_then(|e| e.arquivo) { let _ = fs::remove_file(arquivo); }
}

/* ── O que o resto do engine consulta ────────────────────────────────────────────────────────── */

/// O turno pelos ganchos, quando eles valem: `None` = confiar na tela (sem ganchos confirmados, ou
/// depois de um Esc). Depois do `Stop`, `Some(false)` por um instante e depois a tela de novo.
pub(crate) fn em_turno(terminal: &str) -> Option<bool> {
    let sessoes = RUNTIME.sessoes.lock().ok()?;
    let e = sessoes.get(terminal)?;
    if !e.confirmado || e.interrompido { return None; }
    if e.em_turno { return Some(true); }
    e.fim_do_turno_ms.filter(|fim| now_ms().saturating_sub(*fim) < DEPOIS_DO_FIM_MS).map(|_| false)
}

/// O pedido de permissão pendente (o mais antigo), se houver.
pub(crate) fn pedido(terminal: &str) -> Option<PedidoVisivel> {
    let sessoes = RUNTIME.sessoes.lock().ok()?;
    let p = sessoes.get(terminal)?.pedidos.front()?;
    Some(PedidoVisivel { id: p.id.clone(), origem: "gancho", ferramenta: p.ferramenta.clone(), resumo: p.texto.clone(),
        servidor_mcp: p.servidor_mcp.clone(), arriscado: p.arriscado, expira_em_ms: p.expira_em_ms })
}

pub(crate) fn tem_pedido(terminal: &str) -> bool {
    RUNTIME.sessoes.lock().ok().and_then(|s| s.get(terminal).map(|e| !e.pedidos.is_empty())).unwrap_or(false)
}

/// Fork (para a frente "Atividade", 2B): o tipo de aviso que os ganchos dizem para esta sessão agora,
/// no vocabulário da caixa de Atividade (`precisa_de_voce`, `terminou`, `erro`, `limite`). `None` sem
/// ganchos confirmados ou sem nada a avisar (turno em andamento) — aí vale a leitura de hoje.
/// Para um evento solto (sem sessão), há `omni_core::ganchos::tipo(evento, notification_type, erro)`.
#[allow(dead_code)] // a 2B liga com uma linha depois de rebasear
pub fn tipo(terminal: &str) -> Option<&'static str> {
    let sessoes = RUNTIME.sessoes.lock().ok()?;
    let e = sessoes.get(terminal).filter(|e| e.confirmado)?;
    if !e.pedidos.is_empty() { return Some("precisa_de_voce"); }
    e.tipo
}

/// O `notice` que o `StopFailure` deixou neste turno (a tela pode não mostrar a linha do erro).
pub(crate) fn aviso(terminal: &str) -> Option<String> {
    com_estado(terminal, |e| e.aviso.map(str::to_owned)).flatten()
}

/// Esc ou Ctrl+C mandados para a PTY no meio de um turno: o `Stop` não vem nesse caso.
pub(crate) fn escreveu(terminal: &str, dados: &str) {
    if !matches!(dados, "\x1b" | "\x1b\x1b" | "\u{3}") { return; }
    com_estado(terminal, |e| if e.em_turno { e.em_turno = false; e.interrompido = true; });
}

/// A tela viu um turno começar. Sem evento nenhum em 5 s, a sessão fica `sem_eventos` (versão antiga,
/// pasta sem confiança, política da máquina) e segue pela tela.
pub(crate) fn tela_viu_turno(terminal: &str) {
    com_estado(terminal, |e| if e.injetado && !e.confirmado && e.turno_visto_em_ms.is_none() { e.turno_visto_em_ms = Some(now_ms()); });
}

/// O que a conversa mostra sobre os ganchos (`resumo` do chat).
pub(crate) fn resumo(terminal: &str) -> Value {
    let mut sessoes = match RUNTIME.sessoes.lock() { Ok(s) => s, Err(_) => return json!({"estado": "tela"}) };
    let Some(e) = sessoes.get_mut(terminal) else { return json!({"estado": "tela"}) };
    let sem_eventos = e.injetado && !e.confirmado && e.turno_visto_em_ms.is_some_and(|t| now_ms().saturating_sub(t) >= SEM_EVENTOS_MS);
    if sem_eventos && !e.sem_eventos_registrado {
        e.sem_eventos_registrado = true;
        registro::aviso("ganchos_sem_eventos", &[("terminal", terminal)]);
    }
    let estado = if e.desligado.is_some() { "desligados" } else if e.confirmado { "ativos" } else if sem_eventos { "sem_eventos" } else { "tela" };
    json!({
        "estado": estado,
        "motivo": e.desligado,
        "ferramenta": e.ferramenta_atual.as_deref().filter(|_| e.em_turno),
        "subagentes": e.subagentes.len(),
        "modo": e.modo_de_permissao,
        "esforco": e.esforco,
        "statusline": e.statusline,
        "contexto": e.medicao.as_ref().map(|m| json!({"pct": m.contexto_pct, "custo_usd": m.custo_usd, "modelo": m.modelo})),
        "ultimo_evento": e.ultimo_evento.as_ref().map(|(nome, _)| nome),
        "eventos": e.anel.len(),
    })
}

/// Responde o pedido pendente. `pedido`: o id que a tela mostrou (diferente do pendente → `Outro`).
pub(crate) fn responder(terminal: &str, pedido: Option<&str>, decisao: Decisao) -> Result<(), ErroDeResposta> {
    let remetente = {
        let mut sessoes = RUNTIME.sessoes.lock().map_err(|_| ErroDeResposta::Nenhum)?;
        let e = sessoes.get_mut(terminal).ok_or(ErroDeResposta::Nenhum)?;
        let primeiro = e.pedidos.front().ok_or(ErroDeResposta::Nenhum)?;
        if pedido.is_some_and(|id| id != primeiro.id) { return Err(ErroDeResposta::Outro); }
        e.pedidos.pop_front().and_then(|mut p| p.resposta.take())
    };
    // Quem esperava já desistiu (prazo, conexão caiu): para a tela, o pedido também já era.
    remetente.ok_or(ErroDeResposta::Nenhum)?.send(decisao).map_err(|_| ErroDeResposta::Nenhum)
}

/// Uso da conta pela statusline (o último gravado), para `contas` e o rodapé de uso.
pub(crate) fn uso_da_statusline(dir: &Path, perfil: &str) -> Option<omni_core::usage::AccountUsage> {
    let mapa: HashMap<String, omni_core::usage::AccountUsage> = serde_json::from_slice(&fs::read(dir.join(ARQUIVO_DE_USO)).ok()?).ok()?;
    mapa.get(perfil).cloned()
}

/* ── O servidor ──────────────────────────────────────────────────────────────────────────────── */

fn confere(recebido: &str, esperado: &str) -> bool {
    recebido.len() == esperado.len() && recebido.bytes().zip(esperado.bytes()).fold(0_u8, |a, (x, y)| a | (x ^ y)) == 0
}

fn viva(state: &EngineState, terminal: &str) -> Option<Arc<LiveSession>> {
    match state.sessions.lock().ok()?.get(terminal)? {
        SessionEntry::Live(sessao) => Some(sessao.clone()),
        SessionEntry::Historical(_) => None,
    }
}

/// Host/Origin loopback, token de uma PTY viva. Devolve o terminal.
#[allow(clippy::result_large_err)]
fn autorizar(state: &EngineState, headers: &HeaderMap) -> Result<String, Response> {
    if !crate::mobile::orquestracao::origem_local(headers, porta_de(state)) {
        return Err((StatusCode::FORBIDDEN, "origem não permitida").into_response());
    }
    let recebido = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
    let mut terminal = None;
    if !recebido.is_empty() {
        for (token, dono) in RUNTIME.tokens.lock().map(|t| t.clone()).unwrap_or_default() {
            if confere(recebido, &token) { terminal = Some(dono); }
        }
    }
    let viva = terminal.as_deref().and_then(|t| viva(state, t)).filter(|s| s.meta.lock().is_ok_and(|m| m.pid.is_some()));
    match (terminal, viva) {
        (Some(terminal), Some(_)) => Ok(terminal),
        (terminal, _) => {
            if let Some(morto) = terminal { esquecer_token(&morto); }
            registro::aviso_sem_repetir("gancho_401", "gancho_401", &[]);
            Err((StatusCode::UNAUTHORIZED, "token de gancho inválido").into_response())
        }
    }
}

fn corpo_valido(terminal: &str, corpo: Result<Bytes, BytesRejection>) -> Result<Value, Response> {
    let corpo = corpo.map_err(|_| {
        registro::aviso_sem_repetir(&format!("gancho_413:{terminal}"), "gancho_413", &[("terminal", terminal)]);
        StatusCode::PAYLOAD_TOO_LARGE.into_response()
    })?;
    serde_json::from_slice::<Value>(&corpo).map_err(|_| StatusCode::BAD_REQUEST.into_response())
}

/// O que um evento muda na sessão (aplicado fora da trava do `RUNTIME`).
enum Efeito { Nada, Comecou, Terminou(Option<&'static str>) }

async fn evento(State(state): State<Arc<EngineState>>, headers: HeaderMap, corpo: Result<Bytes, BytesRejection>) -> Response {
    let terminal = match autorizar(&state, &headers) { Ok(t) => t, Err(r) => return r };
    let valor = match corpo_valido(&terminal, corpo) { Ok(v) => v, Err(r) => return r };
    let Some(evento) = regra::Evento::de(&valor, 200) else { return StatusCode::BAD_REQUEST.into_response() };
    let _ = tokio::task::spawn_blocking(move || aplicar(&state, &terminal, &evento)).await;
    StatusCode::OK.into_response()
}

/// Registra o evento no estado da sessão e aplica o efeito no meta (turno, aviso, transcript).
fn aplicar(state: &EngineState, terminal: &str, evento: &regra::Evento) {
    let agora = now_ms();
    let Some((efeito, primeiro, chave, troca)) = com_estado(terminal, |e| {
        let primeiro = !e.confirmado;
        e.confirmado = true;
        e.ultimo_evento = Some((evento.nome.clone(), agora));
        if e.anel.len() >= TAMANHO_DO_ANEL { e.anel.pop_front(); }
        e.anel.push_back(Ocorrido { evento: evento.nome.clone(), ferramenta: evento.ferramenta.clone(), em_ms: agora, agente: evento.agent_type.clone() });
        if evento.permission_mode.is_some() { e.modo_de_permissao = evento.permission_mode.clone(); }
        if evento.esforco.is_some() { e.esforco = evento.esforco.clone(); }
        let troca = evento.session_id.as_ref().filter(|sid| e.session_id.as_ref() != Some(*sid))
            .map(|sid| (sid.clone(), evento.transcript_path.clone()));
        if let Some((sid, _)) = &troca { e.session_id = Some(sid.clone()); }
        if let Some(t) = regra::tipo(&evento.nome, evento.notification_type.as_deref(), evento.erro.as_deref()) { e.tipo = Some(t); }
        let efeito = match evento.nome.as_str() {
            "UserPromptSubmit" => {
                e.em_turno = true; e.interrompido = false; e.aviso = None; e.tipo = None; e.fim_do_turno_ms = None;
                Efeito::Comecou
            }
            "PreToolUse" => { e.ferramenta_atual = evento.resumo.clone(); Efeito::Nada }
            "PreCompact" if evento.gatilho.as_deref() == Some("manual") => { e.em_turno = true; Efeito::Nada }
            "PostCompact" if evento.gatilho.as_deref() == Some("manual") => { e.em_turno = false; e.fim_do_turno_ms = Some(agora); Efeito::Nada }
            "SubagentStart" => {
                if let Some(id) = &evento.agent_id { e.subagentes.insert(id.clone(), evento.agent_type.clone().unwrap_or_default()); }
                Efeito::Nada
            }
            "SubagentStop" => { if let Some(id) = &evento.agent_id { e.subagentes.remove(id); } Efeito::Nada }
            "Stop" | "StopFailure" | "SessionEnd" => {
                let estava = e.em_turno;
                e.em_turno = false; e.fim_do_turno_ms = Some(agora); e.ferramenta_atual = None; e.subagentes.clear();
                if evento.nome == "StopFailure" { e.aviso = Some(regra::aviso_da_falha(evento.erro.as_deref())); }
                if evento.nome == "SessionEnd" && !estava { Efeito::Nada } else { Efeito::Terminou(e.aviso) }
            }
            _ => Efeito::Nada,
        };
        (efeito, primeiro, e.chave.clone(), troca)
    }) else { return };
    if primeiro {
        registro::info("gancho_confirmado", &[("terminal", terminal), ("evento", &evento.nome)]);
        confirmar(state, &chave);
    }
    let Some(sessao) = viva(state, terminal) else { return };
    match efeito {
        Efeito::Nada => {}
        Efeito::Comecou => {
            let mut interaction = sessao.interaction.lock().expect("screen poisoned");
            let mut meta = sessao.meta.lock().expect("metadata poisoned");
            if !interaction.turn_active {
                interaction.avisos_antes_do_turno = notice_counts(&interaction.parser.screen().contents());
                interaction.turnos_iniciados += 1;
                interaction.turn_active = true;
            }
            interaction.aprovacao_contada = false;
            meta.state = SessionState::Working;
            meta.notice = None;
            meta.last_activity_at_ms = agora;
        }
        Efeito::Terminou(aviso) => {
            let mut interaction = sessao.interaction.lock().expect("screen poisoned");
            let mut meta = sessao.meta.lock().expect("metadata poisoned");
            // Uma vez por turno: a mesma trava que o fim pela tela (`settle`) usa.
            if interaction.turn_active {
                interaction.turn_active = false;
                interaction.aprovacao_contada = false;
                meta.attention_seq += 1;
            }
            if meta.state != SessionState::ApprovalRequired || !tem_pedido(terminal) { meta.state = SessionState::Answered; }
            if let Some(aviso) = aviso { meta.notice = Some(aviso.to_owned()); }
        }
    }
    if let Some((sid, Some(caminho))) = troca { atualizar_transcript(state, &sessao, terminal, &sid, &caminho); }
}

fn confirmar(state: &EngineState, chave: &str) {
    if chave.is_empty() { return; }
    let lista = {
        let Ok(mut confirmados) = RUNTIME.confirmados.lock() else { return };
        if !confirmados.insert(chave.to_owned()) { return; }
        let mut lista: Vec<String> = confirmados.iter().cloned().collect();
        lista.sort();
        lista
    };
    let _ = atomic_write_json(&dir_de(state).join(ARQUIVO_DE_CONFIRMADOS), &lista);
}

/// Evento com outra sessão da CLI (`/clear`, `/branch`): o segmento da conversa passa a apontar para
/// ela, e a timeline lê o transcript certo.
fn atualizar_transcript(state: &EngineState, sessao: &LiveSession, terminal: &str, sid: &str, caminho: &str) {
    if Path::new(caminho).file_stem().and_then(|s| s.to_str()) != Some(sid) { return; }
    // Fork (fix-orquestracao, achado 5): a identidade da sessão também troca — reiniciar e bifurcar
    // usavam o id anterior e voltavam ao contexto de antes do `/clear`. O comando de reinício só sabe
    // virar `--resume <id>` quando a linha tem `--session-id <id>` do id da sessão (`comando_ao_reiniciar`):
    // mudar só o id deixava o reinício repetir `--session-id <antigo>`, que a CLI recusa. Por isso a linha
    // troca junto (`comando_com_sessao`). Só quando a sessão já tinha id e ele mudou (a primeira notícia
    // de uma sessão aberta sem id não muda o reinício).
    let (conversa, trocou) = {
        let Ok(mut meta) = sessao.meta.lock() else { return };
        let trocou = meta.provider.as_deref() == Some("claude") && meta.external_session_id.as_deref().is_some_and(|antes| antes != sid);
        if trocou {
            meta.external_session_id = Some(sid.to_owned());
            meta.initial_command = meta.initial_command.as_deref().map(|comando| regra::comando_com_sessao(comando, sid));
        }
        (meta.conversation_id.clone(), trocou)
    };
    if trocou {
        let _ = state.persist();
        registro::info("gancho_sessao_trocada", &[("terminal", terminal)]);
    }
    let Some(conversa) = conversa else { return };
    // Fork (fix-orquestracao, achado 4): ler-alterar-gravar sob a trava do índice.
    let mudou = omni_core::indice_de_conversas::mudar_indice(&dir_de(state), |indice| {
        let Some(segmento) = indice.iter_mut().find(|c| c.id == conversa).and_then(|c| c.segments.last_mut()) else { return false };
        if segmento.provider != "claude" || segmento.terminal_session_id.as_deref().is_some_and(|t| t != terminal) { return false; }
        if segmento.external_session_id.as_deref() == Some(sid) && segmento.transcript_path.as_deref() == Some(caminho) { return false; }
        segmento.external_session_id = Some(sid.to_owned());
        segmento.transcript_path = Some(caminho.to_owned());
        true
    });
    if mudou.unwrap_or(false) {
        registro::info("gancho_transcript_trocado", &[("terminal", terminal), ("conversa", &conversa)]);
    }
}

/// Some quando o handler termina por qualquer caminho (resposta, prazo, conexão caída).
struct Guarda { terminal: String, id: String }
impl Drop for Guarda {
    fn drop(&mut self) {
        com_estado(&self.terminal, |e| e.pedidos.retain(|p| p.id != self.id));
    }
}

fn resposta(decisao: &Decisao) -> Response {
    let decisao = match decisao {
        Decisao::Permitir => json!({"behavior": "allow"}),
        Decisao::Negar(texto) => json!({"behavior": "deny", "message": match texto.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            Some(texto) => format!("Negado no Niski: {texto}"),
            None => "Negado no Niski".to_owned(),
        }}),
        Decisao::Soltar => return StatusCode::OK.into_response(),
    };
    Json(json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": decisao}})).into_response()
}

/// `PermissionRequest`: segura a conexão até alguém decidir. No prazo − 5 s responde vazio e a CLI
/// mostra o diálogo dela (o caminho de hoje). Nunca `updatedPermissions` nem `updatedInput`.
async fn permissao(State(state): State<Arc<EngineState>>, headers: HeaderMap, corpo: Result<Bytes, BytesRejection>) -> Response {
    let terminal = match autorizar(&state, &headers) { Ok(t) => t, Err(r) => return r };
    let valor = match corpo_valido(&terminal, corpo) { Ok(v) => v, Err(r) => return r };
    let Some(evento) = regra::Evento::de(&valor, 2_000) else { return StatusCode::BAD_REQUEST.into_response() };
    let ferramenta = evento.ferramenta.clone().unwrap_or_default();
    let ajustes = regra::ler_ajustes(&dir_de(&state));
    { let (s, t, e) = (state.clone(), terminal.clone(), evento.clone()); let _ = tokio::task::spawn_blocking(move || aplicar(&s, &t, &e)).await; }
    // Pergunta e plano: o cartão da tela continua respondendo (resposta estruturada: A CONFIRMAR).
    if !ajustes.ligado || regra::FERRAMENTAS_DA_TELA.contains(&ferramenta.as_str()) { return StatusCode::OK.into_response(); }
    let prazo_ms = prazo_ms(&state, &ajustes);
    let texto = evento.resumo.clone().unwrap_or_else(|| ferramenta.clone());
    let (remetente, receptor) = oneshot::channel();
    let id = uuid::Uuid::new_v4().to_string();
    // Fork (fix-orquestracao, achado 1): o risco sai da entrada inteira; o corte em 2.000 (com "…") é só
    // para a tela. Antes um `rm -rf` depois do corte passava como pedido comum.
    let completo = regra::resumo_da_ferramenta(&ferramenta, valor.get("tool_input").unwrap_or(&Value::Null), usize::MAX);
    let novo = Pedido { id: id.clone(), ferramenta: ferramenta.clone(), arriscado: omni_core::orquestracao::briefing::pedido_arriscado(&completo),
        texto, servidor_mcp: evento.servidor_mcp.clone(), expira_em_ms: now_ms() + prazo_ms, resposta: Some(remetente) };
    if com_estado(&terminal, |e| e.pedidos.push_back(novo)).is_none() { return StatusCode::OK.into_response(); }
    let _guarda = Guarda { terminal: terminal.clone(), id: id.clone() };
    { let (s, t) = (state.clone(), terminal.clone()); let _ = tokio::task::spawn_blocking(move || marcar_pedido(&s, &t)).await; }
    registro::info("gancho_pedido", &[("terminal", &terminal), ("pedido", &id)]);
    let decisao = match tokio::time::timeout(std::time::Duration::from_millis(prazo_ms), receptor).await {
        Ok(Ok(decisao)) => decisao,
        Ok(Err(_)) => Decisao::Soltar,
        Err(_) => { registro::info("gancho_pedido_prazo", &[("terminal", &terminal), ("pedido", &id)]); Decisao::Soltar }
    };
    let nome = match &decisao { Decisao::Permitir => "permitido", Decisao::Negar(_) => "negado", Decisao::Soltar => "solto" };
    registro::info("gancho_pedido_respondido", &[("terminal", &terminal), ("pedido", &id), ("resposta", nome)]);
    drop(_guarda);
    { let (s, t) = (state.clone(), terminal.clone()); let _ = tokio::task::spawn_blocking(move || pedido_saiu(&s, &t)).await; }
    resposta(&decisao)
}

fn prazo_ms(state: &EngineState, ajustes: &regra::Ajustes) -> u64 {
    #[cfg(test)]
    if let Some(ms) = PRAZOS_DE_TESTE.lock().ok().and_then(|p| p.get(&dir_de(state)).copied()) { return ms; }
    let _ = state;
    ajustes.espera().saturating_sub(regra::FOLGA_ANTES_DO_PRAZO_S) * 1_000
}

/// Testes não esperam 2 minutos: prazo curto por pasta de dados.
#[cfg(test)]
static PRAZOS_DE_TESTE: LazyLock<Mutex<HashMap<PathBuf, u64>>> = LazyLock::new(Default::default);

/// Pedido novo: a sessão vira "aguardando aprovação" e conta um aviso (uma vez; a tela, se mostrar o
/// mesmo diálogo depois, não conta de novo).
fn marcar_pedido(state: &EngineState, terminal: &str) {
    let Some(sessao) = viva(state, terminal) else { return };
    let mut interaction = sessao.interaction.lock().expect("screen poisoned");
    let mut meta = sessao.meta.lock().expect("metadata poisoned");
    interaction.turn_active = true;
    if meta.state != SessionState::ApprovalRequired || !interaction.aprovacao_contada {
        interaction.aprovacao_contada = true;
        meta.attention_seq += 1;
    }
    meta.state = SessionState::ApprovalRequired;
    meta.last_activity_at_ms = now_ms();
}

/// Respondido, solto ou vencido: o agente segue (ou a tela mostra o diálogo e o estado vem dela).
fn pedido_saiu(state: &EngineState, terminal: &str) {
    if tem_pedido(terminal) { return; }
    let Some(sessao) = viva(state, terminal) else { return };
    let mut meta = sessao.meta.lock().expect("metadata poisoned");
    if meta.state == SessionState::ApprovalRequired { meta.state = SessionState::Working; meta.last_activity_at_ms = now_ms(); }
}

/// O que a statusline viu (o `omni-shim` repassa antes de rodar a do usuário). Só números.
async fn statusline(State(state): State<Arc<EngineState>>, headers: HeaderMap, corpo: Result<Bytes, BytesRejection>) -> Response {
    let terminal = match autorizar(&state, &headers) { Ok(t) => t, Err(r) => return r };
    let valor = match corpo_valido(&terminal, corpo) { Ok(v) => v, Err(r) => return r };
    let medicao = regra::medicao(&valor, now_ms());
    let Some(perfil) = com_estado(&terminal, |e| { e.medicao = Some(medicao.clone()); e.perfil.clone() }) else { return StatusCode::OK.into_response() };
    if let (Some(perfil), Some(uso)) = (perfil.as_deref(), perfil.as_deref().and_then(|p| medicao.uso(p))) {
        let (dir, perfil) = (dir_de(&state), perfil.to_owned());
        let _ = tokio::task::spawn_blocking(move || {
            let _trava = RUNTIME.trava_do_uso.lock();
            let caminho = dir.join(ARQUIVO_DE_USO);
            let mut mapa: HashMap<String, omni_core::usage::AccountUsage> =
                fs::read(&caminho).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            mapa.insert(perfil, uso);
            let _ = atomic_write_json(&caminho, &mapa);
        }).await;
    }
    StatusCode::OK.into_response()
}

/// Token da PTY (só para testes de outros módulos, como o do orquestrador).
#[cfg(test)]
pub(crate) fn token_de_teste(terminal: &str) -> Option<String> {
    RUNTIME.tokens.lock().ok()?.iter().find(|(_, dono)| dono.as_str() == terminal).map(|(t, _)| t.clone())
}

#[cfg(test)]
#[path = "ganchos_testes.rs"]
mod testes;
