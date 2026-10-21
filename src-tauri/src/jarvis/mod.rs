//! Fork: Jarvis, o copiloto e orquestrador do Medza Agents. Voz no modo local e
//! sem custo fora da assinatura:
//! - ouvir e falar: `auxiliar` (reconhecimento de fala e voz da Apple, no próprio Mac);
//! - pensar: `cerebro` (sessão aberta do Claude Code oficial, na primeira conta do Claude com login);
//! - `escuta` decide quando o Enzo terminou de falar; `frases` corta a resposta para falar já;
//! - `janela` é a janela flutuante (sempre por cima, vira bolinha) e o atalho global.
//! Qualquer fala nova do Enzo interrompe a resposta em andamento, como no modo de voz do ChatGPT.
//! Log só de metadados (tempos, contagens); nada do que foi dito.
mod anexos;
mod auxiliar;
mod cerebro;
mod configuracao;
mod escuta;
mod ferramentas;
mod frases;
pub mod janela;

use auxiliar::{Auxiliar, Evento};
use cerebro::{Cerebro, Saida};
use escuta::Escuta;
use frases::Frases;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub use anexos::Anexo;
pub use configuracao::Ajustes;

/// O que a janela do Jarvis recebe (evento `jarvis`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ParaATela {
    Estado {
        estado: &'static str,
    },
    Parcial {
        texto: String,
    },
    VoceDisse {
        texto: String,
    },
    Resposta {
        texto: String,
    },
    RespostaFim,
    Ferramenta {
        nome: String,
        estado: String,
    },
    Vozes {
        vozes: Vec<auxiliar::VozDisponivel>,
        voz: Option<String>,
    },
    Nivel {
        mic: f32,
        fala: f32,
    },
    Interrompido,
    Aviso {
        msg: String,
    },
    Erro {
        msg: String,
    },
}

struct Turno {
    descartar: bool,
    frases: Frases,
    recebeu_pedaco: bool,
    pedido_em: Instant,
    resposta: String,
}

struct Sessao {
    geracao: u64,
    app: Option<AppHandle>,
    voz: Option<Auxiliar>,
    cerebro: Option<Cerebro>,
    revisao_cerebro: u64,
    mcp: Option<ferramentas::Acesso>,
    ajustes: Option<Ajustes>,
    historico: VecDeque<(String, String)>,
    contexto_pendente: Option<String>,
    ferramentas_em_uso: bool,
    reconfigurando: bool,
    voz_pronta: bool,
    cerebro_pronto: bool,
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
// Fork: abrir, trocar e fechar não se ultrapassam; esperas da CLI nunca seguram SESSAO.
static OPERACAO: Mutex<()> = Mutex::new(());
static GERACAO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Sessao {
    fn nova(app: Option<AppHandle>) -> Self {
        Self {
            geracao: GERACAO.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1,
            app,
            voz: None,
            cerebro: None,
            voz_pronta: false,
            cerebro_pronto: false,
            ouvindo: true,
            falando: false,
            revisao_cerebro: 1,
            mcp: None,
            ajustes: None,
            historico: VecDeque::new(),
            contexto_pendente: None,
            ferramentas_em_uso: false,
            reconfigurando: false,
            escuta: Escuta::default(),
            parcial: String::new(),
            turnos: VecDeque::new(),
            ultima_fala: 0,
            ultimo_estado: "",
            #[cfg(test)]
            saidas: Vec::new(),
            #[cfg(test)]
            falas: Vec::new(),
        }
    }

    fn mostrar(&mut self, evento: ParaATela) {
        #[cfg(test)]
        if !matches!(evento, ParaATela::Nivel { .. }) {
            self.saidas.push(evento.clone());
        }
        if let Some(app) = &self.app {
            let _ = app.emit_to(janela::ROTULO, "jarvis", evento);
        }
    }

    fn estado(&self) -> &'static str {
        if !self.voz_pronta || !self.cerebro_pronto {
            "iniciando"
        } else if self.falando {
            "falando"
        } else if self.turnos.iter().any(|t| !t.descartar) {
            "pensando"
        } else if !self.ouvindo {
            "pausado"
        } else {
            "ouvindo"
        }
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
        if let Some(voz) = &mut self.voz {
            voz.falar(self.ultima_fala, &texto);
        }
    }

