//! Fork (Editor de vídeo): forma de onda dos clipes de áudio e vídeo na timeline.
//!
//! - Usa o `ffmpeg` que já está no PC (o render do HyperFrames depende dele). Procura pelos caminhos
//!   conhecidos (`omni_core::cli_path`), como o app faz com claude/codex: aberto pelo Finder o app não
//!   herda o PATH do Homebrew. Sem ffmpeg ou sem áudio no arquivo: `None` e a timeline fica sem onda.
//! - Picos de 0 a 255, [`POR_SEGUNDO`] por segundo, guardados em `<dados>/videos/cache/ondas/` pela
//!   chave caminho + tamanho + data (o editor abre de novo sem reprocessar). Um cálculo por vez.
//! - Só arquivo comum dentro da pasta do projeto (sem `..`, sem link).

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use super::arquivos::{self, gravar_atomico, invalido};

pub const POR_SEGUNDO: usize = 20;
const TAXA: usize = 8000;
/// Uma hora de áudio no máximo (o resto é cortado).
const MAXIMO_DE_PICOS: usize = POR_SEGUNDO * 3600;
static UM_POR_VEZ: Mutex<()> = Mutex::new(());

pub fn ffmpeg() -> Option<PathBuf> {
    omni_core::cli_path::ampliar_path();
    crate::engine_client::resolve_on_path("ffmpeg")
}

/// PCM 16 bits mono → pico de cada bloco de `amostras_por_pico`, em 0..=255.
pub fn picos(pcm: &[u8], amostras_por_pico: usize) -> Vec<u8> {
    let amostras: Vec<i16> = pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    amostras.chunks(amostras_por_pico.max(1)).take(MAXIMO_DE_PICOS)
        .map(|bloco| {
            let maior = bloco.iter().map(|a| a.unsigned_abs()).max().unwrap_or(0) as u32;
            (maior * 255 / 32768).min(255) as u8
        })
        .collect()
}

/// Arquivo da mídia dentro do projeto (relativo, sem `..`, arquivo comum).
pub fn arquivo_do_projeto(pasta: &Path, rel: &str) -> io::Result<PathBuf> {
    let rel = rel.trim_start_matches("./");
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|s| s.is_empty() || s == ".." || s.starts_with('.')) {
        return Err(invalido("Arquivo inválido."));
    }
    let caminho = pasta.join(rel);
    let mut atual = pasta.to_path_buf();
    for s in rel.split('/') {
        atual.push(s);
        if fs::symlink_metadata(&atual).map(|m| m.file_type().is_symlink()).unwrap_or(true) { return Err(invalido("Arquivo inválido.")); }
    }
    if arquivos::arquivo_comum(&caminho).is_none() { return Err(invalido("Arquivo inválido.")); }
    Ok(caminho)
}

fn chave(arquivo: &Path) -> io::Result<String> {
    let m = fs::metadata(arquivo)?;
    let mut h = DefaultHasher::new();
    arquivo.hash(&mut h);
    m.len().hash(&mut h);
    m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).hash(&mut h);
    POR_SEGUNDO.hash(&mut h);
    Ok(format!("{:016x}", h.finish()))
}

/// Picos do arquivo (do cache em disco, ou calculados pelo `ffmpeg`). `None`: sem ffmpeg ou sem áudio.
pub fn obter(cache: &Path, arquivo: &Path, ffmpeg: Option<&Path>) -> io::Result<Option<Vec<u8>>> {
    let guardado = cache.join(format!("{}.bin", chave(arquivo)?));
    if let Ok(b) = fs::read(&guardado) { return Ok(Some(b)); }
    let Some(ffmpeg) = ffmpeg else { return Ok(None) };
    let _vez = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(b) = fs::read(&guardado) { return Ok(Some(b)); }
    let mut filho = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-i"]).arg(arquivo)
        .args(["-vn", "-ac", "1", "-ar", &TAXA.to_string(), "-t", "3600", "-f", "s16le", "-"])
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn()?;
    let mut pcm = Vec::new();
    if let Some(mut saida) = filho.stdout.take() { saida.read_to_end(&mut pcm)?; }
    let ok = filho.wait().map(|s| s.success()).unwrap_or(false);
    if !ok || pcm.is_empty() { return Ok(None); }
    let resultado = picos(&pcm, TAXA / POR_SEGUNDO);
    if arquivos::criar_pasta(cache).is_ok() { let _ = gravar_atomico(&guardado, &resultado); }
    Ok(Some(resultado))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn picos_por_bloco_em_0_a_255() {
        let mut pcm = Vec::new();
        for a in [0i16, 100, -32768, 50, 16384, -10] { pcm.extend_from_slice(&a.to_le_bytes()); }
        assert_eq!(picos(&pcm, 2), vec![0, 255, 127]);
        assert!(picos(&[], 400).is_empty());
    }

    #[test]
    fn so_arquivo_comum_dentro_do_projeto() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::write(dir.path().join("assets/a.mp3"), b"x").unwrap();
        fs::write(dir.path().join("fora.mp3"), b"x").unwrap();
        assert!(arquivo_do_projeto(dir.path(), "assets/a.mp3").is_ok());
        assert!(arquivo_do_projeto(dir.path(), "./assets/a.mp3").is_ok());
        for ruim in ["../fora.mp3", "/etc/hosts", "assets/../fora.mp3", "assets/nao.mp3", ".claude/x", ""] {
            assert!(arquivo_do_projeto(dir.path(), ruim).is_err(), "{ruim}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.path().join("fora.mp3"), dir.path().join("assets/link.mp3")).unwrap();
            assert!(arquivo_do_projeto(dir.path(), "assets/link.mp3").is_err());
        }
    }

    #[test]
    fn sem_ffmpeg_nao_tem_onda_e_cache_e_usado() {
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("a.mp3");
        fs::write(&arquivo, b"x").unwrap();
        let cache = dir.path().join("cache");
        assert_eq!(obter(&cache, &arquivo, None).unwrap(), None);
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join(format!("{}.bin", chave(&arquivo).unwrap())), [1u8, 2, 3]).unwrap();
        assert_eq!(obter(&cache, &arquivo, None).unwrap(), Some(vec![1, 2, 3]));
    }

    #[test]
    #[ignore = "precisa do ffmpeg no PC"]
    fn ffmpeg_de_verdade_tira_picos_de_um_tom() {
        let Some(ff) = ffmpeg() else { return };
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("tom.wav");
        let ok = Command::new(&ff).args(["-v", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=2", "-y"]).arg(&arquivo).status().unwrap().success();
        assert!(ok);
        let p = obter(&dir.path().join("cache"), &arquivo, Some(&ff)).unwrap().unwrap();
        assert_eq!(p.len(), 2 * POR_SEGUNDO);
        assert!(p.iter().all(|&v| v > 20));
    }
}
