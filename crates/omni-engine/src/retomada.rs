//! Fork (retomar-sempre): reabrir uma sessão de agente sem beco sem saída.
//!
//! O reinício (botão "reiniciar", aba revivida depois de o engine reiniciar) só olhava a conta da
//! sessão e a pasta calculada do `cwd`. Conversa em outra conta, ou gravada com o caminho escrito de
//! outro jeito, dava `--session-id` repetido ("já em uso") ou `--resume` sem arquivo ("No conversation
//! found"). Aqui o transcript é procurado em todas as contas (`omni_core::achar_transcript`) e copiado
//! para onde a CLI da conta da sessão procura; sem ele em lugar nenhum, uma retomada explícita
//! preserva o comando para a falha aparecer, sem criar histórico vazio. Pasta apagada vira erro com código, que a
//! interface transforma em "abrir no repositório principal".

use omni_core::achar_transcript::{self, Achado};
use omni_core::Profile;
use omni_protocol::TerminalSession;
use std::path::{Path, PathBuf};

/// Começo da mensagem de erro do reinício quando a pasta da sessão sumiu. A interface procura este
/// texto para oferecer outra pasta.
pub const PASTA_SUMIU: &str = "PASTA_SUMIU";

pub fn erro_pasta_sumiu(cwd: &str) -> String {
    format!("{PASTA_SUMIU}: a pasta {cwd} não existe mais (worktree apagada?). Abra a conversa em outra pasta, como o repositório principal.")
}

/// Pasta da conta da sessão: `CLAUDE_CONFIG_DIR`/`CODEX_HOME` do ambiente dela, senão a padrão.
pub fn pasta_da_conta(meta: &TerminalSession, variavel: &str, padrao: &str) -> Option<PathBuf> {
    meta.env.iter().find(|(chave, _)| chave == variavel).map(|(_, valor)| PathBuf::from(valor))
        .or_else(|| std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|home| PathBuf::from(home).join(padrao)))
}

/// A conta da sessão como perfil: a do `profile_id` se ela estiver na lista; senão uma montada da
/// pasta do ambiente (conta nativa ou perfil fora do `profiles.json`).
fn conta_da_sessao(meta: &TerminalSession, perfis: &[Profile], provider: &str, pasta: PathBuf) -> Profile {
    perfis.iter().find(|p| p.provider == provider && Some(&p.id) == meta.profile_id.as_ref()).cloned()
        .unwrap_or_else(|| Profile {
            id: meta.profile_id.clone().unwrap_or_else(|| "sessao".into()), provider: provider.into(), name: "conta da sessão".into(),
            config_dir: pasta.to_string_lossy().into_owned(), builtin: false, created_at_ms: 0, last_used_at_ms: None,
            authenticated: false, email: None,
        })
}

/// O transcript Claude da sessão está (ou agora ficou) onde a CLI da conta dela vai procurar? Procura
/// na conta da sessão e depois nas `perfis`; achado fora do lugar, copia. `false` = não existe em
/// conta nenhuma (nenhuma mensagem foi mandada, ou o arquivo foi apagado).
pub fn transcript_claude_pronto(meta: &TerminalSession, id: &str, perfis: &[Profile]) -> bool {
    let Some(pasta) = pasta_da_conta(meta, "CLAUDE_CONFIG_DIR", ".claude") else { return false };
    if omni_core::conversations::claude_transcript_path(&pasta, &meta.cwd, id).is_file() { return true; }
    let conta = conta_da_sessao(meta, perfis, "claude", pasta);
    let todas: Vec<Profile> = std::iter::once(conta.clone()).chain(perfis.iter().cloned()).collect();
    let Some(achado) = achar_transcript::achar_claude(&todas, Some(&conta.id), &meta.cwd, id) else { return false };
    copiar(&achado, &conta, &meta.cwd, id)
}

/// Codex: o rollout de `id` em outra conta vai para a da sessão (o `codex resume` só procura no
/// CODEX_HOME dele). Nada a fazer quando já está lá ou não existe.
pub fn levar_rollout_codex(meta: &TerminalSession, id: &str, perfis: &[Profile]) {
    let Some(pasta) = pasta_da_conta(meta, "CODEX_HOME", ".codex") else { return };
    if achar_transcript::codex_na_conta(&pasta, id).is_some() { return; }
    let conta = conta_da_sessao(meta, perfis, "codex", pasta);
    if let Some(achado) = achar_transcript::achar_codex(perfis, None, id) {
        copiar(&achado, &conta, &meta.cwd, id);
    }
}

fn copiar(achado: &Achado, conta: &Profile, cwd: &str, id: &str) -> bool {
    match achar_transcript::colocar_onde_a_cli_procura(achado, conta, cwd, id) {
        Ok(_) => {
            omni_core::registro::info("retomada_transcript_copiado", &[("provider", &conta.provider), ("de_outra_conta", if achado.perfil.id == conta.id { "nao" } else { "sim" })]);
            true
        }
        Err(erro) => {
            omni_core::registro::aviso("retomada_transcript_nao_copiado", &[("provider", &conta.provider), ("erro", &erro.to_string())]);
            false
        }
    }
}

