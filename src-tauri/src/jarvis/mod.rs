//! Fork: Jarvis, o copiloto por voz do Medza Agents. Fase 1: conversa só de voz, no modo local e
//! sem custo fora da assinatura:
//! - ouvir e falar: `auxiliar` (reconhecimento de fala e voz da Apple, no próprio Mac);
//! - pensar: `cerebro` (sessão aberta do Claude Code oficial, na primeira conta do Claude com login);
//! - `escuta` decide quando o Enzo terminou de falar; `frases` corta a resposta para falar já;
//! - `janela` é a janela flutuante (sempre por cima, vira bolinha) e o atalho global.
//! Qualquer fala nova do Enzo interrompe a resposta em andamento, como no modo de voz do ChatGPT.
//! Log só de metadados (tempos, contagens); nada do que foi dito.
mod auxiliar;
mod cerebro;
mod escuta;
mod frases;
pub mod janela;

use auxiliar::{Auxiliar, Evento};
use cerebro::{Cerebro, Saida};
use escuta::Escuta;
use frases::Frases;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Modelo do cérebro. O menor e mais rápido: numa conversa falada, a demora pesa mais que a profundidade.
const MODELO: &str = "haiku";

/// O que a janela do Jarvis recebe (evento `jarvis`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ParaATela {
    Estado { estado: &'static str },
    Parcial { texto: String },
    VoceDisse { texto: String },
    Resposta { texto: String },
    RespostaFim,
    Nivel { mic: f32, fala: f32 },
    Interrompido,
    Aviso { msg: String },
    Erro { msg: String },
}

struct Turno {
    descartar: bool,
    frases: Frases,
    recebeu_pedaco: bool,
    pedido_em: Instant,
}

struct Sessao {
    geracao: u64,
    app: Option<AppHandle>,
    voz: Option<Auxiliar>,
    cerebro: Option<Cerebro>,
    voz_pronta: bool,
    ouvindo: bool,
    falando: bool,
    escuta: Escuta,
    parcial: String,
    turnos: VecDeque<Turno>,
    ultima_fala: u64,
    ultimo_estado: &'static str,
    /// Para os testes: o que iria para a tela e para a voz.
    #[cfg(test)]
    saidas: Vec<ParaATela>,
    #[cfg(test)]
    falas: Vec<String>,
}

