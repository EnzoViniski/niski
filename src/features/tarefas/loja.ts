/**
 * Fork (Tarefas): estado em memória + disco (`src-tauri/src/tarefas.rs`). Relê a pasta a cada 15 s
 * com a janela em foco e ao voltar para ela — é assim que as tarefas do primo aparecem quando a
 * pasta é compartilhada. Gravações da mesma tarefa entram numa fila (nunca chegam fora de ordem).
 */
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { criarTarefa, mesclar, normalizarPessoa, normalizarTarefa, slug } from "./dominio";
import { CORES_DE_PESSOA, type AjustesDasTarefas, type Pessoa, type Tarefa } from "./tipos";

export const CHAVE_DOS_AJUSTES = "medza-tarefas-ajustes";
// Fork (Copiloto): 5 s — a IA e outras conversas gravam direto nos arquivos e a tela tem de mostrar logo.
const INTERVALO_MS = 5_000;
const DESTAQUE_MS = 6_000;

/** Como os arquivos chegam ao disco. No app, pelos comandos do Tauri; nos testes e no `dev:vite`, em memória. */
export interface TransporteDasTarefas {
  listar(pasta: string | null, tipo: "tarefas" | "pessoas"): Promise<string[]>;
  gravar(pasta: string | null, tipo: "tarefas" | "pessoas", id: string, conteudo: string): Promise<void>;
  levarPara(pasta: string): Promise<number>;
}

const peloApp: TransporteDasTarefas = {
  listar: (pasta, tipo) => invoke<string[]>("tarefas_listar", { pasta, tipo }),
  gravar: (pasta, tipo, id, conteudo) => invoke("tarefas_gravar", { pasta, tipo, id, conteudo }),
  levarPara: (pasta) => invoke<number>("tarefas_levar_para", { pasta }),
};

/** Fora do Tauri: guarda no localStorage, para a tela funcionar no navegador. */
export function transporteEmMemoria(): TransporteDasTarefas {
  const dados = new Map<string, string>();
  return {
    listar: async (_p, tipo) => [...dados.entries()].filter(([k]) => k.startsWith(`${tipo}/`)).map(([, v]) => v),
    gravar: async (_p, tipo, id, conteudo) => { dados.set(`${tipo}/${id}`, conteudo); },
    levarPara: async () => 0,
  };
}

const noTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
let transporte: TransporteDasTarefas = noTauri ? peloApp : transporteEmMemoria();
export function usarTransporteDasTarefas(novo: TransporteDasTarefas) { transporte = novo; }

// ---- Ajustes ----

function ajustesPadrao(): AjustesDasTarefas {
  return {
    eu: { id: "", nome: "", cor: CORES_DE_PESSOA[0], atualizadaEm: "" },
    pastaCompartilhada: "", visao: "dia", filtroPessoa: "todas", filtroCategoria: "", ocultarFeitasAntigas: true,
  };
}

function lerAjustes(): AjustesDasTarefas {
  const padrao = ajustesPadrao();
  try {
    const salvo = JSON.parse(localStorage.getItem(CHAVE_DOS_AJUSTES) ?? "null") as Partial<AjustesDasTarefas> | null;
    if (!salvo || typeof salvo !== "object") return padrao;
    const eu = normalizarPessoa(salvo.eu) ?? padrao.eu;
    return {
      eu,
      pastaCompartilhada: typeof salvo.pastaCompartilhada === "string" ? salvo.pastaCompartilhada : "",
      visao: ["dia", "quadro", "lista", "semana"].includes(salvo.visao as string) ? (salvo.visao as AjustesDasTarefas["visao"]) : "dia",
      filtroPessoa: typeof salvo.filtroPessoa === "string" ? salvo.filtroPessoa : "todas",
      filtroCategoria: typeof salvo.filtroCategoria === "string" ? salvo.filtroCategoria : "",
      ocultarFeitasAntigas: salvo.ocultarFeitasAntigas !== false,
    };
  } catch {
    return padrao;
  }
}

// ---- Estado ----

