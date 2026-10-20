//! Fork: orquestrador de agentes — servidor MCP do engine e as rotas que o fluxo visual usa.
//!
//! Uma conversa vira orquestradora pelo botão "Tornar orquestradora" (`POST /orquestracao/{id}/ativar`):
//! o engine marca no registro (`omni_core::orquestracao`, arquivo `orquestracao.json`) e reabre a CLI
//! com `--resume` (Codex: `resume <id>`). Toda vez que a PTY de uma orquestradora sobe, `preparar`
//! acrescenta as flags de MCP **só para aquela sessão** e gera um token novo, que vai no ambiente da
//! PTY (`MEDZA_ORQ_TOKEN`) — nunca na linha de comando, no `sessions.json` nem no log, e nunca é o
//! `engine.token`. O token só enxerga os subagentes do próprio orquestrador.
//!
//! Servidor: Streamable HTTP mínimo em `127.0.0.1:<porta do engine + 3>/mcp`, JSON-RPC com resposta
//! `application/json` (sem SSE): `initialize`, `notifications/*`, `ping`, `tools/list`, `tools/call`.
//! `GET` devolve 405 (o cliente então não abre stream). As ferramentas (`orquestracao_ferramentas.rs`)
//! chamam em processo o que o chat já usa (`resolve_spawn`/`spawn_from_mobile`, `enqueue`, timeline,
//! `interaction::capabilities`).
//!
//! Filho de `mobile` (mora em `mobile/`) para usar esses pedaços sem torná-los públicos.

use super::*;
// Fork: revisão separada compartilha registro e entrega inicial, sem tocar nas ferramentas.
#[path = "../revisao_por_agente.rs"]
pub(super) mod revisao_por_agente;
use omni_core::orquestracao as regra;
use std::sync::atomic::AtomicU16;

#[path = "orquestracao_ferramentas.rs"]
mod ferramentas;

/// Estado em memória. Nada daqui vai para disco.
#[derive(Default)]
pub struct Runtime {
    /// token → conversa orquestradora. Um token por orquestradora (o da PTY mais nova).
    tokens: Mutex<HashMap<String, String>>,
    /// Ler-mudar-gravar do `orquestracao.json` (ferramenta e tela podem gravar juntas).
    trava: Mutex<()>,
    porta: AtomicU16,
    /// Subagentes cuja tarefa inicial ainda não foi enviada (a CLI está subindo), com o motivo.
    pendentes: Mutex<HashMap<String, String>>,
    /// Quando cada subagente recebeu a última mensagem e quantos turnos a tela dele já tinha
    /// começado: `esperar` não aceita "pronto" de antes do turno que essa mensagem abriu.
    enviado_em: Mutex<HashMap<String, (u64, u64)>>,
}

impl Runtime {
    fn porta(&self) -> u16 { self.porta.load(Ordering::Relaxed) }
}

fn dir(state: &EngineState) -> &Path { state.state_file.parent().unwrap() }

/// Lê, muda e grava o registro sob a trava.
fn mudar_registro<T>(state: &EngineState, mudar: impl FnOnce(&mut regra::Registro) -> T) -> std::io::Result<T> {
    let _trava = state.mobile.orquestracao.trava.lock().expect("trava poisoned");
    let mut registro = regra::ler(dir(state));
    let resultado = mudar(&mut registro);
    regra::gravar(dir(state), &registro)?;
    Ok(resultado)
}

fn ler_registro(state: &EngineState) -> regra::Registro {
    let _trava = state.mobile.orquestracao.trava.lock().expect("trava poisoned");
    regra::ler(dir(state))
}

fn revogar(state: &EngineState, orquestrador: &str) {
    state.mobile.orquestracao.tokens.lock().expect("tokens poisoned").retain(|_, dono| dono != orquestrador);
}