static SESSAO: Mutex<Option<Sessao>> = Mutex::new(None);
static GERACAO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Sessao {
    fn nova(app: Option<AppHandle>) -> Self {
        Self {
            geracao: GERACAO.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1,
            app, voz: None, cerebro: None, voz_pronta: false, ouvindo: true, falando: false,
            escuta: Escuta::default(), parcial: String::new(), turnos: VecDeque::new(), ultima_fala: 0,
            ultimo_estado: "",
            #[cfg(test)] saidas: Vec::new(),
            #[cfg(test)] falas: Vec::new(),
        }
    }

    fn mostrar(&mut self, evento: ParaATela) {
        #[cfg(test)]
        if !matches!(evento, ParaATela::Nivel { .. }) { self.saidas.push(evento.clone()); }
        if let Some(app) = &self.app { let _ = app.emit_to(janela::ROTULO, "jarvis", evento); }
    }

    fn estado(&self) -> &'static str {
        if !self.voz_pronta { "iniciando" }
        else if self.falando { "falando" }
        else if self.turnos.iter().any(|t| !t.descartar) { "pensando" }
        else if !self.ouvindo { "pausado" }
        else { "ouvindo" }
    }

    fn atualizar_estado(&mut self) {
        let estado = self.estado();
        if estado != self.ultimo_estado {
            self.ultimo_estado = estado;
            self.mostrar(ParaATela::Estado { estado });
        }
    }

    fn falar(&mut self, texto: String) {
        self.ultima_fala += 1;
        self.falando = true;
        #[cfg(test)]
        self.falas.push(texto.clone());
        if let Some(voz) = &mut self.voz { voz.falar(self.ultima_fala, &texto); }
    }

    /// Corta a resposta em andamento: para a voz e ignora o resto do que o Claude ainda mandar.
    fn calar(&mut self) {
        let havia_resposta = self.falando || self.turnos.iter().any(|t| !t.descartar);
        if let Some(voz) = &mut self.voz { voz.calar(); }
        self.falando = false;
        for turno in &mut self.turnos { turno.descartar = true; }
        if havia_resposta { self.mostrar(ParaATela::Interrompido); }
    }

    /// Uma fala completa do Enzo (pela voz ou escrita).
    fn tratar_fala(&mut self, texto: String) {
        self.parcial.clear();
        self.mostrar(ParaATela::Parcial { texto: String::new() });
        if escuta::pede_para_parar(&texto) {
            self.calar();
            self.atualizar_estado();
            return;
        }
        self.calar();
        self.mostrar(ParaATela::VoceDisse { texto: texto.clone() });
        let enviado = match &mut self.cerebro {
            Some(cerebro) => cerebro.perguntar(&texto),
            None => Err("O Claude não está aberto para o Jarvis".to_owned()),
        };
        match enviado {
            Ok(()) => self.turnos.push_back(Turno { descartar: false, frases: Frases::default(), recebeu_pedaco: false, pedido_em: Instant::now() }),
            Err(msg) => self.mostrar(ParaATela::Erro { msg }),
        }
        self.atualizar_estado();
    }

    fn ao_evento_da_voz(&mut self, evento: Option<Evento>) {
        let Some(evento) = evento else {
            self.voz = None;
            self.voz_pronta = false;
            self.mostrar(ParaATela::Erro { msg: "A voz do Jarvis fechou. Feche e abra o Jarvis de novo.".into() });
            omni_core::registro::aviso("jarvis_voz_fechou", &[]);
            return;
        };
        match evento {
            Evento::Pronto { eco } => {
                self.voz_pronta = true;
                let ouvindo = self.ouvindo;
                if let Some(voz) = &mut self.voz { voz.ouvir(ouvindo); }
                if !eco { self.mostrar(ParaATela::Aviso { msg: "Sem cancelamento de eco: para interromper, use Esc ou o botão.".into() }); }
            }
            Evento::BaixandoModelo => self.mostrar(ParaATela::Aviso { msg: "Baixando o reconhecimento de fala em português (só na primeira vez)…".into() }),
            Evento::Parcial { texto } => {
                self.escuta.parcial(Instant::now());
                let antes = self.parcial.clone();
                self.mostrar(ParaATela::Parcial { texto: format!("{antes} {texto}").trim().to_owned() });
            }
            Evento::Final { texto } => {
                self.escuta.trecho_final(&texto, Instant::now());
                self.parcial = format!("{} {texto}", self.parcial).trim().to_owned();
                let parcial = self.parcial.clone();
                self.mostrar(ParaATela::Parcial { texto: parcial });
            }
            Evento::Nivel { mic, fala } => self.mostrar(ParaATela::Nivel { mic, fala }),
            Evento::Falando { .. } => self.falando = true,
            Evento::Falou { id } => if id == self.ultima_fala { self.falando = false },
            Evento::Calado => self.falando = false,
            Evento::Interrompido => self.calar(),
            Evento::Eco => {}
            Evento::Aviso { msg } => self.mostrar(ParaATela::Aviso { msg }),
            Evento::Erro { msg, codigo } => {
                omni_core::registro::aviso("jarvis_voz_erro", &[("codigo", codigo.as_deref().unwrap_or("-"))]);
                self.mostrar(ParaATela::Erro { msg });
            }
        }
        self.atualizar_estado();
    }

    fn ao_cerebro(&mut self, saida: Option<Saida>) {
        let Some(saida) = saida else {
            self.cerebro = None;
            self.turnos.clear();
            self.mostrar(ParaATela::Erro { msg: "A sessão do Claude fechou. Feche e abra o Jarvis de novo.".into() });
            omni_core::registro::aviso("jarvis_cerebro_fechou", &[]);
            self.atualizar_estado();
            return;
        };
        let mut falar = Vec::new();
        match saida {
            Saida::Pronto => {}
            Saida::Pedaco(texto) | Saida::Mensagem(texto) if self.turnos.front().is_some_and(|t| t.descartar) => { let _ = texto; }
            Saida::Pedaco(texto) => if let Some(turno) = self.turnos.front_mut() {
                if !turno.recebeu_pedaco {
                    let ms = turno.pedido_em.elapsed().as_millis().to_string();
                    omni_core::registro::info("jarvis_primeiro_pedaco", &[("ms", &ms)]);
                }
                turno.recebeu_pedaco = true;
                falar = turno.frases.juntar(&texto);
                self.mostrar(ParaATela::Resposta { texto });
            },
            Saida::Mensagem(texto) => if let Some(turno) = self.turnos.front_mut() {
                // Reserva: versão da CLI sem pedaços manda só a mensagem inteira.
                if !turno.recebeu_pedaco {
                    turno.recebeu_pedaco = true;
                    falar = turno.frases.juntar(&texto);
                    self.mostrar(ParaATela::Resposta { texto });
                }
            },
            Saida::FimDoTurno { erro } => if let Some(mut turno) = self.turnos.pop_front() {
                if !turno.descartar {
                    falar.extend(turno.frases.terminar());
                    if let Some(msg) = erro { self.mostrar(ParaATela::Erro { msg: format!("O Claude não respondeu: {msg}") }); }
                    self.mostrar(ParaATela::RespostaFim);
                }
            },
        }
        for frase in falar { self.falar(frase); }
        self.atualizar_estado();
    }

    fn tique(&mut self) {
        if let Some(fala) = self.escuta.pronta(Instant::now()) { self.tratar_fala(fala); }
    }
}

