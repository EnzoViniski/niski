//! Pastas sintéticas em tempdir; nenhuma conta ou CLI real.
use super::*;
use std::fs;

fn skill(raiz: &Path, nome: &str, conteudo: &str) {
    fs::create_dir_all(raiz.join(nome)).unwrap();
    fs::write(raiz.join(nome).join("SKILL.md"), conteudo).unwrap();
}

#[test]
fn frontmatter_linha_e_bloco() {
    assert_eq!(frontmatter("---\nname: a\ndescription: \"Faz X\"\n---\ncorpo"), (Some("a".into()), Some("Faz X".into())));
    assert_eq!(frontmatter("---\nname: b\ndescription: >\n  Primeira\n  segunda\n---\n").1.as_deref(), Some("Primeira segunda"));
    assert_eq!(frontmatter("sem frontmatter"), (None, None));
}

#[test]
fn claude_lista_cli_skills_e_comandos() {
    let dir = tempfile::tempdir().unwrap();
    let (conta, projeto) = (dir.path().join("conta"), dir.path().join("proj"));
    skill(&conta.join("skills"), "revisar", "---\nname: Revisar\ndescription: Revisa código\n---\n");
    skill(&projeto.join(".claude/skills"), "revisar", "---\ndescription: Versão do projeto\n---\n");
    skill(&conta.join("skills"), "nome com espaco", "---\ndescription: x\n---\n");
    fs::create_dir_all(conta.join("commands/front")).unwrap();
    fs::write(conta.join("commands/front/teste.md"), "# Roda os testes do front\n").unwrap();
    let itens = listar("claude", Some(&conta), Some(&projeto), None);
    let nomes: Vec<_> = itens.iter().map(|i| i.nome.as_str()).collect();
    assert!(nomes.contains(&"compact") && nomes.contains(&"front:teste"));
    let revisar: Vec<_> = itens.iter().filter(|i| i.nome == "revisar").collect();
    assert_eq!(revisar.len(), 1);
    assert_eq!((revisar[0].origem, revisar[0].descricao.as_str(), revisar[0].insere.as_str()), ("projeto", "Versão do projeto", "/revisar "));
    assert!(!nomes.contains(&"nome com espaco"));
    assert_eq!(itens.iter().find(|i| i.nome == "front:teste").unwrap().descricao, "Roda os testes do front");
}

#[test]
fn codex_usa_cifrao_e_antigravity_texto() {
    let dir = tempfile::tempdir().unwrap();
    let casa = dir.path();
    skill(&casa.join(".agents/skills"), "doc", "---\ndescription: d\n---\n");
    let codex = listar("codex", Some(&casa.join(".codex")), None, Some(casa));
    assert_eq!(codex.iter().find(|i| i.nome == "doc").unwrap().insere, "$doc ");
    skill(&casa.join(".gemini/config/skills"), "video", "---\ndescription: v\n---\n");
    let agy = listar("antigravity", Some(&casa.join(".gemini/antigravity-cli")), None, Some(casa));
    assert_eq!(agy.iter().find(|i| i.nome == "video").unwrap().insere, "Use a skill video: ");
    assert!(agy.iter().all(|i| i.tipo != "cli"));
}

#[test]
fn texto_para_cli_so_aceita_o_que_foi_listado() {
    let dir = tempfile::tempdir().unwrap();
    skill(&dir.path().join("skills"), "revisar", "---\ndescription: r\n---\n");
    let itens = listar("claude", Some(dir.path()), None, None);
    assert_eq!(texto_para_cli(&itens, "/revisar o diff de hoje").unwrap(), "/revisar o diff de hoje");
    assert_eq!(texto_para_cli(&itens, "  /revisar  ").unwrap(), "/revisar");
    for ruim in ["/model opus", "/config", "/exit", "/compact", "revisar", "/revisar\u{1b}[A", "/revisar\r/exit", "/", "/re visar"] {
        assert!(texto_para_cli(&itens, ruim).is_err(), "{ruim:?}");
    }
}
