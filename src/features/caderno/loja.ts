/**
 * Fork (Caderno): a lista de cadernos e o texto das páginas na memória, gravados pelos comandos do
 * Rust (`cadernos_*`). Gravações do mesmo arquivo entram numa fila: nunca chegam ao disco fora de ordem.
 */
import { invoke } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { lerConversa, normalizarMeta, novoId, VERSAO_DO_CADERNO, type MensagemDoCaderno, type MetaDoCaderno, type PaginaDoCaderno } from "./tipos";

export interface TransporteDosCadernos {
  listar(): Promise<unknown[]>;
  ler(id: string, arquivo: string): Promise<string | null>;
  gravar(id: string, arquivo: string, conteudo: string): Promise<void>;
  anexar(id: string, linha: string): Promise<void>;
  lixeira(id: string, pagina?: string): Promise<void>;
}

const peloApp: TransporteDosCadernos = {
  listar: () => invoke("cadernos_listar"),
  ler: async (id, arquivo) => (await invoke<string | null>("cadernos_ler", { id, arquivo })) ?? null,
  gravar: (id, arquivo, conteudo) => invoke("cadernos_gravar", { id, arquivo, conteudo }),
  anexar: (id, linha) => invoke("cadernos_anexar_conversa", { id, linha }),
  lixeira: (id, pagina) => invoke("cadernos_para_a_lixeira", { id, pagina: pagina ?? null }),
};
let transporte: TransporteDosCadernos = peloApp;
/** Só para testes. */
export function usarTransporteDosCadernos(novo: TransporteDosCadernos) { transporte = novo; }

// ---- Estado ----

let lista: MetaDoCaderno[] | null = null;
let erroDaLista = "";
const textos = new Map<string, string>();
const conversas = new Map<string, MensagemDoCaderno[]>();
let versao = 0;
const ouvintes = new Set<() => void>();
function avisar() { versao++; ouvintes.forEach((o) => o()); }
export function useCadernos(): number {
  return useSyncExternalStore((o) => { ouvintes.add(o); return () => ouvintes.delete(o); }, () => versao);
}

export const cadernos = () => lista;
export const erroDosCadernos = () => erroDaLista;
export const caderno = (id: string) => lista?.find((c) => c.id === id);
export const textoDaPagina = (id: string, pagina: string) => textos.get(`${id}/${pagina}`);
export const conversaDoCaderno = (id: string) => conversas.get(id);

/** Mais recentes primeiro. */
function ordenar(l: MetaDoCaderno[]) { return [...l].sort((a, b) => b.atualizadoEm.localeCompare(a.atualizadoEm)); }

// ---- Fila de gravação ----

const filas = new Map<string, Promise<unknown>>();
function naFila<T>(chave: string, tarefa: () => Promise<T>): Promise<T> {
  const anterior = filas.get(chave) ?? Promise.resolve();
  const proxima = anterior.catch(() => undefined).then(tarefa);
  filas.set(chave, proxima);
  void proxima.finally(() => { if (filas.get(chave) === proxima) filas.delete(chave); }).catch(() => undefined);
  return proxima;
}
export async function gravacoesPendentes() { await Promise.allSettled([...filas.values()]); }

// ---- Lista e meta ----

export async function carregarCadernos(): Promise<void> {
  try {
    const brutos = await transporte.listar();
    lista = ordenar(brutos.map(normalizarMeta).filter((m): m is MetaDoCaderno => m !== null));
    erroDaLista = "";
  } catch (e) {
    erroDaLista = e instanceof Error ? e.message : String(e);
  }
  avisar();
}

function gravarMeta(meta: MetaDoCaderno) {
  return naFila(`${meta.id}/meta.json`, () => transporte.gravar(meta.id, "meta.json", JSON.stringify(meta, null, 2)));
}

function trocarMeta(meta: MetaDoCaderno) {
  lista = ordenar([...(lista ?? []).filter((c) => c.id !== meta.id), meta]);
  avisar();
}

export async function criarCaderno(dados: Pick<MetaDoCaderno, "nome" | "tipo" | "autor" | "capa" | "papel" | "letra" | "ia">, agora = new Date()): Promise<MetaDoCaderno> {
  const em = agora.toISOString();
  const primeira: PaginaDoCaderno = { id: novoId("p", agora.getTime()), titulo: dados.tipo === "livro" ? "Primeiras impressões" : "Primeira página", criadaEm: em, atualizadaEm: em };
  const meta: MetaDoCaderno = { versao: VERSAO_DO_CADERNO, id: novoId("c", agora.getTime()), ...dados, nome: dados.nome.trim().slice(0, 120) || "Caderno sem nome", paginas: [primeira], criadoEm: em, atualizadoEm: em, arquivado: false };
  await gravarMeta(meta);
  textos.set(`${meta.id}/${primeira.id}`, "");
  trocarMeta(meta);
  return meta;
}

