//! Fork: agente numa **worktree isolada** do git — uma cópia de trabalho só dele, numa branch só dele,
//! para agentes que editam código em paralelo não pisarem um no outro.
//!
//! Onde fica: **fora** do repositório, ao lado dele:
//! `<pai do repo>/.medza-worktrees/<nome do repo>/<nome>`, na branch `medza/<nome>`, criada a partir
//! do `HEAD` da pasta de onde o agente sai (mudanças não commitadas ficam na pasta original). Fora do
//! repo para não aparecer no `git status`, não entrar em busca/build/watch do projeto e não exigir
//! mexer no `.gitignore`. Um repo em `~/code/app` ganha `~/code/.medza-worktrees/app/<nome>`.
//!
//! `.worktreeinclude` (na raiz da pasta de origem), mesma semântica do Claude Code e do Codex: padrões
//! no formato do `.gitignore`; um arquivo é copiado para a worktree quando casa com um padrão **e** é
//! ignorado pelo git (ex.: `.env`, `.env.local`, `config/secrets.json`). Arquivo versionado já vem
//! pelo checkout e não é copiado.
//!
//! Remover apaga só a pasta da worktree (`git worktree remove`); a branch `medza/<nome>` fica no
//! repositório com os commits do agente. Com mudanças não commitadas, só remove com `forcar`.
//! Usado pela casca (Novo agente → "Isolar em worktree") e pelo engine (`criar_agente` do orquestrador).

use serde::Serialize;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

/// Pasta irmã do repositório que guarda as worktrees isoladas.
pub const PASTA: &str = ".medza-worktrees";
pub const PREFIXO_DA_BRANCH: &str = "medza/";
pub const ARQUIVO_DE_INCLUSAO: &str = ".worktreeinclude";
const NOME_MAXIMO: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorktreeIsolada {
    /// Raiz da worktree.
    pub caminho: String,
    /// Onde o agente roda: a raiz, ou a mesma subpasta de onde ele saiu (projeto aberto num subdiretório).
    pub pasta_de_trabalho: String,
    pub branch: String,
    pub nome: String,
    /// Quantos arquivos do `.worktreeinclude` foram copiados.
    pub copiados: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Estado {
    pub caminho: String,
    pub branch: String,
    /// Linhas do `git status --porcelain` (modificados, novos, apagados). Ignorados não contam.
    pub mudancas: usize,
    /// Até 10 caminhos com mudança, para o aviso antes de apagar.
    pub exemplos: Vec<String>,
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    git_com_entrada(cwd, args, None).map(|saida| saida.trim_end_matches(['\n', '\r']).to_owned())
}

/// `git` com o que for dado na entrada padrão. Saída crua (para `-z`).
fn git_com_entrada(cwd: &Path, args: &[&str], entrada: Option<&[u8]>) -> Result<String, String> {
    let mut filho = Command::new("git").args(args).current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0").env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(if entrada.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("git não encontrado no PATH: {e}"))?;
    // Fork: drena stdout/stderr enquanto escreve stdin; check-ignore pode encher ambos os pipes.
    let saida = std::thread::scope(|escopo| {
        let escrita = entrada.zip(filho.stdin.take()).map(|(bytes, mut stdin)| {
            escopo.spawn(move || stdin.write_all(bytes))
        });
        let saida = filho.wait_with_output().map_err(|e| e.to_string());
        if let Some(escrita) = escrita {
            escrita.join().map_err(|_| "Falha na thread de entrada do git".to_owned())?
                .map_err(|e| e.to_string())?;
        }
        saida
    })?;
    if saida.status.success() { return Ok(String::from_utf8_lossy(&saida.stdout).into_owned()); }
    // `check-ignore` sai com 1 quando nada é ignorado: não é falha.
    if args.first() == Some(&"check-ignore") && saida.status.code() == Some(1) { return Ok(String::new()); }
    let erro = String::from_utf8_lossy(&saida.stderr).trim().to_owned();
    Err(if erro.is_empty() { format!("`git {}` falhou", args.join(" ")) } else { erro })
}

fn absoluto(base: &Path, caminho: &str) -> PathBuf {
    let caminho = PathBuf::from(caminho);
    if caminho.is_absolute() { caminho } else { base.join(caminho) }
}

/// Nome seguro para pasta e branch: minúsculas, dígitos, `-` e `_`; o resto vira `-`.
/// `"Corrigir Login!"` → `corrigir-login`. Vazio → erro.
pub fn nome_valido(nome: &str) -> Result<String, String> {
    let mut saida = String::new();
    for c in nome.trim().chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a', 'é' | 'ê' | 'è' | 'ë' => 'e', 'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o', 'ú' | 'ù' | 'û' | 'ü' => 'u', 'ç' => 'c', c => c,
        };
        if c.is_ascii_alphanumeric() || c == '_' { saida.push(c) }
        else if !saida.ends_with('-') { saida.push('-') }
    }
    let saida: String = saida.trim_matches('-').chars().take(NOME_MAXIMO).collect();
    let saida = saida.trim_end_matches('-').to_owned();
    if saida.is_empty() { return Err("Nome da worktree vazio ou inválido (use letras, números e -)".into()); }
    Ok(saida)
}

