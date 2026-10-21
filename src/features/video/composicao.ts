// Fork (Editor de vídeo): lê e edita a composição HyperFrames direto no texto do HTML.
//
// A timeline mexe só nos atributos `data-*` de cada clipe (ou tira/insere o elemento inteiro), sem
// reformatar o arquivo: o que o agente escreveu, os scripts e os comentários ficam como estavam.
// Regras do formato conferidas na skill oficial `hyperframes-core` (references/data-attributes.md):
// `data-start` marca o clipe; `data-duration`, `data-track-index` (só faixa visual),
// `data-media-start` (corte dentro da mídia), `data-volume`, `data-hidden`.

export interface Atributo { valor: string; inicio: number; fim: number }

export interface Tag {
  tipo: "abre" | "fecha";
  nome: string;
  inicio: number;
  fim: number;
  atributos: Map<string, Atributo>;
  autoFechada: boolean;
}

const VAZIOS = new Set(["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"]);
const TEXTO_CRU = new Set(["script", "style", "textarea", "title"]);

/** Todas as tags do HTML, pulando comentários e o conteúdo de `<script>`/`<style>`. */
export function lerTags(html: string): Tag[] {
  const tags: Tag[] = [];
  let i = 0;
  const n = html.length;
  while (i < n) {
    const lt = html.indexOf("<", i);
    if (lt < 0) break;
    if (html.startsWith("<!--", lt)) { const fim = html.indexOf("-->", lt + 4); i = fim < 0 ? n : fim + 3; continue; }
    if (html[lt + 1] === "!" || html[lt + 1] === "?") { const fim = html.indexOf(">", lt); i = fim < 0 ? n : fim + 1; continue; }
    const fecha = html[lt + 1] === "/";
    let j = lt + (fecha ? 2 : 1);
    const comecoDoNome = j;
    while (j < n && /[A-Za-z0-9:-]/.test(html[j])) j++;
    if (j === comecoDoNome) { i = lt + 1; continue; }
    const nome = html.slice(comecoDoNome, j).toLowerCase();
    const atributos = new Map<string, Atributo>();
    let autoFechada = false;
    while (j < n && html[j] !== ">") {
      if (/\s/.test(html[j])) { j++; continue; }
      if (html[j] === "/" ) { autoFechada = html[j + 1] === ">"; j++; continue; }
      const inicioDoAtributo = j;
      while (j < n && !/[\s=>/]/.test(html[j])) j++;
      const nomeDoAtributo = html.slice(inicioDoAtributo, j).toLowerCase();
      let valor = "";
      let k = j;
      while (k < n && /\s/.test(html[k])) k++;
      if (html[k] === "=") {
        k++;
        while (k < n && /\s/.test(html[k])) k++;
        const aspas = html[k];
        if (aspas === '"' || aspas === "'") {
          const fimDoValor = html.indexOf(aspas, k + 1);
          valor = html.slice(k + 1, fimDoValor < 0 ? n : fimDoValor);
          j = fimDoValor < 0 ? n : fimDoValor + 1;
        } else {
          const comeco = k;
          while (k < n && !/[\s>]/.test(html[k])) k++;
          valor = html.slice(comeco, k);
          j = k;
        }
      }
      if (nomeDoAtributo && !atributos.has(nomeDoAtributo)) atributos.set(nomeDoAtributo, { valor: decodificar(valor), inicio: inicioDoAtributo, fim: j });
      if (j === inicioDoAtributo) j++;
    }
    const fim = Math.min(n, j + 1);
    tags.push({ tipo: fecha ? "fecha" : "abre", nome, inicio: lt, fim, atributos, autoFechada: autoFechada || VAZIOS.has(nome) });
    i = fim;
    if (!fecha && TEXTO_CRU.has(nome)) {
      const fimDoCru = html.toLowerCase().indexOf(`</${nome}`, i);
      if (fimDoCru < 0) break;
      i = fimDoCru;
    }
  }
  return tags;
}

