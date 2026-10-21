//! Fork (trocar agente): testes da passagem de bastão com transcripts sintéticos (nenhuma CLI real).
use super::*;
use serde_json::json;

fn escrever(caminho: &Path, linhas: &[Value]) {
    std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
    std::fs::write(caminho, linhas.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n").unwrap();
}

/// Conversa do Claude: pedido, leitura, edição, comando com saída, lista de tarefas e resposta.
fn claude(caminho: &Path) {
    escrever(caminho, &[
        json!({"type":"user","uuid":"u1","timestamp":"2026-10-21T10:00:00Z","message":{"role":"user","content":"Corrige o login lento no celular. Vamos usar cache de sessão em vez de validar o token duas vezes."}}),
        json!({"type":"assistant","uuid":"a1","timestamp":"2026-10-21T10:00:05Z","message":{"role":"assistant","content":[
            {"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"/repo/src/auth.ts"}}]}}),
        json!({"type":"assistant","uuid":"a2","timestamp":"2026-10-21T10:00:06Z","message":{"role":"assistant","content":[
            {"type":"tool_use","id":"t2","name":"Edit","input":{"file_path":"/repo/src/auth.ts","old_string":"a","new_string":"b"}}]}}),
        json!({"type":"assistant","uuid":"a3","timestamp":"2026-10-21T10:00:07Z","message":{"role":"assistant","content":[
            {"type":"tool_use","id":"t3","name":"Bash","input":{"command":"npm test -- auth"}}]}}),
        json!({"type":"user","uuid":"r3","timestamp":"2026-10-21T10:00:20Z","message":{"role":"user","content":[
            {"type":"tool_result","tool_use_id":"t3","content":"PASS src/auth.test.ts\n12 passed","is_error":false}]}}),
        json!({"type":"assistant","uuid":"a4","timestamp":"2026-10-21T10:00:21Z","message":{"role":"assistant","content":[
            {"type":"tool_use","id":"t4","name":"TodoWrite","input":{"todos":[
                {"content":"Remover validação duplicada","status":"completed","activeForm":"Removendo"},
                {"content":"Medir o tempo do login no celular","status":"pending","activeForm":"Medindo"}]}}]}}),
        json!({"type":"assistant","uuid":"a5","timestamp":"2026-10-21T10:00:30Z","message":{"role":"assistant","content":[
            {"type":"text","text":"Pronto: tirei a validação duplicada em `authGuard`. Os testes passaram. Falta medir no celular."}]}}),
    ]);
}

/// Conversa do Codex: pedido, comando que falha, patch e resposta.
fn codex(caminho: &Path) {
    escrever(caminho, &[
        json!({"type":"session_meta","timestamp":"2026-10-21T11:00:00Z","payload":{"id":"cx-1","cwd":"/repo"}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:01Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>cwd</environment_context>"}]}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:02Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Adiciona a tela de perfil"}]}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:03Z","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"cargo build\"}","call_id":"c1"}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:09Z","payload":{"type":"function_call_output","call_id":"c1","output":"Process exited with code 101\nOutput:\nerror[E0425]: cannot find value `perfil`"}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:10Z","payload":{"type":"custom_tool_call","name":"apply_patch","call_id":"c2",
            "input":"*** Begin Patch\n*** Add File: src/perfil.rs\n+pub fn perfil() {}\n*** End Patch"}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:20Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Criei `src/perfil.rs`. O build ainda falha no `main.rs`."}]}}),
    ]);
}

fn pedido<'a>(fontes: &'a [Fonte], git: Option<&'a EstadoGit>, para: &'a str, teto: usize) -> Pedido<'a> {
    Pedido { produto: "Medza Agents", conversa_id: "conv-teste", titulo: "Login lento", cwd: "/repo", para, fontes, git, gerado_em: "21/10/2026 12:00", teto }
}

#[test]
fn claude_para_codex_leva_tudo_que_importa() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("claude.jsonl");
    claude(&transcript);
    let fontes = [
        Fonte { provider: "claude".into(), rotulo: rotulo("claude", Some("Pessoal")), caminho: Some(transcript) },
        Fonte { provider: "codex".into(), rotulo: rotulo("codex", Some("Trabalho")), caminho: None },
    ];
    let git = EstadoGit { raiz: "/repo".into(), branch: "feat/login".into(), worktree: true, status: vec![" M src/auth.ts".into()], commits: vec!["abc123 base".into()] };
    let md = gerar(&pedido(&fontes, Some(&git), "Codex · conta Trabalho", TETO_PADRAO));
    // Objetivo e mensagem do usuário na íntegra.
    assert!(md.contains("> Corrige o login lento no celular. Vamos usar cache de sessão em vez de validar o token duas vezes."), "{md}");
    // Onde parou: a última resposta inteira.
    assert!(md.contains("Última resposta do agente anterior:\n\n> Pronto: tirei a validação duplicada"), "{md}");
    // Arquivos, comandos com resultado, tarefas, decisões, git.
    assert!(md.contains("- `/repo/src/auth.ts`"), "{md}");
    assert!(md.contains("- `npm test -- auth` → ok: PASS src/auth.test.ts ⏎ 12 passed"), "{md}");
    assert!(md.contains("- [x] Remover validação duplicada") && md.contains("- [ ] Medir o tempo do login no celular"), "{md}");
    assert!(md.contains("(usuário) Vamos usar cache de sessão em vez de validar o token duas vezes"), "{md}");
    assert!(md.contains("Branch: `feat/login` (worktree)") && md.contains(" M src/auth.ts"), "{md}");
    assert!(md.contains("Agentes até aqui: Claude · conta Pessoal → Codex · conta Trabalho"), "{md}");
    assert!(md.contains("### Trecho 1 — Claude · conta Pessoal") && md.contains("- Read: `/repo/src/auth.ts`"), "{md}");
    assert!(md.contains("Agora continua com: **Codex · conta Trabalho**"));
    assert!(md.contains("raciocínio interno"), "limite dito no topo");
}

#[test]
fn codex_para_claude_le_o_rollout_e_marca_a_falha() {
    let dir = tempfile::tempdir().unwrap();
    let rollout = dir.path().join("rollout.jsonl");
    codex(&rollout);
    let fontes = [Fonte { provider: "codex".into(), rotulo: rotulo("codex", None), caminho: Some(rollout) }];
    let md = gerar(&pedido(&fontes, None, "Claude · conta Pessoal", TETO_PADRAO));
    assert!(md.contains("> Adiciona a tela de perfil"), "{md}");
    assert!(!md.contains("environment_context"), "contexto injetado pela CLI fica de fora: {md}");
    assert!(md.contains("- `cargo build` → falhou: error[E0425]: cannot find value `perfil`"), "{md}");
    assert!(md.contains("- `src/perfil.rs`"), "{md}");
    assert!(md.contains("O build ainda falha no `main.rs`"), "{md}");
    assert!(md.contains("_(a pasta não é um repositório git)_"));
}

#[test]
fn conversa_longa_corta_o_antigo_e_mantem_o_recente_e_todas_as_falas_do_usuario() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("longa.jsonl");
    let mut linhas = Vec::new();
    for i in 0..60 {
        linhas.push(json!({"type":"user","uuid":format!("u{i}"),"timestamp":"2026-10-21T10:00:00Z","message":{"role":"user","content":format!("pedido {i}: ajuste o módulo {i}")}}));
        linhas.push(json!({"type":"assistant","uuid":format!("a{i}"),"timestamp":"2026-10-21T10:00:01Z","message":{"role":"assistant","content":[
            {"type":"text","text":format!("resposta {i} {}", "detalhe ".repeat(800))}]}}));
    }
    escrever(&transcript, &linhas);
    let fontes = [Fonte { provider: "claude".into(), rotulo: "Claude".into(), caminho: Some(transcript) }];
    let teto = 40_000;
    let md = gerar(&pedido(&fontes, None, "Codex", teto));
    assert!(md.chars().count() <= teto + 200, "respeita o teto: {}", md.chars().count());
    for i in [0, 30, 59] { assert!(md.contains(&format!("pedido {i}: ajuste o módulo {i}")), "pedido {i} do usuário fica: falta"); }
    assert!(md.contains("resposta 59 "), "a última resposta fica");
    assert!(md.contains("_(resposta antiga omitida para caber)_") || md.contains("caracteres omitidos"), "o antigo é cortado");
}

