//! Fork: voz no chat. O microfone grava aqui (cpal) e o whisper.cpp transcreve no próprio Mac —
//! nada de áudio sai da máquina. O modelo (~574 MB) é baixado uma vez, conferido por SHA256, para
//! `<Application Support>/<pasta de perfis>/modelos`, compartilhado entre as variantes do app.
//! O log só registra metadados (duração, tempo, tamanho); nunca o texto transcrito.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
#[path = "integridade_modelo.rs"]
mod integridade_modelo;

const MODELO_ARQUIVO: &str = "ggml-large-v3-turbo-q5_0.bin";
const MODELO_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";
const MODELO_SHA256: &str = "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2";
const MODELO_BYTES: u64 = 574_041_195;
/// O whisper só aceita 16 kHz mono.
const TAXA_DO_WHISPER: u32 = 16_000;
/// Teto de uma gravação: 10 min de áudio (~38 MB em f32) — passa disso, para de acumular.
const MAXIMO_DE_AMOSTRAS: usize = 48_000 * 60 * 10;
/// Abaixo disso o áudio é silêncio digital, não fala baixa (fala perto do microfone passa de 0,05).
const PICO_MINIMO: f32 = 0.002;
/// A prévia olha só o fim da fala: o whisper trabalha em blocos de 30 s, e uma janela menor responde rápido.
const JANELA_DA_PREVIA_S: usize = 20;
const MICROFONE_MUDO: &str = "O microfone mandou só silêncio. Libere o Medza Agents em Ajustes do Sistema → Privacidade e Segurança → Microfone e reabra o app.";

struct Gravacao {
    parar: mpsc::Sender<()>,
    amostras: Arc<Mutex<Vec<f32>>>,
    taxa: u32,
    fio: JoinHandle<()>,
    inicio: Instant,
}

static GRAVACAO: Mutex<Option<Gravacao>> = Mutex::new(None);
static CONTEXTO: Mutex<Option<Arc<WhisperContext>>> = Mutex::new(None);
static BAIXANDO: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
pub struct EstadoDaVoz {
    modelo_pronto: bool,
    baixando: bool,
    baixado_bytes: u64,
    total_bytes: u64,
    gravando: bool,
}

fn pasta_de_modelos() -> Result<PathBuf, String> {
    omni_protocol::local_data_root()
        .map(|base| base.join(omni_protocol::PASTA_DE_PERFIS).join("modelos"))
        .ok_or_else(|| "pasta de dados do usuário indisponível".to_string())
}

fn caminho_do_modelo() -> Result<PathBuf, String> {
    pasta_de_modelos().map(|pasta| pasta.join(MODELO_ARQUIVO))
}

fn caminho_parcial() -> Result<PathBuf, String> {
    pasta_de_modelos().map(|pasta| pasta.join(format!("{MODELO_ARQUIVO}.parcial")))
}

#[tauri::command]
pub fn voz_estado() -> EstadoDaVoz {
    let pronto = caminho_do_modelo().ok().and_then(|c| c.metadata().ok()).is_some_and(|m| m.len() == MODELO_BYTES);
    let baixado = caminho_parcial().ok().and_then(|c| c.metadata().ok()).map_or(0, |m| m.len());
    EstadoDaVoz {
        modelo_pronto: pronto,
        baixando: BAIXANDO.load(Ordering::SeqCst),
        baixado_bytes: if pronto { MODELO_BYTES } else { baixado },
        total_bytes: MODELO_BYTES,
        gravando: GRAVACAO.lock().map(|g| g.is_some()).unwrap_or(false),
    }
}

/// Baixa com o `curl` do sistema (o app não tem cliente HTTP próprio) e só aceita o arquivo se o
/// SHA256 bater. O progresso sai do tamanho do `.parcial`, lido por `voz_estado`.
#[tauri::command]
pub async fn voz_baixar_modelo() -> Result<(), String> {
    if BAIXANDO.swap(true, Ordering::SeqCst) {
        return Err("O modelo de voz já está sendo baixado".into());
    }
    let resultado = tauri::async_runtime::spawn_blocking(baixar).await.map_err(|e| e.to_string()).and_then(|r| r);
    BAIXANDO.store(false, Ordering::SeqCst);
    resultado
}

