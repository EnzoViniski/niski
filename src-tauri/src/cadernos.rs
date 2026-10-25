//! Fork (Caderno): cadernos de anotações (um livro, um estudo, a vida...) com páginas em Markdown e
//! a conversa com a IA ao lado. Tudo em `<pasta de dados>/cadernos/<id>/`:
//!
//! - `meta.json`: nome, tipo, cor, papel, IA escolhida e a lista de páginas (o front valida);
//! - `paginas/<id>.md`: o texto de cada página;
//! - `conversa.jsonl`: uma mensagem por linha, só acrescentada.
//!
//! Apagar nunca apaga: a página ou o caderno vão para `cadernos/.lixeira/` com carimbo de data.
//! A gravação reaproveita o jeito dos roadmaps (inteiro ou nada, só para o dono).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub const PASTA: &str = "cadernos";
const LIXEIRA: &str = ".lixeira";
const META: &str = "meta.json";
const CONVERSA: &str = "conversa.jsonl";
const TAMANHO_MAXIMO: usize = 8 * 1024 * 1024;
const LINHA_MAXIMA: usize = 256 * 1024;

static TRAVA: Mutex<()> = Mutex::new(());

fn invalido(mensagem: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, mensagem.to_owned()) }

/// Id de caderno ou de página: `[a-z0-9-]`, 1 a 64, sem começar com `-` (vira nome de pasta/arquivo).
pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && !id.starts_with('-')
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn arquivo_valido(arquivo: &str) -> bool {
    arquivo == META || arquivo == CONVERSA
        || arquivo.strip_prefix("paginas/").and_then(|r| r.strip_suffix(".md")).is_some_and(id_valido)
}

fn caminho(raiz: &Path, id: &str, arquivo: &str) -> io::Result<PathBuf> {
    if !id_valido(id) { return Err(invalido("Caderno inválido.")); }
    if !arquivo_valido(arquivo) { return Err(invalido("Arquivo de caderno inválido.")); }
    Ok(raiz.join(id).join(arquivo))
}

