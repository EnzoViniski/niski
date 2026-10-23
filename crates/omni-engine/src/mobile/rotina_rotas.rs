//! Fork (Rotina): rotas dos dados da rotina e o relógio dos lembretes.
//!
//! - `GET /rotina/ler/{nome}?campo=valor`: operações de leitura (`ler_dia`, `ler_semana`…).
//! - `POST /rotina/op/{nome}` com os argumentos em JSON: qualquer operação (`omni_core::rotina::operacoes`).
//!
//! Servem o desktop (`EngineRequest::Api`) e o celular pareado. Erros são as mensagens curtas das
//! operações; nada do conteúdo vai para o log.
//!
//! O relógio roda no engine a cada 30 s, com a janela fechada: calcula os lembretes vencidos no fuso
//! local (DST e "sem duplicar" no `omni_core::rotina::lembretes`) e grava em `lembretes-estado.json`.
//! A tela lê os avisos de lá; o Telegram da rotina (`rotina_telegram.rs`) também.
use super::*;
use omni_core::rotina::{self as rotina_core, lembretes, operacoes};

const CORPO_MAXIMO: usize = 256 * 1024;

/// O relógio local (fuso do sistema, com horário de verão).
pub(super) fn agora() -> operacoes::Agora {
    let n = chrono::Local::now();
    let hora = chrono::NaiveTime::from_hms_opt(chrono::Timelike::hour(&n), chrono::Timelike::minute(&n), 0).unwrap_or_default();
    operacoes::Agora { ms: n.timestamp_millis(), hoje: n.date_naive(), hora }
}

fn dir(state: &EngineState) -> std::path::PathBuf {
    state.state_file.parent().unwrap().to_path_buf()
}

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/rotina/ler/{nome}", get(ler))
        .route("/rotina/op/{nome}", post(operar).layer(DefaultBodyLimit::max(CORPO_MAXIMO)))
}

