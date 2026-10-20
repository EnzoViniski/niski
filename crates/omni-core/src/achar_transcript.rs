//! Fork (retomar-sempre): onde está a conversa de uma CLI, em **qualquer** conta conhecida, e para
//! onde ela vai quando a pasta de trabalho sumiu.
//!
//! Retomar falhava ("No conversation found with session ID", ou `--session-id` "já em uso") sempre
//! que o OMNI procurava o transcript num lugar só: a conta da sessão e a pasta calculada do `cwd`. Os
//! casos reais: a conversa ficou em outra conta (failover, conta trocada, perfil fora do
//! `profiles.json` desta variante), o caminho foi escrito de outro jeito (`/tmp` × `/private/tmp`,
//! caminho longo com hash da CLI no Bun) ou o agente mudou de pasta no meio. Aqui:
//! - [`achar_claude`]: conta preferida primeiro, depois as outras; em cada uma, a pasta do `cwd`
//!   (com as variantes de `pasta_do_claude`) e, por fim, qualquer `projects/*/<id>.jsonl` — só nomes
//!   de arquivo, nada é lido.
//! - [`achar_codex`]: o rollout `rollout-<data>-<id>.jsonl` em `sessions/` (e `archived_sessions/`)
//!   de cada conta.
//! - [`pasta_principal`]: a worktree apagada vira o repositório de onde ela saiu.

use crate::Profile;
use std::path::{Path, PathBuf};

/// Conversa achada: a conta (perfil) e o arquivo.
#[derive(Debug, Clone)]
pub struct Achado {
    pub perfil: Profile,
    pub caminho: PathBuf,
}

/// Contas do provider, a preferida primeiro (as outras na ordem em que vieram).
fn em_ordem<'a>(perfis: &'a [Profile], provider: &str, preferida: Option<&str>) -> Vec<&'a Profile> {
    let mut lista: Vec<&Profile> = perfis.iter().filter(|p| p.provider == provider).collect();
    lista.sort_by_key(|p| Some(p.id.as_str()) != preferida);
    // Duas entradas para a mesma pasta (perfil descoberto + cadastrado) só custariam outra varredura.
    let mut vistas: Vec<PathBuf> = Vec::new();
    lista.retain(|p| {
        let pasta = Path::new(&p.config_dir).canonicalize().unwrap_or_else(|_| PathBuf::from(&p.config_dir));
        if vistas.contains(&pasta) { return false; }
        vistas.push(pasta);
        true
    });
    lista
}

