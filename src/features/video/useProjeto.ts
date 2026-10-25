// Fork (Editor de vídeo): o projeto aberto no editor. Lê os arquivos, grava cada edição guardando
// antes uma versão (desfazer volta nela) e mantém o BRIEF.md em dia com a biblioteca.
// Nada roda sozinho: sem timer e sem poll; quem muda algo (timeline, chat, par) chama `recarregar`.
import { useCallback, useEffect, useRef, useState } from "react";
import * as disco from "./armazenamento";
import { atualizarReferenciasNoBrief, secaoDeReferencias } from "./modelos";
import type { LinkDeReferencia, Midia, MidiaFaltando, Midias, Papel, PastaDeReferencia, Projeto } from "./tipos";

export interface EstadoDoProjeto {
  projeto: Projeto | null;
  pasta: string | null;
  html: string | null;
  brief: string;
  midias: Midias;
  faltando: MidiaFaltando[];
  /** Sobe a cada mudança da composição: o preview recarrega. */
  versaoDoPreview: number;
  erro: string | null;
  ocupado: string | null;
  podeDesfazer: boolean;
  podeRefazer: boolean;
}

const MIDIAS_VAZIAS: Midias = { versao: 1, itens: [], links: [] };

function texto(e: unknown) { return e instanceof Error ? e.message : String(e); }

function lerJson<T>(conteudo: string | null, padrao: T): T {
  if (!conteudo) return padrao;
  try { const v = JSON.parse(conteudo); return v && typeof v === "object" ? { ...padrao, ...v } : padrao; } catch { return padrao; }
}