/// Chamado a cada spawn/reinício de PTY (`spawn_terminal_inner`, `restart_session`): o comando a
/// digitar, com ou sem as flags de MCP conforme o registro, e o ambiente extra (o token) da PTY.
/// O comando gravado no `sessions.json` continua o original: as flags entram só no que é digitado.
pub(crate) fn preparar(state: &EngineState, conversa: Option<&str>, provider: Option<&str>, comando: Option<String>)
    -> (Option<String>, Vec<(String, String)>) {
    let (Some(conversa), Some(provider), Some(comando)) = (conversa, provider, comando.clone()) else { return (comando, Vec::new()) };
    if !regra::pode_orquestrar(provider) { return (Some(comando), Vec::new()); }
    let runtime = &state.mobile.orquestracao;
    let ativo = ler_registro(state).ativo(conversa);
    let comando = regra::ajustar_comando(&comando, provider, runtime.porta(), ativo);
    if !ativo { revogar(state, conversa); return (Some(comando), Vec::new()); }
    let token = random_token();
    {
        let mut tokens = runtime.tokens.lock().expect("tokens poisoned");
        tokens.retain(|_, dono| dono != conversa);
        tokens.insert(token.clone(), conversa.to_owned());
    }
    registro::info("orquestracao_token_novo", &[("orquestrador", conversa), ("provider", provider)]);
    (Some(comando), vec![(regra::VARIAVEL_DO_TOKEN.to_owned(), token)])
}

/// Sobe o servidor MCP. Porta ocupada: as ferramentas ficam indisponíveis (fica no log), o resto do
/// engine segue.
pub(crate) fn iniciar(state: Arc<EngineState>, porta_do_engine: u16) {
    let porta = regra::porta_do_mcp(porta_do_engine);
    state.mobile.orquestracao.porta.store(porta, Ordering::Relaxed);
    tokio::spawn(async move {
        match TcpListener::bind(("127.0.0.1", porta)).await {
            Ok(listener) => {
                registro::info("orquestracao_mcp_iniciou", &[("porta", &porta.to_string())]);
                let _ = axum::serve(listener, router(state)).await;
            }
            Err(erro) => registro::erro("orquestracao_porta_ocupada", &[("porta", &porta.to_string()), ("erro", &erro.to_string())]),
        }
    });
}

fn router(state: Arc<EngineState>) -> Router {
    Router::new()
        .route("/mcp", post(mcp).get(metodo_nao_permitido).delete(metodo_nao_permitido))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

async fn metodo_nao_permitido() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [("allow", "POST")]).into_response()
}

/// Loopback com a porta certa no `Host` e, se houver `Origin`, também loopback: defesa contra DNS
/// rebinding (página da web chamando o servidor pelo navegador). Fork: também usada pelos ganchos.
pub(crate) fn origem_local(headers: &HeaderMap, porta: u16) -> bool {
    let aceitos = [format!("127.0.0.1:{porta}"), format!("localhost:{porta}")];
    let host = headers.get("host").and_then(|v| v.to_str().ok()).unwrap_or("");
    let origem_ok = headers.get("origin").and_then(|v| v.to_str().ok())
        .is_none_or(|origem| aceitos.iter().any(|a| origem == format!("http://{a}")));
    aceitos.iter().any(|a| a == host) && origem_ok
}

/// Conversa orquestradora dona do token, se ele vale. Compara com todos (tempo constante por token).
fn dono_do_token(state: &EngineState, headers: &HeaderMap) -> Option<String> {
    let recebido = headers.get("authorization").and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
    if recebido.is_empty() { return None; }
    let mut dono = None;
    for (token, orquestrador) in state.mobile.orquestracao.tokens.lock().expect("tokens poisoned").iter() {
        if token_confere(recebido, token) { dono = Some(orquestrador.clone()); }
    }
    dono
}

