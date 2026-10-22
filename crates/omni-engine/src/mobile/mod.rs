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


// Fork: orquestrador de agentes (servidor MCP + rotas do fluxo). Filho deste módulo para chamar as
// rotas e planos daqui (spawn, fila, timeline) sem torná-los públicos.
pub(crate) mod orquestracao;
// Fork: busca do histórico em módulo isolado, com prazo e limites.
mod busca_historico;
// Fork: teto de sessões ativas (agentes e terminais rodando comando), configurável. Sem ele, um toque
// repetido no botão do celular enche o PC de PTYs.
pub(crate) mod limite_de_sessoes;
// Fork: caixa única de atividade do engine, compartilhada por PC e celular.
#[path = "../atividade.rs"]
mod caixa_de_atividade;
#[path = "../agendamentos.rs"]
mod agendamentos; // Fork: executor e rotas de agendamentos.
// Fork (3B): fila de contas (`routing.json`, escolha com explicação, queda só sem tela).
#[path = "../roteamento.rs"]
pub(crate) mod roteamento;

// Fork (Onda 2.0): o antigo `mobile.rs` dividido por assunto, só movido. Cada filho faz
// `use super::*` e este módulo traz os itens de volta com `use filho::*`.
mod assets;
use assets::*;
mod seguranca;
use seguranca::*;
mod celular;
use celular::*;
mod fluxos_rotas;
use fluxos_rotas::*;
mod extensoes_rotas;
use extensoes_rotas::*;
mod contas;
use contas::*;
mod atualizar_pc; // Fork: "Atualizar o PC" pelo celular.
mod anexos;
use anexos::*;
mod terminais;
use terminais::*;
mod catalogo;
use catalogo::*;
mod conversas;
use conversas::*;
mod sessoes;
use sessoes::*;
mod ajustes;
use ajustes::*;
// Fork (chat-estavel): modelo/esforço do Codex antes da primeira mensagem.
mod ajustes_sem_mensagem;
// Fork (retomar-sempre): preparar a retomada, continuar em conversa nova e trocar a pasta sumida.
mod retomada_rotas;
mod memoria_rotas;
mod sobre_mim_rotas; // Fork (Sobre mim): áreas, ajustes e sugestões dos agentes.
mod roadmaps_rotas; // Fork (Roadmaps de estudo): os mesmos arquivos do app, para o celular.
// Fork: comandos oficiais e modo de permissão da Conversa (frente 2C).
mod comandos;
mod fila;
use fila::*;
// Fork (Onda 2): "Responder no terminal" e os ajustes dos ganchos do Claude.
mod ganchos_rotas;
mod codex_rotas; // Fork: Codex app-server, opção por agente.
// Fork (trocar agente): "Continuar com…" outro agente na mesma conversa (passagem de bastão).
mod trocar_agente;
mod telegram; // Fork: Bot API; avisos no engine mesmo com a janela fechada.
mod resumo_rota; // Fork (desempenho): só o resumo da conversa, para a faixa da aba de agente.
pub(crate) use celular::{check, settings, totp};
pub(crate) use catalogo::{publish};
pub(crate) use contas::{comando_do_agente};
// `main.rs` também tem um `compartilhar_extensoes` (o do protocolo): o import explícito desempata.
use extensoes_rotas::compartilhar_extensoes;

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
    /// Arquivos de cada pasta de projeto para o `@` do composer: (lido em, lista). Ver `arquivos_da_conversa`.
    arquivos: Mutex<HashMap<String, (u64, Arc<Vec<String>>)>>,
    /// Fork: tokens e pendências do orquestrador, só em memória (`orquestracao.rs`).
    orquestracao: orquestracao::Runtime,
    atividade: Mutex<caixa_de_atividade::Registro>,
    telegram: telegram::Runtime,
}

#[derive(Default)]
struct MobileHistory {
    scan: omni_core::history::ScanCache,
    entries: Vec<omni_core::history::HistoryEntry>,
    scanned_at_ms: u64,
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
            codex_descoberto_em: Mutex::new(0), arquivos: Mutex::new(HashMap::new()),
            orquestracao: orquestracao::Runtime::default(), atividade: Mutex::new(caixa_de_atividade::Registro::abrir(dir)),
            telegram: telegram::Runtime::abrir(dir) }
    }
}

