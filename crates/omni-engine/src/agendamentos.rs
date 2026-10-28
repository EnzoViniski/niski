//! Executor persistente do engine, filho de mobile para reutilizar spawn, retomada e fila.
//! Automática (3B): substituir `conta_escolhida` por resolução da política antes de resolve_spawn;
//! nunca trocar a conta de uma conversa existente. Até lá só a conta explicitamente escolhida.
use super::*;
use chrono::{Local, NaiveDateTime, TimeZone};
use omni_core::agendamentos::{Destino, Pedido};

const ARQUIVO: &str = "agendamentos.json";
const MAX_HISTORICO: usize = 20;
static TRAVA: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Execucao {
    pub(super) em_ms: i64,
    pub(super) estado: String,
    pub(super) conversa: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pendente {
    chave: String,
    fase: String,
    conversa: Option<String>,
    acao: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Agendamento {
    #[serde(flatten)]
    pub(super) pedido: Pedido,
    pub(super) id: String,
    pub(super) pausado: bool,
    pub(super) proxima_ms: Option<i64>,
    pub(super) fuso: String,
    pub(super) historico: Vec<Execucao>,
    pendente: Option<Pendente>,
    tentar_em_ms: i64,
    #[serde(default)]
    rodar_agora: bool,
}
// Pedido.id é campo de entrada, não deve se duplicar com Agendamento.id no JSON.
#[derive(Default, Serialize, Deserialize)]
struct Registro {
    itens: Vec<Agendamento>,
}
fn ler(dir: &Path) -> Result<Registro> {
    match std::fs::read(dir.join(ARQUIVO)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| anyhow!("Arquivo de agendamentos inválido; preserve-o para recuperação")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registro::default()),
        Err(_) => Err(anyhow!("Não foi possível ler os agendamentos")),
    }
}
fn gravar(dir: &Path, r: &Registro) -> Result<()> {
    atomic_write_json(&dir.join(ARQUIVO), r)?;
    Ok(())
}
fn falha() -> ApiError {
    error(
        StatusCode::CONFLICT,
        "Não foi possível guardar ou executar o agendamento",
    )
}

pub(super) fn iniciar(state: Arc<EngineState>) {
    // Incerto após queda: nunca reenviar automaticamente algo que talvez já chegou à CLI.
    {
        let _trava = TRAVA.lock().unwrap();
        let _ = recuperar(state.state_file.parent().unwrap());
    }
    tokio::spawn(async move {
        let mut intervalo = tokio::time::interval(std::time::Duration::from_secs(30));
        intervalo.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            intervalo.tick().await;
            let state = state.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _trava = TRAVA.lock().unwrap();
                let dir = state.state_file.parent().unwrap();
                let Ok(mut r) = ler(dir) else { return };
                if tique(dir, &mut r, now_ms() as i64, &mut ExecutorReal(&state)).is_err() {
                    registro::aviso("agendamento_falhou", &[("etapa", "persistencia")]);
                }
            })
            .await;
        }
    });
}

fn recuperar(dir: &Path) -> Result<()> {
    let mut r = ler(dir)?;
    let mut mudou = false;
    for a in &mut r.itens {
        if a.pendente.take().is_some() {
            finalizar(a, "interrompido", None);
            mudou = true;
        }
    }
    if mudou {
        gravar(dir, &r)?;
    }
    Ok(())
}

fn finalizar(a: &mut Agendamento, estado: &str, conversa: Option<String>) {
    if let Some(h) = a.historico.last_mut() {
        h.estado = estado.into();
        if conversa.is_some() {
            h.conversa = conversa;
        }
    }
    a.pendente = None;
}
enum Passo {
    Preparada(String),
    Enfileirada(String),
    Aguardar,
    Repetir,
    Terminou,
    Falhou,
}
trait Executor {
    fn passo(&mut self, a: &Agendamento) -> Passo;
}