fn baixar() -> Result<(), String> {
    let destino = caminho_do_modelo()?;
    let parcial = caminho_parcial()?;
    std::fs::create_dir_all(pasta_de_modelos()?).map_err(|e| e.to_string())?;
    let inicio = Instant::now();
    let status = Command::new("curl")
        .args(["-L", "--fail", "--silent", "--show-error", "--retry", "3", "-C", "-", "-o"])
        .arg(&parcial)
        .arg(MODELO_URL)
        .status()
        .map_err(|e| format!("Não consegui rodar o curl: {e}"))?;
    if !status.success() {
        return Err("Falha ao baixar o modelo de voz; tente de novo (o download continua de onde parou)".into());
    }
    // SHA-256 em Rust: o Windows não fornece o `shasum` que existe no macOS.
    if let Err(erro) = integridade_modelo::conferir(&parcial, MODELO_BYTES, MODELO_SHA256) {
        let _ = std::fs::remove_file(&parcial);
        return Err(format!("O modelo de voz não passou na verificação: {erro}. Tente de novo."));
    }
    std::fs::rename(&parcial, &destino).map_err(|e| e.to_string())?;
    omni_core::registro::info("voz_modelo_baixado", &[("ms", &inicio.elapsed().as_millis().to_string())]);
    Ok(())
}

#[tauri::command]
pub fn voz_gravar() -> Result<(), String> {
    let mut atual = GRAVACAO.lock().map_err(|_| "estado da voz corrompido")?;
    if atual.is_some() {
        return Ok(());
    }
    let amostras = Arc::new(Mutex::new(Vec::<f32>::with_capacity(48_000 * 30)));
    let (parar, esperar) = mpsc::channel::<()>();
    let (pronto, abriu) = mpsc::channel::<Result<u32, String>>();
    let destino = amostras.clone();
    // O `Stream` do cpal não é `Send` no macOS: nasce, toca e morre neste fio.
    let fio = std::thread::spawn(move || {
        let stream = match abrir_microfone(destino) {
            Ok((stream, taxa)) => { let _ = pronto.send(Ok(taxa)); stream }
            Err(erro) => { let _ = pronto.send(Err(erro)); return; }
        };
        let _ = esperar.recv();
        drop(stream);
    });
    let taxa = abriu.recv().map_err(|_| "o microfone não respondeu".to_string())??;
    *atual = Some(Gravacao { parar, amostras, taxa, fio, inicio: Instant::now() });
    Ok(())
}

fn abrir_microfone(destino: Arc<Mutex<Vec<f32>>>) -> Result<(cpal::Stream, u32), String> {
    let dispositivo = cpal::default_host().default_input_device()
        .ok_or("Nenhum microfone encontrado")?;
    let config = dispositivo.default_input_config().map_err(|e| format!("Microfone indisponível: {e}"))?;
    let taxa = config.sample_rate();
    let canais = config.channels() as usize;
    let formato = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let stream = match formato {
        SampleFormat::F32 => construir::<f32>(&dispositivo, &config, canais, destino),
        SampleFormat::I16 => construir::<i16>(&dispositivo, &config, canais, destino),
        SampleFormat::I32 => construir::<i32>(&dispositivo, &config, canais, destino),
        SampleFormat::U16 => construir::<u16>(&dispositivo, &config, canais, destino),
        outro => return Err(format!("Formato de áudio não suportado: {outro}")),
    }?;
    stream.play().map_err(|e| format!("Não consegui ligar o microfone: {e}"))?;
    Ok((stream, taxa))
}

