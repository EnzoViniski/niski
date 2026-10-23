//! Fork (Rotina, 0.28): a rotina da semana do usuário — plano da semana (metas e blocos fixos),
//! treino, alimentação, lembretes e os ajustes do agente da rotina.
//!
//! Mora em `<pasta de dados>/rotina/`, **um arquivo por assunto** (`plano.json`, `treino.json`,
//! `alimentacao.json`, `lembretes.json`, `lembretes-estado.json`, `ajustes.json`). Compromissos e
//! tarefas da semana **não** moram aqui: são as Tarefas do app (`crate::tarefas`), e a rotina só as lê
//! (e conclui/remarca pelas funções de lá).
//!
//! Um escritor só: o engine (rotas da tela, ferramentas MCP do agente da rotina, botões do Telegram e o
//! relógio dos lembretes). Toda gravação passa por [`travar`] e grava inteiro num temporário antes de
//! renomear. Leitura tolerante: arquivo ausente vira o padrão, item ilegível numa lista é pulado e
//! arquivo inteiro ilegível ganha uma cópia `<nome>.ilegivel-<hash>` antes de o app seguir com o
//! padrão (nada é apagado). Arquivo de uma versão mais nova do app não é regravado.
//!
//! Nada daqui vai para o log: o conteúdo é pessoal (refeições, treinos, compromissos).

pub mod ajustes;
pub mod alimentacao;
pub mod dia;
pub mod lembretes;
pub mod operacoes;
pub mod plano;
pub mod treino;

#[cfg(test)]
mod testes;
#[cfg(test)]
mod lembretes_testes;

use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Timelike};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};

/// Pasta dentro da pasta de dados.
pub const PASTA: &str = "rotina";
/// Versão atual dos arquivos. Arquivo sem `versao` é tratado como 1.
pub const VERSAO: u32 = 1;
const TAMANHO_MAXIMO: u64 = 16 * 1024 * 1024;

static TRAVA: Mutex<()> = Mutex::new(());
static SEQUENCIA: AtomicU32 = AtomicU32::new(0);

/// Ler-mudar-gravar dos arquivos da rotina, um por vez no processo (tela, MCP, Telegram, relógio).
pub fn travar() -> MutexGuard<'static, ()> {
    TRAVA.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn pasta(dados: &Path) -> PathBuf {
    dados.join(PASTA)
}

/// Um arquivo da rotina: nome e migração do formato antigo (antes de desserializar).
pub trait Arquivo: Serialize + DeserializeOwned + Default {
    const NOME: &'static str;
    fn versao(&self) -> u32;
    fn marcar_versao(&mut self);
    /// Converte o JSON de uma versão anterior. Hoje só existe a 1; arquivo sem `versao` é a 1.
    fn migrar(valor: Value) -> Value {
        valor
    }
}

/// Lê o arquivo. Ausente: padrão. Ilegível: guarda uma cópia para recuperação e devolve o padrão.
pub fn ler<T: Arquivo>(dados: &Path) -> T {
    let caminho = pasta(dados).join(T::NOME);
    let Ok(meta) = fs::metadata(&caminho) else { return T::default() };
    if !meta.is_file() || meta.len() > TAMANHO_MAXIMO {
        return T::default();
    }
    let Ok(bytes) = fs::read(&caminho) else { return T::default() };
    let lido = serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| serde_json::from_value::<T>(T::migrar(v)).ok());
    match lido {
        Some(valor) => valor,
        None => {
            guardar_ilegivel(&caminho, &bytes);
            T::default()
        }
    }
}

fn guardar_ilegivel(caminho: &Path, bytes: &[u8]) {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hash);
    let copia = caminho.with_extension(format!("json.ilegivel-{:x}", hash.finish()));
    if !copia.exists() {
        let _ = fs::write(copia, bytes);
    }
}