function decodificar(valor: string) {
  return valor.replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
}
function codificar(valor: string) {
  return valor.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Fim do elemento aberto por `tags[indice]` (depois da tag de fechamento), ou o fim da própria tag. */
export function fimDoElemento(tags: Tag[], indice: number): number {
  const abre = tags[indice];
  if (abre.autoFechada) return abre.fim;
  let profundidade = 0;
  for (let k = indice; k < tags.length; k++) {
    const t = tags[k];
    if (t.nome !== abre.nome) continue;
    if (t.tipo === "abre" && !t.autoFechada) profundidade++;
    else if (t.tipo === "fecha") { profundidade--; if (profundidade === 0) return t.fim; }
  }
  return abre.fim;
}

export type TipoDeClipe = "video" | "audio" | "imagem" | "composicao" | "texto" | "elemento";

export interface Clipe {
  /** `id` do elemento, ou `@<ordem>` quando não tem (a timeline põe um id na primeira edição). */
  chave: string;
  id?: string;
  ordem: number;
  tag: string;
  tipo: TipoDeClipe;
  inicio: number;
  /** `undefined` = sem duração escrita (vídeo/áudio usam o tamanho da mídia; imagem, 3 s). */
  duracao?: number;
  faixa: number;
  midiaInicio: number;
  volume?: number;
  oculto: boolean;
  mudo: boolean;
  src?: string;
  rotulo: string;
  /** Início escrito como número (dá para arrastar). Referências como `intro.end` não. */
  editavel: boolean;
  /** Dentro de outro clipe (o pai limita quando ele aparece). */
  aninhado: boolean;
  posicao: { inicio: number; fim: number; fimDoElemento: number };
}

export interface Raiz {
  id: string;
  largura: number;
  altura: number;
  duracao?: number;
  fps: number;
  posicao: { inicio: number; fim: number; fimDoElemento: number };
}

export interface Composicao { raiz: Raiz | null; clipes: Clipe[] }

function numero(texto: string | undefined) {
  if (texto === undefined || texto.trim() === "") return undefined;
  const v = Number(texto);
  return Number.isFinite(v) ? v : undefined;
}

function tipoDoClipe(tag: Tag): TipoDeClipe {
  if (tag.nome === "video") return "video";
  if (tag.nome === "audio") return "audio";
  if (tag.nome === "img") return "imagem";
  if (tag.atributos.has("data-composition-src") || tag.atributos.has("data-composition-id")) return "composicao";
  if (/^(h[1-6]|p|span)$/.test(tag.nome)) return "texto";
  return "elemento";
}

function nomeDoArquivo(src: string) { return src.split(/[\\/]/).pop() ?? src; }

function textoLogoDepois(html: string, tags: Tag[], indice: number, fim: number) {
  const pedaco = html.slice(tags[indice].fim, Math.min(fim, tags[indice].fim + 600));
  const semTags = pedaco.replace(/<script[\s\S]*?<\/script>/gi, " ").replace(/<style[\s\S]*?<\/style>/gi, " ").replace(/<[^>]*>/g, " ");
  return decodificar(semTags.replace(/\s+/g, " ").trim()).slice(0, 40);
}

/** Root (`data-composition-id` mais de fora) e todos os clipes (`data-start`) da composição. */
export function lerComposicao(html: string): Composicao {
  const tags = lerTags(html);
  let raiz: Raiz | null = null;
  let indiceDaRaiz = -1;
  for (let k = 0; k < tags.length; k++) {
    const t = tags[k];
    if (t.tipo !== "abre" || !t.atributos.has("data-composition-id")) continue;
    // Um `data-root="true"` explícito vence; senão o primeiro (mais de fora).
    if (indiceDaRaiz >= 0 && t.atributos.get("data-root")?.valor !== "true") continue;
    indiceDaRaiz = k;
    if (t.atributos.get("data-root")?.valor === "true") break;
  }
  if (indiceDaRaiz >= 0) {
    const t = tags[indiceDaRaiz];
    const a = (nome: string) => t.atributos.get(nome)?.valor;
    raiz = {
      id: a("data-composition-id") ?? "main",
      largura: numero(a("data-width")) ?? 1920,
      altura: numero(a("data-height")) ?? 1080,
      duracao: numero(a("data-duration")),
      fps: numero(a("data-fps")) ?? 30,
      posicao: { inicio: t.inicio, fim: t.fim, fimDoElemento: fimDoElemento(tags, indiceDaRaiz) },
    };
  }
  const clipes: Clipe[] = [];
  const abertos: { fim: number }[] = [];
  let ordem = 0;
  for (let k = 0; k < tags.length; k++) {
    const t = tags[k];
    if (t.tipo !== "abre" || k === indiceDaRaiz || !t.atributos.has("data-start")) continue;
    if (raiz && (t.inicio < raiz.posicao.fim || t.inicio >= raiz.posicao.fimDoElemento)) continue;
    const fim = fimDoElemento(tags, k);
    while (abertos.length && abertos[abertos.length - 1].fim <= t.inicio) abertos.pop();
    const aninhado = abertos.length > 0;
    abertos.push({ fim });
    const a = (nome: string) => t.atributos.get(nome)?.valor;
    const inicio = numero(a("data-start"));
    const id = a("id")?.trim() || undefined;
    const src = a("src") ?? a("data-composition-src");
    const tipo = tipoDoClipe(t);
    const texto = tipo === "texto" || tipo === "elemento" ? textoLogoDepois(html, tags, k, fim) : "";
    clipes.push({
      chave: id ?? `@${ordem}`,
      id,
      ordem,
      tag: t.nome,
      tipo,
      inicio: inicio ?? 0,
      duracao: numero(a("data-duration")),
      faixa: Math.max(0, Math.round(numero(a("data-track-index")) ?? ordem)),
      midiaInicio: numero(a("data-media-start")) ?? 0,
      volume: numero(a("data-volume")),
      oculto: t.atributos.has("data-hidden") && a("data-hidden") !== "false",
      mudo: t.atributos.has("muted"),
      src,
      rotulo: texto || (src ? nomeDoArquivo(src) : id ?? `${t.nome} ${ordem + 1}`),
      editavel: inicio !== undefined,
      aninhado,
      posicao: { inicio: t.inicio, fim: t.fim, fimDoElemento: fim },
    });
    ordem++;
  }
  return { raiz, clipes };
}

/** Duração que a timeline desenha: a escrita, ou 3 s para imagem/sem duração. */
export function duracaoVisivel(c: Clipe, duracoesDasMidias: Record<string, number> = {}) {
  if (c.duracao !== undefined) return Math.max(0, c.duracao);
  const daMidia = c.src ? duracoesDasMidias[c.src] : undefined;
  if (daMidia !== undefined) return Math.max(0.1, daMidia - c.midiaInicio);
  return 3;
}

/** Duração total: a do root, ou o fim do último clipe. */
export function duracaoTotal(comp: Composicao, duracoesDasMidias: Record<string, number> = {}) {
  if (comp.raiz?.duracao !== undefined) return comp.raiz.duracao;
  return comp.clipes.reduce((m, c) => Math.max(m, c.inicio + duracaoVisivel(c, duracoesDasMidias)), 0) || 10;
}

// ——— edições (cada uma devolve o HTML novo) ———

interface Troca { inicio: number; fim: number; texto: string }

function aplicar(html: string, trocas: Troca[]) {
  let saida = html;
  for (const t of [...trocas].sort((a, b) => b.inicio - a.inicio)) saida = saida.slice(0, t.inicio) + t.texto + saida.slice(t.fim);
  return saida;
}

/** Troca, põe ou tira (`null`) atributos de uma tag, sem tocar no resto dela. */
function trocasDeAtributos(html: string, tag: Tag, mudancas: Record<string, string | null>): Troca[] {
  const trocas: Troca[] = [];
  const novos: string[] = [];
  for (const [nome, valor] of Object.entries(mudancas)) {
    const atual = tag.atributos.get(nome);
    if (atual) {
      if (valor === null) {
        let inicio = atual.inicio;
        while (inicio > tag.inicio && /\s/.test(html[inicio - 1])) inicio--;
        trocas.push({ inicio, fim: atual.fim, texto: "" });
      } else if (valor === "" && !html.slice(atual.inicio, atual.fim).includes("=")) {
        // Atributo booleano (`muted`) já presente: fica.
      } else {
        trocas.push({ inicio: atual.inicio, fim: atual.fim, texto: valor === "" && nome === "muted" ? nome : `${nome}="${codificar(valor)}"` });
      }
    } else if (valor !== null) {
      novos.push(valor === "" && (nome === "muted" || nome === "playsinline") ? nome : `${nome}="${codificar(valor)}"`);
    }
  }
  if (novos.length) {
    let ponto = tag.fim - 1;
    if (html[ponto - 1] === "/") ponto--;
    while (ponto > tag.inicio && /\s/.test(html[ponto - 1])) ponto--;
    trocas.push({ inicio: ponto, fim: ponto, texto: ` ${novos.join(" ")}` });
  }
  return trocas;
}

function tagDoClipe(html: string, clipe: Clipe) {
  const tag = lerTags(html).find((t) => t.tipo === "abre" && t.inicio === clipe.posicao.inicio);
  if (!tag) throw new Error("O clipe mudou de lugar no arquivo. Recarregue a composição.");
  return tag;
}

function achar(html: string, chave: string) {
  const comp = lerComposicao(html);
  const clipe = comp.clipes.find((c) => c.chave === chave);
  if (!clipe) throw new Error("Não achei esse clipe na composição.");
  return { comp, clipe };
}

/** Número curto no HTML (`2.5`, não `2.4999999`). */
export function seg(v: number) { return String(Math.round(Math.max(0, v) * 1000) / 1000); }

function idsUsados(html: string) {
  return new Set(lerTags(html).flatMap((t) => (t.atributos.get("id")?.valor ? [t.atributos.get("id")!.valor] : [])));
}

export function novoId(html: string, base: string) {
  const usados = idsUsados(html);
  const limpo = base.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 32) || "clipe";
  const inicial = /^[a-z]/.test(limpo) ? limpo : `c-${limpo}`;
  if (!usados.has(inicial)) return inicial;
  for (let n = 2; ; n++) if (!usados.has(`${inicial}-${n}`)) return `${inicial}-${n}`;
}

