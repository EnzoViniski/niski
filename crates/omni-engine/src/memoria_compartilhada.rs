//! Acrescenta contexto só no comando digitado, nunca no comando persistido da sessão.
use crate::*;
use omni_core::memoria_compartilhada as memoria;

pub(crate) fn texto(state: &EngineState) -> Option<String> { texto_para(state, None) }

/// Fork (harness, C2): o texto para a conversa. Subagente do orquestrador recebe só a memória curta.
pub(crate) fn texto_para(state: &EngineState, conversa: Option<&str>) -> Option<String> {
    let dir = state.state_file.parent()?;
    let subagente = conversa.is_some_and(|c| mobile::orquestracao::contexto::eh_subagente(state, c));
    let memoria = match memoria::ler(dir) {
        Ok(m) => memoria::contexto(&m),
        Err(_) => { registro::aviso_sem_repetir("memoria_ilegivel", "memoria_nao_carregada", &[]); None }
    };
    // Fork (Sobre mim): vai junto, pelo mesmo caminho (Claude, Codex, app-server e fluxos).
    let sobre_mim = if subagente { None } else { match omni_core::sobre_mim::contexto(dir) {
        Ok(texto) => texto,
        Err(_) => { registro::aviso_sem_repetir("sobre_mim_ilegivel", "sobre_mim_nao_carregado", &[]); None }
    } };
    match (memoria, sobre_mim) {
        (Some(memoria), Some(sobre_mim)) => Some(format!("{memoria}\n\n{sobre_mim}")),
        (memoria, sobre_mim) => memoria.or(sobre_mim),
    }
}