fn construir<T: SizedSample>(dispositivo: &cpal::Device, config: &cpal::StreamConfig, canais: usize,
    destino: Arc<Mutex<Vec<f32>>>) -> Result<cpal::Stream, String>
where f32: cpal::FromSample<T> {
    dispositivo.build_input_stream(
        *config,
        move |dados: &[T], _: &_| {
            // `lock`, não `try_lock`: a prévia copia o áudio por ~1 ms, e perder o pedaço deixaria buraco na fala.
            let Ok(mut amostras) = destino.lock() else { return };
            if amostras.len() >= MAXIMO_DE_AMOSTRAS { return; }
            // Média dos canais: o whisper quer mono.
            for quadro in dados.chunks(canais.max(1)) {
                let soma: f32 = quadro.iter().map(|s| s.to_sample::<f32>()).sum();
                amostras.push(soma / quadro.len() as f32);
            }
        },
        |erro| omni_core::registro::aviso("voz_erro_no_microfone", &[("erro", &erro.to_string())]),
        None,
    ).map_err(|e| {
        let texto = e.to_string();
        if texto.to_lowercase().contains("permission") {
            "Sem permissão para o microfone: libere em Ajustes do Sistema → Privacidade → Microfone".to_string()
        } else {
            format!("Não consegui abrir o microfone: {texto}")
        }
    })
}

fn encerrar_gravacao() -> Option<(Vec<f32>, u32, Instant)> {
    let gravacao = GRAVACAO.lock().ok()?.take()?;
    let _ = gravacao.parar.send(());
    let _ = gravacao.fio.join();
    let amostras = std::mem::take(&mut *gravacao.amostras.lock().ok()?);
    Some((amostras, gravacao.taxa, gravacao.inicio))
}

#[tauri::command]
pub fn voz_cancelar() {
    let _ = encerrar_gravacao();
}

/// Prévia durante a gravação: transcreve só os últimos segundos (o custo não cresce com a fala) e
/// não para de gravar. O texto final sai de `voz_transcrever`, que relê o áudio inteiro. Sem log:
/// roda a cada segundo e só repetiria o `voz_transcreveu`.
#[tauri::command]
pub async fn voz_previa() -> Result<String, String> {
    let (trecho, taxa) = {
        let atual = GRAVACAO.lock().map_err(|_| "estado da voz corrompido")?;
        let Some(gravacao) = atual.as_ref() else { return Ok(String::new()) };
        let amostras = gravacao.amostras.lock().map_err(|_| "estado da voz corrompido")?;
        let janela = gravacao.taxa as usize * JANELA_DA_PREVIA_S;
        (amostras[amostras.len().saturating_sub(janela)..].to_vec(), gravacao.taxa)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let audio = reamostrar(&trecho, taxa, TAXA_DO_WHISPER);
        if audio.len() < TAXA_DO_WHISPER as usize { return Ok(String::new()); }
        if audio.iter().fold(0.0_f32, |maior, s| maior.max(s.abs())) < PICO_MINIMO { return Ok(String::new()); }
        transcrever(&audio)
    }).await.map_err(|e| e.to_string())?
}

/// Para de gravar e devolve o texto. Primeira vez carrega o modelo (alguns segundos); depois fica
/// na memória enquanto o app estiver aberto.
#[tauri::command]
pub async fn voz_transcrever() -> Result<String, String> {
    let (amostras, taxa, inicio) = encerrar_gravacao().ok_or("Não havia gravação em andamento")?;
    let duracao_ms = inicio.elapsed().as_millis();
    tauri::async_runtime::spawn_blocking(move || {
        let audio = reamostrar(&amostras, taxa, TAXA_DO_WHISPER);
        // Menos de meio segundo: clique sem fala. O whisper costuma inventar texto no silêncio.
        if audio.len() < TAXA_DO_WHISPER as usize / 2 { return Ok(String::new()); }
        // Microfone sem permissão (ou mudo) entrega zeros, e o whisper inventa "E aí" no silêncio.
        let pico = audio.iter().fold(0.0_f32, |maior, s| maior.max(s.abs()));
        if pico < PICO_MINIMO {
            omni_core::registro::aviso("voz_audio_mudo", &[("audio_ms", &duracao_ms.to_string()), ("pico", &format!("{pico:.5}"))]);
            return Err(MICROFONE_MUDO.into());
        }
        let inicio = Instant::now();
        let texto = transcrever(&audio)?;
        omni_core::registro::info("voz_transcreveu", &[
            ("audio_ms", &duracao_ms.to_string()),
            ("ms", &inicio.elapsed().as_millis().to_string()),
            ("caracteres", &texto.chars().count().to_string()),
            ("pico", &format!("{pico:.3}")),
        ]);
        Ok(texto)
    }).await.map_err(|e| e.to_string())?
}

