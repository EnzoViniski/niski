//! Fork (Google Agenda): login, sincronização e criação contra um Google falso em 127.0.0.1.
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

const ID: &str = "123-sintetico.apps.googleusercontent.com";
const SEGREDO: &str = "segredo-sintetico-do-cliente";
const REFRESH: &str = "rt-sintetico-nao-usar";

#[derive(Default)]
struct Falso {
    formularios: Vec<HashMap<String, String>>,
    criados: Vec<Value>,
    recusar_refresh: bool,
}
type Google = Arc<Mutex<Falso>>;

fn campos(corpo: &str) -> HashMap<String, String> {
    corpo.split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (decodificar(k), decodificar(v))).collect()
}

async fn google_falso() -> (Google, String) {
    let falso: Google = Arc::default();
    let token = {
        let f = falso.clone();
        move |corpo: String| {
            let f = f.clone();
            async move {
                let c = campos(&corpo);
                let mut g = f.lock().unwrap();
                g.formularios.push(c.clone());
                let resposta = match c.get("grant_type").map(String::as_str) {
                    Some("authorization_code") if c.get("code").map(String::as_str) == Some("codigo-certo") && c.get("client_secret").map(String::as_str) == Some(SEGREDO) =>
                        (StatusCode::OK, json!({"access_token": "at-1", "refresh_token": REFRESH, "expires_in": 3600})),
                    Some("refresh_token") if !g.recusar_refresh => (StatusCode::OK, json!({"access_token": "at-2", "expires_in": 3600})),
                    _ => (StatusCode::BAD_REQUEST, json!({"error": "invalid_grant", "error_description": "segredo citado aqui"})),
                };
                (resposta.0, Json(resposta.1))
            }
        }
    };
    let listar = |headers: HeaderMap| async move {
        if !headers.get("authorization").and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("Bearer at-")) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        (StatusCode::OK, Json(json!({"summary": "eu@gmail.com", "items": [
            {"id": "g1", "summary": "Consulta", "start": {"date": "2026-11-03"}, "end": {"date": "2026-11-04"}},
            {"id": "g2", "status": "cancelled", "start": {"date": "2026-11-03"}},
        ]})))
    };
    let criar = {
        let f = falso.clone();
        move |Json(corpo): Json<Value>| {
            let f = f.clone();
            async move { f.lock().unwrap().criados.push(corpo); Json(json!({"id": "novo1"})) }
        }
    };
    let app = Router::new()
        .route("/token", post(token))
        .route("/revoke", post(|| async { StatusCode::OK }))
        .route("/calendars/primary/events", get(listar).post(criar));
    let ouvinte = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", ouvinte.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(ouvinte, app).await.unwrap() });
    (falso, base)
}

fn apontar(state: &EngineState, base: &str) {
    *runtime(state).enderecos.lock().unwrap() = Enderecos {
        autorizar: format!("{base}/auth"), token: format!("{base}/token"), api: base.into(), revogar: format!("{base}/revoke"),
    };
}