interface Estado {
  ajustes: AjustesDasTarefas;
  tarefas: Tarefa[];
  pessoas: Pessoa[];
  carregado: boolean;
  erro: string;
  sincronizadoEm: number;
  /** Tarefas que mudaram por fora (IA, copiloto, outra pessoa) há pouco: brilham na tela. */
  destaques: Record<string, number>;
  /** Pedido para a tela abrir os detalhes de uma tarefa (o `n` muda a cada pedido). */
  abrir: { id: string; n: number } | null;
}

let estado: Estado = { ajustes: lerAjustes(), tarefas: [], pessoas: [], carregado: false, erro: "", sincronizadoEm: 0, destaques: {}, abrir: null };
const ouvintes = new Set<() => void>();
function mudar(parcial: Partial<Estado>) {
  estado = { ...estado, ...parcial };
  ouvintes.forEach((o) => o());
}

const pasta = () => estado.ajustes.pastaCompartilhada || null;
const filas = new Map<string, Promise<unknown>>();
function naFila(chave: string, tarefa: () => Promise<unknown>) {
  const proxima = (filas.get(chave) ?? Promise.resolve()).catch(() => undefined).then(tarefa);
  filas.set(chave, proxima);
  void proxima.finally(() => { if (filas.get(chave) === proxima) filas.delete(chave); });
  return proxima;
}

function textoDoErro(erro: unknown) { return erro instanceof Error ? erro.message : String(erro); }

export async function recarregar(): Promise<void> {
  try {
    const [brutasT, brutasP] = await Promise.all([transporte.listar(pasta(), "tarefas"), transporte.listar(pasta(), "pessoas")]);
    const lidas = new Map<string, Tarefa>();
    for (const bruta of brutasT) {
      try {
        const t = normalizarTarefa(JSON.parse(bruta));
        if (t) lidas.set(t.id, lidas.has(t.id) ? mesclar(lidas.get(t.id) as Tarefa, t) : t);
      } catch { /* arquivo corrompido ou pela metade: fica para a próxima leitura */ }
    }
    // Tarefa ainda na fila de gravação: a versão da memória é a mais nova.
    for (const t of estado.tarefas) {
      const disco = lidas.get(t.id);
      if (filas.has(t.id)) lidas.set(t.id, disco ? mesclar(t, disco) : t);
    }
    const pessoas = brutasP.map((b) => { try { return normalizarPessoa(JSON.parse(b)); } catch { return null; } })
      .filter((p): p is Pessoa => p !== null);
    // O que chegou diferente do que a tela já tinha (e não foi esta tela que gravou) ganha destaque.
    const antes = new Map(estado.tarefas.map((t) => [t.id, t.atualizadaEm]));
    const mudaram = estado.carregado ? [...lidas.values()].filter((t) => !filas.has(t.id) && antes.get(t.id) !== t.atualizadaEm).map((t) => t.id) : [];
    mudar({ tarefas: [...lidas.values()], pessoas, carregado: true, erro: "", sincronizadoEm: Date.now(), destaques: comDestaque(mudaram) });
  } catch (erro) {
    mudar({ carregado: true, erro: textoDoErro(erro) });
  }
}

function comDestaque(ids: string[]): Record<string, number> {
  const agora = Date.now();
  const vivos = Object.fromEntries(Object.entries(estado.destaques).filter(([, ate]) => ate > agora));
  for (const id of ids) vivos[id] = agora + DESTAQUE_MS;
  return vivos;
}

/** A tarefa brilha na tela por alguns segundos (mudança feita pela IA). */
export function destacar(ids: string[]) { if (ids.length) mudar({ destaques: comDestaque(ids) }); }

/** Pede para a tela abrir os detalhes de uma tarefa. */
export function pedirAbrirTarefa(id: string) { mudar({ abrir: { id, n: (estado.abrir?.n ?? 0) + 1 } }); }