export function normalizarMidias(m: Partial<Midias> | null | undefined): Midias {
  return {
    versao: 1,
    itens: Array.isArray(m?.itens) ? m!.itens.filter((i) => i && typeof i.arquivo === "string") : [],
    // Fork (segurança): o `midias.json` vem do agente ou do colaborador; link que não é http(s) some.
    links: Array.isArray(m?.links) ? m!.links.filter((l) => l && typeof l.url === "string" && /^https?:\/\//i.test(l.url)) : [],
  };
}

export function useProjeto(id: string) {
  const [estado, setEstado] = useState<EstadoDoProjeto>({
    projeto: null, pasta: null, html: null, brief: "", midias: MIDIAS_VAZIAS, faltando: [], versaoDoPreview: 0,
    erro: null, ocupado: null, podeDesfazer: false, podeRefazer: false,
  });
  const atual = useRef(estado);
  atual.current = estado;
  const desfazer = useRef<number[]>([]);
  const refazer = useRef<number[]>([]);
  const pastas = useRef<Pick<PastaDeReferencia, "nome" | "descricao">[]>([]);
  /** Fork: `index.html` como está no disco (o agente pode mudar por fora, pela conversa normal). */
  const htmlDoDisco = useRef<string | null>(null);

  const mudar = useCallback((parcial: Partial<EstadoDoProjeto>) => {
    setEstado((e) => ({ ...e, ...parcial, podeDesfazer: desfazer.current.length > 0, podeRefazer: refazer.current.length > 0 }));
  }, []);

  const recarregar = useCallback(async (opcoes: { preview?: boolean } = {}) => {
    try {
      const [projeto, html, brief, midias, faltando] = await Promise.all([
        disco.lerArquivo(id, "projeto.json"), disco.lerArquivo(id, "index.html"), disco.lerArquivo(id, "BRIEF.md"),
        disco.lerArquivo(id, "midias.json"), disco.midiasFaltando(id).catch(() => [] as MidiaFaltando[]),
      ]);
      if (!projeto) throw new Error("Esse projeto não existe mais.");
      const p = JSON.parse(projeto) as Projeto;
      htmlDoDisco.current = html ?? "";
      void disco.listarPastas().then((lista) => { pastas.current = lista.filter((x) => p.pastasDeReferencia?.includes(x.id)); }).catch(() => undefined);
      mudar({
        projeto: p, html: html ?? "", brief: brief ?? "", midias: normalizarMidias(lerJson<Partial<Midias>>(midias, MIDIAS_VAZIAS)), faltando,
        erro: null, ...(opcoes.preview === false ? {} : { versaoDoPreview: atual.current.versaoDoPreview + 1 }),
      });
    } catch (e) {
      mudar({ erro: texto(e) });
    }
  }, [id, mudar]);

  useEffect(() => {
    let vivo = true;
    desfazer.current = []; refazer.current = [];
    void disco.abrirProjeto(id).then((pasta) => { if (vivo) mudar({ pasta }); }).catch(() => undefined);
    void recarregar().then(() => {
      // Fork: projeto de antes da 0.14 ou pasta de referências editada: confere as pastas que o agente lê.
      const p = atual.current.projeto;
      if (!vivo || !p || p.aDois) return;
      disco.configurarPastas(id, p.pastaDoComputador ?? null, p.pastasDeReferencia ?? []).catch((e) => {
        if (vivo && p.pastaDoComputador) mudar({ erro: `A pasta do computador deste vídeo não abriu (${texto(e)}). Escolha de novo no topo do editor.` });
      });
    });
    return () => { vivo = false; };
  }, [id, recarregar, mudar]);

  /** Grava a composição com um motivo (vira versão para desfazer). */
  const editarHtml = useCallback(async (novo: string, motivo: string, opcoes: { recarregarPreview?: boolean } = {}) => {
    if (novo === atual.current.html) return;
    // Fork (edição na mão): mudança de estilo/texto já está no preview pela ponte; recarregar piscaria.
    const versaoDoPreview = atual.current.versaoDoPreview + (opcoes.recarregarPreview === false ? 0 : 1);
    // Fork: o agente pode ter mudado o arquivo agora há pouco. Gravar por cima apagaria a edição dele.
    const noDisco = await disco.lerArquivo(id, "index.html").catch(() => null);
    if (noDisco !== null && htmlDoDisco.current !== null && noDisco !== htmlDoDisco.current) {
      await recarregar();
      mudar({ erro: "O agente mudou o vídeo enquanto você editava. Recarreguei a versão dele; faça a mudança de novo." });
      return;
    }
    const anterior = atual.current.html;
    mudar({ html: novo, versaoDoPreview });
    try {
      const n = await disco.gravarArquivo(id, "index.html", novo, motivo);
      htmlDoDisco.current = novo;
      if (n !== null) desfazer.current.push(n);
      refazer.current = [];
      mudar({ erro: null });
    } catch (e) {
      mudar({ html: anterior, erro: `Não salvei a mudança: ${texto(e)}`, versaoDoPreview: atual.current.versaoDoPreview + 1 });
    }
  }, [id, mudar, recarregar]);

  /** Fork (edição na mão): edita `index.html` ou uma sub-composição (`compositions/*.html`), com versão. */
  const editarArquivo = useCallback(async (rel: string, op: (texto: string) => string, motivo: string, opcoes: { recarregarPreview?: boolean } = {}) => {
    if (rel === "index.html") {
      const html = atual.current.html;
      if (html === null) return;
      await editarHtml(op(html), motivo, opcoes);
      return;
    }
    try {
      const texto = await disco.lerArquivo(id, rel);
      if (texto === null) throw new Error("Esse arquivo da composição não existe mais.");
      const n = await disco.gravarArquivo(id, rel, op(texto), motivo);
      if (n !== null) desfazer.current.push(n);
      refazer.current = [];
      mudar(opcoes.recarregarPreview === false ? {} : { versaoDoPreview: atual.current.versaoDoPreview + 1 });
    } catch (e) { mudar({ erro: texto(e) }); }
  }, [id, editarHtml, mudar]);

  const gravarBrief = useCallback(async (novo: string, motivo = "Briefing editado") => {
    if (novo === atual.current.brief) return;
    mudar({ brief: novo });
    try {
      const n = await disco.gravarArquivo(id, "BRIEF.md", novo, motivo);
      if (n !== null) desfazer.current.push(n);
      refazer.current = [];
      mudar({});
    } catch (e) { mudar({ erro: `Não salvei o briefing: ${texto(e)}` }); }
  }, [id, mudar]);

  const mudarProjeto = useCallback(async (parcial: Partial<Projeto>) => {
    const p = atual.current.projeto;
    if (!p) return;
    const novo = { ...p, ...parcial };
    mudar({ projeto: novo });
    try { await disco.gravarArquivo(id, "projeto.json", JSON.stringify(novo, null, 2)); }
    catch (e) { mudar({ projeto: p, erro: `Não salvei o projeto: ${texto(e)}` }); }
  }, [id, mudar]);

  /** Regrava `midias.json` (papel, nota, links) e a seção de referências do BRIEF.md. */
  const gravarMidias = useCallback(async (midias: Midias, motivo?: string) => {
    mudar({ midias });
    await disco.gravarArquivo(id, "midias.json", JSON.stringify(midias, null, 2), motivo);
    const brief = atualizarReferenciasNoBrief(atual.current.brief, secaoDeReferencias(midias.itens, midias.links, pastas.current, atual.current.projeto?.pastaDoComputador));
    if (brief !== atual.current.brief) { mudar({ brief }); await disco.gravarArquivo(id, "BRIEF.md", brief); }
  }, [id, mudar]);

  const depoisDeMudarMidias = useCallback(async () => {
    const [m, faltando] = await Promise.all([disco.lerArquivo(id, "midias.json"), disco.midiasFaltando(id).catch(() => [])]);
    const midias = normalizarMidias(lerJson<Partial<Midias>>(m, MIDIAS_VAZIAS));
    mudar({ faltando });
    await gravarMidias(midias);
    return midias;
  }, [id, gravarMidias, mudar]);

  /** Importa arquivos (caminhos do disco). Devolve os itens e os erros, um por arquivo. */
  const importar = useCallback(async (caminhos: string[], papel: Papel) => {
    const itens: Midia[] = [];
    const erros: string[] = [];
    mudar({ ocupado: caminhos.length > 1 ? `Importando ${caminhos.length} arquivos…` : "Importando…" });
    for (const caminho of caminhos) {
      try { itens.push(await disco.importarMidia(id, caminho, papel)); }
      catch (e) { erros.push(`${caminho.split(/[\\/]/).pop()}: ${texto(e)}`); }
    }
    try { await depoisDeMudarMidias(); } catch (e) { erros.push(texto(e)); }
    mudar({ ocupado: null, erro: erros.length ? erros.join(" · ") : null });
    return { itens, erros };
  }, [id, depoisDeMudarMidias, mudar]);

  const removerMidia = useCallback(async (item: string) => {
    try { await disco.removerMidia(id, item); await depoisDeMudarMidias(); }
    catch (e) { mudar({ erro: texto(e) }); }
  }, [id, depoisDeMudarMidias, mudar]);

  const mudarMidia = useCallback(async (item: string, parcial: Partial<Pick<Midia, "papel" | "nota">>) => {
    const m = atual.current.midias;
    const novo = { ...m, itens: m.itens.map((i) => (i.id === item ? { ...i, ...parcial } : i)) };
    try { await gravarMidias(novo); } catch (e) { mudar({ erro: texto(e) }); }
  }, [gravarMidias, mudar]);

  const mudarLinks = useCallback(async (links: LinkDeReferencia[]) => {
    try { await gravarMidias({ ...atual.current.midias, links }); } catch (e) { mudar({ erro: texto(e) }); }
  }, [gravarMidias, mudar]);

  const aplicarPasta = useCallback(async (pasta: PastaDeReferencia) => {
    mudar({ ocupado: `Trazendo “${pasta.nome}”…` });
    try {
      await disco.aplicarPasta(pasta.id, id);
      const p = atual.current.projeto;
      if (p && !p.pastasDeReferencia?.includes(pasta.id)) {
        const refs = [...(p.pastasDeReferencia ?? []), pasta.id];
        await disco.configurarPastas(id, p.pastaDoComputador ?? null, refs).catch(() => undefined); // Fork: o agente lê a pasta.
        await mudarProjeto({ pastasDeReferencia: refs });
      }
      if (!pastas.current.some((x) => x.nome === pasta.nome)) pastas.current = [...pastas.current, { nome: pasta.nome, descricao: pasta.descricao }];
      await depoisDeMudarMidias();
      mudar({ ocupado: null, erro: null });
    } catch (e) { mudar({ ocupado: null, erro: texto(e) }); }
  }, [id, depoisDeMudarMidias, mudarProjeto, mudar]);

  /** Fork: liga/desliga a pasta do computador e as pastas de referências deste vídeo. O Rust confere
   *  a pasta e grava o que o agente pode ler (CLAUDE.md/AGENTS.md e `.claude/settings.json`). */
  const configurarPastas = useCallback(async (mudancas: { pastaDoComputador?: string | null; pastasDeReferencia?: string[] }) => {
    const p = atual.current.projeto;
    if (!p) return false;
    const computador = mudancas.pastaDoComputador !== undefined ? mudancas.pastaDoComputador : p.pastaDoComputador ?? null;
    const refs = mudancas.pastasDeReferencia ?? p.pastasDeReferencia ?? [];
    try {
      await disco.configurarPastas(id, computador, refs);
      const { pastaDoComputador: _, ...resto } = p;
      const novo: Projeto = { ...resto, pastasDeReferencia: refs, ...(computador ? { pastaDoComputador: computador } : {}) };
      mudar({ projeto: novo });
      await disco.gravarArquivo(id, "projeto.json", JSON.stringify(novo, null, 2));
      pastas.current = (await disco.listarPastas().catch(() => [] as PastaDeReferencia[])).filter((x) => refs.includes(x.id));
      await gravarMidias(atual.current.midias);
      mudar({ erro: null });
      return true;
    } catch (e) { mudar({ erro: texto(e) }); return false; }
  }, [id, gravarMidias, mudar]);

  /** Antes de o agente editar: guarda uma versão e põe no desfazer. */
  const guardarVersao = useCallback(async (motivo: string) => {
    const n = await disco.criarVersao(id, motivo).catch(() => null);
    if (n !== null) { desfazer.current.push(n); refazer.current = []; mudar({}); }
    return n;
  }, [id, mudar]);

  const voltar = useCallback(async (de: "desfazer" | "refazer") => {
    const pilha = de === "desfazer" ? desfazer.current : refazer.current;
    const outra = de === "desfazer" ? refazer.current : desfazer.current;
    const n = pilha.pop();
    if (n === undefined) return;
    mudar({ ocupado: de === "desfazer" ? "Desfazendo…" : "Refazendo…" });
    try {
      const contrario = await disco.restaurarVersao(id, n);
      if (contrario !== null) outra.push(contrario);
      await recarregar();
      mudar({ ocupado: null });
    } catch (e) { pilha.push(n); mudar({ ocupado: null, erro: texto(e) }); }
  }, [id, recarregar, mudar]);

  /** Volta para uma versão escolhida no histórico (o estado atual vira "refazer"). */
  const voltarPara = useCallback(async (n: number) => {
    mudar({ ocupado: "Voltando para a versão…" });
    try {
      const contrario = await disco.restaurarVersao(id, n);
      if (contrario !== null) desfazer.current.push(contrario);
      refazer.current = [];
      await recarregar();
      mudar({ ocupado: null });
    } catch (e) { mudar({ ocupado: null, erro: texto(e) }); }
  }, [id, recarregar, mudar]);

  return {
    estado, recarregar, editarHtml, editarArquivo, gravarBrief, mudarProjeto, importar, removerMidia, mudarMidia, mudarLinks, aplicarPasta, configurarPastas,
    guardarVersao, desfazer: () => voltar("desfazer"), refazer: () => voltar("refazer"), voltarPara,
    limparErro: () => mudar({ erro: null }),
  };
}

export type AcoesDoProjeto = ReturnType<typeof useProjeto>;
