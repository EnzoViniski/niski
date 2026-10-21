//! Acrescenta contexto só no comando digitado, nunca no comando persistido da sessão.
use crate::*;
use omni_core::memoria_compartilhada as memoria;

pub(crate) fn texto(state: &EngineState) -> Option<String> {
    let dir = state.state_file.parent()?;
    let memoria = match memoria::ler(dir) {
        Ok(m) => memoria::contexto(&m),
        Err(_) => { registro::aviso_sem_repetir("memoria_ilegivel", "memoria_nao_carregada", &[]); None }
    };
    // Fork (Sobre mim): vai junto, pelo mesmo caminho (Claude, Codex, app-server e fluxos).
    let sobre_mim = match omni_core::sobre_mim::contexto(dir) {
        Ok(texto) => texto,
        Err(_) => { registro::aviso_sem_repetir("sobre_mim_ilegivel", "sobre_mim_nao_carregado", &[]); None }
    };
    match (memoria, sobre_mim) {
        (Some(memoria), Some(sobre_mim)) => Some(format!("{memoria}\n\n{sobre_mim}")),
        (memoria, sobre_mim) => memoria.or(sobre_mim),
    }
}

pub(crate) fn argumentos_codex(state: &EngineState, origem: &SessionOrigin) -> Vec<String> {
    let Some(texto) = texto(state) else { return vec![] };
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

pub(crate) fn preparar(state: &EngineState, origem: &SessionOrigin, comando: Option<String>) -> Option<String> {
    let mut comando = comando?;
    // Comandos personalizados e servidores remotos mantêm as próprias instruções.
    if comando.contains("--remote") || comando.contains("developer_instructions") { return Some(comando); }
    let Some(texto) = texto(state) else { return Some(comando) };
    match origem.provider.as_deref() {
        Some("claude") if !comando.contains("--append-system-prompt-file") => {
            // Caminho fixo da memória não contém o texto pessoal na linha do shell.
            let dir = state.state_file.parent()?;
            let arquivo = dir.join("memoria-compartilhada.md");
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
            for argumento in argumentos { comando.push(' '); comando.push_str(&citar(&argumento)); }
        }
        _ => {}
    }
    Some(comando)
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
        let comando = preparar(&state, &origem, Some(original.clone())).unwrap();
        assert!(comando.contains("--append-system-prompt-file") && comando.contains("--system-prompt-snapshot off"));
        assert!(!comando.contains("nunca-executar"));
        assert!(fs::read_to_string(dir.path().join("memoria-compartilhada.md")).unwrap().contains("Enzo"));
        assert_eq!(original, "claude --resume abc");
        origem.provider = Some("codex".into());
        origem.env = vec![("CODEX_HOME".into(), dir.path().to_string_lossy().into_owned())];
        let args = argumentos_codex(&state, &origem);
        assert_eq!(args[0], "-c");
        assert!(args[1].contains("developer_instructions="));
        let comando = preparar(&state, &origem, Some("codex resume abc".into())).unwrap();
        assert!(comando.contains("'developer_instructions="));
        assert_eq!(preparar(&state, &origem, Some("codex --remote unix:///x resume abc".into())).unwrap(), "codex --remote unix:///x resume abc");
        memoria::salvar(dir.path(), &memoria::Memoria { ativa: false, texto: "guardada".into() }).unwrap();
        assert_eq!(preparar(&state, &origem, Some("codex resume abc".into())).unwrap(), "codex resume abc");
    }
}
