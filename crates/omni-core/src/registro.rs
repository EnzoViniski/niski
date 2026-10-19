//! Log em disco do app e do engine. **Só metadados**: ids, estados, durações, códigos de erro.
//! Nunca conteúdo de conversa, prompt, saída de terminal, token ou credencial.
//!
//! Sem isto, um defeito que só aparece no app aberto pelo Finder (onde não há terminal para ver o
//! `eprintln!`) não deixa rastro nenhum.
//!
//! Onde fica (o nome da pasta é o da pasta de dados, então dev, release e forks não se misturam):
//! - macOS: `~/Library/Logs/<nome>/`
//! - Windows: `%LOCALAPPDATA%\<nome>\logs\`
//! - Linux: `$XDG_STATE_HOME/<nome>/logs/` (ou `~/.local/state/<nome>/logs/`)
//!
//! `OMNI_LOG_DIR` troca a pasta. Um arquivo por componente (`engine.log`, `app.log`), girado ao
//! passar de [`TAMANHO_MAXIMO`], com [`ARQUIVOS_ANTIGOS`] cópias antigas.

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const TAMANHO_MAXIMO: u64 = 2 * 1024 * 1024;
pub const ARQUIVOS_ANTIGOS: usize = 3;
/// Valor maior que isto é cortado: metadado não precisa de mais, e corta qualquer vazamento longo.
const VALOR_MAXIMO: usize = 200;

struct Destino {
    caminho: PathBuf,
    arquivo: Option<File>,
}

static DESTINO: Mutex<Option<Destino>> = Mutex::new(None);

/// Pasta de logs para a pasta de dados `<raiz>/<nome>/engine` (ou `<raiz>/<nome>`).
pub fn pasta_de_logs(pasta_de_dados: &Path) -> Option<PathBuf> {
    if let Some(pasta) = env::var_os("OMNI_LOG_DIR").filter(|valor| !valor.is_empty()) {
        return Some(PathBuf::from(pasta));
    }
    let nome = nome_da_identidade(pasta_de_dados)?;
    let home = env::var_os("HOME").map(PathBuf::from);
    if cfg!(windows) {
        return env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join(nome).join("logs"));
    }
    if cfg!(target_os = "macos") {
        return home.map(|home| home.join("Library").join("Logs").join(nome));
    }
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|caminho| caminho.is_absolute())
        .or_else(|| home.map(|home| home.join(".local").join("state")))
        .map(|base| base.join(nome).join("logs"))
}

/// `.../com.omni.agents/engine` → `com.omni.agents`.
fn nome_da_identidade(pasta_de_dados: &Path) -> Option<String> {
    let pasta = if pasta_de_dados.file_name().is_some_and(|nome| nome == "engine") {
        pasta_de_dados.parent()?
    } else {
        pasta_de_dados
    };
    pasta.file_name().map(|nome| nome.to_string_lossy().into_owned())
}

/// Liga o log deste processo. Chamar uma vez, cedo. Falhar aqui nunca derruba o app.
pub fn iniciar(componente: &str, pasta_de_dados: &Path) {
    let Some(pasta) = pasta_de_logs(pasta_de_dados) else { return };
    iniciar_em(&pasta.join(format!("{componente}.log")));
}

pub fn iniciar_em(caminho: &Path) {
    if let Some(pai) = caminho.parent() {
        let _ = fs::create_dir_all(pai);
    }
    if let Ok(mut destino) = DESTINO.lock() {
        *destino = Some(Destino { caminho: caminho.to_path_buf(), arquivo: None });
    }
}

pub fn caminho_atual() -> Option<PathBuf> {
    DESTINO.lock().ok()?.as_ref().map(|destino| destino.caminho.clone())
}

/// Registra pânicos no log antes do comportamento padrão (que só imprime no stderr).
pub fn registrar_panicos() {
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let local = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let mensagem = info
            .payload()
            .downcast_ref::<&str>()
            .map(|texto| texto.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        erro("panico", &[("local", &local), ("mensagem", &mensagem)]);
        anterior(info);
    }));
}

pub fn info(evento: &str, campos: &[(&str, &str)]) {
    escrever("INFO", evento, campos);
}

pub fn aviso(evento: &str, campos: &[(&str, &str)]) {
    escrever("AVISO", evento, campos);
}

pub fn erro(evento: &str, campos: &[(&str, &str)]) {
    escrever("ERRO", evento, campos);
}

/// Aviso que pode se repetir em laço (a UI faz polling): a mesma `chave` só é gravada de novo
/// depois de 30 s.
pub fn aviso_sem_repetir(chave: &str, evento: &str, campos: &[(&str, &str)]) {
    use std::{collections::HashMap, time::{Duration, Instant}};
    static VISTOS: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
    if let Ok(mut vistos) = VISTOS.lock() {
        let vistos = vistos.get_or_insert_with(HashMap::new);
        if vistos.get(chave).is_some_and(|quando| quando.elapsed() < Duration::from_secs(30)) {
            return;
        }
        vistos.insert(chave.to_string(), Instant::now());
    }
    aviso(evento, campos);
}

/// Só grava com `OMNI_LOG_DEBUG=1` (modo de depuração ligado de propósito).
pub fn depurar(evento: &str, campos: &[(&str, &str)]) {
    if env::var_os("OMNI_LOG_DEBUG").is_some_and(|valor| valor == "1") {
        escrever("DEBUG", evento, campos);
    }
}

