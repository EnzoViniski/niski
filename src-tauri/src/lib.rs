mod conversations;
mod account_usage;
mod agent_runtime;
mod mobile;
mod engine_client;
mod git_client;
mod docker_client;
mod ssh;
mod target_files;
mod history;
mod instalacao;
mod profiles;
mod extensoes;
mod relatorio;
mod agentes;
mod fluxos;
mod voz;
mod atualizacao;
// Fork: abrir no app padrão o arquivo que o agente gerou (cartão na Conversa).
mod abrir_entregavel;
// Fork: caminho citado na Conversa vira link (abre pasta/arquivo sem executar nada).
mod abrir_caminho;
// Fork: aba Navegador e trava de link que tomava a janela.
mod navegador;
// Fork: agente numa branch própria (worktree do git).
mod worktree;
#[cfg(target_os = "macos")]
mod tela_cheia;

pub fn run() {
    // Fork: contas com pasta mas fora do `profiles.json` desta variante voltam a ser reconhecidas.
    if let Ok(raiz) = profiles::raiz_de_perfis() { omni_core::definir_raiz_de_perfis(raiz); }
    if let Ok(dir) = engine_client::engine_dir() {
        omni_core::registro::iniciar("app", &dir);
    }
    omni_core::registro::registrar_panicos();
    atualizacao::marcar_abertura();
    let inicio = std::time::Instant::now();
    #[cfg(unix)]
    let path_do_shell = engine_client::import_login_shell_path();
    #[cfg(not(unix))]
    let path_do_shell = "nao_se_aplica";
    // Antes de qualquer detecção ou spawn: o engine e os terminais herdam este PATH.
    omni_core::cli_path::ampliar_path();
    registrar_inicio(path_do_shell, inicio.elapsed().as_millis());
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(navegador::plugin())
        .plugin(tauri_plugin_notification::init())
        // Fork: em tela cheia no Mac, o conteúdo desce junto da barra revelada (tela_cheia.rs).
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            tela_cheia::instalar(_app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_client::engine_status,
            abrir_entregavel::abrir_entregavel,
            abrir_entregavel::mostrar_entregavel_na_pasta,
            abrir_caminho::abrir_caminho_da_conversa,
            navegador::navegador_abrir,
            navegador::navegador_posicionar,
            navegador::navegador_acao,
            navegador::navegador_fechar,
            navegador::navegador_abrir_fora,
            worktree::git_worktree_da_branch,
            voz::voz_estado,
            voz::voz_baixar_modelo,
            voz::voz_gravar,
            voz::voz_transcrever,
            voz::voz_previa,
            voz::voz_cancelar,
            atualizacao::versao_nova_no_disco,
            atualizacao::reiniciar_para_atualizar,
            engine_client::registrar_evento_da_ui,
            account_usage::account_usage,
            extensoes::listar_extensoes,
            extensoes::compartilhamento_de_extensoes,
            extensoes::definir_compartilhamento_de_extensoes,
            extensoes::definir_conta_principal,
            agent_runtime::agent_runtime,
            mobile::mobile_settings,
            mobile::publish_workspace,
            instalacao::install_hint,
            instalacao::install_agent_cli,
            mobile::mobile_check,
            mobile::mobile_totp,
            engine_client::ensure_engine,
            engine_client::terminal_sessions,
            engine_client::agent_cli_statuses,
            engine_client::connect_agent_cli,
            engine_client::ensure_agent_trust,
            engine_client::spawn_terminal,
            engine_client::write_terminal,
            engine_client::resize_terminal,
            engine_client::stop_terminal,
            engine_client::close_terminal,
            engine_client::duplicate_terminal,
            engine_client::restart_terminal,
            engine_client::terminal_snapshot,
            conversations::list_conversations,
            conversations::begin_conversation,
            conversations::attach_terminal,
            conversations::handoff_prompt,
            conversations::plan_switch,
            history::agent_history,
            history::agent_history_transcript,
            profiles::list_profiles,
            profiles::create_profile,
            profiles::rename_profile,
            profiles::delete_profile,
            git_client::git_status,
            docker_client::docker_containers,
            docker_client::docker_container_action,
            git_client::git_diff,
            git_client::git_stage,
            git_client::git_unstage,
            git_client::git_commit,
            git_client::git_push,
            git_client::git_pull,
            engine_client::wsl_distros,
            engine_client::rename_conversation,
            engine_client::rename_session,
            engine_client::engine_api,
            ssh::ssh_connections,
            ssh::save_ssh_connection,
            ssh::remove_ssh_connection,
            ssh::test_ssh_connection,
            ssh::ssh_mount_status,
            ssh::ssh_mount,
            ssh::ssh_unmount,
            target_files::target_project_files,
            git_client::git_branches,
            git_client::git_checkout_branch,
            git_client::git_log_graph,
            git_client::git_conta,
            relatorio::relatorio_ia,
            agentes::agentes_listar,
            agentes::agentes_salvar,
            agentes::agentes_apagar,
            agentes::agentes_restaurar_kit,
            agentes::agentes_instalar_claude,
            agentes::agentes_criar_com_ia,
            agentes::agentes_modelos,
            fluxos::fluxos_criar_com_ia,
            fluxos::fluxos_listar,
            fluxos::fluxos_salvar,
            fluxos::fluxos_apagar,
            fluxos::fluxos_validar,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        // Fork: solta o modelo de voz antes de sair. Com ele carregado na GPU, a limpeza do Metal do
        // whisper.cpp no `exit` aborta (`GGML_ASSERT` em ggml-metal-device.m) e o macOS mostra
        // "encerrou inesperadamente" na próxima abertura.
        .run(|_, evento| if let tauri::RunEvent::Exit = evento { voz::liberar() });
}

/// Metadados da subida: o que costuma faltar quando o app é aberto pelo Finder (PATH, TERM, LANG) e
/// onde o binário está (`AppTranslocation` = aberto de dentro do `.dmg` ou de Downloads).
fn registrar_inicio(path_do_shell: &str, ms: u128) {
    let achou = |nome: &str| if engine_client::resolve_on_path(nome).is_some() { "sim" } else { "nao" };
    let presente = |nome: &str| if std::env::var_os(nome).is_some_and(|v| !v.is_empty()) { "sim" } else { "nao" };
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    omni_core::registro::info("app_iniciou", &[
        ("versao", env!("CARGO_PKG_VERSION")),
        ("exe", &exe),
        ("path_do_shell", path_do_shell),
        ("path_ms", &ms.to_string()),
        ("claude", achou("claude")),
        ("codex", achou("codex")),
        ("term", presente("TERM")),
        ("lang", presente("LANG")),
        ("porta", &engine_client::engine_port().to_string()),
    ]);
}
