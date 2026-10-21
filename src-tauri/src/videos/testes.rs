//! Fork (Editor de vídeo): pasta do projeto, versões, mídias por hash e pastas de referências.
use super::{arquivos, midias, projetos, referencias};
use serde_json::json;
use std::fs;
use std::path::Path;

fn novo(nome: &str) -> projetos::NovoProjeto {
    projetos::NovoProjeto {
        projeto: json!({ "nome": nome, "formato": "16:9" }),
        brief: "# Brief\n".into(),
        index: "<div data-composition-id=\"main\"></div>".into(),
        instrucoes: "# Instruções\n".into(),
    }
}

fn criar(raiz: &Path, nome: &str) -> String { projetos::criar(raiz, &novo(nome), None, 1_000, 7).unwrap() }

#[test]
fn cria_a_pasta_completa_e_lista_primeiro_o_mais_novo() {
    let dir = tempfile::tempdir().unwrap();
    let a = criar(dir.path(), "Primeiro");
    let b = projetos::criar(dir.path(), &novo("Segundo"), None, 2_000, 9).unwrap();
    let pasta = dir.path().join("projetos").join(&a);
    for rel in ["projeto.json", "BRIEF.md", "index.html", "midias.json", "CLAUDE.md", "AGENTS.md"] { assert!(pasta.join(rel).is_file(), "{rel}"); }
    for sub in ["assets", "compositions", "renders", "historico"] { assert!(pasta.join(sub).is_dir(), "{sub}"); }
    let projeto: serde_json::Value = serde_json::from_str(&fs::read_to_string(pasta.join("projeto.json")).unwrap()).unwrap();
    assert_eq!(projeto["id"], a.as_str());
    assert_eq!(projeto["hyperframes"], projetos::VERSAO_DO_HYPERFRAMES);
    let lista = projetos::listar(dir.path());
    assert_eq!(lista.iter().map(|i| i.id.clone()).collect::<Vec<_>>(), vec![b, a]);
}

#[test]
fn projeto_sem_nome_nao_e_criado() {
    let dir = tempfile::tempdir().unwrap();
    let mut n = novo(" ");
    assert!(projetos::criar(dir.path(), &n, None, 1, 1).is_err());
    n.projeto = json!("texto");
    assert!(projetos::criar(dir.path(), &n, None, 1, 1).is_err());
}

#[test]
fn pasta_fora_do_indice_continua_na_lista() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "Solto");
    fs::remove_file(dir.path().join("indice.json")).unwrap();
    assert_eq!(projetos::listar(dir.path())[0].id, id);
}

#[test]
fn so_grava_arquivos_da_lista_fechada() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    for ruim in ["../fora.html", "compositions/../x.html", "assets/a.mp4", "compositions/.oculto.html", "outro.txt", "conversa.jsonl"] {
        assert!(projetos::gravar(dir.path(), &id, ruim, "x", None, 1).is_err(), "{ruim}");
    }
    assert!(projetos::gravar(dir.path(), &id, "midias.json", "não é json", None, 1).is_err());
    assert!(projetos::gravar(dir.path(), &id, "compositions/abertura_1.html", "<template></template>", None, 1).is_ok());
    assert!(projetos::gravar(dir.path(), "inexistente", "index.html", "x", None, 1).is_err());
    assert!(projetos::ler(dir.path(), &id, "../../etc/passwd").is_err());
    assert_eq!(projetos::composicoes(dir.path(), &id).unwrap(), vec!["compositions/abertura_1.html"]);
}

#[cfg(unix)]
#[test]
fn nao_segue_link_simbolico() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    let fora = dir.path().join("segredo.txt");
    fs::write(&fora, "segredo").unwrap();
    let alvo = dir.path().join("projetos").join(&id).join("BRIEF.md");
    fs::remove_file(&alvo).unwrap();
    std::os::unix::fs::symlink(&fora, &alvo).unwrap();
    assert!(projetos::ler(dir.path(), &id, "BRIEF.md").is_err());
    assert!(projetos::gravar(dir.path(), &id, "BRIEF.md", "troca", None, 1).is_err());
    assert_eq!(fs::read_to_string(&fora).unwrap(), "segredo");
}

