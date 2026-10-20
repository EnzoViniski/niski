//! Fork (3A): um app-server por agente/conversa. Sem OAuth no Medza; a CLI oficial faz o login.
//! Falha de transporte nunca repete um prompt de entrega incerta. Retoma na PTY o thread já criado.
use super::*;
use omni_core::codex_protocolo::{self as protocolo, Id};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, mpsc, LazyLock},
    time::{Duration, Instant},
};

const AJUSTES: &str = "codex-app-server.json";
static CLIENTES: LazyLock<Mutex<HashMap<String, Arc<Cliente>>>> = LazyLock::new(Default::default);
static FALLBACK: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

#[derive(Clone)]
struct Pendente {
    rpc: Id,
    visivel: ganchos::PedidoVisivel,
}
#[derive(Default, Clone)]
struct Estado {
    thread: String,
    caminho: Option<String>,
    turno: Option<String>,
    ultimo_fim: Option<String>,
    ocupado: bool,
    pedidos: VecDeque<Pendente>,
    revisao: u64,
    fins: u64,
    aviso: Option<String>,
    tarefas: Option<Vec<omni_core::plano_do_agente::Tarefa>>,
    tokens: Option<Value>,
    limites: Option<Value>,
    limites_em_ms: u64,
}

struct Cliente {
    geracao: String,
    writer: Mutex<Box<dyn Write + Send>>,
    websocket: bool,
    processos: Mutex<Vec<std::process::Child>>,
    respostas: Mutex<HashMap<Id, mpsc::Sender<Result<Value>>>>,
    proximo: AtomicU64,
    estado: Mutex<Estado>,
    morreu: AtomicBool,
    encerrado: AtomicBool,
    socket_dir: Option<PathBuf>,
    desconhecidos: Mutex<HashSet<String>>,
}

