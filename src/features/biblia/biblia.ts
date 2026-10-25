/**
 * Fork (Bíblia do dia): o texto da Bíblia Ave Maria (arquivo de `src-tauri/src/biblia.rs`) dividido
 * em passagens curtas, por trilha (Evangelhos, Salmos, Sabedoria...). Tudo aqui é puro e
 * determinístico: o mesmo arquivo dá sempre as mesmas passagens, na mesma ordem.
 *
 * O texto é mostrado como está no arquivo; só saem as marcas de nota de rodapé (`*`) e os hífens
 * invisíveis da diagramação. Capítulo com numeração quebrada no arquivo (versículos grudados, como
 * em algumas genealogias) fica fora das passagens, para nunca mostrar versículo trocado.
 */

export type Trilha = "evangelhos" | "salmos" | "sabedoria" | "cartas" | "antigo";

export interface Versiculo { n: number; texto: string }
export interface Capitulo { n: number; versiculos: Versiculo[] }
export interface Livro { indice: number; nome: string; capitulos: Capitulo[] }
export interface Biblia { livros: Livro[] }

export interface Passagem {
  /** `<livro>:<capítulo>:<de>-<até>` (livro pela posição no arquivo). */
  id: string;
  trilha: Trilha;
  livro: number;
  capitulo: number;
  de: number;
  ate: number;
}

export const TRILHAS: Trilha[] = ["evangelhos", "salmos", "sabedoria", "cartas", "antigo"];

export const SOBRE_A_TRILHA: Record<Trilha, { nome: string; rotulo: string; emoji: string; dica: string }> = {
  evangelhos: { nome: "Evangelhos", rotulo: "Evangelho", emoji: "✝️", dica: "Mateus, Marcos, Lucas e João, em ordem." },
  salmos: { nome: "Salmos", rotulo: "Salmo", emoji: "🎶", dica: "Um salmo por dia (os longos, em partes)." },
  sabedoria: { nome: "Provérbios e Sabedoria", rotulo: "Sabedoria", emoji: "🕯️", dica: "Provérbios, Eclesiastes, Sabedoria, Eclesiástico, Jó e Cântico." },
  cartas: { nome: "Atos, cartas e Apocalipse", rotulo: "Novo Testamento", emoji: "✉️", dica: "Atos dos Apóstolos, as cartas e o Apocalipse." },
  antigo: { nome: "Antigo Testamento", rotulo: "Antigo Testamento", emoji: "📜", dica: "Do Gênesis a Malaquias (história e profetas)." },
};

/** Livros de cada trilha, na ordem de leitura (nomes como estão no arquivo). */
export const LIVROS_DA_TRILHA: Record<Trilha, string[]> = {
  evangelhos: ["São Mateus", "São Marcos", "São Lucas", "São João"],
  salmos: ["Salmos"],
  sabedoria: ["Provérbios", "Eclesiastes", "Sabedoria", "Eclesiástico", "Jó", "Cântico dos Cânticos"],
  cartas: [
    "Atos dos Apóstolos", "Romanos", "I Coríntios", "II Coríntios", "Gálatas", "Efésios", "Filipenses", "Colossenses",
    "I Tessalonicenses", "II Tessalonicenses", "I Timóteo", "II Timóteo", "Tito", "Filêmon", "Hebreus", "São Tiago",
    "I São Pedro", "II São Pedro", "I São João", "II São João", "III São João", "São Judas", "Apocalipse",
  ],
  antigo: [
    "Gênesis", "Êxodo", "Levítico", "Números", "Deuteronômio", "Josué", "Juízes", "Rute", "I Samuel", "II Samuel",
    "I Reis", "II Reis", "I Crônicas", "II Crônicas", "Esdras", "Neemias", "Tobias", "Judite", "Ester", "I Macabeus",
    "II Macabeus", "Isaías", "Jeremias", "Lamentações", "Baruc", "Ezequiel", "Daniel", "Oséias", "Joel", "Amós", "Abdias",
    "Jonas", "Miquéias", "Naum", "Habacuc", "Sofonias", "Ageu", "Zacarias", "Malaquias",
  ],
};

/** Tamanho das passagens (em versículos) por trilha: corta no fim de frase entre o mínimo e o máximo. */
const TAMANHO: Record<Trilha, { min: number; max: number }> = {
  evangelhos: { min: 6, max: 12 },
  salmos: { min: 6, max: 12 },
  sabedoria: { min: 4, max: 8 },
  cartas: { min: 5, max: 10 },
  antigo: { min: 8, max: 15 },
};
/** Salmo até este tamanho vem inteiro. */
const SALMO_INTEIRO = 20;