/// A pasta da sessão ainda existe? Erro com [`PASTA_SUMIU`] quando não.
pub fn conferir_pasta(cwd: &str) -> anyhow::Result<()> {
    if Path::new(cwd).is_dir() { Ok(()) } else { Err(anyhow::anyhow!(erro_pasta_sumiu(cwd))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessao(config: &Path, provider: &str, cwd: &str) -> TerminalSession {
        let variavel = if provider == "claude" { "CLAUDE_CONFIG_DIR" } else { "CODEX_HOME" };
        serde_json::from_value(serde_json::json!({
            "id": "t", "project_id": "p", "name": "n", "cwd": cwd, "shell": "bash", "state": "stopped", "pid": null,
            "created_at_ms": 0, "last_activity_at_ms": 0, "output_seq": 0, "rows": 24, "cols": 80,
            "provider": provider, "profile_id": "a", "external_session_id": "abc-123",
            "env": [[variavel, config.to_string_lossy()]],
        })).unwrap()
    }

    fn perfil(id: &str, provider: &str, dir: &Path) -> Profile {
        Profile { id: id.into(), provider: provider.into(), name: id.into(), config_dir: dir.to_string_lossy().into_owned(),
            builtin: false, created_at_ms: 0, last_used_at_ms: None, authenticated: false, email: None }
    }

    #[test]
    fn transcript_em_outra_conta_e_trazido_para_a_da_sessao() {
        let raiz = tempfile::tempdir().unwrap();
        let (a, b) = (raiz.path().join("a"), raiz.path().join("b"));
        let pasta = b.join("projects").join("-repo");
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("abc-123.jsonl"), "{}\n").unwrap();
        let meta = sessao(&a, "claude", "/repo");
        let perfis = [perfil("a", "claude", &a), perfil("b", "claude", &b)];
        assert!(!transcript_claude_pronto(&meta, "abc-123", &perfis[..1]), "só a conta da sessão: não acha");
        assert!(transcript_claude_pronto(&meta, "abc-123", &perfis));
        assert!(a.join("projects").join("-repo").join("abc-123.jsonl").is_file(), "copiado para a conta da sessão");
        assert!(!transcript_claude_pronto(&meta, "outro", &perfis));
    }

    #[test]
    fn rollout_codex_em_outra_conta_vai_para_o_codex_home_da_sessao() {
        let raiz = tempfile::tempdir().unwrap();
        let (a, b) = (raiz.path().join("a"), raiz.path().join("b"));
        let dia = b.join("sessions").join("2026").join("10").join("20");
        std::fs::create_dir_all(&dia).unwrap();
        std::fs::write(dia.join("rollout-2026-10-20T10-00-00-abc-123.jsonl"), "{}\n").unwrap();
        let meta = sessao(&a, "codex", "/repo");
        levar_rollout_codex(&meta, "abc-123", &[perfil("a", "codex", &a), perfil("b", "codex", &b)]);
        assert!(achar_transcript::codex_na_conta(&a, "abc-123").is_some());
    }

    fn estado(dir: &Path) -> std::sync::Arc<crate::EngineState> {
        std::sync::Arc::new(crate::EngineState {
            persist_lock: std::sync::Mutex::new(()), mobile: crate::mobile::MobileRuntime::new(dir), token: "t".into(),
            state_file: dir.join("sessions.json"), sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
            id_sequence: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Aba que o engine anterior deixou numa worktree apagada: o reinício não abre um shell em pasta
    /// nenhuma — devolve o erro com código, e a aba aparece como "retoma ao abrir" até lá.
    #[test]
    fn reiniciar_sessao_de_pasta_apagada_devolve_pasta_sumiu() {
        let raiz = tempfile::tempdir().unwrap();
        let state = estado(raiz.path());
        let mut meta = sessao(raiz.path(), "claude", "/nao/existe/worktree");
        meta.initial_command = Some("echo retomada".into());
        state.sessions.lock().unwrap().insert("t".into(), crate::SessionEntry::Historical(meta.clone()));
        let erro = crate::restart_session(&state, "t").unwrap_err().to_string();
        assert!(erro.starts_with(PASTA_SUMIU), "{erro}");
        let lista = state.sessions.lock().unwrap().get("t").unwrap().metadata();
        assert!(lista.retoma_ao_abrir, "agente histórico parado é marcado");
        meta.provider = None;
        assert!(!crate::SessionEntry::Historical(meta).metadata().retoma_ao_abrir, "terminal puro não é marcado");
    }

    /// `--resume` sem transcript continua explícito, sem criar uma conversa vazia com o mesmo id.
    #[test]
    fn reinicio_de_resume_sem_transcript_preserva_retomada_sem_criar_historico_vazio() {
        let raiz = tempfile::tempdir().unwrap();
        let mut meta = sessao(raiz.path(), "claude", "/repo");
        meta.initial_command = Some("claude --resume abc-123 --model opus".into());
        assert_eq!(crate::comando_ao_reiniciar(&meta).as_deref(), Some("claude --resume abc-123 --model opus"));
        let pasta = raiz.path().join("projects").join("-repo");
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("abc-123.jsonl"), "{}\n").unwrap();
        assert_eq!(crate::comando_ao_reiniciar(&meta).as_deref(), Some("claude --resume abc-123 --model opus"));
    }

    #[test]
    fn pasta_apagada_vira_erro_com_codigo() {
        let raiz = tempfile::tempdir().unwrap();
        assert!(conferir_pasta(&raiz.path().to_string_lossy()).is_ok());
        let erro = conferir_pasta("/nao/existe/mais").unwrap_err().to_string();
        assert!(erro.starts_with(PASTA_SUMIU) && erro.contains("/nao/existe/mais"), "{erro}");
    }
}