/** Root cresce quando um clipe passa do fim (senão o render corta o clipe). */
function trocasDaDuracaoDaRaiz(html: string, comp: Composicao, novoFim: number): Troca[] {
  if (!comp.raiz || comp.raiz.duracao === undefined || novoFim <= comp.raiz.duracao + 1e-6) return [];
  const tag = lerTags(html).find((t) => t.tipo === "abre" && t.inicio === comp.raiz!.posicao.inicio);
  return tag ? trocasDeAtributos(html, tag, { "data-duration": seg(Math.ceil(novoFim * 10) / 10) }) : [];
}

/** Muda atributos de um clipe (e põe `id` se ele não tinha). Base das outras edições. */
export function mudarClipe(html: string, chave: string, mudancas: Record<string, string | null>, novoFim?: number) {
  const { comp, clipe } = achar(html, chave);
  const tag = tagDoClipe(html, clipe);
  const extra: Record<string, string | null> = { ...mudancas };
  if (!clipe.id && !("id" in extra)) extra.id = novoId(html, clipe.rotulo || clipe.tag);
  return aplicar(html, [...trocasDeAtributos(html, tag, extra), ...(novoFim !== undefined ? trocasDaDuracaoDaRaiz(html, comp, novoFim) : [])]);
}

export function moverClipe(html: string, chave: string, inicio: number, faixa?: number, duracoesDasMidias: Record<string, number> = {}) {
  const { clipe } = achar(html, chave);
  if (!clipe.editavel) throw new Error("O início deste clipe depende de outro; peça ao agente para mudar.");
  const mudancas: Record<string, string | null> = { "data-start": seg(inicio) };
  if (faixa !== undefined) mudancas["data-track-index"] = String(Math.max(0, Math.round(faixa)));
  return mudarClipe(html, chave, mudancas, inicio + duracaoVisivel(clipe, duracoesDasMidias));
}

