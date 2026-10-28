//! Fork (pergunta-da-cli): telas sintéticas montadas com os textos que a documentação oficial
//! cita (code.claude.com/docs, CHANGELOG do Claude Code; telas do Codex em `codex-rs/tui`, snapshots
//! `*.snap`, commit 464add4 de 2026-10-20). O desenho (numeração, cursor, rodapé) do Claude Code é
//! sintético onde a doc não mostra a tela; nada aqui é conteúdo real de conversa.
use super::*;
use crate::interaction::{approval, escolha};

const BORDA: &str = "────────────────────────────────────────";

fn rotulos(menu: &Escolha) -> Vec<&str> { menu.opcoes.iter().map(|o| o.rotulo.as_str()).collect() }
/// Os dois caminhos juntos, como o `interaction::menu_na_tela`.
fn menu_ou_escolha(tela: &str, provider: &str) -> Option<Escolha> { escolha(tela).or_else(|| menu_no_texto(tela, provider, None)) }

/* ── Claude Code ─────────────────────────────────────────────────────────────────────────────── */

/// Menu do rewind (checkpointing.md): "Summarize from here" é o "resumir" que trava o agente quando
/// o Esc duplo abre o menu. Rodapé sintético sem "Esc to cancel", que o `escolha` exige.
#[test] fn claude_rewind_com_resumir() {
    let tela = format!("● Pronto.\n\n{BORDA}\n Rewind\n\n   1. Restore code and conversation\n   2. Restore conversation\n   3. Restore code\n ❯ 4. Summarize from here\n   5. Never mind\n\n Enter to continue · Esc to exit\n");
    assert!(escolha(&tela).is_none(), "o caminho antigo não via este menu");
    let menu = menu_no_texto(&tela, "claude", None).expect("menu genérico");
    assert!(menu.da_cli);
    assert_eq!(menu.pergunta, "Rewind");
    assert_eq!(rotulos(&menu), ["Restore code and conversation", "Restore conversation", "Restore code", "Summarize from here", "Never mind"]);
    assert_eq!(menu.teclas, None, "Claude responde pelo número");
}

/// Confiar na pasta (2.1.272, já capturada no `interaction`): sem número, o cursor em "No, exit".
#[test] fn claude_confiar_na_pasta_sem_numero() {
    let tela = " Quick safety check: Is this a project you created or one you trust?\n ❯ No, exit\n   Yes, I trust this folder\n Enter to confirm · Esc to cancel";
    let menu = menu_ou_escolha(tela, "claude").expect("menu de confiança");
    assert_eq!(menu.pergunta, "Quick safety check: Is this a project you created or one you trust?");
    assert_eq!(rotulos(&menu), ["No, exit", "Yes, I trust this folder"]);
    assert_eq!(menu.teclas.as_deref(), Some(&["\r".to_owned(), "\x1b[B\r".to_owned()][..]));
    assert_eq!(teclas_da_opcao(&menu, 2), Some((Some("\x1b[B".into()), "\r".into())));
    assert_eq!(teclas_da_opcao(&menu, 1), Some((None, "\r".into())));
}

/// `.mcp.json` novo (channels-reference.md: "New MCP server found in this project: webhook" /
/// "Use this MCP server"). A segunda opção da doc não está citada: rótulo sintético.
#[test] fn claude_mcp_novo_no_projeto() {
    let tela = format!("{BORDA}\n New MCP server found in this project: webhook\n\n ❯ 1. Use this MCP server\n   2. Opção sintética\n\n Enter to confirm · Esc to exit\n");
    let menu = menu_ou_escolha(&tela, "claude").expect("menu do MCP");
    assert_eq!(menu.pergunta, "New MCP server found in this project: webhook");
    assert_eq!(rotulos(&menu), ["Use this MCP server", "Opção sintética"]);
}

/// `/rate-limit-options` (interactive-mode.md): as duas primeiras linhas citadas na doc, sem rodapé.
#[test] fn claude_opcoes_do_limite_de_uso() {
    let tela = format!("{BORDA}\n Usage limit reached · limit resets 3:45pm\n\n ❯ 1. Wait here, then continue automatically\n   2. Don't continue automatically\n");
    let menu = menu_no_texto(&tela, "claude", None).expect("opções do limite");
    assert_eq!(menu.pergunta, "Usage limit reached · limit resets 3:45pm");
    assert_eq!(rotulos(&menu), ["Wait here, then continue automatically", "Don't continue automatically"]);
}

