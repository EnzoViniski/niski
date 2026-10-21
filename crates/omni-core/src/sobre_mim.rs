//! Fork (Sobre mim): o que o usuário conta de si para os agentes, em markdown, um arquivo por área.
//!
//! Mora em `<pasta de dados>/sobre-mim/`: `quem-sou.md`, `vida-pessoal.md`, `trabalho.md`,
//! `estudos.md`, `preferencias.md`, `outros.md`, mais `propostas.md` (onde os agentes anotam fatos
//! novos para o usuário aceitar) e `LEIA-ME.md` (índice gerado a cada gravação). Nada aqui lê
//! transcripts nem credenciais; o texto só sai desta pasta pelo contexto que o engine entrega à CLI
//! (o mesmo caminho da memória compartilhada) e nunca vai para o log.
//!
//! O contexto tem orçamento fixo ([`ORCAMENTO_DO_CONTEXTO`]): áreas grandes entram cortadas com o
//! caminho do arquivo completo, para o agente ler quando precisar.
use serde::{Deserialize, Serialize};
use std::{fs, io::{self, Write}, path::{Path, PathBuf}};

pub const PASTA: &str = "sobre-mim";
pub const MAX_POR_AREA: usize = 10_000;
/// Caracteres de conteúdo das áreas que vão junto no contexto de cada agente.
pub const ORCAMENTO_DO_CONTEXTO: usize = 6_000;
pub const ARQUIVO_DE_PROPOSTAS: &str = "propostas.md";
pub const ARQUIVO_DE_AJUSTES: &str = "ajustes.json";
pub const ARQUIVO_DO_INDICE: &str = "LEIA-ME.md";
/// A tela mostra no máximo isto; o arquivo de propostas maior que [`MAX_BYTES_DE_PROPOSTAS`] não é lido.
pub const MAX_PROPOSTAS: usize = 100;
pub const MAX_BYTES_DE_PROPOSTAS: u64 = 256 * 1024;
const MAX_PROPOSTA: usize = 1_000;

pub struct Area { pub id: &'static str, pub titulo: &'static str, pub descricao: &'static str }

impl Area {
    pub fn arquivo(&self) -> String { format!("{}.md", self.id) }
}

pub const AREAS: &[Area] = &[
    Area { id: "quem-sou", titulo: "Quem sou", descricao: "Nome, como prefere ser chamado, onde mora, idiomas, um resumo de você." },
    Area { id: "vida-pessoal", titulo: "Vida pessoal", descricao: "Família, rotina, hobbies, saúde e o que mais quiser que os agentes saibam." },
    Area { id: "trabalho", titulo: "Empresas e trabalho", descricao: "Empresas, sócios, cargo, clientes, produtos e projetos em andamento." },
    Area { id: "estudos", titulo: "Estudos", descricao: "Escola, faculdade, cursos e o que está aprendendo agora." },
    Area { id: "preferencias", titulo: "Como trabalhar comigo", descricao: "Tom das respostas, idioma, o que os agentes podem decidir sozinhos e o que perguntar antes." },
    Area { id: "outros", titulo: "Outros", descricao: "Qualquer coisa que não coube nas outras áreas." },
];

pub fn area(id: &str) -> Option<&'static Area> { AREAS.iter().find(|a| a.id == id) }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Ajustes {
    /// Entrar no contexto dos agentes.
    pub ativo: bool,
    /// Pedir aos agentes que anotem fatos novos em `propostas.md`.
    pub sugestoes: bool,
}

impl Default for Ajustes { fn default() -> Self { Self { ativo: true, sugestoes: true } } }

