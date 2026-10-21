//! Fork (Editor de vídeo): pastas de referências reaproveitáveis ("Reels da Medza", "Tutorial
//! calmo"...). Cada uma guarda vídeos, fotos, fontes, áudio, links e uma descrição do estilo; ao
//! criar ou editar um projeto, a pessoa escolhe a pasta e tudo entra no projeto de uma vez.
//!
//! ```text
//! <dados>/videos/referencias/<id>/pasta.json    nome, descrição, cor, itens (hash) e links
//! <dados>/videos/referencias/<id>/arquivos/     os arquivos (clone do APFS quando dá)
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::arquivos::{self, criar_pasta, invalido, travar};

pub const REFERENCIAS: &str = "referencias";
pub const PASTA_JSON: &str = "pasta.json";
pub const ARQUIVOS: &str = "arquivos";
pub const PREFIXO: &str = "arquivos/";

pub fn pasta(raiz: &Path, id: &str) -> io::Result<PathBuf> {
    if !arquivos::id_valido(id) { return Err(invalido("Pasta de referências inválida.")); }
    Ok(raiz.join(REFERENCIAS).join(id))
}

pub fn existente(raiz: &Path, id: &str) -> io::Result<PathBuf> {
    let p = pasta(raiz, id)?;
    if arquivos::arquivo_comum(&p.join(PASTA_JSON)).is_none() { return Err(invalido("Essa pasta de referências não existe.")); }
    Ok(p)
}

/// Todas as pastas, da editada mais recentemente para a mais antiga.
pub fn listar(raiz: &Path) -> Vec<serde_json::Value> {
    let mut lista: Vec<serde_json::Value> = fs::read_dir(raiz.join(REFERENCIAS)).into_iter().flatten().flatten()
        .filter_map(|item| {
            let id = item.file_name().to_str()?.to_owned();
            if !arquivos::id_valido(&id) || !arquivos::pasta_comum(&item.path()) { return None; }
            arquivos::ler_json(&item.path().join(PASTA_JSON)).filter(|v| v.is_object())
        })
        .collect();
    lista.sort_by_key(|v| std::cmp::Reverse(v["atualizadoEm"].as_u64().unwrap_or(0)));
    lista
}

/// Campos que o front pode mudar numa pasta (os itens só mudam por importar/remover).
fn aplicar_campos(destino: &mut serde_json::Value, origem: &serde_json::Value) {
    for (campo, limite) in [("nome", 80), ("descricao", 4000), ("cor", 16)] {
        if let Some(texto) = origem[campo].as_str() { destino[campo] = texto.chars().take(limite).collect::<String>().into(); }
    }
    if let Some(links) = origem["links"].as_array() {
        let limpos: Vec<serde_json::Value> = links.iter().filter_map(|l| {
            let url = l["url"].as_str()?.trim();
            if !(url.starts_with("https://") || url.starts_with("http://")) || url.len() > 2000 { return None; }
            Some(serde_json::json!({ "url": url, "nota": l["nota"].as_str().unwrap_or_default().chars().take(500).collect::<String>() }))
        }).take(200).collect();
        destino["links"] = limpos.into();
    }
}

pub fn criar(raiz: &Path, dados: &serde_json::Value, agora_ms: u128, aleatorio: u32) -> io::Result<serde_json::Value> {
    if dados["nome"].as_str().is_none_or(|n| n.trim().is_empty()) { return Err(invalido("Dê um nome para a pasta.")); }
    let base = raiz.join(REFERENCIAS);
    let id = arquivos::novo_id(&base, "r", agora_ms, aleatorio);
    let mut valor = serde_json::json!({ "versao": 1, "id": id, "nome": "", "descricao": "", "cor": "#7c9cff", "itens": [], "links": [],
        "criadoEm": agora_ms as u64, "atualizadoEm": agora_ms as u64 });
    aplicar_campos(&mut valor, dados);
    let _trava = travar();
    criar_pasta(&base.join(&id).join(ARQUIVOS))?;
    arquivos::gravar_json(&base.join(&id).join(PASTA_JSON), &valor)?;
    Ok(valor)
}

pub fn editar(raiz: &Path, id: &str, dados: &serde_json::Value, agora_ms: u128) -> io::Result<serde_json::Value> {
    let p = existente(raiz, id)?;
    let _trava = travar();
    let mut valor = arquivos::ler_json(&p.join(PASTA_JSON)).filter(|v| v.is_object()).ok_or_else(|| invalido("Pasta de referências inválida."))?;
    aplicar_campos(&mut valor, dados);
    valor["atualizadoEm"] = (agora_ms as u64).into();
    arquivos::gravar_json(&p.join(PASTA_JSON), &valor)?;
    Ok(valor)
}

pub fn ler(raiz: &Path, id: &str) -> io::Result<serde_json::Value> {
    let p = existente(raiz, id)?;
    arquivos::ler_json(&p.join(PASTA_JSON)).filter(|v| v.is_object()).ok_or_else(|| invalido("Pasta de referências inválida."))
}

fn tocar(p: &Path, agora_ms: u128) {
    let _trava = travar();
    if let Some(mut valor) = arquivos::ler_json(&p.join(PASTA_JSON)).filter(|v| v.is_object()) {
        valor["atualizadoEm"] = (agora_ms as u64).into();
        let _ = arquivos::gravar_json(&p.join(PASTA_JSON), &valor);
    }
}

pub fn importar(raiz: &Path, id: &str, origem: &Path, papel: &str, nota: Option<&str>, agora_ms: u128) -> io::Result<serde_json::Value> {
    let p = existente(raiz, id)?;
    let item = super::midias::importar(&p.join(PASTA_JSON), &p.join(ARQUIVOS), PREFIXO, origem, papel, nota, agora_ms)?;
    tocar(&p, agora_ms);
    Ok(item)
}

pub fn remover_item(raiz: &Path, id: &str, item: &str, carimbo: &str) -> io::Result<bool> {
    let p = existente(raiz, id)?;
    super::midias::remover(&p.join(PASTA_JSON), &p.join(ARQUIVOS), PREFIXO, item, &raiz.join(super::projetos::LIXEIRA).join("midias"), carimbo)
}

pub fn mandar_para_a_lixeira(raiz: &Path, id: &str, carimbo: &str) -> io::Result<PathBuf> {
    let p = existente(raiz, id)?;
    let lixeira = raiz.join(super::projetos::LIXEIRA);
    criar_pasta(&lixeira)?;
    let mut destino = lixeira.join(format!("{id}-{carimbo}"));
    let mut n = 1;
    while destino.exists() { n += 1; destino = lixeira.join(format!("{id}-{carimbo}-{n}")); }
    let _trava = travar();
    fs::rename(&p, &destino)?;
    Ok(destino)
}

/// Backup da atualização: só o `pasta.json` de cada pasta (os arquivos ficam pelo hash).
pub fn copiar_tudo(raiz: &Path, destino: &Path) -> io::Result<usize> {
    let mut n = 0;
    for item in fs::read_dir(raiz.join(REFERENCIAS)).into_iter().flatten().flatten() {
        let Some(id) = item.file_name().to_str().map(str::to_owned) else { continue };
        if !arquivos::id_valido(&id) || !arquivos::pasta_comum(&item.path()) { continue; }
        let origem = item.path().join(PASTA_JSON);
        if arquivos::arquivo_comum(&origem).is_none() { continue; }
        let alvo = destino.join(REFERENCIAS).join(&id);
        fs::create_dir_all(&alvo)?;
        fs::copy(origem, alvo.join(PASTA_JSON))?;
        n += 1;
    }
    Ok(n)
}
