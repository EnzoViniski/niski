use super::*;
use std::fs;
use tempfile::TempDir;

fn repositorio() -> (TempDir, Repo) {
    let temp = tempfile::tempdir().unwrap();
    let pasta = temp.path().to_str().unwrap().to_string();
    run_git(&pasta, &["init", "-q"]).unwrap();
    let repo = Repo::abrir(pasta).unwrap();
    repo.git(&["config", "user.name", "Teste sintético"])
        .unwrap();
    repo.git(&["config", "user.email", "teste@example.invalid"])
        .unwrap();
    repo.git(&["config", "core.autocrlf", "false"]).unwrap();
    repo.git(&["config", "commit.gpgsign", "false"]).unwrap();
    repo.git(&["config", "core.hooksPath", &repo.raiz]).unwrap();
    (temp, repo)
}

fn salvar(repo: &Repo, caminho: &str, texto: &str) {
    fs::write(std::path::Path::new(&repo.raiz).join(caminho), texto).unwrap();
}

fn base(repo: &Repo, caminho: &str, texto: &str) {
    salvar(repo, caminho, texto);
    repo.git(&["add", "--", caminho]).unwrap();
    repo.git(&["commit", "-qm", "base sintética"]).unwrap();
}

fn aplicar(repo: &Repo, a: &Arquivo, acao: &str, trecho: Option<usize>) -> Result<(), String> {
    revisao_aplicar(
        repo.pasta.clone(),
        a.caminho.clone(),
        a.staged,
        a.patch.clone(),
        trecho,
        acao.into(),
    )
}

fn texto_base() -> String {
    (1..=24).map(|n| format!("linha {n}\n")).collect()
}

#[test]
fn parse_numeros_contexto_adicao_remocao_e_sem_newline() {
    let patch = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -2,2 +2,3 @@ função\n igual\n-antes\n+depois\n+extra\n\\ No newline at end of file\n";
    let t = parse_trechos(patch);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].patch, patch);
    assert_eq!(
        (t[0].linhas[0].antiga, t[0].linhas[0].nova),
        (Some(2), Some(2))
    );
    assert_eq!(
        (t[0].linhas[1].antiga, t[0].linhas[1].nova),
        (Some(3), None)
    );
    assert_eq!(
        (t[0].linhas[3].antiga, t[0].linhas[3].nova),
        (None, Some(4))
    );
    assert_eq!((t[0].linhas[4].antiga, t[0].linhas[4].nova), (None, None));
}

#[test]
fn patch_por_trecho_nao_inclui_o_outro() {
    let patch = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-a\n+b\n@@ -20,0 +21 @@\n+c\n";
    let t = parse_trechos(patch);
    assert_eq!(t.len(), 2);
    assert!(!t[0].patch.contains("+c\n"));
    assert!(t[1]
        .patch
        .starts_with("diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -20,0 +21 @@"));
    assert!(!t[1].patch.contains("+b\n"));
    assert!(parse_trechos("diff --git a/a b/a\nold mode 100644\nnew mode 100755\n").is_empty());
}

#[test]
fn stage_unstage_e_descartar_so_um_dos_dois_trechos() {
    let (_temp, repo) = repositorio();
    let original = texto_base();
    base(&repo, "a.txt", &original);
    let modificado = original
        .replace("linha 2\n", "nova linha 2\nextra\n")
        .replace("linha 22\n", "nova linha 22\n");
    salvar(&repo, "a.txt", &modificado);
    let a = repo.arquivo("a.txt".into(), false).unwrap();
    assert_eq!(a.trechos.len(), 2);
    aplicar(&repo, &a, "stage", Some(0)).unwrap();
    let indice = repo.git(&["show", ":a.txt"]).unwrap();
    assert!(indice.contains("nova linha 2\nextra\n"));
    assert!(!indice.contains("nova linha 22"));
    let staged = repo.arquivo("a.txt".into(), true).unwrap();
    aplicar(&repo, &staged, "unstage", Some(0)).unwrap();
    assert_eq!(repo.git(&["show", ":a.txt"]).unwrap(), original);
    let atual = repo.arquivo("a.txt".into(), false).unwrap();
    aplicar(&repo, &atual, "descartar", Some(1)).unwrap();
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&repo.raiz).join("a.txt")).unwrap(),
        original.replace("linha 2\n", "nova linha 2\nextra\n")
    );
}