/// Relógio e executor injetados. O prazo consumido é gravado ANTES de qualquer efeito na CLI.
fn tique(dir: &Path, r: &mut Registro, agora: i64, executor: &mut impl Executor) -> Result<()> {
    for i in 0..r.itens.len() {
        let a = &mut r.itens[i];
        if a.pendente.is_none() {
            if !a.rodar_agora && (a.pausado || !a.proxima_ms.is_some_and(|p| p <= agora)) {
                continue;
            }
            // Rodar agora mantém a pausa e a próxima recorrência; se já venceu, consome uma só.
            if !a.rodar_agora || a.proxima_ms.is_some_and(|p| p <= agora) {
                a.proxima_ms = a.pedido.regra.proxima(agora, &Local);
            }
            a.rodar_agora = false;
            a.historico.push(Execucao {
                em_ms: agora,
                estado: "em_andamento".into(),
                conversa: None,
            });
            if a.historico.len() > MAX_HISTORICO {
                a.historico.remove(0);
            }
            a.pendente = Some(Pendente {
                chave: format!("agenda-{}-{agora}", a.id),
                fase: "preparar".into(),
                conversa: None,
                acao: None,
            });
            a.tentar_em_ms = agora;
            gravar(dir, r)?;
        }
        let a = &mut r.itens[i];
        if agora < a.tentar_em_ms {
            continue;
        }
        // Prazo para não deixar um agente que nunca ficou pronto bloquear o calendário para sempre.
        if a.historico
            .last()
            .is_some_and(|h| agora.saturating_sub(h.em_ms) > 24 * 3600_000)
        {
            finalizar(a, "expirado", None);
        } else {
            match executor.passo(a) {
                Passo::Preparada(conversa) => {
                    let p = a.pendente.as_mut().unwrap();
                    p.fase = "entregar".into();
                    p.conversa = Some(conversa.clone());
                    if let Some(h) = a.historico.last_mut() {
                        h.conversa = Some(conversa);
                    }
                }
                Passo::Enfileirada(acao) => {
                    let p = a.pendente.as_mut().unwrap();
                    p.fase = "enviada".into();
                    p.acao = Some(acao);
                }
                Passo::Repetir => {
                    let p = a.pendente.as_mut().unwrap();
                    p.fase = "entregar".into();
                    p.acao = None;
                    p.chave = format!(
                        "agenda-{}-{}-{agora}",
                        a.id,
                        a.historico.last().unwrap().em_ms
                    );
                    a.tentar_em_ms = agora + 60_000;
                    if let Some(h) = a.historico.last_mut() {
                        h.estado = "adiado".into();
                    }
                }
                Passo::Aguardar => {
                    a.tentar_em_ms = agora + 60_000;
                    if let Some(h) = a.historico.last_mut() {
                        h.estado = "adiado".into();
                    }
                }
                Passo::Terminou => finalizar(a, "enviado", None),
                Passo::Falhou => finalizar(a, "erro", None),
            }
        }
        gravar(dir, r)?;
    }
    Ok(())
}

