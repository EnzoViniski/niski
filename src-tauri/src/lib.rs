mod conversations;
mod account_usage;
mod agent_runtime;
mod mobile;
mod engine_client;
// Fork (energia): snapshot do terminal com espera (long-poll).
mod espera_do_terminal;
mod git_client;
// Fork: revisão por arquivo/trecho.
mod revisao;
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
// Fork: cópia do estado antes de instalar versão nova pelo updater.
mod backup_antes_de_atualizar;
// Fork: cópia em disco do localStorage da interface (aparência, workspaces), restaurada ao abrir.
mod estado_da_interface;
// Fork: uma janela do app por pasta de dados (duas cópias derrubavam o engine uma da outra).
mod instancia_unica;
// Fork: abrir no app padrão o arquivo que o agente gerou (cartão na Conversa).
mod abrir_entregavel;
// Fork: caminho citado na Conversa vira link (abre pasta/arquivo sem executar nada).
mod abrir_caminho;
// Fork: aba Navegador e trava de link que tomava a janela.
mod navegador;
// Fork: agente numa branch própria (worktree do git).
mod worktree;
// Fork: agente em worktree isolada (branch medza/<nome>) e limite de sessões configurável.
mod worktree_isolada;
mod limite_de_sessoes;
#[cfg(target_os = "macos")]
mod tela_cheia;
mod pedido_de_atualizacao; // Fork: "Atualizar o PC" pedido pelo celular.
// Fork: Jarvis, copiloto por voz (janela flutuante, voz da Apple, Claude Code pensando).
mod jarvis;
// Fork: Roadmaps de estudo (trilhas, cards de revisão e IA ao lado).
mod roadmaps;
mod tarefas; // Fork: tarefas (uma por arquivo, pasta local ou compartilhada).
mod biblia; // Fork: Bíblia do dia (Ave Maria, baixada uma vez e conferida pelo SHA-256).
mod cadernos; // Fork: Caderno de anotações (páginas em Markdown e IA ao lado).
// Fork: Editor de vídeo (projetos HyperFrames, biblioteca, chat do agente, exportar, edição a dois).
mod videos;

