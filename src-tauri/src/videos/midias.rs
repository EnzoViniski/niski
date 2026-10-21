//! Fork (Editor de vídeo): mídias de um projeto ou de uma pasta de referências.
//!
//! Cada arquivo importado vai para `assets/` (projeto) ou `arquivos/` (pasta de referências) com
//! nome seguro, por clone do APFS quando dá (não ocupa espaço). A lista (`midias.json` ou
//! `pasta.json`, campo `itens`) guarda o SHA-256 e o tamanho: os backups levam só a lista, e o app
//! avisa quando um arquivo sumiu ou mudou. O mesmo arquivo duas vezes vira um item só.

use std::fs;
use std::io;
use std::path::Path;

use super::arquivos::{self, invalido, travar};

/// Papel da mídia: entra no vídeo, ou só mostra ao agente o estilo que a pessoa quer.
pub fn papel_valido(papel: &str) -> bool { matches!(papel, "usar" | "referencia") }

/// Importa `origem` para `pasta_dos_arquivos` e registra em `lista` (JSON com `itens`).
/// `prefixo` é como o caminho aparece no item (`assets/` ou `arquivos/`). Devolve o item.
pub fn importar(lista: &Path, pasta_dos_arquivos: &Path, prefixo: &str, origem: &Path, papel: &str, nota: Option<&str>, agora_ms: u128) -> io::Result<serde_json::Value> {
    if !papel_valido(papel) { return Err(invalido("Papel de mídia inválido.")); }
    let meta = fs::metadata(origem).map_err(|_| invalido("Não achei esse arquivo."))?;
    if !meta.is_file() { return Err(invalido("Só dá para importar arquivos (pasta não).")); }
    let hash = arquivos::sha256_do_arquivo(origem)?;
    let original = origem.file_name().and_then(|n| n.to_str()).unwrap_or("arquivo").to_owned();

    // Já está na lista: devolve o mesmo item (atualiza o papel se mudou).
    {
        let _trava = travar();
        let mut valor = ler_lista(lista)?;
        if let Some(item) = valor["itens"].as_array_mut().and_then(|itens| itens.iter_mut().find(|i| i["hash"] == hash.as_str())) {
            let arquivo = item["arquivo"].as_str().unwrap_or_default().to_owned();
            let rel = arquivo.strip_prefix(prefixo).unwrap_or(&arquivo).to_owned();
            if arquivos::arquivo_comum(&pasta_dos_arquivos.join(&rel)).is_some() {
                item["papel"] = papel.into();
                let devolver = item.clone();
                arquivos::gravar_json(lista, &valor)?;
                return Ok(devolver);
            }
        }
    }

    let seguro = arquivos::nome_seguro(&original);
    let mut nome = seguro.clone();
    if pasta_dos_arquivos.join(&nome).exists() {
        let (tronco, extensao) = seguro.rsplit_once('.').map(|(t, e)| (t.to_owned(), format!(".{e}"))).unwrap_or((seguro.clone(), String::new()));
        nome = format!("{tronco}-{}{extensao}", &hash[..8]);
        let mut n = 1;
        while pasta_dos_arquivos.join(&nome).exists() { n += 1; nome = format!("{tronco}-{}-{n}{extensao}", &hash[..8]); }
    }
    arquivos::copiar_atomico(origem, &pasta_dos_arquivos.join(&nome))?;
    let item = serde_json::json!({
        "id": &hash[..16],
        "nome": original,
        "arquivo": format!("{prefixo}{nome}"),
        "tipo": arquivos::tipo_pela_extensao(&nome),
        "papel": papel,
        "hash": hash,
        "tamanho": meta.len(),
        "nota": nota.map(|n| n.chars().take(500).collect::<String>()).unwrap_or_default(),
        "adicionadoEm": agora_ms as u64,
    });
    let _trava = travar();
    let mut valor = ler_lista(lista)?;
    valor["itens"].as_array_mut().ok_or_else(|| invalido("Lista de mídias inválida."))?.push(item.clone());
    arquivos::gravar_json(lista, &valor)?;
    Ok(item)
}

fn ler_lista(lista: &Path) -> io::Result<serde_json::Value> {
    let mut valor = arquivos::ler_json(lista).filter(|v| v.is_object()).ok_or_else(|| invalido("Lista de mídias inválida."))?;
    if !valor["itens"].is_array() { valor["itens"] = serde_json::json!([]); }
    if !valor["links"].is_array() { valor["links"] = serde_json::json!([]); }
    Ok(valor)
}

