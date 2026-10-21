//! Fork (Editor de vídeo): exportar o vídeo com o render **local** do HyperFrames, na versão fixada.
//!
//! - `npx --yes hyperframes@0.8.183 render --output renders/<nome> --format <f> --quality <q>`,
//!   na pasta do projeto, com `nice` (prioridade baixa) e fora da janela. Nada de render em nuvem.
//! - Progresso: o CLI imprime `@hf-progress {"pct":..}` quando a saída não é terminal
//!   (`packages/cli/src/ui/progress.ts` da v0.8.183); a barra `NN%  etapa` é a reserva.
//! - Cancelar mata o grupo do processo (o Chrome do render vai junto).
//! - `doctor --json` mostra se Node 22+, FFmpeg e o Chrome estão prontos.

use std::path::Path;
use std::time::{Duration, Instant};

use tauri::Emitter;

use super::processo::{self, Execucao, Linha};
use super::projetos::VERSAO_DO_HYPERFRAMES;

const LIMITE: Duration = Duration::from_secs(2 * 60 * 60);
pub const EVENTO: &str = "video-render";
const FORMATOS: &[&str] = &["mp4", "webm", "mov", "gif"];
const QUALIDADES: &[&str] = &["draft", "standard", "high"];

fn pacote() -> String { format!("hyperframes@{VERSAO_DO_HYPERFRAMES}") }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PedidoDeExportacao {
    pub projeto: String,
    pub formato: String,
    pub qualidade: String,
    pub fps: Option<u32>,
    pub nome: String,
}

pub fn argumentos(p: &PedidoDeExportacao, saida: &str) -> Result<Vec<String>, String> {
    if !FORMATOS.contains(&p.formato.as_str()) { return Err("Formato inválido.".into()); }
    if !QUALIDADES.contains(&p.qualidade.as_str()) { return Err("Qualidade inválida.".into()); }
    let mut args = vec!["--yes".into(), pacote(), "render".into(), "--output".into(), saida.into(),
        "--format".into(), p.formato.clone(), "--quality".into(), p.qualidade.clone()];
    if let Some(fps) = p.fps {
        if !(1..=120).contains(&fps) { return Err("Quadros por segundo inválidos.".into()); }
        args.push("--fps".into());
        args.push(fps.to_string());
    }
    Ok(args)
}

fn sem_cores(linha: &str) -> String {
    let mut s = String::with_capacity(linha.len());
    let mut chars = linha.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') { chars.next(); for d in chars.by_ref() { if d.is_ascii_alphabetic() { break; } } }
            continue;
        }
        s.push(c);
    }
    s
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Progresso { pub pct: f64, pub etapa: String }

/// Progresso de uma linha do render, se ela tiver um.
pub fn progresso(linha: &str) -> Option<Progresso> {
    let limpa = sem_cores(linha);
    let t = limpa.trim();
    if let Some(json) = t.strip_prefix("@hf-progress ") {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        let pct = v["pct"].as_f64()?;
        // Na v0.8.183 a etapa vem em `code` (conferido num render real); `done`/`total` são quadros.
        let etapa = match v["code"].as_str().or(v["label"].as_str()).unwrap_or_default() {
            "compile" => "Preparando a composição", "start_browsers" => "Abrindo o navegador do render",
            "extract_video" => "Lendo os vídeos", "process_audio" => "Mixando o áudio", "capture" => "Capturando os quadros",
            "encode" => "Codificando", "assemble" => "Montando o arquivo", outro => outro,
        }.to_owned();
        let etapa = match (v["done"].as_u64(), v["total"].as_u64()) {
            (Some(f), Some(t)) if t > 0 && !etapa.is_empty() => format!("{etapa} ({f} de {t})"),
            _ => etapa,
        };
        return Some(Progresso { pct: pct.clamp(0.0, 100.0), etapa });
    }
    let (antes, depois) = t.split_once('%')?;
    let numero: String = antes.chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<Vec<_>>().into_iter().rev().collect();
    let pct: f64 = numero.parse().ok()?;
    if !(0.0..=100.0).contains(&pct) { return None; }
    Some(Progresso { pct, etapa: depois.trim().chars().take(120).collect() })
}

#[derive(Clone, serde::Serialize)]
struct EventoDoRender<'a> { projeto: &'a str, pct: f64, etapa: &'a str }