async fn rodar(state: Arc<EngineState>, nome: String, args: Value) -> Result<Json<Value>, ApiError> {
    if !operacoes::existe(&nome) {
        return Err(error(StatusCode::NOT_FOUND, "Operação da rotina desconhecida"));
    }
    tokio::task::spawn_blocking(move || {
        operacoes::executar(&dir(&state), &nome, &args, &agora()).map(Json).map_err(|e| error(StatusCode::BAD_REQUEST, &e))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Rotina indisponível"))?
}

async fn ler(State(state): State<Arc<EngineState>>, RoutePath(nome): RoutePath<String>, Query(campos): Query<HashMap<String, String>>)
    -> Result<Json<Value>, ApiError> {
    if !operacoes::so_le(&nome) {
        return Err(error(StatusCode::METHOD_NOT_ALLOWED, "Use POST para mudar a rotina"));
    }
    let args = Value::Object(campos.into_iter().map(|(k, v)| (k, Value::String(v))).collect());
    rodar(state, nome, args).await
}

async fn operar(State(state): State<Arc<EngineState>>, RoutePath(nome): RoutePath<String>, corpo: Option<Json<Value>>)
    -> Result<Json<Value>, ApiError> {
    rodar(state, nome, corpo.map(|Json(v)| v).unwrap_or_else(operacoes::sem_argumentos)).await
}

/* ── Relógio dos lembretes ─────────────────────────────────────────────────────────────────── */

/// Um tique: grava os lembretes vencidos (cada um uma vez). Devolve os que saíram agora.
pub(super) fn tique<T: chrono::TimeZone>(dados: &Path, agora_ms: i64, fuso: &T) -> Vec<lembretes::Disparo> {
    let _trava = rotina_core::travar();
    let (lembretes_, plano, alimentacao, treino) = (rotina_core::ler::<lembretes::Lembretes>(dados), rotina_core::ler(dados),
        rotina_core::ler(dados), rotina_core::ler(dados));
    let mut estado: lembretes::Estado = rotina_core::ler(dados);
    let tarefas = omni_core::tarefas::contexto(dados).map(|c| omni_core::tarefas::todas(&c.raiz)).unwrap_or_default();
    let fontes = lembretes::Fontes { lembretes: &lembretes_, plano: &plano, alimentacao: &alimentacao, treino: &treino, tarefas: &tarefas };
    let vencidos = lembretes::vencidos(&fontes, &estado, agora_ms, fuso);
    if vencidos.is_empty() {
        return vencidos;
    }
    estado.registrar(&vencidos, agora_ms);
    if rotina_core::gravar(dados, &estado).is_err() {
        registro::aviso("rotina_lembretes_nao_gravou", &[]);
        return Vec::new(); // Sem gravar, não avisa: senão sairia de novo no próximo tique.
    }
    for d in &vencidos {
        registro::info("rotina_lembrete", &[("tipo", &d.tipo)]);
    }
    vencidos
}

pub(super) fn iniciar(state: Arc<EngineState>) {
    tokio::spawn(async move {
        let mut intervalo = tokio::time::interval(std::time::Duration::from_secs(30));
        intervalo.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            intervalo.tick().await;
            let dados = dir(&state);
            let _ = tokio::task::spawn_blocking(move || tique(&dados, chrono::Utc::now().timestamp_millis(), &chrono::Local)).await;
        }
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use tower::ServiceExt;

    fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(EngineState {
            persist_lock: Mutex::new(()),
            mobile: MobileRuntime::new(dir.path()),
            token: "fixture".into(),
            state_file: dir.path().join("sessions.json"),
            sessions: Mutex::new(HashMap::new()),
            id_sequence: AtomicU64::new(1),
        });
        (dir, state)
    }

    async fn pedir(app: &Router, metodo: &str, uri: &str, corpo: Option<Value>) -> (StatusCode, Value) {
        let req = axum::http::Request::builder().method(metodo).uri(uri).header("content-type", "application/json")
            .body(axum::body::Body::from(corpo.map(|c| c.to_string()).unwrap_or_default())).unwrap();
        let resposta = app.clone().oneshot(req).await.unwrap();
        let status = resposta.status();
        let bytes = axum::body::to_bytes(resposta.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    #[tokio::test]
    async fn rotas_leem_e_gravam_pela_mesma_operacao() {
        let (_dir, state) = fixture();
        let app = rotas().with_state(state);
        let (s, v) = pedir(&app, "POST", "/rotina/op/registrar_agua", Some(json!({"copos": 2}))).await;
        assert_eq!((s, v["agua_ml"].as_u64()), (StatusCode::OK, Some(500)));
        let (s, v) = pedir(&app, "GET", "/rotina/ler/ler_dia", None).await;
        assert_eq!((s, v["agua"]["ml"].as_u64()), (StatusCode::OK, Some(500)));
        let (s, _) = pedir(&app, "GET", "/rotina/ler/registrar_agua", None).await;
        assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED, "GET não muda nada");
        let (s, _) = pedir(&app, "POST", "/rotina/op/nao_existe", Some(json!({}))).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        let (s, v) = pedir(&app, "POST", "/rotina/op/salvar_bloco", Some(json!({"titulo": "x"}))).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert!(v["error"].as_str().unwrap().contains("inicio"));
        let (s, v) = pedir(&app, "GET", "/rotina/ler/ler_semana?semana=2026-10-26", None).await;
        assert_eq!((s, v["segunda"].as_str()), (StatusCode::OK, Some("2026-10-26")));
    }

    #[test]
    fn tique_grava_uma_vez_e_sobrevive_ao_reinicio() {
        let dir = tempfile::tempdir().unwrap();
        let utc = chrono::FixedOffset::east_opt(0).unwrap();
        let hoje = chrono::NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let a = operacoes::Agora { ms: 0, hoje, hora: chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap() };
        operacoes::executar(dir.path(), "salvar_lembretes", &json!({"agua": {"ativo": false}, "refeicoes": false, "planejar": {"ativo": false}}), &a).unwrap();
        operacoes::executar(dir.path(), "salvar_lembrete", &json!({"texto": "Tomar creatina", "hora": "09:00", "tipo": "remedio"}), &a).unwrap();
        let nove = hoje.and_hms_opt(9, 0, 30).unwrap().and_utc().timestamp_millis();
        let saiu = tique(dir.path(), nove, &utc);
        assert_eq!(saiu.len(), 1);
        assert_eq!(saiu[0].texto, "Tomar creatina");
        assert!(tique(dir.path(), nove + 30_000, &utc).is_empty(), "o estado em disco impede repetir");
        let avisos = operacoes::executar(dir.path(), "ler_avisos", &json!({}), &a).unwrap();
        assert_eq!(avisos["nao_vistos"], 1);
    }
}
