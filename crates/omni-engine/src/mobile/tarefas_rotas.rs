//! Fork (Tarefas no celular): a tela de Tarefas do celular lê e grava os mesmos arquivos do PC.
//!
//! No app do PC a tela fala com os comandos do Tauri (`src-tauri/src/tarefas.rs`); o celular só alcança
//! o engine. A pasta não vem do pedido: é a do PC, lida do espelho da interface (`omni_core::tarefas::contexto`,
//! a padrão ou a compartilhada escolhida em Configurações). Gravar usa a mesma regra (tipo e id
//! conferidos, temporário + rename, 1 MB) do `omni_core::tarefas::gravar`.
use super::*;
use omni_core::tarefas as armazenamento;

/// Uma tarefa tem no máximo 1 MB (`armazenamento::gravar`); o JSON do pedido leva o texto escapado.
const CORPO_MAXIMO: usize = 3 * 1024 * 1024;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/tarefas/contexto", get(contexto))
        .route("/tarefas/listar/{tipo}", get(listar))
        .route("/tarefas/gravar", post(gravar).layer(DefaultBodyLimit::max(CORPO_MAXIMO)))
}

fn dir(state: &EngineState) -> PathBuf { state.state_file.parent().map(Path::to_path_buf).unwrap_or_default() }

fn erro_de_io(erro: std::io::Error) -> ApiError {
    if erro.kind() == std::io::ErrorKind::InvalidInput { error(StatusCode::BAD_REQUEST, &erro.to_string()) }
    else { error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui acessar as tarefas no computador") }
}

/// Quem é o "eu" do PC (para a tarefa criada no celular ter dono) e se a pasta é compartilhada.
async fn contexto(State(state): State<Arc<EngineState>>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let ajustes = std::fs::read_to_string(dir(&state).join("estado-da-interface.json")).ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("chaves")?.get("medza-tarefas-ajustes")?.as_str().map(str::to_owned))
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .unwrap_or(Value::Null);
        let compartilhada = ajustes.get("pastaCompartilhada").and_then(Value::as_str).is_some_and(|p| !p.trim().is_empty());
        Ok(Json(json!({"eu": ajustes.get("eu").cloned().unwrap_or(Value::Null), "compartilhada": compartilhada})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler as tarefas"))?
}

async fn listar(State(state): State<Arc<EngineState>>, RoutePath(tipo): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let contexto = armazenamento::contexto(&dir(&state)).map_err(erro_de_io)?;
        let itens = armazenamento::listar(&contexto.raiz, &tipo).map_err(erro_de_io)?;
        Ok(Json(json!({"itens": itens})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler as tarefas"))?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gravacao { tipo: String, id: String, conteudo: String }

async fn gravar(State(state): State<Arc<EngineState>>, Json(corpo): Json<Gravacao>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        // O conteúdo tem de ser JSON (é o que a tela e o copiloto leem); texto qualquer não entra na pasta.
        serde_json::from_str::<Value>(&corpo.conteudo).map_err(|_| error(StatusCode::BAD_REQUEST, "Tarefa inválida"))?;
        let contexto = armazenamento::contexto(&dir(&state)).map_err(erro_de_io)?;
        armazenamento::gravar(&contexto.raiz, &corpo.tipo, &corpo.id, &corpo.conteudo).map_err(erro_de_io)?;
        Ok(Json(json!({"ok": true})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao gravar a tarefa"))?
}