pub fn start(state: Arc<EngineState>) {
    agendamentos::iniciar(state.clone()); // Fork: continua com a janela fechada.
    caixa_de_atividade::iniciar(state.clone()); // Fork: observa mesmo sem clientes conectados.
    telegram::iniciar(state.clone()); // Fork: mensagens só quando o usuário conecta seu bot.
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
        .merge(agendamentos::rotas()) // Fork: agendamentos (3D).
        .merge(caixa_de_atividade::rotas()) // Fork: atividade e vistos, mesma API nos dois transportes.
        .merge(telegram::rotas()) // Fork: configuração exclusiva do IPC do desktop.
        .route("/projetos",get(projects))
        .route("/painel",get(painel))
        .route("/conversas/{id}/timeline",get(timeline))
        .route("/conversas/{id}/terminal",get(terminal))
        .route("/conversas/{id}/imagens/{n}/{k}",get(imagem_da_mensagem))
        // Fork: documento que o agente gerou (PDF, planilha…), para o celular baixar.
        .route("/conversas/{id}/arquivos/{n}/{k}",get(arquivo_da_mensagem))
        .route("/conversas/{id}/teclas",post(teclas))
        .route("/conversas/{id}/destravar",post(destravar_rota))
        .route("/conversas/{id}/prompt",post(prompt))
        .route("/conversas/{id}/aprovar",post(approve))
        .route("/conversas/{id}/escolher",post(escolher))
        .route("/conversas/{id}/nome",post(renomear))
        .route("/conversas/{id}/ocultar",post(ocultar))
        .route("/fluxos/execucoes",get(fluxos_listar).post(fluxos_iniciar))
        .route("/fluxos/execucoes/{id}",get(fluxos_obter))
        .route("/fluxos/execucoes/{id}/{acao}",post(fluxos_acao))
        .route("/fluxos/execucoes/{id}/nos/{no}/historico",get(fluxos_historico))
        .route("/fluxos/execucoes/{id}/nos/{no}/aprovar",post(fluxos_aprovar))
        .route("/fluxos/execucoes/{id}/nos/{no}/conversa",post(fluxos_conversa))
        .route("/conversas/{id}/bifurcar",post(bifurcar_rota))
        .merge(comandos::rotas()) // Fork: /comando e os quatro modos oficiais (2C).
        .route("/conversas/{id}/opcoes",get(opcoes))
        .route("/conversas/{id}/ajustes",post(ajustar))
        .route("/conversas/{id}/retomar",post(retomar_rota))
        .route("/contas",get(contas))
        .route("/contas/{id}/uso",post(atualizar_uso))
        .route("/pc/atualizar",post(atualizar_pc::pedir_atualizacao))
        .route("/historico/levar",post(levar_do_historico))
        .route("/historico/buscar",post(busca_historico::rota)) // Fork: busca textual limitada.
        .route("/extensoes",get(extensoes))
        .route("/extensoes/compartilhar",post(compartilhar_extensoes))
        .route("/extensoes/principal",post(definir_principal))
        .route("/conversas/{id}/arquivos",get(arquivos_da_conversa))
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
        // Fork: fluxo do orquestrador (`orquestracao.rs`).
        .merge(orquestracao::rotas())
        .merge(orquestracao::revisao_por_agente::rotas()) // Fork: revisão em agente separado (3C).
        // Fork: modo com que o app abre o Antigravity (`antigravity_permissoes.rs`).
        .merge(crate::antigravity_permissoes::rotas())
        .merge(ganchos_rotas::rotas()) // Fork (Onda 2): ganchos do Claude.
        .merge(roteamento::rotas()) // Fork (3B): fila de contas.
        .merge(codex_rotas::rotas()) // Fork: 3A.
        .merge(retomada_rotas::rotas()) // Fork (retomar-sempre).
        .merge(trocar_agente::rotas()) // Fork (trocar agente).
        .merge(memoria_rotas::rotas())
        .merge(sobre_mim_rotas::rotas()) // Fork (Sobre mim).
        .merge(roadmaps_rotas::rotas()) // Fork (Roadmaps).
        .merge(resumo_rota::rotas()) // Fork (desempenho).
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
    // Fork (desempenho): o roteador (~80 rotas) era montado de novo a cada pedido do desktop
    // (vários por segundo com conversas abertas). Monta uma vez; clonar é barato (Arc por dentro).
    static ROTAS_DO_DESKTOP: std::sync::OnceLock<Router<Arc<EngineState>>> = std::sync::OnceLock::new();
    let app = ROTAS_DO_DESKTOP.get_or_init(|| rotas().layer(DefaultBodyLimit::max(32 * 1024))).clone().with_state(state);
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
    // Fork: capacidade em memória do IPC; o HTTP do celular nunca recebe esta extensão.
    let request = request.extension(orquestracao::jarvis::Desktop).body(corpo);
    let Ok(request) = request else {
        return EngineResponse::Error { code: "INVALID_REQUEST".into(), message: format!("requisição inválida: {method} {path}") };
    };
    let response = match tower::ServiceExt::oneshot(app, request).await { Ok(response) => response, Err(never) => match never {} };
    let status = response.status().as_u16();
    // Fork (desempenho): o `ETag` atravessa o IPC; com ele o desktop manda `If-None-Match` e recebe `304`.
    let etag = response.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_owned);
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024).await.unwrap_or_default();
    EngineResponse::Api { status, body: String::from_utf8_lossy(&bytes).into_owned(), etag }
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

#[cfg(test)]
mod testes;
