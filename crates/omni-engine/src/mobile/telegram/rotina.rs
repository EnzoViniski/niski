//! Fork (Rotina): o bot **da rotina** — separado do bot dos avisos (que só manda metadados).
//!
//! Com ele conectado, os lembretes da rotina vão **com o texto** (título da tarefa, refeição, meta) e
//! botões: Feito, Adiar 15 min, Pular (na água, +1 copo). Opcional (desligado por padrão): texto
//! mandado ao bot vai para o agente da rotina e a resposta volta pelo Telegram.
//!
//! Token só no engine: arquivo próprio `rotina/telegram-rotina.json` modo 0600, nunca em resposta,
//! log nem na tela. Cadastro só pelo IPC do desktop. Pareamento por desafio como o bot dos avisos
//! (`/start <código>` num chat privado, código novo que expira em 10 min). Só o chat pareado manda.
use super::*;
use omni_core::rotina::{self as rotina_core, ajustes::Ajustes as AjustesDaRotina, lembretes, operacoes};

const ARQUIVO: &str = "telegram-rotina.json";

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::mobile) struct Config {
    token: String,
    bot: String,
    chat: Option<i64>,
    ativo: bool,
}

#[derive(Default)]
struct Dados {
    config: Config,
    pareando: Option<Pareando>,
    erro: Option<String>,
}

pub(in crate::mobile) struct Runtime {
    dados: Mutex<Dados>,
    operacao: tokio::sync::Mutex<()>,
    geracao: AtomicU64,
}

fn caminho(dir: &Path) -> std::path::PathBuf {
    rotina_core::pasta(dir).join(ARQUIVO)
}

impl Runtime {
    pub(in crate::mobile) fn abrir(dir: &Path) -> Self {
        Self {
            dados: Mutex::new(Dados { config: omni_protocol::read_json_or_default(&caminho(dir)), ..Default::default() }),
            operacao: tokio::sync::Mutex::new(()),
            geracao: AtomicU64::new(0),
        }
    }
    fn status(&self) -> Value {
        let d = self.dados.lock().expect("telegram da rotina poisoned");
        json!({"conectado": d.config.chat.is_some(), "ativo": d.config.ativo, "bot": d.config.bot, "erro": d.erro,
            "parear_url": d.pareando.as_ref().filter(|p| p.expira > now_ms()).map(|p| format!("https://t.me/{}?start={}", d.config.bot, p.codigo)),
            "expira_em_ms": d.pareando.as_ref().map(|p| p.expira)})
    }
}

fn salvar_config(dir: &Path, config: &Config) -> std::io::Result<()> {
    use std::io::Write;
    let pasta = rotina_core::pasta(dir);
    fs::create_dir_all(&pasta)?;
    let temporario = pasta.join(format!(".telegram-rotina-{}.tmp", uuid::Uuid::new_v4()));
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    let resultado = (|| {
        let mut arquivo = opcoes.open(&temporario)?;
        arquivo.write_all(&serde_json::to_vec(config)?)?;
        arquivo.sync_all()?;
        fs::rename(&temporario, caminho(dir))
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporario);
    }
    resultado
}

fn runtime(state: &EngineState) -> &Runtime {
    &state.mobile.rotina_telegram
}

/* ── Mensagens ─────────────────────────────────────────────────────────────────────────────── */

/// Código curto e estável da chave do lembrete (o `callback_data` do Telegram tem no máximo 64 bytes).
fn codigo(chave: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    chave.hash(&mut h);
    format!("{:012x}", h.finish() & 0xffff_ffff_ffff)
}

fn rotulo(botao: &str) -> &'static str {
    match botao {
        "feito" => "✅ Feito",
        "adiar" => "⏰ Adiar 15 min",
        "pular" => "Pular",
        "copo" => "💧 +1 copo",
        "planejar" => "Planejar agora",
        _ => "OK",
    }
}

fn icone(tipo: &str) -> &'static str {
    match tipo {
        "agua" => "💧",
        "refeicao" => "🍽",
        "treino" => "🏋️",
        "planejar" => "🗓",
        "tarefa" => "📌",
        _ => "⏰",
    }
}