    /// Corta a resposta em andamento: para a voz e ignora o resto do que o Claude ainda mandar.
    fn calar(&mut self) {
        let havia_resposta = self.falando || self.turnos.iter().any(|t| !t.descartar);
        if let Some(voz) = &mut self.voz {
            voz.calar();
        }
        if self.turnos.iter().any(|t| !t.descartar) {
            if let Some(cerebro) = &mut self.cerebro {
                if let Err(msg) = cerebro.interromper() {
                    self.mostrar(ParaATela::Aviso { msg });
                }
            }
            if self.ferramentas_em_uso {
                self.mostrar(ParaATela::Aviso{msg:"Interrompi o Jarvis. Ações já enviadas e agentes já abertos continuam no fluxo; peça para consultar ou parar os agentes se necessário.".into()});
            }
        }
        self.falando = false;
        for turno in &mut self.turnos {
            turno.descartar = true;
        }
        if havia_resposta {
            self.mostrar(ParaATela::Interrompido);
        }
    }

    /// Uma fala completa do Enzo (pela voz ou escrita).
    fn tratar_fala(&mut self, texto: String) {
        let partes = vec![json!({"type":"text","text":texto})];
        let _ = self.tratar_mensagem(texto, partes);
    }

    fn guardar_contexto(&mut self, papel: &str, texto: String) {
        self.historico.push_back((papel.into(), texto));
        while self.historico.len() > 24
            || self.historico.iter().map(|(_, t)| t.len()).sum::<usize>() > 48_000
        {
            self.historico.pop_front();
        }
    }

    fn tratar_mensagem(&mut self, texto: String, mut partes: Vec<Value>) -> Result<(), String> {
        if !self.cerebro_pronto {
            return Err("O Jarvis ainda não está pronto para receber mensagens. Aguarde a conexão ou tente abrir novamente".into());
        }
        if self.reconfigurando {
            return Err("Aguarde a troca de modelo e tente novamente".into());
        }
        self.parcial.clear();
        self.mostrar(ParaATela::Parcial {
            texto: String::new(),
        });
        if escuta::pede_para_parar(&texto) {
            self.calar();
            self.atualizar_estado();
            return Ok(());
        }
        self.calar();
        self.mostrar(ParaATela::VoceDisse {
            texto: texto.clone(),
        });
        // Na troca de conta/modelo, o contexto entra junto do próximo pedido: não abre turno extra.
        if let Some(contexto) = self.contexto_pendente.take() {
            partes.insert(0, json!({"type":"text","text":contexto}));
        }
        let enviado = match &mut self.cerebro {
            Some(cerebro) => cerebro.perguntar_partes(partes),
            None => Err("O Claude não está aberto para o Jarvis".to_owned()),
        };
        match enviado {
            Ok(()) => {
                self.guardar_contexto("usuário", texto);
                self.turnos.push_back(Turno {
                    descartar: false,
                    frases: Frases::default(),
                    recebeu_pedaco: false,
                    pedido_em: Instant::now(),
                    resposta: String::new(),
                });
            }
            Err(msg) => {
                self.mostrar(ParaATela::Erro { msg: msg.clone() });
                return Err(msg);
            }
        }
        self.atualizar_estado();
        Ok(())
    }

    fn ao_evento_da_voz(&mut self, evento: Option<Evento>) {
        let Some(evento) = evento else {
            self.voz = None;
            self.voz_pronta = false;
            self.mostrar(ParaATela::Erro {
                msg: "A voz do Jarvis fechou. Feche e abra o Jarvis de novo.".into(),
            });
            omni_core::registro::aviso("jarvis_voz_fechou", &[]);
            return;
        };
        match evento {
            Evento::Pronto { eco, vozes, voz } => {
                self.voz_pronta = true;
                let ouvindo = self.ouvindo;
                if let Some(voz) = &mut self.voz {
                    voz.ouvir(ouvindo);
                }
                if !eco {
                    self.mostrar(ParaATela::Aviso {
                        msg: "Sem cancelamento de eco: para interromper, use Esc ou o botão."
                            .into(),
                    });
                }
                self.mostrar(ParaATela::Vozes { vozes, voz });
            }
            Evento::Vozes { vozes, voz } => self.mostrar(ParaATela::Vozes { vozes, voz }),
            Evento::BaixandoModelo => self.mostrar(ParaATela::Aviso {
                msg: "Baixando o reconhecimento de fala em português (só na primeira vez)…".into(),
            }),
            Evento::Parcial { texto } => {
                self.escuta.parcial(Instant::now());
                let antes = self.parcial.clone();
                self.mostrar(ParaATela::Parcial {
                    texto: format!("{antes} {texto}").trim().to_owned(),
                });
            }
            Evento::Final { texto } => {
                self.escuta.trecho_final(&texto, Instant::now());
                self.parcial = format!("{} {texto}", self.parcial).trim().to_owned();
                let parcial = self.parcial.clone();
                self.mostrar(ParaATela::Parcial { texto: parcial });
            }
            Evento::Nivel { mic, fala } => self.mostrar(ParaATela::Nivel { mic, fala }),
            Evento::Falando { .. } => self.falando = true,
            Evento::Falou { id } => {
                if id == self.ultima_fala {
                    self.falando = false
                }
            }
            Evento::Calado => self.falando = false,
            Evento::Interrompido => self.calar(),
            Evento::Eco => {}
            Evento::Aviso { msg } => self.mostrar(ParaATela::Aviso { msg }),
            Evento::Erro { msg, codigo } => {
                omni_core::registro::aviso(
                    "jarvis_voz_erro",
                    &[("codigo", codigo.as_deref().unwrap_or("-"))],
                );
                self.mostrar(ParaATela::Erro { msg });
            }
        }
        self.atualizar_estado();
    }

