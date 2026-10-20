//! Read-only provider data. Never reads authentication files or calls provider APIs.
pub mod cli_path;
pub mod agendamentos; // Fork: calendário e destinos dos agendamentos.
// Fork: comandos interativos permitidos nos controles da Conversa.
pub mod comandos_oficiais;
pub mod usage;
pub mod conversations;
pub mod feed;
pub mod history;
// Fork: busca textual limitada nos transcripts do índice.
pub mod busca_historico;
pub mod runtime;
pub mod targets;
pub mod compartilhar;
pub mod extensoes;
pub mod relatorio;
pub mod agentes;
pub mod fluxos;
pub mod registro;
pub mod antigravity;
// Fork: pasta dos transcripts do Claude igual à da CLI (caminho longo é cortado com hash).
pub mod pasta_do_claude;
// Fork (retomar-sempre): a conversa em qualquer conta e a pasta principal de uma worktree apagada.
pub mod achar_transcript;
// Fork: modelos do Claude com versão (não só os apelidos) e arquivos que o agente gerou.
pub mod modelos_claude;
pub mod entregaveis;
// Fork: registro e regra pura do orquestrador de agentes (o servidor MCP mora no engine).
pub mod orquestracao;
// Fork: teto de sessões ativas (configurável) e agente em worktree isolada do git.
pub mod limite_de_sessoes;
pub mod worktree_isolada;
// Fork: conversa rica — lista de tarefas, plano do plan mode, modo de permissão e diffs do agente.
pub mod plano_do_agente;
pub mod diff_do_agente;
// Fork (Onda 2): ganchos HTTP do Claude Code por sessão, pedidos de permissão e statusline (parte pura).
pub mod ganchos;
// Fork (Fase 3): fila de contas — escolha pura da conta com explicação.
pub mod roteamento;
// Fork (fix-orquestracao): ler-alterar-gravar o índice de conversas sob trava, com `.tmp` exclusivo.
pub mod indice_de_conversas;
pub mod memoria_compartilhada;
pub mod arquivo_de_conversas;

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
    let mut lidos: Vec<Profile> = fs::read(dir.join("profiles.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<Store>(&bytes).ok())
        .map(|store| store.profiles).unwrap_or_default();
    let descobertos = perfis_descobertos(&lidos);
    lidos.extend(descobertos);
    lidos
}

/// Fork: raiz das pastas de conta criadas pelo app (`<dados>/<PASTA_DE_PERFIS>/profiles`), comum a
/// todas as variantes (instalado, dev, Local). Definida no começo do engine e do app.
static RAIZ_DE_PERFIS: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn definir_raiz_de_perfis(raiz: PathBuf) { let _ = RAIZ_DE_PERFIS.set(raiz); }

/// Fork: contas que têm pasta (`<raiz>/<provider>/<id>`) mas sumiram do `profiles.json` desta pasta
/// de dados — o registro é por variante e as pastas são compartilhadas. Sem isto a conversa feita
/// numa delas perdia o histórico no chat ("nenhum transcript foi associado") e o chip mostrava a
/// conta errada. Só Claude e Codex: os únicos com conta extra.
pub fn perfis_descobertos(conhecidos: &[Profile]) -> Vec<Profile> {
    let Some(raiz) = RAIZ_DE_PERFIS.get() else { return Vec::new() };
    perfis_em(raiz, conhecidos)
}

fn perfis_em(raiz: &Path, conhecidos: &[Profile]) -> Vec<Profile> {
    let mut achados = Vec::new();
    for provider in ["claude", "codex"] {
        let Ok(entradas) = fs::read_dir(raiz.join(provider)) else { continue };
        for entrada in entradas.flatten() {
            if !entrada.file_type().is_ok_and(|t| t.is_dir()) { continue; }
            let id = entrada.file_name().to_string_lossy().into_owned();
            if id.starts_with('.') || id.is_empty() { continue; }
            let pasta = entrada.path().to_string_lossy().into_owned();
            let ja = conhecidos.iter().any(|p| (p.id == id && p.provider == provider) || p.config_dir == pasta);
            if ja { continue; }
            let criado = entrada.metadata().ok().and_then(|m| m.created().or_else(|_| m.modified()).ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
            achados.push(Profile { id: id.clone(), provider: provider.into(), name: id, config_dir: pasta, builtin: false,
                created_at_ms: criado, last_used_at_ms: None, authenticated: false, email: None });
        }
    }
    achados
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
mod perfis_orfaos {
    use super::*;

    #[test]
    fn pasta_de_conta_fora_do_registro_volta_como_perfil() {
        let raiz = std::env::temp_dir().join(format!("omni-perfis-{}", std::process::id()));
        fs::create_dir_all(raiz.join("claude").join("trabalho")).unwrap();
        fs::create_dir_all(raiz.join("claude").join("registrada")).unwrap();
        fs::create_dir_all(raiz.join("gemini").join("velha")).unwrap();
        let conhecida = Profile { id: "registrada".into(), provider: "claude".into(), name: "R".into(),
            config_dir: raiz.join("claude").join("registrada").to_string_lossy().into(), builtin: false,
            created_at_ms: 0, last_used_at_ms: None, authenticated: false, email: None };
        let achados = perfis_em(&raiz, &[conhecida]);
        assert_eq!(achados.iter().map(|p| (p.provider.as_str(), p.id.as_str())).collect::<Vec<_>>(), vec![("claude", "trabalho")]);
        let _ = fs::remove_dir_all(&raiz);
    }
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
    /// `CLAUDE_CONFIG_DIR`, mesmo para `~/.claude`, pode fazer a CLI procurar outra entrada e pedir login).
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
// Fork: subconjunto puro do protocolo oficial do Codex app-server.
pub mod codex_protocolo;
