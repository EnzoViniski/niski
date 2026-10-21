// Fork (Editor de vídeo, edição a dois): o documento compartilhado (Yjs 13.6.33, MIT).
//
// - `arquivos`: texto de cada arquivo do projeto (index.html, BRIEF.md, midias.json, compositions/*).
//   Cada arquivo é um valor do mapa: o último que grava vence por arquivo (a edição fina de texto a
//   dois fica para depois). A mídia não entra: o `midias.json` leva o hash e o arquivo vem por HTTP.
// - `agente`: quem está com o agente agora (um agente por vez reescreve os arquivos).
// - Presença (awareness, y-protocols 1.0.7): nome, cor, clipe selecionado, agulha.
//
// Mensagens (JSON, o binário do Yjs em base64): `ola` (vetor de estado; quem tem responde com o que
// falta), `estado`/`u` (atualizações) e `a` (presença). O transporte é de fora (Tauri ou falso).
import * as Y from "yjs";
import { Awareness, applyAwarenessUpdate, encodeAwarenessUpdate, removeAwarenessStates } from "y-protocols/awareness";

export interface Presenca { nome: string; cor: string; selecionado?: string | null; tempo?: number; comAgente?: boolean }
export interface Colega extends Presenca { cliente: number }
export interface TransporteDaSessao { enviar(corpo: string): void | Promise<void> }

const REMOTO = Symbol("remoto");
const LOCAL = Symbol("local");
export const CORES_DOS_COLEGAS = ["#f472b6", "#34d399", "#fbbf24", "#22d3ee", "#a78bfa", "#fb923c"];

export function paraBase64(dados: Uint8Array) {
  let s = "";
  for (let i = 0; i < dados.length; i += 0x8000) s += String.fromCharCode(...dados.subarray(i, i + 0x8000));
  return btoa(s);
}
export function deBase64(texto: string) {
  const s = atob(texto);
  const saida = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) saida[i] = s.charCodeAt(i);
  return saida;
}

export type MudancaDeArquivo = (rel: string, conteudo: string | undefined) => void;

export class DocumentoADois {
  readonly doc = new Y.Doc();
  readonly arquivos = this.doc.getMap<string>("arquivos");
  readonly agente = this.doc.getMap<unknown>("agente");
  readonly presenca = new Awareness(this.doc);
  /** Já recebeu o estado de alguém (o convidado só manda o que é dele depois disso). */
  sincronizado = false;
  private ouvintes = new Set<() => void>();
  private aoMudarArquivo: MudancaDeArquivo[] = [];

  constructor(private transporte: TransporteDaSessao, eu: Presenca, host: boolean) {
    this.sincronizado = host;
    this.doc.on("update", (u: Uint8Array, origem: unknown) => {
      if (origem !== REMOTO) void this.transporte.enviar(JSON.stringify({ t: "u", u: paraBase64(u) }));
    });
    this.presenca.on("update", ({ added, updated, removed }: { added: number[]; updated: number[]; removed: number[] }, origem: unknown) => {
      const mudou = [...added, ...updated, ...removed];
      if (origem !== REMOTO && mudou.length) void this.transporte.enviar(JSON.stringify({ t: "a", u: paraBase64(encodeAwarenessUpdate(this.presenca, mudou)) }));
      this.avisar();
    });
    this.arquivos.observe((evento) => {
      if (evento.transaction.origin !== REMOTO) return;
      for (const rel of evento.keysChanged) for (const f of this.aoMudarArquivo) f(rel, this.arquivos.get(rel));
    });
    this.agente.observe(() => this.avisar());
    this.presenca.setLocalState(eu);
  }

  /** Pede o que falta a quem já está na sessão (ao entrar e ao voltar de uma queda). */
  ola() { void this.transporte.enviar(JSON.stringify({ t: "ola", sv: paraBase64(Y.encodeStateVector(this.doc)) })); }

  receber(corpo: string) {
    let m: { t?: string; u?: string; sv?: string };
    try { m = JSON.parse(corpo); } catch { return; }
    if (m.t === "ola" && m.sv) {
      if (!this.sincronizado) return; // quem também acabou de entrar não responde com estado vazio
      void this.transporte.enviar(JSON.stringify({ t: "estado", u: paraBase64(Y.encodeStateAsUpdate(this.doc, deBase64(m.sv))) }));
      const todos = [...this.presenca.getStates().keys()];
      void this.transporte.enviar(JSON.stringify({ t: "a", u: paraBase64(encodeAwarenessUpdate(this.presenca, todos)) }));
    } else if ((m.t === "u" || m.t === "estado") && m.u) {
      Y.applyUpdate(this.doc, deBase64(m.u), REMOTO);
      if (m.t === "estado" && !this.sincronizado) { this.sincronizado = true; this.avisar(); }
    } else if (m.t === "a" && m.u) {
      applyAwarenessUpdate(this.presenca, deBase64(m.u), REMOTO);
    }
  }

  /** Arquivo mudou aqui (timeline, agente, briefing): vai para os outros se for diferente. */
  definirArquivo(rel: string, conteudo: string) {
    if (this.arquivos.get(rel) === conteudo) return;
    this.doc.transact(() => this.arquivos.set(rel, conteudo), LOCAL);
  }

  ouvirArquivos(f: MudancaDeArquivo) { this.aoMudarArquivo.push(f); return () => { this.aoMudarArquivo = this.aoMudarArquivo.filter((x) => x !== f); }; }

  mudarPresenca(parcial: Partial<Presenca>) {
    const atual = this.presenca.getLocalState() as Presenca | null;
    if (!atual) return;
    const novo = { ...atual, ...parcial };
    if (JSON.stringify(novo) !== JSON.stringify(atual)) this.presenca.setLocalState(novo);
  }

  colegas(): Colega[] {
    const lista: Colega[] = [];
    this.presenca.getStates().forEach((estado, cliente) => {
      if (cliente !== this.doc.clientID && estado && typeof (estado as Presenca).nome === "string") lista.push({ ...(estado as Presenca), cliente });
    });
    return lista;
  }

  /** Quem está com o agente, se não for eu e se a pessoa ainda estiver na sessão. */
  agenteCom(): string | null {
    const cliente = this.agente.get("cliente") as number | undefined;
    if (cliente === undefined || cliente === this.doc.clientID) return null;
    if (!this.presenca.getStates().has(cliente)) return null; // saiu ou caiu: a trava não vale mais
    return (this.agente.get("nome") as string) ?? "Alguém";
  }

  /** Pega a vez do agente. `false` = outra pessoa está com ele. */
  pegarAgente(nome: string): boolean {
    if (this.agenteCom()) return false;
    this.doc.transact(() => { this.agente.set("cliente", this.doc.clientID); this.agente.set("nome", nome); this.agente.set("desde", Date.now()); }, LOCAL);
    this.mudarPresenca({ comAgente: true });
    return true;
  }

  soltarAgente() {
    if (this.agente.get("cliente") === this.doc.clientID) this.doc.transact(() => { this.agente.delete("cliente"); this.agente.delete("nome"); this.agente.delete("desde"); }, LOCAL);
    this.mudarPresenca({ comAgente: false });
  }

  assinar(f: () => void) { this.ouvintes.add(f); return () => { this.ouvintes.delete(f); }; }
  private avisar() { for (const f of this.ouvintes) f(); }

  /** Sai avisando os outros e para o relógio interno da presença. */
  sair() {
    removeAwarenessStates(this.presenca, [this.doc.clientID], "saiu");
    this.presenca.destroy();
    this.doc.destroy();
    this.ouvintes.clear();
  }
}