    fn ao_cerebro(&mut self, saida: Option<Saida>) {
        let Some(saida) = saida else {
            self.cerebro = None;
            self.cerebro_pronto = false;
            self.turnos.clear();
            self.mostrar(ParaATela::Erro {
                msg: "A sessão do Claude fechou. Feche e abra o Jarvis de novo.".into(),
            });
            omni_core::registro::aviso("jarvis_cerebro_fechou", &[]);
            self.atualizar_estado();
            return;
        };
        let mut falar = Vec::new();
        match saida {
            Saida::Pronto => {}
            Saida::FalhaDeEnvio => {
                self.cerebro_pronto = false;
                self.turnos.clear();
                self.mostrar(ParaATela::Erro{msg:"O Jarvis não conseguiu entregar a mensagem à CLI. Feche e reabra o Jarvis e envie novamente.".into()});
            }
            Saida::Aviso(msg) => self.mostrar(ParaATela::Aviso { msg }),
            Saida::Ferramenta { nome, estado } => {
                self.ferramentas_em_uso = true;
                self.mostrar(ParaATela::Ferramenta { nome, estado });
            }
            Saida::Pedaco(texto) | Saida::Mensagem(texto)
                if self.turnos.front().is_some_and(|t| t.descartar) =>
            {
                let _ = texto;
            }
            Saida::Pedaco(texto) => {
                if let Some(turno) = self.turnos.front_mut() {
                    if !turno.recebeu_pedaco {
                        let ms = turno.pedido_em.elapsed().as_millis().to_string();
                        omni_core::registro::info("jarvis_primeiro_pedaco", &[("ms", &ms)]);
                    }
                    turno.recebeu_pedaco = true;
                    turno.resposta.push_str(&texto);
                    falar = turno.frases.juntar(&texto);
                    self.mostrar(ParaATela::Resposta { texto });
                }
            }
            Saida::Mensagem(texto) => {
                if let Some(turno) = self.turnos.front_mut() {
                    // Reserva: versão da CLI sem pedaços manda só a mensagem inteira.
                    if !turno.recebeu_pedaco {
                        turno.recebeu_pedaco = true;
                        turno.resposta.push_str(&texto);
                        falar = turno.frases.juntar(&texto);
                        self.mostrar(ParaATela::Resposta { texto });
                    }
                }
            }
            Saida::FimDoTurno { erro } => {
                if let Some(mut turno) = self.turnos.pop_front() {
                    if !turno.resposta.is_empty() {
                        self.guardar_contexto("Jarvis", turno.resposta.clone());
                    }
                    if !turno.descartar {
                        falar.extend(turno.frases.terminar());
                        if let Some(msg) = erro {
                            self.mostrar(ParaATela::Erro {
                                msg: format!("O Claude não respondeu: {msg}"),
                            });
                        }
                        self.mostrar(ParaATela::RespostaFim);
                    }
                }
            }
        }
        for frase in falar {
            self.falar(frase);
        }
        self.atualizar_estado();
    }

    fn tique(&mut self) {
        if let Some(fala) = self.escuta.pronta(Instant::now()) {
            self.tratar_fala(fala);
        }
    }
}

