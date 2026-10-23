//! Fork (Onda 2, itens 1–3): rotas da conversa e das Configurações para os ganchos do Claude.
//! - `POST /conversas/{id}/pedido/terminal` — "Responder no terminal": solta o pedido do gancho e a CLI
//!   mostra o diálogo dela;
//! - `GET|POST /ganchos/ajustes` — eventos (padrão ligado), permissões pelo app (`ligado`, padrão desligado),
//!   prazo do pedido (2/5/10 min) e
//!   "Ler uso pela statusline" por conta (padrão ligado desde a decisão C5: é a única fonte do uso).
//!
//! O `/conversas/{id}/aprovar` continua em `fila.rs`; quando há pedido do gancho ele chama
//! `responder_pelo_gancho` daqui em vez de entrar na fila de teclas.
use super::*;
use crate::ganchos::{self as ganchos, Decisao, ErroDeResposta};
use omni_core::ganchos as regra;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/conversas/{id}/pedido/terminal", post(soltar))
        .route("/ganchos/ajustes", get(ler_ajustes).post(gravar_ajustes))
}

/// Terminal da sessão viva da conversa.
fn terminal_da_conversa(state: &EngineState, id: &str) -> Result<String, ApiError> {
    let (_, sessao) = current(state, id)?;
    let sessao = sessao.ok_or_else(|| error(StatusCode::CONFLICT, "Sessão não está aberta no PC"))?;
    sessao.meta.lock().map(|m| m.id.clone()).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Sessão indisponível"))
}

fn erro_da_resposta(erro: ErroDeResposta) -> ApiError {
    match erro {
        ErroDeResposta::Nenhum => error(StatusCode::CONFLICT, "Este pedido já foi respondido"),
        ErroDeResposta::Outro => error(StatusCode::PRECONDITION_FAILED, "O pedido mudou; confira o novo antes de responder"),
    }
}

/// Mensagem de quem nega ("dizer o que fazer"): texto curto, sem caractere de controle.
fn mensagem_valida(mensagem: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(texto) = mensagem.map(str::trim).filter(|t| !t.is_empty()) else { return Ok(None) };
    if texto.chars().count() > 2_000 || texto.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(error(StatusCode::BAD_REQUEST, "Mensagem de até 2000 caracteres, sem caracteres de controle"));
    }
    Ok(Some(texto.to_owned()))
}

/// `Some` quando a conversa tem pedido do gancho (ou a tela mandou o id de um): responde direto,
/// sem tecla e sem fila. `None`: segue o caminho de hoje (diálogo na tela).
pub(super) async fn responder_pelo_gancho(state: &Arc<EngineState>, id: &str, permitir: bool, pedido: Option<String>, mensagem: Option<String>)
    -> Option<Result<(StatusCode, Json<Value>), ApiError>> {
    let (s, conversa) = (state.clone(), id.to_owned());
    let terminal = tokio::task::spawn_blocking(move || terminal_da_conversa(&s, &conversa)).await.ok()?.ok();
    let tem = terminal.as_deref().is_some_and(ganchos::tem_pedido);
    if !tem && pedido.is_none() { return None; }
    Some((|| {
        let terminal = terminal.ok_or_else(|| erro_da_resposta(ErroDeResposta::Nenhum))?;
        let decisao = if permitir { Decisao::Permitir } else { Decisao::Negar(mensagem_valida(mensagem.as_deref())?) };
        // Pedido do gancho exige o id que a tela mostrou: sem ele, um pedido novo seria aprovado às cegas.
        let pedido = pedido.ok_or_else(|| erro_da_resposta(ErroDeResposta::Outro))?;
        ganchos::responder(&terminal, Some(&pedido), decisao).map_err(erro_da_resposta)?;
        Ok((StatusCode::OK, Json(json!({"state": "sent", "via": "gancho"}))))
    })())
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct SoltarBody { #[serde(default)] pedido: Option<String> }

/// Corpo opcional: `{"pedido": "<id>"}` confere que é o pedido que a tela mostrou.
async fn soltar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, corpo: axum::body::Bytes) -> Result<Json<Value>, ApiError> {
    let pedido = if corpo.iter().all(u8::is_ascii_whitespace) { None } else {
        serde_json::from_slice::<SoltarBody>(&corpo).map_err(|_| error(StatusCode::BAD_REQUEST, "Corpo inválido"))?.pedido
    };
    tokio::task::spawn_blocking(move || {
        let terminal = terminal_da_conversa(&state, &id)?;
        ganchos::responder(&terminal, pedido.as_deref(), Decisao::Soltar).map_err(erro_da_resposta)?;
        Ok(Json(json!({"ok": true})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao soltar o pedido"))?
}

fn ajustes_em_json(state: &EngineState) -> Value {
    let dir = state.state_file.parent().unwrap();
    let ajustes = regra::ler_ajustes(dir);
    let contas: Vec<Value> = omni_core::profiles(dir).into_iter().filter(|p| p.provider == "claude")
        .map(|p| json!({"id": p.id, "nome": p.name, "statusline": ajustes.statusline_ligada(&p.id)})).collect();
    json!({"eventos": ajustes.eventos, "ligado": ajustes.ligado, "espera_s": ajustes.espera(), "esperas": regra::ESPERAS_PERMITIDAS_S, "contas": contas})
}

async fn ler_ajustes(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || ajustes_em_json(&state)).await.unwrap_or(json!({})))
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct StatuslineDaConta { conta: String, ligado: bool }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct AjustesBody { #[serde(default)] eventos: Option<bool>, #[serde(default)] ligado: Option<bool>, #[serde(default)] espera_s: Option<u64>, #[serde(default)] statusline: Option<StatuslineDaConta> }

/// Vale para as sessões que abrirem (ou reabrirem) depois: as já abertas seguem como estão.
async fn gravar_ajustes(State(state): State<Arc<EngineState>>, Json(body): Json<AjustesBody>) -> Result<Json<Value>, ApiError> {
    if body.espera_s.is_some_and(|s| !regra::ESPERAS_PERMITIDAS_S.contains(&s)) {
        return Err(error(StatusCode::BAD_REQUEST, "Prazo deve ser 2, 5 ou 10 minutos"));
    }
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap().to_path_buf();
        let mut ajustes = regra::ler_ajustes(&dir);
        if let Some(eventos) = body.eventos { ajustes.eventos = eventos; }
        if let Some(ligado) = body.ligado { ajustes.ligado = ligado; }
        if let Some(espera) = body.espera_s { ajustes.espera_s = espera; }
        if let Some(StatuslineDaConta { conta, ligado }) = body.statusline {
            if !omni_core::profiles(&dir).iter().any(|p| p.id == conta && p.provider == "claude") {
                return Err(error(StatusCode::NOT_FOUND, "Conta Claude não encontrada"));
            }
            ajustes.definir_statusline(&conta, ligado);
        }
        regra::gravar_ajustes(&dir, &ajustes).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui gravar"))?;
        omni_core::registro::info("ganchos_ajustes", &[("eventos", if ajustes.eventos { "sim" } else { "nao" }), ("ligado", if ajustes.ligado { "sim" } else { "nao" }),
            ("espera_s", &ajustes.espera().to_string()), ("contas_sem_statusline", &ajustes.statusline_desligada.len().to_string())]);
        Ok(Json(ajustes_em_json(&state)))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao gravar"))?
}