#[test]
fn versao_guarda_antes_de_gravar_e_desfazer_volta() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    projetos::gravar(dir.path(), &id, "index.html", "v1", None, 10).unwrap();
    let n1 = projetos::gravar(dir.path(), &id, "index.html", "v2", Some("Cortei o começo"), 20).unwrap().unwrap();
    // Nada mudou: não cria versão repetida.
    assert_eq!(projetos::criar_versao(dir.path(), &id, "de novo", 30).unwrap(), Some(n1 + 1));
    assert_eq!(projetos::criar_versao(dir.path(), &id, "de novo", 31).unwrap(), None);
    let versoes = projetos::listar_versoes(dir.path(), &id).unwrap();
    assert_eq!(versoes.len(), 2);
    assert_eq!(versoes[1]["motivo"], "Cortei o começo");

    projetos::gravar(dir.path(), &id, "compositions/nova.html", "<template></template>", None, 40).unwrap();
    let refazer = projetos::restaurar_versao(dir.path(), &id, n1, 50).unwrap().unwrap();
    assert_eq!(projetos::ler(dir.path(), &id, "index.html").unwrap().unwrap(), "v1");
    assert!(projetos::composicoes(dir.path(), &id).unwrap().is_empty(), "composição nova sai, mas fica na versão do refazer");
    projetos::restaurar_versao(dir.path(), &id, refazer, 60).unwrap();
    assert_eq!(projetos::ler(dir.path(), &id, "index.html").unwrap().unwrap(), "v2");
    assert_eq!(projetos::composicoes(dir.path(), &id).unwrap(), vec!["compositions/nova.html"]);
    assert!(projetos::restaurar_versao(dir.path(), &id, 999, 70).is_err());
}

#[test]
fn conversa_so_cresce_e_recusa_linha_invalida() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    projetos::anexar_na_conversa(dir.path(), &id, r#"{"de":"eu","texto":"oi"}"#).unwrap();
    projetos::anexar_na_conversa(dir.path(), &id, r#"{"de":"ia","texto":"olá"}"#).unwrap();
    assert!(projetos::anexar_na_conversa(dir.path(), &id, "nao json").is_err());
    assert!(projetos::anexar_na_conversa(dir.path(), &id, "{\"a\":1}\n{\"b\":2}").is_err());
    let texto = projetos::ler(dir.path(), &id, "conversa.jsonl").unwrap().unwrap();
    assert_eq!(texto.lines().count(), 2);
}

#[test]
fn midia_entra_por_hash_sem_duplicar_e_avisa_quando_some() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    let pasta = dir.path().join("projetos").join(&id);
    let origem = dir.path().join("Vídeo da Câmera.MP4");
    fs::write(&origem, b"quadros falsos").unwrap();
    let lista = pasta.join("midias.json");
    let item = midias::importar(&lista, &pasta.join("assets"), "assets/", &origem, "usar", None, 1).unwrap();
    assert_eq!(item["arquivo"], "assets/video-da-camera.mp4");
    assert_eq!(item["tipo"], "video");
    assert_eq!(item["hash"].as_str().unwrap().len(), 64);
    assert_eq!(fs::read(pasta.join("assets/video-da-camera.mp4")).unwrap(), b"quadros falsos");
    // Mesmo arquivo de novo: mesmo item, papel atualizado.
    let de_novo = midias::importar(&lista, &pasta.join("assets"), "assets/", &origem, "referencia", None, 2).unwrap();
    assert_eq!(de_novo["id"], item["id"]);
    assert_eq!(de_novo["papel"], "referencia");
    // Outro conteúdo com o mesmo nome: nome com o hash.
    let outro = dir.path().join("sub").join("Vídeo da Câmera.MP4");
    fs::create_dir_all(outro.parent().unwrap()).unwrap();
    fs::write(&outro, b"outros quadros").unwrap();
    let segundo = midias::importar(&lista, &pasta.join("assets"), "assets/", &outro, "usar", None, 3).unwrap();
    assert_ne!(segundo["arquivo"], item["arquivo"]);
    let valor: serde_json::Value = serde_json::from_str(&fs::read_to_string(&lista).unwrap()).unwrap();
    assert_eq!(valor["itens"].as_array().unwrap().len(), 2);

    assert!(midias::faltando(&lista, &pasta.join("assets"), "assets/").is_empty());
    fs::remove_file(pasta.join("assets/video-da-camera.mp4")).unwrap();
    let falta = midias::faltando(&lista, &pasta.join("assets"), "assets/");
    assert_eq!(falta.len(), 1);
    assert_eq!(falta[0]["motivo"], "sumiu");

    assert!(midias::importar(&lista, &pasta.join("assets"), "assets/", dir.path(), "usar", None, 4).is_err(), "pasta não entra");
    assert!(midias::importar(&lista, &pasta.join("assets"), "assets/", &outro, "qualquer", None, 4).is_err());
}

