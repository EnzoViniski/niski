//! Fork (espaço em disco): cópia por clone do APFS (`clonefile`).
//!
//! O arquivo de conversas guarda uma cópia de cada transcript das CLIs (ver `arquivo_de_conversas`).
//! Copiando byte a byte, isso dobrava o espaço (1,7 GB numa máquina de uso diário) e recopiava o
//! transcript inteiro da conversa ativa a cada rodada. O clone divide os blocos com o original: não
//! ocupa espaço até um dos dois mudar, e só o que mudou ganha bloco próprio. Fora do APFS (ou em outro
//! sistema) devolve `false` e quem chamou copia do jeito antigo.
use std::{io, path::Path};

/// Clona `origem` em `destino` (que não pode existir). `Ok(false)` = clone indisponível aqui.
pub fn clonar(origem: &Path, destino: &Path) -> io::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        unsafe extern "C" {
            fn clonefile(src: *const std::ffi::c_char, dst: *const std::ffi::c_char, flags: u32) -> std::ffi::c_int;
        }
        /// Não segue link simbólico e não copia o dono (fica o do processo, como numa cópia comum).
        const CLONE_NOFOLLOW: u32 = 0x0001;
        const CLONE_NOOWNERCOPY: u32 = 0x0002;
        let origem = CString::new(origem.as_os_str().as_bytes()).map_err(io::Error::other)?;
        let destino = CString::new(destino.as_os_str().as_bytes()).map_err(io::Error::other)?;
        if unsafe { clonefile(origem.as_ptr(), destino.as_ptr(), CLONE_NOFOLLOW | CLONE_NOOWNERCOPY) } == 0 {
            return Ok(true);
        }
        let erro = io::Error::last_os_error();
        // ENOTSUP (volume sem clone) e EXDEV (outro volume): copia normal.
        return match erro.raw_os_error() {
            Some(45) | Some(18) => Ok(false),
            _ => Err(erro),
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (origem, destino);
        Ok(false)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn clone_tem_o_mesmo_conteudo_e_muda_independente_do_original() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("a.jsonl");
        let copia = dir.path().join("b.jsonl");
        std::fs::write(&original, "linha 1\n").unwrap();
        let clonou = clonar(&original, &copia).unwrap();
        if !clonou {
            return; // Volume de teste sem clone (CI fora do APFS): nada a conferir.
        }
        assert_eq!(std::fs::read_to_string(&copia).unwrap(), "linha 1\n");
        std::fs::write(&original, "linha 1\nlinha 2\n").unwrap();
        assert_eq!(std::fs::read_to_string(&copia).unwrap(), "linha 1\n");
    }

    #[test]
    fn destino_existente_e_erro_e_nao_sobrescreve() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("a");
        let copia = dir.path().join("b");
        std::fs::write(&original, "novo").unwrap();
        std::fs::write(&copia, "velho").unwrap();
        if let Ok(clonou) = clonar(&original, &copia) {
            assert!(!clonou);
        }
        assert_eq!(std::fs::read_to_string(&copia).unwrap(), "velho");
    }
}
