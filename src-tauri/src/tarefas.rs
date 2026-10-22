//! Fork (Tarefas): uma tarefa por arquivo JSON, para dividir com outra pessoa sem servidor.
//!
//! A pasta é `<dados>/tarefas` ou, se o usuário escolher, uma pasta sincronizada (Google Drive,
//! iCloud, Dropbox...) onde o app cria `niski-tarefas/`. Um arquivo por tarefa evita que duas
//! pessoas editando coisas diferentes briguem pelo mesmo arquivo; apagar grava a tarefa marcada
//! como apagada (o sincronizador espalha a marca, não um arquivo sumido). O domínio (campos,
//! mescla, recorrência) mora no front, em `src/features/tarefas/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Subpasta criada dentro da pasta compartilhada escolhida.
pub const SUBPASTA: &str = "niski-tarefas";
const TAMANHO_MAXIMO: u64 = 1024 * 1024;
const TIPOS: &[&str] = &["tarefas", "pessoas"];

fn invalido(texto: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, texto.to_string())
}

fn conferir(tipo: &str, id: &str) -> io::Result<()> {
    if !TIPOS.contains(&tipo) {
        return Err(invalido("Tipo de arquivo desconhecido."));
    }
    let valido = !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valido {
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
    let pasta = raiz.join(tipo);
    let Ok(entradas) = fs::read_dir(&pasta) else { return Ok(Vec::new()) };
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

fn padrao() -> Result<PathBuf, String> {
    Ok(crate::engine_client::engine_dir()?.join("tarefas"))
}

fn texto_do_erro(erro: io::Error) -> String {
    if erro.kind() == io::ErrorKind::InvalidInput { erro.to_string() } else { format!("Não consegui acessar as tarefas: {erro}") }
}

#[tauri::command(async)]
pub fn tarefas_listar(pasta: Option<String>, tipo: String) -> Result<Vec<String>, String> {
    let raiz = raiz(pasta.as_deref(), &padrao()?).map_err(texto_do_erro)?;
    listar(&raiz, &tipo).map_err(texto_do_erro)
}

#[tauri::command(async)]
pub fn tarefas_gravar(pasta: Option<String>, tipo: String, id: String, conteudo: String) -> Result<(), String> {
    let raiz = raiz(pasta.as_deref(), &padrao()?).map_err(texto_do_erro)?;
    gravar(&raiz, &tipo, &id, &conteudo).map_err(texto_do_erro)
}

/// Copia as tarefas locais para a pasta compartilhada recém-escolhida (sem sobrescrever as que já
/// estão lá: a pessoa que compartilhou primeiro manda). Devolve quantas foram copiadas.
#[tauri::command(async)]
pub fn tarefas_levar_para(pasta: String) -> Result<usize, String> {
    let origem = padrao()?;
    let destino = raiz(Some(&pasta), &origem).map_err(texto_do_erro)?;
    levar(&origem, &destino).map_err(texto_do_erro)
}

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
}