struct ExecutorReal<'a>(&'a Arc<EngineState>);
fn conta_escolhida(conta: &Option<String>) -> Option<String> {
    conta.clone()
}
fn acao_interna(state: &EngineState, conversa: &str, kind: ActionKind) -> Action {
    Action {
        id: state.next_id(),
        conversation_id: conversa.into(),
        session_id: String::new(),
        state: "executing".into(),
        created_at_ms: now_ms(),
        error: None,
        key: String::new(),
        revision: String::new(),
        kind,
    }
}
impl Executor for ExecutorReal<'_> {
    fn passo(&mut self, a: &Agendamento) -> Passo {
        let state = self.0;
        let Some(p) = &a.pendente else {
            return Passo::Falhou;
        };
        if p.fase == "enviada" {
            let actions = state.mobile.actions.lock().unwrap();
            return match actions.iter().find(|x| Some(&x.id) == p.acao.as_ref()) {
                Some(x) if x.state == "sent" => Passo::Terminou,
                Some(x) if x.state == "queued" || x.state == "executing" => Passo::Aguardar,
                // Só estes erros ocorrem comprovadamente antes de escrever qualquer byte na PTY.
                Some(x)
                    if matches!(
                        x.error.as_deref(),
                        Some("Entrada não está disponível" | "Contexto mudou antes da execução")
                    ) =>
                {
                    Passo::Repetir
                }
                _ => Passo::Falhou,
            };
        }
        if p.fase == "preparar" {
            return match preparar(state, a) {
                Ok(id) => Passo::Preparada(id),
                Err((status, _)) if status == StatusCode::TOO_MANY_REQUESTS => Passo::Aguardar,
                Err(_) => Passo::Falhou,
            };
        }
        let Some(id) = p.conversa.as_deref() else {
            return Passo::Falhou;
        };
        let mut headers = HeaderMap::new();
        let Ok(chave) = p.chave.parse() else {
            return Passo::Falhou;
        };
        headers.insert("idempotency-key", chave);
        // Mesma fila do chat: confere campo vazio agora e novamente na execução, inclusive em turno.
        match enqueue(
            state,
            id,
            &headers,
            ActionKind::Fila(a.pedido.prompt.clone()),
        ) {
            Ok((_, Json(v))) => v["id"]
                .as_str()
                .map(|id| Passo::Enfileirada(id.into()))
                .unwrap_or(Passo::Falhou),
            Err((status, _))
                if status == StatusCode::CONFLICT || status == StatusCode::TOO_MANY_REQUESTS =>
            {
                Passo::Aguardar
            }
            Err(_) => Passo::Falhou,
        }
    }
}
fn preparar(state: &Arc<EngineState>, a: &Agendamento) -> Result<String, ApiError> {
    match &a.pedido.destino {
        Destino::Conversa { conversa } => {
            // Histórico nativo ganha id estável ao retomar. Nas próximas recorrências reutiliza
            // essa conversa, inclusive após reiniciar o engine, em vez de abrir outra PTY.
            let retomada_id = stable_id("conv", &[conversa]);
            let alvo = if current(state, conversa).is_ok() {
                conversa
            } else if current(state, &retomada_id).is_ok() {
                &retomada_id
            } else {
                conversa
            };
            if let Ok((_, Some(viva))) = current(state, alvo) {
                // Stop conserva Live no mapa; só reutiliza uma sessão com processo ativo.
                let (sid, ativa) = {
                    let meta = viva.meta.lock().map_err(|_| falha())?;
                    (meta.id.clone(), meta.pid.is_some() && matches!(meta.state,
                        SessionState::Working | SessionState::Answered | SessionState::ApprovalRequired))
                };
                if ativa && viva.child.lock().map_err(|_| falha())?.try_wait().map_err(|_| falha())?.is_none() {
                    return Ok(alvo.clone());
                }
                // O planejador de retomada também recusa uma Live vinculada, mesmo já encerrada.
                close_session(state, &sid).map_err(|_| falha())?;
            }
            limite_de_sessoes::conferir(state)?;
            let plano = plano_de_retomada(
                state,
                alvo,
                Ajustes {
                    profile_id: None,
                    model: None,
                    effort: None,
                },
            )?;
            let id = plano.conversation_id.clone();
            let action = acao_interna(state, &id, ActionKind::Retomar(plano.clone()));
            retomar(state, &action, &plano).map_err(|_| falha())?;
            Ok(id)
        }
        Destino::NovoAgente {
            projeto,
            plataforma,
            conta,
            modelo,
            pasta,
            worktree,
        } => {
            let body = SpawnBody {
                project_id: projeto.clone(),
                provider: plataforma.clone(),
                profile_id: conta_escolhida(conta),
                titulo: Some(a.pedido.titulo.clone()),
            };
            let mut plano = resolve_spawn(state, &body)?; // inclui limite_de_sessoes e conta da plataforma.
            let raiz = Path::new(&plano.cwd).canonicalize().map_err(|_| falha())?;
            let pasta = Path::new(pasta)
                .canonicalize()
                .map_err(|_| error(StatusCode::CONFLICT, "A pasta não existe"))?;
            if !pasta.starts_with(&raiz) {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "Escolha uma pasta dentro do projeto",
                ));
            }
            plano.cwd = pasta.to_string_lossy().into_owned();
            let wt = if *worktree {
                Some(
                    omni_core::worktree_isolada::criar(
                        &pasta,
                        &omni_core::worktree_isolada::nome_a_partir_de(&a.pedido.titulo),
                    )
                    .map_err(|_| falha())?,
                )
            } else {
                None
            };
            if let Some(wt) = &wt {
                plano.cwd = wt.pasta_de_trabalho.clone();
            }
            if let Some(modelo) = modelo {
                plano.command.push_str(&format!(" --model {modelo}"));
            }
            // resolve_spawn calculou o transcript antes de escolher a subpasta/worktree.
            if let Some(sid) = &plano.external_session_id {
                plano.transcript_path = omni_core::profiles(state.state_file.parent().unwrap())
                    .iter()
                    .find(|p| Some(&p.id) == plano.profile_id.as_ref())
                    .map(|p| {
                        omni_core::conversations::claude_transcript_path(
                            Path::new(&p.config_dir),
                            &plano.cwd,
                            sid,
                        )
                        .to_string_lossy()
                        .into_owned()
                    });
            }
            let id = format!("conv-{}", uuid::Uuid::new_v4());
            plano.conversation_id = Some(id.clone());
            let action = acao_interna(state, &id, ActionKind::Spawn(plano.clone()));
            if spawn_from_mobile(state, &action, &plano).is_err() {
                if let Some(wt) = wt {
                    let _ = omni_core::worktree_isolada::remover(Path::new(&wt.caminho), false);
                }
                return Err(falha());
            }
            Ok(id)
        }
    }
}

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/agendamentos", get(listar).post(salvar))
        .route("/agendamentos/opcoes", get(opcoes))
        .route("/agendamentos/data", post(data_local))
        .route("/agendamentos/{id}/{acao}", post(acionar))
}
async fn listar(State(state): State<Arc<EngineState>>) -> Result<Json<Value>, ApiError> {
    let _trava = TRAVA.lock().unwrap();
    let r = ler(state.state_file.parent().unwrap()).map_err(|_| falha())?;
    Ok(Json(json!(r.itens)))
}
async fn opcoes(State(state): State<Arc<EngineState>>) -> Json<Value> {
    let contas: Vec<_> = omni_core::profiles(state.state_file.parent().unwrap())
        .iter()
        .map(|p| json!({"id":p.id, "name":p.name, "provider":p.provider}))
        .collect();
    let plataformas: Vec<_> = state
        .mobile
        .workspace
        .lock()
        .unwrap()
        .agents
        .iter()
        .map(|a| json!({"id":a.id, "label":a.label}))
        .collect();
    Json(
        json!({"projetos": known_projects(&state), "contas":contas, "plataformas":plataformas, "fuso": Local::now().offset().to_string()}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataLocal {
    local: String,
}
async fn data_local(Json(body): Json<DataLocal>) -> Result<Json<Value>, ApiError> {
    let data = NaiveDateTime::parse_from_str(&body.local, "%Y-%m-%dT%H:%M")
        .ok()
        .and_then(|d| Local.from_local_datetime(&d).earliest())
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                "Data inválida ou hora inexistente no fuso do engine",
            )
        })?;
    Ok(Json(json!({"em_ms": data.timestamp_millis()})))
}
fn validar_destino(state: &EngineState, destino: &Destino) -> Result<(), ApiError> {
    match destino {
        Destino::Conversa { conversa } => {
            if current(state, conversa).is_err() && ultima_sessao(state, conversa).is_err() {
                return Err(error(StatusCode::BAD_REQUEST, "Conversa não encontrada"));
            }
        }
        Destino::NovoAgente {
            projeto,
            plataforma,
            conta,
            pasta,
            ..
        } => {
            let projeto = known_projects(state)
                .into_iter()
                .find(|p| &p.id == projeto)
                .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Projeto não encontrado"))?;
            if !state
                .mobile
                .workspace
                .lock()
                .unwrap()
                .agents
                .iter()
                .any(|a| &a.id == plataforma)
            {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "CLI não publicada pelo desktop",
                ));
            }
            if conta.as_ref().is_some_and(|id| {
                !omni_core::profiles(state.state_file.parent().unwrap())
                    .iter()
                    .any(|p| &p.id == id && &p.provider == plataforma)
            }) {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "Conta não encontrada para esta plataforma",
                ));
            }
            let raiz = Path::new(&projeto.path)
                .canonicalize()
                .map_err(|_| error(StatusCode::BAD_REQUEST, "Projeto sem pasta"))?;
            let pasta = Path::new(pasta)
                .canonicalize()
                .map_err(|_| error(StatusCode::BAD_REQUEST, "A pasta não existe"))?;
            if !pasta.is_dir() || !pasta.starts_with(raiz) {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "Escolha uma pasta dentro do projeto",
                ));
            }
        }
    }
    Ok(())
}