/// Nome automático a partir de um texto (tarefa, título): as primeiras palavras. Sem nada útil, "agente".
pub fn nome_a_partir_de(texto: &str) -> String {
    let palavras: Vec<&str> = texto.split_whitespace().take(5).collect();
    nome_valido(&palavras.join(" ")).map(|n| n.chars().take(32).collect::<String>().trim_end_matches('-').to_owned())
        .ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "agente".into())
}

/// Raiz do checkout principal (não a de uma worktree), a partir de qualquer pasta do repositório.
fn raiz_principal(pasta: &Path) -> Result<PathBuf, String> {
    let comum = git(pasta, &["rev-parse", "--git-common-dir"])?;
    let comum = absoluto(pasta, &comum);
    let comum = comum.canonicalize().unwrap_or(comum);
    comum.parent().map(Path::to_path_buf).ok_or_else(|| "Repositório sem pasta de trabalho".to_owned())
}

/// `<pai>/.medza-worktrees/<repo>`: onde ficam as worktrees isoladas deste repositório.
pub fn pasta_base(raiz_principal: &Path) -> PathBuf {
    let nome = raiz_principal.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "projeto".into());
    raiz_principal.parent().unwrap_or(raiz_principal).join(PASTA).join(nome)
}

/// `(repo, nome)` se `caminho` está dentro de uma worktree isolada (`…/.medza-worktrees/<repo>/<nome>/…`).
pub fn da_pasta(caminho: &Path) -> Option<(String, String)> {
    let partes: Vec<String> = caminho.components().filter_map(|c| match c {
        Component::Normal(parte) => Some(parte.to_string_lossy().into_owned()),
        _ => None,
    }).collect();
    let i = partes.iter().rposition(|p| p == PASTA)?;
    Some((partes.get(i + 1)?.clone(), partes.get(i + 2)?.clone()))
}

