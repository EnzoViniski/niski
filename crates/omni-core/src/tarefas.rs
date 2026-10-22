//! Fork (Tarefas): uma tarefa por arquivo JSON, para dividir com outra pessoa sem servidor.
//!
//! A pasta é `<dados>/tarefas` ou, se o usuário escolher, uma pasta sincronizada (Google Drive,
//! iCloud, Dropbox...) onde o app cria `niski-tarefas/`. Um arquivo por tarefa evita que duas
//! pessoas editando coisas diferentes briguem pelo mesmo arquivo; apagar grava a tarefa marcada
//! como apagada (o sincronizador espalha a marca, não um arquivo sumido).
//!
//! O app (Tauri) lê e grava pelo front (`src/features/tarefas/`, dono do domínio completo). O engine
//! usa as funções de baixo para as ferramentas MCP do copiloto/orquestradores: criar, editar,
//! concluir e listar, com o mesmo formato que o front normaliza ao ler.

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde_json::{json, Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Subpasta criada dentro da pasta compartilhada escolhida.
pub const SUBPASTA: &str = "niski-tarefas";
/// Subpasta padrão dentro da pasta de dados do engine.
pub const PASTA_PADRAO: &str = "tarefas";
const TAMANHO_MAXIMO: u64 = 1024 * 1024;
const TIPOS: &[&str] = &["tarefas", "pessoas"];

pub const STATUS: &[&str] = &["a_fazer", "fazendo", "revisao", "feito"];
pub const PRIORIDADES: &[&str] = &["baixa", "media", "alta", "urgente"];
pub const CATEGORIAS: &[&str] = &["trabalho", "faculdade", "saude", "casa", "financas", "pessoal", "outro"];
pub const RECORRENCIAS: &[&str] = &["nunca", "diaria", "dias_uteis", "semanal", "mensal"];

fn invalido(texto: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, texto.into())
}

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn conferir(tipo: &str, id: &str) -> io::Result<()> {
    if !TIPOS.contains(&tipo) {
        return Err(invalido("Tipo de arquivo desconhecido."));
    }
    if !id_valido(id) {
        return Err(invalido("Identificador de tarefa inválido."));
    }
    Ok(())
}

/// Raiz dos arquivos: a pasta compartilhada (tem que existir; o app não cria a pasta do usuário)
/// ou a padrão, dentro da pasta de dados.
pub fn raiz(compartilhada: Option<&str>, padrao: &Path) -> io::Result<PathBuf> {
    match compartilhada.map(str::trim).filter(|p| !p.is_empty()) {
        Some(pasta) => {
            let base = PathBuf::from(pasta);
            if !base.is_dir() {
                return Err(invalido("A pasta compartilhada não existe mais. Escolha outra em Configurações → Tarefas."));
            }
            Ok(base.join(SUBPASTA))
        }
        None => Ok(padrao.to_path_buf()),
    }
}

/// Conteúdo de cada `<raiz>/<tipo>/*.json`. Arquivo grande demais, ilegível ou temporário fica de fora.
pub fn listar(raiz: &Path, tipo: &str) -> io::Result<Vec<String>> {
    conferir(tipo, "x")?;
    let Ok(entradas) = fs::read_dir(raiz.join(tipo)) else { return Ok(Vec::new()) };
    let mut nomes: Vec<PathBuf> = entradas
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !nome.starts_with('.') && nome.ends_with(".json")
        })
        .collect();
    nomes.sort();
    Ok(nomes
        .into_iter()
        .filter(|p| fs::metadata(p).map(|m| m.is_file() && m.len() <= TAMANHO_MAXIMO).unwrap_or(false))
        .filter_map(|p| fs::read_to_string(p).ok())
        .collect())
}

/// Grava inteiro num temporário e renomeia: o sincronizador nunca pega um arquivo pela metade.
pub fn gravar(raiz: &Path, tipo: &str, id: &str, conteudo: &str) -> io::Result<()> {
    conferir(tipo, id)?;
    if conteudo.len() as u64 > TAMANHO_MAXIMO {
        return Err(invalido("A tarefa ficou grande demais (máximo 1 MB)."));
    }
    let pasta = raiz.join(tipo);
    fs::create_dir_all(&pasta)?;
    let temporario = pasta.join(format!(".{id}.json.tmp"));
    fs::write(&temporario, conteudo)?;
    fs::rename(&temporario, pasta.join(format!("{id}.json")))
}

