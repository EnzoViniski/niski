//! Fork (Editor de vídeo): cada projeto é uma pasta HyperFrames legível fora do app.
//!
//! ```text
//! <dados>/videos/
//!   indice.json                       ordem da lista de projetos
//!   projetos/<id>/projeto.json        nome, formato, IA escolhida, versão do formato
//!   projetos/<id>/BRIEF.md            o pedido: estilo, duração, formato, público, referências
//!   projetos/<id>/index.html          composição raiz (HyperFrames)
//!   projetos/<id>/compositions/*.html sub-composições
//!   projetos/<id>/midias.json         mídias (hash, tamanho, papel) e links de referência
//!   projetos/<id>/assets/             arquivos das mídias (clone do APFS quando dá)
//!   projetos/<id>/conversa.jsonl      chat com o agente, uma mensagem por linha (só cresce)
//!   projetos/<id>/historico/<n>/      versões para desfazer (só os arquivos de texto)
//!   projetos/<id>/renders/            exportações
//!   projetos/<id>/CLAUDE.md, AGENTS.md, .claude/skills/   instruções e skills do agente
//!   referencias/<id>/                 pastas de referências reaproveitáveis (`referencias.rs`)
//!   lixeira/                          projeto, pasta ou mídia apagados (nada some de verdade)
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::arquivos::{self, criar_pasta, gravar_atomico, invalido, travar};

pub const PASTA: &str = "videos";
pub const PROJETOS: &str = "projetos";
pub const LIXEIRA: &str = "lixeira";
const INDICE: &str = "indice.json";
pub const VERSAO_DO_FORMATO: u64 = 1;
/// Versão do HyperFrames fixada para a CLI, o player e as skills. Mudar exige conferir a doc.
pub const VERSAO_DO_HYPERFRAMES: &str = "0.8.183";

pub const PROJETO: &str = "projeto.json";
pub const BRIEF: &str = "BRIEF.md";
pub const INDEX: &str = "index.html";
pub const MIDIAS: &str = "midias.json";
pub const CONVERSA: &str = "conversa.jsonl";
const HISTORICO: &str = "historico";
/// Texto que o front e o agente leem e gravam. Fora disso só `compositions/<nome>.html`.
const ARQUIVOS: &[&str] = &[PROJETO, BRIEF, INDEX, MIDIAS, CONVERSA];
/// O que entra numa versão do histórico (desfazer): a composição e o pedido, não o chat.
const VERSIONADOS: &[&str] = &[BRIEF, INDEX, MIDIAS];
pub const TAMANHO_MAXIMO: usize = 8 * 1024 * 1024;
const LINHA_MAXIMA: usize = 512 * 1024;
const VERSOES_GUARDADAS: usize = 120;