export async function mudarCaderno(id: string, mudanca: Partial<Omit<MetaDoCaderno, "id" | "versao">>, agora = new Date()) {
  const atual = caderno(id);
  if (!atual) return;
  const meta = { ...atual, ...mudanca, atualizadoEm: agora.toISOString() };
  trocarMeta(meta);
  await gravarMeta(meta);
}

export async function mandarCadernoParaALixeira(id: string) {
  await gravacoesPendentes();
  await transporte.lixeira(id);
  lista = (lista ?? []).filter((c) => c.id !== id);
  avisar();
}

// ---- Páginas ----

export async function lerPagina(id: string, pagina: string): Promise<string> {
  const chave = `${id}/${pagina}`;
  const naMemoria = textos.get(chave);
  if (naMemoria !== undefined) return naMemoria;
  const lido = (await transporte.ler(id, `paginas/${pagina}.md`)) ?? "";
  if (!textos.has(chave)) textos.set(chave, lido);
  avisar();
  return textos.get(chave) ?? lido;
}

/** Texto novo na memória já; o disco recebe pela fila (quem chama decide quando: `gravarPagina`). */
export function digitarNaPagina(id: string, pagina: string, texto: string) {
  textos.set(`${id}/${pagina}`, texto);
  avisar();
}

export async function gravarPagina(id: string, pagina: string, agora = new Date()) {
  const texto = textos.get(`${id}/${pagina}`);
  if (texto === undefined) return;
  await naFila(`${id}/paginas/${pagina}.md`, () => transporte.gravar(id, `paginas/${pagina}.md`, texto));
  const atual = caderno(id);
  if (atual) await mudarCaderno(id, { paginas: atual.paginas.map((p) => (p.id === pagina ? { ...p, atualizadaEm: agora.toISOString() } : p)) }, agora);
}

export async function novaPagina(id: string, titulo: string, agora = new Date()): Promise<PaginaDoCaderno | undefined> {
  const atual = caderno(id);
  if (!atual) return;
  const em = agora.toISOString();
  const pagina: PaginaDoCaderno = { id: novoId("p", agora.getTime()), titulo: titulo.trim().slice(0, 200), criadaEm: em, atualizadaEm: em };
  textos.set(`${id}/${pagina.id}`, "");
  await naFila(`${id}/paginas/${pagina.id}.md`, () => transporte.gravar(id, `paginas/${pagina.id}.md`, ""));
  await mudarCaderno(id, { paginas: [...atual.paginas, pagina] }, agora);
  return pagina;
}

export async function renomearPagina(id: string, pagina: string, titulo: string) {
  const atual = caderno(id);
  if (!atual) return;
  await mudarCaderno(id, { paginas: atual.paginas.map((p) => (p.id === pagina ? { ...p, titulo: titulo.slice(0, 200) } : p)) });
}

export async function moverPagina(id: string, pagina: string, destino: string) {
  const atual = caderno(id);
  if (!atual || pagina === destino) return;
  const paginas = atual.paginas.filter((p) => p.id !== pagina);
  const movida = atual.paginas.find((p) => p.id === pagina);
  const de = atual.paginas.findIndex((p) => p.id === pagina), para = atual.paginas.findIndex((p) => p.id === destino);
  if (!movida || para < 0) return;
  const pos = paginas.findIndex((p) => p.id === destino);
  paginas.splice(de < para ? pos + 1 : pos, 0, movida);
  await mudarCaderno(id, { paginas });
}

export async function mandarPaginaParaALixeira(id: string, pagina: string) {
  const atual = caderno(id);
  if (!atual) return;
  await gravacoesPendentes();
  await transporte.lixeira(id, pagina).catch((e) => {
    // Página que nunca chegou ao disco (criada e apagada em seguida): só tira da lista.
    if (!String(e instanceof Error ? e.message : e).includes("Não encontrei")) throw e;
  });
  textos.delete(`${id}/${pagina}`);
  await mudarCaderno(id, { paginas: atual.paginas.filter((p) => p.id !== pagina) });
}

// ---- Conversa ----

export async function lerConversaDoCaderno(id: string): Promise<MensagemDoCaderno[]> {
  const naMemoria = conversas.get(id);
  if (naMemoria) return naMemoria;
  const lidas = lerConversa(await transporte.ler(id, "conversa.jsonl"));
  conversas.set(id, lidas);
  avisar();
  return lidas;
}

export async function acrescentarNaConversa(id: string, mensagem: MensagemDoCaderno) {
  conversas.set(id, [...(conversas.get(id) ?? []), mensagem]);
  avisar();
  await naFila(`${id}/conversa.jsonl`, () => transporte.anexar(id, JSON.stringify(mensagem)));
}

/** Só para testes. */
export function limparCadernos() { lista = null; erroDaLista = ""; textos.clear(); conversas.clear(); filas.clear(); avisar(); }