#[test]
fn remover_midia_manda_para_a_lixeira() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    let pasta = dir.path().join("projetos").join(&id);
    let origem = dir.path().join("logo.png");
    fs::write(&origem, b"png").unwrap();
    let lista = pasta.join("midias.json");
    let item = midias::importar(&lista, &pasta.join("assets"), "assets/", &origem, "usar", None, 1).unwrap();
    let lixeira = dir.path().join("lixeira").join("midias");
    assert!(midias::remover(&lista, &pasta.join("assets"), "assets/", item["id"].as_str().unwrap(), &lixeira, "20261021-120000").unwrap());
    assert!(!pasta.join("assets/logo.png").exists());
    assert_eq!(fs::read(lixeira.join("20261021-120000-logo.png")).unwrap(), b"png");
    assert!(!midias::remover(&lista, &pasta.join("assets"), "assets/", "nao-existe", &lixeira, "x").unwrap());
}

#[test]
fn pasta_de_referencias_entra_no_projeto_com_links() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    let pasta = referencias::criar(dir.path(), &json!({ "nome": "Reels", "descricao": "Cortes rápidos", "links": [
        { "url": "https://exemplo.com/ref", "nota": "ritmo" }, { "url": "javascript:alert(1)" }
    ] }), 5, 3).unwrap();
    let ref_id = pasta["id"].as_str().unwrap().to_owned();
    assert_eq!(pasta["links"].as_array().unwrap().len(), 1, "só http(s)");
    let foto = dir.path().join("estilo.jpg");
    fs::write(&foto, b"jpg").unwrap();
    referencias::importar(dir.path(), &ref_id, &foto, "referencia", Some("cores"), 6).unwrap();
    let p = referencias::existente(dir.path(), &ref_id).unwrap();
    let projeto = dir.path().join("projetos").join(&id);
    let copiados = midias::copiar_itens(&p.join("pasta.json"), &p.join("arquivos"), "arquivos/", &projeto.join("midias.json"), &projeto.join("assets"), "assets/", None, 7).unwrap();
    assert_eq!(copiados.len(), 1);
    assert_eq!(copiados[0]["papel"], "referencia");
    assert_eq!(copiados[0]["nota"], "cores");
    let midias_do_projeto: serde_json::Value = serde_json::from_str(&fs::read_to_string(projeto.join("midias.json")).unwrap()).unwrap();
    assert_eq!(midias_do_projeto["links"][0]["url"], "https://exemplo.com/ref");
    // Aplicar de novo não duplica.
    midias::copiar_itens(&p.join("pasta.json"), &p.join("arquivos"), "arquivos/", &projeto.join("midias.json"), &projeto.join("assets"), "assets/", None, 8).unwrap();
    let midias_do_projeto: serde_json::Value = serde_json::from_str(&fs::read_to_string(projeto.join("midias.json")).unwrap()).unwrap();
    assert_eq!(midias_do_projeto["itens"].as_array().unwrap().len(), 1);
    assert_eq!(midias_do_projeto["links"].as_array().unwrap().len(), 1);
    assert_eq!(referencias::listar(dir.path()).len(), 1);
    referencias::mandar_para_a_lixeira(dir.path(), &ref_id, "c").unwrap();
    assert!(referencias::listar(dir.path()).is_empty());
}