#[derive(Clone, Debug, Serialize)]
pub struct AreaPreenchida {
    pub id: &'static str,
    pub titulo: &'static str,
    pub descricao: &'static str,
    pub arquivo: String,
    pub texto: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Proposta {
    /// A linha exata do arquivo: é por ela que aceitar/descartar acha a proposta.
    pub linha: String,
    pub area: String,
    pub texto: String,
    pub origem: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SobreMim {
    pub pasta: String,
    pub ajustes: Ajustes,
    pub areas: Vec<AreaPreenchida>,
    pub propostas: Vec<Proposta>,
    pub max_por_area: usize,
}

pub fn pasta(dir_de_dados: &Path) -> PathBuf { dir_de_dados.join(PASTA) }

fn invalido(mensagem: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, mensagem.to_owned()) }

fn ler_texto(caminho: &Path) -> io::Result<String> {
    match fs::read_to_string(caminho) {
        Ok(texto) => Ok(texto),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(erro) => Err(erro),
    }
}

/// Grava inteiro ou nada (`.tmp` exclusivo + rename), só para o dono (0600, pasta 0700).
fn gravar(caminho: &Path, conteudo: &[u8]) -> io::Result<()> {
    let pasta = caminho.parent().ok_or_else(|| invalido("caminho sem pasta"))?;
    if !pasta.exists() {
        fs::create_dir_all(pasta)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(pasta, fs::Permissions::from_mode(0o700));
        }
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCIA: AtomicU64 = AtomicU64::new(0);
    let nome = caminho.file_name().and_then(|n| n.to_str()).unwrap_or("arquivo");
    let temporario = pasta.join(format!(".{nome}.{}.{}.tmp", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    let resultado = (|| {
        let mut arquivo = opcoes.open(&temporario)?;
        arquivo.write_all(conteudo)?;
        arquivo.sync_all()?;
        fs::rename(&temporario, caminho)
    })();
    if resultado.is_err() { let _ = fs::remove_file(&temporario); }
    resultado
}

fn validar_texto(texto: &str, maximo: usize) -> io::Result<()> {
    if texto.chars().count() > maximo {
        return Err(invalido(&format!("Use até {maximo} caracteres.")));
    }
    if texto.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err(invalido("O texto tem caracteres de controle."));
    }
    Ok(())
}

pub fn ler_ajustes(dir_de_dados: &Path) -> io::Result<Ajustes> {
    match fs::read(pasta(dir_de_dados).join(ARQUIVO_DE_AJUSTES)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Ajustes do Sobre mim ilegíveis. O arquivo foi preservado.")),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => Ok(Ajustes::default()),
        Err(erro) => Err(erro),
    }
}

pub fn salvar_ajustes(dir_de_dados: &Path, ajustes: &Ajustes) -> io::Result<()> {
    gravar(&pasta(dir_de_dados).join(ARQUIVO_DE_AJUSTES), &serde_json::to_vec_pretty(ajustes)?)
}

pub fn ler_area(dir_de_dados: &Path, id: &str) -> io::Result<String> {
    let area = area(id).ok_or_else(|| invalido("Área desconhecida."))?;
    ler_texto(&pasta(dir_de_dados).join(area.arquivo()))
}

pub fn ler(dir_de_dados: &Path) -> io::Result<SobreMim> {
    let base = pasta(dir_de_dados);
    let mut areas = Vec::with_capacity(AREAS.len());
    for area in AREAS {
        let arquivo = base.join(area.arquivo());
        areas.push(AreaPreenchida {
            id: area.id, titulo: area.titulo, descricao: area.descricao,
            texto: ler_texto(&arquivo)?, arquivo: arquivo.to_string_lossy().into_owned(),
        });
    }
    Ok(SobreMim {
        pasta: base.to_string_lossy().into_owned(),
        ajustes: ler_ajustes(dir_de_dados)?,
        areas,
        propostas: propostas(dir_de_dados)?,
        max_por_area: MAX_POR_AREA,
    })
}

pub fn salvar_area(dir_de_dados: &Path, id: &str, texto: &str) -> io::Result<()> {
    let area = area(id).ok_or_else(|| invalido("Área desconhecida."))?;
    validar_texto(texto, MAX_POR_AREA)?;
    gravar(&pasta(dir_de_dados).join(area.arquivo()), texto.as_bytes())?;
    preparar_pasta(dir_de_dados)
}

/// Cabeçalho do `propostas.md` (criado se faltar) e o índice `LEIA-ME.md` (sempre refeito).
fn preparar_pasta(dir_de_dados: &Path) -> io::Result<()> {
    let base = pasta(dir_de_dados);
    let propostas = base.join(ARQUIVO_DE_PROPOSTAS);
    if !propostas.exists() {
        gravar(&propostas, cabecalho_das_propostas().as_bytes())?;
    }
    let indice = contexto_com(dir_de_dados, true)?.unwrap_or_else(|| "# Sobre mim\n\nNada escrito ainda.\n".into());
    gravar(&base.join(ARQUIVO_DO_INDICE), indice.as_bytes())
}

fn cabecalho_das_propostas() -> String {
    format!("# Sugestões dos agentes\n\nUma por linha: `- [área] fato curto (agente, AAAA-MM-DD)`. Áreas: {}.\nO Niski mostra estas linhas para o usuário aceitar, editar ou descartar.\n\n",
        AREAS.iter().map(|a| a.id).collect::<Vec<_>>().join(", "))
}

/// `- [área] texto (origem)`. Área desconhecida vira "outros"; linha fora do formato é ignorada.
pub fn interpretar(linha: &str) -> Option<Proposta> {
    let limpa = linha.trim();
    let resto = limpa.strip_prefix("- [").or_else(|| limpa.strip_prefix("* ["))?;
    let (area_bruta, texto) = resto.split_once(']')?;
    let mut texto = texto.trim().to_owned();
    if texto.is_empty() { return None; }
    let chave = area_bruta.trim().to_lowercase();
    let area = AREAS.iter()
        .find(|a| a.id == chave || a.titulo.to_lowercase() == chave)
        .map_or("outros", |a| a.id).to_owned();
    let mut origem = None;
    if texto.ends_with(')') {
        if let Some(inicio) = texto.rfind('(') {
            let dentro = texto[inicio + 1..texto.len() - 1].trim().to_owned();
            let antes = texto[..inicio].trim_end().to_owned();
            if !dentro.is_empty() && !antes.is_empty() && dentro.chars().count() <= 80 {
                origem = Some(dentro);
                texto = antes;
            }
        }
    }
    Some(Proposta { linha: limpa.to_owned(), area, texto: texto.chars().take(MAX_PROPOSTA).collect(), origem })
}

pub fn propostas(dir_de_dados: &Path) -> io::Result<Vec<Proposta>> {
    let caminho = pasta(dir_de_dados).join(ARQUIVO_DE_PROPOSTAS);
    match fs::metadata(&caminho) {
        Ok(meta) if meta.len() > MAX_BYTES_DE_PROPOSTAS => return Err(io::Error::new(io::ErrorKind::InvalidData,
            "O arquivo de sugestões passou de 256 KB. Abra a pasta e limpe o propostas.md.")),
        Ok(_) => {}
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(erro) => return Err(erro),
    }
    Ok(ler_texto(&caminho)?.lines().filter_map(interpretar).take(MAX_PROPOSTAS).collect())
}

/// Tira a primeira linha igual a `linha` do `propostas.md`. `NotFound` se ela não está mais lá.
fn retirar(dir_de_dados: &Path, linha: &str) -> io::Result<()> {
    let caminho = pasta(dir_de_dados).join(ARQUIVO_DE_PROPOSTAS);
    let atual = ler_texto(&caminho)?;
    let alvo = linha.trim();
    let mut achou = false;
    let mut novo = String::with_capacity(atual.len());
    for l in atual.split_inclusive('\n') {
        if !achou && l.trim() == alvo { achou = true; continue; }
        novo.push_str(l);
    }
    if !achou || alvo.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Essa sugestão já foi tratada."));
    }
    gravar(&caminho, novo.as_bytes())
}

pub fn descartar(dir_de_dados: &Path, linha: &str) -> io::Result<()> { retirar(dir_de_dados, linha) }

/// Acrescenta `texto` (talvez editado pelo usuário) como item no fim da área e tira a proposta.
pub fn aceitar(dir_de_dados: &Path, linha: &str, area_id: &str, texto: &str) -> io::Result<()> {
    let area = area(area_id).ok_or_else(|| invalido("Área desconhecida."))?;
    let texto = texto.trim();
    if texto.is_empty() { return Err(invalido("A sugestão está vazia.")); }
    validar_texto(texto, MAX_PROPOSTA)?;
    let caminho = pasta(dir_de_dados).join(ARQUIVO_DE_PROPOSTAS);
    if !ler_texto(&caminho)?.lines().any(|l| l.trim() == linha.trim()) {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Essa sugestão já foi tratada."));
    }
    let mut atual = ler_area(dir_de_dados, area.id)?;
    if !atual.is_empty() && !atual.ends_with('\n') { atual.push('\n'); }
    atual.push_str(&format!("- {}\n", texto.replace(['\r', '\n'], " ")));
    if atual.chars().count() > MAX_POR_AREA {
        return Err(invalido(&format!("A área \"{}\" está cheia. Resuma o texto dela antes de aceitar.", area.titulo)));
    }
    salvar_area(dir_de_dados, area.id, &atual)?;
    retirar(dir_de_dados, linha)
}

/// Corta em `limite` caracteres, de preferência numa quebra de linha.
fn cortar(texto: &str, limite: usize) -> &str {
    let fim = texto.char_indices().nth(limite).map_or(texto.len(), |(i, _)| i);
    let parte = &texto[..fim];
    match parte.rfind('\n') { Some(i) if i > fim / 2 => &parte[..i], _ => parte }
}

/// Cota de cada área no orçamento: as pequenas entram inteiras e o que sobra é dividido igual.
fn cotas(tamanhos: &[usize], orcamento: usize) -> Vec<usize> {
    let mut cotas = vec![0; tamanhos.len()];
    let mut restantes: Vec<usize> = (0..tamanhos.len()).collect();
    let mut sobra = orcamento;
    while !restantes.is_empty() {
        let parte = sobra / restantes.len();
        let (cabem, nao): (Vec<usize>, Vec<usize>) = restantes.iter().partition(|&&i| tamanhos[i] <= parte);
        if cabem.is_empty() {
            for i in nao { cotas[i] = parte; }
            break;
        }
        for i in cabem { cotas[i] = tamanhos[i]; sobra -= tamanhos[i]; }
        restantes = nao;
    }
    cotas
}

/// O texto que vai para o agente, ou `None` se desligado ou vazio.
pub fn contexto(dir_de_dados: &Path) -> io::Result<Option<String>> { contexto_com(dir_de_dados, false) }

fn contexto_com(dir_de_dados: &Path, mesmo_desligado: bool) -> io::Result<Option<String>> {
    let ajustes = ler_ajustes(dir_de_dados)?;
    if !ajustes.ativo && !mesmo_desligado { return Ok(None); }
    let base = pasta(dir_de_dados);
    let mut preenchidas = Vec::new();
    for area in AREAS {
        let texto = ler_area(dir_de_dados, area.id)?;
        let texto = texto.trim().to_owned();
        if !texto.is_empty() { preenchidas.push((area, texto)); }
    }
    if preenchidas.is_empty() { return Ok(None); }
    let tamanhos: Vec<usize> = preenchidas.iter().map(|(_, t)| t.chars().count()).collect();
    let cotas = cotas(&tamanhos, ORCAMENTO_DO_CONTEXTO);
    let pasta_txt = base.to_string_lossy();
    let mut saida = format!(
        "## Sobre o usuário (Sobre mim, do Niski)\n\
         O usuário escreveu isto para os agentes o conhecerem. Use quando ajudar na tarefa; a tarefa atual e as regras do projeto vêm primeiro. \
         Não invente fatos pessoais nem repita estes dados sem necessidade.\n\
         Arquivos completos (um por área, markdown): {pasta_txt}\n");
    for (((area, texto), tamanho), cota) in preenchidas.iter().zip(&tamanhos).zip(&cotas) {
        saida.push_str(&format!("\n### {}\n", area.titulo));
        if tamanho <= cota {
            saida.push_str(texto);
        } else {
            saida.push_str(cortar(texto, *cota).trim_end());
            saida.push_str(&format!("\n[… cortado aqui. O texto completo está em {}]", base.join(area.arquivo()).to_string_lossy()));
        }
        saida.push('\n');
    }
    if ajustes.sugestoes {
        saida.push_str(&format!(
            "\n### Aprendeu algo novo sobre o usuário?\n\
             Se nesta conversa você souber de algo durável sobre o usuário que não está acima (vida, empresas, estudos, preferências de trabalho), \
             acrescente UMA linha no fim de {} no formato `- [área] fato curto (seu nome, AAAA-MM-DD)`, com área entre: {}. \
             Não edite os outros arquivos: o usuário revisa as sugestões no app. Nunca anote senhas, tokens ou outros segredos.\n",
            base.join(ARQUIVO_DE_PROPOSTAS).to_string_lossy(),
            AREAS.iter().map(|a| a.id).collect::<Vec<_>>().join(", ")));
    }
    Ok(Some(saida))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_uma_area_por_arquivo_com_indice_e_permissao_do_dono() {
        let dir = tempfile::tempdir().unwrap();
        assert!(contexto(dir.path()).unwrap().is_none(), "sem nada escrito não há contexto");
        salvar_area(dir.path(), "trabalho", "Sócio da Medza.").unwrap();
        let base = pasta(dir.path());
        assert_eq!(fs::read_to_string(base.join("trabalho.md")).unwrap(), "Sócio da Medza.");
        assert!(fs::read_to_string(base.join(ARQUIVO_DO_INDICE)).unwrap().contains("Sócio da Medza."));
        assert!(fs::read_to_string(base.join(ARQUIVO_DE_PROPOSTAS)).unwrap().starts_with("# Sugestões dos agentes"));
        let lido = ler(dir.path()).unwrap();
        assert_eq!(lido.areas.len(), AREAS.len());
        assert_eq!(lido.areas.iter().find(|a| a.id == "trabalho").unwrap().texto, "Sócio da Medza.");
        assert!(lido.propostas.is_empty(), "o cabeçalho não vira proposta");
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(base.join("trabalho.md")).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(fs::metadata(&base).unwrap().permissions().mode() & 0o777, 0o700);
        }
        assert!(salvar_area(dir.path(), "../fora", "x").is_err());
        assert!(salvar_area(dir.path(), "outros", &"x".repeat(MAX_POR_AREA + 1)).is_err());
        assert!(salvar_area(dir.path(), "outros", "a\0b").is_err());
        assert!(!dir.path().join("fora.md").exists());
    }