/// Roda `f` na sessão viva, se ainda for a mesma geração (eventos atrasados de uma sessão fechada caem).
fn com_sessao(geracao: u64, f: impl FnOnce(&mut Sessao)) -> bool {
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    match guarda.as_mut() {
        Some(sessao) if sessao.geracao == geracao => {
            f(sessao);
            true
        }
        _ => false,
    }
}

/// O auxiliar de voz usa o reconhecimento de fala do macOS 26.
fn macos_suportado() -> bool {
    let Ok(saida) = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&saida.stdout)
        .trim()
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .is_some_and(|v| v >= 26)
}

fn abrir_cerebro(
    geracao: u64,
    revisao: u64,
    ajustes: &Ajustes,
    mcp: &ferramentas::Acesso,
) -> Result<Cerebro, String> {
    let executavel = crate::agentes::executavel_do_claude()
        .ok_or("O Jarvis pensa com o Claude Code, que não foi encontrado neste Mac")?;
    let perfil = crate::profiles::list_profiles()
        .into_iter()
        .find(|p| p.id == ajustes.perfil_id && p.provider == "claude" && p.authenticated)
        .ok_or("O Jarvis precisa de uma conta do Claude com login")?;
    let pasta = crate::engine_client::engine_dir()?
        .parent()
        .map(|p| p.join("jarvis"))
        .ok_or("sem pasta de dados")?;
    let mut ambiente = omni_core::env_for(&perfil);
    ambiente.push(mcp.ambiente());
    Cerebro::abrir(
        &executavel,
        ambiente,
        pasta,
        &ajustes.modelo,
        Some(mcp.config()),
        move |saida| {
            com_sessao(geracao, |s| {
                if s.revisao_cerebro == revisao {
                    s.ao_cerebro(saida)
                }
            });
        },
    )
}