/// Grava inteiro num temporário e renomeia. Recusa regravar arquivo de uma versão mais nova.
pub fn gravar<T: Arquivo>(dados: &Path, valor: &T) -> Result<(), String> {
    let dir = pasta(dados);
    let caminho = dir.join(T::NOME);
    if let Some(versao) = versao_em_disco(&caminho) {
        if versao > VERSAO {
            return Err("Este arquivo da rotina é de uma versão mais nova do app. Atualize o Niski para mudar a rotina.".into());
        }
    }
    let mut valor = serde_json::to_value(valor).map_err(|_| "Não consegui preparar a rotina para gravar.".to_owned())?;
    if let Some(objeto) = valor.as_object_mut() {
        objeto.insert("versao".into(), Value::from(VERSAO));
    }
    let texto = serde_json::to_vec_pretty(&valor).map_err(|_| "Não consegui preparar a rotina para gravar.".to_owned())?;
    if texto.len() as u64 > TAMANHO_MAXIMO {
        return Err("A rotina ficou grande demais para gravar.".into());
    }
    let escrever = || -> io::Result<()> {
        fs::create_dir_all(&dir)?;
        let temporario = dir.join(format!(".{}.tmp", T::NOME));
        fs::write(&temporario, &texto)?;
        fs::rename(&temporario, &caminho)
    };
    escrever().map_err(|_| "Não consegui gravar a rotina no disco.".to_owned())
}

fn versao_em_disco(caminho: &Path) -> Option<u32> {
    let bytes = fs::read(caminho).ok()?;
    let valor: Value = serde_json::from_slice(&bytes).ok()?;
    valor.get("versao")?.as_u64().map(|v| v as u32)
}

/// Lê, muda e grava um arquivo. Quem chama já segura [`travar`].
pub fn mudar<T: Arquivo, R>(dados: &Path, f: impl FnOnce(&mut T) -> Result<R, String>) -> Result<R, String> {
    let mut valor: T = ler(dados);
    let resultado = f(&mut valor)?;
    valor.marcar_versao();
    gravar(dados, &valor)?;
    Ok(resultado)
}

/* ── Datas, horas e textos ─────────────────────────────────────────────────────────────────── */

pub fn data(texto: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(texto.trim(), "%Y-%m-%d").ok()
}