async fn salvar(
    State(state): State<Arc<EngineState>>,
    Json(mut pedido): Json<Pedido>,
) -> Result<Json<Value>, ApiError> {
    pedido
        .validar()
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e))?;
    validar_destino(&state, &pedido.destino)?;
    let agora = now_ms() as i64;
    let proxima_ms = pedido
        .regra
        .proxima(agora, &Local)
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Escolha uma data futura"))?;
    let _trava = TRAVA.lock().unwrap();
    let dir = state.state_file.parent().unwrap();
    let mut r = ler(dir).map_err(|_| falha())?;
    let existente = pedido.id.take();
    let id = existente
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let novo = Agendamento {
        pedido,
        id: id.clone(),
        pausado: false,
        proxima_ms: Some(proxima_ms),
        fuso: "local_do_engine".into(),
        historico: vec![],
        pendente: None,
        tentar_em_ms: 0,
        rodar_agora: false,
    };
    if existente.is_some() {
        let a = r
            .itens
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "Agendamento não encontrado"))?;
        if a.pendente.is_some() {
            return Err(error(
                StatusCode::CONFLICT,
                "Aguarde a execução atual para editar",
            ));
        }
        *a = Agendamento {
            historico: a.historico.clone(),
            pausado: a.pausado,
            ..novo.clone()
        };
    } else {
        if r.itens.len() >= 200 {
            return Err(error(
                StatusCode::TOO_MANY_REQUESTS,
                "Limite de 200 agendamentos",
            ));
        }
        r.itens.push(novo);
    }
    gravar(dir, &r).map_err(|_| falha())?;
    Ok(Json(json!({"id": id, "proxima_ms": proxima_ms})))
}
async fn acionar(
    State(state): State<Arc<EngineState>>,
    RoutePath((id, acao)): RoutePath<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let _trava = TRAVA.lock().unwrap();
    let dir = state.state_file.parent().unwrap();
    let mut r = ler(dir).map_err(|_| falha())?;
    let a = r
        .itens
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Agendamento não encontrado"))?;
    match acao.as_str() {
        "pausar" => a.pausado = true,
        "retomar" => {
            a.pausado = false;
            a.proxima_ms = a.pedido.regra.proxima(now_ms() as i64, &Local);
        }
        "rodar" if a.pendente.is_none() => a.rodar_agora = true,
        "apagar" if a.pendente.is_none() => r.itens.retain(|a| a.id != id),
        "rodar" | "apagar" => return Err(error(StatusCode::CONFLICT, "Aguarde a execução atual")),
        _ => return Err(error(StatusCode::BAD_REQUEST, "Ação inválida")),
    }
    gravar(dir, &r).map_err(|_| falha())?;
    Ok(Json(json!({"ok": true})))
}

