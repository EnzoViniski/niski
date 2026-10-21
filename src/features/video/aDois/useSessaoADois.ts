// Fork (Editor de vídeo, edição a dois): liga o documento Yjs ao projeto aberto.
//
// - Disco → documento: quando a composição, o briefing ou as mídias mudam aqui (timeline, agente,
//   biblioteca), o arquivo vai para os outros. Mídia nova do convidado sobe antes do `midias.json`.
// - Documento → disco: o que chega dos outros é gravado com uma versão antes ("Mudança de <nome>"),
//   então nada se perde; mídia que falta o convidado baixa do host pelo hash.
// - Fechada a sessão, nada disto roda (sem ouvinte, sem timer).
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as disco from "../armazenamento";
import type { AcoesDoProjeto } from "../useProjeto";
import type { Midias } from "../tipos";
import { CORES_DOS_COLEGAS, DocumentoADois, type Colega } from "./documento";

export interface SituacaoDaSessao {
  tailscale: { pronto: boolean; login?: string | null; dns?: string | null; ips: string[]; erro?: string | null };
  config: { nome?: string; permitidos?: string[] };
  papel: "host" | "convidado" | null;
  projeto: string | null;
  convidados: [string, string][];
}

type EventoDaSessao =
  | { tipo: "mensagem"; de: string; login?: string | null; corpo: string }
  | { tipo: "entrou" | "saiu"; cliente: string; login: string }
  | { tipo: "pedido"; login: string }
  | { tipo: "midiaRecebida"; arquivo: string }
  | { tipo: "caiu"; erro: string }
  | { tipo: "voltou" };

const ARQUIVOS_FIXOS = ["index.html", "BRIEF.md", "midias.json"] as const;

export function corDoNome(nome: string) {
  let h = 0;
  for (const c of nome) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return CORES_DOS_COLEGAS[h % CORES_DOS_COLEGAS.length];
}

