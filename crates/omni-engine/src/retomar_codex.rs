//! Fork: reiniciar (ou reviver depois de o engine reiniciar) uma sessão do Codex volta para a mesma
//! conversa.
//!
//! O `comando_ao_reiniciar` do autor só sabe retomar o Claude: o Codex era redigitado como na
//! abertura (`codex ...`) e começava uma conversa nova — no terminal o histórico "sumia" e a Conversa
//! seguia lendo o rollout antigo, sem o que o agente fazia agora. Uma sessão bifurcada (`codex fork
//! <id>`) bifurcava de novo a cada reinício. O id nativo vem do índice de conversas (a descoberta de
//! rollouts o preenche assim que o primeiro prompt cria o arquivo); sem ele, nada a retomar.

use omni_core::conversations::{discover_codex_transcripts, read_index};
use omni_protocol::TerminalSession;
use std::path::Path;

/// `codex resume <id>` com as opções do comando original (modelo, esforço). `None` quando não é
/// Codex, quando a sessão ainda não tem rollout ou quando o comando tem algo que não dá para
/// reaproveitar com segurança (um prompt entre aspas, por exemplo).
pub fn comando(dir: &Path, meta: &TerminalSession, original: &str) -> Option<String> {
    if meta.provider.as_deref() != Some("codex") { return None; }
    // Fork 3A: app-server já conhece o id, mesmo antes de existir rollout em disco.
    if let Some(id)=meta.external_session_id.as_deref().filter(|id|!id.is_empty() && id.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-')) {
        if let Some(comando)=com_resume(original,id) {return Some(comando);}
    }
    let mut indice = read_index(dir);
    discover_codex_transcripts(&mut indice, &omni_core::profiles(dir));
    // Fork (trocar agente): vale só o **último** trecho desta PTY. Depois de Codex → Claude → Codex na
    // mesma aba, o Codex novo ainda sem rollout não pode retomar o rollout do primeiro trecho.
    let ultimo = indice.iter()
        .filter(|c| meta.conversation_id.as_deref().is_none_or(|id| id == c.id))
        .flat_map(|c| c.segments.iter())
        .filter(|s| s.terminal_session_id.as_deref() == Some(meta.id.as_str()))
        .next_back()?;
    let sessao = (ultimo.provider == "codex").then(|| ultimo.external_session_id.clone()).flatten()?;
    com_resume(original, &sessao)
}

fn com_resume(original: &str, sessao: &str) -> Option<String> {
    // Fork 3A: flags TOML entre aspas também precisam retomar o thread, sem repetir um fork.
    if original.contains(['"', '\'']) { return crate::codex_app_server::comando_de_retomada(original,sessao); }
    let mut partes = original.split_whitespace();
    let binario = partes.next()?;
    let mut resto: Vec<&str> = partes.collect();
    // Já é retomada ou bifurcação: tira o subcomando e o id antigos; vale o id desta sessão.
    if let Some(posicao) = resto.iter().position(|p| *p == "resume" || *p == "fork") {
        let fim = (posicao + 2).min(resto.len());
        resto.drain(posicao..fim);
    }
    let opcoes = if resto.is_empty() { String::new() } else { format!(" {}", resto.join(" ")) };
    Some(format!("{binario} resume {sessao}{opcoes}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omni_core::conversations::{write_index, Conversation, Segment, TitleSource};

    fn sessao(id: &str, provider: &str) -> TerminalSession {
        serde_json::from_value(serde_json::json!({
            "id": id, "project_id": "p", "name": "Codex", "cwd": "/projeto", "shell": "zsh",
            "state": "stopped", "pid": null, "created_at_ms": 0, "last_activity_at_ms": 0, "output_seq": 0,
            "rows": 24, "cols": 80, "provider": provider, "conversation_id": "c",
        })).unwrap()
    }

    fn indice(dir: &Path, externo: Option<&str>) {
        let trecho = Segment { provider: "codex".into(), profile_id: None, external_session_id: externo.map(str::to_owned),
            transcript_path: None, terminal_session_id: Some("t".into()), started_at_ms: 0, ended_at_ms: None };
        let conversa = Conversation { id: "c".into(), project_id: "p".into(), cwd: "/projeto".into(), title: "x".into(),
            title_source: TitleSource::Auto, created_at_ms: 0, segments: vec![trecho] };
        write_index(dir, &[conversa]).unwrap();
    }

    #[test]
    fn codex_com_rollout_volta_para_a_mesma_conversa() {
        let dir = tempfile::tempdir().unwrap();
        indice(dir.path(), Some("019a-codex"));
        let meta = sessao("t", "codex");
        assert_eq!(comando(dir.path(), &meta, "codex -m gpt-x -c model_reasoning_effort=high").as_deref(),
            Some("codex resume 019a-codex -m gpt-x -c model_reasoning_effort=high"));
        assert_eq!(comando(dir.path(), &meta, "codex").as_deref(), Some("codex resume 019a-codex"));
        // Fork reiniciado retoma a cópia, não bifurca de novo.
        assert_eq!(comando(dir.path(), &meta, "codex fork original").as_deref(), Some("codex resume 019a-codex"));
    }

    #[test]
    fn sem_rollout_ou_sem_codex_fica_o_comando_original() {
        let dir = tempfile::tempdir().unwrap();
        indice(dir.path(), None);
        assert_eq!(comando(dir.path(), &sessao("t", "codex"), "codex"), None, "nenhum prompt ainda: abre como antes");
        indice(dir.path(), Some("019a-codex"));
        assert_eq!(comando(dir.path(), &sessao("outra", "codex"), "codex"), None, "trecho de outra sessão");
        assert_eq!(comando(dir.path(), &sessao("t", "claude"), "claude"), None);
        assert_eq!(comando(dir.path(), &sessao("t", "codex"), "codex \"faça x\""), None);
    }

    /// Fork (trocar agente): Codex → Claude → Codex na mesma PTY. O Codex novo ainda sem rollout abre
    /// conversa nova (a passagem de bastão leva o contexto), não volta ao rollout do primeiro trecho.
    #[test]
    fn codex_depois_de_trocar_de_agente_nao_retoma_o_trecho_antigo() {
        let dir = tempfile::tempdir().unwrap();
        let trecho = |provider: &str, externo: Option<&str>| Segment { provider: provider.into(), profile_id: None,
            external_session_id: externo.map(str::to_owned), transcript_path: None, terminal_session_id: Some("t".into()), started_at_ms: 0, ended_at_ms: None };
        let conversa = Conversation { id: "c".into(), project_id: "p".into(), cwd: "/projeto".into(), title: "x".into(),
            title_source: TitleSource::Auto, created_at_ms: 0, segments: vec![trecho("codex", Some("antigo")), trecho("claude", Some("cl")), trecho("codex", None)] };
        write_index(dir.path(), &[conversa.clone()]).unwrap();
        assert_eq!(comando(dir.path(), &sessao("t", "codex"), "codex"), None);
        let mut com_rollout = conversa;
        com_rollout.segments[2].external_session_id = Some("novo".into());
        write_index(dir.path(), &[com_rollout]).unwrap();
        assert_eq!(comando(dir.path(), &sessao("t", "codex"), "codex").as_deref(), Some("codex resume novo"));
    }
}
