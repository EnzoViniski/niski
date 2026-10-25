//! Integridade do modelo de voz, sem comandos externos e sem carregar o arquivo inteiro na RAM.
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub(crate) fn conferir(caminho: &Path, bytes: u64, esperado: &str) -> Result<(), String> {
    let mut arquivo = File::open(caminho).map_err(|e| e.to_string())?;
    if arquivo.metadata().map_err(|e| e.to_string())?.len() != bytes {
        return Err("tamanho diferente do esperado".into());
    }
    let mut hash = Sha256::new();
    let mut bloco = [0_u8; 64 * 1024];
    loop {
        let n = arquivo.read(&mut bloco).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        hash.update(&bloco[..n]);
    }
    if format!("{:x}", hash.finalize()) != esperado { return Err("SHA-256 diferente do esperado".into()); }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn aceita_o_modelo_integro_e_recusa_conteudo_ou_tamanho_diferente() {
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("modelo.bin");
        std::fs::write(&arquivo, b"abc").unwrap();
        let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(conferir(&arquivo, 3, hash).is_ok());
        assert!(conferir(&arquivo, 4, hash).is_err());
        std::fs::write(&arquivo, b"abd").unwrap();
        assert!(conferir(&arquivo, 3, hash).is_err());
    }
}