pub fn chave(projeto: &str) -> String { format!("video-render:{projeto}") }

/// Roda o render com `exe` (separado para os testes usarem um `npx` falso). Devolve o caminho
/// relativo do arquivo exportado.
pub fn exportar_com(p: &PedidoDeExportacao, exe: &Path, pasta: &Path, carimbo: &str, mut avisar: impl FnMut(&Progresso)) -> Result<String, String> {
    let nome = super::arquivos::nome_seguro(&p.nome);
    let tronco = nome.rsplit_once('.').map(|(t, _)| t.to_owned()).unwrap_or(nome);
    let saida = format!("renders/{tronco}-{carimbo}.{}", p.formato);
    let args = argumentos(p, &saida)?;
    super::arquivos::criar_pasta(&pasta.join("renders")).map_err(|e| e.to_string())?;
    let chave = chave(&p.projeto);
    let mut ultima = String::new();
    let fim = processo::rodar(Execucao {
        chave: &chave, exe, args: &args, entrada: None, env: &processo::ambiente_do_hyperframes(), pasta, limite: LIMITE, prioridade_baixa: true,
    }, |linha| {
        let texto = match linha { Linha::Saida(l) | Linha::Erro(l) => l };
        if let Some(pr) = progresso(&texto) { avisar(&pr); } else if !texto.trim().is_empty() { ultima = sem_cores(&texto).trim().chars().take(300).collect(); }
    })?;
    if !fim.sucesso || super::arquivos::arquivo_comum(&pasta.join(&saida)).is_none() {
        let detalhe = if !ultima.is_empty() { ultima } else { fim.ultima_linha_de_erro };
        return Err(format!("A exportação falhou{}", if detalhe.is_empty() { ".".into() } else { format!(": {detalhe}") }));
    }
    Ok(saida)
}

fn pasta_do(projeto: &str) -> Result<std::path::PathBuf, String> {
    let raiz = super::raiz()?;
    let pasta = super::projetos::pasta_do_projeto(&raiz, projeto).map_err(super::texto_do_erro)?;
    if !pasta.join(super::projetos::PROJETO).is_file() { return Err("Esse projeto não existe.".into()); }
    Ok(pasta)
}

fn npx() -> Result<std::path::PathBuf, String> {
    processo::npx().ok_or_else(|| "Não achei o Node.js (npx) neste PC. Instale o Node 22 ou mais novo para exportar.".into())
}

#[tauri::command(async)]
pub fn videos_exportar(app: tauri::AppHandle, pedido: PedidoDeExportacao) -> Result<String, String> {
    let pasta = pasta_do(&pedido.projeto)?;
    let exe = npx()?;
    let inicio = Instant::now();
    let projeto = pedido.projeto.clone();
    let carimbo = crate::backup_antes_de_atualizar::carimbo(std::time::SystemTime::now());
    let r = exportar_com(&pedido, &exe, &pasta, &carimbo, |pr| { let _ = app.emit(EVENTO, EventoDoRender { projeto: &projeto, pct: pr.pct, etapa: &pr.etapa }); });
    omni_core::registro::info("video_exportar", &[("formato", &pedido.formato), ("ok", if r.is_ok() { "sim" } else { "nao" }), ("ms", &inicio.elapsed().as_millis().to_string())]);
    r.map(|rel| pasta.join(rel).to_string_lossy().into_owned())
}

#[tauri::command]
pub fn videos_exportar_cancelar(projeto: String) { processo::cancelar(&chave(&projeto)); }

/// `doctor --json` do HyperFrames: `{ ok, checks: [{ name, ok, detail, hint }] }`.
#[tauri::command(async)]
pub fn videos_requisitos() -> Result<serde_json::Value, String> {
    let exe = npx()?;
    let pasta = std::env::temp_dir();
    let args: Vec<String> = vec!["--yes".into(), pacote(), "doctor".into(), "--json".into()];
    let mut saida = String::new();
    processo::rodar(Execucao { chave: "video-requisitos", exe: &exe, args: &args, entrada: None, env: &processo::ambiente_do_hyperframes(), pasta: &pasta, limite: Duration::from_secs(300), prioridade_baixa: false }, |l| {
        if let Linha::Saida(l) = l { saida.push_str(&l); saida.push('\n'); }
    })?;
    let inicio = saida.find('{').ok_or("O HyperFrames não respondeu ao doctor.")?;
    serde_json::from_str(&saida[inicio..]).map_err(|_| "Não entendi a resposta do doctor do HyperFrames.".into())
}