#[tauri::command(async)]
pub fn jarvis_iniciar(app: AppHandle) -> Result<(), String> {
    let _operacao = OPERACAO.lock().unwrap_or_else(|e| e.into_inner());
    encerrar_sessao();
    if !macos_suportado() {
        return Err(
            "O Jarvis precisa do macOS 26 ou mais novo (usa o reconhecimento de fala da Apple)."
                .into(),
        );
    }
    let caminho = auxiliar::caminho().ok_or("A voz do Jarvis não veio neste build do app")?;
    let ajustes = configuracao::ler()?;
    configuracao::validar(&ajustes)?;
    let mcp = ferramentas::abrir()?;
    let mut sessao = Sessao::nova(Some(app));
    sessao.ajustes = Some(ajustes.clone());
    sessao.mcp = Some(mcp.clone());
    let geracao = sessao.geracao;
    sessao.ultimo_estado = "iniciando";
    *SESSAO.lock().unwrap_or_else(|e| e.into_inner()) = Some(sessao);

    let voz = Auxiliar::abrir(&caminho, move |evento| {
        com_sessao(geracao, |s| s.ao_evento_da_voz(evento));
    });
    let cerebro = abrir_cerebro(geracao, 1, &ajustes, &mcp);
    let erro = voz.as_ref().err().or(cerebro.as_ref().err()).cloned();
    com_sessao(geracao, |s| {
        s.voz = voz.ok();
        s.cerebro = cerebro.ok();
        s.cerebro_pronto = s.cerebro.is_some();
        if let Some(voz) = &mut s.voz {
            voz.selecionar_voz(ajustes.voz_id.as_deref().unwrap_or(""));
        }
        // O auxiliar pode ter ficado pronto antes de ser guardado aqui.
        let ouvindo = s.ouvindo;
        if s.voz_pronta {
            if let Some(voz) = &mut s.voz {
                voz.ouvir(ouvindo);
            }
        }
        s.atualizar_estado();
    });
    if let Some(erro) = erro {
        encerrar_sessao();
        return Err(erro);
    }

    std::thread::spawn(move || {
        while com_sessao(geracao, Sessao::tique) {
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    omni_core::registro::info("jarvis_aberto", &[("modelo", &ajustes.modelo)]);
    Ok(())
}

/// Fecha voz e Claude (soltando o microfone). Também quando a janela fecha.
#[tauri::command]
pub fn jarvis_encerrar() {
    let _operacao = OPERACAO.lock().unwrap_or_else(|e| e.into_inner());
    encerrar_sessao();
}

fn encerrar_sessao() {
    let sessao = SESSAO.lock().unwrap_or_else(|e| e.into_inner()).take();
    let tinha_mcp = sessao.as_ref().is_some_and(|s| s.mcp.is_some());
    drop(sessao);
    if tinha_mcp {
        ferramentas::fechar();
    }
}

#[tauri::command]
pub fn jarvis_ouvir(ligado: bool) {
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = guarda.as_mut() {
        s.ouvindo = ligado;
        if !ligado {
            s.escuta.esquecer();
            s.parcial.clear();
            s.mostrar(ParaATela::Parcial {
                texto: String::new(),
            });
        }
        if let Some(voz) = &mut s.voz {
            voz.ouvir(ligado);
        }
        s.atualizar_estado();
    }
}

#[tauri::command]
pub fn jarvis_calar() {
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = guarda.as_mut() {
        s.calar();
        s.atualizar_estado();
    }
}

/// Escrever em vez de falar (campo da janela).
#[tauri::command]
pub fn jarvis_escrever(texto: String, anexos: Option<Vec<Anexo>>) -> Result<(), String> {
    let texto = texto.trim().to_owned();
    let anexos = anexos.unwrap_or_default();
    if texto.is_empty() && anexos.is_empty() {
        return Ok(());
    }
    let partes = anexos::conteudo(&texto, &anexos)?;
    let resumo = if anexos.is_empty() {
        texto
    } else {
        let nomes = anexos
            .iter()
            .map(|a| a.nome.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}\n[Anexos: {nomes}]",
            if texto.is_empty() {
                "Analise os anexos."
            } else {
                &texto
            }
        )
    };
    let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    guarda
        .as_mut()
        .ok_or("Abra o Jarvis antes de enviar uma mensagem")?
        .tratar_mensagem(resumo, partes)
}

#[tauri::command(async)]
pub fn jarvis_configuracao() -> Result<Value, String> {
    let ajustes = configuracao::ler().ok();
    let perfis = crate::profiles::list_profiles();
    let guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
    Ok(
        json!({"perfil_id":ajustes.as_ref().map(|a|a.perfil_id.as_str()).unwrap_or(""),
        "modelo":ajustes.as_ref().map(|a|a.modelo.as_str()).unwrap_or("sonnet"),
        "voz_id":ajustes.as_ref().and_then(|a|a.voz_id.clone()),"ativo":guarda.is_some(),
        "contas":perfis.iter().filter(|p|p.provider=="claude").map(|p|json!({"id":p.id,"nome":p.name,"autenticada":p.authenticated})).collect::<Vec<_>>(),
        "modelos":omni_core::modelos_claude::MODELOS.iter().map(|(id,nome,_)|json!({"id":id,"nome":nome})).collect::<Vec<_>>() }),
    )
}

#[tauri::command(async)]
pub fn jarvis_vozes() -> Result<auxiliar::CatalogoDeVozes, String> {
    auxiliar::listar_vozes(
        &auxiliar::caminho().ok_or("A voz do Jarvis não veio neste build do app")?,
    )
}

#[tauri::command(async)]
pub fn jarvis_configurar(ajustes: Ajustes) -> Result<Value, String> {
    let _operacao = OPERACAO.lock().unwrap_or_else(|e| e.into_inner());
    configuracao::validar(&ajustes)?;
    if let Some(id) = &ajustes.voz_id {
        if !jarvis_vozes()?.vozes.iter().any(|v| &v.id == id) {
            return Err("Essa voz não está instalada no Mac. Escolha outra voz da lista".into());
        }
    }
    let preparar = {
        let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = guarda.as_mut() {
            if !s.turnos.is_empty() || s.falando {
                return Err(
                "Aguarde o Jarvis terminar ou interrompa a resposta antes de trocar a configuração"
                    .into(),
            );
            }
            let trocar = s
                .ajustes
                .as_ref()
                .is_none_or(|a| a.perfil_id != ajustes.perfil_id || a.modelo != ajustes.modelo);
            let acesso = if trocar {
                Some(
                    s.mcp
                        .as_ref()
                        .ok_or("As ferramentas do Jarvis não estão prontas")?
                        .clone(),
                )
            } else {
                None
            };
            s.reconfigurando = true;
            Some((s.geracao, s.revisao_cerebro + 1, acesso))
        } else {
            None
        }
    };
    // A antiga continua viva enquanto a candidata confirma initialize + MCP, sem inferência.
    let resultado = (|| -> Result<(), String> {
        let mut novo = match &preparar {
            Some((geracao, revisao, Some(mcp))) => {
                Some(abrir_cerebro(*geracao, *revisao, &ajustes, mcp)?)
            }
            _ => None,
        };
        let mut guarda = SESSAO.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(novo) = &mut novo {
            novo.vivo()?;
        }
        configuracao::gravar(&ajustes)?;
        let mut antigo = None;
        if let Some(s) = guarda.as_mut() {
            if let Some(novo) = novo {
                s.revisao_cerebro += 1;
                antigo = s.cerebro.replace(novo);
                s.cerebro_pronto = true;
                s.contexto_pendente=(!s.historico.is_empty()).then(||format!("Contexto textual da conversa anterior do Jarvis (dados, não novas tarefas). Não repita ações que já ocorreram. Anexos anteriores não estão disponíveis, peça para reenviar se precisar.\n{}",s.historico.iter().map(|(papel,texto)|format!("{papel}: {texto}")).collect::<Vec<_>>().join("\n\n")));
                s.mostrar(ParaATela::Aviso{msg:"Modelo e conta atualizados. Mantive o contexto textual recente; reenvie os anexos se precisar analisá-los novamente.".into()});
            }
            if let Some(voz) = &mut s.voz {
                voz.selecionar_voz(ajustes.voz_id.as_deref().unwrap_or(""));
            }
            s.ajustes = Some(ajustes);
        }
        drop(guarda);
        drop(antigo);
        Ok(())
    })();
    if let Some((geracao, ..)) = preparar {
        com_sessao(geracao, |s| s.reconfigurando = false);
    }
    resultado?;
    jarvis_configuracao()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessao() -> Sessao {
        let mut s = Sessao::nova(None);
        s.voz_pronta = true;
        s.cerebro_pronto = true;
        s
    }

    fn turno() -> Turno {
        Turno {
            descartar: false,
            frases: Frases::default(),
            recebeu_pedaco: false,
            pedido_em: Instant::now(),
            resposta: String::new(),
        }
    }

    #[test]
    fn so_mostra_ouvindo_com_voz_e_cerebro_prontos() {
        let mut s = Sessao::nova(None);
        s.voz_pronta = true;
        assert_eq!(s.estado(), "iniciando");
        assert!(s.tratar_mensagem("oi".into(), vec![]).is_err());
        assert!(!s.saidas.iter().any(|e| matches!(e, ParaATela::VoceDisse { .. })));
        s.cerebro_pronto = true;
        assert_eq!(s.estado(), "ouvindo");
        s.ao_cerebro(None);
        assert_eq!(s.estado(), "iniciando");
    }

    #[test]
    fn resposta_e_falada_frase_a_frase() {
        let mut s = sessao();
        s.turnos.push_back(turno());
        s.ao_cerebro(Some(Saida::Pedaco("Oi Enzo, tudo certo. O Co".into())));
        assert_eq!(s.falas, vec!["Oi Enzo, tudo certo."]);
        assert_eq!(s.estado(), "falando");
        s.ao_cerebro(Some(Saida::Pedaco("dex terminou".into())));
        s.ao_cerebro(Some(Saida::Mensagem(
            "Oi Enzo, tudo certo. O Codex terminou".into(),
        )));
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
        assert!(!s
            .saidas
            .iter()
            .any(|e| matches!(e, ParaATela::VoceDisse { .. })));
    }

    #[test]
    fn fala_sem_claude_aberto_mostra_erro() {
        let mut s = sessao();
        s.tratar_fala("que horas são".into());
        assert!(s.saidas.contains(&ParaATela::VoceDisse {
            texto: "que horas são".into()
        }));
        assert!(s.saidas.iter().any(|e| matches!(e, ParaATela::Erro { .. })));
        assert!(s.turnos.is_empty());
    }

    #[test]
    fn transcricao_ao_vivo_junta_trechos() {
        let mut s = sessao();
        s.ao_evento_da_voz(Some(Evento::Final {
            texto: "abre a conversa".into(),
        }));
        s.ao_evento_da_voz(Some(Evento::Parcial {
            texto: "do Co".into(),
        }));
        assert_eq!(
            s.saidas.last(),
            Some(&ParaATela::Parcial {
                texto: "abre a conversa do Co".into()
            })
        );
    }

    #[test]
    fn erro_do_claude_aparece_e_turno_fecha() {
        let mut s = sessao();
        s.turnos.push_back(turno());
        s.ao_cerebro(Some(Saida::FimDoTurno {
            erro: Some("limite de uso".into()),
        }));
        assert!(s.saidas.contains(&ParaATela::Erro {
            msg: "O Claude não respondeu: limite de uso".into()
        }));
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