    #[test]
    fn contexto_respeita_desligar_sugestoes_e_orcamento() {
        let dir = tempfile::tempdir().unwrap();
        salvar_area(dir.path(), "quem-sou", "Enzo, prefere respostas curtas.").unwrap();
        salvar_area(dir.path(), "outros", &"linha longa de detalhes\n".repeat(400)).unwrap();
        let texto = contexto(dir.path()).unwrap().unwrap();
        assert!(texto.contains("### Quem sou\nEnzo, prefere respostas curtas."), "a área pequena entra inteira");
        assert!(texto.contains("cortado aqui") && texto.contains("outros.md"), "a grande é cortada com o caminho");
        assert!(texto.chars().count() < ORCAMENTO_DO_CONTEXTO + 2_000, "o contexto não explode: {}", texto.chars().count());
        assert!(texto.contains("propostas.md") && texto.contains("- [área]"));
        salvar_ajustes(dir.path(), &Ajustes { ativo: true, sugestoes: false }).unwrap();
        assert!(!contexto(dir.path()).unwrap().unwrap().contains("propostas.md"));
        salvar_ajustes(dir.path(), &Ajustes { ativo: false, sugestoes: true }).unwrap();
        assert!(contexto(dir.path()).unwrap().is_none());
    }

    #[test]
    fn cotas_dao_as_pequenas_inteiras_e_dividem_o_resto() {
        assert_eq!(cotas(&[100, 50], 1000), vec![100, 50]);
        assert_eq!(cotas(&[100, 5000, 5000], 1000), vec![100, 450, 450]);
        assert_eq!(cotas(&[], 1000), Vec::<usize>::new());
    }

