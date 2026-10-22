//! Fork: cópia em disco do que a interface guarda no localStorage (aparência, workspaces, abas,
//! ordem da lateral). O localStorage do WebKit é a fonte do dia a dia, mas perde as últimas gravações
//! quando o processo sai sem esperar o WebKit (reinício da atualização, app derrubado) e some inteiro
//! se a pasta do WebKit for limpa. A interface grava aqui a cada mudança e, ao abrir, restaura daqui
//! quando esta cópia é mais nova que a do localStorage (`src/lib/espelhoDaInterface.ts`).
//!
//! Arquivo `<pasta de dados do engine>/estado-da-interface.json`, só com as chaves de [`CHAVES`]
//! (nada de token: `omni-token` é do celular e nunca entra).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

pub const ARQUIVO: &str = "estado-da-interface.json";
const TAMANHO_MAXIMO: usize = 8 * 1024 * 1024;

/// Chaves do localStorage da interface do desktop que valem guardar.
pub const CHAVES: &[&str] = &[
    "omni-agents-workspaces",
    "omni-agents-workspace",
    "omni-agents-workspaces.backup",
    "omni-agents-settings",
    "omni-agents-kanban",
    "omni-agents-recent-projects",
    "omni-ordem-lateral",
    "omni-pastas-abertas",
    "omni-lateral-largura",
    "omni-todo",
    "medza-escolhas",
    "medza-orquestradoras-recolhidas",
    "medza-manha", // Fork: rotina da manhã (ajustes e sessão do dia).
    "medza-tarefas-ajustes", // Fork: tarefas (nome, pasta compartilhada, filtros).
];

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EstadoDaInterface {
    /// Quando a interface gravou (ms desde 1970). É o que decide quem é mais novo ao abrir.
    pub salvo_em: u64,
    pub chaves: BTreeMap<String, String>,
}

fn filtrado(estado: EstadoDaInterface) -> EstadoDaInterface {
    let chaves = estado.chaves.into_iter().filter(|(chave, _)| CHAVES.contains(&chave.as_str())).collect();
    EstadoDaInterface { salvo_em: estado.salvo_em, chaves }
}

pub fn ler(dir: &Path) -> Option<EstadoDaInterface> {
    let dados = fs::read(dir.join(ARQUIVO)).ok()?;
    if dados.len() > TAMANHO_MAXIMO { return None; }
    serde_json::from_slice::<EstadoDaInterface>(&dados).ok().map(filtrado)
}

/// Grava por cima (escrita atômica). Uma cópia mais velha que a gravada não substitui a nova: duas
/// janelas (ou uma gravação atrasada) não voltam o estado para trás.
pub fn gravar(dir: &Path, estado: EstadoDaInterface) -> io::Result<bool> {
    let estado = filtrado(estado);
    let tamanho: usize = estado.chaves.iter().map(|(chave, valor)| chave.len() + valor.len()).sum();
    if tamanho > TAMANHO_MAXIMO {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "estado da interface grande demais"));
    }
    if ler(dir).is_some_and(|atual| atual.salvo_em > estado.salvo_em) {
        return Ok(false);
    }
    fs::create_dir_all(dir)?;
    let destino = dir.join(ARQUIVO);
    let temporario = dir.join(format!(".{ARQUIVO}.{}.tmp", std::process::id()));
    fs::write(&temporario, serde_json::to_vec(&estado)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporario, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(temporario, destino)?;
    Ok(true)
}

#[tauri::command(async)]
pub fn ler_estado_da_interface() -> Result<Option<EstadoDaInterface>, String> {
    Ok(ler(&crate::engine_client::engine_dir()?))
}

#[tauri::command(async)]
pub fn gravar_estado_da_interface(estado: EstadoDaInterface) -> Result<bool, String> {
    let dir = crate::engine_client::engine_dir()?;
    gravar(&dir, estado).map_err(|erro| {
        omni_core::registro::aviso("estado_da_interface_falhou", &[("erro", &erro.kind().to_string())]);
        format!("Não consegui guardar o estado da interface: {erro}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn estado(salvo_em: u64, pares: &[(&str, &str)]) -> EstadoDaInterface {
        EstadoDaInterface { salvo_em, chaves: pares.iter().map(|(c, v)| (c.to_string(), v.to_string())).collect() }
    }

    #[test]
    fn grava_e_le_so_as_chaves_conhecidas() {
        let dir = tempfile::tempdir().unwrap();
        gravar(dir.path(), estado(10, &[("omni-agents-settings", "{\"accent\":\"verde\"}"), ("omni-token", "segredo")])).unwrap();
        let lido = ler(dir.path()).unwrap();
        assert_eq!(lido, estado(10, &[("omni-agents-settings", "{\"accent\":\"verde\"}")]));
        let bruto = fs::read_to_string(dir.path().join(ARQUIVO)).unwrap();
        assert!(!bruto.contains("segredo"));
    }

    #[test]
    fn copia_mais_velha_nao_substitui_a_nova() {
        let dir = tempfile::tempdir().unwrap();
        assert!(gravar(dir.path(), estado(20, &[("omni-todo", "novo")])).unwrap());
        assert!(!gravar(dir.path(), estado(10, &[("omni-todo", "velho")])).unwrap());
        assert_eq!(ler(dir.path()).unwrap().chaves["omni-todo"], "novo");
    }

    #[test]
    fn arquivo_corrompido_vira_sem_copia() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(ARQUIVO), "{nao é json").unwrap();
        assert!(ler(dir.path()).is_none());
        assert!(gravar(dir.path(), estado(5, &[("omni-todo", "[]")])).unwrap());
        assert_eq!(ler(dir.path()).unwrap().salvo_em, 5);
    }

    #[cfg(unix)]
    #[test]
    fn arquivo_so_do_dono() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        gravar(dir.path(), estado(1, &[("omni-todo", "[]")])).unwrap();
        let modo = fs::metadata(dir.path().join(ARQUIVO)).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o600);
    }
}