#[test]
fn troca_de_conta_copia_o_transcript_e_nada_repete() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.jsonl");
    claude(&a);
    let b = dir.path().join("b.jsonl");
    std::fs::copy(&a, &b).unwrap();
    let fontes = [
        Fonte { provider: "claude".into(), rotulo: rotulo("claude", Some("A")), caminho: Some(a) },
        Fonte { provider: "claude".into(), rotulo: rotulo("claude", Some("B")), caminho: Some(b) },
    ];
    let md = gerar(&pedido(&fontes, None, "Codex", TETO_PADRAO));
    assert_eq!(md.matches("**Usuário**").count(), 1, "{md}");
}

#[test]
fn prompt_curto_e_marca_que_some_da_conversa() {
    let sem = prompt_curto("Claude · conta Pessoal", "Codex · conta Trabalho", ".omni-agents/passagens/conv-1-2.md", None);
    assert!(sem.starts_with(MARCA) && sem.contains("leia o arquivo .omni-agents/passagens/conv-1-2.md") && sem.contains("espere o próximo pedido"));
    assert!(sem.contains("Não refaça o que já foi feito"));
    assert_eq!(sem_preambulo(&sem), "", "sem mensagem do usuário, nada aparece");
    let com = prompt_curto("Claude", "Codex", "x.md", Some("  agora mede no celular  "));
    assert!(com.contains("atenda o pedido do usuário"));
    assert_eq!(sem_preambulo(&com), "agora mede no celular");
    assert_eq!(sem_preambulo("texto normal"), "texto normal");
    // Pela leitura da conversa (`feed::sem_colado`), inclusive colado pela CLI.
    let colado = format!("<pasted_content id=\"1\">\n{com}\n</pasted_content id=\"1\">");
    assert_eq!(crate::feed::sem_colado(&colado), "agora mede no celular");
    // Nenhum caractere de controle além de quebra de linha: vai pela mesma porta do chat.
    assert!(!com.chars().any(|c| c.is_control() && c != '\n'));
}

