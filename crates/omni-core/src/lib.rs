//! Read-only provider data. Never reads authentication files or calls provider APIs.
pub mod cli_path;
pub mod usage;
pub mod conversations;
pub mod feed;
pub mod history;
pub mod runtime;
pub mod targets;
pub mod compartilhar;
pub mod extensoes;
pub mod relatorio;
pub mod agentes;
pub mod fluxos;
pub mod registro;

use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub config_dir: String,
    #[serde(default)] pub builtin: bool,
    pub created_at_ms: u64,
    #[serde(default)] pub last_used_at_ms: Option<u64>,
    /// Calculado a cada leitura, nunca lido do `profiles.json`. Só `skip_deserializing` — com
    /// `skip` o campo também não chegava ao front, que via toda conta como "sem login" e deixava o
    /// failover sem nenhuma conta elegível. Falso não é serializado, então o arquivo segue limpo.
    #[serde(default, skip_deserializing, skip_serializing_if = "std::ops::Not::not")]
    pub authenticated: bool,
    /// E-mail da conta logada, para distinguir perfis. Mesma regra: calculado, nunca persistido.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Env var que cada CLI lê para trocar de diretório de configuração. `None` = o provider não
/// suporta isolamento, então só existe o perfil padrão dele.
///
/// Mora aqui, e não só no lado Tauri, porque o engine também precisa montar o ambiente: uma sessão
/// aberta pelo celular nasce dentro do engine, com a janela do desktop possivelmente fechada.
pub fn config_dir_var(provider: &str) -> Option<&'static str> {
    match provider {
        "claude" => Some("CLAUDE_CONFIG_DIR"),
        "codex" => Some("CODEX_HOME"),
        _ => None,
    }
}

/// Variáveis que isolam a conta. Perfil `builtin` não sobrescreve nada: ele **é** o diretório
/// nativo do CLI, e apontar a env var para ele mudaria o comportamento padrão sem necessidade.
pub fn env_for(profile: &Profile) -> Vec<(String, String)> {
    match config_dir_var(&profile.provider) {
        Some(variable) if !profile.builtin => vec![(variable.to_owned(), profile.config_dir.clone())],
        _ => Vec::new(),
    }
}

pub fn profiles(dir: &Path) -> Vec<Profile> {
    #[derive(Deserialize)] struct Store { profiles: Vec<Profile> }
    fs::read(dir.join("profiles.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<Store>(&bytes).ok())
        .map(|store| store.profiles).unwrap_or_default()
}

/// Only visit ordinary directories/files, never follow profile symlinks into other accounts.
pub fn rollouts(root: &Path) -> Vec<PathBuf> {
    fn visit(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
        if depth > 4 { return; }
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() { visit(&entry.path(), depth + 1, found); }
            else if kind.is_file() && entry.file_name().to_string_lossy().starts_with("rollout-")
                && entry.path().extension().is_some_and(|ext| ext == "jsonl") { found.push(entry.path()); }
        }
    }
    let mut found = Vec::new();
    visit(root, 0, &mut found);
    found
}

#[cfg(test)]
mod isolamento_de_conta {
    use super::*;

    fn perfil(provider: &str, builtin: bool, dir: &str) -> Profile {
        Profile { id: format!("{provider}-x"), provider: provider.into(), name: "x".into(), config_dir: dir.into(),
            builtin, created_at_ms: 0, last_used_at_ms: None, authenticated: false, email: None }
    }

    /// Regra do fork: um perfil nunca pode cair na conta de outro. O padrão não define variável
    /// nenhuma (no macOS o token dele mora no Keychain sob o nome da conta nativa; definir
    /// `CLAUDE_CONFIG_DIR`, mesmo para `~/.claude`, faria a CLI procurar outra entrada e pedir login).
    /// Cada extra aponta para a própria pasta.
    #[test]
    fn padrao_nao_define_pasta_e_extras_definem_a_propria() {
        assert!(env_for(&perfil("claude", true, "/home/u/.claude")).is_empty());
        assert!(env_for(&perfil("codex", true, "/home/u/.codex")).is_empty());
        assert_eq!(env_for(&perfil("claude", false, "/dados/claude/team")),
            vec![("CLAUDE_CONFIG_DIR".to_string(), "/dados/claude/team".to_string())]);
        assert_eq!(env_for(&perfil("codex", false, "/dados/codex/b")),
            vec![("CODEX_HOME".to_string(), "/dados/codex/b".to_string())]);
        assert!(env_for(&perfil("cursor", false, "/dados/cursor/c")).is_empty());
    }
}