/// Cria a worktree isolada a partir da pasta `pasta` (raiz ou subpasta de um repositório). Se o nome já
/// estiver em uso (pasta ou branch), usa `<nome>-2`, `<nome>-3`…
pub fn criar(pasta: &Path, nome: &str) -> Result<WorktreeIsolada, String> {
    let nome = nome_valido(nome)?;
    let raiz = PathBuf::from(git(pasta, &["rev-parse", "--show-toplevel"]).map_err(|_| "Esta pasta não é um repositório git".to_owned())?);
    let subpasta = git(pasta, &["rev-parse", "--show-prefix"]).unwrap_or_default();
    git(&raiz, &["rev-parse", "--verify", "--quiet", "HEAD"]).map_err(|_| "O repositório ainda não tem nenhum commit: faça o primeiro antes de isolar um agente".to_owned())?;
    let base = pasta_base(&raiz_principal(&raiz)?);

    let (destino, nome_final, branch) = (1..=50).find_map(|n| {
        let candidato = if n == 1 { nome.clone() } else { format!("{nome}-{n}") };
        let destino = base.join(&candidato);
        let branch = format!("{PREFIXO_DA_BRANCH}{candidato}");
        let branch_existe = git(&raiz, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok();
        (!destino.exists() && !branch_existe).then_some((destino, candidato, branch))
    }).ok_or_else(|| format!("Já existem worktrees demais chamadas {nome}"))?;

    std::fs::create_dir_all(&base).map_err(|e| format!("Não consegui criar {}: {e}", base.display()))?;
    let destino_txt = destino.to_string_lossy().into_owned();
    git(&raiz, &["worktree", "add", "-b", &branch, &destino_txt, "HEAD"])?;
    let copiados = copiar_incluidos(&raiz, &destino).unwrap_or_else(|erro| {
        crate::registro::aviso("worktree_isolada_inclusao_falhou", &[("erro", &erro)]);
        0
    });
    let pasta_de_trabalho = destino.join(subpasta.trim_end_matches('/'));
    let pasta_de_trabalho = if pasta_de_trabalho.is_dir() { pasta_de_trabalho } else { destino.clone() };
    crate::registro::info("worktree_isolada_criada", &[("copiados", &copiados.to_string())]);
    Ok(WorktreeIsolada { caminho: destino_txt, pasta_de_trabalho: pasta_de_trabalho.to_string_lossy().into_owned(),
        branch, nome: nome_final, copiados })
}

/// Arquivos (relativos à raiz) que o `.worktreeinclude` manda copiar: casam com um padrão dele **e** são
/// ignorados pelo git. Quem decide os dois é o próprio git, com a semântica exata do `.gitignore`.
pub fn incluidos(raiz: &Path) -> Result<Vec<String>, String> {
    let arquivo = raiz.join(ARQUIVO_DE_INCLUSAO);
    if !arquivo.is_file() { return Ok(Vec::new()); }
    let filtro = format!("--exclude-from={}", arquivo.to_string_lossy());
    // Não versionados que casam com o `.worktreeinclude` (lido como lista de "ignorar")…
    let candidatos = git_com_entrada(raiz, &["ls-files", "-z", "--others", "--ignored", &filtro], None)?;
    if candidatos.is_empty() { return Ok(Vec::new()); }
    // …e, deles, os que o `.gitignore` de verdade ignora.
    let ignorados = git_com_entrada(raiz, &["check-ignore", "-z", "--stdin"], Some(candidatos.as_bytes()))?;
    let mut lista: Vec<String> = ignorados.split('\0').filter(|c| !c.is_empty()).map(str::to_owned).collect();
    lista.sort();
    lista.dedup();
    Ok(lista)
}

fn copiar_incluidos(origem: &Path, destino: &Path) -> Result<usize, String> {
    let mut copiados = 0;
    for relativo in incluidos(origem)? {
        let rel = Path::new(&relativo);
        if rel.components().any(|c| !matches!(c, Component::Normal(_))) { continue; }
        let (de, para) = (origem.join(rel), destino.join(rel));
        if para.exists() { continue; }
        if let Some(pai) = para.parent() { std::fs::create_dir_all(pai).map_err(|e| e.to_string())?; }
        let meta = std::fs::symlink_metadata(&de).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            #[cfg(unix)]
            { std::os::unix::fs::symlink(std::fs::read_link(&de).map_err(|e| e.to_string())?, &para).map_err(|e| e.to_string())?; }
            #[cfg(not(unix))]
            { std::fs::copy(&de, &para).map_err(|e| e.to_string())?; }
        } else if meta.is_file() {
            std::fs::copy(&de, &para).map_err(|e| e.to_string())?;
        } else { continue; }
        copiados += 1;
    }
    Ok(copiados)
}