    #[test]
    fn interpreta_propostas_e_ignora_o_que_nao_esta_no_formato() {
        let p = interpretar("- [trabalho] Abriu a Medza em 2025. (Claude, 2026-10-21)").unwrap();
        assert_eq!((p.area.as_str(), p.texto.as_str(), p.origem.as_deref()), ("trabalho", "Abriu a Medza em 2025.", Some("Claude, 2026-10-21")));
        assert_eq!(interpretar("- [Estudos] Cursa Engenharia").unwrap().area, "estudos");
        assert_eq!(interpretar("- [hobby] Corre aos sábados").unwrap().area, "outros");
        assert!(interpretar("# título").is_none());
        assert!(interpretar("- [quem-sou]   ").is_none());
        assert!(interpretar("texto solto").is_none());
    }

    #[test]
    fn aceitar_vai_para_a_area_e_descartar_so_tira_a_linha() {
        let dir = tempfile::tempdir().unwrap();
        salvar_area(dir.path(), "estudos", "Ensino médio no Colégio X").unwrap();
        let arquivo = pasta(dir.path()).join(ARQUIVO_DE_PROPOSTAS);
        let mut f = fs::OpenOptions::new().append(true).open(&arquivo).unwrap();
        writeln!(f, "- [estudos] Faz faculdade de Administração (Codex, 2026-10-20)").unwrap();
        writeln!(f, "- [vida-pessoal] Tem um cachorro").unwrap();
        drop(f);
        let lista = propostas(dir.path()).unwrap();
        assert_eq!(lista.len(), 2);
        aceitar(dir.path(), &lista[0].linha, "estudos", "Faz faculdade de Administração na Y").unwrap();
        assert_eq!(ler_area(dir.path(), "estudos").unwrap(), "Ensino médio no Colégio X\n- Faz faculdade de Administração na Y\n");
        assert_eq!(propostas(dir.path()).unwrap().len(), 1);
        assert_eq!(aceitar(dir.path(), &lista[0].linha, "estudos", "de novo").unwrap_err().kind(), io::ErrorKind::NotFound);
        descartar(dir.path(), &lista[1].linha).unwrap();
        assert!(propostas(dir.path()).unwrap().is_empty());
        assert_eq!(ler_area(dir.path(), "vida-pessoal").unwrap(), "", "descartar não escreve em área nenhuma");
        assert!(fs::read_to_string(&arquivo).unwrap().starts_with("# Sugestões"), "o cabeçalho fica");
    }

    #[test]
    fn arquivo_de_propostas_gigante_nao_e_lido() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(pasta(dir.path())).unwrap();
        fs::write(pasta(dir.path()).join(ARQUIVO_DE_PROPOSTAS), "- [outros] x\n".repeat(30_000)).unwrap();
        assert_eq!(propostas(dir.path()).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