// ---- Ler o arquivo ----

interface ArquivoBruto { antigoTestamento?: unknown; novoTestamento?: unknown }

/** Texto como vai para a tela: sem `*` de nota, sem hífen invisível, espaços simples. */
export function limparTexto(texto: string): string {
  return texto.replace(/­/g, "").replace(/\*/g, "").replace(/\s+/g, " ").trim();
}

/** Lê o JSON do arquivo (formato `{antigoTestamento, novoTestamento}`) e confere que estão os 73 livros. */
export function lerBiblia(bruto: string): Biblia {
  const dado = JSON.parse(bruto) as ArquivoBruto;
  const grupos = [dado.antigoTestamento, dado.novoTestamento];
  if (!grupos.every(Array.isArray)) throw new Error("O arquivo da Bíblia não está no formato esperado");
  const livros: Livro[] = [];
  for (const grupo of grupos as unknown[][]) {
    for (const l of grupo) {
      const livro = l as { nome?: unknown; capitulos?: unknown };
      if (typeof livro.nome !== "string" || !Array.isArray(livro.capitulos)) throw new Error("Livro sem nome ou capítulos no arquivo da Bíblia");
      livros.push({
        indice: livros.length,
        nome: livro.nome,
        capitulos: (livro.capitulos as { capitulo: number; versiculos: { versiculo: number; texto: string }[] }[]).map((c) => ({
          n: c.capitulo,
          versiculos: c.versiculos.map((v) => ({ n: v.versiculo, texto: v.texto })),
        })),
      });
    }
  }
  const nomes = new Set(livros.map((l) => l.nome));
  const faltando = TRILHAS.flatMap((t) => LIVROS_DA_TRILHA[t]).filter((n) => !nomes.has(n));
  if (livros.length !== 73 || faltando.length) throw new Error(`O arquivo da Bíblia está incompleto${faltando.length ? ` (faltam ${faltando.slice(0, 3).join(", ")})` : ""}`);
  return { livros };
}

/**
 * Capítulo que não dá para mostrar com segurança: versículo vazio (`[]`), número de outro versículo
 * no meio do texto (dois versículos grudados e a numeração deslocada) ou numeração fora de ordem.
 */
export function capituloComDefeito(cap: Capitulo): boolean {
  return cap.versiculos.some((v, i) => {
    if (v.n !== i + 1) return true;
    const t = v.texto.trim();
    if (/^[[\]\s\-–.]*$/.test(t) || t.length > 800) return true;
    for (const m of t.matchAll(/(?:^|\s)(\d{1,3})(?:-\d{1,3})?\s/g)) {
      const n = Number(m[1]);
      if (n > v.n && n <= v.n + 40) return true;
    }
    return false;
  });
}