#[test]
fn stage_e_unstage_do_segundo_trecho_com_deslocamento() {
    let (_temp, repo) = repositorio();
    let original = texto_base();
    base(&repo, "a.txt", &original);
    salvar(
        &repo,
        "a.txt",
        &original
            .replace("linha 2\n", "a\nb\nc\n")
            .replace("linha 22\n", "fim\n"),
    );
    aplicar(
        &repo,
        &repo.arquivo("a.txt".into(), false).unwrap(),
        "stage",
        Some(1),
    )
    .unwrap();
    assert_eq!(
        repo.git(&["show", ":a.txt"]).unwrap(),
        original.replace("linha 22\n", "fim\n")
    );
    aplicar(
        &repo,
        &repo.arquivo("a.txt".into(), true).unwrap(),
        "unstage",
        Some(0),
    )
    .unwrap();
    assert_eq!(repo.git(&["show", ":a.txt"]).unwrap(), original);
}

#[test]
fn arquivo_novo_em_repo_sem_commit_stage_unstage_e_descartar() {
    let (_temp, repo) = repositorio();
    salvar(&repo, "novo com espaço ç.txt", "conteúdo sem newline");
    let lista = revisao_listar(repo.pasta.clone(), false).unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].status, "Novo");
    aplicar(&repo, &lista[0], "stage", Some(0)).unwrap();
    assert_eq!(
        repo.git(&["show", ":novo com espaço ç.txt"]).unwrap(),
        "conteúdo sem newline"
    );
    let a = repo.arquivo("novo com espaço ç.txt".into(), true).unwrap();
    aplicar(&repo, &a, "unstage", None).unwrap();
    assert!(repo.git(&["ls-files"]).unwrap().is_empty());
    aplicar(
        &repo,
        &repo.arquivo("novo com espaço ç.txt".into(), false).unwrap(),
        "descartar",
        None,
    )
    .unwrap();
    assert!(!std::path::Path::new(&repo.raiz)
        .join("novo com espaço ç.txt")
        .exists());
}

#[test]
fn lista_as_duas_camadas_e_filtra_staged() {
    let (_temp, repo) = repositorio();
    base(&repo, "a.txt", "a\n");
    salvar(&repo, "a.txt", "b\n");
    repo.git(&["add", "a.txt"]).unwrap();
    salvar(&repo, "a.txt", "c\n");
    let lista = revisao_listar(repo.pasta.clone(), false).unwrap();
    assert_eq!(lista.len(), 2);
    assert_eq!((lista[0].staged, lista[1].staged), (false, true));
    assert_eq!((lista[0].adicionadas, lista[0].removidas), (1, 1));
    assert_eq!(revisao_listar(repo.pasta, true).unwrap().len(), 1);
}

#[test]
fn recusa_patch_obsoleto_acao_errada_e_indice_invalido() {
    let (_temp, repo) = repositorio();
    base(&repo, "a.txt", "a\n");
    salvar(&repo, "a.txt", "b\n");
    let a = repo.arquivo("a.txt".into(), false).unwrap();
    assert!(aplicar(&repo, &a, "unstage", None).is_err());
    assert!(aplicar(&repo, &a, "stage", Some(99)).is_err());
    salvar(&repo, "a.txt", "c\n");
    assert!(aplicar(&repo, &a, "descartar", None)
        .unwrap_err()
        .contains("mudou"));
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&repo.raiz).join("a.txt")).unwrap(),
        "c\n"
    );
}

#[test]
fn descarte_staged_remove_indice_e_arvore_e_preserva_sobreposicao() {
    let (_temp, repo) = repositorio();
    base(&repo, "a.txt", "a\n");
    salvar(&repo, "a.txt", "b\n");
    repo.git(&["add", "a.txt"]).unwrap();
    let a = repo.arquivo("a.txt".into(), true).unwrap();
    salvar(&repo, "a.txt", "c\n");
    assert!(aplicar(&repo, &a, "descartar", Some(0)).is_err());
    assert_eq!(repo.git(&["show", ":a.txt"]).unwrap(), "b\n");
    salvar(&repo, "a.txt", "b\n");
    aplicar(&repo, &a, "descartar", Some(0)).unwrap();
    assert_eq!(repo.git(&["show", ":a.txt"]).unwrap(), "a\n");
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&repo.raiz).join("a.txt")).unwrap(),
        "a\n"
    );
}