pub fn linha(nivel: &str, evento: &str, campos: &[(&str, &str)]) -> String {
    let mut linha = format!(
        "{} {nivel} {} pid={}",
        agora_utc().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
        limpar(evento),
        std::process::id()
    );
    for (chave, valor) in campos {
        linha.push(' ');
        linha.push_str(&limpar(chave));
        linha.push('=');
        let valor = limpar(valor);
        if valor.is_empty() || valor.contains(' ') {
            linha.push('"');
            linha.push_str(&valor.replace('"', "'"));
            linha.push('"');
        } else {
            linha.push_str(&valor);
        }
    }
    linha.push('\n');
    linha
}

/// Uma linha, sem controles, curta, e sem trechos entre aspas — mensagens de erro de parser (serde,
/// toml) citam o valor que falhou, e esse valor pode ser conteúdo do usuário.
pub fn limpar(valor: &str) -> String {
    let mut saida = String::with_capacity(valor.len().min(VALOR_MAXIMO));
    let mut dentro: Option<char> = None;
    for c in valor.chars() {
        match dentro {
            Some(fecha) if c == fecha => {
                dentro = None;
                saida.push(c);
            }
            Some(_) => {}
            None if c == '`' || c == '"' => {
                dentro = Some(c);
                saida.push(c);
                saida.push('…');
            }
            None if c.is_control() => saida.push(' '),
            None => saida.push(c),
        }
        if saida.chars().count() >= VALOR_MAXIMO {
            saida.push('…');
            break;
        }
    }
    saida.trim().to_string()
}

/// `chrono` aqui é compilado sem o relógio (`clock`), então a hora vem do `SystemTime`.
fn agora_utc() -> chrono::DateTime<chrono::Utc> {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default()
}

fn escrever(nivel: &str, evento: &str, campos: &[(&str, &str)]) {
    let texto = linha(nivel, evento, campos);
    let Ok(mut guarda) = DESTINO.lock() else { return };
    let Some(destino) = guarda.as_mut() else { return };
    if destino.arquivo.is_none() || precisa_girar(&destino.caminho) {
        destino.arquivo = None;
        girar(&destino.caminho);
        destino.arquivo = OpenOptions::new().create(true).append(true).open(&destino.caminho).ok();
    }
    if let Some(arquivo) = destino.arquivo.as_mut() {
        if arquivo.write_all(texto.as_bytes()).is_err() {
            destino.arquivo = None;
        }
    }
}

fn precisa_girar(caminho: &Path) -> bool {
    fs::metadata(caminho).is_ok_and(|meta| meta.len() >= TAMANHO_MAXIMO)
}

fn antigo(caminho: &Path, indice: usize) -> PathBuf {
    let base = caminho.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    caminho.with_file_name(format!("{base}.{indice}.log"))
}

/// `app.log` → `app.1.log` → `app.2.log` …; o mais velho é descartado.
fn girar(caminho: &Path) {
    if !precisa_girar(caminho) {
        return;
    }
    let _ = fs::remove_file(antigo(caminho, ARQUIVOS_ANTIGOS));
    for indice in (1..ARQUIVOS_ANTIGOS).rev() {
        let _ = fs::rename(antigo(caminho, indice), antigo(caminho, indice + 1));
    }
    let _ = fs::rename(caminho, antigo(caminho, 1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limpar_tira_trechos_citados_e_quebras_de_linha() {
        assert_eq!(limpar("unknown variant `segredo`, expected `ping`"), "unknown variant `…`, expected `…`");
        assert_eq!(limpar("invalid type: string \"meu prompt\"\nlinha 2"), "invalid type: string \"…\" linha 2");
        assert!(limpar(&"x".repeat(1000)).chars().count() <= VALOR_MAXIMO + 1);
    }

    #[test]
    fn linha_tem_nivel_evento_e_campos() {
        let texto = linha("INFO", "sessao_aberta", &[("id", "t-1"), ("cwd", "/a b")]);
        assert!(texto.contains(" INFO sessao_aberta pid="));
        assert!(texto.ends_with("id=t-1 cwd=\"/a b\"\n"));
    }

    #[test]
    fn pasta_de_logs_usa_o_nome_da_pasta_de_dados() {
        let pasta = Path::new("/x/Application Support/com.omni.agents.dev/engine");
        assert_eq!(nome_da_identidade(pasta).as_deref(), Some("com.omni.agents.dev"));
        assert_eq!(nome_da_identidade(Path::new("/x/com.fork")).as_deref(), Some("com.fork"));
    }

    #[test]
    fn gira_quando_passa_do_limite() {
        let dir = tempfile::tempdir().unwrap();
        let caminho = dir.path().join("app.log");
        fs::write(&caminho, vec![b'a'; TAMANHO_MAXIMO as usize]).unwrap();
        fs::write(antigo(&caminho, 1), "velho1").unwrap();
        girar(&caminho);
        assert!(!caminho.exists());
        assert_eq!(fs::metadata(antigo(&caminho, 1)).unwrap().len(), TAMANHO_MAXIMO);
        assert_eq!(fs::read_to_string(antigo(&caminho, 2)).unwrap(), "velho1");
    }
}