pub(super) fn mensagem_do_lembrete(chat: i64, d: &lembretes::Disparo) -> Value {
    let texto = format!("{} {}\n{}", icone(&d.tipo), d.titulo.chars().take(200).collect::<String>(), d.texto.chars().take(600).collect::<String>());
    let botoes: Vec<Value> = d.botoes.iter().map(|b| json!({"text": rotulo(b), "callback_data": format!("r|{b}|{}", codigo(&d.chave))})).collect();
    json!({"chat_id": chat, "text": texto.trim(), "link_preview_options": {"is_disabled": true}, "reply_markup": {"inline_keyboard": [botoes]}})
}

/// Resumo do dia em texto curto (comando `/hoje`).
fn resumo_do_dia(dados: &Path) -> String {
    let dia = operacoes::executar(dados, "ler_dia", &json!({}), &super::super::rotina_rotas::agora()).unwrap_or_default();
    let mut linhas = vec![format!("Hoje ({})", dia["dia_da_semana"].as_str().unwrap_or(""))];
    for item in dia["itens"].as_array().into_iter().flatten().take(25) {
        let marca = match item["estado"].as_str() { Some("feito") => "✅", Some("pulado") => "➖", _ => "•" };
        linhas.push(format!("{marca} {} {}", item["inicio"].as_str().unwrap_or(""), item["titulo"].as_str().unwrap_or("")));
    }
    for item in dia["sem_hora"].as_array().into_iter().flatten().take(10) {
        linhas.push(format!("• {}", item["titulo"].as_str().unwrap_or("")));
    }
    let agua = &dia["agua"];
    linhas.push(format!("💧 {} de {} ml", agua["ml"].as_u64().unwrap_or(0), agua["meta_ml"].as_u64().unwrap_or(0)));
    linhas.join("\n")
}

/* ── Trabalhador ───────────────────────────────────────────────────────────────────────────── */

/// O que o trabalhador lembra entre um passo e outro (só em memória).
#[derive(Default)]
pub(super) struct Memoria {
    /// Avisos já mandados (ou que já existiam quando o bot foi ligado: não manda histórico).
    vistos: BTreeSet<String>,
    destino: Option<i64>,
    offset: i64,
    /// Mensagens mais velhas que isto (de antes de o engine subir) não viram prompt.
    desde_ms: u64,
}

async fn parear(state: &EngineState, cliente: &impl api::BotApi) -> Result<bool, api::Falha> {
    let (config, offset) = {
        let mut d = runtime(state).dados.lock().expect("telegram da rotina poisoned");
        let Some(p) = &d.pareando else { return Ok(false) };
        if p.expira <= now_ms() {
            d.pareando = None;
            d.erro = Some("O link expirou. Conecte o bot da rotina novamente.".into());
            return Ok(true);
        }
        (d.config.clone(), p.offset)
    };
    let updates = cliente.chamar(&config.token, "getUpdates", json!({"offset": offset, "timeout": 0, "allowed_updates": ["message"]})).await?;
    let mut d = runtime(state).dados.lock().expect("telegram da rotina poisoned");
    for update in updates.as_array().into_iter().flatten() {
        let Some(p) = &mut d.pareando else { break };
        if let Some(id) = update["update_id"].as_i64() {
            p.offset = p.offset.max(id.saturating_add(1));
        }
        if let Some(chat) = destinatario(update, p) {
            let mut pronta = d.config.clone();
            pronta.chat = Some(chat);
            pronta.ativo = true;
            salvar_config(state.state_file.parent().unwrap(), &pronta).map_err(|_| api::Falha::disco())?;
            d.config = pronta;
            d.pareando = None;
            d.erro = None;
        }
    }
    Ok(true)
}

