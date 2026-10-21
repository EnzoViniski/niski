use super::*;
use std::path::PathBuf;

#[test]
fn claude_edita_so_na_pasta_e_continua_a_sessao() {
    let (args, entrada) = argumentos("claude", Some("sonnet"), None, "11111111-2222-3333-4444-555555555555", "corta 3 s").unwrap();
    let junto = args.join(" ");
    assert!(junto.starts_with("-p --output-format stream-json --verbose --permission-mode acceptEdits --setting-sources project --strict-mcp-config"));
    assert!(junto.contains("Bash(npx --yes hyperframes@0.8.183 lint:*)"));
    assert!(junto.contains("--disallowedTools WebSearch WebFetch"));
    assert!(junto.contains("--session-id 11111111-2222-3333-4444-555555555555"));
    assert!(!junto.contains("--resume") && !junto.contains("dangerously"));
    assert_eq!(entrada, "corta 3 s");
    let (de_novo, _) = argumentos("claude", None, Some("abcdef12-0000"), "x", "p").unwrap();
    let i = de_novo.iter().position(|a| a == "--resume").unwrap();
    assert_eq!(de_novo[i + 1], "abcdef12-0000");
    assert!(!de_novo.contains(&"--session-id".to_string()));
}

#[test]
fn codex_com_sandbox_da_pasta_e_resume() {
    let (args, entrada) = argumentos("codex", Some("gpt-5.5"), None, "x", "pedido").unwrap();
    let junto = args.join(" ");
    assert!(junto.starts_with("exec --json --skip-git-repo-check -c sandbox_mode=\"workspace-write\""));
    assert!(junto.contains("sandbox_workspace_write.network_access=true"));
    assert!(!junto.contains("danger"));
    assert!(entrada.starts_with("Você está no editor de vídeo") && entrada.ends_with("pedido"));
    let (resume, entrada) = argumentos("codex", None, Some("thread-123456"), "x", "de novo").unwrap();
    assert_eq!(&resume[..3], &["exec", "resume", "thread-123456"]);
    assert_eq!(entrada, "de novo");
}

#[test]
fn recusa_modelo_sessao_e_cli_estranhos() {
    assert!(argumentos("gemini", None, None, "x", "p").is_err());
    assert!(argumentos("claude", Some("--dangerously-skip-permissions"), None, "x", "p").is_err());
    assert!(argumentos("claude", None, Some("../../x; rm"), "x", "p").is_err());
}

#[test]
fn le_eventos_do_claude() {
    let pasta = Path::new("/dados/videos/projetos/v-1");
    let mut edicoes = Vec::new();
    let mut r = Resposta::default();
    let linhas = [
        r#"{"type":"system","subtype":"init","session_id":"s-1","model":"claude-sonnet-5-5"}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Vou cortar."},{"type":"tool_use","id":"t1","name":"Edit","input":{"file_path":"/dados/videos/projetos/v-1/index.html"}}]}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"Bash","input":{"command":"npx --yes hyperframes@0.8.183 lint"}}]}}"#,
        r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"result":"Cortei os 3 primeiros segundos.","session_id":"s-1"}"#,
    ];
    let eventos: Vec<Evento> = linhas.iter().flat_map(|l| eventos_da_linha("claude", l, pasta, &mut edicoes, &mut r)).collect();
    assert_eq!(eventos[0], Evento::Inicio { sessao: Some("s-1".into()), modelo: Some("claude-sonnet-5-5".into()) });
    assert!(eventos.contains(&Evento::Ferramenta { nome: "Edit".into(), detalhe: "index.html".into() }));
    assert!(eventos.contains(&Evento::Ferramenta { nome: "Bash".into(), detalhe: "npx --yes hyperframes@0.8.183 lint".into() }));
    assert!(eventos.contains(&Evento::Arquivo { arquivo: "index.html".into() }));
    assert_eq!(r.texto, "Cortei os 3 primeiros segundos.");
    assert_eq!(r.arquivos, vec!["index.html"]);
    let mut r2 = Resposta::default();
    let erro = eventos_da_linha("claude", r#"{"type":"result","subtype":"error_max_turns","is_error":true,"result":""}"#, pasta, &mut edicoes, &mut r2);
    assert!(matches!(&erro[0], Evento::Erro { .. }));
    assert!(eventos_da_linha("claude", "lixo", pasta, &mut edicoes, &mut r2).is_empty());
}

