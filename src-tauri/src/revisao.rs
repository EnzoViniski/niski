//! Fork: revisão por camada (índice/árvore), com patches sempre reconstruídos pelo Git.
use crate::git_client::{git_invocation, hidden, run_git};
use serde::Serialize;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Serialize)]
pub struct Linha {
    texto: String,
    tipo: char,
    antiga: Option<u32>,
    nova: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct Trecho {
    cabecalho: String,
    linhas: Vec<Linha>,
    patch: String,
}

#[derive(Debug, Serialize)]
pub struct Arquivo {
    caminho: String,
    staged: bool,
    status: String,
    adicionadas: usize,
    removidas: usize,
    patch: String,
    trechos: Vec<Trecho>,
    granular: bool,
}

/// Mantém cabeçalhos, CRLF e marcadores de ausência de newline exatamente como o Git gerou.
fn parse_trechos(patch: &str) -> Vec<Trecho> {
    let mut cabecalho = String::new();
    let mut trechos: Vec<Trecho> = Vec::new();
    let (mut antiga, mut nova) = (0, 0);
    for linha in patch.split_inclusive('\n') {
        if linha.starts_with("@@ ") {
            let partes: Vec<_> = linha.split_whitespace().collect();
            let numero = |i: usize| {
                partes
                    .get(i)
                    .and_then(|p| p.get(1..)?.split(',').next()?.parse::<u32>().ok())
            };
            let (Some(a), Some(n)) = (numero(1), numero(2)) else {
                continue;
            };
            antiga = a;
            nova = n;
            trechos.push(Trecho {
                cabecalho: linha.trim_end().into(),
                linhas: Vec::new(),
                patch: format!("{cabecalho}{linha}"),
            });
        } else if let Some(trecho) = trechos.last_mut() {
            trecho.patch.push_str(linha);
            let tipo = linha.chars().next().unwrap_or(' ');
            let a = matches!(tipo, ' ' | '-').then_some(antiga);
            let n = matches!(tipo, ' ' | '+').then_some(nova);
            trecho.linhas.push(Linha {
                texto: linha.trim_end_matches('\n').get(1..).unwrap_or("").into(),
                tipo,
                antiga: a,
                nova: n,
            });
            if a.is_some() {
                antiga += 1;
            }
            if n.is_some() {
                nova += 1;
            }
        } else {
            cabecalho.push_str(linha);
        }
    }
    trechos
}

struct Repo {
    pasta: String,
    raiz: String,
}

impl Repo {
    fn abrir(pasta: String) -> Result<Self, String> {
        let raiz = run_git(&pasta, &["rev-parse", "--show-toplevel"])?;
        Ok(Self {
            pasta,
            raiz: raiz.trim_end_matches(['\r', '\n']).into(),
        })
    }