async fn mcp(State(state): State<Arc<EngineState>>, headers: HeaderMap, corpo: axum::body::Bytes) -> Response {
    if !origem_local(&headers, state.mobile.orquestracao.porta()) {
        return error(StatusCode::FORBIDDEN, "Origem não permitida").into_response();
    }
    let Some(orquestrador) = dono_do_token(&state, &headers) else {
        registro::aviso_sem_repetir("orquestracao_401", "orquestracao_token_recusado", &[]);
        return error(StatusCode::UNAUTHORIZED, "Token do orquestrador inválido").into_response();
    };
    // Deixou de orquestrar com a CLI ainda aberta: o token morre junto.
    let ativo = { let state = state.clone(); let id = orquestrador.clone();
        tokio::task::spawn_blocking(move || ler_registro(&state).ativo(&id)).await.unwrap_or(false) };
    if !ativo {
        revogar(&state, &orquestrador);
        return error(StatusCode::UNAUTHORIZED, "Esta conversa não é mais orquestradora").into_response();
    }
    let Ok(mensagem) = serde_json::from_slice::<Value>(&corpo) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"JSON inválido"}}))).into_response();
    };
    let respostas = match mensagem {
        Value::Array(lote) => {
            let mut respostas = Vec::new();
            for item in lote { if let Some(resposta) = responder(&state, &orquestrador, item).await { respostas.push(resposta); } }
            if respostas.is_empty() { None } else { Some(Value::Array(respostas)) }
        }
        item => responder(&state, &orquestrador, item).await,
    };
    match respostas {
        Some(corpo) => Json(corpo).into_response(),
        // Notificação (sem `id`): só o recibo.
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// Versões do protocolo MCP que este servidor fala (o subconjunto usado não mudou entre elas).
const VERSOES: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Fork (harness): o método que o orquestrador segue. Vai no `initialize` (as CLIs põem no contexto) e,
/// resumido, na descrição do `criar_agente`.
const INSTRUCOES: &str = "Ferramentas do Medza Agents para orquestrar subagentes de qualquer plataforma (Claude Code, Codex…). \
O usuário acompanha tudo num fluxo visual e aprova permissões por lá. Método:\n\
1. Entenda o pedido e explore o projeto antes de criar subagentes: estrutura, arquivos envolvidos, convenções, como testar.\n\
2. Divida em tarefas independentes que possam correr em paralelo, cada uma com arquivos próprios e DISJUNTOS. O engine recusa \
um subagente que peça arquivo ou pasta de outro subagente vivo. O que for compartilhado (tipos, interfaces), faça você antes ou \
dê a um subagente só. Subagentes que EDITAM código em paralelo podem ir cada um numa worktree isolada (criar_agente com \
worktree: true): cópia do repo e branch medza/<nome> só dele, sem disputa de arquivo; no fim ele commita, você mescla \
(git merge medza/<nome>) e roda os testes.\n\
3. Em criar_agente, prepare o subagente por completo: tarefa clara e única; contexto (decisões, interfaces, convenções, comandos de \
teste); ler (arquivos para ler antes); editar e criar (exatamente o que ele pode mexer); criterios (como saber que acabou). O \
engine monta o briefing, inclui o que é dos outros como 'não mexa' e as regras.\n\
4. Plataforma e modelo pelo tamanho da tarefa (listar_plataformas_e_contas): modelo menor para tarefa simples.\n\
5. Quando esperar devolver 'esperando você' com um pedido: se é permissão dentro da tarefa e dos arquivos do subagente, \
aprove com responder_pedido(permitir: true); se sai disso, negue e oriente com mandar_prompt; pergunta ou menu da CLI (ex.: \
confiar na pasta), escolha a opção com responder_pedido(opcao). Pedido arriscado (apagar em massa, push, instalar global, \
credenciais) fica para o usuário: avise-o.\n\
6. esperar e ler_conversa; revise o resultado (rode os testes você mesmo) e use conferir_arquivos para ver se alguém mexeu fora \
do que era dele; corrija com mandar_prompt.\n\
7. parar_agente quando cada um terminar (libera vaga no limite) e responda ao usuário com o resumo.";

async fn responder(state: &Arc<EngineState>, orquestrador: &str, mensagem: Value) -> Option<Value> {
    let metodo = mensagem["method"].as_str().unwrap_or("").to_owned();
    let id = mensagem.get("id").cloned()?; // notificação: nada a responder
    let resultado = match metodo.as_str() {
        "initialize" => {
            let pedida = mensagem["params"]["protocolVersion"].as_str().unwrap_or("");
            let versao = VERSOES.iter().find(|v| **v == pedida).copied().unwrap_or(VERSOES[1]);
            Ok(json!({"protocolVersion": versao, "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "medza-agents", "version": env!("CARGO_PKG_VERSION")}, "instructions": INSTRUCOES}))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": ferramentas::lista()})),
        "tools/call" => {
            let nome = mensagem["params"]["name"].as_str().unwrap_or("").to_owned();
            let argumentos = mensagem["params"]["arguments"].clone();
            let saida = ferramentas::chamar(state, orquestrador, &nome, &argumentos).await;
            registro::info("orquestracao_ferramenta", &[("orquestrador", orquestrador), ("ferramenta", &nome),
                ("resultado", if saida.is_ok() { "ok" } else { "erro" })]);
            Ok(match saida {
                Ok(texto) => json!({"content": [{"type": "text", "text": texto}]}),
                Err(texto) => json!({"content": [{"type": "text", "text": texto}], "isError": true}),
            })
        }
        _ => Err(json!({"code": -32601, "message": format!("Método não suportado: {metodo}")})),
    };
    Some(match resultado {
        Ok(resultado) => json!({"jsonrpc": "2.0", "id": id, "result": resultado}),
        Err(erro) => json!({"jsonrpc": "2.0", "id": id, "error": erro}),
    })
}

/* ── O que o fluxo visual e as ferramentas leem de cada conversa ──────────────────────────────── */

/// Estado ao vivo de uma conversa (orquestradora ou subagente).
fn estado_de(state: &EngineState, conversa: &str) -> (regra::Estado, Option<Arc<LiveSession>>, Option<interaction::Capabilities>) {
    let Ok((_, Some(sessao))) = current(state, conversa) else { return (regra::Estado::Parado, None, None) };
    let meta = settle(&sessao);
    let cap = interaction::capabilities(&sessao);
    let estado = serde_json::to_value(&meta.state).ok().and_then(|v| v.as_str().map(str::to_owned));
    let rodando = interaction::provider_running_recente(meta.pid, meta.provider.as_deref().unwrap_or(""));
    let sinais = regra::Sinais { viva: true, estado: estado.as_deref(), aviso: meta.notice.as_deref(), pode_mandar: cap.prompt,
        pode_aprovar: cap.approve, tem_escolha: cap.escolha.is_some() || cap.espera.is_some() /* Fork (pergunta-da-cli) */, cli_rodando: rodando };
    (regra::estado(&sinais), Some(sessao), Some(cap))
}

/// Mensagens de texto da conversa (de quem, texto), da mais antiga para a mais nova, da última página.
fn mensagens(state: &EngineState, conversa: &str) -> Vec<(String, String)> {
    let Ok((conversa, _)) = current(state, conversa) else { return Vec::new() };
    let perfis = omni_core::profiles(dir(state));
    let timeline = {
        let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
        omni_core::conversations::timeline_cached(&conversa, &perfis, None, TIMELINE_PAGINA, &mut feeds)
    };
    timeline.messages.into_iter()
        .filter(|m| m.kind == omni_core::feed::Kind::Text && !m.text.trim().is_empty())
        .map(|m| (m.role, m.text)).collect()
}

/// Turnos que a tela da sessão já começou ("esc to interrupt" apareceu), ver `append_output`.
fn turnos(sessao: &LiveSession) -> u64 {
    sessao.interaction.lock().map(|c| c.turnos_iniciados).unwrap_or(0)
}

fn ultima_do_agente(state: &EngineState, conversa: &str) -> Option<String> {
    mensagens(state, conversa).into_iter().rev().find(|(papel, _)| papel == "assistant").map(|(_, texto)| texto)
}

fn nome_da_conta(perfis: &[omni_core::Profile], id: Option<&str>) -> Option<String> {
    id.and_then(|id| perfis.iter().find(|p| p.id == id)).map(|p| p.name.clone())
}

/// Um nó do fluxo.
fn no(state: &EngineState, conversa: &str, perfis: &[omni_core::Profile], agente: Option<&regra::Agente>) -> Value {
    let indice = current(state, conversa).ok().map(|(c, _)| c);
    let segmento = indice.as_ref().and_then(|c| c.segments.last());
    let (estado, _, cap) = estado_de(state, conversa);
    let conta_id = segmento.and_then(|s| s.profile_id.clone()).or_else(|| agente.and_then(|a| a.conta.clone()));
    let pendente = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").get(conversa).cloned();
    json!({
        "id": conversa,
        "titulo": indice.as_ref().map(omni_core::conversations::display_title),
        "plataforma": segmento.map(|s| s.provider.clone()).or_else(|| agente.map(|a| a.plataforma.clone())),
        "conta_id": conta_id,
        "conta": nome_da_conta(perfis, conta_id.as_deref()),
        "modelo": agente.and_then(|a| a.modelo.clone()),
        "esforco": agente.and_then(|a| a.esforco.clone()),
        "tarefa": agente.map(|a| a.tarefa.clone()),
        "editar": agente.map(|a| a.editar.clone()).unwrap_or_default(),
        "criar": agente.map(|a| a.criar.clone()).unwrap_or_default(),
        // Fork: worktree isolada do subagente (`criar_agente` com `worktree`).
        "branch": agente.and_then(|a| a.branch.clone()),
        "criado_em": agente.map(|a| a.criado_em),
        "estado": estado,
        "estado_texto": estado.texto(),
        // Pedido que só o usuário aprova (o orquestrador não pode): o nó destaca.
        "arriscado": cap.as_ref().filter(|c| c.approve).and_then(|c| c.approval_text.as_deref())
            .and_then(omni_core::orquestracao::briefing::pedido_arriscado),
        "pendente": pendente,
        "ultima_mensagem": ultima_do_agente(state, conversa).map(|t| regra::resumir(&t, 600)),
        "capabilities": cap,
    })
}

/* ── Rotas da tela (pelo protocolo local do desktop, como o resto do chat) ────────────────────── */

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/orquestracao", get(listar))
        .route("/orquestracao/limite", post(definir_limite))
        .route("/orquestracao/{id}", get(fluxo))
        .route("/orquestracao/{id}/ativar", post(ativar))
        .route("/orquestracao/{id}/agentes/{agente}/parar", post(parar_pela_tela))
}

/// Resumo leve para a lateral e a faixa: quem é orquestradora e quantos subagentes tem.
async fn listar(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || {
        let registro = ler_registro(&state);
        json!({
            "limite": registro.limite(),
            "orquestradores": registro.orquestradores.iter().map(|o| json!({
                "orquestrador": o.orquestrador, "ativo": o.ativo, "agentes": o.agentes.len(),
                "subagentes": o.agentes.iter().map(|a| a.conversation_key.clone()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }).await.unwrap_or(json!({"limite": regra::LIMITE_PADRAO, "orquestradores": []})))
}

/// O fluxo inteiro de uma orquestradora (ou da dona de um subagente).
async fn fluxo(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let registro = ler_registro(&state);
        let dono = if registro.orquestrador(&id).is_some() { id.clone() }
            else { registro.dono_de(&id).map(str::to_owned).unwrap_or_else(|| id.clone()) };
        let perfis = omni_core::profiles(dir(&state));
        let entrada = registro.orquestrador(&dono);
        let mut centro = no(&state, &dono, &perfis, None);
        // Modelo do orquestrador: o que a CLI está usando (lido do transcript, como os chips do chat).
        if let Ok(opcoes) = opcoes_da_conversa(&state, &dono) { centro["modelo"] = opcoes["model"].clone(); centro["esforco"] = opcoes["effort"].clone(); }
        let plataforma = centro["plataforma"].as_str().unwrap_or("").to_owned();
        Ok(Json(json!({
            "orquestrador": centro,
            "ativo": entrada.is_some_and(|o| o.ativo),
            "pode_orquestrar": regra::pode_orquestrar(&plataforma),
            "e_subagente": registro.dono_de(&dono).is_some(),
            "limite": registro.limite(),
            "agentes": entrada.map(|o| o.agentes.iter().map(|a| no(&state, &a.conversation_key, &perfis, Some(a))).collect::<Vec<_>>()).unwrap_or_default(),
        })))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao montar o fluxo"))?
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct AtivarBody { ativo: bool }

/// "Tornar orquestradora" / "Deixar de orquestrar": marca no registro e reabre a CLI (com `--resume`)
/// para as flags e o token valerem — ou saírem. Sem sessão aberta, vale na próxima abertura.
async fn ativar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<AtivarBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let (conversa, viva) = current(&state, &id)?;
        let provider = conversa.segments.last().map(|s| s.provider.clone()).unwrap_or_default();
        if body.ativo && !regra::pode_orquestrar(&provider) {
            return Err(error(StatusCode::CONFLICT, "Esta plataforma ainda não pode ser orquestradora (por enquanto só Claude e Codex). Ela pode ser subagente."));
        }
        if body.ativo && ler_registro(&state).dono_de(&id).is_some() {
            return Err(error(StatusCode::CONFLICT, "Um subagente não pode orquestrar outros (sem recursão nesta versão)"));
        }
        if let Some(sessao) = &viva {
            let cap = interaction::capabilities(sessao);
            let texto = sessao.interaction.lock().map(|c| c.parser.screen().contents()).unwrap_or_default();
            let (pid, prov) = sessao.meta.lock().map(|m| (m.pid, m.provider.clone().unwrap_or_default())).unwrap_or((None, String::new()));
            if cap.approve || cap.escolha.is_some() { return Err(error(StatusCode::CONFLICT, "Responda a pergunta do agente antes")); }
            if !cap.prompt && interaction::provider_running(pid, &prov) && interaction::trabalhando(&texto) {
                return Err(error(StatusCode::CONFLICT, "Espere o agente terminar o turno: reabrir agora cortaria o trabalho"));
            }
        }
        mudar_registro(&state, |r| r.definir_ativo(&id, body.ativo))
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui gravar o registro"))?;
        if !body.ativo { revogar(&state, &id); }
        registro::info("orquestracao_ativar", &[("orquestrador", &id), ("ativo", if body.ativo { "sim" } else { "nao" }), ("provider", &provider)]);
        // Reiniciar reabre a mesma conversa (`--resume`/`resume <id>`); `preparar` põe ou tira as flags.
        let reaberta = match &viva {
            Some(sessao) => {
                let sessao_id = sessao.meta.lock().map(|m| m.id.clone()).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Sessão indisponível"))?;
                restart_session(&state, &sessao_id).map_err(|e| error(StatusCode::CONFLICT, &e.to_string()))?;
                if provider == "claude" { passar_pela_confianca(&state, &sessao_id); }
                true
            }
            None => false,
        };
        Ok(Json(json!({"ativo": body.ativo, "reaberta": reaberta})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ativar"))?
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct LimiteBody { limite: usize }

async fn definir_limite(State(state): State<Arc<EngineState>>, Json(body): Json<LimiteBody>) -> Result<Json<Value>, ApiError> {
    if body.limite == 0 || body.limite > regra::LIMITE_MAXIMO {
        return Err(error(StatusCode::BAD_REQUEST, &format!("Limite entre 1 e {}", regra::LIMITE_MAXIMO)));
    }
    tokio::task::spawn_blocking(move || {
        mudar_registro(&state, |r| { r.limite_de_agentes = Some(body.limite); r.limite() })
            .map(|limite| Json(json!({"limite": limite})))
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui gravar o registro"))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao gravar"))?
}

async fn parar_pela_tela(State(state): State<Arc<EngineState>>, RoutePath((id, agente)): RoutePath<(String, String)>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        ferramentas::parar(&state, &id, &agente).map(|texto| Json(json!({"ok": true, "mensagem": texto})))
            .map_err(|texto| error(StatusCode::CONFLICT, &texto))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao parar"))?
}

#[cfg(test)]
#[path = "orquestracao_testes.rs"]
mod testes;