fn criar_pasta(pasta: &Path) -> io::Result<()> {
    if pasta.is_dir() { return Ok(()); }
    fs::create_dir_all(pasta)?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(pasta, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn gravar_atomico(destino: &Path, conteudo: &[u8]) -> io::Result<()> {
    let pasta = destino.parent().ok_or_else(|| invalido("caminho sem pasta"))?;
    criar_pasta(pasta)?;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCIA: AtomicU64 = AtomicU64::new(0);
    let nome = destino.file_name().and_then(|n| n.to_str()).unwrap_or("arquivo");
    let temporario = pasta.join(format!(".{nome}.{}.{}.tmp", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    let resultado = (|| {
        let mut arquivo = opcoes.open(&temporario)?;
        arquivo.write_all(conteudo)?;
        arquivo.sync_all()?;
        fs::rename(&temporario, destino)?;
        if let Ok(p) = fs::File::open(pasta) { let _ = p.sync_all(); }
        Ok(())
    })();
    if resultado.is_err() { let _ = fs::remove_file(&temporario); }
    resultado
}

fn e_arquivo(caminho: &Path) -> bool {
    fs::symlink_metadata(caminho).map(|m| m.file_type().is_file()).unwrap_or(false)
}

fn e_pasta(caminho: &Path) -> bool {
    fs::symlink_metadata(caminho).map(|m| m.file_type().is_dir()).unwrap_or(false)
}

/// O `meta.json` de cada caderno (o front normaliza). Pasta sem meta ou com meta quebrado fica de fora.
pub fn listar(raiz: &Path) -> Vec<serde_json::Value> {
    let Ok(itens) = fs::read_dir(raiz) else { return Vec::new() };
    let mut lista: Vec<(String, serde_json::Value)> = itens.flatten()
        .filter_map(|item| {
            let id = item.file_name().to_str()?.to_owned();
            if !id_valido(&id) || !e_pasta(&item.path()) { return None; }
            let meta = item.path().join(META);
            if !e_arquivo(&meta) { return None; }
            let valor = serde_json::from_str::<serde_json::Value>(&fs::read_to_string(meta).ok()?).ok()?;
            valor.is_object().then_some((id, valor))
        })
        .collect();
    lista.sort_by(|a, b| a.0.cmp(&b.0));
    lista.into_iter().map(|(_, v)| v).collect()
}

pub fn ler(raiz: &Path, id: &str, arquivo: &str) -> io::Result<Option<String>> {
    let caminho = caminho(raiz, id, arquivo)?;
    match fs::symlink_metadata(&caminho) {
        Ok(meta) if !meta.file_type().is_file() => return Err(invalido("Arquivo de caderno inválido.")),
        Ok(meta) if meta.len() as usize > TAMANHO_MAXIMO => return Err(invalido("Arquivo grande demais.")),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(erro) => return Err(erro),
        Ok(_) => {}
    }
    fs::read_to_string(caminho).map(Some)
}

/// Grava inteiro. O `meta.json` cria o caderno; os outros arquivos exigem que ele exista.
pub fn gravar(raiz: &Path, id: &str, arquivo: &str, conteudo: &str) -> io::Result<()> {
    let destino = caminho(raiz, id, arquivo)?;
    if arquivo == CONVERSA { return Err(invalido("A conversa só recebe mensagens novas.")); }
    if conteudo.len() > TAMANHO_MAXIMO { return Err(invalido("Arquivo grande demais.")); }
    if arquivo == META && !serde_json::from_str::<serde_json::Value>(conteudo).map(|v| v.is_object()).unwrap_or(false) {
        return Err(invalido("Conteúdo não é JSON válido."));
    }
    if arquivo != META && !e_arquivo(&raiz.join(id).join(META)) { return Err(invalido("Esse caderno não existe.")); }
    let _trava = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
    gravar_atomico(&destino, conteudo.as_bytes())
}

/// Acrescenta uma mensagem no fim da conversa, sem reescrever as anteriores.
pub fn anexar_na_conversa(raiz: &Path, id: &str, linha: &str) -> io::Result<()> {
    let linha = linha.trim();
    if linha.is_empty() || linha.len() > LINHA_MAXIMA || linha.contains('\n') { return Err(invalido("Mensagem inválida.")); }
    if serde_json::from_str::<serde_json::Value>(linha).is_err() { return Err(invalido("Mensagem não é JSON válido.")); }
    let destino = caminho(raiz, id, CONVERSA)?;
    if !e_arquivo(&raiz.join(id).join(META)) { return Err(invalido("Esse caderno não existe.")); }
    let _trava = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
    let tamanho = fs::metadata(&destino).map(|m| m.len() as usize).unwrap_or(0);
    if tamanho + linha.len() > TAMANHO_MAXIMO { return Err(invalido("A conversa deste caderno chegou no limite.")); }
    let mut opcoes = fs::OpenOptions::new();
    opcoes.create(true).append(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    let mut arquivo = opcoes.open(&destino)?;
    arquivo.write_all(format!("{linha}\n").as_bytes())?;
    arquivo.sync_all()
}

/// Leva uma página (`paginas/<id>.md`) ou o caderno inteiro (`pagina = None`) para a lixeira.
pub fn para_a_lixeira(raiz: &Path, id: &str, pagina: Option<&str>, carimbo: &str) -> io::Result<PathBuf> {
    let origem = match pagina {
        Some(p) => caminho(raiz, id, &format!("paginas/{p}.md"))?,
        None => { if !id_valido(id) { return Err(invalido("Caderno inválido.")); } raiz.join(id) }
    };
    let existe = if pagina.is_some() { e_arquivo(&origem) } else { e_pasta(&origem) };
    if !existe { return Err(invalido("Não encontrei o que apagar.")); }
    let _trava = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
    let lixeira = raiz.join(LIXEIRA);
    criar_pasta(&lixeira)?;
    let base = match pagina { Some(p) => format!("{carimbo}-{id}-{p}.md"), None => format!("{carimbo}-{id}") };
    let mut destino = lixeira.join(&base);
    let mut n = 1;
    while destino.exists() { n += 1; destino = lixeira.join(format!("{n}-{base}")); }
    fs::rename(&origem, &destino)?;
    Ok(destino)
}

/// Copia os cadernos (sem a lixeira e sem links) para o backup da atualização. Devolve quantos arquivos.
pub fn copiar_tudo(raiz: &Path, destino: &Path) -> io::Result<usize> {
    if !e_pasta(raiz) { return Ok(0); }
    let mut copiados = 0;
    for item in fs::read_dir(raiz)?.flatten() {
        let Some(id) = item.file_name().to_str().map(str::to_owned) else { continue };
        if !id_valido(&id) || !e_pasta(&item.path()) { continue; }
        for rel in [META.to_owned(), CONVERSA.to_owned()].into_iter().chain(paginas(&item.path())) {
            let de = item.path().join(&rel);
            if !e_arquivo(&de) { continue; }
            let para = destino.join(&id).join(&rel);
            if let Some(p) = para.parent() { fs::create_dir_all(p)?; }
            fs::copy(&de, para)?;
            copiados += 1;
        }
    }
    Ok(copiados)
}

fn paginas(pasta: &Path) -> Vec<String> {
    if !e_pasta(&pasta.join("paginas")) { return Vec::new(); }
    fs::read_dir(pasta.join("paginas")).map(|itens| itens.flatten()
        .filter_map(|i| i.file_name().to_str().map(|n| format!("paginas/{n}")))
        .filter(|rel| arquivo_valido(rel))
        .collect()).unwrap_or_default()
}

// ---- Comandos ----

fn raiz() -> Result<PathBuf, String> {
    Ok(crate::engine_client::engine_dir()?.join(PASTA))
}

fn texto_do_erro(erro: io::Error) -> String {
    if erro.kind() == io::ErrorKind::InvalidInput { erro.to_string() } else { format!("Não consegui gravar o caderno: {erro}") }
}

#[tauri::command(async)]
pub fn cadernos_listar() -> Result<Vec<serde_json::Value>, String> { Ok(listar(&raiz()?)) }

#[tauri::command(async)]
pub fn cadernos_ler(id: String, arquivo: String) -> Result<Option<String>, String> {
    ler(&raiz()?, &id, &arquivo).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn cadernos_gravar(id: String, arquivo: String, conteudo: String) -> Result<(), String> {
    gravar(&raiz()?, &id, &arquivo, &conteudo).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn cadernos_anexar_conversa(id: String, linha: String) -> Result<(), String> {
    anexar_na_conversa(&raiz()?, &id, &linha).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn cadernos_para_a_lixeira(id: String, pagina: Option<String>) -> Result<(), String> {
    let carimbo = crate::backup_antes_de_atualizar::carimbo(SystemTime::now());
    para_a_lixeira(&raiz()?, &id, pagina.as_deref(), &carimbo).map(|_| ()).map_err(texto_do_erro)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn meta(id: &str) -> String { format!(r#"{{"versao":1,"id":"{id}","nome":"Livro"}}"#) }

    #[test]
    fn grava_le_e_lista_so_cadernos_validos() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path();
        gravar(raiz, "c-1", "meta.json", &meta("c-1")).unwrap();
        gravar(raiz, "c-1", "paginas/p-1.md", "# Capítulo 1").unwrap();
        assert_eq!(ler(raiz, "c-1", "paginas/p-1.md").unwrap().as_deref(), Some("# Capítulo 1"));
        assert_eq!(ler(raiz, "c-1", "paginas/p-2.md").unwrap(), None);
        fs::create_dir_all(raiz.join("sem-meta")).unwrap();
        fs::create_dir_all(raiz.join("Maiusculo")).unwrap();
        fs::write(raiz.join("Maiusculo/meta.json"), "{}").unwrap();
        let lista = listar(raiz);
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0]["id"], "c-1");
    }

    #[test]
    fn recusa_caminhos_fora_da_pasta_e_pagina_sem_caderno() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path();
        assert!(gravar(raiz, "..", "meta.json", "{}").is_err());
        assert!(gravar(raiz, "c-1", "../fora.md", "x").is_err());
        assert!(gravar(raiz, "c-1", "paginas/../../x.md", "x").is_err());
        assert!(gravar(raiz, "c-1", "paginas/p-1.md", "x").is_err(), "sem meta não cria página");
        assert!(gravar(raiz, "c-1", "meta.json", "[1]").is_err());
        gravar(raiz, "c-1", "meta.json", &meta("c-1")).unwrap();
        assert!(gravar(raiz, "c-1", "conversa.jsonl", "{}").is_err(), "conversa só por anexar");
    }

    #[test]
    fn conversa_so_acrescenta_linhas_json() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path();
        gravar(raiz, "c-1", "meta.json", &meta("c-1")).unwrap();
        anexar_na_conversa(raiz, "c-1", r#"{"de":"eu","texto":"oi"}"#).unwrap();
        anexar_na_conversa(raiz, "c-1", r#"{"de":"ia","texto":"olá"}"#).unwrap();
        assert!(anexar_na_conversa(raiz, "c-1", "não é json").is_err());
        assert!(anexar_na_conversa(raiz, "c-1", "{}\n{}").is_err());
        assert_eq!(ler(raiz, "c-1", "conversa.jsonl").unwrap().unwrap().lines().count(), 2);
    }

    #[test]
    fn apagar_leva_para_a_lixeira_sem_perder_nada() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path();
        gravar(raiz, "c-1", "meta.json", &meta("c-1")).unwrap();
        gravar(raiz, "c-1", "paginas/p-1.md", "texto").unwrap();
        let pagina = para_a_lixeira(raiz, "c-1", Some("p-1"), "2026-10-25").unwrap();
        assert_eq!(fs::read_to_string(pagina).unwrap(), "texto");
        assert_eq!(ler(raiz, "c-1", "paginas/p-1.md").unwrap(), None);
        let caderno = para_a_lixeira(raiz, "c-1", None, "2026-10-25").unwrap();
        assert!(caderno.join("meta.json").is_file());
        assert!(listar(raiz).is_empty(), "a lixeira não aparece na lista");
        assert!(para_a_lixeira(raiz, "c-1", None, "2026-10-25").is_err());
    }

    #[test]
    fn backup_copia_paginas_e_conversa_mas_nao_a_lixeira() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("cadernos");
        gravar(&raiz, "c-1", "meta.json", &meta("c-1")).unwrap();
        gravar(&raiz, "c-1", "paginas/p-1.md", "a").unwrap();
        anexar_na_conversa(&raiz, "c-1", r#"{"de":"eu","texto":"oi"}"#).unwrap();
        gravar(&raiz, "c-2", "meta.json", &meta("c-2")).unwrap();
        para_a_lixeira(&raiz, "c-2", None, "x").unwrap();
        let destino = dir.path().join("backup");
        assert_eq!(copiar_tudo(&raiz, &destino).unwrap(), 3);
        assert!(destino.join("c-1/paginas/p-1.md").is_file());
        assert!(!destino.join(".lixeira").exists());
    }
}
