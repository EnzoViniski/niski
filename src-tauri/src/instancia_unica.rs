//! Fork: uma janela do app por pasta de dados.
//!
//! Duas cópias do mesmo app (o `.app` de /Applications e um esquecido em outra pasta, por exemplo)
//! têm o mesmo bundle id, a mesma pasta de dados e a mesma porta do engine. Abertas juntas, cada uma
//! achava que o engine da outra era um "binário trocado" e o derrubava: agentes reiniciando, lateral
//! perdendo conversas e o localStorage do WebKit gravado por dois processos ao mesmo tempo.
//!
//! A primeira que abre segura uma trava (`flock`) em `<pasta de dados>/app.lock` até sair. A segunda
//! espera um pouco (o reinício da atualização abre a nova antes de a velha terminar de sair), e se a
//! trava continuar presa traz a janela que já está aberta para frente e sai.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const ARQUIVO: &str = "app.lock";
/// Quanto a segunda cópia espera a trava soltar antes de desistir.
const ESPERA: Duration = Duration::from_secs(6);

static TRAVA: OnceLock<File> = OnceLock::new();

pub enum Resultado {
    /// Esta é a única aberta: segue.
    Unica,
    /// Outra já está aberta (caminho do executável dela, quando gravado).
    JaAberta(Option<PathBuf>),
}

/// Tenta pegar a trava por até `espera`. Quem pega guarda o arquivo aberto (a trava vive com ele).
pub fn travar(dir: &Path, espera: Duration) -> std::io::Result<(Resultado, Option<File>)> {
    std::fs::create_dir_all(dir)?;
    let mut arquivo = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join(ARQUIVO))?;
    let inicio = Instant::now();
    loop {
        match arquivo.try_lock() {
            Ok(()) => {
                arquivo.set_len(0)?;
                arquivo.seek(SeekFrom::Start(0))?;
                let exe = std::env::current_exe().map(|caminho| caminho.to_string_lossy().into_owned()).unwrap_or_default();
                write!(arquivo, "{}\n{}\n", std::process::id(), exe)?;
                arquivo.flush()?;
                return Ok((Resultado::Unica, Some(arquivo)));
            }
            Err(TryLockError::WouldBlock) if inicio.elapsed() < espera => std::thread::sleep(Duration::from_millis(200)),
            Err(TryLockError::WouldBlock) => {
                let mut conteudo = String::new();
                let _ = arquivo.read_to_string(&mut conteudo);
                let exe = conteudo.lines().nth(1).filter(|linha| !linha.is_empty()).map(PathBuf::from);
                return Ok((Resultado::JaAberta(exe), None));
            }
            Err(TryLockError::Error(erro)) => return Err(erro),
        }
    }
}

/// `.../Medza Agents.app/Contents/MacOS/omni-agents` → `.../Medza Agents.app`.
pub fn pacote_do_app(exe: &Path) -> Option<PathBuf> {
    exe.ancestors().find(|parte| parte.extension().is_some_and(|ext| ext == "app")).map(Path::to_path_buf)
}

/// Chamado no começo do `run()`. Se outra cópia já está aberta, traz ela para frente e encerra esta.
pub fn garantir(dir: &Path) {
    match travar(dir, ESPERA) {
        Ok((Resultado::Unica, Some(arquivo))) => { let _ = TRAVA.set(arquivo); }
        Ok((Resultado::JaAberta(exe), _)) => {
            omni_core::registro::aviso("app_ja_aberto", &[("acao", "focar_e_sair")]);
            #[cfg(target_os = "macos")]
            if let Some(pacote) = exe.as_deref().and_then(pacote_do_app) {
                let _ = std::process::Command::new("/usr/bin/open").arg("-a").arg(pacote).status();
            }
            #[cfg(not(target_os = "macos"))]
            let _ = exe;
            std::process::exit(0);
        }
        Ok((Resultado::Unica, None)) => {}
        // Sem como travar (pasta só leitura, sistema sem flock): abre como antes.
        Err(erro) => omni_core::registro::aviso("trava_do_app_falhou", &[("erro", &erro.kind().to_string())]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segunda_copia_nao_pega_a_trava_e_le_quem_esta_aberto() {
        let dir = tempfile::tempdir().unwrap();
        let (primeira, arquivo) = travar(dir.path(), Duration::ZERO).unwrap();
        assert!(matches!(primeira, Resultado::Unica));
        assert!(arquivo.is_some());
        // Outro descritor do mesmo arquivo = outra cópia do app (flock é por descritor aberto).
        let (segunda, _) = travar(dir.path(), Duration::from_millis(300)).unwrap();
        match segunda {
            Resultado::JaAberta(Some(exe)) => assert_eq!(exe, std::env::current_exe().unwrap()),
            _ => panic!("a segunda cópia devia achar a primeira aberta"),
        }
    }

    #[test]
    fn trava_solta_quando_a_primeira_sai() {
        let dir = tempfile::tempdir().unwrap();
        let (_, arquivo) = travar(dir.path(), Duration::ZERO).unwrap();
        drop(arquivo);
        let (depois, _) = travar(dir.path(), Duration::ZERO).unwrap();
        assert!(matches!(depois, Resultado::Unica));
    }

    #[test]
    fn acha_o_pacote_do_app_pelo_executavel() {
        assert_eq!(
            pacote_do_app(Path::new("/Applications/Medza Agents.app/Contents/MacOS/omni-agents")),
            Some(PathBuf::from("/Applications/Medza Agents.app")),
        );
        assert_eq!(pacote_do_app(Path::new("/usr/local/bin/omni-agents")), None);
    }
}