/// Copia as tarefas locais para a pasta compartilhada (sem sobrescrever as que já estão lá).
pub fn levar(origem: &Path, destino: &Path) -> io::Result<usize> {
    let mut copiadas = 0;
    for tipo in TIPOS {
        let Ok(entradas) = fs::read_dir(origem.join(tipo)) else { continue };
        fs::create_dir_all(destino.join(tipo))?;
        for entrada in entradas.flatten() {
            let caminho = entrada.path();
            let Some(nome) = caminho.file_name().and_then(|n| n.to_str()) else { continue };
            if nome.starts_with('.') || !nome.ends_with(".json") {
                continue;
            }
            let alvo = destino.join(tipo).join(nome);
            if !alvo.exists() {
                fs::copy(&caminho, &alvo)?;
                copiadas += 1;
            }
        }
    }
    Ok(copiadas)
}

// ---- Para o engine (ferramentas do copiloto) ----

/// Onde estão as tarefas e quem é o usuário, lidos do espelho da interface
/// (`<pasta de dados>/estado-da-interface.json`, chave `medza-tarefas-ajustes`).
pub struct Contexto { pub raiz: PathBuf, pub eu: String }

pub fn contexto(pasta_de_dados: &Path) -> io::Result<Contexto> {
    let ajustes = fs::read_to_string(pasta_de_dados.join("estado-da-interface.json")).ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("chaves")?.get("medza-tarefas-ajustes")?.as_str().map(str::to_owned))
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or(Value::Null);
    let compartilhada = ajustes.get("pastaCompartilhada").and_then(Value::as_str);
    let eu = ajustes.get("eu").and_then(|e| e.get("id")).and_then(Value::as_str).unwrap_or("").to_owned();
    Ok(Contexto { raiz: raiz(compartilhada, &pasta_de_dados.join(PASTA_PADRAO))?, eu })
}

/// Tarefas (não apagadas) como JSON.
pub fn todas(raiz: &Path) -> Vec<Value> {
    listar(raiz, "tarefas").unwrap_or_default().iter()
        .filter_map(|t| serde_json::from_str::<Value>(t).ok())
        .filter(|t| t.get("id").and_then(Value::as_str).is_some_and(id_valido) && t.get("apagada") != Some(&Value::Bool(true)))
        .collect()
}

pub fn ler(raiz: &Path, id: &str) -> io::Result<Value> {
    conferir("tarefas", id)?;
    let texto = fs::read_to_string(raiz.join("tarefas").join(format!("{id}.json")))
        .map_err(|_| invalido(format!("Tarefa {id} não encontrada.")))?;
    serde_json::from_str(&texto).map_err(|_| invalido("Arquivo da tarefa ilegível."))
}

fn data_valida(v: &str) -> bool { v.is_empty() || NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok() }
fn hora_valida(v: &str) -> bool {
    v.is_empty() || (v.len() == 5 && v.as_bytes()[2] == b':' && v[..2].parse::<u8>().is_ok_and(|h| h < 24) && v[3..].parse::<u8>().is_ok_and(|m| m < 60))
}