#[test]
fn backup_leva_o_texto_e_deixa_a_midia_grande_de_fora() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    let pasta = dir.path().join("projetos").join(&id);
    fs::write(pasta.join("assets/grande.mp4"), vec![0u8; 1024]).unwrap();
    fs::create_dir_all(pasta.join(".claude/skills/x")).unwrap();
    fs::write(pasta.join(".claude/skills/x/SKILL.md"), "skill").unwrap();
    projetos::gravar(dir.path(), &id, "index.html", "v2", Some("mudança"), 3).unwrap();
    referencias::criar(dir.path(), &json!({ "nome": "Ref" }), 1, 1).unwrap();
    let destino = tempfile::tempdir().unwrap();
    let n = projetos::copiar_tudo(dir.path(), destino.path()).unwrap();
    let copia = destino.path().join("projetos").join(&id);
    assert!(copia.join("index.html").is_file());
    assert!(copia.join("midias.json").is_file());
    assert!(copia.join("historico/000001/index.html").is_file(), "o histórico vai junto");
    assert!(!copia.join("assets").exists());
    assert!(!copia.join(".claude").exists());
    assert!(destino.path().join("indice.json").is_file());
    assert!(n >= 8);
    let backups = tempfile::tempdir().unwrap();
    let feito = projetos::copiar_para_backups(dir.path(), &id, "sessao-a-dois", backups.path(), "20261021-100000").unwrap();
    assert!(feito.join("index.html").is_file());
    assert!(feito.starts_with(backups.path().join("videos")));
}

#[test]
fn apagar_vai_para_a_lixeira_e_sai_da_lista() {
    let dir = tempfile::tempdir().unwrap();
    let id = criar(dir.path(), "P");
    let destino = projetos::mandar_para_a_lixeira(dir.path(), &id, "20261021-100000").unwrap();
    assert!(destino.join("index.html").is_file());
    assert!(projetos::listar(dir.path()).is_empty());
}

#[test]
fn skills_sao_instaladas_uma_vez_por_versao() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("skills-do-app");
    fs::create_dir_all(skills.join("hyperframes-core/references")).unwrap();
    fs::write(skills.join("hyperframes-core/SKILL.md"), "core").unwrap();
    fs::write(skills.join("hyperframes-core/references/a.md"), "a").unwrap();
    let id = projetos::criar(dir.path(), &novo("P"), Some(&skills), 1, 1).unwrap();
    let pasta = dir.path().join("projetos").join(&id);
    assert_eq!(fs::read_to_string(pasta.join(".claude/skills/hyperframes-core/SKILL.md")).unwrap(), "core");
    assert_eq!(projetos::instalar_skills(&pasta, &skills).unwrap(), 0, "mesma versão: não recopia");
}

#[test]
fn nomes_seguros_para_assets() {
    assert_eq!(arquivos::nome_seguro("Música Final (v2).MP3"), "musica-final-v2.mp3");
    assert_eq!(arquivos::nome_seguro("../../etc/passwd"), "passwd");
    assert_eq!(arquivos::nome_seguro(".bashrc"), "bashrc");
    assert_eq!(arquivos::nome_seguro("???.png"), "arquivo.png");
    assert_eq!(arquivos::tipo_pela_extensao("fonte.woff2"), "fonte");
}