/// Depois de o computador dormir (errors.md): "Your usage limit has reset" / "Press enter to continue".
#[test] fn claude_limite_voltou_pede_enter() {
    let tela = "● Trabalho parado.\n\n Your usage limit has reset\n Press enter to continue\n";
    let menu = menu_no_texto(tela, "claude", None).expect("botão de continuar");
    assert_eq!(menu.pergunta, "Your usage limit has reset");
    assert_eq!(rotulos(&menu), ["Continuar"]);
    assert_eq!(teclas_da_opcao(&menu, 1), Some((None, "\r".into())));
}

/// Espera automática do limite (errors.md): não é menu, mas o agente está parado — vira aviso.
#[test] fn claude_espera_automatica_do_limite_vira_aviso() {
    let tela = "● Parei aqui.\n\n Usage limit reached · limit resets 3:45pm\n Continuing automatically at 3:45pm · esc to cancel\n";
    assert!(menu_ou_escolha(tela, "claude").is_none());
    let espera = espera_no_texto(tela, "claude").expect("aviso de espera");
    assert_eq!(espera.dica.as_deref(), Some("Continuing automatically at 3:45pm · esc to cancel"));
    assert!(espera.texto.contains("Usage limit reached"));
}

/// Pedido de permissão que o `approval` recusa (sem a opção de uso único: permissions.md cita
/// "Yes, and don't ask again for: npm test *"): nunca vira botão, só o aviso com o terminal.
#[test] fn claude_permissao_so_persistente_fica_no_terminal() {
    let tela = format!("{BORDA}\n Bash command\n   npm test\n This command requires approval\n\n Do you want to proceed?\n ❯ 1. Yes, and don't ask again for: npm test *\n   2. No\n\n Esc to cancel · Tab to amend\n");
    assert!(approval(&tela, "claude").is_none());
    assert!(menu_ou_escolha(&tela, "claude").is_none());
    let espera = espera_no_texto(&tela, "claude").expect("aviso de espera");
    assert!(espera.texto.contains("Do you want to proceed?"));
}

/// Menu desenhado em cima do campo (como o "Teach auto mode", tela real do `interaction`), com um
/// rodapé que o `escolha` não conhece.
#[test] fn claude_menu_em_cima_do_campo() {
    let tela = format!("❯ faça algo\n● ok\n{BORDA}\n  Try the new fullscreen renderer?\n\n  ❯ 1. Yes\n    2. No\n\n  Enter to confirm · Esc to exit\n{BORDA}\n❯\n{BORDA}\n  ? for shortcuts\n");
    let menu = menu_no_texto(&tela, "claude", None).expect("menu acima do campo");
    assert_eq!(menu.pergunta, "Try the new fullscreen renderer?");
    assert_eq!(rotulos(&menu), ["Yes", "No"]);
}

/// "(y/n)" de linha com o cursor nela: precisa de Enter; com o cursor escondido, não.
#[test] fn sim_ou_nao_de_linha() {
    let tela = "● Preparando.\n\nContinue? (y/n) ";
    let menu = menu_no_texto(tela, "claude", Some(2)).expect("sim/não");
    assert_eq!(menu.pergunta, "Continue? (y/n)");
    assert_eq!(teclas_da_opcao(&menu, 1), Some((None, "y\r".into())));
    assert_eq!(teclas_da_opcao(&menu, 2), Some((None, "n\r".into())));
    let sem_cursor = menu_no_texto(tela, "claude", None).unwrap();
    assert_eq!(teclas_da_opcao(&sem_cursor, 1), Some((None, "y".into())));
}

/// Codex com `TERM=dumb` pergunta antes de abrir, numa leitura de linha comum, e o cursor vai
/// para a linha de baixo. Sim/Não precisam do Enter, senão o "y" fica escrito e nada acontece.
#[test] fn codex_term_dumb_pede_enter() {
    let tela = "$ codex\nWARNING: TERM is set to \"dumb\". Codex's interactive TUI may not work in this terminal.\nContinue anyway? [y/N]:\n";
    let menu = menu_no_texto(tela, "codex", Some(3)).expect("sim/não");
    assert!(menu.pergunta.starts_with("Continue anyway?"));
    assert_eq!(teclas_da_opcao(&menu, 1), Some((None, "y\r".into())));
    assert_eq!(teclas_da_opcao(&menu, 2), Some((None, "n\r".into())));
}