pub fn texto_da_data(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// `H:MM` ou `HH:MM` (também `8h`, `8h30`, que o agente às vezes manda).
pub fn hora(texto: &str) -> Option<NaiveTime> {
    let t = texto.trim().to_lowercase();
    let (h, m) = if let Some((h, m)) = t.split_once(':') {
        (h.to_owned(), m.to_owned())
    } else if let Some((h, m)) = t.split_once('h') {
        (h.to_owned(), if m.is_empty() { "00".to_owned() } else { m.to_owned() })
    } else {
        return None;
    };
    let (h, m) = (h.trim().parse::<u32>().ok()?, m.trim().parse::<u32>().ok()?);
    if h > 23 || m > 59 {
        return None;
    }
    NaiveTime::from_hms_opt(h, m, 0)
}

pub fn texto_da_hora(t: NaiveTime) -> String {
    format!("{:02}:{:02}", t.hour(), t.minute())
}

/// Hora normalizada `HH:MM`; vazio continua vazio.
pub fn normalizar_hora(texto: &str, campo: &str) -> Result<String, String> {
    if texto.trim().is_empty() {
        return Ok(String::new());
    }
    hora(texto).map(texto_da_hora).ok_or_else(|| format!("`{campo}` é HH:MM (recebi {})", limpar(texto, 12)))
}

pub fn normalizar_data(texto: &str, campo: &str) -> Result<NaiveDate, String> {
    data(texto).ok_or_else(|| format!("`{campo}` é AAAA-MM-DD (recebi {})", limpar(texto, 12)))
}

/// Segunda-feira da semana de `d`.
pub fn segunda_de(d: NaiveDate) -> NaiveDate {
    d - Duration::days(i64::from(d.weekday().num_days_from_monday()))
}

/// Segunda = 0 … domingo = 6.
pub fn dia_da_semana(d: NaiveDate) -> u8 {
    d.weekday().num_days_from_monday() as u8
}

pub const NOMES_DOS_DIAS: [&str; 7] = ["segunda", "terça", "quarta", "quinta", "sexta", "sábado", "domingo"];

pub fn minutos(t: NaiveTime) -> u32 {
    t.hour() * 60 + t.minute()
}

/// Minutos de `inicio` a `fim`; `fim` antes de `inicio` passa da meia-noite.
pub fn duracao(inicio: &str, fim: &str) -> u32 {
    match (hora(inicio), hora(fim)) {
        (Some(i), Some(f)) => {
            let (i, f) = (minutos(i), minutos(f));
            if f >= i { f - i } else { 24 * 60 - i + f }
        }
        _ => 0,
    }
}

/// Id novo: prefixo, instante e uma sequência do processo (dois no mesmo milissegundo não colidem).
pub fn novo_id(prefixo: &str, agora_ms: i64) -> String {
    let n = SEQUENCIA.fetch_add(1, Ordering::Relaxed);
    format!("{prefixo}-{:x}{:03x}", agora_ms.max(0), n % 4096)
}

pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Texto curto sem controles (quebra de linha só quando `max` > 300), aparado.
pub fn limpar(texto: &str, max: usize) -> String {
    let multilinha = max > 300;
    texto
        .trim()
        .chars()
        .filter(|c| !c.is_control() || (multilinha && (*c == '\n' || *c == '\t')))
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Chave de comparação de nomes (exercícios, etiquetas): minúsculas, sem acento, espaços únicos.
pub fn chave(nome: &str) -> String {
    let sem_acento: String = nome
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            outro => outro,
        })
        .collect();
    sem_acento.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// ISO em UTC (`2026-10-23T18:00:00.000Z`), como as tarefas.
pub fn iso(agora_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(agora_ms).unwrap_or_default().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/* ── Desserialização tolerante ─────────────────────────────────────────────────────────────── */

/// Lista em que um item ilegível é pulado (os outros ficam).
pub(crate) fn lista<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Vec<T>, D::Error> {
    let valor = Value::deserialize(d)?;
    Ok(match valor {
        Value::Array(itens) => itens.into_iter().filter_map(|i| serde_json::from_value(i).ok()).collect(),
        _ => Vec::new(),
    })
}

/// Mapa em que um valor ilegível é pulado.
pub(crate) fn mapa<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<std::collections::BTreeMap<String, T>, D::Error> {
    let valor = Value::deserialize(d)?;
    Ok(match valor {
        Value::Object(itens) => itens.into_iter().filter_map(|(k, v)| serde_json::from_value(v).ok().map(|v| (k, v))).collect(),
        _ => Default::default(),
    })
}

/// Número que aceita texto (`"62,5"`) e cai para `None` quando não é número.
pub(crate) fn numero_opcional<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(numero(&Value::deserialize(d)?))
}

pub fn numero(valor: &Value) -> Option<f64> {
    match valor {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").trim_end_matches("kg").trim().parse::<f64>().ok(),
        _ => None,
    }
    .filter(|n| n.is_finite())
}

/// Texto de um campo do pedido (o agente às vezes manda número no lugar de texto).
pub fn texto_de(args: &Value, campo: &str) -> Option<String> {
    match args.get(campo)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub fn numero_de(args: &Value, campo: &str) -> Option<f64> {
    args.get(campo).and_then(numero)
}

pub fn inteiro_de(args: &Value, campo: &str) -> Option<i64> {
    numero_de(args, campo).map(|n| n.round() as i64)
}

pub fn booleano_de(args: &Value, campo: &str) -> Option<bool> {
    match args.get(campo)? {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "sim" | "1" => Some(true),
            "false" | "nao" | "não" | "0" => Some(false),
            _ => None,
        },
        Value::Number(n) => Some(n.as_f64() != Some(0.0)),
        _ => None,
    }
}

/// Dias da semana (0 = segunda … 6 = domingo). Aceita números ou nomes (`"seg"`, `"sábado"`).
pub fn dias_de(valor: Option<&Value>) -> Result<Vec<u8>, String> {
    let Some(Value::Array(itens)) = valor else { return Ok(Vec::new()) };
    let mut dias = Vec::new();
    for item in itens {
        let dia = match item {
            Value::Number(n) => n.as_u64().filter(|d| *d <= 6).map(|d| d as u8),
            Value::String(s) => {
                let s = chave(s);
                NOMES_DOS_DIAS.iter().position(|nome| chave(nome).starts_with(&s[..s.len().min(3)]) && s.len() >= 3).map(|d| d as u8)
                    .or_else(|| s.parse::<u8>().ok().filter(|d| *d <= 6))
            }
            _ => None,
        };
        let dia = dia.ok_or_else(|| "Dias da semana: 0 = segunda … 6 = domingo (ou \"seg\", \"ter\"…)".to_owned())?;
        if !dias.contains(&dia) {
            dias.push(dia);
        }
    }
    dias.sort_unstable();
    Ok(dias)
}