/// Copia para `destino` só os campos editáveis de `campos`, conferindo valores. Erro diz o campo.
fn aplicar_campos(destino: &mut Map<String, Value>, campos: &Value, agora_ms: u128) -> io::Result<()> {
    let Some(campos) = campos.as_object() else { return Ok(()) };
    let texto = |nome: &str, max: usize| campos.get(nome).and_then(Value::as_str).map(|s| s.chars().take(max).collect::<String>());
    for (nome, max) in [("titulo", 300), ("descricao", 20_000), ("local", 200), ("responsavel", 80)] {
        if let Some(v) = texto(nome, max) { destino.insert(nome.into(), json!(v.trim())); }
    }
    for (nome, opcoes) in [("status", STATUS), ("prioridade", PRIORIDADES), ("categoria", CATEGORIAS), ("recorrencia", RECORRENCIAS)] {
        if let Some(v) = texto(nome, 40) {
            if !opcoes.contains(&v.as_str()) { return Err(invalido(format!("`{nome}` inválido: use {}", opcoes.join(", ")))); }
            destino.insert(nome.into(), json!(v));
        }
    }
    if let Some(v) = texto("data", 10) {
        if !data_valida(&v) { return Err(invalido("`data` é AAAA-MM-DD (ou vazio)")); }
        destino.insert("data".into(), json!(v));
    }
    if let Some(v) = texto("hora", 5) {
        if !hora_valida(&v) { return Err(invalido("`hora` é HH:MM (ou vazio)")); }
        destino.insert("hora".into(), json!(v));
    }
    if let Some(n) = campos.get("duracaoMin").and_then(Value::as_u64) { destino.insert("duracaoMin".into(), json!(n.min(24 * 60))); }
    if let Some(lista) = campos.get("etiquetas").and_then(Value::as_array) {
        let etiquetas: Vec<String> = lista.iter().filter_map(Value::as_str).map(|s| s.trim().chars().take(40).collect()).filter(|s: &String| !s.is_empty()).collect();
        destino.insert("etiquetas".into(), json!(etiquetas));
    }
    if let Some(lista) = campos.get("checklist").and_then(Value::as_array) {
        let itens: Vec<Value> = lista.iter().enumerate().filter_map(|(n, item)| {
            let (texto, feito) = match item {
                Value::String(s) => (s.clone(), false),
                Value::Object(o) => (o.get("texto").and_then(Value::as_str)?.to_owned(), o.get("feito") == Some(&Value::Bool(true))),
                _ => return None,
            };
            Some(json!({"id": format!("i-{agora_ms:x}-{n}"), "texto": texto.chars().take(500).collect::<String>(), "feito": feito}))
        }).collect();
        destino.insert("checklist".into(), json!(itens));
    }
    Ok(())
}

fn iso(agora_ms: u128) -> String {
    chrono::DateTime::from_timestamp_millis(agora_ms as i64).unwrap_or_default().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn salvar(raiz: &Path, id: &str, tarefa: &Value) -> io::Result<()> {
    gravar(raiz, "tarefas", id, &serde_json::to_string_pretty(tarefa).unwrap_or_default())
}

/// Cria uma tarefa nova com o formato completo do front. `aleatorio` entra no id.
pub fn criar(raiz: &Path, campos: &Value, eu: &str, agora_ms: u128, aleatorio: u32) -> io::Result<Value> {
    let titulo = campos.get("titulo").and_then(Value::as_str).map(str::trim).unwrap_or("");
    if titulo.is_empty() { return Err(invalido("Falta o `titulo` da tarefa.")); }
    let id = format!("t-{agora_ms:x}-{aleatorio:x}");
    let quando = iso(agora_ms);
    let mut tarefa = json!({
        "versao": 1, "id": id, "titulo": titulo, "descricao": "", "status": "a_fazer", "prioridade": "media",
        "categoria": "outro", "etiquetas": [], "responsavel": eu, "criadaPor": eu, "data": "", "hora": "",
        "duracaoMin": 0, "local": "", "recorrencia": "nunca", "checklist": [], "relacionadas": [], "comentarios": [],
        "posicao": agora_ms as u64, "criadaEm": quando, "atualizadaEm": quando, "concluidaEm": "", "apagada": false,
    });
    aplicar_campos(tarefa.as_object_mut().expect("objeto"), campos, agora_ms)?;
    salvar(raiz, &id, &tarefa)?;
    Ok(tarefa)
}

pub fn editar(raiz: &Path, id: &str, campos: &Value, agora_ms: u128) -> io::Result<Value> {
    let mut tarefa = ler(raiz, id)?;
    let objeto = tarefa.as_object_mut().ok_or_else(|| invalido("Tarefa ilegível."))?;
    aplicar_campos(objeto, campos, agora_ms)?;
    objeto.insert("atualizadaEm".into(), json!(iso(agora_ms)));
    salvar(raiz, id, &tarefa)?;
    Ok(tarefa)
}

/// Próxima data de uma recorrente (mesma regra do front, `dominio.ts`).
pub fn proxima_data(dia: &str, recorrencia: &str, hoje: NaiveDate) -> String {
    let base = NaiveDate::parse_from_str(dia, "%Y-%m-%d").unwrap_or(hoje);
    let proxima = match recorrencia {
        "diaria" => base + Duration::days(1),
        "semanal" => base + Duration::days(7),
        "dias_uteis" => {
            let mut d = base + Duration::days(1);
            while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) { d += Duration::days(1); }
            d
        }
        "mensal" => {
            let (ano, mes) = if base.month() == 12 { (base.year() + 1, 1) } else { (base.year(), base.month() + 1) };
            (1..=base.day()).rev().find_map(|d| NaiveDate::from_ymd_opt(ano, mes, d)).unwrap_or(base)
        }
        _ => base,
    };
    proxima.format("%Y-%m-%d").to_string()
}