    fn args<'a>(&'a self, args: &'a [&'a str]) -> Vec<&'a str> {
        let mut todos = vec!["--literal-pathspecs", "-C", &self.raiz];
        todos.extend_from_slice(args);
        todos
    }

    fn git(&self, args: &[&str]) -> Result<String, String> {
        run_git(&self.pasta, &self.args(args))
    }

    /// Só `diff --no-index` aceita código 1 (há diferenças). Nenhum comando mutante o aceita.
    fn entrada(&self, args: &[&str], patch: Option<&str>, diff: bool) -> Result<String, String> {
        let (programa, argumentos) = git_invocation(&self.pasta, &self.args(args));
        let diretorio = if programa == "git" {
            self.pasta.clone()
        } else {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_else(|_| ".".into())
        };
        let mut filho = hidden(&mut Command::new(programa))
            .args(argumentos)
            .current_dir(diretorio)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(if patch.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        // Drena stdout/stderr enquanto escreve: patches grandes não podem travar o pipe.
        let escrita = patch.map(|texto| {
            let mut stdin = filho.stdin.take().expect("stdin do patch");
            let texto = texto.as_bytes().to_vec();
            std::thread::spawn(move || stdin.write_all(&texto))
        });
        let saida = filho.wait_with_output().map_err(|e| e.to_string())?;
        if let Some(escrita) = escrita {
            let _ = escrita.join();
        }
        if !saida.status.success() && !(diff && saida.status.code() == Some(1)) {
            return Err(String::from_utf8_lossy(&saida.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&saida.stdout).into_owned())
    }

    fn novo(&self, caminho: &str) -> Result<bool, String> {
        Ok(!self
            .git(&[
                "ls-files",
                "--others",
                "--exclude-standard",
                "-z",
                "--",
                caminho,
            ])?
            .is_empty())
    }

    fn patch(&self, caminho: &str, staged: bool) -> Result<String, String> {
        if caminho.is_empty() || caminho.starts_with('/') || caminho.split('/').any(|p| p == "..") {
            return Err("Caminho inválido para revisão.".into());
        }
        if !self
            .git(&["ls-files", "--unmerged", "-z", "--", caminho])?
            .is_empty()
        {
            return Err("Resolva o conflito neste arquivo antes de revisá-lo.".into());
        }
        let mut args = vec![
            "diff",
            "--no-relative",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--binary",
            "--no-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ];
        if staged {
            args.push("--cached");
        }
        if !staged && self.novo(caminho)? {
            args.extend(["--no-index", "--", "/dev/null", caminho]);
            self.entrada(&args, None, true)
        } else {
            args.extend(["--", caminho]);
            self.git(&args)
        }
    }

    fn arquivo(&self, caminho: String, staged: bool) -> Result<Arquivo, String> {
        let patch = self.patch(&caminho, staged)?;
        let trechos = parse_trechos(&patch);
        let adicionadas = trechos
            .iter()
            .flat_map(|t| &t.linhas)
            .filter(|l| l.tipo == '+')
            .count();
        let removidas = trechos
            .iter()
            .flat_map(|t| &t.linhas)
            .filter(|l| l.tipo == '-')
            .count();
        let status = if patch.contains("\nnew file mode ") || (!staged && self.novo(&caminho)?) {
            "Novo"
        } else if patch.contains("\ndeleted file mode ") {
            "Excluído"
        } else {
            "Modificado"
        }
        .into();
        // Mudança de modo junto do conteúdo é atômica para não preparar metadados por acidente.
        let granular = !trechos.is_empty()
            && !patch.contains("\nold mode ")
            && !patch.contains("GIT binary patch");
        Ok(Arquivo {
            caminho,
            staged,
            status,
            adicionadas,
            removidas,
            patch,
            trechos,
            granular,
        })
    }
}

#[tauri::command(async)]
pub fn revisao_listar(project_path: String, staged: bool) -> Result<Vec<Arquivo>, String> {
    let repo = Repo::abrir(project_path)?;
    let mut arquivos = Vec::new();
    for camada in [false, true] {
        if staged && !camada {
            continue;
        }
        let mut args = vec![
            "diff",
            "--no-relative",
            "--no-ext-diff",
            "--no-textconv",
            "--name-only",
            "--no-renames",
            "-z",
        ];
        if camada {
            args.push("--cached");
        }
        let mut nomes = repo.git(&args)?;
        if !camada {
            nomes.push_str(&repo.git(&["ls-files", "--others", "--exclude-standard", "-z"])?);
        }
        for nome in nomes.split('\0').filter(|n| !n.is_empty()) {
            arquivos.push(repo.arquivo(nome.into(), camada)?);
        }
    }
    Ok(arquivos)
}

#[tauri::command(async)]
pub fn revisao_aplicar(
    project_path: String,
    caminho: String,
    staged: bool,
    esperado: String,
    trecho: Option<usize>,
    acao: String,
) -> Result<(), String> {
    let repo = Repo::abrir(project_path)?;
    let arquivo = repo.arquivo(caminho.clone(), staged)?;
    if arquivo.patch != esperado {
        return Err("O arquivo mudou. Atualize a revisão antes de aplicar.".into());
    }
    let patch = if let Some(i) = trecho {
        if !arquivo.granular {
            return Err("Use a ação do arquivo inteiro para esta alteração.".into());
        }
        arquivo
            .trechos
            .get(i)
            .ok_or("Trecho não encontrado; atualize a revisão.")?
            .patch
            .as_str()
    } else {
        &arquivo.patch
    };
    let args: Vec<&str> = match (acao.as_str(), staged) {
        // Até arquivo inteiro usa o snapshot: git add poderia incluir uma edição concorrente.
        ("stage", false) => vec!["apply", "--cached", "--whitespace=nowarn", "-"],
        ("unstage", true) => vec!["apply", "--cached", "-R", "--whitespace=nowarn", "-"],
        ("descartar", false) => vec!["apply", "-R", "--whitespace=nowarn", "-"],
        // --index valida índice E árvore antes de descartar staged, preservando edições sobrepostas.
        ("descartar", true) => vec!["apply", "-R", "--index", "--whitespace=nowarn", "-"],
        _ => return Err("Ação incompatível com o escopo.".into()),
    };
    repo.entrada(&args, Some(patch), false).map(|_| ())
}

#[cfg(test)]
#[path = "revisao_testes.rs"]
mod testes;