/// Roda `f` na sessão viva, se ainda for a mesma geração (eventos atrasados de uma sessão fechada caem).
fn com_sessao(geracao: u64, f: impl FnOnce(&mut Sessao)) -> bool {
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    match guarda.as_mut() {
        Some(sessao) if sessao.geracao == geracao => { f(sessao); true }
        _ => false,
    }
}

/// O auxiliar de voz usa o reconhecimento de fala do macOS 26.
fn macos_suportado() -> bool {
    let Ok(saida) = std::process::Command::new("sw_vers").arg("-productVersion").output() else { return false };
    String::from_utf8_lossy(&saida.stdout).trim().split('.').next().and_then(|v| v.parse::<u32>().ok()).is_some_and(|v| v >= 26)
}

fn abrir_cerebro(geracao: u64) -> Result<Cerebro, String> {
    let executavel = crate::agentes::executavel_do_claude().ok_or("O Jarvis pensa com o Claude Code, que não foi encontrado neste Mac")?;
    let perfil = crate::profiles::list_profiles().into_iter().find(|p| p.provider == "claude" && p.authenticated)
        .ok_or("O Jarvis precisa de uma conta do Claude com login")?;
    let pasta = crate::engine_client::engine_dir()?.parent().map(|p| p.join("jarvis")).ok_or("sem pasta de dados")?;
    Cerebro::abrir(&executavel, omni_core::env_for(&perfil), pasta, MODELO, move |saida| {
        com_sessao(geracao, |s| s.ao_cerebro(saida));
    })
}

#[tauri::command(async)]
pub fn jarvis_iniciar(app: AppHandle) -> Result<(), String> {
    jarvis_encerrar();
    if !macos_suportado() { return Err("O Jarvis precisa do macOS 26 ou mais novo (usa o reconhecimento de fala da Apple).".into()); }
    let caminho = auxiliar::caminho().ok_or("A voz do Jarvis não veio neste build do app")?;
    let mut sessao = Sessao::nova(Some(app));
    let geracao = sessao.geracao;
    sessao.ultimo_estado = "iniciando";
    *SESSAO.lock().unwrap_or_else(|e| e.into_inner()) = Some(sessao);

    let voz = Auxiliar::abrir(&caminho, move |evento| { com_sessao(geracao, |s| s.ao_evento_da_voz(evento)); });
    let cerebro = abrir_cerebro(geracao);
    let erro = voz.as_ref().err().or(cerebro.as_ref().err()).cloned();
    com_sessao(geracao, |s| {
        s.voz = voz.ok();
        s.cerebro = cerebro.ok();
        // O auxiliar pode ter ficado pronto antes de ser guardado aqui.
        let ouvindo = s.ouvindo;
        if s.voz_pronta { if let Some(voz) = &mut s.voz { voz.ouvir(ouvindo); } }
    });
    if let Some(erro) = erro { jarvis_encerrar(); return Err(erro); }

    std::thread::spawn(move || {
        while com_sessao(geracao, Sessao::tique) { std::thread::sleep(Duration::from_millis(100)); }
    });
    omni_core::registro::info("jarvis_aberto", &[("modelo", MODELO)]);
    Ok(())
}

/// Fecha voz e Claude (soltando o microfone). Também quando a janela fecha.
#[tauri::command]
pub fn jarvis_encerrar() {
    let sessao = SESSAO.lock().unwrap_or_else(|e| e.into_inner()).take();
    drop(sessao);
}

#[tauri::command]
pub fn jarvis_ouvir(ligado: bool) {
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = guarda.as_mut() {
        s.ouvindo = ligado;
        if !ligado { s.escuta.esquecer(); s.parcial.clear(); s.mostrar(ParaATela::Parcial { texto: String::new() }); }
        if let Some(voz) = &mut s.voz { voz.ouvir(ligado); }
        s.atualizar_estado();
    }
}