/** Corta a ponta esquerda (`lado: "inicio"`) ou direita de um clipe até o tempo `t` da timeline. */
export function aparar(html: string, chave: string, lado: "inicio" | "fim", t: number, duracoesDasMidias: Record<string, number> = {}) {
  const { clipe } = achar(html, chave);
  const dur = duracaoVisivel(clipe, duracoesDasMidias);
  const fim = clipe.inicio + dur;
  const minimo = 0.1;
  if (lado === "fim") {
    const novoFim = Math.max(clipe.inicio + minimo, t);
    return mudarClipe(html, chave, { "data-duration": seg(novoFim - clipe.inicio) }, novoFim);
  }
  const novoInicio = Math.min(Math.max(0, t), fim - minimo);
  const delta = novoInicio - clipe.inicio;
  const mudancas: Record<string, string | null> = { "data-start": seg(novoInicio), "data-duration": seg(fim - novoInicio) };
  if (clipe.tipo === "video" || clipe.tipo === "audio") mudancas["data-media-start"] = seg(Math.max(0, clipe.midiaInicio + delta));
  return mudarClipe(html, chave, mudancas);
}

/** Divide um clipe no tempo `t`: o original vai até `t`, a cópia começa em `t` (mídia continua de onde parou). */
export function dividir(html: string, chave: string, t: number, duracoesDasMidias: Record<string, number> = {}) {
  const { clipe } = achar(html, chave);
  const dur = duracaoVisivel(clipe, duracoesDasMidias);
  if (!(t > clipe.inicio + 0.05 && t < clipe.inicio + dur - 0.05)) throw new Error("Ponha a agulha dentro do clipe para dividir.");
  let atual = clipe.id ? html : mudarClipe(html, chave, {});
  const { clipe: c } = achar(atual, clipe.id ?? lerComposicao(atual).clipes[clipe.ordem].chave);
  const elemento = atual.slice(c.posicao.inicio, c.posicao.fimDoElemento);
  const idNovo = novoId(atual, `${c.id}-b`);
  const tagsDaCopia = lerTags(elemento);
  const abre = tagsDaCopia[0];
  const mudancasDaCopia: Record<string, string | null> = { id: idNovo, "data-start": seg(t), "data-duration": seg(c.inicio + dur - t) };
  if (c.tipo === "video" || c.tipo === "audio") mudancasDaCopia["data-media-start"] = seg(c.midiaInicio + (t - c.inicio));
  // Ids dentro da cópia também mudam (o HyperFrames pede id único no documento montado).
  const trocasInternas: Troca[] = tagsDaCopia.slice(1).flatMap((tg) => {
    const id = tg.atributos.get("id");
    return id ? trocasDeAtributos(elemento, tg, { id: `${id.valor}-b` }) : [];
  });
  const copia = aplicar(elemento, [...trocasDeAtributos(elemento, abre, mudancasDaCopia), ...trocasInternas]);
  const tagOriginal = tagDoClipe(atual, c);
  atual = aplicar(atual, [
    ...trocasDeAtributos(atual, tagOriginal, { "data-duration": seg(t - c.inicio) }),
    { inicio: c.posicao.fimDoElemento, fim: c.posicao.fimDoElemento, texto: `\n${indentacao(atual, c.posicao.inicio)}${copia}` },
  ]);
  return atual;
}