/* ── Falsos positivos ────────────────────────────────────────────────────────────────────────── */

/// Lista numerada que é saída do agente, com o campo embaixo: CLI ociosa, nada a mostrar.
#[test] fn lista_numerada_com_o_campo_embaixo_nao_e_pergunta() {
    let tela = format!("● Passos:\n  1. Instalar\n  2. Testar\n  3. Publicar\n\nQuer que eu continue?\n{BORDA}\n❯ \n{BORDA}\n  ? for shortcuts\n");
    assert!(menu_no_texto(&tela, "claude", None).is_none());
    assert!(espera_no_texto(&tela, "claude").is_none(), "pergunta do agente com o campo livre não é espera");
    // A mesma lista no pé, sem cursor e sem rodapé de teclas: só texto.
    assert!(menu_no_texto("● Passos:\n  1. Instalar\n  2. Testar\n", "claude", None).is_none());
}

/// Pesquisa da sessão (data-usage.md: "How is Claude doing this session?" … "Dismiss") fica ao lado
/// do campo e não trava o agente; aviso de contexto e login expirado (errors.md) também não são menu.
#[test] fn avisos_ao_lado_do_campo_nao_sao_pergunta() {
    for aviso in [
        "● How is Claude doing this session? (optional)\n  1: Bad   2: Fine   3: Good   0: Dismiss",
        "● Login expired · Please run /login",
        "● Feito.",
    ] {
        let tela = format!("{aviso}\n{BORDA}\n❯ \n{BORDA}\n  Context left until auto-compact: 8%\n");
        assert!(menu_no_texto(&tela, "claude", None).is_none(), "{aviso}");
        assert!(espera_no_texto(&tela, "claude").is_none(), "{aviso}");
    }
}

/// No meio do turno ("esc to interrupt"), nada é pergunta; o Antigravity tem leitura própria.
#[test] fn turno_em_andamento_e_antigravity_ficam_de_fora() {
    let tela = "  1. Yes\n❯ 2. No\nEnter to confirm\n✻ Pensando… (12s · esc to interrupt)\n";
    assert!(menu_no_texto(tela, "claude", None).is_none());
    assert!(espera_no_texto(tela, "claude").is_none());
    assert!(menu_no_texto("❯ 1. Yes\n  2. No\nEnter to confirm\n", "antigravity", None).is_none());
}

/// Prompt antigo com várias linhas no histórico não vira menu sem número.
#[test] fn prompt_antigo_de_varias_linhas_nao_e_menu() {
    let tela = format!("❯ primeira linha\n  segunda linha\n\n● Resposta.\n{BORDA}\n❯ \n{BORDA}\n");
    assert!(menu_no_texto(&tela, "claude", None).is_none());
}

/* ── Codex (textos dos snapshots de `codex-rs/tui`) ──────────────────────────────────────────── */

/// onboarding/trust_directory.rs: "Folder access" … "› 1. Trust and continue" / "2. Quit" e
/// "enter continue · esc quit". Codex anda com as setas a partir do `›`.
#[test] fn codex_confiar_na_pasta() {
    let tela = "  Folder access\n\n  /Users/ana/projeto\n\n  Trust this folder? Codex can read, edit, and run files here, subject to your permission settings.\n\n› 1. Trust and continue\n  2. Quit\n\n  enter continue · esc quit\n";
    assert!(escolha(tela).is_none());
    let menu = menu_no_texto(tela, "codex", None).expect("confiança do Codex");
    assert_eq!(menu.pergunta, "Folder access");
    assert!(menu.detalhe.as_deref().unwrap().contains("Trust this folder?"));
    assert_eq!(rotulos(&menu), ["Trust and continue", "Quit"]);
    assert_eq!(teclas_da_opcao(&menu, 1), Some((None, "\r".into())));
    assert_eq!(teclas_da_opcao(&menu, 2), Some((Some("\x1b[B".into()), "\r".into())));
}