#[tauri::command]
pub fn jarvis_calar() {
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = guarda.as_mut() { s.calar(); s.atualizar_estado(); }
}

/// Escrever em vez de falar (campo da janela).
#[tauri::command]
pub fn jarvis_escrever(texto: String) {
    let texto = texto.trim().to_owned();
    if texto.is_empty() { return; }
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = guarda.as_mut() { s.tratar_fala(texto); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessao() -> Sessao {
        let mut s = Sessao::nova(None);
        s.voz_pronta = true;
        s
    }

    fn turno() -> Turno { Turno { descartar: false, frases: Frases::default(), recebeu_pedaco: false, pedido_em: Instant::now() } }

    #[test]
    fn resposta_e_falada_frase_a_frase() {
        let mut s = sessao();
        s.turnos.push_back(turno());
        s.ao_cerebro(Some(Saida::Pedaco("Oi Enzo, tudo certo. O Co".into())));
        assert_eq!(s.falas, vec!["Oi Enzo, tudo certo."]);
        assert_eq!(s.estado(), "falando");
        s.ao_cerebro(Some(Saida::Pedaco("dex terminou".into())));
        s.ao_cerebro(Some(Saida::Mensagem("Oi Enzo, tudo certo. O Codex terminou".into())));
        s.ao_cerebro(Some(Saida::FimDoTurno { erro: None }));
        assert_eq!(s.falas, vec!["Oi Enzo, tudo certo.", "O Codex terminou"]);
        assert!(s.saidas.contains(&ParaATela::RespostaFim));
        s.ao_evento_da_voz(Some(Evento::Falou { id: 1 }));
        assert_eq!(s.estado(), "falando");
        s.ao_evento_da_voz(Some(Evento::Falou { id: 2 }));
        assert_eq!(s.estado(), "ouvindo");
    }

    #[test]
    fn interrupcao_descarta_o_resto_da_resposta() {
        let mut s = sessao();
        s.turnos.push_back(turno());
        s.ao_cerebro(Some(Saida::Pedaco("Primeira frase longa. ".into())));
        s.ao_evento_da_voz(Some(Evento::Interrompido));
        assert!(s.saidas.contains(&ParaATela::Interrompido));
        s.ao_cerebro(Some(Saida::Pedaco("Segunda frase que não sai. ".into())));
        s.ao_cerebro(Some(Saida::FimDoTurno { erro: None }));
        assert_eq!(s.falas, vec!["Primeira frase longa."]);
        assert!(s.turnos.is_empty());
        assert_eq!(s.estado(), "ouvindo");
    }

    #[test]
    fn pedido_de_parar_nao_vai_para_o_claude() {
        let mut s = sessao();
        s.turnos.push_back(turno());
        s.falando = true;
        s.tratar_fala("Medza, para.".into());
        assert!(!s.falando);
        assert_eq!(s.turnos.len(), 1);
        assert!(s.turnos[0].descartar);
        assert!(!s.saidas.iter().any(|e| matches!(e, ParaATela::VoceDisse { .. })));
    }

    #[test]
    fn fala_sem_claude_aberto_mostra_erro() {
        let mut s = sessao();
        s.tratar_fala("que horas são".into());
        assert!(s.saidas.contains(&ParaATela::VoceDisse { texto: "que horas são".into() }));
        assert!(s.saidas.iter().any(|e| matches!(e, ParaATela::Erro { .. })));
        assert!(s.turnos.is_empty());
    }

    #[test]
    fn transcricao_ao_vivo_junta_trechos() {
        let mut s = sessao();
        s.ao_evento_da_voz(Some(Evento::Final { texto: "abre a conversa".into() }));
        s.ao_evento_da_voz(Some(Evento::Parcial { texto: "do Co".into() }));
        assert_eq!(s.saidas.last(), Some(&ParaATela::Parcial { texto: "abre a conversa do Co".into() }));
    }

    #[test]
    fn erro_do_claude_aparece_e_turno_fecha() {
        let mut s = sessao();
        s.turnos.push_back(turno());
        s.ao_cerebro(Some(Saida::FimDoTurno { erro: Some("limite de uso".into()) }));
        assert!(s.saidas.contains(&ParaATela::Erro { msg: "O Claude não respondeu: limite de uso".into() }));
        assert_eq!(s.estado(), "ouvindo");
    }

    #[test]
    fn pausado_quando_nao_ouve() {
        let mut s = sessao();
        s.ouvindo = false;
        assert_eq!(s.estado(), "pausado");
        s.voz_pronta = false;
        assert_eq!(s.estado(), "iniciando");
    }
}