function indentacao(html: string, posicao: number) {
  const comecoDaLinha = html.lastIndexOf("\n", posicao - 1) + 1;
  const m = /^[ \t]*/.exec(html.slice(comecoDaLinha, posicao));
  return m ? m[0] : "";
}

export function duplicar(html: string, chave: string, duracoesDasMidias: Record<string, number> = {}) {
  const { clipe } = achar(html, chave);
  const atual = clipe.id ? html : mudarClipe(html, chave, {});
  const { clipe: c, comp } = achar(atual, clipe.id ?? lerComposicao(atual).clipes[clipe.ordem].chave);
  const dur = duracaoVisivel(c, duracoesDasMidias);
  const elemento = atual.slice(c.posicao.inicio, c.posicao.fimDoElemento);
  const tags = lerTags(elemento);
  const idNovo = novoId(atual, `${c.id}-copia`);
  const copia = aplicar(elemento, [
    ...trocasDeAtributos(elemento, tags[0], { id: idNovo, "data-start": seg(c.inicio + dur) }),
    ...tags.slice(1).flatMap((tg) => { const id = tg.atributos.get("id"); return id ? trocasDeAtributos(elemento, tg, { id: `${id.valor}-copia` }) : []; }),
  ]);
  return aplicar(atual, [
    { inicio: c.posicao.fimDoElemento, fim: c.posicao.fimDoElemento, texto: `\n${indentacao(atual, c.posicao.inicio)}${copia}` },
    ...trocasDaDuracaoDaRaiz(atual, comp, c.inicio + 2 * dur),
  ]);
}

/** Tira o elemento inteiro (a versão anterior fica no histórico). */
export function apagar(html: string, chave: string) {
  const { clipe } = achar(html, chave);
  let inicio = clipe.posicao.inicio;
  while (inicio > 0 && (html[inicio - 1] === " " || html[inicio - 1] === "\t")) inicio--;
  let fim = clipe.posicao.fimDoElemento;
  if (html[fim] === "\n") fim++;
  else if (inicio > 0 && html[inicio - 1] === "\n") inicio--;
  return html.slice(0, inicio) + html.slice(fim);
}

