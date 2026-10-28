//! Testes do servidor do celular, um arquivo por assunto (como os módulos de `mobile/`).
//! Aqui ficam só o que é comum: `fixture`, `req`, os endereços padrão e o harness visual.
use super::*;
/// Endereço/origem padrão do celular, da identidade (`MOBILE_PORT`): os testes acompanham a porta.
static HOST_LOCAL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| format!("127.0.0.1:{}", omni_protocol::MOBILE_PORT));
static ORIGEM_LOCAL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| format!("http://{}", *HOST_LOCAL));
use tower::ServiceExt;
use axum::body::{Body,to_bytes};

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock:Mutex::new(()),mobile:MobileRuntime::new(dir.path()),
        token:"test-only".into(),state_file:dir.path().join("sessions.json"),sessions:Mutex::new(HashMap::new()),id_sequence:AtomicU64::new(1) });
    // Fork (segurança): sem token a API recusa tudo; os testes falam como um celular pareado (`req`).
    state.mobile.config.lock().unwrap().token = TOKEN_DE_TESTE.into();
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":"C:/test","title":"Teste","created_at_ms":1,"segments":[]
    }]})).unwrap();
    (dir,state)
}

/// Local visual harness: no real profiles, credentials, sessions or user data.
#[tokio::test]
#[ignore = "Run explicitly while checking the mobile UI in a browser (10 minutes)"]
async fn browser_fixture() {
    let (dir,state) = fixture();
    *state.mobile.config.lock().unwrap() = MobileConfig{enabled:true,bind:"127.0.0.1:47329".into(),token:"fixture-token".into(),serve:false,totp_secret:String::new(),totp_confirmed:false};

    // Conteúdo de mentira, mas com cara de uso real: é para julgar o visual, não a lógica.
    let conta = dir.path().join("claude");
    let transcript = conta.join("projects").join("fixture").join("sessao.jsonl");
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    let linha = |i: usize, role: &str, texto: &str| json!({"uuid":format!("m{i}"),"type":role,"timestamp":format!("2026-09-17T13:0{i}:00Z"),
        "message":{"role":role,"content":texto}}).to_string();
    let mensagens = [
        linha(0,"user","O login está lento no celular. Consegue ver o que está acontecendo?"),
        linha(1,"assistant","Achei a causa. O **token é validado duas vezes** a cada requisição:\n\n1. no middleware de sessão\n2. de novo em `authGuard`\n\n```ts\nexport function authGuard(req: Request) {\n  return validar(req.token); // duplicado\n}\n```\n\n| Etapa | Antes | Depois |\n|---|---|---|\n| Login | 1,8 s | 0,6 s |\n| Refresh | 900 ms | 310 ms |"),
        linha(2,"user","Perfeito. Segue o print do erro que aparece e o log do servidor.\n\n[Anexos enviados pelo celular]\n- C:\\dev\\omni-agents\\.omni-agents\\anexos\\1726000000000-ab12-print-erro.png\n- C:\\dev\\omni-agents\\.omni-agents\\anexos\\1726000000001-cd34-servidor.log"),
        linha(3,"assistant","Vi os dois. O erro do print é o `401` que o segundo `authGuard` devolve quando o token acabou de ser renovado. Removi a validação duplicada e os testes passaram."),
        // Fork (vídeo na conversa): `OMNI_FIXTURE_VIDEO` aponta um .mp4 de verdade para conferir o player.
        linha(4,"assistant",&std::env::var("OMNI_FIXTURE_VIDEO").map(|v| format!("O vídeo ficou pronto: `{v}`")).unwrap_or_else(|_| "Sem vídeo nesta rodada.".into())),
    ];
    // Fork (Tarefas no celular): o "eu" do PC e duas tarefas na pasta padrão.
    let ajustes = json!({"eu": {"id": "enzo", "nome": "Enzo", "cor": "#f97316", "atualizadaEm": ""}, "pastaCompartilhada": ""}).to_string();
    atomic_write_json(&dir.path().join("estado-da-interface.json"), &json!({"chaves": {"medza-tarefas-ajustes": ajustes}})).unwrap();
    let hoje = chrono::Local::now().format("%Y-%m-%d").to_string();
    for (id, titulo, status) in [("t-1", "Gravar o vídeo da Aircook", "a_fazer"), ("t-2", "Revisar contrato da Medza", "fazendo")] {
        let tarefa = json!({"versao": 1, "id": id, "titulo": titulo, "descricao": "", "status": status, "prioridade": "alta", "categoria": "trabalho",
            "responsavel": "enzo", "criadaPor": "enzo", "data": hoje, "criadaEm": "2026-10-23T10:00:00Z", "atualizadaEm": "2026-10-23T10:00:00Z"});
        omni_core::tarefas::gravar(&dir.path().join("tarefas"), "tarefas", id, &tarefa.to_string()).unwrap();
    }
    std::fs::write(&transcript,mensagens.join("\n")).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[
        {"id":"claude-p","provider":"claude","name":"Pessoal","config_dir":conta.to_string_lossy(),"builtin":false,"created_at_ms":0},
        {"id":"codex-p","provider":"codex","name":"Trabalho","config_dir":dir.path().join("codex").to_string_lossy(),"builtin":false,"created_at_ms":0},
    ]})).unwrap();
    let segmento = |provider: &str, perfil: &str, caminho: Option<String>| json!({"provider":provider,"profile_id":perfil,
        "external_session_id":null,"transcript_path":caminho,"terminal_session_id":null,"started_at_ms":1,"ended_at_ms":null});
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[
        {"id":"c","project_id":"p","cwd":"C:/dev/omni-agents","title":"Login lento no celular","created_at_ms":3,
            "segments":[segmento("claude","claude-p",Some(transcript.to_string_lossy().into_owned()))]},
        {"id":"c2","project_id":"p","cwd":"C:/dev/omni-agents","title":"Revisar PR de pagamentos","created_at_ms":2,
            "segments":[segmento("codex","codex-p",None)]},
        {"id":"c3","project_id":"loja","cwd":"C:/dev/loja","title":"Migrar para Next 16","created_at_ms":1,"segments":[]},
    ]})).unwrap();
    publish(&state,
        vec![PublishedProject{id:"p".into(),name:"omni-agents".into(),path:"C:/dev/omni-agents".into()},
             PublishedProject{id:"loja".into(),name:"loja-virtual".into(),path:"C:/dev/loja".into()}],
        vec![PublishedAgent{id:"claude".into(),label:"Claude".into(),command:"claude".into(),resume:Some("--continue".into())},
             PublishedAgent{id:"codex".into(),label:"Codex".into(),command:"codex".into(),resume:None}],
        Some(PublishedTheme{accent:std::env::var("OMNI_FIXTURE_ACCENT").unwrap_or_else(|_|"roxo".into()),
            theme:std::env::var("OMNI_FIXTURE_THEME").unwrap_or_else(|_|"escuro".into()),
            background:"gradient-waves".into(),glass:"forte".into(),
            estilo:std::env::var("OMNI_FIXTURE_ESTILO").unwrap_or_default()}),
    ).unwrap();

    let listener = TcpListener::bind("127.0.0.1:47329").await.unwrap();
    let stop = Arc::new(Notify::new());
    let notify = stop.clone();
    let app = router(state).route("/__test_stop",post(move || { let notify = notify.clone(); async move { notify.notify_one(); StatusCode::OK } }));
    axum::serve(listener,app).with_graceful_shutdown(async move {
        tokio::select! { _ = stop.notified() => {}, _ = tokio::time::sleep(std::time::Duration::from_secs(600)) => {} }
    }).await.unwrap();
}
const TOKEN_DE_TESTE: &str = "token-de-teste";

fn req(method:&str, uri:&str, origin:&str, body:&str) -> Request {
    Request::builder().method(method).uri(uri).header("host",&*HOST_LOCAL).header("origin",origin).header("x-omni-token",TOKEN_DE_TESTE)
        .header("content-type","application/json").header("idempotency-key","test-key").body(Body::from(body.to_owned())).unwrap()
}

mod anexos;
mod celular;
mod conversas;
mod extensoes;
mod fila;
mod rotas;
mod seguranca;
mod sessoes;
mod memoria;
mod sobre_mim; // Fork (Sobre mim).
mod roadmaps; // Fork (Roadmaps de estudo).
mod video; // Vídeo do agente na conversa do celular.
mod tarefas; // Tarefas no celular.
mod terminais;
mod desempenho; // Fork (desempenho): resumo da conversa e ETag pelo IPC.
// Fork (retomar-sempre): rotas /retomada/* e Retomar sem beco sem saída.
#[cfg(unix)]
mod retomada;
// Fork (trocar agente): rota /conversas/{id}/trocar-agente com CLIs falsas.
#[cfg(unix)]
mod trocar_agente;
// Só no Windows (PTY falsa com o `FakeChild`).
#[cfg(windows)]
mod acoes;
