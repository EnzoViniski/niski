//! Fork: só reabre uma PTY depois de encerrar sua CLI em primeiro plano.
//! `portable_pty::Child::kill` sinaliza o shell, não o grupo da CLI que ele abriu.
use super::*;
#[cfg(test)]
#[path = "encerramento_da_pty_testes.rs"]
mod testes;

pub(crate) fn encerrar(sessao: &LiveSession) -> Result<()> {
    let mut filho = sessao
        .child
        .lock()
        .map_err(|_| anyhow!("terminal child poisoned"))?;
    // Um backend sem PID (incluindo os Child sintéticos dos testes) não oferece espera de processo.
    if filho.process_id().is_none() {
        filho.kill()?;
        return Ok(());
    }
    #[cfg(unix)]
    if let Some(shell) = filho
        .process_id()
        .filter(|_| matches!(filho.try_wait(), Ok(None)))
    {
        let frente = sessao
            .master
            .lock()
            .map_err(|_| anyhow!("terminal master poisoned"))?
            .process_group_leader();
        if let Some(grupo) = frente.filter(|g| *g > 1 && *g != shell as i32) {
            // A PTY cria uma sessão POSIX própria (setsid). Não sinaliza grupo de outro app,
            // nem grupo herdado pelo engine, mesmo se o descritor já foi fechado/reutilizado.
            let nosso = unsafe { libc::getsid(grupo) == shell as i32 && grupo != libc::getpgrp() };
            if nosso {
                unsafe {
                    libc::kill(-grupo, libc::SIGTERM);
                }
                let prazo = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while unsafe { libc::kill(-grupo, 0) } == 0 && std::time::Instant::now() < prazo {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                if unsafe { libc::kill(-grupo, 0) } == 0 {
                    unsafe {
                        libc::kill(-grupo, libc::SIGKILL);
                    }
                }
            }
        }
    }
    if filho.try_wait()?.is_none() {
        filho.kill()?;
    }
    let prazo = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while filho.try_wait()?.is_none() {
        if std::time::Instant::now() >= prazo {
            return Err(anyhow!(
                "A sessão anterior ainda está encerrando; tente novamente"
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(())
}