const FIM_DE_FRASE = /[.!?…][”’»"')\]]*$/;

/** Divide um capítulo em pedaços que terminam em fim de frase (ou no máximo, se a frase não acabar). */
export function dividirCapitulo(cap: Capitulo, min: number, max: number): [number, number][] {
  const vs = cap.versiculos;
  const partes: [number, number][] = [];
  let inicio = 0;
  for (let i = 0; i < vs.length; i++) {
    const tamanho = i - inicio + 1;
    const fim = FIM_DE_FRASE.test(limparTexto(vs[i].texto));
    if ((tamanho >= min && fim) || tamanho >= max + 6 || (tamanho >= max && /[;:]$/.test(limparTexto(vs[i].texto)))) {
      partes.push([vs[inicio].n, vs[i].n]);
      inicio = i + 1;
    }
  }
  if (inicio < vs.length) {
    const resto = vs.length - inicio;
    const ultima = partes[partes.length - 1];
    // Sobra pequena junta na anterior (não fica uma passagem de 1 ou 2 versículos solta).
    if (ultima && resto <= Math.max(1, Math.floor(min / 2)) && ultima[1] - ultima[0] + 1 + resto <= max + 6) ultima[1] = vs[vs.length - 1].n;
    else partes.push([vs[inicio].n, vs[vs.length - 1].n]);
  }
  return partes;
}

/** Todas as passagens da trilha, na ordem de leitura. */
export function passagensDaTrilha(biblia: Biblia, trilha: Trilha): Passagem[] {
  const { min, max } = TAMANHO[trilha];
  const lista: Passagem[] = [];
  for (const nome of LIVROS_DA_TRILHA[trilha]) {
    const livro = biblia.livros.find((l) => l.nome === nome);
    if (!livro) continue;
    for (const cap of livro.capitulos) {
      if (!cap.versiculos.length || capituloComDefeito(cap)) continue;
      const partes = trilha === "salmos" && cap.versiculos.length <= SALMO_INTEIRO
        ? [[1, cap.versiculos.length] as [number, number]]
        : dividirCapitulo(cap, min, max);
      for (const [de, ate] of partes) lista.push({ id: `${livro.indice}:${cap.n}:${de}-${ate}`, trilha, livro: livro.indice, capitulo: cap.n, de, ate });
    }
  }
  return lista;
}

const cache = new WeakMap<Biblia, Map<Trilha, Passagem[]>>();
export function passagens(biblia: Biblia, trilha: Trilha): Passagem[] {
  let porTrilha = cache.get(biblia);
  if (!porTrilha) { porTrilha = new Map(); cache.set(biblia, porTrilha); }
  let lista = porTrilha.get(trilha);
  if (!lista) { lista = passagensDaTrilha(biblia, trilha); porTrilha.set(trilha, lista); }
  return lista;
}

/** Versículos da passagem, já limpos para a tela. */
export function versiculosDa(biblia: Biblia, p: Passagem): Versiculo[] {
  const cap = biblia.livros[p.livro]?.capitulos.find((c) => c.n === p.capitulo);
  return (cap?.versiculos ?? []).filter((v) => v.n >= p.de && v.n <= p.ate).map((v) => ({ n: v.n, texto: limparTexto(v.texto) }));
}

/** "São Mateus 5, 1-12", "Salmo 22" (salmo inteiro) ou "Provérbios 3, 5". */
export function referencia(biblia: Biblia, p: Passagem): string {
  const livro = biblia.livros[p.livro];
  const total = livro?.capitulos.find((c) => c.n === p.capitulo)?.versiculos.length ?? 0;
  const nome = livro?.nome === "Salmos" ? "Salmo" : livro?.nome ?? "?";
  if (livro?.nome === "Salmos" && p.de === 1 && p.ate === total) return `${nome} ${p.capitulo}`;
  return `${nome} ${p.capitulo}, ${p.de === p.ate ? p.de : `${p.de}-${p.ate}`}`;
}

// ---- Onde parou ----

/** Próximo ponto a ler numa trilha. `ciclo` conta as vezes que a trilha recomeçou do início. */
export interface Marcador { livro: number; capitulo: number; versiculo: number; ciclo: number }

function antes(a: { livro: number; capitulo: number; versiculo: number }, b: { livro: number; capitulo: number; versiculo: number }, ordem: Map<number, number>) {
  const la = ordem.get(a.livro) ?? -1, lb = ordem.get(b.livro) ?? -1;
  return la !== lb ? la < lb : a.capitulo !== b.capitulo ? a.capitulo < b.capitulo : a.versiculo < b.versiculo;
}

function ordemDosLivros(biblia: Biblia, trilha: Trilha) {
  return new Map(LIVROS_DA_TRILHA[trilha].map((nome, i) => [biblia.livros.find((l) => l.nome === nome)?.indice ?? -1, i]));
}

/** A passagem a ler agora: a primeira que começa no marcador ou depois dele. */
export function passagemAtual(biblia: Biblia, trilha: Trilha, marcador?: Marcador): { passagem: Passagem; indice: number; total: number } | null {
  const lista = passagens(biblia, trilha);
  if (!lista.length) return null;
  if (!marcador) return { passagem: lista[0], indice: 0, total: lista.length };
  const ordem = ordemDosLivros(biblia, trilha);
  const i = lista.findIndex((p) => !antes({ livro: p.livro, capitulo: p.capitulo, versiculo: p.de }, marcador, ordem));
  return i < 0 ? { passagem: lista[0], indice: 0, total: lista.length } : { passagem: lista[i], indice: i, total: lista.length };
}

/** Marcador depois de ler `p`: a passagem seguinte, ou o começo de novo (ciclo + 1) no fim da trilha. */
export function marcadorDepois(biblia: Biblia, p: Passagem, marcador?: Marcador): Marcador {
  const lista = passagens(biblia, p.trilha);
  const i = lista.findIndex((x) => x.id === p.id);
  const ciclo = marcador?.ciclo ?? 0;
  const proxima = i >= 0 ? lista[i + 1] : undefined;
  if (!proxima) return { livro: lista[0].livro, capitulo: lista[0].capitulo, versiculo: lista[0].de, ciclo: ciclo + 1 };
  return { livro: proxima.livro, capitulo: proxima.capitulo, versiculo: proxima.de, ciclo };
}

/** Passagem pelo id (para reabrir o que já foi lido hoje). */
export function passagemPeloId(biblia: Biblia, trilha: Trilha, id: string): Passagem | undefined {
  return passagens(biblia, trilha).find((p) => p.id === id);
}
