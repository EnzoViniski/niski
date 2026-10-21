//! Fork (Roadmaps de estudo): os mesmos arquivos que o app grava (`omni_core::roadmaps`), para o celular
//! pareado estudar, revisar cards e escrever no caderno. Mesmas regras: lista fechada de nomes, JSON
//! validado, gravação atômica, sem seguir link. Erros respondem só mensagens fixas: o conteúdo nunca
//! volta em erro nem vai para o log.
use super::*;
use omni_core::roadmaps;

/// `cards.json` de anos de estudo cabe com folga (o limite de cada arquivo é 16 MB).
const CORPO_MAXIMO: usize = roadmaps::TAMANHO_MAXIMO + 64 * 1024;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/roadmaps", get(listar))
        .route("/roadmaps/{id}/arquivo", get(ler).post(gravar).layer(DefaultBodyLimit::max(CORPO_MAXIMO)))
        .route("/roadmaps/{id}/conversa", post(anexar).layer(DefaultBodyLimit::max(512 * 1024)))
}

#[derive(Deserialize)]
struct Nome { nome: String }

#[derive(Deserialize)]
struct Gravacao { nome: String, conteudo: String }

#[derive(Deserialize)]
struct Linha { linha: String }

fn raiz(state: &EngineState) -> std::path::PathBuf {
    state.state_file.parent().unwrap_or(std::path::Path::new(".")).join(roadmaps::PASTA)
}

fn falha(erro: std::io::Error) -> ApiError {
    if erro.kind() == std::io::ErrorKind::InvalidInput { error(StatusCode::BAD_REQUEST, &erro.to_string()) }
    else { error(StatusCode::INTERNAL_SERVER_ERROR, "Não foi possível acessar o roadmap.") }
}

async fn fora_do_runtime<T: Send + 'static>(f: impl FnOnce() -> Result<T, ApiError> + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Roadmaps indisponíveis"))?
}

async fn listar(State(state): State<Arc<EngineState>>) -> Result<Json<Value>, ApiError> {
    let raiz = raiz(&state);
    fora_do_runtime(move || Ok(Json(json!(roadmaps::listar(&raiz))))).await
}

async fn ler(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(q): Query<Nome>) -> Result<Json<Value>, ApiError> {
    let raiz = raiz(&state);
    fora_do_runtime(move || roadmaps::ler(&raiz, &id, &q.nome).map(|c| Json(json!({ "conteudo": c }))).map_err(falha)).await
}

async fn gravar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(corpo): Json<Gravacao>) -> Result<Json<Value>, ApiError> {
    let raiz = raiz(&state);
    fora_do_runtime(move || roadmaps::gravar(&raiz, &id, &corpo.nome, &corpo.conteudo).map(|_| Json(json!({ "ok": true }))).map_err(falha)).await
}

async fn anexar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(corpo): Json<Linha>) -> Result<Json<Value>, ApiError> {
    let raiz = raiz(&state);
    fora_do_runtime(move || roadmaps::anexar_na_conversa(&raiz, &id, &corpo.linha).map(|_| Json(json!({ "ok": true }))).map_err(falha)).await
}