fn nome_de_composicao(rel: &str) -> bool {
    rel.strip_prefix("compositions/").and_then(|n| n.strip_suffix(".html")).is_some_and(|nome| {
        !nome.is_empty() && nome.len() <= 64 && !nome.starts_with(['-', '.'])
            && nome.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

pub fn arquivo_valido(rel: &str) -> bool { ARQUIVOS.contains(&rel) || nome_de_composicao(rel) }

pub fn raiz_dos_projetos(raiz: &Path) -> PathBuf { raiz.join(PROJETOS) }

pub fn pasta_do_projeto(raiz: &Path, id: &str) -> io::Result<PathBuf> {
    if !arquivos::id_valido(id) { return Err(invalido("Projeto inválido.")); }
    Ok(raiz.join(PROJETOS).join(id))
}

fn existente(raiz: &Path, id: &str) -> io::Result<PathBuf> {
    let pasta = pasta_do_projeto(raiz, id)?;
    if arquivos::arquivo_comum(&pasta.join(PROJETO)).is_none() { return Err(invalido("Esse projeto não existe.")); }
    Ok(pasta)
}

fn conferir_conteudo(rel: &str, conteudo: &str) -> io::Result<()> {
    if conteudo.len() > TAMANHO_MAXIMO { return Err(invalido("Arquivo grande demais.")); }
    if rel.ends_with(".json") {
        let valor = serde_json::from_str::<serde_json::Value>(conteudo).map_err(|_| invalido("Conteúdo não é JSON válido."))?;
        if !valor.is_object() { return Err(invalido("Conteúdo não é JSON válido.")); }
    }
    if rel == CONVERSA {
        for linha in conteudo.lines().filter(|l| !l.trim().is_empty()) {
            if serde_json::from_str::<serde_json::Value>(linha).is_err() { return Err(invalido("Linha da conversa não é JSON válido.")); }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ItemDaLista {
    pub id: String,
    pub projeto: serde_json::Value,
}

fn ler_indice(raiz: &Path) -> Vec<String> {
    arquivos::ler_json(&raiz.join(INDICE))
        .and_then(|v| v["ordem"].as_array().cloned())
        .map(|ordem| ordem.into_iter().filter_map(|i| i.as_str().map(str::to_owned)).filter(|i| arquivos::id_valido(i)).collect())
        .unwrap_or_default()
}

/// Projetos na ordem do índice; pasta fora do índice entra no fim (nada some por causa do índice).
pub fn listar(raiz: &Path) -> Vec<ItemDaLista> {
    let mut ordem = ler_indice(raiz);
    let mut soltos: Vec<String> = fs::read_dir(raiz.join(PROJETOS)).map(|itens| itens.flatten()
        .filter(|i| i.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|i| i.file_name().to_str().map(str::to_owned))
        .filter(|nome| arquivos::id_valido(nome) && !ordem.contains(nome))
        .collect()).unwrap_or_default();
    soltos.sort();
    ordem.extend(soltos);
    let mut vistos = std::collections::HashSet::new();
    ordem.retain(|id| vistos.insert(id.clone()));
    ordem.into_iter().filter_map(|id| {
        let projeto = arquivos::ler_json(&raiz.join(PROJETOS).join(&id).join(PROJETO)).filter(|v| v.is_object())?;
        Some(ItemDaLista { id, projeto })
    }).collect()
}

pub fn ordenar(raiz: &Path, ordem: &[String]) -> io::Result<()> {
    if ordem.iter().any(|id| !arquivos::id_valido(id)) { return Err(invalido("Projeto inválido.")); }
    let _trava = travar();
    arquivos::gravar_json(&raiz.join(INDICE), &serde_json::json!({ "versao": 1, "ordem": ordem }))
}

/// O que o front manda para criar um projeto (o conteúdo é dele; aqui só a gravação segura).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NovoProjeto {
    pub projeto: serde_json::Value,
    pub brief: String,
    pub index: String,
    pub instrucoes: String,
}

/// Cria a pasta completa e põe o projeto no começo da lista. `skills`: pasta com as skills do
/// HyperFrames embutidas no app (copiadas para `.claude/skills/`). Devolve o id.
pub fn criar(raiz: &Path, novo: &NovoProjeto, skills: Option<&Path>, agora_ms: u128, aleatorio: u32) -> io::Result<String> {
    if !novo.projeto.is_object() || novo.projeto["nome"].as_str().is_none_or(|n| n.trim().is_empty()) {
        return Err(invalido("Projeto sem nome."));
    }
    for (rel, texto) in [(BRIEF, &novo.brief), (INDEX, &novo.index), ("CLAUDE.md", &novo.instrucoes)] {
        if texto.len() > TAMANHO_MAXIMO { return Err(invalido(&format!("{rel} grande demais."))); }
    }
    let projetos = raiz.join(PROJETOS);
    let id = arquivos::novo_id(&projetos, "v", agora_ms, aleatorio);
    let pasta = projetos.join(&id);
    let mut projeto = novo.projeto.clone();
    projeto["id"] = id.clone().into();
    projeto["versao"] = VERSAO_DO_FORMATO.into();
    projeto["hyperframes"] = VERSAO_DO_HYPERFRAMES.into();
    projeto["criadoEm"] = (agora_ms as u64).into();
    projeto["atualizadoEm"] = (agora_ms as u64).into();
    {
        let _trava = travar();
        for sub in ["", "assets", "compositions", "renders", HISTORICO] { criar_pasta(&pasta.join(sub))?; }
        arquivos::gravar_json(&pasta.join(PROJETO), &projeto)?;
        gravar_atomico(&pasta.join(BRIEF), novo.brief.as_bytes())?;
        gravar_atomico(&pasta.join(INDEX), novo.index.as_bytes())?;
        arquivos::gravar_json(&pasta.join(MIDIAS), &serde_json::json!({ "versao": 1, "itens": [], "links": [] }))?;
        // Claude lê CLAUDE.md; Codex, AGENTS.md. Mesmo texto nos dois.
        gravar_atomico(&pasta.join("CLAUDE.md"), novo.instrucoes.as_bytes())?;
        gravar_atomico(&pasta.join("AGENTS.md"), novo.instrucoes.as_bytes())?;
    }
    if let Some(skills) = skills { instalar_skills(&pasta, skills)?; }
    let mut ordem = ler_indice(raiz);
    ordem.retain(|i| i != &id);
    ordem.insert(0, id.clone());
    ordenar(raiz, &ordem)?;
    Ok(id)
}

/// Copia (clone do APFS quando dá) as skills embutidas para `.claude/skills/` do projeto. Já
/// instaladas ficam como estão; troca só quando a versão fixada do HyperFrames muda.
pub fn instalar_skills(pasta: &Path, skills: &Path) -> io::Result<usize> {
    if !arquivos::pasta_comum(skills) { return Ok(0); }
    let destino = pasta.join(".claude").join("skills");
    let marca = destino.join(".versao-medza");
    if fs::read_to_string(&marca).is_ok_and(|v| v.trim() == VERSAO_DO_HYPERFRAMES) { return Ok(0); }
    let mut n = 0;
    fn andar(origem: &Path, destino: &Path, n: &mut usize) -> io::Result<()> {
        for item in fs::read_dir(origem)?.flatten() {
            let Ok(meta) = fs::symlink_metadata(item.path()) else { continue };
            let alvo = destino.join(item.file_name());
            if meta.is_dir() { criar_pasta(&alvo)?; andar(&item.path(), &alvo, n)?; }
            else if meta.file_type().is_file() {
                let _ = fs::remove_file(&alvo);
                arquivos::copiar_atomico(&item.path(), &alvo)?;
                *n += 1;
            }
        }
        Ok(())
    }
    criar_pasta(&destino)?;
    andar(skills, &destino, &mut n)?;
    let _trava = travar();
    gravar_atomico(&marca, VERSAO_DO_HYPERFRAMES.as_bytes())?;
    Ok(n)
}

/// Texto do arquivo, ou `None` se ainda não existe.
pub fn ler(raiz: &Path, id: &str, rel: &str) -> io::Result<Option<String>> {
    if !arquivo_valido(rel) { return Err(invalido("Arquivo de projeto inválido.")); }
    let caminho = pasta_do_projeto(raiz, id)?.join(rel);
    match fs::symlink_metadata(&caminho) {
        Ok(meta) if !meta.file_type().is_file() => Err(invalido("Arquivo de projeto inválido.")),
        Ok(meta) if meta.len() as usize > TAMANHO_MAXIMO => Err(invalido("Arquivo grande demais.")),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(erro) => Err(erro),
        Ok(_) => fs::read_to_string(caminho).map(Some),
    }
}

/// Grava um arquivo do projeto. Com `motivo`, guarda antes uma versão (desfazer volta nela).
/// Devolve o número da versão criada, se criou.
pub fn gravar(raiz: &Path, id: &str, rel: &str, conteudo: &str, motivo: Option<&str>, agora_ms: u128) -> io::Result<Option<u64>> {
    if !arquivo_valido(rel) || rel == CONVERSA { return Err(invalido("Arquivo de projeto inválido.")); }
    conferir_conteudo(rel, conteudo)?;
    let pasta = existente(raiz, id)?;
    let versao = match motivo { Some(m) => criar_versao(raiz, id, m, agora_ms)?, None => None };
    let _trava = travar();
    gravar_atomico(&pasta.join(rel), conteudo.as_bytes())?;
    if rel != PROJETO { tocar(&pasta, agora_ms); }
    Ok(versao)
}

/// Marca `atualizadoEm` (a lista mostra os mais recentes). Falhar aqui não perde nada.
fn tocar(pasta: &Path, agora_ms: u128) {
    if let Some(mut projeto) = arquivos::ler_json(&pasta.join(PROJETO)).filter(|v| v.is_object()) {
        projeto["atualizadoEm"] = (agora_ms as u64).into();
        let _ = arquivos::gravar_json(&pasta.join(PROJETO), &projeto);
    }
}

/// Acrescenta uma mensagem no fim da conversa, sem reescrever as anteriores.
pub fn anexar_na_conversa(raiz: &Path, id: &str, linha: &str) -> io::Result<()> {
    use std::io::Write;
    let linha = linha.trim();
    if linha.is_empty() || linha.len() > LINHA_MAXIMA || linha.contains('\n') { return Err(invalido("Mensagem inválida.")); }
    if serde_json::from_str::<serde_json::Value>(linha).is_err() { return Err(invalido("Mensagem não é JSON válido.")); }
    let destino = existente(raiz, id)?.join(CONVERSA);
    let _trava = travar();
    if fs::symlink_metadata(&destino).is_ok_and(|m| !m.file_type().is_file()) { return Err(invalido("Conversa inválida.")); }
    let mut opcoes = fs::OpenOptions::new();
    opcoes.create(true).append(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    let mut arquivo = opcoes.open(&destino)?;
    // Uma escrita só com a quebra de linha: um fim cortado (queda de luz) estraga no máximo a última.
    arquivo.write_all(format!("{linha}\n").as_bytes())?;
    arquivo.sync_all()
}

/// Composições do projeto (`compositions/*.html`), em ordem de nome.
pub fn composicoes(raiz: &Path, id: &str) -> io::Result<Vec<String>> {
    let pasta = pasta_do_projeto(raiz, id)?.join("compositions");
    let mut lista: Vec<String> = fs::read_dir(&pasta).map(|itens| itens.flatten()
        .filter_map(|i| i.file_name().to_str().map(|n| format!("compositions/{n}")))
        .filter(|rel| nome_de_composicao(rel) && arquivos::arquivo_comum(&pasta.join(&rel["compositions/".len()..])).is_some())
        .collect()).unwrap_or_default();
    lista.sort();
    Ok(lista)
}

fn versionados(raiz: &Path, id: &str) -> io::Result<Vec<String>> {
    let mut lista: Vec<String> = VERSIONADOS.iter().map(|s| s.to_string()).collect();
    lista.extend(composicoes(raiz, id)?);
    Ok(lista)
}

fn numero_da_pasta(nome: &str) -> Option<u64> {
    (nome.len() == 6 && nome.bytes().all(|b| b.is_ascii_digit())).then(|| nome.parse().ok()).flatten()
}

fn versoes_em_disco(pasta: &Path) -> Vec<u64> {
    let mut lista: Vec<u64> = fs::read_dir(pasta.join(HISTORICO)).map(|itens| itens.flatten()
        .filter(|i| i.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|i| i.file_name().to_str().and_then(numero_da_pasta))
        .collect()).unwrap_or_default();
    lista.sort_unstable();
    lista
}

/// Guarda os arquivos de texto do projeto como uma versão nova (`historico/<n>/`). Se nada mudou
/// desde a última, não cria outra e devolve `None`. Mantém as [`VERSOES_GUARDADAS`] mais recentes.
pub fn criar_versao(raiz: &Path, id: &str, motivo: &str, agora_ms: u128) -> io::Result<Option<u64>> {
    let pasta = existente(raiz, id)?;
    let rels = versionados(raiz, id)?;
    let atuais: Vec<(String, Vec<u8>)> = rels.iter()
        .filter_map(|rel| fs::read(pasta.join(rel)).ok().map(|c| (rel.clone(), c)))
        .collect();
    let _trava = travar();
    let existentes = versoes_em_disco(&pasta);
    if let Some(ultima) = existentes.last() {
        let anterior = pasta.join(HISTORICO).join(format!("{ultima:06}"));
        let lista_anterior: Vec<String> = arquivos::ler_json(&anterior.join("versao.json"))
            .and_then(|v| v["arquivos"].as_array().cloned())
            .map(|a| a.into_iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let iguais = lista_anterior.len() == atuais.len()
            && atuais.iter().all(|(rel, c)| fs::read(anterior.join(rel)).is_ok_and(|a| &a == c));
        if iguais { return Ok(None); }
    }
    let n = existentes.last().map_or(1, |u| u + 1);
    let destino = pasta.join(HISTORICO).join(format!("{n:06}"));
    criar_pasta(&destino)?;
    for (rel, conteudo) in &atuais { gravar_atomico(&destino.join(rel), conteudo)?; }
    let motivo: String = motivo.chars().take(200).collect();
    arquivos::gravar_json(&destino.join("versao.json"), &serde_json::json!({
        "n": n, "motivo": motivo, "quando": agora_ms as u64,
        "arquivos": atuais.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(),
    }))?;
    for velha in existentes.iter().take(existentes.len().saturating_sub(VERSOES_GUARDADAS - 1)) {
        let _ = fs::remove_dir_all(pasta.join(HISTORICO).join(format!("{velha:06}")));
    }
    Ok(Some(n))
}

/// Versões do histórico, da mais nova para a mais antiga (`{n, motivo, quando, arquivos}`).
pub fn listar_versoes(raiz: &Path, id: &str) -> io::Result<Vec<serde_json::Value>> {
    let pasta = existente(raiz, id)?;
    Ok(versoes_em_disco(&pasta).into_iter().rev()
        .filter_map(|n| arquivos::ler_json(&pasta.join(HISTORICO).join(format!("{n:06}")).join("versao.json")))
        .collect())
}

/// Volta os arquivos de texto para a versão `n`. Antes guarda o estado atual como versão nova
/// (é o "refazer"), e devolve o número dela. Composição que não existia na versão vai junto
/// para essa cópia e sai da pasta.
pub fn restaurar_versao(raiz: &Path, id: &str, n: u64, agora_ms: u128) -> io::Result<Option<u64>> {
    let pasta = existente(raiz, id)?;
    let origem = pasta.join(HISTORICO).join(format!("{n:06}"));
    let lista: Vec<String> = arquivos::ler_json(&origem.join("versao.json"))
        .and_then(|v| v["arquivos"].as_array().cloned())
        .ok_or_else(|| invalido("Essa versão não existe."))?
        .into_iter().filter_map(|x| x.as_str().map(str::to_owned))
        .filter(|rel| arquivo_valido(rel) && rel != CONVERSA && rel != PROJETO)
        .collect();
    let refazer = criar_versao(raiz, id, &format!("Antes de voltar para a versão {n}"), agora_ms)?;
    let atuais = composicoes(raiz, id)?;
    let _trava = travar();
    for rel in &lista {
        let conteudo = fs::read(origem.join(rel))?;
        gravar_atomico(&pasta.join(rel), &conteudo)?;
    }
    for rel in atuais.iter().filter(|r| !lista.contains(r)) { let _ = fs::remove_file(pasta.join(rel)); }
    tocar(&pasta, agora_ms);
    Ok(refazer)
}

/// Apagar = mover para `videos/lixeira/` (com a data no nome). Nada some de verdade.
pub fn mandar_para_a_lixeira(raiz: &Path, id: &str, carimbo: &str) -> io::Result<PathBuf> {
    let pasta = existente(raiz, id)?;
    let lixeira = raiz.join(LIXEIRA);
    criar_pasta(&lixeira)?;
    let mut destino = lixeira.join(format!("{id}-{carimbo}"));
    let mut n = 1;
    while destino.exists() { n += 1; destino = lixeira.join(format!("{id}-{carimbo}-{n}")); }
    let _trava = travar();
    fs::rename(&pasta, &destino)?;
    drop(_trava);
    let mut ordem = ler_indice(raiz);
    ordem.retain(|i| i != id);
    ordenar(raiz, &ordem)?;
    Ok(destino)
}

/// Pastas que ficam fora das cópias de segurança: mídia grande (referenciada por hash em
/// `midias.json`), exportações, skills (vêm do app) e o histórico já é texto pequeno (entra).
const FORA_DO_BACKUP: &[&str] = &["assets", "renders", ".claude", "referencias-copiadas"];
/// Texto acima disso também fica de fora (nenhum arquivo do projeto chega perto).
const LIMITE_DO_BACKUP: u64 = TAMANHO_MAXIMO as u64;

/// Copia um projeto (só texto) para `destino`. Devolve quantos arquivos copiou.
pub fn copiar_projeto(pasta: &Path, destino: &Path) -> io::Result<usize> {
    arquivos::copiar_textos(pasta, destino, FORA_DO_BACKUP, LIMITE_DO_BACKUP)
}

/// Backup da atualização: índice, todos os projetos e as pastas de referências, só o texto.
pub fn copiar_tudo(raiz: &Path, destino: &Path) -> io::Result<usize> {
    if !arquivos::pasta_comum(raiz) { return Ok(0); }
    let mut n = 0;
    if arquivos::arquivo_comum(&raiz.join(INDICE)).is_some() {
        criar_pasta(destino)?;
        fs::copy(raiz.join(INDICE), destino.join(INDICE))?;
        n += 1;
    }
    for item in fs::read_dir(raiz.join(PROJETOS)).into_iter().flatten().flatten() {
        let Some(id) = item.file_name().to_str().map(str::to_owned) else { continue };
        if !arquivos::id_valido(&id) || !arquivos::pasta_comum(&item.path()) { continue; }
        n += copiar_projeto(&item.path(), &destino.join(PROJETOS).join(&id))?;
    }
    n += super::referencias::copiar_tudo(raiz, destino)?;
    Ok(n)
}

/// Antes de migrar o formato de um projeto ou de abrir uma sessão a dois: cópia do texto do
/// projeto em `<backups>/videos/<id>-<motivo>-<carimbo>/`.
pub fn copiar_para_backups(raiz: &Path, id: &str, motivo: &str, backups: &Path, carimbo: &str) -> io::Result<PathBuf> {
    let pasta = existente(raiz, id)?;
    let motivo: String = motivo.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(40).collect();
    let base = backups.join(PASTA).join(format!("{id}-{motivo}-{carimbo}"));
    let mut destino = base.clone();
    let mut n = 1;
    while destino.exists() { n += 1; destino = PathBuf::from(format!("{}-{n}", base.display())); }
    copiar_projeto(&pasta, &destino)?;
    Ok(destino)
}