/** Esconder sem apagar (`data-hidden`, vale no preview e no render). */
export function alternarOculto(html: string, chave: string) {
  const { clipe } = achar(html, chave);
  return mudarClipe(html, chave, { "data-hidden": clipe.oculto ? null : "true" });
}

export function definirDuracaoTotal(html: string, segundos: number) {
  const comp = lerComposicao(html);
  if (!comp.raiz) throw new Error("A composição não tem root (`data-composition-id`).");
  const tag = lerTags(html).find((t) => t.tipo === "abre" && t.inicio === comp.raiz!.posicao.inicio)!;
  return aplicar(html, trocasDeAtributos(html, tag, { "data-duration": seg(Math.max(0.5, segundos)) }));
}

export interface MidiaParaInserir { arquivo: string; tipo: string; nome: string; duracao?: number }

/** Põe uma mídia da biblioteca na timeline (dentro do root, no fim). */
export function inserirMidia(html: string, midia: MidiaParaInserir, inicio: number, faixa: number) {
  const comp = lerComposicao(html);
  if (!comp.raiz) throw new Error("A composição não tem root (`data-composition-id`).");
  const id = novoId(html, midia.nome.replace(/\.[^.]+$/, ""));
  const src = codificar(midia.arquivo);
  const comum = `data-start="${seg(inicio)}" data-track-index="${faixa}"`;
  let elemento: string;
  let duracao: number;
  if (midia.tipo === "video") {
    duracao = midia.duracao ?? 5;
    elemento = `<video id="${id}" src="${src}" ${comum} data-duration="${seg(duracao)}" data-media-start="0" playsinline data-has-audio="true" style="position:absolute;inset:0;width:100%;height:100%;object-fit:cover"></video>`;
  } else if (midia.tipo === "audio") {
    duracao = midia.duracao ?? 10;
    elemento = `<audio id="${id}" src="${src}" ${comum} data-duration="${seg(duracao)}" data-media-start="0" data-volume="1"></audio>`;
  } else if (midia.tipo === "imagem") {
    duracao = 3;
    elemento = `<img id="${id}" class="clip" src="${src}" ${comum} data-duration="3" alt="" style="width:100%;height:100%;object-fit:cover" />`;
  } else {
    throw new Error("Esse tipo de arquivo não entra direto na timeline. Peça ao agente para usar (ex.: fonte, animação).");
  }
  return pendurarNaRaiz(html, comp, elemento, inicio + duracao);
}

/** Legenda simples na parte de baixo do quadro. */
export function inserirTexto(html: string, texto: string, inicio: number, duracao: number, faixa: number) {
  const comp = lerComposicao(html);
  if (!comp.raiz) throw new Error("A composição não tem root (`data-composition-id`).");
  const id = novoId(html, `texto-${texto}`);
  const tamanho = Math.round(Math.min(comp.raiz.largura, comp.raiz.altura) * 0.05);
  // Um elemento só (sem filhos): o lint do HyperFrames pede sub-composição para clipe com filhos.
  const elemento = `<p id="${id}" class="clip" data-start="${seg(inicio)}" data-duration="${seg(duracao)}" data-track-index="${faixa}" style="box-sizing:border-box;margin:0;display:flex;align-items:flex-end;justify-content:center;padding:0 8% 8%;color:#fff;font:600 ${tamanho}px/1.2 system-ui,sans-serif;text-align:center;text-shadow:0 2px 14px rgba(0,0,0,0.85)">${codificar(texto)}</p>`;
  return pendurarNaRaiz(html, comp, elemento, inicio + duracao);
}

function pendurarNaRaiz(html: string, comp: Composicao, elemento: string, fim: number) {
  const raiz = comp.raiz!;
  const tags = lerTags(html);
  const fechamento = tags.find((t) => t.tipo === "fecha" && t.fim === raiz.posicao.fimDoElemento);
  const ponto = fechamento ? fechamento.inicio : raiz.posicao.fimDoElemento;
  const recuo = indentacao(html, raiz.posicao.inicio);
  let antes = ponto;
  while (antes > 0 && (html[antes - 1] === " " || html[antes - 1] === "\t")) antes--;
  const trocas: Troca[] = [{ inicio: antes, fim: ponto, texto: `${recuo}  ${elemento}\n${recuo}` }, ...trocasDaDuracaoDaRaiz(html, comp, fim)];
  return aplicar(html, trocas);
}