#[test]
fn le_eventos_do_codex() {
    let pasta = Path::new("/p/v-1");
    let mut edicoes = Vec::new();
    let mut r = Resposta::default();
    let linhas = [
        r#"{"type":"thread.started","thread_id":"th-9"}"#,
        r#"{"type":"item.started","item":{"type":"command_execution","command":"cat BRIEF.md"}}"#,
        r#"{"type":"item.completed","item":{"type":"file_change","changes":[{"path":"/p/v-1/compositions/abertura.html","kind":"add"}]}}"#,
        r#"{"type":"item.completed","item":{"type":"agent_message","text":"Criei a abertura."}}"#,
        r#"{"type":"turn.completed"}"#,
    ];
    let eventos: Vec<Evento> = linhas.iter().flat_map(|l| eventos_da_linha("codex", l, pasta, &mut edicoes, &mut r)).collect();
    assert_eq!(r.sessao.as_deref(), Some("th-9"));
    assert!(eventos.contains(&Evento::Arquivo { arquivo: "compositions/abertura.html".into() }));
    assert_eq!(r.texto, "Criei a abertura.");
    let falhou = eventos_da_linha("codex", r#"{"type":"turn.failed","error":{"message":"sem crédito"}}"#, pasta, &mut edicoes, &mut r);
    assert_eq!(falhou, vec![Evento::Erro { texto: "sem crédito".into() }]);
}

#[cfg(unix)]
fn cli_falsa(dir: &Path, corpo: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let caminho = dir.join("cli-falsa");
    std::fs::write(&caminho, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
    caminho
}

fn pedido(projeto: &str, cli: &str) -> PedidoAoAgente {
    PedidoAoAgente { projeto: projeto.into(), cli: cli.into(), conta: None, modelo: None, sessao: None, texto: "troca a música".into() }
}

#[cfg(unix)]
#[test]
fn conversa_com_cli_falsa_que_edita_o_arquivo() {
    let dir = tempfile::tempdir().unwrap();
    let pasta = dir.path().join("projeto");
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::write(pasta.join("index.html"), "antes").unwrap();
    // Lê o pedido da entrada, edita o arquivo de verdade e imprime os eventos como o Claude.
    let exe = cli_falsa(dir.path(), r#"read pedido
echo "depois: $pedido" > index.html
echo '{"type":"system","subtype":"init","session_id":"s-falsa","model":"modelo-falso"}'
echo '{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Write","input":{"file_path":"'"$PWD"'/index.html"}}]}}'
echo '{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1"}]}}'
echo '{"type":"result","subtype":"success","is_error":false,"result":"Troquei a música.","session_id":"s-falsa"}'"#);
    let mut vistos = Vec::new();
    let r = conversar(&pedido("v-falso-1", "claude"), &exe, &[], &pasta, Duration::from_secs(20), |e| vistos.push(e.clone())).unwrap();
    assert_eq!(r.texto, "Troquei a música.");
    assert_eq!(r.sessao.as_deref(), Some("s-falsa"));
    assert_eq!(r.modelo.as_deref(), Some("modelo-falso"));
    assert_eq!(r.arquivos, vec!["index.html"]);
    assert!(vistos.iter().any(|e| matches!(e, Evento::Arquivo { .. })));
    assert_eq!(std::fs::read_to_string(pasta.join("index.html")).unwrap().trim(), "depois: troca a música");
}

#[cfg(unix)]
#[test]
fn erro_da_cli_vira_mensagem_e_cancelar_para() {
    let dir = tempfile::tempdir().unwrap();
    let exe = cli_falsa(dir.path(), r#"echo '{"type":"turn.failed","error":{"message":"limite da conta"}}'; exit 1"#);
    let erro = conversar(&pedido("v-falso-2", "codex"), &exe, &[], dir.path(), Duration::from_secs(20), |_| {}).unwrap_err();
    assert_eq!(erro, "limite da conta");

    let lenta = cli_falsa(dir.path(), "sleep 30");
    let projeto = "v-falso-3".to_owned();
    let chave_ = chave(&projeto);
    let cancelador = std::thread::spawn(move || {
        for _ in 0..100 { if processo::rodando(&chave_) { break; } std::thread::sleep(Duration::from_millis(20)); }
        std::thread::sleep(Duration::from_millis(100));
        processo::cancelar(&chave_);
    });
    let inicio = Instant::now();
    let erro = conversar(&pedido(&projeto, "claude"), &lenta, &[], dir.path(), Duration::from_secs(20), |_| {}).unwrap_err();
    cancelador.join().unwrap();
    assert_eq!(erro, processo::CANCELADO);
    assert!(inicio.elapsed() < Duration::from_secs(10));
}