#[test]
fn exclusao_rename_e_binario_por_arquivo() {
    let (_temp, repo) = repositorio();
    base(&repo, "a.txt", "a\n");
    fs::rename(
        std::path::Path::new(&repo.raiz).join("a.txt"),
        std::path::Path::new(&repo.raiz).join("b.txt"),
    )
    .unwrap();
    assert_eq!(revisao_listar(repo.pasta.clone(), false).unwrap().len(), 2);
    let excluido = repo.arquivo("a.txt".into(), false).unwrap();
    assert_eq!(excluido.status, "Excluído");
    aplicar(&repo, &excluido, "descartar", None).unwrap();
    fs::write(
        std::path::Path::new(&repo.raiz).join("bin"),
        b"\x00\x01\x02",
    )
    .unwrap();
    let bin = repo.arquivo("bin".into(), false).unwrap();
    assert!(!bin.granular);
    aplicar(&repo, &bin, "stage", None).unwrap();
    aplicar(
        &repo,
        &repo.arquivo("bin".into(), true).unwrap(),
        "unstage",
        None,
    )
    .unwrap();
    assert!(repo.novo("bin").unwrap());
}

#[test]
fn arquivo_vazio_e_pathspec_literal_nao_atingem_outros_arquivos() {
    let (_temp, repo) = repositorio();
    salvar(&repo, "[a].txt", "");
    salvar(&repo, "a.txt", "preservar\n");
    let a = repo.arquivo("[a].txt".into(), false).unwrap();
    assert!(a.trechos.is_empty());
    aplicar(&repo, &a, "stage", None).unwrap();
    assert_eq!(repo.git(&["ls-files"]).unwrap(), "[a].txt\n");
    aplicar(
        &repo,
        &repo.arquivo("[a].txt".into(), true).unwrap(),
        "unstage",
        None,
    )
    .unwrap();
    aplicar(&repo, &a, "descartar", None).unwrap();
    assert!(std::path::Path::new(&repo.raiz).join("a.txt").exists());
}

#[test]
fn pasta_interna_resolve_raiz_e_preserva_crlf() {
    let (_temp, repo) = repositorio();
    fs::create_dir(std::path::Path::new(&repo.raiz).join("sub")).unwrap();
    base(&repo, "sub/a.txt", "a\r\nb\r\n");
    salvar(&repo, "sub/a.txt", "a\r\nc\r\n");
    let pasta = std::path::Path::new(&repo.raiz)
        .join("sub")
        .to_str()
        .unwrap()
        .to_string();
    repo.git(&["config", "diff.relative", "true"]).unwrap();
    let lista = revisao_listar(pasta.clone(), false).unwrap();
    assert_eq!(lista[0].caminho, "sub/a.txt");
    revisao_aplicar(
        pasta,
        lista[0].caminho.clone(),
        false,
        lista[0].patch.clone(),
        Some(0),
        "stage".into(),
    )
    .unwrap();
    assert_eq!(repo.git(&["show", ":sub/a.txt"]).unwrap(), "a\r\nc\r\n");
}

#[test]
fn caminho_fora_do_repo_e_trecho_de_modo_sao_recusados() {
    let (_temp, repo) = repositorio();
    assert!(repo.patch("../fora", false).is_err());
    assert!(repo.patch("/tmp/fora", false).is_err());
    base(&repo, "a.txt", "a\n");
    repo.git(&["update-index", "--chmod=+x", "a.txt"]).unwrap();
    let a = repo.arquivo("a.txt".into(), true).unwrap();
    assert!(!a.granular);
    aplicar(&repo, &a, "unstage", None).unwrap();
}

#[test]
fn worktree_tem_indice_e_alteracoes_proprios() {
    let (_temp, repo) = repositorio();
    base(&repo, "a.txt", "principal\n");
    let local = tempfile::tempdir().unwrap();
    let pasta = local.path().join("isolada").to_str().unwrap().to_string();
    repo.git(&["worktree", "add", "-qb", "revisao-sintetica", &pasta])
        .unwrap();
    let isolada = Repo::abrir(pasta).unwrap();
    salvar(&isolada, "a.txt", "alterado na worktree\n");
    let a = revisao_listar(isolada.pasta.clone(), false)
        .unwrap()
        .remove(0);
    aplicar(&isolada, &a, "stage", Some(0)).unwrap();
    assert_eq!(
        isolada.git(&["show", ":a.txt"]).unwrap(),
        "alterado na worktree\n"
    );
    assert_eq!(repo.git(&["show", ":a.txt"]).unwrap(), "principal\n");
    assert!(revisao_listar(repo.pasta, true).unwrap().is_empty());
}
