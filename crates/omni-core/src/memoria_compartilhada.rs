//! Preferências explícitas do usuário, separadas dos transcripts e das credenciais das contas.
//! Só este arquivo é compartilhado: não varre nem importa dados pessoais automaticamente.
use serde::{Deserialize, Serialize};
use std::{fs, io::{self, Write}, path::Path};

pub const MAX_CARACTERES: usize = 12_000;
pub const ARQUIVO: &str = "memoria-compartilhada.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memoria {
    pub ativa: bool,
    pub texto: String,
}

pub fn ler(dir: &Path) -> io::Result<Memoria> {
    match fs::read(dir.join(ARQUIVO)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Não foi possível ler a memória compartilhada. O arquivo foi preservado.")),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => Ok(Memoria::default()),
        Err(erro) => Err(erro),
    }
}

pub fn salvar(dir: &Path, memoria: &Memoria) -> io::Result<()> {
    if memoria.texto.chars().count() > MAX_CARACTERES || memoria.texto.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Use até 12000 caracteres de texto, sem caracteres de controle."));
    }
    fs::create_dir_all(dir)?;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCIA: AtomicU64 = AtomicU64::new(0);
    let temporario = dir.join(format!(".{ARQUIVO}.{}.{}.tmp", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    let resultado = (|| {
        let mut arquivo = opcoes.open(&temporario)?;
        arquivo.write_all(&serde_json::to_vec_pretty(memoria)?)?;
        arquivo.sync_all()?;
        fs::rename(&temporario, dir.join(ARQUIVO))
    })();
    if resultado.is_err() { let _ = fs::remove_file(temporario); }
    resultado
}

/// Lê só a instrução adicional do config.toml da conta, sem consultar arquivos de autenticação.
pub fn instrucao_codex(config_dir: &Path, memoria: &str) -> io::Result<String> {
    let anterior = match fs::read_to_string(config_dir.join("config.toml")) {
        Ok(texto) => texto.parse::<toml::Table>().map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Configuração do Codex ilegível"))?
            .get("developer_instructions").and_then(toml::Value::as_str).unwrap_or("").to_owned(),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => String::new(),
        Err(erro) => return Err(erro),
    };
    Ok(format!("developer_instructions={}", serde_json::to_string(&format!("{anterior}\n\n{memoria}"))?))
}

pub fn contexto(memoria: &Memoria) -> Option<String> {
    (memoria.ativa && !memoria.texto.trim().is_empty()).then(|| format!(
        "Preferências compartilhadas pelo usuário do Medza Agents. A tarefa atual e as regras do projeto continuam valendo. Não invente informações pessoais.\n\n{}", memoria.texto.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persiste_texto_explicito_e_desativar_interrompe_contexto() {
        let dir = tempfile::tempdir().unwrap();
        assert!(contexto(&ler(dir.path()).unwrap()).is_none());
        salvar(dir.path(), &Memoria { ativa: true, texto: "Enzo; português, decisões objetivas".into() }).unwrap();
        let mut memoria = ler(dir.path()).unwrap();
        assert!(contexto(&memoria).unwrap().contains("Enzo"));
        memoria.ativa = false;
        assert!(contexto(&memoria).is_none());
        assert!(salvar(dir.path(), &Memoria { ativa: true, texto: "x".repeat(MAX_CARACTERES + 1) }).is_err());
        assert!(salvar(dir.path(), &Memoria { ativa: true, texto: "x\0".into() }).is_err());
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(dir.path().join(ARQUIVO)).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
    #[test]
    fn preserva_instrucao_do_codex_e_cita_texto_como_toml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.toml"), "developer_instructions = 'Regra da conta'\n").unwrap();
        let argumento = instrucao_codex(dir.path(), "Olá \"Enzo\"\n$(`teste`)").unwrap();
        let tabela: toml::Table = argumento.parse().unwrap();
        assert_eq!(tabela["developer_instructions"].as_str().unwrap(), "Regra da conta\n\nOlá \"Enzo\"\n$(`teste`)");
        assert_eq!(fs::read_to_string(dir.path().join("config.toml")).unwrap(), "developer_instructions = 'Regra da conta'\n");
    }
}
