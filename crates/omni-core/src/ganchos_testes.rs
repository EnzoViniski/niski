//! Fork: testes da parte pura dos ganchos (JSON gerado, flag idempotente, configurações do usuário,
//! eventos sintéticos, statusline). Nada aqui roda CLI.

use super::*;

fn sem_usuario() -> ConfigDoUsuario { ConfigDoUsuario::default() }

#[test]
fn settings_tem_os_eventos_os_prazos_e_nenhum_token() {
    let settings = montar_settings(47_425, 300, &sem_usuario(), None);
    let hooks = settings["hooks"].as_object().unwrap();
    for evento in EVENTOS_OBSERVADOS {
        let gancho = &hooks[evento][0]["hooks"][0];
        assert_eq!(gancho["type"], "http", "{evento}");
        assert_eq!(gancho["url"], "http://127.0.0.1:47425/ganchos", "{evento}");
        assert_eq!(gancho["timeout"], if evento == "SessionEnd" { 2 } else { 5 }, "{evento}");
        assert_eq!(gancho["headers"]["Authorization"], "Bearer ${MEDZA_GANCHO_TOKEN}");
        assert_eq!(gancho["allowedEnvVars"], json!(["MEDZA_GANCHO_TOKEN"]));
    }
    assert_eq!(hooks["PreToolUse"][0]["matcher"], "*");
    assert!(hooks.get("PostToolUse").is_none(), "PostToolUse traz o conteúdo dos arquivos: fica de fora");
    assert!(hooks.get("SessionStart").is_none(), "SessionStart não aceita http");
    let permissao = &hooks["PermissionRequest"][0];
    assert_eq!(permissao["matcher"], "*");
    assert_eq!(permissao["hooks"][0]["url"], "http://127.0.0.1:47425/ganchos/permissao");
    assert_eq!(permissao["hooks"][0]["timeout"], 310, "prazo do engine + 10 s");
    assert_eq!(permissao["hooks"][0]["statusMessage"], STATUS_DA_ESPERA);
    // Sem allowlist do usuário, nada de restringir os ganchos http dele.
    assert!(settings.get("allowedHttpHookUrls").is_none() && settings.get("httpHookAllowedEnvVars").is_none());
    assert!(settings.get("statusLine").is_none());
    assert!(settings.get("disableAllHooks").is_none(), "nunca reativamos o que o usuário desligou");
    let texto = settings.to_string();
    assert!(!texto.contains("engine.token"));
}

#[test]
fn allowlist_so_entra_quando_alguem_ja_definiu() {
    let config = ConfigDoUsuario { limita_urls: true, ..sem_usuario() };
    let settings = montar_settings(47_445, 120, &config, None);
    assert_eq!(settings["allowedHttpHookUrls"], json!(["http://127.0.0.1:47445/*"]));
    assert!(settings.get("httpHookAllowedEnvVars").is_none());
    let config = ConfigDoUsuario { limita_variaveis: true, ..sem_usuario() };
    let settings = montar_settings(47_445, 120, &config, None);
    assert_eq!(settings["httpHookAllowedEnvVars"], json!(["MEDZA_GANCHO_TOKEN"]));
    assert!(settings.get("allowedHttpHookUrls").is_none());
}

#[test]
fn statusline_copia_a_aparencia_do_usuario() {
    let config = ConfigDoUsuario { statusline: Some(json!({"type": "command", "command": "~/linha.sh", "padding": 2,
        "refreshInterval": 5, "hideVimModeIndicator": true, "outra": 1})), ..sem_usuario() };
    let settings = montar_settings(1, 300, &config, Some("'/x/omni-shim' --medza-statusline 1"));
    assert_eq!(settings["statusLine"], json!({"type": "command", "command": "'/x/omni-shim' --medza-statusline 1",
        "padding": 2, "refreshInterval": 5, "hideVimModeIndicator": true}));
    assert_eq!(statusline_do_usuario(&config).as_deref(), Some("~/linha.sh"));
    assert_eq!(statusline_do_usuario(&ConfigDoUsuario { statusline: Some(json!({"type": "outra", "command": "x"})), ..sem_usuario() }), None);
    #[cfg(not(windows))]
    {
        assert_eq!(comando_da_statusline("/Applications/Medza Agents.app/omni-shim", 47425).as_deref(),
            Some("'/Applications/Medza Agents.app/omni-shim' --medza-statusline 47425"));
        assert_eq!(comando_da_statusline("/a'b/omni-shim", 1), None);
    }
}

#[test]
fn flag_settings_e_idempotente_e_so_mexe_na_nossa() {
    let arquivo = Path::new("/dados/engine/ganchos/terminal-1.json");
    let um = com_settings("claude --resume abc", arquivo).unwrap();
    assert_eq!(um, "claude --resume abc --settings '/dados/engine/ganchos/terminal-1.json'");
    assert_eq!(com_settings(&um, arquivo).unwrap(), um, "idempotente");
    assert_eq!(sem_settings(&um), "claude --resume abc");
    // Com o MCP do orquestrador antes, e um --settings do próprio usuário que não é nosso.
    let do_usuario = "claude --settings '/home/eu/minha.json' --mcp-config '{\"mcpServers\":{}}'";
    let com = com_settings(do_usuario, arquivo).unwrap();
    assert!(com.starts_with(do_usuario));
    assert_eq!(sem_settings(&com), do_usuario);
    assert!(com_settings("claude", Path::new("/a'b/ganchos/t.json")).is_none(), "aspa simples quebraria a linha");
    assert!(!um.contains("MEDZA_GANCHO_TOKEN"), "o token nunca vai na linha digitada");
}