pub fn run() {
    // Fork: contas com pasta mas fora do `profiles.json` desta variante voltam a ser reconhecidas.
    if let Ok(raiz) = profiles::raiz_de_perfis() { omni_core::definir_raiz_de_perfis(raiz); }
    if let Ok(dir) = engine_client::engine_dir() {
        omni_core::registro::iniciar("app", &dir);
        // Fork: outra cópia do app já aberta nesta pasta de dados → foca ela e sai (instancia_unica.rs).
        instancia_unica::garantir(&dir);
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
        // Fork (Editor de vídeo): arquivos do projeto para o preview, com CSP fechada (videos/protocolo.rs).
        .register_asynchronous_uri_scheme_protocol(videos::protocolo::ESQUEMA, videos::protocolo::atender)
        // Fork: em tela cheia no Mac, o conteúdo desce junto da barra revelada (tela_cheia.rs).
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            tela_cheia::instalar(_app);
            jarvis::janela::instalar(_app); // Fork: atalho global do Jarvis.
            Ok(())
        })
        .on_window_event(jarvis::janela::ao_evento_da_janela) // Fork: Jarvis guarda posição e fecha junto.
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
            navegador::navegador_conferir, // Fork: some a página de navegador que sobrou sem aba.
            worktree::git_worktree_da_branch,
            worktree_isolada::criar_worktree_isolada,
            worktree_isolada::estado_da_worktree_isolada,
            worktree_isolada::remover_worktree_isolada,
            limite_de_sessoes::limite_de_sessoes,
            limite_de_sessoes::definir_limite_de_sessoes,
            voz::voz_estado,
            voz::voz_baixar_modelo,
            voz::voz_gravar,
            voz::voz_transcrever,
            voz::voz_previa,
            voz::voz_niveis,
            voz::voz_cancelar,
            // Fork: Jarvis.
            jarvis::jarvis_iniciar,
            jarvis::jarvis_encerrar,
            jarvis::jarvis_ouvir,
            jarvis::jarvis_calar,
            jarvis::jarvis_escrever,
            // Fork: ajustes persistidos e vozes locais do orquestrador Jarvis.
            jarvis::jarvis_configuracao,
            jarvis::jarvis_configurar,
            jarvis::jarvis_vozes,
            jarvis::janela::jarvis_alternar,
            jarvis::janela::jarvis_fechar,
            jarvis::janela::jarvis_bolinha,
            jarvis::janela::jarvis_janela_aberta,
            jarvis::janela::jarvis_abrir_fluxo,
            atualizacao::versao_nova_no_disco,
            atualizacao::reiniciar_para_atualizar,
            // Fork: "Atualizar agora" (updater) na barra.
            atualizacao::agentes_trabalhando,
            pedido_de_atualizacao::consumir_pedido_de_atualizacao,
            backup_antes_de_atualizar::backup_antes_de_atualizar,
            tarefas::tarefas_listar, // Fork: Tarefas.
            tarefas::tarefas_gravar,
            tarefas::tarefas_levar_para,
            roadmaps::roadmaps_listar, // Fork: Roadmaps de estudo.
            roadmaps::roadmaps_criar,
            roadmaps::roadmaps_ler,
            roadmaps::roadmaps_gravar,
            roadmaps::roadmaps_anexar_conversa,
            roadmaps::roadmaps_ordenar,
            roadmaps::roadmaps_backup_antes_de_migrar,
            roadmaps::ia::roadmaps_ia,
            roadmaps::ia::roadmaps_ia_cancelar,
            biblia::biblia_ler, // Fork: Bíblia do dia.
            biblia::biblia_baixar,
            cadernos::cadernos_listar, // Fork: Caderno.
            cadernos::cadernos_ler,
            cadernos::cadernos_gravar,
            cadernos::cadernos_anexar_conversa,
            cadernos::cadernos_para_a_lixeira,
            videos::videos_listar, // Fork: Editor de vídeo.
            videos::protocolo::videos_endereco_base,
            videos::agente::videos_agente,
            videos::agente::videos_agente_cancelar,
            videos::agente::videos_agente_rodando,
            videos::render::videos_exportar,
            videos::render::videos_exportar_cancelar,
            videos::render::videos_requisitos,
            videos::render::videos_preparar_navegador,
            videos::sessao::videos_sessao_situacao,
            videos::sessao::videos_sessao_config_gravar,
            videos::sessao::videos_sessao_iniciar,
            videos::sessao::videos_sessao_parar,
            videos::sessao::videos_sessao_permitir,
            videos::sessao::videos_sessao_entrar,
            videos::sessao::videos_sessao_ler_convite,
            videos::sessao::videos_sessao_enviar,
            videos::sessao::videos_sessao_baixar,
            videos::sessao::videos_sessao_subir,
            videos::videos_criar,
            videos::videos_abrir,
            videos::videos_ler,
            videos::videos_gravar,
            videos::videos_anexar_conversa,
            videos::videos_ordenar,
            videos::videos_composicoes,
            videos::videos_versoes,
            videos::videos_criar_versao,
            videos::videos_restaurar_versao,
            videos::videos_apagar,
            videos::videos_importar_midia,
            videos::videos_remover_midia,
            videos::videos_midias_faltando,
            videos::videos_backup,
            videos::videos_ref_listar,
            videos::videos_ref_criar,
            videos::videos_ref_editar,
            videos::videos_ref_ler,
            videos::videos_ref_importar,
            videos::videos_ref_remover_item,
            videos::videos_ref_apagar,
            videos::videos_ref_aplicar,
            videos::videos_ref_de_projeto,
            // Fork: chat normal no editor de vídeo, pasta do computador e exportados.
            videos::videos_configurar_pastas,
            videos::videos_conferir_pasta,
            videos::videos_assinatura,
            videos::videos_registrar_depuracao,
            videos::videos_onda,
            videos::videos_abrir_render,
            videos::videos_copiar_render,
            estado_da_interface::ler_estado_da_interface,
            estado_da_interface::gravar_estado_da_interface,
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
            espera_do_terminal::terminal_snapshot_espera, // Fork (energia): long-poll do terminal.
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
            // Fork: painel de revisão.
            revisao::revisao_listar,
            revisao::revisao_aplicar,
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
        .run(|_, evento| if let tauri::RunEvent::Exit = evento { voz::liberar(); jarvis::jarvis_encerrar() });
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