/// Um passo: pareamento, lembretes novos e respostas do usuário (botões, comandos, texto).
pub(super) async fn passo(state: &Arc<EngineState>, cliente: &(impl api::BotApi + Clone + 'static), m: &mut Memoria) -> Result<(), api::Falha> {
    if parear(state, cliente).await? {
        return Ok(());
    }
    let config = runtime(state).dados.lock().expect("telegram da rotina poisoned").config.clone();
    let dados = state.state_file.parent().unwrap().to_path_buf();
    let destino = config.ativo.then_some(config.chat).flatten();
    if destino.is_none() {
        m.destino = None; // Desligado: nem lê a pasta.
        return Ok(());
    }
    let estado: lembretes::Estado = { let d = dados.clone(); tokio::task::spawn_blocking(move || rotina_core::ler(&d)).await.unwrap_or_default() };
    if destino != m.destino {
        // Ligou, trocou de chat ou o engine subiu agora: o que já existia não sai.
        m.vistos = estado.avisos.iter().map(|a| a.disparo.chave.clone()).collect();
        m.destino = destino;
        if m.desde_ms == 0 { m.desde_ms = now_ms(); }
    }
    let Some(chat) = destino else { return Ok(()) };
    let ajustes: AjustesDaRotina = { let d = dados.clone(); tokio::task::spawn_blocking(move || rotina_core::ler(&d)).await.unwrap_or_default() };

    for aviso in &estado.avisos {
        if m.vistos.contains(&aviso.disparo.chave) { continue; }
        if ajustes.telegram_lembretes && aviso.resposta.is_empty() {
            cliente.chamar(&config.token, "sendMessage", mensagem_do_lembrete(chat, &aviso.disparo)).await?;
            registro::info("rotina_telegram_lembrete", &[("tipo", &aviso.disparo.tipo)]);
        }
        m.vistos.insert(aviso.disparo.chave.clone());
    }
    let atuais: BTreeSet<&String> = estado.avisos.iter().map(|a| &a.disparo.chave).collect();
    m.vistos.retain(|c| atuais.contains(c));

    let updates = cliente.chamar(&config.token, "getUpdates",
        json!({"offset": m.offset, "timeout": 0, "allowed_updates": ["message", "callback_query"]})).await?;
    for update in updates.as_array().into_iter().flatten() {
        if let Some(id) = update["update_id"].as_i64() { m.offset = m.offset.max(id.saturating_add(1)); }
        if update.get("callback_query").is_some() {
            responder_botao(state, cliente, &config.token, chat, &update["callback_query"], &estado).await?;
        } else if update.get("message").is_some() {
            responder_mensagem(state, cliente, &config.token, chat, &update["message"], &ajustes, m.desde_ms).await?;
        }
    }
    Ok(())
}

async fn responder_botao(state: &Arc<EngineState>, cliente: &impl api::BotApi, token: &str, chat: i64, consulta: &Value, estado: &lembretes::Estado)
    -> Result<(), api::Falha> {
    let id = consulta["id"].as_str().unwrap_or_default().to_owned();
    // Só o dono do chat pareado aperta.
    if consulta["from"]["id"].as_i64() != Some(chat) || consulta["message"]["chat"]["id"].as_i64() != Some(chat) {
        cliente.chamar(token, "answerCallbackQuery", json!({"callback_query_id": id})).await?;
        return Ok(());
    }
    let dados_do_botao = consulta["data"].as_str().unwrap_or("");
    let mut partes = dados_do_botao.splitn(3, '|');
    let (Some("r"), Some(acao), Some(cod)) = (partes.next(), partes.next(), partes.next()) else {
        cliente.chamar(token, "answerCallbackQuery", json!({"callback_query_id": id})).await?;
        return Ok(());
    };
    let aviso = estado.avisos.iter().rev().find(|a| codigo(&a.disparo.chave) == cod).map(|a| a.disparo.chave.clone());
    let texto = match aviso {
        None => "Esse lembrete já saiu da lista.".to_owned(),
        Some(chave) if acao == "planejar" => {
            let s = state.clone();
            let _ = operacoes::executar(state.state_file.parent().unwrap(), "responder_aviso", &json!({"chave": chave, "acao": "planejar"}),
                &super::super::rotina_rotas::agora());
            let aberta = tokio::task::spawn_blocking(move || super::super::orquestracao::rotina::abrir(&s,
                &super::super::orquestracao::rotina::PedidoDeConversa { modo: "planejar".into(), ..Default::default() })).await;
            match aberta {
                Ok(Ok(_)) => "Abri a conversa de planejamento no Niski. Pode responder por aqui ou no computador.".into(),
                _ => "Não consegui abrir a conversa. Abra a Rotina no computador.".into(),
            }
        }
        Some(chave) => {
            let dados = state.state_file.parent().unwrap().to_path_buf();
            let (acao_, chave_) = (acao.to_owned(), chave.clone());
            let r = tokio::task::spawn_blocking(move || operacoes::executar(&dados, "responder_aviso",
                &json!({"chave": chave_, "acao": acao_, "minutos": 15}), &super::super::rotina_rotas::agora())).await;
            match (acao, r) {
                (_, Ok(Err(e))) => e,
                (_, Err(_)) => "Falhou. Tente pelo app.".into(),
                ("adiar", _) => "Lembro de novo em 15 minutos.".into(),
                ("copo" | "feito", Ok(Ok(v))) if v["agua_ml"].is_u64() => format!("Anotado: {} ml hoje.", v["agua_ml"]),
                ("feito", _) => "Marcado como feito.".into(),
                ("pular", _) => "Pulado.".into(),
                _ => "Anotado.".into(),
            }
        }
    };
    registro::info("rotina_telegram_botao", &[("acao", acao)]);
    cliente.chamar(token, "answerCallbackQuery", json!({"callback_query_id": id, "text": texto.chars().take(190).collect::<String>()})).await?;
    // Tira os botões da mensagem respondida (adiar gera outra mensagem depois).
    if let Some(mensagem) = consulta["message"]["message_id"].as_i64() {
        let _ = cliente.chamar(token, "editMessageReplyMarkup", json!({"chat_id": chat, "message_id": mensagem, "reply_markup": {"inline_keyboard": []}})).await;
    }
    Ok(())
}

async fn responder_mensagem(state: &Arc<EngineState>, cliente: &(impl api::BotApi + Clone + 'static), token: &str, chat: i64, m: &Value,
    ajustes: &AjustesDaRotina, desde_ms: u64) -> Result<(), api::Falha> {
    if m["chat"]["id"].as_i64() != Some(chat) || m["from"]["id"].as_i64() != Some(chat) || m["chat"]["type"] != "private" {
        return Ok(());
    }
    if m["date"].as_u64().unwrap_or(0).saturating_mul(1000) + 60_000 < desde_ms {
        return Ok(()); // De antes de o engine subir: não vira prompt atrasado.
    }
    let texto = m["text"].as_str().unwrap_or("").trim().to_owned();
    if texto.is_empty() { return Ok(()); }
    let dados = state.state_file.parent().unwrap().to_path_buf();
    let enviar = |t: String| json!({"chat_id": chat, "text": t, "link_preview_options": {"is_disabled": true}});
    match texto.split_whitespace().next().unwrap_or("") {
        "/start" => return Ok(()),
        "/hoje" => {
            let resumo = tokio::task::spawn_blocking(move || resumo_do_dia(&dados)).await.unwrap_or_default();
            cliente.chamar(token, "sendMessage", enviar(resumo)).await?;
            return Ok(());
        }
        "/agua" => {
            let r = tokio::task::spawn_blocking(move || operacoes::executar(&dados, "registrar_agua", &json!({}), &super::super::rotina_rotas::agora())).await;
            let t = match r { Ok(Ok(v)) => format!("💧 {} ml hoje.", v["agua_ml"]), _ => "Não consegui anotar.".into() };
            cliente.chamar(token, "sendMessage", enviar(t)).await?;
            return Ok(());
        }
        c if c.starts_with('/') => {
            cliente.chamar(token, "sendMessage", enviar("Comandos: /hoje (o dia), /agua (+1 copo). Texto livre vai para o agente da rotina, se estiver ligado nos Ajustes da Rotina.".into())).await?;
            return Ok(());
        }
        _ => {}
    }
    if !ajustes.telegram_conversa {
        cliente.chamar(token, "sendMessage", enviar("Para conversar com o agente da rotina por aqui, ligue \"Conversar pelo Telegram\" em Rotina → Ajustes no Niski.".into())).await?;
        return Ok(());
    }
    registro::info("rotina_telegram_conversa", &[]);
    let _ = cliente.chamar(token, "sendChatAction", json!({"chat_id": chat, "action": "typing"})).await;
    // A resposta pode levar minutos: roda à parte e o trabalhador segue (botões continuam respondendo).
    let (s, c, t) = (state.clone(), cliente.clone(), token.to_owned());
    tokio::spawn(async move {
        let resposta = tokio::task::spawn_blocking(move || super::super::orquestracao::rotina::conversar(&s, &texto, std::time::Duration::from_secs(600))).await;
        let corpo = match resposta { Ok(Ok(r)) => r, Ok(Err(e)) => e, Err(_) => "Falhou. Tente pelo app.".into() };
        let _ = c.chamar(&t, "sendMessage", json!({"chat_id": chat, "text": corpo.chars().take(4000).collect::<String>()})).await;
    });
    Ok(())
}

pub(in crate::mobile) fn iniciar(state: Arc<EngineState>) {
    tokio::spawn(async move {
        let Ok(cliente) = api::Cliente::novo() else { return };
        observar(state, Arc::new(cliente), std::time::Duration::from_secs(3)).await;
    });
}

impl api::BotApi for Arc<api::Cliente> {
    async fn chamar(&self, token: &str, metodo: &str, corpo: Value) -> Result<Value, api::Falha> {
        api::Cliente::chamar(self, token, metodo, corpo).await
    }
}

async fn observar(state: Arc<EngineState>, cliente: impl api::BotApi + Clone + 'static, ritmo: std::time::Duration) {
    let mut memoria = Memoria::default();
    let (mut proxima, mut geracao) = (0, 0);
    let mut intervalo = tokio::time::interval(ritmo);
    intervalo.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        intervalo.tick().await;
        let atual = runtime(&state).geracao.load(Ordering::SeqCst);
        if atual != geracao { geracao = atual; proxima = 0; }
        if now_ms() < proxima { continue; }
        let _operacao = runtime(&state).operacao.lock().await;
        match passo(&state, &cliente, &mut memoria).await {
            Ok(()) => runtime(&state).dados.lock().expect("telegram da rotina poisoned").erro = None,
            Err(falha) => {
                proxima = now_ms() + falha.espera_ms;
                runtime(&state).dados.lock().expect("telegram da rotina poisoned").erro = Some(falha.texto.into());
            }
        }
    }
}

/* ── Rotas (só o desktop) ──────────────────────────────────────────────────────────────────── */

type Desktop = Option<axum::Extension<orquestracao::jarvis::Desktop>>;
fn desktop(acesso: &Desktop) -> Result<(), ApiError> {
    if acesso.is_some() { Ok(()) } else { Err(error(StatusCode::FORBIDDEN, "Configure o Telegram da rotina pelo app do computador")) }
}
fn falha(e: api::Falha) -> ApiError {
    error(StatusCode::BAD_GATEWAY, e.texto)
}

pub(in crate::mobile) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/rotina/telegram", get(status).post(preferencias))
        .route("/rotina/telegram/conectar", post(conectar))
        .route("/rotina/telegram/desconectar", post(desconectar))
        .route("/rotina/telegram/testar", post(testar))
}