/// Raiz da worktree isolada que contém `caminho`, conferindo que é mesmo uma (`…/.medza-worktrees/<repo>/<nome>`)
/// e não o repositório principal.
fn raiz_isolada(caminho: &Path) -> Result<PathBuf, String> {
    let nao_e = || "Esta pasta não é uma worktree isolada do Medza".to_owned();
    if da_pasta(caminho).is_none() { return Err(nao_e()); }
    let raiz = PathBuf::from(git(caminho, &["rev-parse", "--show-toplevel"]).map_err(|_| nao_e())?);
    let (_, nome) = da_pasta(&raiz).ok_or_else(nao_e)?;
    let principal = raiz_principal(&raiz)?;
    // Fork: o sufixo pode ser imitado em qualquer pasta; exige a base deste repositório.
    let raiz = raiz.canonicalize().map_err(|_| nao_e())?;
    let esperado = pasta_base(&principal).join(&nome).canonicalize().map_err(|_| nao_e())?;
    if raiz == principal || raiz != esperado { return Err(nao_e()); }
    Ok(raiz)
}

/// Branch e mudanças não commitadas da worktree isolada que contém `caminho`.
pub fn estado(caminho: &Path) -> Result<Estado, String> {
    let raiz = raiz_isolada(caminho)?;
    let branch = git(&raiz, &["branch", "--show-current"]).unwrap_or_default();
    let status = git(&raiz, &["status", "--porcelain", "--untracked-files=all"])?;
    let linhas: Vec<&str> = status.lines().filter(|l| !l.trim().is_empty()).collect();
    Ok(Estado {
        caminho: raiz.to_string_lossy().into_owned(), branch, mudancas: linhas.len(),
        exemplos: linhas.iter().take(10).map(|l| l.get(3..).unwrap_or(l).to_owned()).collect(),
    })
}