/// Itens cujo arquivo sumiu ou mudou de tamanho (`{id, nome, arquivo, motivo}`). Só tamanho, sem
/// reler o hash: abrir o projeto não lê gigabytes.
pub fn faltando(lista: &Path, pasta_dos_arquivos: &Path, prefixo: &str) -> Vec<serde_json::Value> {
    let Ok(valor) = ler_lista(lista) else { return Vec::new() };
    valor["itens"].as_array().into_iter().flatten().filter_map(|item| {
        let arquivo = item["arquivo"].as_str()?;
        let rel = arquivo.strip_prefix(prefixo)?;
        if rel.contains("..") || rel.contains('/') || rel.contains('\\') { return None; }
        let motivo = match arquivos::arquivo_comum(&pasta_dos_arquivos.join(rel)) {
            None => "sumiu",
            Some(t) if item["tamanho"].as_u64().is_some_and(|esperado| esperado != t) => "mudou",
            Some(_) => return None,
        };
        Some(serde_json::json!({ "id": item["id"], "nome": item["nome"], "arquivo": arquivo, "motivo": motivo }))
    }).collect()
}

/// Tira o item da lista e manda o arquivo para `lixeira` (nada some de verdade). Devolve se achou.
pub fn remover(lista: &Path, pasta_dos_arquivos: &Path, prefixo: &str, id: &str, lixeira: &Path, carimbo: &str) -> io::Result<bool> {
    let _trava = travar();
    let mut valor = ler_lista(lista)?;
    let itens = valor["itens"].as_array_mut().ok_or_else(|| invalido("Lista de mídias inválida."))?;
    let Some(posicao) = itens.iter().position(|i| i["id"] == id) else { return Ok(false) };
    let item = itens.remove(posicao);
    if let Some(rel) = item["arquivo"].as_str().and_then(|a| a.strip_prefix(prefixo)).filter(|r| !r.contains(['/', '\\']) && !r.contains("..")) {
        let caminho = pasta_dos_arquivos.join(rel);
        if arquivos::arquivo_comum(&caminho).is_some() {
            arquivos::criar_pasta(lixeira)?;
            let destino = lixeira.join(format!("{carimbo}-{rel}"));
            if fs::rename(&caminho, &destino).is_err() { arquivos::copiar_atomico(&caminho, &destino)?; fs::remove_file(&caminho)?; }
        }
    }
    arquivos::gravar_json(lista, &valor)?;
    Ok(true)
}

/// Copia os itens de uma lista para outra pasta (pasta de referências → projeto, ou o contrário),
/// pelo mesmo caminho de [`importar`]: sem duplicar o que já está lá. Devolve os itens copiados.
pub fn copiar_itens(origem_lista: &Path, origem_pasta: &Path, origem_prefixo: &str, destino_lista: &Path, destino_pasta: &Path, destino_prefixo: &str, papel: Option<&str>, agora_ms: u128) -> io::Result<Vec<serde_json::Value>> {
    let valor = ler_lista(origem_lista)?;
    let mut copiados = Vec::new();
    for item in valor["itens"].as_array().into_iter().flatten() {
        let Some(rel) = item["arquivo"].as_str().and_then(|a| a.strip_prefix(origem_prefixo)) else { continue };
        if rel.contains(['/', '\\']) || rel.contains("..") { continue; }
        let caminho = origem_pasta.join(rel);
        if arquivos::arquivo_comum(&caminho).is_none() { continue; }
        let papel = papel.or(item["papel"].as_str()).filter(|p| papel_valido(p)).unwrap_or("referencia");
        copiados.push(importar(destino_lista, destino_pasta, destino_prefixo, &caminho, papel, item["nota"].as_str(), agora_ms)?);
    }
    // Links também vão junto, sem repetir.
    let links = valor["links"].as_array().cloned().unwrap_or_default();
    if !links.is_empty() {
        let _trava = travar();
        let mut destino = ler_lista(destino_lista)?;
        let lista = destino["links"].as_array_mut().ok_or_else(|| invalido("Lista de links inválida."))?;
        for link in links {
            if link["url"].as_str().is_some_and(|u| !lista.iter().any(|l| l["url"].as_str() == Some(u))) { lista.push(link); }
        }
        arquivos::gravar_json(destino_lista, &destino)?;
    }
    Ok(copiados)
}