#[test]
fn gravar_na_pasta_do_projeto_e_fora_do_git_status() {
    let dir = tempfile::tempdir().unwrap();
    let ok = Command::new("git").args(["init", "-q"]).current_dir(dir.path()).status().is_ok_and(|s| s.success());
    let caminho = gravar(dir.path(), "conv-abc", 2, "# oi\n").unwrap();
    assert!(caminho.ends_with(".omni-agents/passagens/conv-abc-2.md") && caminho.is_file());
    assert_eq!(caminho_relativo("conv-abc", 2), ".omni-agents/passagens/conv-abc-2.md");
    if ok {
        let exclude = std::fs::read_to_string(dir.path().join(".git/info/exclude")).unwrap();
        assert_eq!(exclude.matches(".omni-agents/").count(), 1);
        gravar(dir.path(), "conv-abc", 3, "# de novo\n").unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join(".git/info/exclude")).unwrap().matches(".omni-agents/").count(), 1, "não repete");
        let git = estado_git(dir.path()).expect("é repositório");
        assert!(git.status.is_empty(), "a passagem não aparece no status: {:?}", git.status);
    }
    assert!(gravar(dir.path(), "../fora", 1, "x").is_err());
}

#[test]
#[ignore = "imprime um exemplo do arquivo (cargo test -p omni-core exemplo_impresso -- --ignored --nocapture)"]
fn exemplo_impresso() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("claude.jsonl"), dir.path().join("rollout.jsonl"));
    claude(&a);
    codex(&b);
    let fontes = [
        Fonte { provider: "claude".into(), rotulo: rotulo("claude", Some("Pessoal")), caminho: Some(a) },
        Fonte { provider: "codex".into(), rotulo: rotulo("codex", Some("Trabalho")), caminho: Some(b) },
    ];
    let git = EstadoGit { raiz: "/repo".into(), branch: "feat/login".into(), worktree: false, status: vec![" M src/auth.ts".into(), "?? src/perfil.rs".into()], commits: vec!["abc123 base".into()] };
    println!("{}", gerar(&pedido(&fontes, Some(&git), "Claude · conta Pessoal", TETO_PADRAO)));
}