/// Solta o microfone e o modelo (buffers do Metal) antes do processo sair; ver `lib.rs`.
pub fn liberar() {
    let _ = encerrar_gravacao();
    if let Ok(mut guardado) = CONTEXTO.lock() { guardado.take(); }
}

fn contexto() -> Result<Arc<WhisperContext>, String> {
    let mut guardado = CONTEXTO.lock().map_err(|_| "modelo de voz corrompido")?;
    if let Some(contexto) = guardado.as_ref() { return Ok(contexto.clone()); }
    let caminho = caminho_do_modelo()?;
    if !caminho.is_file() { return Err("Baixe o modelo de voz antes".into()); }
    let contexto = WhisperContext::new_with_params(&caminho, WhisperContextParameters::default())
        .map_err(|e| format!("Não consegui carregar o modelo de voz: {e}"))?;
    let contexto = Arc::new(contexto);
    *guardado = Some(contexto.clone());
    Ok(contexto)
}

fn transcrever(audio: &[f32]) -> Result<String, String> {
    let contexto = contexto()?;
    let mut estado = contexto.create_state().map_err(|e| e.to_string())?;
    let mut parametros = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    parametros.set_language(Some("pt"));
    parametros.set_n_threads(std::thread::available_parallelism().map_or(4, |n| n.get().min(8)) as i32);
    parametros.set_no_context(true);
    parametros.set_suppress_blank(true);
    parametros.set_print_special(false);
    parametros.set_print_progress(false);
    parametros.set_print_realtime(false);
    parametros.set_print_timestamps(false);
    estado.full(parametros, audio).map_err(|e| format!("Falha na transcrição: {e}"))?;
    let partes: Vec<String> = estado.as_iter()
        .filter_map(|segmento| segmento.to_str_lossy().ok().map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .collect();
    Ok(partes.join(" "))
}

/// Interpolação linear: suficiente para voz (o microfone do Mac grava a 48 kHz).
fn reamostrar(amostras: &[f32], de: u32, para: u32) -> Vec<f32> {
    if de == para || amostras.is_empty() { return amostras.to_vec(); }
    let razao = de as f64 / para as f64;
    let total = (amostras.len() as f64 / razao) as usize;
    (0..total).map(|i| {
        let pos = i as f64 * razao;
        let base = pos.floor() as usize;
        let fracao = (pos - base as f64) as f32;
        let a = amostras[base.min(amostras.len() - 1)];
        let b = amostras[(base + 1).min(amostras.len() - 1)];
        a + (b - a) * fracao
    }).collect()
}

#[cfg(test)]
mod testes {
    use super::reamostrar;

    #[test]
    fn reamostrar_48k_para_16k_mantem_a_duracao() {
        let um_segundo = vec![0.5_f32; 48_000];
        let saida = reamostrar(&um_segundo, 48_000, 16_000);
        assert_eq!(saida.len(), 16_000);
        assert!(saida.iter().all(|s| (s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn reamostrar_mesma_taxa_nao_muda() {
        assert_eq!(reamostrar(&[0.1, 0.2], 16_000, 16_000), vec![0.1, 0.2]);
    }
}
