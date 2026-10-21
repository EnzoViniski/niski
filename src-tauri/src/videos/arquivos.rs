//! Fork (Editor de vídeo): peças de disco usadas por todo o módulo. Mesma garantia dos roadmaps:
//! grava inteiro ou nada (`.tmp` exclusivo + `fsync` + rename + `fsync` da pasta), só para o dono,
//! sem seguir link simbólico e uma gravação por vez no processo.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Uma gravação por vez no processo (a janela é uma só: `instancia_unica.rs`).
pub static TRAVA: Mutex<()> = Mutex::new(());

pub fn travar() -> std::sync::MutexGuard<'static, ()> { TRAVA.lock().unwrap_or_else(|e| e.into_inner()) }

pub fn invalido(mensagem: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, mensagem.to_owned()) }

/// Id de projeto ou de pasta de referências: `[a-z0-9-]`, 1 a 64, sem começar com `-` (vira nome de pasta).
pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && !id.starts_with('-')
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `<prefixo>-<tempo em base 36>-<aleatório>`, com sufixo se já existir em `raiz`.
pub fn novo_id(raiz: &Path, prefixo: &str, agora_ms: u128, aleatorio: u32) -> String {
    let base = format!("{prefixo}-{}-{:x}", base36(agora_ms), aleatorio & 0xffff);
    let mut id = base.clone();
    let mut n = 1;
    while raiz.join(&id).exists() { n += 1; id = format!("{base}-{n}"); }
    id
}

pub fn base36(mut n: u128) -> String {
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 { return "0".into(); }
    let mut s = Vec::new();
    while n > 0 { s.push(D[(n % 36) as usize]); n /= 36; }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

pub fn criar_pasta(pasta: &Path) -> io::Result<()> {
    if pasta.is_dir() { return Ok(()); }
    fs::create_dir_all(pasta)?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(pasta, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn temporario_ao_lado(destino: &Path) -> io::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCIA: AtomicU64 = AtomicU64::new(0);
    let pasta = destino.parent().ok_or_else(|| invalido("caminho sem pasta"))?;
    let nome = destino.file_name().and_then(|n| n.to_str()).unwrap_or("arquivo");
    Ok(pasta.join(format!(".{nome}.{}.{}.tmp", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed))))
}

fn abrir_exclusivo(caminho: &Path) -> io::Result<fs::File> {
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    opcoes.open(caminho)
}

fn sincronizar_pasta(pasta: &Path) {
    // O rename só fica durável com a pasta sincronizada.
    if let Ok(p) = fs::File::open(pasta) { let _ = p.sync_all(); }
}

/// Grava inteiro ou nada. Quem chama segura [`travar`].
pub fn gravar_atomico(destino: &Path, conteudo: &[u8]) -> io::Result<()> {
    let pasta = destino.parent().ok_or_else(|| invalido("caminho sem pasta"))?;
    criar_pasta(pasta)?;
    if fs::symlink_metadata(destino).is_ok_and(|m| m.file_type().is_symlink()) { return Err(invalido("Arquivo inválido.")); }
    let temporario = temporario_ao_lado(destino)?;
    let resultado = (|| {
        let mut arquivo = abrir_exclusivo(&temporario)?;
        arquivo.write_all(conteudo)?;
        arquivo.sync_all()?;
        fs::rename(&temporario, destino)?;
        sincronizar_pasta(pasta);
        Ok(())
    })();
    if resultado.is_err() { let _ = fs::remove_file(&temporario); }
    resultado
}

/// Copia um arquivo grande para `destino` (que não pode existir): clone do APFS quando dá (não ocupa
/// espaço), senão cópia comum num `.tmp` e rename. Nunca deixa um arquivo pela metade no destino.
pub fn copiar_atomico(origem: &Path, destino: &Path) -> io::Result<()> {
    let pasta = destino.parent().ok_or_else(|| invalido("caminho sem pasta"))?;
    criar_pasta(pasta)?;
    let temporario = temporario_ao_lado(destino)?;
    let resultado = (|| {
        if !omni_core::clonar_arquivo::clonar(origem, &temporario)? {
            let mut entrada = fs::File::open(origem)?;
            let mut saida = abrir_exclusivo(&temporario)?;
            io::copy(&mut entrada, &mut saida)?;
            saida.sync_all()?;
        }
        if destino.exists() { return Err(io::Error::new(io::ErrorKind::AlreadyExists, "destino já existe")); }
        fs::rename(&temporario, destino)?;
        sincronizar_pasta(pasta);
        Ok(())
    })();
    if resultado.is_err() { let _ = fs::remove_file(&temporario); }
    resultado
}

/// Arquivo comum (link não conta) e o tamanho dele.
pub fn arquivo_comum(caminho: &Path) -> Option<u64> {
    fs::symlink_metadata(caminho).ok().filter(|m| m.file_type().is_file()).map(|m| m.len())
}

pub fn pasta_comum(caminho: &Path) -> bool {
    fs::symlink_metadata(caminho).map(|m| m.file_type().is_dir()).unwrap_or(false)
}

/// SHA-256 em hexadecimal, lido aos pedaços (vídeo de vários GB não vai para a memória).
pub fn sha256_do_arquivo(caminho: &Path) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut arquivo = fs::File::open(caminho)?;
    let mut hash = Sha256::new();
    let mut bloco = vec![0u8; 1 << 20];
    loop {
        let lidos = arquivo.read(&mut bloco)?;
        if lidos == 0 { break; }
        hash.update(&bloco[..lidos]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn ler_json(caminho: &Path) -> Option<serde_json::Value> {
    arquivo_comum(caminho)?;
    serde_json::from_str(&fs::read_to_string(caminho).ok()?).ok()
}

pub fn gravar_json(caminho: &Path, valor: &serde_json::Value) -> io::Result<()> {
    let texto = serde_json::to_string_pretty(valor).map_err(io::Error::other)?;
    gravar_atomico(caminho, texto.as_bytes())
}

/// Nome de arquivo seguro para `assets/`: minúsculas, `[a-z0-9._-]`, sem começar com ponto ou traço,
/// extensão preservada, até 80 caracteres.
pub fn nome_seguro(original: &str) -> String {
    let base = original.rsplit(['/', '\\']).next().unwrap_or(original);
    let (tronco, extensao) = match base.rsplit_once('.') {
        Some((t, e)) if !t.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()) => (t, Some(e.to_ascii_lowercase())),
        _ => (base, None),
    };
    let mut limpo = String::new();
    for c in tronco.chars() {
        let c = sem_acento(c).to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' { limpo.push(c); }
        else if !limpo.ends_with('-') { limpo.push('-'); }
    }
    let limpo: String = limpo.trim_matches(['-', '.']).chars().take(64).collect();
    let limpo = if limpo.is_empty() { "arquivo".to_owned() } else { limpo };
    match extensao { Some(e) => format!("{limpo}.{e}"), None => limpo }
}

fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        outro => outro,
    }
}