/// Baixa o Chrome que o render usa (`browser ensure`). Só quando a pessoa pede.
#[tauri::command(async)]
pub fn videos_preparar_navegador() -> Result<(), String> {
    let exe = npx()?;
    let pasta = std::env::temp_dir();
    let args: Vec<String> = vec!["--yes".into(), pacote(), "browser".into(), "ensure".into()];
    let fim = processo::rodar(Execucao { chave: "video-navegador", exe: &exe, args: &args, entrada: None, env: &processo::ambiente_do_hyperframes(), pasta: &pasta, limite: Duration::from_secs(30 * 60), prioridade_baixa: true }, |_| {})?;
    if fim.sucesso { Ok(()) } else { Err(format!("Não consegui preparar o navegador do render: {}", fim.ultima_linha_de_erro)) }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pedido(formato: &str) -> PedidoDeExportacao {
        PedidoDeExportacao { projeto: "v-render-1".into(), formato: formato.into(), qualidade: "standard".into(), fps: Some(30), nome: "Meu Vídeo".into() }
    }

    #[test]
    fn argumentos_fixam_a_versao_e_recusam_o_estranho() {
        let a = argumentos(&pedido("mp4"), "renders/x.mp4").unwrap();
        assert_eq!(a[..3], ["--yes", "hyperframes@0.8.183", "render"].map(String::from));
        assert!(a.join(" ").contains("--output renders/x.mp4 --format mp4 --quality standard --fps 30"));
        assert!(!a.iter().any(|x| x.contains("cloud") || x.contains("lambda")));
        assert!(argumentos(&pedido("exe"), "x").is_err());
        let mut p = pedido("mp4"); p.qualidade = "--docker".into();
        assert!(argumentos(&p, "x").is_err());
        let mut p = pedido("mp4"); p.fps = Some(999);
        assert!(argumentos(&p, "x").is_err());
    }

    #[test]
    fn le_o_progresso_das_duas_formas() {
        assert_eq!(progresso(r#"@hf-progress {"code":"capture","done":12,"total":180,"pct":42}"#), Some(Progresso { pct: 42.0, etapa: "Capturando os quadros (12 de 180)".into() }));
        assert_eq!(progresso(r#"@hf-progress {"code":"compile","pct":5}"#).unwrap().etapa, "Preparando a composição");
        assert_eq!(progresso("  \u{1b}[32m█████\u{1b}[0m░░░  \u{1b}[1m55%\u{1b}[0m  \u{1b}[2mEncoding\u{1b}[0m").map(|p| p.pct), Some(55.0));
        assert_eq!(progresso("Rendering composition index.html"), None);
    }

    #[cfg(unix)]
    #[test]
    fn exporta_com_npx_falso_e_avisa_o_progresso() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let npx = dir.path().join("npx-falso");
        // Confere a telemetria desligada e cria o arquivo pedido em --output.
        std::fs::write(&npx, "#!/bin/sh\n[ \"$HYPERFRAMES_NO_TELEMETRY\" = 1 ] || exit 3\nwhile [ \"$1\" != \"--output\" ]; do shift; done\necho '@hf-progress {\"code\":\"capture\",\"pct\":50}'\necho '  100%  pronto'\necho video > \"$2\"\n").unwrap();
        std::fs::set_permissions(&npx, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut vistos = Vec::new();
        let saida = exportar_com(&pedido("mp4"), &npx, dir.path(), "20261021-120000", |p| vistos.push(p.pct)).unwrap();
        assert_eq!(saida, "renders/meu-video-20261021-120000.mp4");
        assert_eq!(std::fs::read_to_string(dir.path().join(&saida)).unwrap().trim(), "video");
        assert_eq!(vistos, vec![50.0, 100.0]);
    }

    #[cfg(unix)]
    #[test]
    fn falha_do_render_vira_mensagem() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let npx = dir.path().join("npx-falso");
        std::fs::write(&npx, "#!/bin/sh\necho 'Error: FFmpeg not found' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&npx, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut p = pedido("webm"); p.projeto = "v-render-2".into();
        let erro = exportar_com(&p, &npx, dir.path(), "c", |_| {}).unwrap_err();
        assert!(erro.contains("FFmpeg not found"), "{erro}");
    }
}
