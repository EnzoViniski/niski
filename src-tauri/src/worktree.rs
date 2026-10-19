//! Fork: agente numa branch diferente da que está aberta na pasta do projeto.
//!
//! Trocar a branch da pasta (`git checkout`) mudaria o chão de todos os agentes que rodam nela. Em vez
//! disso o agente novo ganha uma **worktree** do git: uma pasta própria com a branch escolhida, em
//! `<repo>/.omni-agents/worktrees/<branch>`, que divide o histórico com o repositório. Se a branch já
//! está aberta em alguma worktree (inclusive a pasta principal), usa aquela. `.omni-agents/` entra
//! no `.git/info/exclude` para não aparecer como arquivo novo no `git status`.
//! Só projeto local: em WSL/SSH o caminho da worktree seria do outro lado.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let saida = Command::new("git").args(args).current_dir(cwd).env("GIT_TERMINAL_PROMPT", "0").output()
        .map_err(|e| format!("git não encontrado no PATH: {e}"))?;
    if saida.status.success() { return Ok(String::from_utf8_lossy(&saida.stdout).trim().to_string()); }
    let erro = String::from_utf8_lossy(&saida.stderr).trim().to_string();
    Err(if erro.is_empty() { format!("`git {}` falhou", args.join(" ")) } else { erro })
}

/// Nome de pasta a partir da branch: `feat/login` → `feat-login`.
fn pasta_da_branch(branch: &str) -> String {
    branch.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).collect::<String>()
        .trim_matches(['-', '.']).to_string()
}

/// Worktrees existentes: (caminho, branch curta).
fn worktrees(raiz: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let lista = git(raiz, &["worktree", "list", "--porcelain"])?;
    let mut saida = Vec::new();
    let mut atual: Option<PathBuf> = None;
    for linha in lista.lines() {
        if let Some(c) = linha.strip_prefix("worktree ") { atual = Some(PathBuf::from(c)); }
        if let (Some(b), Some(c)) = (linha.strip_prefix("branch refs/heads/"), atual.as_ref()) { saida.push((c.clone(), b.to_string())); }
    }
    Ok(saida)
}

pub fn preparar(projeto: &Path, branch: &str, criar: bool) -> Result<PathBuf, String> {
    let branch = branch.trim();
    git(projeto, &["check-ref-format", "--branch", branch]).map_err(|_| format!("“{branch}” não é um nome de branch válido"))?;
    let raiz = PathBuf::from(git(projeto, &["rev-parse", "--show-toplevel"]).map_err(|_| "Esta pasta não é um repositório git".to_string())?);
    if !criar {
        if let Some((caminho, _)) = worktrees(&raiz)?.into_iter().find(|(_, b)| b == branch) { return Ok(caminho); }
    }
    let nome = pasta_da_branch(branch);
    if nome.is_empty() { return Err("Nome de branch inválido".into()); }
    let destino = raiz.join(".omni-agents").join("worktrees").join(&nome);
    if destino.exists() { return Err(format!("Já existe uma pasta em {}", destino.display())); }
    let destino_txt = destino.to_string_lossy().to_string();
    if criar { git(&raiz, &["worktree", "add", "-b", branch, &destino_txt])?; }
    else { git(&raiz, &["worktree", "add", &destino_txt, branch])?; }
    excluir_do_status(&raiz);
    omni_core::registro::info("worktree_criada", &[("nova_branch", if criar { "sim" } else { "nao" })]);
    Ok(destino)
}

fn excluir_do_status(raiz: &Path) {
    let Ok(comum) = git(raiz, &["rev-parse", "--git-common-dir"]) else { return };
    let comum = if Path::new(&comum).is_absolute() { PathBuf::from(comum) } else { raiz.join(comum) };
    let arquivo = comum.join("info").join("exclude");
    let atual = std::fs::read_to_string(&arquivo).unwrap_or_default();
    if atual.lines().any(|l| l.trim() == ".omni-agents/") { return; }
    let _ = std::fs::create_dir_all(comum.join("info"));
    let _ = std::fs::write(&arquivo, format!("{}{}.omni-agents/\n", atual, if atual.is_empty() || atual.ends_with('\n') { "" } else { "\n" }));
}

/// Pasta onde o agente vai rodar com a `branch` (criada a partir da atual se `criar`).
#[tauri::command(async)]
pub fn git_worktree_da_branch(project_path: String, branch: String, criar: bool) -> Result<String, String> {
    if omni_core::targets::resolve(&project_path, &crate::ssh::connections()).remote_path().is_some() {
        return Err("Branch separada por agente só funciona em projeto local".into());
    }
    preparar(Path::new(&project_path), &branch, criar).map(|p| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for args in [vec!["init", "-q", "-b", "main"], vec!["config", "user.email", "t@t"], vec!["config", "user.name", "t"],
                     vec!["commit", "-q", "--allow-empty", "-m", "inicio"], vec!["branch", "feat/login"]] {
            git(dir.path(), &args).unwrap();
        }
        dir
    }

    #[test]
    fn worktree_por_branch_sem_mexer_na_pasta_principal() {
        let dir = repo();
        let raiz = PathBuf::from(git(dir.path(), &["rev-parse", "--show-toplevel"]).unwrap());
        // Branch já aberta na pasta principal: usa ela mesma.
        assert_eq!(preparar(dir.path(), "main", false).unwrap(), raiz);
        // Outra branch existente: pasta própria, e a principal continua em main.
        let wt = preparar(dir.path(), "feat/login", false).unwrap();
        assert!(wt.ends_with(".omni-agents/worktrees/feat-login"));
        assert_eq!(git(&wt, &["branch", "--show-current"]).unwrap(), "feat/login");
        assert_eq!(git(dir.path(), &["branch", "--show-current"]).unwrap(), "main");
        // Segunda vez reaproveita.
        assert_eq!(preparar(dir.path(), "feat/login", false).unwrap(), wt);
        // Branch nova nasce da atual.
        let nova = preparar(dir.path(), "corrige-bug", true).unwrap();
        assert_eq!(git(&nova, &["branch", "--show-current"]).unwrap(), "corrige-bug");
        assert!(git(dir.path(), &["status", "--porcelain"]).unwrap().is_empty());
        assert!(preparar(dir.path(), "nome ruim..", true).is_err());
    }
}