export function salvar(t: Tarefa): Promise<unknown> {
  const atualizada = { ...t, atualizadaEm: new Date(Math.max(Date.now(), Date.parse(t.atualizadaEm) + 1 || 0)).toISOString() };
  const outras = estado.tarefas.filter((x) => x.id !== t.id);
  mudar({ tarefas: [...outras, atualizada] });
  return naFila(t.id, () => transporte.gravar(pasta(), "tarefas", t.id, JSON.stringify(atualizada, null, 2)))
    .catch((erro) => { mudar({ erro: textoDoErro(erro) }); throw erro; });
}

export function editar(id: string, mudanca: (t: Tarefa) => Tarefa): Promise<unknown> {
  const atual = estado.tarefas.find((t) => t.id === id);
  return atual ? salvar(mudanca(atual)) : Promise.resolve();
}

export function adicionar(campos: Partial<Tarefa> & { titulo: string }): Tarefa {
  const t = criarTarefa(campos, estado.ajustes.eu.id);
  void salvar(t).catch(() => undefined);
  return t;
}

export function apagar(id: string) {
  return editar(id, (t) => ({ ...t, apagada: true }));
}

export function desfazerApagar(id: string) {
  return editar(id, (t) => ({ ...t, apagada: false }));
}

export function salvarAjustes(mudanca: Partial<AjustesDasTarefas>) {
  const anterior = estado.ajustes;
  const ajustes = { ...anterior, ...mudanca };
  if (mudanca.eu) ajustes.eu = { ...mudanca.eu, id: anterior.eu.id || slug(mudanca.eu.nome), atualizadaEm: new Date().toISOString() };
  try { localStorage.setItem(CHAVE_DOS_AJUSTES, JSON.stringify(ajustes)); } catch { /* sem localStorage */ }
  mudar({ ajustes });
  if (mudanca.eu || mudanca.pastaCompartilhada !== undefined) void publicarEu();
  if (mudanca.pastaCompartilhada !== undefined && mudanca.pastaCompartilhada !== anterior.pastaCompartilhada) void recarregar();
}

/** Grava "quem sou eu" na pasta, para a outra pessoa poder me atribuir tarefas. */
async function publicarEu() {
  const { eu } = estado.ajustes;
  if (!eu.id || !eu.nome) return;
  try { await transporte.gravar(pasta(), "pessoas", eu.id, JSON.stringify(eu, null, 2)); } catch (erro) { mudar({ erro: textoDoErro(erro) }); }
}

/** Ao escolher a pasta compartilhada: leva as tarefas deste computador para lá e passa a usar a pasta. */
export async function compartilharEm(novaPasta: string): Promise<number> {
  const copiadas = await transporte.levarPara(novaPasta);
  salvarAjustes({ pastaCompartilhada: novaPasta });
  return copiadas;
}

// ---- Sincronia automática ----

let relogio: ReturnType<typeof setInterval> | null = null;
function ligar() {
  if (relogio) return;
  void recarregar().then(() => publicarEu());
  relogio = setInterval(() => { if (!document.hidden && document.hasFocus()) void recarregar(); }, INTERVALO_MS);
  window.addEventListener("focus", aoVoltar);
}
function desligar() {
  if (relogio) clearInterval(relogio);
  relogio = null;
  window.removeEventListener("focus", aoVoltar);
}
function aoVoltar() { void recarregar(); }

function assinar(ouvinte: () => void) {
  ouvintes.add(ouvinte);
  ligar();
  return () => {
    ouvintes.delete(ouvinte);
    if (!ouvintes.size) desligar();
  };
}

export function useTarefas(): Estado {
  return useSyncExternalStore(assinar, () => estado);
}

/** Pessoas conhecidas: eu primeiro, depois quem publicou na pasta. */
export function pessoasConhecidas(e: Estado): Pessoa[] {
  const eu = e.ajustes.eu;
  const outras = e.pessoas.filter((p) => p.id !== eu.id);
  return eu.id ? [eu, ...outras] : outras;
}

/** Só para testes. */
export function reiniciarParaTeste() {
  desligar();
  filas.clear();
  estado = { ajustes: lerAjustes(), tarefas: [], pessoas: [], carregado: false, erro: "", sincronizadoEm: 0, destaques: {}, abrir: null };
}