/// Apaga a pasta da worktree isolada (a branch fica). Com mudanças não commitadas, só com `forcar`.
pub fn remover(caminho: &Path, forcar: bool) -> Result<(), String> {
    let raiz = raiz_isolada(caminho)?;
    let atual = estado(&raiz)?;
    if atual.mudancas > 0 && !forcar {
        return Err(format!("A worktree tem {} mudança(s) não commitada(s) na branch {}. Commite antes ou confirme para apagar mesmo assim.",
            atual.mudancas, atual.branch));
    }
    let principal = raiz_principal(&raiz)?;
    let raiz_txt = raiz.to_string_lossy().into_owned();
    let mut args = vec!["worktree", "remove"];
    if forcar { args.push("--force"); }
    args.push(&raiz_txt);
    git(&principal, &args)?;
    // Pastas de cima vazias saem também (só se vazias: `remove_dir` não apaga conteúdo).
    if let Some(repo) = raiz.parent() {
        let _ = std::fs::remove_dir(repo);
        if let Some(base) = repo.parent() { let _ = std::fs::remove_dir(base); }
    }
    crate::registro::info("worktree_isolada_removida", &[("forcada", if forcar { "sim" } else { "nao" })]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Repositório de verdade numa pasta temporária: `<tmp>/app` com um commit, `.gitignore` e `.env`.
    fn repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().canonicalize().unwrap().join("app");
        std::fs::create_dir_all(raiz.join("src")).unwrap();
        for args in [vec!["init", "-q", "-b", "main"], vec!["config", "user.email", "t@t"], vec!["config", "user.name", "t"]] {
            git(&raiz, &args).unwrap();
        }
        std::fs::write(raiz.join(".gitignore"), ".env*\nnode_modules/\nsegredos/\n*.log\n").unwrap();
        std::fs::write(raiz.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(raiz.join(".worktreeinclude"), ".env\n.env.local\nsegredos/\nsrc/main.rs\nnovo.txt\n").unwrap();
        git(&raiz, &["add", "."]).unwrap();
        git(&raiz, &["commit", "-q", "-m", "inicio"]).unwrap();
        std::fs::write(raiz.join(".env"), "A=1\n").unwrap();
        std::fs::write(raiz.join(".env.local"), "B=2\n").unwrap();
        std::fs::write(raiz.join(".env.producao"), "C=3\n").unwrap(); // ignorado, mas fora do .worktreeinclude
        std::fs::create_dir_all(raiz.join("segredos")).unwrap();
        std::fs::write(raiz.join("segredos/chave.json"), "{}").unwrap();
        std::fs::create_dir_all(raiz.join("node_modules/x")).unwrap();
        std::fs::write(raiz.join("node_modules/x/i.js"), "").unwrap();
        std::fs::write(raiz.join("novo.txt"), "não ignorado").unwrap(); // casa, mas não é ignorado
        std::fs::write(raiz.join("debug.log"), "").unwrap();
        (dir, raiz)
    }

    #[test]
    fn nomes() {
        assert_eq!(nome_valido(" Corrigir Login! ").unwrap(), "corrigir-login");
        assert_eq!(nome_valido("ação/feat..x").unwrap(), "acao-feat-x");
        assert!(nome_valido("///").is_err());
        assert_eq!(nome_a_partir_de("Implementar a tela de login com OAuth e testes"), "implementar-a-tela-de-login");
        assert_eq!(nome_a_partir_de("!!!"), "agente");
        assert_eq!(da_pasta(Path::new("/c/.medza-worktrees/app/login/src")), Some(("app".into(), "login".into())));
        assert_eq!(da_pasta(Path::new("/c/app")), None);
        assert_eq!(da_pasta(Path::new("/c/.medza-worktrees/app")), None);
    }

    #[test]
    fn worktreeinclude_copia_so_o_que_casa_e_e_ignorado() {
        let (_dir, raiz) = repo();
        assert_eq!(incluidos(&raiz).unwrap(), vec![".env", ".env.local", "segredos/chave.json"]);
        // Sem o arquivo: nada.
        std::fs::remove_file(raiz.join(".worktreeinclude")).unwrap();
        assert!(incluidos(&raiz).unwrap().is_empty());
    }

    #[test]
    fn cria_fora_do_repo_com_branch_propria_e_remove_com_aviso() {
        let (dir, raiz) = repo();
        let wt = criar(&raiz, "Tela de Login").unwrap();
        let esperado = dir.path().canonicalize().unwrap().join(".medza-worktrees/app/tela-de-login");
        assert_eq!(PathBuf::from(&wt.caminho), esperado);
        assert_eq!(wt.branch, "medza/tela-de-login");
        assert_eq!(wt.copiados, 3);
        assert_eq!(std::fs::read_to_string(esperado.join(".env")).unwrap(), "A=1\n");
        assert!(esperado.join("segredos/chave.json").is_file());
        assert!(!esperado.join(".env.producao").exists());
        assert!(!esperado.join("node_modules").exists());
        assert_eq!(git(&esperado, &["branch", "--show-current"]).unwrap(), "medza/tela-de-login");
        // A pasta principal continua em main e sem nada novo no status.
        assert_eq!(git(&raiz, &["branch", "--show-current"]).unwrap(), "main");
        assert_eq!(git(&raiz, &["status", "--porcelain"]).unwrap(), "?? novo.txt");

        // Mesmo nome de novo: sufixo.
        let segunda = criar(&raiz, "tela-de-login").unwrap();
        assert_eq!(segunda.nome, "tela-de-login-2");
        assert_eq!(segunda.branch, "medza/tela-de-login-2");

        // Limpa: estado 0 e remove sem forçar; a branch fica.
        let e = estado(&esperado.join("src")).unwrap();
        assert_eq!((e.branch.as_str(), e.mudancas), ("medza/tela-de-login", 0));
        // Com mudança não commitada: recusa sem forçar e diz quantas.
        std::fs::write(esperado.join("src/main.rs"), "fn main() { mudou() }\n").unwrap();
        std::fs::write(esperado.join("rascunho.txt"), "x").unwrap();
        let e = estado(&esperado).unwrap();
        assert_eq!(e.mudancas, 2);
        assert!(e.exemplos.contains(&"src/main.rs".to_owned()));
        let erro = remover(&esperado, false).unwrap_err();
        assert!(erro.contains("2 mudança"), "{erro}");
        assert!(esperado.exists());
        remover(&esperado, true).unwrap();
        assert!(!esperado.exists());
        assert!(git(&raiz, &["rev-parse", "--verify", "refs/heads/medza/tela-de-login"]).is_ok(), "a branch fica");

        remover(&PathBuf::from(&segunda.caminho), false).unwrap();
        assert!(!dir.path().join(".medza-worktrees").exists(), "pastas vazias saem");
        // Nunca apaga o repositório principal nem pasta qualquer.
        assert!(remover(&raiz, true).is_err());
        assert!(estado(&raiz).is_err());
    }

    #[test]
    fn recusa_remover_worktree_fora_da_base_do_repositorio() {
        let (dir, raiz) = repo();
        for relativo in ["fora/.medza-worktrees/app/tarefa", ".medza-worktrees/outro/tarefa"] {
            let externa = dir.path().canonicalize().unwrap().join(relativo);
            std::fs::create_dir_all(externa.parent().unwrap()).unwrap();
            git(&raiz, &["worktree", "add", "--detach", externa.to_str().unwrap(), "HEAD"]).unwrap();
            assert!(estado(&externa).is_err(), "aceitou estado de {}", externa.display());
            assert!(remover(&externa, true).is_err(), "aceitou remover {}", externa.display());
            assert!(externa.join("src/main.rs").is_file(), "worktree externa foi apagada");
        }
    }

    #[test]
    fn check_ignore_drena_saida_enquanto_escreve_entrada_grande() {
        const VAR: &str = "MEDZA_TESTE_CHECK_IGNORE_RAIZ";
        if let Some(raiz) = std::env::var_os(VAR) {
            // Mais que a capacidade dos dois pipes, sem criar milhares de arquivos.
            let entrada = ".env.sintetico\0".repeat(100_000);
            let saida = git_com_entrada(Path::new(&raiz), &["check-ignore", "-z", "--stdin"], Some(entrada.as_bytes())).unwrap();
            assert_eq!(saida, entrada);
            return;
        }
        let (_dir, raiz) = repo();
        // Processo separado: um deadlock vira falha com prazo, sem deixar o runner preso.
        let mut filho = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "worktree_isolada::tests::check_ignore_drena_saida_enquanto_escreve_entrada_grande", "--nocapture"])
            .env(VAR, &raiz).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(status) = filho.try_wait().unwrap() {
                let saida = filho.wait_with_output().unwrap();
                assert!(status.success(), "{}", String::from_utf8_lossy(&saida.stderr));
                break;
            }
            if std::time::Instant::now() >= limite {
                filho.kill().unwrap();
                filho.wait().unwrap();
                panic!("check-ignore travou com stdin/stdout maiores que os pipes");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn subpasta_e_worktree_a_partir_de_outra_worktree() {
        let (dir, raiz) = repo();
        let wt = criar(&raiz.join("src"), "a").unwrap();
        assert!(wt.pasta_de_trabalho.ends_with(".medza-worktrees/app/a/src"), "{}", wt.pasta_de_trabalho);
        // Orquestrador numa worktree isolada abre subagente isolado: mesma base, não aninhada.
        let filha = criar(Path::new(&wt.caminho), "b").unwrap();
        assert_eq!(PathBuf::from(&filha.caminho), dir.path().canonicalize().unwrap().join(".medza-worktrees/app/b"));
    }

    #[test]
    fn repo_sem_commit_ou_sem_git() {
        let dir = tempfile::tempdir().unwrap();
        assert!(criar(dir.path(), "x").unwrap_err().contains("não é um repositório"));
        git(dir.path(), &["init", "-q"]).unwrap();
        assert!(criar(dir.path(), "x").unwrap_err().contains("nenhum commit"));
    }
}
