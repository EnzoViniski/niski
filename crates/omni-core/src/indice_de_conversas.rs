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
use crate::conversations::Conversation;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const ARQUIVO: &str = "conversations.json";
const TRAVA: &str = "conversations.json.lock";
const COPIAS: usize = 3;

fn decodificar(bytes: &[u8]) -> io::Result<Vec<Conversation>> {
    #[derive(serde::Deserialize)]
    struct Store { conversations: Vec<Conversation> }
    serde_json::from_slice::<Store>(bytes).map(|s| s.conversations)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "O histórico está ilegível. Os arquivos foram preservados para recuperação."))
}

/// Nunca transforma um arquivo ilegível numa lista vazia gravável. As cópias mantêm o mesmo
/// formato, e só entram quando o índice principal não pode ser lido.
pub fn ler(dir: &Path) -> io::Result<Vec<Conversation>> {
    let principal = fs::read(dir.join(ARQUIVO)).and_then(|bytes| decodificar(&bytes));
    if let Ok(conversas) = principal { return Ok(conversas); }
    let erro = principal.unwrap_err();
    for n in 1..=COPIAS {
        if let Ok(conversas) = fs::read(dir.join(format!("{ARQUIVO}.bak.{n}"))).and_then(|bytes| decodificar(&bytes)) {
            crate::registro::aviso_sem_repetir("indice_recuperado", "historico_recuperado", &[("copia", &n.to_string())]);
            return Ok(conversas);
        }
    }
    // Primeira execução, sem índice e sem cópia, é o único vazio implícito permitido.
    if erro.kind() == io::ErrorKind::NotFound && !(1..=COPIAS).any(|n| dir.join(format!("{ARQUIVO}.bak.{n}")).exists()) {
        return Ok(Vec::new());
    }
    Err(erro)
}

fn travar(dir: &Path) -> io::Result<File> {
    fs::create_dir_all(dir)?;
    let arquivo = OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(TRAVA))?;
    arquivo.lock()?;
    Ok(arquivo)
}

fn gravar_atomico(dir: &Path, nome: &str, corpo: &[u8]) -> io::Result<()> {
    static SEQUENCIA: AtomicU64 = AtomicU64::new(0);
    let temporario = dir.join(format!(".{nome}.{}.{}.tmp", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
    let mut opcoes = OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    let resultado = opcoes.open(&temporario).and_then(|mut arquivo| {
        arquivo.write_all(corpo)?;
        arquivo.sync_all()
    }).and_then(|()| fs::rename(&temporario, dir.join(nome)));
    if resultado.is_err() { let _ = fs::remove_file(&temporario); }
    resultado
}

fn gravar_com_a_trava(dir: &Path, conversas: &[Conversation]) -> io::Result<()> {
    let corpo = serde_json::to_vec_pretty(&serde_json::json!({"version": 1, "conversations": conversas}))?;
    // Confere antes até na gravação integral: corrupção sem cópia não autoriza apagar o original.
    ler(dir)?;
    if let Ok(anterior) = fs::read(dir.join(ARQUIVO)) {
        if decodificar(&anterior).is_ok() {
            for n in (1..COPIAS).rev() {
                if let Ok(bytes) = fs::read(dir.join(format!("{ARQUIVO}.bak.{n}"))) {
                    if decodificar(&bytes).is_ok() { gravar_atomico(dir, &format!("{ARQUIVO}.bak.{}", n + 1), &bytes)?; }
                }
            }
        }
    }
    // A cópia mais recente guarda a transação completa, inclusive as conversas recém-criadas.
    gravar_atomico(dir, &format!("{ARQUIVO}.bak.1"), &corpo)?;
    gravar_atomico(dir, ARQUIVO, &corpo)?;
    #[cfg(unix)] File::open(dir)?.sync_all()?;
    Ok(())
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
    let mut indice = ler(dir)?;
    if !f(&mut indice) { return Ok(false); }
    gravar_com_a_trava(dir, &indice)?;
    Ok(true)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::conversations::{read_index, write_index, Segment, TitleSource};

    fn conversa(id: String) -> Conversation {
        Conversation { id, project_id: "p".into(), cwd: "/p".into(), title: "t".into(), title_source: TitleSource::Auto,
            created_at_ms: 1, segments: vec![Segment { provider: "claude".into(), profile_id: None, external_session_id: None,
                transcript_path: None, terminal_session_id: None, started_at_ms: 1, ended_at_ms: None }] }
    }

    #[test]
    fn indice_truncado_recupera_copia_sem_perder_conversas_na_proxima_escrita() {
        let dir = tempfile::tempdir().unwrap();
        write_index(dir.path(), &[conversa("a".into())]).unwrap();
        mudar_indice(dir.path(), |lista| { lista.push(conversa("b".into())); true }).unwrap();
        fs::write(dir.path().join(ARQUIVO), b"{truncado").unwrap();
        assert_eq!(read_index(dir.path())[0].id, "a");
        mudar_indice(dir.path(), |lista| { lista.push(conversa("c".into())); true }).unwrap();
        assert_eq!(read_index(dir.path()).iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(ler(dir.path()).unwrap().len(), 3);
    }

    #[test]
    fn corrupcao_sem_copia_impede_tanto_mutacao_quanto_sobrescrita() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(ARQUIVO), b"{preservar").unwrap();
        assert!(mudar_indice(dir.path(), |_| panic!("não pode alterar um índice ilegível")).is_err());
        assert!(write_index(dir.path(), &[]).is_err());
        assert_eq!(fs::read(dir.path().join(ARQUIVO)).unwrap(), b"{preservar");
    }

    #[test]
    fn tenta_copias_mais_antigas_e_nao_trata_copia_ilegivel_como_primeira_execucao() {
        let dir = tempfile::tempdir().unwrap();
        for id in ["a", "b", "c", "d"] { write_index(dir.path(), &[conversa(id.into())]).unwrap(); }
        fs::remove_file(dir.path().join(ARQUIVO)).unwrap();
        fs::write(dir.path().join(format!("{ARQUIVO}.bak.1")), "corrompida").unwrap();
        assert_eq!(ler(dir.path()).unwrap()[0].id, "c");
        for n in 2..=COPIAS { fs::remove_file(dir.path().join(format!("{ARQUIVO}.bak.{n}"))).unwrap(); }
        assert!(ler(dir.path()).is_err());
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