pub(crate) fn argumentos_codex(state: &EngineState, origem: &SessionOrigin) -> Vec<String> {
    let Some(texto) = texto_para(state, origem.conversation_id.as_deref()) else { return vec![] };
    let pasta = origem.env.iter().find(|(k, _)| k == "CODEX_HOME").map(|(_, v)| PathBuf::from(v))
        .or_else(|| env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")).map(|h| PathBuf::from(h).join(".codex")));
    let Some(pasta) = pasta else { return vec![] };
    match memoria::instrucao_codex(&pasta, &texto) {
        Ok(instrucao) => vec!["-c".into(), instrucao],
        Err(_) => { registro::aviso_sem_repetir("memoria_codex", "memoria_nao_carregada", &[("provider", "codex")]); vec![] }
    }
}

fn citar(texto: &str) -> String {
    if cfg!(windows) { format!("'{}'", texto.replace('\'', "''")) }
    else { format!("'{}'", texto.replace('\'', "'\\''")) }
}

/// Fork (harness, A3): nome da variável do ambiente da PTY com o `developer_instructions=…` do Codex.
/// A linha digitada leva só `-c "$MEDZA_CODEX_INSTRUCOES"`: o texto pessoal não passa pelo campo do shell
/// (nem pelo histórico dele) e a linha fica curta. Entre aspas duplas o shell não reavalia o valor.
pub(crate) const VARIAVEL_DO_CODEX: &str = "MEDZA_CODEX_INSTRUCOES";

/// O comando a digitar e o ambiente extra da PTY. O comando persistido da sessão nunca muda.
pub(crate) fn preparar(state: &EngineState, origem: &SessionOrigin, comando: Option<String>) -> (Option<String>, Vec<(String, String)>) {
    let Some(mut comando) = comando else { return (None, Vec::new()) };
    let mut ambiente = Vec::new();
    // Comandos personalizados e servidores remotos mantêm as próprias instruções.
    if comando.contains("--remote") || comando.contains("developer_instructions") || comando.contains(VARIAVEL_DO_CODEX) {
        return (Some(comando), ambiente);
    }
    let Some(texto) = texto_para(state, origem.conversation_id.as_deref()) else { return (Some(comando), ambiente) };
    match origem.provider.as_deref() {
        Some("claude") if !comando.contains("--append-system-prompt-file") => {
            // Caminho fixo da memória não contém o texto pessoal na linha do shell.
            let Some(dir) = state.state_file.parent() else { return (Some(comando), ambiente) };
            // Fork (harness, C2): subagente tem arquivo próprio (só a memória curta). Um arquivo só para os
            // dois faria a abertura de um subagente trocar o texto das conversas que já estão abertas.
            let subagente = origem.conversation_id.as_deref().is_some_and(|c| mobile::orquestracao::contexto::eh_subagente(state, c));
            let arquivo = dir.join(if subagente { "memoria-compartilhada-subagente.md" } else { "memoria-compartilhada.md" });
            // O arquivo é uma projeção do JSON, recriada a cada abertura; nenhum transcript é lido.
            if gravar_texto(&arquivo, &texto).is_ok() {
                comando.push_str(&format!(" --append-system-prompt-file {}", citar(&arquivo.to_string_lossy())));
                if !comando.contains("--system-prompt-snapshot") { comando.push_str(" --system-prompt-snapshot off"); }
            } else {
                registro::aviso_sem_repetir("memoria_claude", "memoria_nao_carregada", &[("provider", "claude")]);
            }
        }
        Some("codex") => {
            let argumentos = argumentos_codex(state, origem);
            match argumentos.as_slice() {
                // Fork (harness, A3): só em shells POSIX (zsh, bash, sh, fish aceitam "$VAR"); no Windows segue como era.
                [flag, valor] if cfg!(unix) && flag == "-c" => {
                    ambiente.push((VARIAVEL_DO_CODEX.to_owned(), valor.clone()));
                    comando.push_str(&format!(" -c \"${VARIAVEL_DO_CODEX}\""));
                }
                _ => for argumento in argumentos { comando.push(' '); comando.push_str(&citar(&argumento)); },
            }
        }
        _ => {}
    }
    (Some(comando), ambiente)
}

fn gravar_texto(arquivo: &Path, texto: &str) -> std::io::Result<()> {
    use std::io::Write;
    let temporario = arquivo.with_file_name(format!(".memoria-{}.tmp", uuid::Uuid::new_v4()));
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    let resultado = (|| {
        let mut f = opcoes.open(&temporario)?;
        f.write_all(texto.as_bytes())?;
        f.sync_all()?;
        fs::rename(&temporario, arquivo)
    })();
    if resultado.is_err() { let _ = fs::remove_file(temporario); }
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;
    fn estado(dir: &Path) -> EngineState {
        EngineState {
            persist_lock: std::sync::Mutex::new(()), mobile: mobile::MobileRuntime::new(dir), token: "sintetico".into(),
            state_file: dir.join("sessions.json"), sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
            id_sequence: std::sync::atomic::AtomicU64::new(1),
        }
    }
    #[test]
    fn contexto_entra_na_cli_sem_mudar_o_comando_original_e_sem_virar_prompt_de_shell() {
        let dir = tempfile::tempdir().unwrap();
        let state = estado(dir.path());
        memoria::salvar(dir.path(), &memoria::Memoria { ativa: true, texto: "Enzo\nResponda em português; $(nunca-executar)".into() }).unwrap();
        let mut origem = SessionOrigin { provider: Some("claude".into()), ..Default::default() };
        let original = "claude --resume abc".to_owned();
        let comando = preparar(&state, &origem, Some(original.clone())).0.unwrap();
        assert!(comando.contains("--append-system-prompt-file") && comando.contains("--system-prompt-snapshot off"));
        assert!(!comando.contains("nunca-executar"));
        assert!(fs::read_to_string(dir.path().join("memoria-compartilhada.md")).unwrap().contains("Enzo"));
        assert_eq!(original, "claude --resume abc");
        origem.provider = Some("codex".into());
        origem.env = vec![("CODEX_HOME".into(), dir.path().to_string_lossy().into_owned())];
        let args = argumentos_codex(&state, &origem);
        assert_eq!(args[0], "-c");
        assert!(args[1].contains("developer_instructions="));
        let (comando, ambiente) = preparar(&state, &origem, Some("codex resume abc".into()));
        let comando = comando.unwrap();
        // Fork (harness, A3): o texto vai no ambiente da PTY; a linha digitada só cita a variável.
        assert_eq!(comando, "codex resume abc -c \"$MEDZA_CODEX_INSTRUCOES\"");
        assert!(!comando.contains("nunca-executar") && !comando.contains("Enzo"));
        assert_eq!(ambiente.len(), 1);
        assert!(ambiente[0].0 == VARIAVEL_DO_CODEX && ambiente[0].1.starts_with("developer_instructions=") && ambiente[0].1.contains("nunca-executar"));
        assert_eq!(preparar(&state, &origem, Some("codex --remote unix:///x resume abc".into())).0.unwrap(), "codex --remote unix:///x resume abc");
        assert_eq!(preparar(&state, &origem, Some(comando.clone())).0.unwrap(), comando, "idempotente");
        memoria::salvar(dir.path(), &memoria::Memoria { ativa: false, texto: "guardada".into() }).unwrap();
        assert!(preparar(&state, &origem, Some("codex resume abc".into())) == (Some("codex resume abc".into()), vec![]));
    }
}

#[cfg(all(test, unix))]
#[path = "memoria_no_ambiente_testes.rs"]
mod memoria_no_ambiente_testes;