#[test]
fn configuracoes_do_usuario_leem_so_as_chaves_que_importam() {
    let dir = tempfile::tempdir().unwrap();
    let conta = dir.path().join("conta");
    let projeto = dir.path().join("projeto");
    std::fs::create_dir_all(conta.as_path()).unwrap();
    std::fs::create_dir_all(projeto.join(".claude")).unwrap();
    let mut arquivos = Arquivos::de(&conta, &projeto);
    arquivos.gerenciado = dir.path().join("gerenciado.json");
    assert_eq!(ler_config_do_usuario(&arquivos), ConfigDoUsuario::default());

    std::fs::write(conta.join("settings.json"), r#"{"statusLine":{"type":"command","command":"usuario.sh"},"allowedHttpHookUrls":["https://x/*"]}"#).unwrap();
    std::fs::write(projeto.join(".claude/settings.json"), r#"{"statusLine":{"type":"command","command":"projeto.sh"}}"#).unwrap();
    let config = ler_config_do_usuario(&arquivos);
    assert!(config.limita_urls && !config.limita_variaveis && !config.ganchos_desligados);
    assert_eq!(statusline_do_usuario(&config).as_deref(), Some("projeto.sh"), "projeto vence usuário");
    std::fs::write(projeto.join(".claude/settings.local.json"), r#"{"statusLine":{"type":"command","command":"local.sh"},"disableAllHooks":true}"#).unwrap();
    let config = ler_config_do_usuario(&arquivos);
    assert_eq!(statusline_do_usuario(&config).as_deref(), Some("local.sh"), "local vence projeto");
    assert!(config.ganchos_desligados);

    std::fs::write(&arquivos.gerenciado, r#"{"allowManagedHooksOnly":true,"httpHookAllowedEnvVars":[],"statusLine":{}}"#).unwrap();
    let config = ler_config_do_usuario(&arquivos);
    assert!(config.so_ganchos_gerenciados && config.limita_variaveis && config.statusline_gerenciada);
    // Arquivo quebrado não conta.
    std::fs::write(conta.join("settings.json"), "{ não é json").unwrap();
    assert!(!ler_config_do_usuario(&arquivos).limita_urls);
}

#[test]
fn ajustes_tem_padrao_ligado_e_prazo_valido() {
    let dir = tempfile::tempdir().unwrap();
    let padrao = ler_ajustes(dir.path());
    assert!(!padrao.ligado && padrao.statusline.is_empty());
    assert_eq!(padrao.espera(), 300);
    gravar_ajustes(dir.path(), &Ajustes { ligado: false, espera_s: 600, statusline: vec!["claude-p".into()] }).unwrap();
    let lido = ler_ajustes(dir.path());
    assert!(!lido.ligado && lido.espera() == 600 && lido.statusline == ["claude-p"]);
    assert_eq!(Ajustes { espera_s: 7, ..Ajustes::default() }.espera(), 300);
    std::fs::write(dir.path().join(ARQUIVO_DE_AJUSTES), r#"{"espera_s":120}"#).unwrap();
    assert!(!ler_ajustes(dir.path()).ligado, "campo ausente = desligado");
}

#[test]
fn versao_minima_dos_ganchos_http() {
    assert_eq!(versao_de("2.1.304 (Claude Code)"), Some((2, 1, 304)));
    assert_eq!(versao_de("claude 2.1.63"), Some((2, 1, 63)));
    assert_eq!(versao_de("sem versão"), None);
    assert!(suporta_ganchos_http((2, 1, 63)) && suporta_ganchos_http((2, 2, 0)) && suporta_ganchos_http((3, 0, 0)));
    assert!(!suporta_ganchos_http((2, 1, 62)) && !suporta_ganchos_http((1, 9, 999)));
}

#[test]
fn eventos_sinteticos_de_cada_tipo() {
    let comum = |nome: &str| json!({"hook_event_name": nome, "session_id": "s1", "transcript_path": "/t/s1.jsonl",
        "cwd": "/p", "permission_mode": "default", "effort": {"level": "high"}});
    let e = Evento::de(&comum("UserPromptSubmit"), 200).unwrap();
    assert_eq!((e.nome.as_str(), e.session_id.as_deref(), e.esforco.as_deref()), ("UserPromptSubmit", Some("s1"), Some("high")));

    let mut pre = comum("PreToolUse");
    pre["tool_name"] = json!("Bash");
    pre["tool_input"] = json!({"command": format!("npm test {}", "x".repeat(400)), "description": "roda"});
    let e = Evento::de(&pre, 200).unwrap();
    assert!(e.resumo.as_deref().unwrap().starts_with("Bash · npm test"));
    assert_eq!(e.resumo.unwrap().chars().count(), 200);

    let mut pedido = comum("PermissionRequest");
    pedido["tool_name"] = json!("mcp__medza__criar_agente");
    pedido["tool_input"] = json!({"tarefa": "x"});
    pedido["mcp_server"] = json!({"name": "medza"});
    let e = Evento::de(&pedido, 2000).unwrap();
    assert_eq!(e.servidor_mcp.as_deref(), Some("medza"));
    assert_eq!(e.resumo.as_deref(), Some("mcp__medza__criar_agente · x"));

    let mut aviso = comum("Notification");
    aviso["notification_type"] = json!("idle_prompt");
    aviso["message"] = json!("texto que não guardamos");
    let e = Evento::de(&aviso, 200).unwrap();
    assert_eq!(tipo(&e.nome, e.notification_type.as_deref(), None), Some("precisa_de_voce"));
    assert_eq!(tipo("Notification", Some("auth_success"), None), None);

    let mut falha = comum("StopFailure");
    falha["error"] = json!("rate_limit");
    let e = Evento::de(&falha, 200).unwrap();
    assert_eq!(tipo(&e.nome, None, e.erro.as_deref()), Some("limite"));
    assert_eq!(aviso_da_falha(e.erro.as_deref()), "usage_limit");
    assert_eq!(tipo("StopFailure", None, Some("overloaded")), Some("erro"));
    assert_eq!(aviso_da_falha(Some("authentication_failed")), "api_error");
    assert_eq!(tipo("Stop", None, None), Some("terminou"));
    assert_eq!(tipo("PermissionRequest", None, None), Some("precisa_de_voce"));
    assert_eq!(tipo("PreToolUse", None, None), None);

    let mut sub = comum("SubagentStart");
    sub["agent_id"] = json!("a1");
    sub["agent_type"] = json!("Explore");
    let e = Evento::de(&sub, 200).unwrap();
    assert_eq!((e.agent_id.as_deref(), e.agent_type.as_deref()), (Some("a1"), Some("Explore")));
    let mut compactar = comum("PreCompact");
    compactar["trigger"] = json!("manual");
    assert_eq!(Evento::de(&compactar, 200).unwrap().gatilho.as_deref(), Some("manual"));

    // Tolerante: campo de tipo estranho vira None; sem nome do evento, não é evento.
    let estranho = json!({"hook_event_name": "Stop", "session_id": 42, "error": {"type": "overloaded"}});
    let e = Evento::de(&estranho, 200).unwrap();
    assert_eq!((e.session_id, e.erro.as_deref()), (None, Some("overloaded")));
    assert!(Evento::de(&json!({"session_id": "s"}), 200).is_none());
    assert!(Evento::de(&json!([1, 2]), 200).is_none());
}

#[test]
fn statusline_vira_medicao_e_uso_da_conta() {
    let entrada = json!({"session_id": "s", "model": {"id": "claude-x", "display_name": "Opus"}, "version": "2.1.304",
        "cost": {"total_cost_usd": 0.82, "total_lines_added": 10, "total_lines_removed": 3},
        "context_window": {"used_percentage": 34.5, "context_window_size": 200000}, "fast_mode": false,
        "effort": {"level": "medium"},
        "rate_limits": {"five_hour": {"used_percentage": 12.0, "resets_at": 2_000_000_000_i64}, "seven_day": {"used_percentage": 40, "resets_at": 2_100_000_000_i64}}});
    let m = medicao(&entrada, 1_000);
    assert_eq!(m.modelo.as_deref(), Some("Opus"));
    assert_eq!((m.contexto_pct, m.contexto_tamanho, m.custo_usd), (Some(34.5), Some(200_000), Some(0.82)));
    assert_eq!((m.linhas_mais, m.linhas_menos, m.fast), (Some(10), Some(3), Some(false)));
    let uso = m.uso("claude-p").unwrap();
    assert_eq!((uso.source.as_str(), uso.status.as_str(), uso.observed_at_ms), ("claude_statusline", "available", Some(1_000)));
    let cinco = uso.primary.unwrap();
    assert_eq!((cinco.used_percent, cinco.window_minutes, cinco.resets_at), (12.0, 300, Some(2_000_000_000)));
    assert_eq!(uso.secondary.unwrap().window_minutes, 10080);

    // Sem `rate_limits` (conta sem Pro/Max, ou antes da 1ª resposta): sem uso, o resto vale.
    let sem = medicao(&json!({"context_window": {"used_percentage": 120}}), 5);
    assert!(sem.uso("p").is_none());
    assert_eq!(sem.contexto_pct, None, "porcentagem fora de 0–100 não vale");
    // Janela vencida vira "stale".
    let velha = medicao(&json!({"rate_limits": {"five_hour": {"used_percentage": 1, "resets_at": 10}}}), 20_000);
    assert_eq!(velha.uso("p").unwrap().status, "stale");
}
