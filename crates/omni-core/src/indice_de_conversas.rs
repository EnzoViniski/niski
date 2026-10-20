//! Fork (fix-orquestracao): o índice de conversas (`conversations.json`) tem vários escritores — no
//! engine, os ganchos do Claude (troca de transcript), o app-server do Codex (`vincular`), abrir,
//! bifurcar e trocar de conta, a descoberta de rollouts; na casca, o Novo agente e a troca de conta.
//! Cada um lia o índice, mudava e regravava inteiro sem trava: uma atualização apagava a outra (conversa,
//! nome ou segmento), e todos disputavam o mesmo `conversations.json.tmp`.
//!
//! Aqui fica a transação única:
//! - trava exclusiva em `conversations.json.lock` (`File::lock`, um `flock`: vale entre threads — cada
//!   chamada abre o arquivo de novo — e entre o engine e a casca). Solta quando o arquivo fecha;
//! - `.tmp` exclusivo por gravação (`create_new`, com pid e sequência no nome), renomeado por cima do
//!   índice; se algo falha no meio, o `.tmp` sai.
//!
//! Nunca chame `write_index`/`mudar_indice` de dentro do `f` de `mudar_indice`: a trava não é reentrante.
use crate::conversations::{read_index, Conversation};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const ARQUIVO: &str = "conversations.json";
const TRAVA: &str = "conversations.json.lock";

fn travar(dir: &Path) -> io::Result<File> {
    let arquivo = OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(TRAVA))?;
    arquivo.lock()?;
    Ok(arquivo)
}

fn gravar_com_a_trava(dir: &Path, conversas: &[Conversation]) -> io::Result<()> {
    static SEQUENCIA: AtomicU64 = AtomicU64::new(0);
    let corpo = serde_json::to_vec_pretty(&serde_json::json!({"version": 1, "conversations": conversas}))?;
    let temporario = dir.join(format!(".{ARQUIVO}.{}.{}.tmp", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
    let resultado = OpenOptions::new().write(true).create_new(true).open(&temporario)
        .and_then(|mut arquivo| arquivo.write_all(&corpo))
        .and_then(|()| fs::rename(&temporario, dir.join(ARQUIVO)));
    if resultado.is_err() { let _ = fs::remove_file(&temporario); }
    resultado
}

/// Grava o índice inteiro (sob a trava, `.tmp` exclusivo). Quem lê antes para alterar usa `mudar_indice`.
pub fn gravar(dir: &Path, conversas: &[Conversation]) -> io::Result<()> {
    let _trava = travar(dir)?;
    gravar_com_a_trava(dir, conversas)
}

/// Ler, alterar e gravar numa transação só. `f` devolve se mudou algo (sem mudança, nada é gravado);
/// a resposta é o mesmo `bool`.
pub fn mudar_indice(dir: &Path, f: impl FnOnce(&mut Vec<Conversation>) -> bool) -> io::Result<bool> {
    let _trava = travar(dir)?;
    let mut indice = read_index(dir);
    if !f(&mut indice) { return Ok(false); }
    gravar_com_a_trava(dir, &indice)?;
    Ok(true)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::conversations::{write_index, Segment, TitleSource};

    fn conversa(id: String) -> Conversation {
        Conversation { id, project_id: "p".into(), cwd: "/p".into(), title: "t".into(), title_source: TitleSource::Auto,
            created_at_ms: 1, segments: vec![Segment { provider: "claude".into(), profile_id: None, external_session_id: None,
                transcript_path: None, terminal_session_id: None, started_at_ms: 1, ended_at_ms: None }] }
    }

    /// Muitas alterações ao mesmo tempo (o `vincular` do Codex, o transcript dos ganchos, a casca): nenhuma
    /// se perde, porque cada uma lê o índice já com a trava.
    #[test]
    fn alteracoes_simultaneas_nao_apagam_umas_as_outras() {
        let dir = tempfile::tempdir().unwrap();
        std::thread::scope(|s| {
            for t in 0..12 {
                let dir = dir.path();
                s.spawn(move || for i in 0..10 {
                    mudar_indice(dir, |indice| { indice.push(conversa(format!("c-{t}-{i}"))); true }).unwrap();
                });
            }
        });
        assert_eq!(read_index(dir.path()).len(), 120);
        // Sem mudança, nada é gravado.
        let antes = std::fs::metadata(dir.path().join("conversations.json")).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!mudar_indice(dir.path(), |_| false).unwrap());
        assert_eq!(std::fs::metadata(dir.path().join("conversations.json")).unwrap().modified().unwrap(), antes);
    }

    /// Gravações simultâneas não disputam o mesmo `.tmp` (antes um `rename` achava o arquivo já movido
    /// pelo outro e falhava, ou gravava o conteúdo misturado) e não deixam `.tmp` para trás.
    #[test]
    fn gravacoes_simultaneas_usam_tmp_exclusivo() {
        let dir = tempfile::tempdir().unwrap();
        std::thread::scope(|s| {
            for t in 0..12 {
                let dir = dir.path();
                s.spawn(move || for i in 0..20 {
                    write_index(dir, &[conversa(format!("c-{t}-{i}"))]).unwrap();
                });
            }
        });
        assert_eq!(read_index(dir.path()).len(), 1);
        let sobras: Vec<_> = std::fs::read_dir(dir.path()).unwrap().filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".tmp")).collect();
        assert!(sobras.is_empty(), "{sobras:?}");
    }
}