async fn status(State(state): State<Arc<EngineState>>, acesso: Desktop) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    Ok(Json(runtime(&state).status()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conectar { token: String }

async fn conectar(State(state): State<Arc<EngineState>>, acesso: Desktop, Json(p): Json<Conectar>) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let token = p.token.trim().to_owned();
    if !super::rotas::token_valido(&token) {
        return Err(error(StatusCode::BAD_REQUEST, "Cole o token completo que o BotFather forneceu"));
    }
    if runtime_avisos_usa(&state, &token) {
        return Err(error(StatusCode::CONFLICT, "Este é o bot dos avisos. Crie outro bot no BotFather só para a rotina."));
    }
    let _operacao = runtime(&state).operacao.lock().await;
    let cliente = api::Cliente::novo().map_err(falha)?;
    let bot = cliente.chamar(&token, "getMe", json!({})).await.map_err(falha)?;
    let nome = bot["username"].as_str().filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .ok_or_else(|| error(StatusCode::BAD_GATEWAY, "O Telegram não informou o nome do bot"))?;
    let webhook = cliente.chamar(&token, "getWebhookInfo", json!({})).await.map_err(falha)?;
    if webhook["url"].as_str().is_some_and(|u| !u.is_empty()) {
        return Err(error(StatusCode::CONFLICT, "Este bot já tem outra integração. Crie um bot exclusivo para a rotina."));
    }
    let config = Config { token, bot: nome.into(), ..Default::default() };
    salvar_config(state.state_file.parent().unwrap(), &config).map_err(|_| falha(api::Falha::disco()))?;
    *runtime(&state).dados.lock().expect("telegram da rotina poisoned") = Dados {
        config,
        pareando: Some(Pareando { codigo: uuid::Uuid::new_v4().simple().to_string(), inicio: now_ms(), expira: now_ms() + 600_000, offset: 0 }),
        erro: None,
    };
    runtime(&state).geracao.fetch_add(1, Ordering::SeqCst);
    Ok(Json(runtime(&state).status()))
}

/// O mesmo bot nos dois lugares disputaria o `getUpdates` (o Telegram devolve 409).
fn runtime_avisos_usa(state: &EngineState, token: &str) -> bool {
    state.mobile.telegram.dados.lock().map(|d| !d.config.token.is_empty() && d.config.token == token).unwrap_or(false)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferencias { ativo: bool }

async fn preferencias(State(state): State<Arc<EngineState>>, acesso: Desktop, Json(p): Json<Preferencias>) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let _operacao = runtime(&state).operacao.lock().await;
    let mut d = runtime(&state).dados.lock().expect("telegram da rotina poisoned");
    if p.ativo && d.config.chat.is_none() {
        return Err(error(StatusCode::CONFLICT, "Conecte o bot da rotina primeiro"));
    }
    let mut config = d.config.clone();
    config.ativo = p.ativo;
    salvar_config(state.state_file.parent().unwrap(), &config).map_err(|_| falha(api::Falha::disco()))?;
    d.config = config;
    drop(d);
    runtime(&state).geracao.fetch_add(1, Ordering::SeqCst);
    Ok(Json(runtime(&state).status()))
}

async fn desconectar(State(state): State<Arc<EngineState>>, acesso: Desktop) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let _operacao = runtime(&state).operacao.lock().await;
    salvar_config(state.state_file.parent().unwrap(), &Config::default()).map_err(|_| falha(api::Falha::disco()))?;
    *runtime(&state).dados.lock().expect("telegram da rotina poisoned") = Dados::default();
    runtime(&state).geracao.fetch_add(1, Ordering::SeqCst);
    Ok(Json(runtime(&state).status()))
}

async fn testar(State(state): State<Arc<EngineState>>, acesso: Desktop) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let _operacao = runtime(&state).operacao.lock().await;
    let config = runtime(&state).dados.lock().expect("telegram da rotina poisoned").config.clone();
    let Some(chat) = config.chat else { return Err(error(StatusCode::CONFLICT, "Conecte o bot da rotina primeiro")) };
    let cliente = api::Cliente::novo().map_err(falha)?;
    cliente.chamar(&config.token, "sendMessage", json!({"chat_id": chat, "text": format!("{}: bot da rotina conectado. Os lembretes chegam aqui com botões. Comandos: /hoje e /agua.", omni_protocol::NOME_DO_PRODUTO)}))
        .await.map_err(falha)?;
    Ok(Json(json!({"ok": true})))
}

#[cfg(test)]
#[path = "rotina_testes.rs"]
mod testes;