/// A conversa na tela depois da troca: Claude e depois Codex em sequência, o separador "Continuou com
/// o Codex · conta Trabalho" no ponto da troca, e a marca da passagem fora; a mensagem que o usuário
/// já tinha escrito aparece como dele.
#[test]
fn timeline_com_dois_agentes_mostra_o_separador_e_esconde_a_passagem() {
    let dir = tempfile::tempdir().unwrap();
    let (conta_claude, conta_codex) = (dir.path().join("claude"), dir.path().join("codex"));
    let transcript = conta_claude.join("projects").join("repo").join("s1.jsonl");
    escrever(&transcript, &[
        json!({"type":"user","uuid":"u1","timestamp":"2026-10-21T10:00:00Z","message":{"role":"user","content":"corrige o login"}}),
        json!({"type":"assistant","uuid":"a1","timestamp":"2026-10-21T10:00:05Z","message":{"role":"assistant","content":[{"type":"text","text":"corrigido"}]}}),
    ]);
    let rollout = conta_codex.join("sessions").join("2026").join("10").join("21").join("rollout-cx.jsonl");
    let passagem = prompt_curto("Claude · conta Pessoal", "Codex · conta Trabalho", ".omni-agents/passagens/c-2.md", Some("agora mede no celular"));
    escrever(&rollout, &[
        json!({"type":"session_meta","timestamp":"2026-10-21T11:00:00Z","payload":{"id":"cx","cwd":"/repo"}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:01Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":passagem}]}}),
        json!({"type":"response_item","timestamp":"2026-10-21T11:00:09Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"medido: 0,6 s"}]}}),
    ]);
    let perfis: Vec<crate::Profile> = serde_json::from_value(json!([
        {"id":"p","provider":"claude","name":"Pessoal","config_dir":conta_claude.to_string_lossy(),"created_at_ms":0},
        {"id":"q","provider":"codex","name":"Trabalho","config_dir":conta_codex.to_string_lossy(),"created_at_ms":0}])).unwrap();
    let ms = |t: &str| chrono::DateTime::parse_from_rfc3339(t).unwrap().timestamp_millis() as u64;
    let segmento = |provider: &str, perfil: &str, caminho: &Path, inicio: u64| Segment { provider: provider.into(), profile_id: Some(perfil.into()),
        external_session_id: None, transcript_path: Some(caminho.to_string_lossy().into_owned()), terminal_session_id: Some("t1".into()),
        started_at_ms: inicio, ended_at_ms: None };
    let conversa = Conversation { id: "c".into(), project_id: "p".into(), cwd: "/repo".into(), title: "Conversa".into(),
        title_source: Default::default(), created_at_ms: 0,
        segments: vec![segmento("claude", "p", &transcript, ms("2026-10-21T10:00:00Z")), segmento("codex", "q", &rollout, ms("2026-10-21T10:59:59Z"))] };
    let textos: Vec<String> = crate::conversations::timeline(&conversa, &perfis, None, 50).messages.into_iter().map(|m| m.text).collect();
    assert_eq!(textos, ["corrige o login", "corrigido", "Continuou com o Codex · conta Trabalho", "agora mede no celular", "medido: 0,6 s"]);
    // O nome da conversa continua vindo do primeiro pedido.
    assert_eq!(crate::conversations::auto_title(&conversa).as_deref(), Some("corrige o login"));
    // E a próxima passagem lê os dois trechos.
    let md = gerar(&pedido(&fontes(&conversa, &perfis), None, "Claude · conta Pessoal", TETO_PADRAO));
    assert!(md.contains("Agentes até aqui: Claude · conta Pessoal → Codex · conta Trabalho") && md.contains("> agora mede no celular") && !md.contains(MARCA), "{md}");
}