/// update_prompt.rs: "Update available · 0.0.0 → 9.9.9" … "enter continue · esc skip".
#[test] fn codex_atualizacao_disponivel() {
    let tela = "  Update available · 0.0.0 → 9.9.9\n\n  Release notes: https://github.com/openai/codex/releases/latest\n\n› 1. Update now (runs `npm install -g @openai/codex`)\n  2. Skip\n  3. Skip until next version\n\n  enter continue · esc skip\n";
    let menu = menu_no_texto(tela, "codex", None).expect("atualização");
    assert_eq!(menu.pergunta, "Update available · 0.0.0 → 9.9.9");
    assert_eq!(rotulos(&menu), ["Update now (runs `npm install -g @openai/codex`)", "Skip", "Skip until next version"]);
    assert_eq!(teclas_da_opcao(&menu, 3), Some((Some("\x1b[B\x1b[B".into()), "\r".into())));
}

/// model_migration.rs: "Codex just got an upgrade. Introducing {model}." / "enter/esc confirm".
#[test] fn codex_migracao_de_modelo() {
    let tela = "  Codex just got an upgrade. Introducing modelo-x.\n\n› 1. Try new model\n  2. Use existing model\n\n  enter/esc confirm · ⌃c quit\n";
    let menu = menu_no_texto(tela, "codex", None).expect("migração");
    assert_eq!(rotulos(&menu), ["Try new model", "Use existing model"]);
}

/// startup_hooks_review.rs: "Hooks need review" / "enter confirm · esc skip".
#[test] fn codex_revisar_ganchos() {
    let tela = "  Hooks need review\n  2 hooks are new or changed.\n\n› 1. Review hooks\n  2. Trust all and continue\n  3. Continue without trusting (hooks won't run)\n\n  enter confirm · esc skip\n";
    let menu = menu_no_texto(tela, "codex", None).expect("ganchos");
    assert_eq!(menu.pergunta, "Hooks need review");
    assert_eq!(menu.detalhe.as_deref(), Some("2 hooks are new or changed."));
    assert_eq!(rotulos(&menu).len(), 3);
}

/// permission_popups.rs: "Enable full access?" / "enter select · esc back".
#[test] fn codex_acesso_total() {
    let tela = "  Enable full access?\n\n› 1. Yes, continue anyway\n  2. Cancel\n\n  enter select · esc back\n";
    let menu = menu_no_texto(tela, "codex", None).expect("acesso total");
    assert_eq!(menu.pergunta, "Enable full access?");
    assert_eq!(rotulos(&menu), ["Yes, continue anyway", "Cancel"]);
}

/// rate_limits.rs: "Approaching rate limits" / "Switch to {model} for lower credit usage?".
#[test] fn codex_perto_do_limite() {
    let tela = "  Approaching rate limits\n  Switch to modelo-mini for lower credit usage?\n\n› 1. Switch to modelo-mini\n  2. Keep current model\n  3. Keep current model (never show again)\n\n  Press enter to confirm or esc to go back\n";
    let menu = menu_no_texto(tela, "codex", None).expect("limite");
    assert_eq!(menu.pergunta, "Approaching rate limits");
    assert_eq!(rotulos(&menu).len(), 3);
}

/// request_user_input: o rodapé diz "esc to interrupt", mas junto com "enter to submit answer".
#[test] fn codex_pergunta_do_agente_com_esc_to_interrupt() {
    let tela = "  Question 1/1 (1 unanswered)\n  Qual banco usar?\n\n› 1. Postgres\n  2. SQLite\n  3. None of the above\n\n  tab to add notes | enter to submit answer | esc to interrupt\n";
    let menu = menu_no_texto(tela, "codex", None).expect("pergunta do Codex");
    assert_eq!(menu.pergunta, "Qual banco usar?");
    assert_eq!(rotulos(&menu), ["Postgres", "SQLite", "None of the above"]);
}

/// approval_overlay.rs: rede pergunta "Do you want to approve network access to …?" e só tem opções
/// que guardam permissão além da de uma vez — fica no terminal, como aviso.
#[test] fn codex_acesso_a_rede_fica_no_terminal() {
    let tela = "  Do you want to approve network access to \"example.com\"?\n\n› 1. Yes, just this once (y)\n  2. Yes, and allow this host for this conversation (a)\n  3. No, and tell Codex what to do differently (esc)\n\n  Press enter to confirm or esc to cancel\n";
    assert!(menu_ou_escolha(tela, "codex").is_none());
    assert!(espera_no_texto(tela, "codex").is_some());
}

/// "You’ve hit your usage limit." (protocol/src/error.rs) é erro com o campo embaixo: não é menu.
#[test] fn codex_limite_atingido_nao_e_menu() {
    let tela = "■ You’ve hit your usage limit. Try again later.\n\n› Ask Codex to do anything\n\n  ⏎ send   ⌃J newline   ⌃T transcript   ⌃C quit\n";
    assert!(menu_no_texto(tela, "codex", None).is_none());
    assert!(espera_no_texto(tela, "codex").is_none());
}