impl Cliente {
    fn enviar(&self, valor: &Value) -> Result<()> {
        let dados = serde_json::to_vec(valor)?;
        let mut writer = self.writer.lock().map_err(|_| anyhow!("writer poisoned"))?;
        if self.websocket {
            crate::codex_transporte::quadro(&mut **writer, 1, &dados)?;
        } else {
            writer.write_all(&dados)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
        Ok(())
    }
    fn pedir(&self, metodo: &str, params: Value) -> Result<Value> {
        anyhow::ensure!(
            !self.morreu.load(Ordering::SeqCst),
            "App-server desconectado"
        );
        let n = self.proximo.fetch_add(1, Ordering::SeqCst);
        let id = Id::Numero(n as i64);
        let (tx, rx) = mpsc::channel();
        self.respostas.lock().unwrap().insert(id.clone(), tx);
        let resultado = self
            .enviar(&protocolo::pedido(n, metodo, params))
            .and_then(|_| {
                rx.recv_timeout(Duration::from_secs(15))
                    .map_err(|_| anyhow!("App-server sem resposta"))?
            });
        self.respostas.lock().unwrap().remove(&id);
        if resultado.is_err() {
            self.morreu.store(true, Ordering::SeqCst);
        }
        resultado
    }
    fn fechar(&self) {
        self.encerrado.store(true, Ordering::SeqCst);
        for p in self.processos.lock().unwrap().iter_mut() {
            let _ = p.kill();
            let _ = p.wait();
        }
        if let Some(dir) = &self.socket_dir {
            let _ = fs::remove_file(dir.join("s.sock"));
            let _ = fs::remove_dir(dir);
        }
    }
    fn revision(&self, e: &Estado) -> String {
        format!("codex-{}-{}", self.geracao, e.revisao)
    }
    fn despachar(&self, m: protocolo::Mensagem) -> Result<()> {
        let Some(metodo) = m.method.as_deref() else {
            if let Some(tx) =
                m.id.and_then(|id| self.respostas.lock().unwrap().remove(&id))
            {
                let _ = tx.send(if m.error.is_some() {
                    Err(anyhow!("Método recusado pelo app-server"))
                } else {
                    Ok(m.result.unwrap_or(Value::Null))
                });
            }
            return Ok(());
        };
        let p = &m.params;
        let mut e = self.estado.lock().unwrap();
        if p.get("threadId")
            .and_then(Value::as_str)
            .is_some_and(|t| !e.thread.is_empty() && t != e.thread)
        {
            return Ok(());
        }
        if let Some(ferramenta) = protocolo::aprovacao(metodo) {
            let rpc = m.id.ok_or_else(|| anyhow!("Aprovação sem id"))?;
            if !e.pedidos.iter().any(|p| p.rpc == rpc) {
                let resumo = p["command"]
                    .as_str()
                    .or_else(|| p["reason"].as_str())
                    .unwrap_or("Alterar arquivos")
                    .chars()
                    .take(500)
                    .collect::<String>();
                let id = format!("codex-{}-{}", self.geracao, uuid::Uuid::new_v4());
                e.pedidos.push_back(Pendente {
                    rpc,
                    visivel: ganchos::PedidoVisivel {
                        id,
                        origem: "codex_app_server",
                        ferramenta: ferramenta.into(),
                        arriscado: omni_core::orquestracao::briefing::pedido_arriscado(&resumo),
                        resumo,
                        servidor_mcp: None,
                        expira_em_ms: now_ms() + 600_000,
                    },
                });
                e.revisao += 1;
            }
            return Ok(());
        }
        if let Some(id) = m.id {
            // Pedidos novos (elicitation/permissões granulares) não recebem um aceite genérico.
            drop(e);
            self.desconhecido(metodo);
            self.enviar(&json!({"id":id,"error":{"code":-32601,"message":"Use o terminal para este tipo de pedido"}}))?;
            return Ok(());
        }
        match metodo {
            "turn/started" => {
                e.turno = p
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                e.ocupado = true;
                e.aviso = None;
                e.revisao += 1;
            }
            "turn/completed" => {
                let id = p.pointer("/turn/id").and_then(Value::as_str);
                if id.is_some() && e.ultimo_fim.as_deref() != id {
                    e.ultimo_fim = id.map(str::to_owned);
                    e.turno = None;
                    e.ocupado = false;
                    e.fins += 1;
                    e.pedidos.clear();
                    e.revisao += 1;
                    if p.pointer("/turn/status").and_then(Value::as_str) == Some("failed") {
                        e.aviso = Some("api_error".into());
                    }
                }
            }
            "thread/status/changed" => {
                e.ocupado = p.pointer("/status/type").and_then(Value::as_str) == Some("active");
                e.revisao += 1;
            }
            "serverRequest/resolved" => {
                if let Ok(id) = serde_json::from_value::<Id>(p["requestId"].clone()) {
                    e.pedidos.retain(|p| p.rpc != id);
                    e.revisao += 1;
                }
            }
            "turn/plan/updated" => {
                let payload = json!({"type":"function_call","name":"update_plan","arguments":json!({"plan":p["plan"]}).to_string()});
                e.tarefas = omni_core::plano_do_agente::extras_codex(&payload).tarefas;
            }
            "thread/tokenUsage/updated" => e.tokens = Some(p["tokenUsage"].clone()),
            "account/rateLimits/updated" => {
                e.limites = Some(p["rateLimits"].clone());
                e.limites_em_ms = now_ms();
            }
            _ => {
                self.desconhecido(metodo);
            }
        }
        Ok(())
    }
    fn desconhecido(&self, metodo: &str) {
        if metodo.len() <= 96
            && metodo
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
        {
            let mut nomes = self.desconhecidos.lock().unwrap();
            if nomes.len() < 128 && nomes.insert(metodo.into()) {
                registro::info("codex_app_server_evento", &[("metodo", metodo)]);
            }
        }
    }
}

impl Drop for Cliente {
    fn drop(&mut self) {
        self.fechar();
    }
}

fn leitor(cliente: &Arc<Cliente>, mut reader: Box<dyn Read + Send>) {
    let fraco = Arc::downgrade(cliente);
    std::thread::spawn(move || {
        loop {
            let Some(c) = fraco.upgrade() else { break };
            let recebido = if c.websocket {
                crate::codex_transporte::receber(&mut *reader, &c.writer)
            } else {
                // Um byte por vez evita perder o buffer entre mensagens (só fixtures usam stdio).
                let mut dados = Vec::new();
                loop {
                    let mut b = [0];
                    if reader.read_exact(&mut b).is_err() {
                        break;
                    }
                    dados.push(b[0]);
                    if b[0] == b'\n' || dados.len() > protocolo::MENSAGEM_MAX {
                        break;
                    }
                }
                if dados.is_empty() {
                    Err(anyhow!("App-server encerrou"))
                } else {
                    Ok(dados)
                }
            };
            let resultado = recebido
                .and_then(|v| protocolo::ler(&v).map_err(|e| anyhow!(e)))
                .and_then(|m| c.despachar(m));
            if resultado.is_err() {
                c.morreu.store(true, Ordering::SeqCst);
                c.respostas.lock().unwrap().clear();
                break;
            }
        }
    });
}

fn cliente(
    estado: Estado,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    websocket: bool,
    processos: Vec<std::process::Child>,
    socket_dir: Option<PathBuf>,
) -> Arc<Cliente> {
    let c = Arc::new(Cliente {
        geracao: uuid::Uuid::new_v4().to_string(),
        writer: Mutex::new(writer),
        websocket,
        processos: Mutex::new(processos),
        respostas: Mutex::new(HashMap::new()),
        proximo: AtomicU64::new(1),
        estado: Mutex::new(estado),
        morreu: AtomicBool::new(false),
        encerrado: AtomicBool::new(false),
        socket_dir,
        desconhecidos: Mutex::new(HashSet::new()),
    });
    leitor(&c, reader);
    c
}

pub(crate) fn habilitados(dir: &Path) -> HashSet<String> {
    fs::read(dir.join(AJUSTES))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
pub(crate) fn gravar(dir: &Path, ids: &HashSet<String>) -> Result<()> {
    Ok(atomic_write_json(&dir.join(AJUSTES), ids)?)
}
fn obter(id: &str) -> Option<Arc<Cliente>> {
    CLIENTES
        .lock()
        .ok()?
        .get(id)
        .cloned()
        .filter(|c| !c.encerrado.load(Ordering::SeqCst))
}
pub(crate) fn ativo(id: &str) -> bool {
    obter(id).is_some()
}
pub(crate) fn esquecer(id: &str) {
    let c = CLIENTES.lock().unwrap().remove(id);
    if let Some(c) = c {
        c.fechar();
    }
}

/// Só comandos oficiais simples. Flags e `-c` (MCP do orquestrador inclusive) são argumentos, nunca shell.
/// Comandos personalizados complexos continuam no caminho PTY existente.
fn argumentos(comando: &str) -> Option<(String, Vec<String>, Option<(String, String)>)> {
    let partes = separar(comando)?;
    let bin = partes.first()?.clone();
    if Path::new(&bin).file_name()?.to_str()? != "codex" {
        return None;
    }
    let mut flags = Vec::new();
    let mut thread = None;
    let mut i = 1;
    while i < partes.len() {
        let p = &partes[i];
        match p.as_str() {
            "resume" | "fork" => {
                let id = partes.get(i + 1)?.clone();
                if !id_valido(&id) {
                    return None;
                }
                thread = Some((p.clone(), id));
                i += 2;
            }
            "-c" | "--config" | "-m" | "--model" | "-p" | "--profile" | "-a"
            | "--ask-for-approval" | "-s" | "--sandbox" | "--enable" | "--disable" => {
                let v = partes.get(i + 1)?.clone();
                let nome = match p.as_str() {
                    "-m" | "--model" => Some("model"),
                    "-a" | "--ask-for-approval" => Some("approval_policy"),
                    "-s" | "--sandbox" => Some("sandbox_mode"),
                    _ => None,
                };
                if let Some(nome) = nome {
                    flags.extend([
                        "-c".into(),
                        format!("{nome}={}", serde_json::to_string(&v).ok()?),
                    ]);
                } else {
                    flags.extend([p.clone(), v]);
                }
                i += 2;
            }
            "--no-alt-screen" | "--no-daemon" => i += 1,
            _ => return None,
        }
    }
    Some((bin, flags, thread))
}
fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn separar(s: &str) -> Option<Vec<String>> {
    let mut partes = Vec::new();
    let mut atual = String::new();
    let mut aspas = None;
    for c in s.chars() {
        if matches!(
            c,
            '\n' | '\r' | '`' | '$' | '\\' | ';' | '|' | '&' | '<' | '>'
        ) {
            return None;
        }
        match (aspas, c) {
            (Some(a), c) if a == c => aspas = None,
            (None, '\'' | '"') => aspas = Some(c),
            (None, c) if c.is_whitespace() => {
                if !atual.is_empty() {
                    partes.push(std::mem::take(&mut atual));
                }
            }
            _ => atual.push(c),
        }
    }
    if aspas.is_some() {
        return None;
    }
    if !atual.is_empty() {
        partes.push(atual);
    }
    Some(partes)
}
fn citar(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// A PTY retoma o id conhecido mesmo se as flags originais tinham aspas (MCP/esforço).
pub(crate) fn comando_de_retomada(original: &str, id: &str) -> Option<String> {
    if !id_valido(id) {
        return None;
    }
    let (bin, flags, _) = argumentos(original)?;
    let simples = |s: String| {
        if s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./:=+-".contains(&b))
        {
            s
        } else {
            citar(&s)
        }
    };
    Some(
        [vec![bin, "resume".into(), id.into()], flags]
            .concat()
            .into_iter()
            .map(simples)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(unix)]
fn abrir(
    bin: &str,
    flags: &[String],
    cwd: &str,
    env: &[(String, String)],
) -> Result<(Arc<Cliente>, String)> {
    use std::os::unix::fs::DirBuilderExt;
    // SUN_LEN no macOS é 104: o diretório de dados completo pode ultrapassar esse limite.
    let dir = PathBuf::from("/tmp").join(format!("medza-cdx-{}", uuid::Uuid::new_v4().simple()));
    fs::DirBuilder::new().mode(0o700).create(&dir)?;
    let socket = dir.join("s.sock");
    let endpoint = format!("unix://{}", socket.display());
    let mut processo = match Command::new(bin)
        .args(["app-server", "--listen", &endpoint])
        .args(flags)
        .current_dir(cwd)
        .envs(env.iter().cloned())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(p) => p,
        Err(e) => {
            let _ = fs::remove_dir(&dir);
            return Err(e.into());
        }
    };
    let limite = Instant::now() + Duration::from_secs(10);
    let conectado = loop {
        if socket.exists() {
            break crate::codex_transporte::conectar(&socket);
        }
        if processo.try_wait()?.is_some() || Instant::now() >= limite {
            break Err(anyhow!("App-server não abriu socket"));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    match conectado {
        Ok((r, w)) => Ok((
            cliente(Estado::default(), r, w, true, vec![processo], Some(dir)),
            endpoint,
        )),
        Err(e) => {
            let _ = processo.kill();
            let _ = processo.wait();
            let _ = fs::remove_file(socket);
            let _ = fs::remove_dir(dir);
            Err(e)
        }
    }
}

fn iniciar_thread(c: &Cliente, cwd: &str, alvo: Option<(String, String)>) -> Result<()> {
    let inicial = protocolo::inicializar(0, env!("CARGO_PKG_VERSION"));
    c.pedir("initialize", inicial["params"].clone())?;
    c.enviar(&json!({"method":"initialized"}))?;
    let (metodo, params) = match alvo {
        Some((operacao, id)) => (
            format!("thread/{operacao}"),
            json!({"threadId":id,"cwd":cwd}),
        ),
        None => ("thread/start".into(), json!({"cwd":cwd})),
    };
    let resposta = c.pedir(&metodo, params)?;
    let em_andamento = resposta
        .pointer("/thread/turns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|t| t["status"] == "inProgress")
        .and_then(|t| t["id"].as_str())
        .map(str::to_owned);
    let resposta: protocolo::ThreadResposta = serde_json::from_value(resposta)?;
    anyhow::ensure!(id_valido(&resposta.thread.id), "Id de thread inválido");
    let mut e = c.estado.lock().unwrap();
    e.thread = resposta.thread.id;
    e.caminho = resposta.thread.path;
    e.turno = em_andamento;
    e.revisao += 1;
    Ok(())
}

pub(crate) fn preparar(
    state: &Arc<EngineState>,
    terminal: &str,
    origem: &SessionOrigin,
    cwd: &str,
    comando: Option<String>,
    env_extra: &[(String, String)],
) -> Option<String> {
    #[cfg(unix)]
    {
        preparar_com(state, terminal, origem, cwd, comando, env_extra, abrir)
    }
    #[cfg(not(unix))]
    {
        preparar_com(
            state,
            terminal,
            origem,
            cwd,
            comando,
            env_extra,
            |_, _, _, _| Err(anyhow!("Plataforma sem socket Unix")),
        )
    }
}

fn preparar_com(
    state: &Arc<EngineState>,
    terminal: &str,
    origem: &SessionOrigin,
    cwd: &str,
    comando: Option<String>,
    env_extra: &[(String, String)],
    abrir_servidor: impl FnOnce(
        &str,
        &[String],
        &str,
        &[(String, String)],
    ) -> Result<(Arc<Cliente>, String)>,
) -> Option<String> {
    esquecer(terminal);
    let Some(original) = comando.as_ref() else {
        return comando;
    };
    if FALLBACK.lock().unwrap().remove(terminal)
        || origem.provider.as_deref() != Some("codex")
        || !origem
            .conversation_id
            .as_ref()
            .is_some_and(|id| habilitados(state.state_file.parent().unwrap()).contains(id))
    {
        return comando;
    }
    let Some((bin, flags, mut alvo)) = argumentos(original) else {
        falhou(terminal, "comando");
        return comando;
    };
    if alvo.is_none() {
        alvo = origem
            .external_session_id
            .clone()
            .map(|id| ("resume".into(), id));
    }
    {
        let ambiente = [origem.env.clone(), env_extra.to_vec()].concat();
        match abrir_servidor(&bin, &flags, cwd, &ambiente).and_then(|(c, endpoint)| {
            if let Err(e) = iniciar_thread(&c, cwd, alvo) {
                c.fechar();
                return Err(e);
            }
            Ok((c, endpoint))
        }) {
            Ok((c, endpoint)) => {
                let thread = c.estado.lock().unwrap().thread.clone();
                CLIENTES.lock().unwrap().insert(terminal.into(), c.clone());
                acompanhar(Arc::downgrade(state), terminal.into(), c);
                return Some(format!(
                    "{} --remote {} resume {}",
                    citar(&bin),
                    citar(&endpoint),
                    citar(&thread)
                ));
            }
            Err(_) => falhou(terminal, "inicio"),
        }
    }
    comando
}

fn falhou(terminal: &str, fase: &str) {
    registro::info(
        "codex_app_server_falhou",
        &[("terminal", terminal), ("fase", fase)],
    );
}

fn acompanhar(state: std::sync::Weak<EngineState>, terminal: String, c: Arc<Cliente>) {
    std::thread::spawn(move || {
        let inicio = Instant::now();
        let mut fins = 0;
        let mut viu_sessao = false;
        let mut ultimo_vinculo = Instant::now() - Duration::from_secs(1);
        loop {
            std::thread::sleep(Duration::from_millis(100));
            if c.encerrado.load(Ordering::SeqCst) {
                break;
            }
            let Some(state) = state.upgrade() else {
                c.fechar();
                break;
            };
            let sessao = state
                .sessions
                .lock()
                .ok()
                .and_then(|s| match s.get(&terminal) {
                    Some(SessionEntry::Live(s)) => Some(s.clone()),
                    _ => None,
                });
            let Some(sessao) = sessao else {
                if viu_sessao || inicio.elapsed() > Duration::from_secs(15) {
                    c.fechar();
                    break;
                }
                continue;
            };
            viu_sessao = true;
            let processos_vivos = c
                .processos
                .lock()
                .unwrap()
                .iter_mut()
                .all(|p| p.try_wait().ok().flatten().is_none());
            let anexou = inicio.elapsed() < Duration::from_secs(20) || {
                let m = sessao.meta.lock().unwrap();
                interaction::provider_running_recente(m.pid, "codex")
            };
            if c.morreu.load(Ordering::SeqCst) || !processos_vivos || !anexou {
                falhou(
                    &terminal,
                    if !anexou { "anexo_terminal" } else { "conexao" },
                );
                // A revisão do RPC nunca é válida na PTY. Reiniciar abre só resume, sem reenviar texto.
                FALLBACK.lock().unwrap().insert(terminal.clone());
                let e = c.estado.lock().unwrap().clone();
                sessao.meta.lock().unwrap().external_session_id = Some(e.thread.clone());
                vincular(&state, &terminal, &e);
                esquecer(&terminal);
                let _ = restart_session(&state, &terminal);
                break;
            }
            let e = c.estado.lock().unwrap().clone();
            let mudou_id = {
                let mut contexto = sessao.interaction.lock().unwrap();
                let mut meta = sessao.meta.lock().unwrap();
                if e.fins > fins {
                    meta.attention_seq += e.fins - fins;
                    fins = e.fins;
                }
                if !e.pedidos.is_empty() && meta.state != SessionState::ApprovalRequired {
                    meta.attention_seq += 1;
                }
                contexto.turn_active = e.turno.is_some();
                meta.state = estado_da_sessao(&e);
                meta.notice = e.aviso.clone();
                let mudou = meta.external_session_id.as_deref() != Some(&e.thread);
                meta.external_session_id = Some(e.thread.clone());
                mudou
            };
            if mudou_id {
                let _ = state.persist();
            }
            if ultimo_vinculo.elapsed() >= Duration::from_secs(1) {
                vincular(&state, &terminal, &e);
                ultimo_vinculo = Instant::now();
            }
            // Expiração segura: recusa o pedido, nunca permite por tempo.
            let vencido = e
                .pedidos
                .front()
                .filter(|p| p.visivel.expira_em_ms <= now_ms());
            if let Some(p) = vencido {
                let _ = responder(&terminal, &p.visivel.id, false);
            }
        }
    });
}

fn vincular(state: &EngineState, terminal: &str, e: &Estado) {
    if e.thread.is_empty() {
        return;
    }
    let dir = state.state_file.parent().unwrap();
    let mut indice = omni_core::conversations::read_index(dir);
    let mut mudou = false;
    for s in indice
        .iter_mut()
        .flat_map(|c| &mut c.segments)
        .filter(|s| s.provider == "codex" && s.terminal_session_id.as_deref() == Some(terminal))
    {
        if s.external_session_id.as_deref() != Some(&e.thread)
            || (e.caminho.is_some() && s.transcript_path != e.caminho)
        {
            s.external_session_id = Some(e.thread.clone());
            if e.caminho.is_some() {
                s.transcript_path = e.caminho.clone();
            }
            mudou = true;
        }
    }
    if mudou {
        let _ = omni_core::conversations::write_index(dir, &indice);
    }
}

fn estado_da_sessao(e: &Estado) -> SessionState {
    if !e.pedidos.is_empty() {
        SessionState::ApprovalRequired
    } else if e.turno.is_some() || e.ocupado {
        SessionState::Working
    } else {
        SessionState::Answered
    }
}
pub(crate) fn metadata(sessao: &LiveSession) -> Option<TerminalSession> {
    let mut meta = sessao.meta.lock().ok()?.clone();
    let c = obter(&meta.id)?;
    let e = c.estado.lock().ok()?;
    meta.state = estado_da_sessao(&e);
    meta.notice = e.aviso.clone();
    Some(meta)
}
pub(crate) fn capabilities(sessao: &Arc<LiveSession>) -> Option<interaction::Capabilities> {
    let id = sessao.meta.lock().ok()?.id.clone();
    let c = obter(&id)?;
    let e = c.estado.lock().ok()?.clone();
    let livre = !sessao.reserved.load(Ordering::SeqCst) && !c.morreu.load(Ordering::SeqCst);
    let pedido = e.pedidos.front().map(|p| p.visivel.clone());
    Some(interaction::Capabilities {
        prompt: livre && pedido.is_none() && e.turno.is_none() && !e.ocupado,
        approve: livre && pedido.is_some(),
        revision: c.revision(&e),
        approval_text: pedido.as_ref().map(|p| p.resumo.clone()),
        reason: None,
        enfileirar: livre && pedido.is_none() && e.turno.is_some(),
        escolha: None,
        pedido,
    })
}

pub(crate) fn enviar_prompt(terminal: &str, texto: &str, revisao: &str, fila: bool) -> Result<()> {
    let c = obter(terminal).ok_or_else(|| anyhow!("App-server encerrado"))?;
    let e = c.estado.lock().unwrap().clone();
    anyhow::ensure!(
        (fila || revisao == c.revision(&e)) && e.pedidos.is_empty(),
        "Contexto mudou antes da execução"
    );
    let (metodo, params) = if let Some(turno) = e.turno {
        (
            "turn/steer",
            json!({"threadId":e.thread,"expectedTurnId":turno,"input":protocolo::entrada(texto)}),
        )
    } else {
        (
            "turn/start",
            json!({"threadId":e.thread,"input":protocolo::entrada(texto)}),
        )
    };
    let resposta = c.pedir(metodo, params)?;
    if metodo == "turn/start" {
        let mut e = c.estado.lock().unwrap();
        let turno = resposta
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if e.turno.is_none() && e.ultimo_fim != turno {
            e.turno = turno;
            e.revisao += 1;
        }
    }
    Ok(())
}
pub(crate) fn responder(terminal: &str, pedido: &str, permitir: bool) -> Result<()> {
    let c = obter(terminal).ok_or_else(|| anyhow!("App-server encerrado"))?;
    let mut e = c.estado.lock().unwrap();
    let p = e
        .pedidos
        .front()
        .ok_or_else(|| anyhow!("Pedido já respondido"))?;
    anyhow::ensure!(
        p.visivel.id == pedido,
        "O pedido mudou; confira antes de responder"
    );
    c.enviar(&protocolo::decisao(&p.rpc, permitir))?;
    e.pedidos.pop_front();
    e.revisao += 1;
    Ok(())
}
pub(crate) fn interromper(terminal: &str) -> Option<Result<()>> {
    let c = obter(terminal)?;
    let e = c.estado.lock().unwrap().clone();
    Some(if let Some(turno) = e.turno {
        c.pedir(
            "turn/interrupt",
            json!({"threadId":e.thread,"turnId":turno}),
        )
        .map(|_| ())
    } else {
        Ok(())
    })
}

/// Timeline textual continua no rollout oficial. Plano e contexto chegam antes da gravação dele.
pub(crate) fn complementar(terminal: &str, timeline: &mut omni_core::conversations::Timeline) {
    if let Some(c) = obter(terminal) {
        if let Ok(e) = c.estado.lock() {
            if e.tarefas.is_some() {
                timeline.tarefas = e.tarefas.clone();
            }
        }
    }
}

pub(crate) fn resumo(terminal: &str) -> Option<Value> {
    let c = obter(terminal)?;
    let e = c.estado.lock().ok()?;
    let tokens = e.tokens.as_ref();
    let pct = tokens
        .and_then(|t| {
            Some(
                t.pointer("/last/totalTokens")?.as_f64()? * 100.0
                    / t["modelContextWindow"].as_f64()?,
            )
        })
        .filter(|n| n.is_finite() && *n >= 0.0)
        .map(|n| n.min(100.0));
    Some(json!({"ativo":true,"contexto_pct":pct,"token_usage":tokens}))
}

pub(crate) fn uso_da_conta(
    state: &EngineState,
    perfil: &str,
) -> Option<omni_core::usage::AccountUsage> {
    use omni_core::usage::{AccountUsage, UsageWindow};
    let ids: Vec<String> = state
        .sessions
        .lock()
        .ok()?
        .values()
        .filter_map(|s| match s {
            SessionEntry::Live(s) => s
                .meta
                .lock()
                .ok()
                .filter(|m| m.profile_id.as_deref() == Some(perfil))
                .map(|m| m.id.clone()),
            _ => None,
        })
        .collect();
    let e = ids
        .iter()
        .filter_map(|id| obter(id))
        .filter_map(|c| c.estado.lock().ok().map(|e| e.clone()))
        .filter(|e| e.limites.is_some())
        .max_by_key(|e| e.limites_em_ms)?;
    let limite = e.limites?;
    let janela = |nome: &str| -> Option<UsageWindow> {
        let v = &limite[nome];
        let usado = v["usedPercent"].as_f64()?;
        if !usado.is_finite() || !(0.0..=100.0).contains(&usado) {
            return None;
        }
        Some(UsageWindow {
            used_percent: usado,
            window_minutes: v["windowDurationMins"].as_u64()?,
            resets_at: v["resetsAt"].as_i64(),
            reset_label: None,
        })
    };
    let primary = janela("primary");
    let secondary = janela("secondary");
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    let stale = [&primary, &secondary]
        .into_iter()
        .flatten()
        .any(|j| j.resets_at.is_some_and(|t| t <= now_ms() as i64 / 1000));
    Some(AccountUsage {
        profile_id: perfil.into(),
        provider: "codex".into(),
        source: "codex_app_server".into(),
        status: if stale { "stale" } else { "available" }.into(),
        observed_at_ms: Some(e.limites_em_ms),
        primary,
        secondary,
        reason: None,
    })
}

#[cfg(test)]
#[path = "codex_app_server_testes.rs"]
mod testes;