/// Concluir (ou reabrir com `feita = false`). Recorrente concluída volta para "a fazer" na próxima data.
pub fn concluir(raiz: &Path, id: &str, feita: bool, agora_ms: u128, hoje: NaiveDate) -> io::Result<Value> {
    let mut tarefa = ler(raiz, id)?;
    let quando = iso(agora_ms);
    let recorrencia = tarefa.get("recorrencia").and_then(Value::as_str).unwrap_or("nunca").to_owned();
    let data = tarefa.get("data").and_then(Value::as_str).unwrap_or("").to_owned();
    let objeto = tarefa.as_object_mut().ok_or_else(|| invalido("Tarefa ilegível."))?;
    if !feita {
        objeto.insert("status".into(), json!("a_fazer"));
        objeto.insert("concluidaEm".into(), json!(""));
    } else if recorrencia != "nunca" {
        objeto.insert("status".into(), json!("a_fazer"));
        objeto.insert("data".into(), json!(proxima_data(&data, &recorrencia, hoje)));
        objeto.insert("concluidaEm".into(), json!(quando));
        if let Some(Value::Array(itens)) = objeto.get_mut("checklist") {
            for item in itens { if let Some(o) = item.as_object_mut() { o.insert("feito".into(), json!(false)); } }
        }
    } else {
        objeto.insert("status".into(), json!("feito"));
        objeto.insert("concluidaEm".into(), json!(quando));
    }
    objeto.insert("atualizadaEm".into(), json!(quando));
    salvar(raiz, id, &tarefa)?;
    Ok(tarefa)
}