export function useSessaoADois(projeto: string, acoes: AcoesDoProjeto) {
  const [papel, setPapel] = useState<"host" | "convidado" | null>(null);
  const [colegas, setColegas] = useState<Colega[]>([]);
  const [agenteCom, setAgenteCom] = useState<string | null>(null);
  const [conexao, setConexao] = useState<"ok" | "caiu">("ok");
  const [pedidos, setPedidos] = useState<string[]>([]);
  const [convite, setConvite] = useState<string | null>(null);
  const [meuNome, setMeuNome] = useState("Eu");
  const [erro, setErro] = useState<string | null>(null);
  const documento = useRef<DocumentoADois | null>(null);
  const acoesAtuais = useRef(acoes);
  acoesAtuais.current = acoes;
  /** Último conteúdo que veio de fora, por arquivo: não devolve para os outros o que acabou de chegar. */
  const doRemoto = useRef(new Map<string, string>());
  const hashesConhecidos = useRef(new Set<string>());

  const ligar = useCallback((comoHost: boolean, nome: string) => {
    documento.current?.sair();
    const doc = new DocumentoADois({ enviar: (corpo) => invoke("videos_sessao_enviar", { corpo }).then(() => undefined, () => undefined) },
      { nome, cor: corDoNome(nome) }, comoHost);
    documento.current = doc;
    doc.assinar(() => { setColegas(doc.colegas()); setAgenteCom(doc.agenteCom()); });
    doc.ouvirArquivos((rel, conteudo) => {
      if (conteudo === undefined) return;
      doRemoto.current.set(rel, conteudo);
      void (async () => {
        const autor = doc.colegas()[0]?.nome ?? "outra pessoa";
        await disco.gravarArquivo(projeto, rel, conteudo, `Mudança de ${autor}`).catch((e) => setErro(String(e)));
        if (rel === "midias.json") await baixarMidiasQueFaltam(comoHost);
        await acoesAtuais.current.recarregar();
      })();
    });
    setPapel(comoHost ? "host" : "convidado");
    setMeuNome(nome);
    return doc;
  }, [projeto]);

  /** Convidado: baixa do host o que o `midias.json` lista e ainda não está aqui. */
  const baixarMidiasQueFaltam = useCallback(async (comoHost: boolean) => {
    if (comoHost) return;
    const faltando = await disco.midiasFaltando(projeto).catch(() => []);
    if (!faltando.length) return;
    const m = JSON.parse((await disco.lerArquivo(projeto, "midias.json")) ?? "{}") as Midias;
    for (const f of faltando) {
      const item = m.itens?.find((i) => i.id === f.id);
      if (!item) continue;
      try { await invoke("videos_sessao_baixar", { projeto, arquivo: item.arquivo, hash: item.hash }); hashesConhecidos.current.add(item.hash); }
      catch (e) { setErro(`Não baixei “${item.nome}”: ${String(e)}`); }
    }
  }, [projeto]);

  // Eventos do Rust só enquanto há sessão.
  useEffect(() => {
    if (!papel) return;
    let parar: (() => void) | undefined;
    let vivo = true;
    void import("@tauri-apps/api/event").then(({ listen }) => listen<EventoDaSessao>("video-sessao", ({ payload: e }) => {
      const doc = documento.current;
      if (!doc) return;
      if (e.tipo === "mensagem") doc.receber(e.corpo);
      else if (e.tipo === "pedido") setPedidos((l) => (l.includes(e.login) ? l : [...l, e.login]));
      else if (e.tipo === "caiu") setConexao("caiu");
      else if (e.tipo === "voltou") { setConexao("ok"); doc.ola(); }
      else if (e.tipo === "midiaRecebida") void acoesAtuais.current.recarregar();
    })).then((p) => { if (vivo) parar = p; else p(); }).catch(() => undefined);
    return () => { vivo = false; parar?.(); };
  }, [papel]);

  // Ao abrir o editor: já existe sessão deste projeto (o editor remontou)? Liga de novo.
  useEffect(() => {
    let vivo = true;
    void invoke<SituacaoDaSessao>("videos_sessao_situacao").then((s) => {
      if (!vivo || !s || s.projeto !== projeto || !s.papel) return;
      const nome = s.config?.nome || s.tailscale?.login || "Eu";
      const doc = ligar(s.papel === "host", nome);
      if (s.papel === "host") void semear(doc); else doc.ola();
    }).catch(() => undefined);
    return () => { vivo = false; documento.current?.sair(); documento.current = null; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projeto]);

  /** Host: o documento começa com os arquivos do disco. */
  const semear = useCallback(async (doc: DocumentoADois) => {
    const rels = [...ARQUIVOS_FIXOS, ...((await disco.listarComposicoes(projeto).catch(() => null)) ?? [])];
    for (const rel of rels) {
      const c = await disco.lerArquivo(projeto, rel).catch(() => null);
      if (c !== null) doc.definirArquivo(rel, c);
    }
    const m = JSON.parse((await disco.lerArquivo(projeto, "midias.json")) ?? "{}") as Midias;
    for (const i of m.itens ?? []) hashesConhecidos.current.add(i.hash);
  }, [projeto]);

  // Disco → documento, a cada mudança local (o que veio de fora não volta).
  const { html, brief, midias, versaoDoPreview } = acoes.estado;
  useEffect(() => {
    const doc = documento.current;
    if (!doc || !doc.sincronizado || html === null) return;
    void (async () => {
      if (html !== doRemoto.current.get("index.html")) doc.definirArquivo("index.html", html);
      if (brief !== doRemoto.current.get("BRIEF.md")) doc.definirArquivo("BRIEF.md", brief);
      const lido = await disco.lerArquivo(projeto, "midias.json").catch(() => null);
      if (lido !== null && lido !== doRemoto.current.get("midias.json")) {
        // Convidado: mídia nova sobe para o host antes de o `midias.json` dizer que ela existe.
        if (papel === "convidado") {
          for (const i of midias.itens) {
            if (hashesConhecidos.current.has(i.hash)) continue;
            try { await invoke("videos_sessao_subir", { projeto, arquivo: i.arquivo, hash: i.hash }); hashesConhecidos.current.add(i.hash); }
            catch (e) { setErro(`Não mandei “${i.nome}” para a sessão: ${String(e)}`); }
          }
        } else for (const i of midias.itens) hashesConhecidos.current.add(i.hash);
        doc.definirArquivo("midias.json", lido);
      }
      // Composições que o agente criou ou mudou.
      for (const rel of (await disco.listarComposicoes(projeto).catch(() => null)) ?? []) {
        const c = await disco.lerArquivo(projeto, rel).catch(() => null);
        if (c !== null && c !== doRemoto.current.get(rel)) doc.definirArquivo(rel, c);
      }
    })();
  }, [html, brief, midias, versaoDoPreview, papel, projeto]);

  const iniciar = useCallback(async () => {
    setErro(null);
    try {
      const c = await invoke<string>("videos_sessao_iniciar", { projeto });
      const s = await invoke<SituacaoDaSessao>("videos_sessao_situacao");
      const doc = ligar(true, s.config?.nome || s.tailscale?.login || "Host");
      await semear(doc);
      setConvite(c);
      return c;
    } catch (e) { setErro(String(e)); return null; }
  }, [ligar, projeto, semear]);

  const parar = useCallback(async () => {
    documento.current?.soltarAgente();
    documento.current?.sair();
    documento.current = null;
    setPapel(null); setColegas([]); setAgenteCom(null); setConvite(null); setPedidos([]);
    await invoke("videos_sessao_parar").catch((e) => setErro(String(e)));
  }, []);

  const permitir = useCallback(async (login: string) => {
    await invoke("videos_sessao_permitir", { login }).catch((e) => setErro(String(e)));
    setPedidos((l) => l.filter((x) => x !== login));
  }, []);

  const colaboracao = useMemo(() => papel ? {
    colegas: colegas.map((c) => ({ nome: c.nome, cor: c.cor, tempo: c.tempo, selecionado: c.selecionado ?? undefined })),
    agenteCom,
    meuNome,
    aoMudarSelecao: (chave: string | null) => documento.current?.mudarPresenca({ selecionado: chave }),
    aoMudarTempo: (t: number) => documento.current?.mudarPresenca({ tempo: Math.round(t * 10) / 10 }),
    pegarAgente: () => documento.current?.pegarAgente(meuNome) ?? true,
    soltarAgente: () => documento.current?.soltarAgente(),
  } : undefined, [papel, colegas, agenteCom, meuNome]);

  return { papel, colegas, agenteCom, conexao, pedidos, convite, erro, iniciar, parar, permitir, colaboracao, limparErro: () => setErro(null) };
}

export type SessaoADois = ReturnType<typeof useSessaoADois>;