/// Itens reconstruídos do histórico persistido. Nenhum prompt entra na Atividade.
pub(super) fn atividade(state: &EngineState) -> Vec<Value> {
    let _trava = TRAVA.lock().unwrap();
    let dir = state.state_file.parent().unwrap();
    let Ok(r) = ler(dir) else { return vec![] };
    let indice = omni_core::conversations::read_index(dir);
    r.itens.iter().flat_map(|a| a.historico.iter().filter(|h| h.estado != "em_andamento").map(|h| {
        let c = h.conversa.as_ref().and_then(|id| indice.iter().find(|c| &c.id == id));
        let segmento = c.and_then(|c| c.segments.last());
        json!({"id":format!("agendamento:{}:{}:{}",a.id,h.em_ms,h.estado), "conversa": h.conversa.clone().unwrap_or_else(|| format!("agendamento:{}", a.id)),
            "sessao": segmento.and_then(|s|s.terminal_session_id.as_deref()).unwrap_or(""),
            "projeto_id":c.map(|c|c.project_id.as_str()).unwrap_or(""), "projeto":"Agendamentos", "titulo":a.pedido.titulo,
            "plataforma": segmento.map(|s|s.provider.as_str()).unwrap_or(""), "conta_id":segmento.and_then(|s|s.profile_id.clone()),
            "conta":"Conta escolhida", "tipo":"agendamento", "desde_ms":h.em_ms, "visto":false,
            "resumo":format!("Agendamento: {}", h.estado), "orquestrador":null})
    })).collect()
}

#[cfg(test)]
#[path = "agendamentos_testes.rs"]
mod testes;