pub fn apagar(raiz: &Path, id: &str, agora_ms: u128) -> io::Result<Value> {
    let mut tarefa = ler(raiz, id)?;
    let objeto = tarefa.as_object_mut().ok_or_else(|| invalido("Tarefa ilegível."))?;
    objeto.insert("apagada".into(), json!(true));
    objeto.insert("atualizadaEm".into(), json!(iso(agora_ms)));
    salvar(raiz, id, &tarefa)?;
    Ok(tarefa)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_lista_e_ignora_temporarios() {
        let dir = tempfile::tempdir().unwrap();
        gravar(dir.path(), "tarefas", "t-1", "{\"id\":\"t-1\"}").unwrap();
        gravar(dir.path(), "tarefas", "t-2", "{\"id\":\"t-2\"}").unwrap();
        fs::write(dir.path().join("tarefas/.t-3.json.tmp"), "meio").unwrap();
        fs::write(dir.path().join("tarefas/notas.txt"), "x").unwrap();
        assert_eq!(listar(dir.path(), "tarefas").unwrap(), vec!["{\"id\":\"t-1\"}", "{\"id\":\"t-2\"}"]);
        assert!(listar(dir.path(), "pessoas").unwrap().is_empty());
    }

    #[test]
    fn recusa_id_e_tipo_que_saem_da_pasta() {
        let dir = tempfile::tempdir().unwrap();
        assert!(gravar(dir.path(), "tarefas", "../fora", "{}").is_err());
        assert!(gravar(dir.path(), "tarefas", "", "{}").is_err());
        assert!(gravar(dir.path(), "../x", "a", "{}").is_err());
        assert!(listar(dir.path(), "segredos").is_err());
        assert!(ler(dir.path(), "../../etc/passwd").is_err());
    }

    #[test]
    fn pasta_compartilhada_precisa_existir() {
        let dir = tempfile::tempdir().unwrap();
        let padrao = dir.path().join("padrao");
        assert_eq!(raiz(None, &padrao).unwrap(), padrao);
        assert_eq!(raiz(Some("  "), &padrao).unwrap(), padrao);
        assert_eq!(raiz(Some(dir.path().to_str().unwrap()), &padrao).unwrap(), dir.path().join(SUBPASTA));
        assert!(raiz(Some("/nao/existe/mesmo"), &padrao).is_err());
    }

    #[test]
    fn levar_copia_sem_sobrescrever() {
        let dir = tempfile::tempdir().unwrap();
        let (origem, destino) = (dir.path().join("o"), dir.path().join("d"));
        gravar(&origem, "tarefas", "a", "minha").unwrap();
        gravar(&origem, "tarefas", "b", "minha").unwrap();
        gravar(&destino, "tarefas", "b", "do primo").unwrap();
        assert_eq!(levar(&origem, &destino).unwrap(), 1);
        assert_eq!(fs::read_to_string(destino.join("tarefas/b.json")).unwrap(), "do primo");
        assert_eq!(fs::read_to_string(destino.join("tarefas/a.json")).unwrap(), "minha");
    }

    #[test]
    fn contexto_le_pasta_e_eu_do_espelho() {
        let dir = tempfile::tempdir().unwrap();
        let ajustes = json!({"eu": {"id": "enzo"}, "pastaCompartilhada": ""}).to_string();
        fs::write(dir.path().join("estado-da-interface.json"), json!({"salvo_em": 1, "chaves": {"medza-tarefas-ajustes": ajustes}}).to_string()).unwrap();
        let c = contexto(dir.path()).unwrap();
        assert_eq!((c.raiz, c.eu.as_str()), (dir.path().join(PASTA_PADRAO), "enzo"));
        let vazio = tempfile::tempdir().unwrap();
        assert_eq!(contexto(vazio.path()).unwrap().eu, "");
    }

    #[test]
    fn criar_editar_concluir_e_apagar() {
        let dir = tempfile::tempdir().unwrap();
        let agora = 1_792_000_000_000u128;
        let t = criar(dir.path(), &json!({"titulo": " Dentista ", "data": "2026-10-23", "hora": "15:00", "categoria": "saude", "checklist": ["RG", {"texto": "Carteirinha"}], "id": "hack"}), "enzo", agora, 7).unwrap();
        let id = t["id"].as_str().unwrap().to_owned();
        assert!(id.starts_with("t-") && id != "hack");
        assert_eq!((t["titulo"].as_str(), t["categoria"].as_str(), t["responsavel"].as_str()), (Some("Dentista"), Some("saude"), Some("enzo")));
        assert_eq!(t["checklist"].as_array().unwrap().len(), 2);
        assert!(criar(dir.path(), &json!({"titulo": "x", "categoria": "nada"}), "enzo", agora, 1).is_err());
        assert!(criar(dir.path(), &json!({"titulo": "x", "hora": "25:00"}), "enzo", agora, 2).is_err());
        assert!(criar(dir.path(), &json!({"descricao": "sem título"}), "enzo", agora, 3).is_err());

        let e = editar(dir.path(), &id, &json!({"prioridade": "alta", "status": "fazendo"}), agora + 1).unwrap();
        assert_eq!((e["prioridade"].as_str(), e["status"].as_str(), e["titulo"].as_str()), (Some("alta"), Some("fazendo"), Some("Dentista")));
        let hoje = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        assert_eq!(concluir(dir.path(), &id, true, agora + 2, hoje).unwrap()["status"], "feito");
        assert_eq!(concluir(dir.path(), &id, false, agora + 3, hoje).unwrap()["status"], "a_fazer");
        apagar(dir.path(), &id, agora + 4).unwrap();
        assert!(todas(dir.path()).is_empty());
    }

    #[test]
    fn recorrente_volta_na_proxima_data() {
        let dir = tempfile::tempdir().unwrap();
        let hoje = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let t = criar(dir.path(), &json!({"titulo": "Remédio", "data": "2026-10-23", "recorrencia": "dias_uteis", "checklist": [{"texto": "x", "feito": true}]}), "", 1, 1).unwrap();
        let c = concluir(dir.path(), t["id"].as_str().unwrap(), true, 2, hoje).unwrap();
        assert_eq!((c["status"].as_str(), c["data"].as_str()), (Some("a_fazer"), Some("2026-10-26")));
        assert_eq!(c["checklist"][0]["feito"], false);
        assert_eq!(proxima_data("2026-01-31", "mensal", hoje), "2026-02-28");
        assert_eq!(proxima_data("", "diaria", hoje), "2026-10-22");
    }
}