/// Só caracteres de id: o valor vira parte de nome de arquivo.
fn id_seguro(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// O transcript `<id>.jsonl` do Claude em uma conta: pasta do `cwd` (e variantes), senão qualquer
/// pasta de projeto.
pub fn claude_na_conta(config_dir: &Path, cwd: &str, id: &str) -> Option<PathBuf> {
    if !id_seguro(id) { return None; }
    let calculado = crate::pasta_do_claude::localizar(config_dir, cwd, id);
    if calculado.is_file() { return Some(calculado); }
    let arquivo = format!("{id}.jsonl");
    std::fs::read_dir(config_dir.join("projects")).into_iter().flatten().flatten()
        .map(|pasta| pasta.path().join(&arquivo))
        .find(|caminho| caminho.is_file())
}

/// Procura a conversa Claude em todas as contas, começando pela `preferida`.
pub fn achar_claude(perfis: &[Profile], preferida: Option<&str>, cwd: &str, id: &str) -> Option<Achado> {
    em_ordem(perfis, "claude", preferida).into_iter().find_map(|perfil| {
        claude_na_conta(Path::new(&perfil.config_dir), cwd, id).map(|caminho| Achado { perfil: perfil.clone(), caminho })
    })
}

/// O rollout do Codex com este id numa conta (`sessions/AAAA/MM/DD/rollout-*-<id>.jsonl`).
pub fn codex_na_conta(config_dir: &Path, id: &str) -> Option<PathBuf> {
    if !id_seguro(id) { return None; }
    let fim = format!("-{id}.jsonl");
    for raiz in ["sessions", "archived_sessions"] {
        let mut pilha = vec![(config_dir.join(raiz), 0_u8)];
        while let Some((pasta, nivel)) = pilha.pop() {
            for entrada in std::fs::read_dir(&pasta).into_iter().flatten().flatten() {
                let caminho = entrada.path();
                let nome = entrada.file_name().to_string_lossy().into_owned();
                if caminho.is_dir() {
                    if nivel < 4 { pilha.push((caminho, nivel + 1)); }
                } else if nome.starts_with("rollout-") && nome.ends_with(&fim) {
                    return Some(caminho);
                }
            }
        }
    }
    None
}

pub fn achar_codex(perfis: &[Profile], preferida: Option<&str>, id: &str) -> Option<Achado> {
    em_ordem(perfis, "codex", preferida).into_iter().find_map(|perfil| {
        codex_na_conta(Path::new(&perfil.config_dir), id).map(|caminho| Achado { perfil: perfil.clone(), caminho })
    })
}

/// Onde a CLI da conta `alvo` procura esta conversa. Claude: pela pasta do `cwd`; Codex: mesma posição
/// relativa (dentro do CODEX_HOME) que o arquivo tem na conta de origem.
pub fn destino_na_conta(achado: &Achado, alvo: &Profile, cwd: &str, id: &str) -> Option<PathBuf> {
    match alvo.provider.as_str() {
        "claude" => Some(Path::new(&alvo.config_dir).join("projects").join(crate::pasta_do_claude::slug(cwd)).join(format!("{id}.jsonl"))),
        "codex" => {
            let raiz = Path::new(&achado.perfil.config_dir).canonicalize().ok()?;
            let relativo = achado.caminho.canonicalize().ok()?.strip_prefix(&raiz).ok()?.to_path_buf();
            Some(Path::new(&alvo.config_dir).join(relativo))
        }
        _ => None,
    }
}

/// Deixa a conversa onde a CLI da conta `alvo`, aberta em `cwd`, vai procurar. Copia (nunca move:
/// o original continua valendo para a conta dele). Devolve o caminho onde ela ficou.
pub fn colocar_onde_a_cli_procura(achado: &Achado, alvo: &Profile, cwd: &str, id: &str) -> std::io::Result<PathBuf> {
    // Claude já no lugar: a pasta calculada (ou uma variante dela) desta conta tem o arquivo.
    if alvo.provider == "claude" {
        let atual = crate::pasta_do_claude::localizar(Path::new(&alvo.config_dir), cwd, id);
        if atual.is_file() { return Ok(atual); }
    }
    if alvo.provider == "codex" && mesma_pasta(&achado.perfil.config_dir, &alvo.config_dir) {
        return Ok(achado.caminho.clone());
    }
    let destino = destino_na_conta(achado, alvo, cwd, id)
        .ok_or_else(|| std::io::Error::other("conta sem lugar conhecido para a conversa"))?;
    if destino != achado.caminho {
        if let Some(pasta) = destino.parent() { std::fs::create_dir_all(pasta)?; }
        std::fs::copy(&achado.caminho, &destino)?;
    }
    Ok(destino)
}

fn mesma_pasta(a: &str, b: &str) -> bool {
    let canon = |p: &str| Path::new(p).canonicalize().unwrap_or_else(|_| PathBuf::from(p));
    canon(a) == canon(b)
}

/// Pasta de trabalho que sumiu (worktree apagada, por exemplo) → o repositório principal dela, ou a
/// pasta existente mais próxima que seja repositório. `conhecidas` são as pastas dos projetos abertos
/// no app. `None` quando a pasta existe ou não há sugestão.
pub fn pasta_principal(cwd: &str, conhecidas: &[String]) -> Option<String> {
    let alvo = Path::new(cwd);
    if alvo.is_dir() { return None; }
    let existe = |p: &Path| p.is_dir();
    // 1. Worktree isolada do Medza: `<pai>/.medza-worktrees/<repo>/<nome>/<sub>` → `<pai>/<repo>/<sub>`.
    let partes: Vec<String> = alvo.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if let Some(i) = partes.iter().rposition(|p| p == crate::worktree_isolada::PASTA) {
        if let Some(repo) = partes.get(i + 1) {
            let mut principal: PathBuf = partes[..i].iter().collect();
            principal.push(repo);
            let sub: PathBuf = partes.iter().skip(i + 3).collect();
            let com_sub = principal.join(&sub);
            if !sub.as_os_str().is_empty() && existe(&com_sub) { return Some(com_sub.to_string_lossy().into_owned()); }
            if existe(&principal) { return Some(principal.to_string_lossy().into_owned()); }
        }
    }
    let conhecidas: Vec<&Path> = conhecidas.iter().map(Path::new).filter(|p| existe(p)).collect();
    // 2. Um projeto aberto cujo git ainda registra esta worktree (`.git/worktrees/*/gitdir`).
    let gitdir_esperado = alvo.join(".git");
    for projeto in &conhecidas {
        for entrada in std::fs::read_dir(projeto.join(".git").join("worktrees")).into_iter().flatten().flatten() {
            let Ok(texto) = std::fs::read_to_string(entrada.path().join("gitdir")) else { continue };
            if Path::new(texto.trim()) == gitdir_esperado { return Some(projeto.to_string_lossy().into_owned()); }
        }
    }
    // 3. Um projeto aberto que contém a pasta (worktree do Claude em `<repo>/.claude/worktrees/x`).
    if let Some(dono) = conhecidas.iter().filter(|p| alvo.starts_with(p)).max_by_key(|p| p.as_os_str().len()) {
        return Some(dono.to_string_lossy().into_owned());
    }
    // 4. Um projeto aberto com o mesmo nome de pasta (worktree do Codex: `~/.codex/worktrees/<hash>/<repo>`).
    if let Some(nome) = alvo.file_name() {
        if let Some(mesmo) = conhecidas.iter().find(|p| p.file_name() == Some(nome)) { return Some(mesmo.to_string_lossy().into_owned()); }
    }
    // 5. A pasta existente mais próxima que é repositório.
    let mut pai = alvo.parent();
    while let Some(pasta) = pai {
        if pasta.parent().is_none() { break; }
        if existe(pasta) && pasta.join(".git").exists() { return Some(pasta.to_string_lossy().into_owned()); }
        pai = pasta.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perfil(id: &str, provider: &str, dir: &Path) -> Profile {
        Profile { id: id.into(), provider: provider.into(), name: id.into(), config_dir: dir.to_string_lossy().into_owned(),
            builtin: false, created_at_ms: 0, last_used_at_ms: None, authenticated: false, email: None }
    }

    #[test]
    fn claude_achado_em_outra_conta_e_copiado_para_a_da_sessao() {
        let raiz = tempfile::tempdir().unwrap();
        let (a, b) = (raiz.path().join("a"), raiz.path().join("b"));
        let pasta = b.join("projects").join("-repo-x");
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("s1.jsonl"), "{}\n").unwrap();
        let perfis = [perfil("a", "claude", &a), perfil("b", "claude", &b), perfil("c", "codex", &a)];
        let achado = achar_claude(&perfis, Some("a"), "/repo/x", "s1").expect("achou na conta b");
        assert_eq!(achado.perfil.id, "b");
        let destino = colocar_onde_a_cli_procura(&achado, &perfis[0], "/repo/x", "s1").unwrap();
        assert_eq!(destino, a.join("projects").join("-repo-x").join("s1.jsonl"));
        assert!(destino.is_file());
        assert!(achar_claude(&perfis, Some("a"), "/repo/x", "nada").is_none());
        assert!(achar_claude(&perfis, None, "/repo/x", "../x").is_none(), "id com barra não vira caminho");
    }

    #[test]
    fn claude_em_outra_pasta_de_projeto_da_mesma_conta_vai_para_a_do_cwd() {
        let raiz = tempfile::tempdir().unwrap();
        let outra = raiz.path().join("projects").join("-repo-x-sub");
        std::fs::create_dir_all(&outra).unwrap();
        std::fs::write(outra.join("s1.jsonl"), "{}\n").unwrap();
        let perfis = [perfil("a", "claude", raiz.path())];
        let achado = achar_claude(&perfis, Some("a"), "/repo/x", "s1").unwrap();
        assert_eq!(achado.caminho, outra.join("s1.jsonl"));
        let destino = colocar_onde_a_cli_procura(&achado, &perfis[0], "/repo/x", "s1").unwrap();
        assert_eq!(destino, raiz.path().join("projects").join("-repo-x").join("s1.jsonl"));
    }

    #[cfg(unix)]
    #[test]
    fn tmp_e_private_tmp_sao_a_mesma_pasta() {
        let raiz = tempfile::tempdir().unwrap();
        let pasta = raiz.path().join("projects").join("-private-tmp-proj");
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("s1.jsonl"), "{}\n").unwrap();
        assert_eq!(crate::pasta_do_claude::localizar(raiz.path(), "/tmp/proj", "s1"), pasta.join("s1.jsonl"));
        let curta = raiz.path().join("projects").join("-tmp-outro");
        std::fs::create_dir_all(&curta).unwrap();
        std::fs::write(curta.join("s2.jsonl"), "{}\n").unwrap();
        assert_eq!(crate::pasta_do_claude::localizar(raiz.path(), "/private/tmp/outro", "s2"), curta.join("s2.jsonl"));
    }

    #[test]
    fn codex_achado_pelo_nome_do_rollout_em_qualquer_conta() {
        let raiz = tempfile::tempdir().unwrap();
        let (a, b) = (raiz.path().join("a"), raiz.path().join("b"));
        let dia = b.join("sessions").join("2026").join("10").join("19");
        std::fs::create_dir_all(&dia).unwrap();
        std::fs::create_dir_all(a.join("sessions")).unwrap();
        let id = "01a136b9-c1c7-7b21-95b4-ace8ba4ce273";
        std::fs::write(dia.join(format!("rollout-2026-10-19T10-00-00-{id}.jsonl")), "{}\n").unwrap();
        let perfis = [perfil("a", "codex", &a), perfil("b", "codex", &b)];
        let achado = achar_codex(&perfis, Some("a"), id).expect("achou na conta b");
        assert_eq!(achado.perfil.id, "b");
        let destino = colocar_onde_a_cli_procura(&achado, &perfis[0], "/qualquer", id).unwrap();
        assert!(destino.starts_with(&a) && destino.is_file(), "copiado na mesma posição dentro da conta a");
        assert!(achar_codex(&perfis, None, "inexistente").is_none());
    }

    #[test]
    fn worktree_apagada_sugere_o_repositorio_principal() {
        let raiz = tempfile::tempdir().unwrap();
        let repo = raiz.path().join("app");
        std::fs::create_dir_all(repo.join(".git").join("worktrees").join("frente")).unwrap();
        std::fs::create_dir_all(repo.join("src")).unwrap();
        let medza = raiz.path().join(".medza-worktrees").join("app").join("tarefa").join("src");
        assert_eq!(pasta_principal(&medza.to_string_lossy(), &[]), Some(repo.join("src").to_string_lossy().into_owned()));

        let fora = raiz.path().join("wt").join("frente");
        std::fs::write(repo.join(".git").join("worktrees").join("frente").join("gitdir"), format!("{}\n", fora.join(".git").display())).unwrap();
        let conhecidas = [repo.to_string_lossy().into_owned()];
        assert_eq!(pasta_principal(&fora.to_string_lossy(), &conhecidas), Some(conhecidas[0].clone()));

        let claude = repo.join(".claude").join("worktrees").join("x");
        assert_eq!(pasta_principal(&claude.to_string_lossy(), &[]), Some(conhecidas[0].clone()));

        let codex = raiz.path().join("codex-wt").join("abc").join("app");
        assert_eq!(pasta_principal(&codex.to_string_lossy(), &conhecidas), Some(conhecidas[0].clone()));
        assert_eq!(pasta_principal(&repo.to_string_lossy(), &conhecidas), None, "pasta que existe não precisa de sugestão");
    }
}