/// Tipo da mídia pela extensão (o front mostra o ícone e o agente sabe o que é).
pub fn tipo_pela_extensao(nome: &str) -> &'static str {
    let extensao = nome.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match extensao.as_str() {
        "mp4" | "mov" | "webm" | "m4v" | "mkv" | "avi" => "video",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" | "heic" => "imagem",
        "mp3" | "wav" | "m4a" | "aac" | "ogg" | "flac" | "opus" => "audio",
        "ttf" | "otf" | "woff" | "woff2" => "fonte",
        "json" | "lottie" => "animacao",
        "srt" | "vtt" | "txt" | "md" => "texto",
        _ => "outro",
    }
}

/// Copia os arquivos de texto pequenos de `pasta` (recursivo, sem link e sem as subpastas em
/// `pular`) para `destino`. Devolve quantos copiou. Usado nos backups: mídia grande fica de fora.
pub fn copiar_textos(pasta: &Path, destino: &Path, pular: &[&str], limite: u64) -> io::Result<usize> {
    fn andar(base: &Path, rel: &Path, destino: &Path, pular: &[&str], limite: u64, n: &mut usize) -> io::Result<()> {
        let atual = base.join(rel);
        let Ok(itens) = fs::read_dir(&atual) else { return Ok(()) };
        for item in itens.flatten() {
            let nome = item.file_name();
            let Some(nome_txt) = nome.to_str() else { continue };
            if nome_txt.ends_with(".tmp") { continue; }
            let rel_item = rel.join(nome_txt);
            let rel_txt = rel_item.to_string_lossy().replace('\\', "/");
            if pular.iter().any(|p| rel_txt == *p) { continue; }
            let Ok(meta) = fs::symlink_metadata(item.path()) else { continue };
            if meta.file_type().is_dir() {
                andar(base, &rel_item, destino, pular, limite, n)?;
            } else if meta.file_type().is_file() && meta.len() <= limite {
                let alvo = destino.join(&rel_item);
                if let Some(p) = alvo.parent() { fs::create_dir_all(p)?; }
                fs::copy(item.path(), alvo)?;
                *n += 1;
            }
        }
        Ok(())
    }
    let mut n = 0;
    if pasta_comum(pasta) { andar(pasta, Path::new(""), destino, pular, limite, &mut n)?; }
    Ok(n)
}