/// Tela viva: o `vt100` com o cursor escondido e o `menu` lendo linha a linha.
#[test] fn tela_viva_do_vt100() {
    let mut parser = vt100::Parser::new(12, 80, 0);
    parser.process("\x1b[?25l  Enable full access?\r\n\r\n› 1. Yes, continue anyway\r\n  2. Cancel\r\n\r\n  enter select · esc back".as_bytes());
    let menu = menu(parser.screen(), "codex").expect("menu na tela viva");
    assert_eq!(rotulos(&menu), ["Yes, continue anyway", "Cancel"]);
    assert!(espera(parser.screen(), "codex", 500).is_none(), "tela recém-desenhada não vira aviso");
}

/* ── Codex 0.164: telas dos snapshots oficiais (tag rust-v0.164.0, `codex-rs/tui/src/bottom_pane`) ── */

/// Formulário de aprovação de ferramenta do MCP (`mcp_server_elicitation_approval_form_with_session_persist.snap`),
/// com histórico em cima: a pergunta era a linha velha do histórico (a caixa subia 20 linhas sem borda).
#[test] fn codex_formulario_do_mcp_mostra_a_pergunta_certa() {
    let tela = "• Rodei o preview do vídeo.\n\n› gere o vídeo de novo\n\n• Chamando a ferramenta do MCP hyperframes.\n\n  Field 1/1\n  Allow this request?\n  › 1. Allow                   Run the tool and continue\n    2. Allow for this session  Run the tool and remember this choice for this session\n    3. Always allow            Run the tool and remember this choice for future tool calls\n    4. Cancel                  Cancel this tool call\n\n\n  enter to submit | esc to cancel\n";
    let menu = menu_ou_escolha(tela, "codex").expect("formulário do MCP");
    assert_eq!(menu.pergunta, "Allow this request?");
    assert_eq!(rotulos(&menu), ["Allow", "Allow for this session", "Always allow", "Cancel"]);
    assert_eq!(menu.opcoes[1].descricao.as_deref(), Some("Run the tool and remember this choice for this session"));
}

/// Acesso à rede (`network_exec_prompt.snap`): quatro opções; antes ficava "responda no terminal".
#[test] fn codex_acesso_a_rede_vira_cartao_com_as_quatro_opcoes() {
    let tela = "  Do you want to approve network access to \"example.com\"?\n\n  Reason: network request blocked\n\n\n\n› 1. Yes, just this once (y)\n  2. Yes, and allow this host for this conversation (a)\n  3. Yes, and allow this host in the future (p)\n  4. No, and tell Codex what to do differently (esc)\n\n  Press enter to confirm or esc to cancel\n";
    let (sim, nao, _) = approval(tela, "codex").expect("vira o cartão de permissão");
    assert_eq!((sim.as_str(), nao.as_str()), ("y", "\x1b"));
    let opcoes = crate::opcoes_da_permissao::opcoes(tela, "codex");
    assert_eq!(opcoes.iter().map(|o| o.papel).collect::<Vec<_>>(), ["permitir", "outra", "outra", "negar"]);
    assert_eq!(opcoes[2].setas.as_deref(), Some("\x1b[B\x1b[B"));
}

/// Pergunta do agente no Plan mode (`request_user_input_options.snap`) com o histórico em cima.
#[test] fn codex_pergunta_do_plan_mode_da_tag_0_164() {
    let tela = "› me pergunte qual opção usar\n\n• Vou perguntar.\n\n  Question 1/1 (1 unanswered)\n  Choose an option.\n\n  › 1. Option 1           First choice.\n    2. Option 2           Second choice.\n    3. Option 3           Third choice.\n    4. None of the above  Optionally, add details in notes (tab)\n\n  tab to add notes | enter to submit answer | esc to interrupt\n";
    let menu = menu_ou_escolha(tela, "codex").expect("pergunta do Codex");
    assert_eq!(menu.pergunta, "Choose an option.");
    assert_eq!(rotulos(&menu), ["Option 1", "Option 2", "Option 3", "None of the above"]);
    assert_eq!(teclas_da_opcao(&menu, 3), Some((Some("\x1b[B\x1b[B".into()), "\r".into())));
}