async fn pedir(state: &Arc<EngineState>, metodo: &str, uri: &str, corpo: Value, desktop: bool) -> (StatusCode, Value) {
    let mut app = rotas().with_state(state.clone());
    if desktop {
        app = app.layer(axum::Extension(orquestracao::jarvis::Desktop));
    }
    let req = axum::http::Request::builder().method(metodo).uri(uri).header("content-type", "application/json")
        .body(axum::body::Body::from(if metodo == "GET" { String::new() } else { corpo.to_string() })).unwrap();
    let resposta = app.oneshot(req).await.unwrap();
    let status = resposta.status();
    let bytes = axum::body::to_bytes(resposta.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn parametro(url: &str, nome: &str) -> String {
    let consulta = url.split_once('?').unwrap().1;
    campos(consulta).remove(nome).unwrap_or_default()
}

fn conectado_com_token_vencido(state: &EngineState) {
    salvar_credenciais(&dir(state), &Credenciais { client_id: ID.into(), client_secret: SEGREDO.into(), refresh_token: REFRESH.into(),
        access_token: "at-velho".into(), expira_ms: 0 }).unwrap();
}

#[tokio::test]
async fn login_no_navegador_guarda_a_credencial_e_traz_os_compromissos() {
    let (_dir, state) = fixture();
    let (falso, base) = google_falso().await;
    apontar(&state, &base);
    let (s, v) = pedir(&state, "POST", "/rotina/google/conectar", json!({"client_id": ID, "client_secret": SEGREDO}), true).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let url = v["url"].as_str().unwrap();
    assert!(url.starts_with(&format!("{base}/auth?")));
    assert_eq!(parametro(url, "scope"), ESCOPO);
    assert_eq!(parametro(url, "code_challenge_method"), "S256");
    assert_eq!(parametro(url, "access_type"), "offline");
    assert!(v["conectando"] == true && !v.to_string().contains(SEGREDO));
    let (redirecionar, estado) = (parametro(url, "redirect_uri"), parametro(url, "state"));
    assert!(redirecionar.starts_with("http://127.0.0.1:"));

    // O navegador volta com o estado errado: não conecta.
    let navegador = cliente().unwrap();
    let pagina = navegador.get(format!("{redirecionar}/?code=codigo-certo&state=outro")).send().await.unwrap().text().await.unwrap();
    assert!(pagina.contains("Pedido inválido"));
    assert!(!conectado(&state));
    // Agora o certo.
    let pagina = navegador.get(format!("{redirecionar}/?code=codigo-certo&state={estado}&scope=x")).send().await.unwrap().text().await.unwrap();
    assert!(pagina.contains("Google Agenda conectado"), "{pagina}");
    let mut v = Value::Null;
    for _ in 0..50 {
        v = pedir(&state, "GET", "/rotina/google", Value::Null, true).await.1;
        if v["eventos"].as_u64() == Some(1) { break; }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!((v["conectado"].as_bool(), v["conectando"].as_bool(), v["conta"].as_str(), v["eventos"].as_u64()),
        (Some(true), Some(false), Some("eu@gmail.com"), Some(1)), "{v}");
    let troca = falso.lock().unwrap().formularios[0].clone();
    assert_eq!((troca.get("redirect_uri"), troca["code_verifier"].len()), (Some(&redirecionar), 64));
    assert_eq!(desafio_pkce(&troca["code_verifier"]), parametro(url, "code_challenge"));

    let texto = v.to_string();
    assert!(!texto.contains(REFRESH) && !texto.contains(SEGREDO) && !texto.contains("at-1"), "status não vaza credencial");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(caminho(&dir(&state))).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let a = omni_core::rotina::operacoes::Agora { ms: 0, hoje: chrono::NaiveDate::from_ymd_opt(2026, 11, 3).unwrap(),
        hora: chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap() };
    let dia = omni_core::rotina::operacoes::executar(&dir(&state), "ler_dia", &json!({}), &a).unwrap();
    assert!(dia["sem_hora"].as_array().unwrap().iter().any(|i| i["id"] == "google:g1" && i["tipo"] == "compromisso"));
}

#[tokio::test]
async fn criar_renova_o_token_manda_hora_local_e_ressincroniza() {
    let (_dir, state) = fixture();
    let (falso, base) = google_falso().await;
    apontar(&state, &base);
    conectado_com_token_vencido(&state);
    let (s, v) = pedir(&state, "POST", "/rotina/google/evento", json!({"titulo": "Prova de cálculo", "data": "2026-11-05", "inicio": "19:00",
        "local": "Bloco B"}), false).await;
    assert_eq!((s, v["id"].as_str()), (StatusCode::OK, Some("novo1")), "{v}");
    let corpo = falso.lock().unwrap().criados[0].clone();
    assert_eq!((corpo["summary"].as_str(), corpo["location"].as_str()), (Some("Prova de cálculo"), Some("Bloco B")));
    assert!(corpo["start"]["dateTime"].as_str().unwrap().starts_with("2026-11-05T19:00:00"));
    assert!(corpo["end"]["dateTime"].as_str().unwrap().starts_with("2026-11-05T20:00:00"), "1 hora por padrão");
    assert_eq!(ler_credenciais(&dir(&state)).access_token, "at-2", "token renovado e guardado");
    let agenda: AgendaDoGoogle = rotina_core::ler(&dir(&state));
    assert_eq!(agenda.eventos.len(), 1, "sincronizou depois de criar");

    let (s, _) = pedir(&state, "POST", "/rotina/google/evento", json!({"titulo": "Dia todo", "data": "2026-11-06"}), false).await;
    assert_eq!(s, StatusCode::OK);
    let corpo = falso.lock().unwrap().criados[1].clone();
    assert_eq!((corpo["start"]["date"].as_str(), corpo["end"]["date"].as_str()), (Some("2026-11-06"), Some("2026-11-07")));
    let (s, v) = pedir(&state, "POST", "/rotina/google/evento", json!({"titulo": "x", "data": "2026-11-06", "inicio": "10:00", "fim": "09:00"}), false).await;
    assert_eq!((s, v["error"].as_str()), (StatusCode::BAD_REQUEST, Some("O fim precisa ser depois do início.")));
}

#[tokio::test]
async fn acesso_revogado_vira_aviso_curto_e_mantem_os_compromissos() {
    let (_dir, state) = fixture();
    let (falso, base) = google_falso().await;
    apontar(&state, &base);
    conectado_com_token_vencido(&state);
    assert_eq!(sincronizar(&state).await, Ok(1));
    falso.lock().unwrap().recusar_refresh = true;
    let mut c = ler_credenciais(&dir(&state));
    c.expira_ms = 0;
    salvar_credenciais(&dir(&state), &c).unwrap();
    let erro = sincronizar(&state).await.unwrap_err();
    assert!(erro.contains("conectar de novo") && !erro.contains("segredo citado"), "{erro}");
    let v = pedir(&state, "GET", "/rotina/google", Value::Null, true).await.1;
    assert_eq!((v["eventos"].as_u64(), v["erro"].as_str()), (Some(1), Some(erro.as_str())));
}

#[tokio::test]
async fn celular_nao_conecta_nem_desconecta_e_desconectar_apaga_tudo() {
    let (_dir, state) = fixture();
    let (_falso, base) = google_falso().await;
    apontar(&state, &base);
    let (s, _) = pedir(&state, "POST", "/rotina/google/conectar", json!({"client_id": ID, "client_secret": SEGREDO}), false).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = pedir(&state, "GET", "/rotina/google", Value::Null, false).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, v) = pedir(&state, "POST", "/rotina/google/conectar", json!({"client_id": "qualquer", "client_secret": SEGREDO}), true).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");

    conectado_com_token_vencido(&state);
    sincronizar(&state).await.unwrap();
    let (s, v) = pedir(&state, "POST", "/rotina/google/desconectar", json!({}), true).await;
    assert_eq!((s, v["conectado"].as_bool(), v["eventos"].as_u64()), (StatusCode::OK, Some(false), Some(0)));
    assert!(ler_credenciais(&dir(&state)).refresh_token.is_empty());
}

#[test]
fn le_os_parametros_da_volta_do_navegador() {
    let p = parametros("GET /?code=4%2F0Ab-c&state=xyz&scope=a+b HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
    assert_eq!((p["code"].as_str(), p["state"].as_str(), p["scope"].as_str()), ("4/0Ab-c", "xyz", "a b"));
    assert!(parametros("GET /favicon.ico HTTP/1.1\r\n\r\n").is_none());
    assert_eq!(decodificar("%E2%9C%93%"), "✓%");
    assert_eq!(codificar("a b/ç"), "a%20b%2F%C3%A7");
}

#[test]
fn recusa_do_google_diz_o_que_fazer_sem_repetir_a_mensagem() {
    let api_desligada = json!({"error": {"code": 403, "message": "Google Calendar API has not been used in project 123",
        "errors": [{"reason": "accessNotConfigured"}], "status": "PERMISSION_DENIED", "details": [{"reason": "SERVICE_DISABLED"}]}});
    let texto = recusado(403, &api_desligada);
    assert!(texto.contains("Calendar API não está ativada") && !texto.contains("123"));
    let sem_escopo = json!({"error": {"errors": [{"reason": "insufficientPermissions"}], "details": [{"reason": "ACCESS_TOKEN_SCOPE_INSUFFICIENT"}]}});
    assert!(recusado(403, &sem_escopo).contains("permissão da agenda"));
    assert!(recusado(401, &Value::Null).contains("conectar de novo"));
    assert!(recusado(403, &Value::Null).contains("recusou"));
}
