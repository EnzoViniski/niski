/**
 * Fork (Manhã): questões de múltipla escolha geradas no próprio app (sem IA, sem internet).
 * Semente pelo dia: as mesmas questões se o app reabrir no meio da sessão. Cada gerador devolve
 * enunciado, resposta certa, distratores plausíveis (erros comuns) e a explicação curta.
 */

export type TemaGerado = "aritmetica" | "porcentagem" | "fracoes" | "equacoes" | "sequencias" | "logica" | "potencias" | "problemas";
export type Tema = TemaGerado | "roadmaps" | "livre";
export type Nivel = 1 | 2 | 3;

export interface Questao {
  id: string;
  tema: Tema;
  enunciado: string;
  opcoes: string[];
  correta: number;
  explicacao: string;
  /** Revisão de roadmap: de onde veio o card, para registrar a revisão (FSRS). */
  origem?: { roadmap: string; card: string; nome: string };
}

export const ROTULO_DO_TEMA: Record<Tema, string> = {
  aritmetica: "Cálculo mental", porcentagem: "Porcentagem", fracoes: "Frações", equacoes: "Equações",
  sequencias: "Sequências", logica: "Lógica", potencias: "Potências e raízes", problemas: "Problemas",
  roadmaps: "Revisão do roadmap", livre: "Tema livre",
};
export const EMOJI_DO_TEMA: Record<Tema, string> = {
  aritmetica: "🧮", porcentagem: "％", fracoes: "½", equacoes: "𝑥", sequencias: "🔢", logica: "🧩",
  potencias: "√", problemas: "📐", roadmaps: "🗺️", livre: "✨",
};
export const TEMAS_GERADOS: TemaGerado[] = ["aritmetica", "porcentagem", "fracoes", "equacoes", "sequencias", "logica", "potencias", "problemas"];

// ---- Aleatório com semente ----

export type Rng = () => number;
export function semente(texto: string): number {
  let h = 2166136261;
  for (let i = 0; i < texto.length; i++) h = Math.imul(h ^ texto.charCodeAt(i), 16777619);
  return h >>> 0;
}
export function criarRng(seed: number): Rng {
  let a = seed || 1;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const int = (rng: Rng, min: number, max: number) => min + Math.floor(rng() * (max - min + 1));
const escolher = <T>(rng: Rng, lista: readonly T[]): T => lista[Math.floor(rng() * lista.length)];
export function embaralhar<T>(rng: Rng, lista: T[]): T[] {
  const copia = lista.slice();
  for (let i = copia.length - 1; i > 0; i--) { const j = Math.floor(rng() * (i + 1)); [copia[i], copia[j]] = [copia[j], copia[i]]; }
  return copia;
}

const fmt = (n: number) => (Number.isInteger(n) ? n.toLocaleString("pt-BR") : n.toLocaleString("pt-BR", { maximumFractionDigits: 2 }));
/** Dinheiro: centavos sempre com dois dígitos (R$ 52,50). */
const reais = (n: number) => `R$ ${Number.isInteger(n) ? n.toLocaleString("pt-BR") : n.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;

interface Bruta { enunciado: string; correta: string; distratores: string[]; explicacao: string }

/** Monta as 4 opções: certa + distratores distintos (completa com vizinhos se faltar). */
export function montarOpcoes(rng: Rng, bruta: Bruta, numerico?: number): { opcoes: string[]; correta: number } {
  const vistos = new Set([bruta.correta]);
  const erradas: string[] = [];
  for (const d of bruta.distratores) if (!vistos.has(d) && erradas.length < 3) { vistos.add(d); erradas.push(d); }
  // Faltou distrator (dois erros comuns deram o mesmo valor): vizinhos do número certo, com o
  // mesmo prefixo/sufixo ("R$ ", "%", " km").
  const partes = bruta.correta.match(/^(.*?)(-?[\d.]+(?:,\d+)?)(.*)$/);
  const base = numerico ?? (partes ? Number(partes[2].replace(/\./g, "").replace(",", ".")) : NaN);
  const [antes, depois] = numerico !== undefined || !partes ? ["", ""] : [partes[1], partes[3]];
  for (let passo = 1; erradas.length < 3 && Number.isFinite(base) && passo < 50; passo++) {
    const candidato = `${antes}${fmt(base + (passo % 2 ? passo : -passo) * Math.max(1, Math.round(Math.abs(base) * 0.1)))}${depois}`;
    if (!vistos.has(candidato)) { vistos.add(candidato); erradas.push(candidato); }
  }
  const opcoes = embaralhar(rng, [bruta.correta, ...erradas]);
  return { opcoes, correta: opcoes.indexOf(bruta.correta) };
}

type Gerador = (rng: Rng, nivel: Nivel) => Bruta & { valor?: number };

const aritmetica: Gerador = (rng, nivel) => {
  const tipo = int(rng, 0, nivel === 1 ? 2 : 4);
  if (tipo === 0) {
    const a = int(rng, 10 ** nivel + 7, 10 ** (nivel + 1) - 1), b = int(rng, 10 ** nivel + 3, 10 ** (nivel + 1) - 1);
    return { enunciado: `Quanto é ${fmt(a)} + ${fmt(b)}?`, correta: fmt(a + b), valor: a + b, distratores: [fmt(a + b + 10), fmt(a + b - 10), fmt(a + b + 1)], explicacao: `Some por partes: dezenas e unidades. ${fmt(a)} + ${fmt(b)} = ${fmt(a + b)}.` };
  }
  if (tipo === 1) {
    const a = int(rng, 10 ** nivel * 3, 10 ** (nivel + 1)), b = int(rng, 11, a - 5);
    return { enunciado: `Quanto é ${fmt(a)} − ${fmt(b)}?`, correta: fmt(a - b), valor: a - b, distratores: [fmt(a - b + 10), fmt(a - b - 10), fmt(a + b)], explicacao: `Complete até a dezena: ${fmt(a)} − ${fmt(b)} = ${fmt(a - b)}.` };
  }
  if (tipo === 2) {
    const a = int(rng, nivel === 1 ? 6 : 12, nivel === 1 ? 12 : 25 * nivel), b = int(rng, 3, nivel === 1 ? 9 : 15);
    return { enunciado: `Quanto é ${a} × ${b}?`, correta: fmt(a * b), valor: a * b, distratores: [fmt(a * (b + 1)), fmt((a + 1) * b), fmt(a * b + 10)], explicacao: `Decomponha: ${a} × ${b} = ${a} × ${b - 1} + ${a} = ${fmt(a * b)}.` };
  }
  if (tipo === 3) {
    const b = int(rng, 3, 14), q = int(rng, 6, 12 * nivel), a = b * q;
    return { enunciado: `Quanto é ${fmt(a)} ÷ ${b}?`, correta: fmt(q), valor: q, distratores: [fmt(q + 1), fmt(q - 1), fmt(q + b)], explicacao: `${b} × ${q} = ${fmt(a)}, então ${fmt(a)} ÷ ${b} = ${q}.` };
  }
  const a = int(rng, 3, 9), b = int(rng, 4, 12), c = int(rng, 2, 6), d = c * int(rng, 2, 9);
  const certo = a * b - d / c;
  return {
    enunciado: `Quanto é ${a} × ${b} − ${d} ÷ ${c}?`, correta: fmt(certo), valor: certo,
    distratores: [fmt((a * b - d) / c), fmt(a * (b - d / c) + 1), fmt(certo + c)],
    explicacao: `Multiplicação e divisão vêm antes: ${a * b} − ${d / c} = ${fmt(certo)}.`,
  };
};

const porcentagem: Gerador = (rng, nivel) => {
  const tipo = int(rng, 0, nivel === 1 ? 1 : 3);
  if (tipo <= 1) {
    const p = escolher(rng, nivel === 1 ? [10, 20, 25, 50] : [5, 15, 12, 30, 35, 40, 75]);
    const base = int(rng, 2, 30) * (nivel === 1 ? 20 : 40);
    const certo = (p / 100) * base;
    return { enunciado: `Quanto é ${p}% de ${fmt(base)}?`, correta: fmt(certo), valor: certo, distratores: [fmt(certo * 2), fmt(certo / 2), fmt(base - certo)], explicacao: `${p}% = ${p}/100. ${fmt(base)} × ${p} ÷ 100 = ${fmt(certo)}.` };
  }
  if (tipo === 2) {
    const preco = int(rng, 4, 40) * 10, p = escolher(rng, [10, 15, 20, 25, 30]);
    const certo = preco * (1 - p / 100);
    return { enunciado: `Um produto de ${reais(preco)} está com ${p}% de desconto. Quanto fica?`, correta: reais(certo), distratores: [reais(preco * p / 100), reais(preco - p), reais(preco * (1 + p / 100))], explicacao: `Desconto de ${p}% = pagar ${100 - p}%: ${fmt(preco)} × ${(100 - p) / 100} = ${fmt(certo)}.` };
  }
  const antes = int(rng, 4, 20) * 10, depois = antes + antes * escolher(rng, [0.1, 0.2, 0.25, 0.5]);
  const p = ((depois - antes) / antes) * 100;
  return { enunciado: `Um valor subiu de ${fmt(antes)} para ${fmt(depois)}. Qual foi o aumento percentual?`, correta: `${fmt(p)}%`, distratores: [`${fmt(depois - antes)}%`, `${fmt(p / 2)}%`, `${fmt((depois - antes) / depois * 100)}%`], explicacao: `Aumento ÷ valor inicial: ${fmt(depois - antes)} ÷ ${fmt(antes)} = ${fmt(p / 100)} = ${fmt(p)}%.` };
};

const mdc = (a: number, b: number): number => (b ? mdc(b, a % b) : Math.abs(a));
const fracao = (n: number, d: number) => { const g = mdc(n, d); return d / g === 1 ? `${n / g}` : `${n / g}/${d / g}`; };

const fracoes: Gerador = (rng, nivel) => {
  const tipo = int(rng, 0, nivel === 1 ? 1 : 3);
  if (tipo === 0) {
    const d = escolher(rng, [3, 4, 5, 6, 8, 10]), n = int(rng, 1, d - 1), total = d * int(rng, 3, 15);
    if (mdc(n, d) !== 1) return fracoes(rng, nivel);
    return { enunciado: `Quanto é ${fracao(n, d)} de ${total}?`, correta: fmt((total / d) * n), valor: (total / d) * n, distratores: [fmt(total / d), fmt(total - (total / d) * n), fmt((total / n) * d > 999 ? total + n : (total / n) * d)], explicacao: `Divida por ${d} e multiplique por ${n}: ${total} ÷ ${d} × ${n} = ${fmt((total / d) * n)}.` };
  }
  if (tipo === 1) {
    const g = int(rng, 2, 9), n = int(rng, 1, 7), d = n + int(rng, 1, 7);
    if (mdc(n, d) !== 1) return fracoes(rng, nivel);
    return { enunciado: `Simplifique ${n * g}/${d * g}.`, correta: `${n}/${d}`, distratores: [`${n + 1}/${d}`, `${n}/${d + 1}`, `${d}/${n}`], explicacao: `Divida em cima e embaixo por ${g}: ${n * g}/${d * g} = ${n}/${d}.` };
  }
  const [a, b] = [escolher(rng, [2, 3, 4, 5, 6]), escolher(rng, [2, 3, 4, 5, 6, 8])];
  const [x, y] = [int(rng, 1, a - 1) || 1, int(rng, 1, b - 1) || 1];
  if (tipo === 2) {
    return { enunciado: `Quanto é ${x}/${a} + ${y}/${b}?`, correta: fracao(x * b + y * a, a * b), distratores: [fracao(x + y, a + b), fracao(x * b + y * a + 1, a * b), fracao(x + y, a * b)], explicacao: `Denominador comum ${a * b}: ${x * b}/${a * b} + ${y * a}/${a * b} = ${fracao(x * b + y * a, a * b)}.` };
  }
  return { enunciado: `Quanto é ${x}/${a} × ${y}/${b}?`, correta: fracao(x * y, a * b), distratores: [fracao(x * b, a * y), fracao(x + y, a + b), fracao(x * y, a + b)], explicacao: `Multiplique em cima e embaixo: ${x * y}/${a * b} = ${fracao(x * y, a * b)}.` };
};

const equacoes: Gerador = (rng, nivel) => {
  const x = int(rng, -3 * (nivel - 1), 12) || 4;
  if (nivel === 1) {
    const a = int(rng, 2, 9), b = int(rng, 1, 20);
    return { enunciado: `Se ${a}x + ${b} = ${a * x + b}, quanto vale x?`, correta: fmt(x), valor: x, distratores: [fmt(x + 1), fmt((a * x + b + b) / a), fmt(x * 2)], explicacao: `Tire ${b} dos dois lados: ${a}x = ${a * x}. Divida por ${a}: x = ${x}.` };
  }
  const a = int(rng, 3, 9), c = int(rng, 1, a - 1), b = int(rng, -10, 15);
  const d = a * x + b - c * x;
  const sinal = (n: number) => (n < 0 ? `− ${-n}` : `+ ${n}`);
  return {
    enunciado: `Resolva: ${a}x ${sinal(b)} = ${c}x ${sinal(d)}. Quanto vale x?`, correta: fmt(x), valor: x,
    distratores: [fmt(-x || 1), fmt(x + 2), fmt((d + b) / (a - c) === x ? x - 1 : (d + b) / (a - c))],
    explicacao: `Junte os x: ${a - c === 1 ? "" : a - c}x = ${fmt(d - b)}. Então x = ${fmt(d - b)} ÷ ${a - c} = ${x}.`,
  };
};

const sequencias: Gerador = (rng, nivel) => {
  const tipo = int(rng, 0, nivel === 1 ? 1 : 4);
  let termos: number[], proximo: number, regra: string;
  if (tipo === 0) {
    const a = int(rng, 1, 30), r = int(rng, 2, 9) * (rng() < 0.3 ? -1 : 1);
    termos = [0, 1, 2, 3, 4].map((i) => a + r * i); proximo = a + r * 5; regra = r < 0 ? `subtrai ${-r} a cada passo` : `soma ${r} a cada passo`;
  } else if (tipo === 1) {
    const a = int(rng, 1, 5), r = int(rng, 2, 3);
    termos = [0, 1, 2, 3].map((i) => a * r ** i); proximo = a * r ** 4; regra = `multiplica por ${r}`;
  } else if (tipo === 2) {
    const a = int(rng, 1, 4), b = int(rng, 2, 6);
    termos = [a, b]; while (termos.length < 6) termos.push(termos[termos.length - 1] + termos[termos.length - 2]);
    proximo = termos[4] + termos[5]; regra = "cada termo é a soma dos dois anteriores";
  } else if (tipo === 3) {
    const a = int(rng, 1, 5);
    termos = [0, 1, 2, 3, 4].map((i) => (i + a) ** 2); proximo = (5 + a) ** 2; regra = `quadrados perfeitos (${a}², ${a + 1}², …)`;
  } else {
    const a = int(rng, 2, 10), d = int(rng, 1, 3);
    termos = [a]; for (let i = 1; i < 5; i++) termos.push(termos[i - 1] + d * i);
    proximo = termos[4] + d * 5; regra = `a diferença cresce ${d} a cada passo`;
  }
  const ultimo = termos[termos.length - 1], diff = ultimo - termos[termos.length - 2];
  return { enunciado: `Qual é o próximo número? ${termos.map(fmt).join(", ")}, …`, correta: fmt(proximo), valor: proximo, distratores: [fmt(ultimo + diff), fmt(proximo + 1), fmt(proximo - 2)], explicacao: `A regra: ${regra}. Próximo: ${fmt(proximo)}.` };
};

const NOMES = ["Ana", "Bruno", "Carla", "Diego", "Elisa", "Fábio", "Gabi", "Hugo"];
const DIAS = ["domingo", "segunda", "terça", "quarta", "quinta", "sexta", "sábado"];

const logica: Gerador = (rng, nivel) => {
  const tipo = int(rng, 0, 3);
  if (tipo === 0) {
    const hoje = int(rng, 0, 6), n = int(rng, 8, nivel === 1 ? 20 : 100);
    const certo = DIAS[(hoje + n) % 7];
    return { enunciado: `Hoje é ${DIAS[hoje]}. Que dia da semana será daqui a ${n} dias?`, correta: certo, distratores: [DIAS[(hoje + n + 1) % 7], DIAS[(hoje + n + 6) % 7], DIAS[(hoje + n + 3) % 7]], explicacao: `${n} dias = ${Math.floor(n / 7)} semanas + ${n % 7} dias. ${DIAS[hoje]} + ${n % 7} = ${certo}.` };
  }
  if (tipo === 1) {
    const [a, b, c] = embaralhar(rng, NOMES).slice(0, 3);
    return { enunciado: `${a} é mais alto(a) que ${b}. ${c} é mais baixo(a) que ${b}. Quem é o(a) mais baixo(a)?`, correta: c, distratores: [b, a, "Não dá para saber"], explicacao: `Ordem: ${a} > ${b} > ${c}. O(a) mais baixo(a) é ${c}.` };
  }
  if (tipo === 2) {
    const h = int(rng, 1, 11), m = escolher(rng, nivel === 1 ? [0, 30] : [0, 15, 20, 30, 40]);
    const angulo = Math.abs(30 * h - 5.5 * m), menor = Math.min(angulo, 360 - angulo);
    return { enunciado: `Qual o menor ângulo entre os ponteiros de um relógio às ${h}h${m ? String(m).padStart(2, "0") : ""}?`, correta: `${fmt(menor)}°`, distratores: [`${fmt(30 * h > 180 ? 360 - 30 * h : 30 * h)}°`, `${fmt(menor + 15)}°`, `${fmt(Math.abs(menor - 30))}°`].filter((x) => x !== `${fmt(menor)}°`), explicacao: `Horas andam 30° por hora + 0,5° por minuto; minutos andam 6° por minuto. Diferença: ${fmt(menor)}°.` };
  }
  const pernas = int(rng, 10, 30) * 2, cabecas = int(rng, Math.ceil(pernas / 4) + 1, pernas / 2 - 1);
  const coelhos = (pernas - 2 * cabecas) / 2;
  return { enunciado: `Num sítio há galinhas e coelhos: ${cabecas} cabeças e ${pernas} pernas. Quantos coelhos?`, correta: fmt(coelhos), valor: coelhos, distratores: [fmt(cabecas - coelhos), fmt(coelhos + 2), fmt(pernas / 4)], explicacao: `Se todos fossem galinhas: ${2 * cabecas} pernas. Sobram ${pernas - 2 * cabecas}, 2 a mais por coelho: ${coelhos}.` };
};

const potencias: Gerador = (rng, nivel) => {
  const tipo = int(rng, 0, nivel === 1 ? 1 : 3);
  if (tipo === 0) { const n = int(rng, 11, nivel === 1 ? 15 : 25); return { enunciado: `Quanto é ${n}²?`, correta: fmt(n * n), valor: n * n, distratores: [fmt(n * 2), fmt(n * n + n), fmt((n - 1) * (n + 1))], explicacao: `${n}² = ${n} × ${n} = ${fmt(n * n)}.` }; }
  if (tipo === 1) { const n = int(rng, 6, nivel === 1 ? 15 : 30); return { enunciado: `Quanto é √${fmt(n * n)}?`, correta: fmt(n), valor: n, distratores: [fmt(n + 1), fmt(n - 1), fmt(n + 2)], explicacao: `${n} × ${n} = ${fmt(n * n)}.` }; }
  if (tipo === 2) { const e = int(rng, 5, 12); return { enunciado: `Quanto é 2^${e}?`, correta: fmt(2 ** e), valor: 2 ** e, distratores: [fmt(2 * e), fmt(2 ** (e - 1)), fmt(2 ** (e + 1))], explicacao: `Dobrando ${e} vezes a partir de 1: ${fmt(2 ** e)}.` }; }
  const b = int(rng, 2, 5), e1 = int(rng, 2, 4), e2 = int(rng, 2, 4);
  return { enunciado: `Simplifique ${b}^${e1} × ${b}^${e2}.`, correta: `${b}^${e1 + e2}`, distratores: [`${b}^${e1 * e2}`, `${b * b}^${e1 + e2}`, `${b}^${Math.abs(e1 - e2) || 1}`], explicacao: `Mesma base: some os expoentes. ${e1} + ${e2} = ${e1 + e2}.` };
};

const problemas: Gerador = (rng, nivel) => {
  const tipo = int(rng, 0, 3);
  if (tipo === 0) {
    const v = int(rng, 4, 12) * 10, t = escolher(rng, nivel === 1 ? [1, 2, 3] : [1.5, 2.5, 0.75, 2]);
    const d = v * t;
    return { enunciado: `Um carro a ${v} km/h anda quantos km em ${fmt(t)} h?`, correta: `${fmt(d)} km`, distratores: [`${fmt(v / t)} km`, `${fmt(v + t)} km`, `${fmt(d + 10)} km`], explicacao: `Distância = velocidade × tempo = ${v} × ${fmt(t)} = ${fmt(d)} km.` };
  }
  if (tipo === 1) {
    const q = int(rng, 3, 8), preco = int(rng, 2, 9) + escolher(rng, [0, 0.5]), n = q + int(rng, 2, 6);
    return { enunciado: `${q} cadernos custam ${reais(q * preco)}. Quanto custam ${n}?`, correta: reais(n * preco), distratores: [reais(q * preco + n - q), reais((n - 1) * preco), reais(n * preco + preco)], explicacao: `Cada um custa ${reais(preco)}. ${n} × ${fmt(preco)} = ${reais(n * preco)}.` };
  }
  if (tipo === 2) {
    const a = int(rng, 2, 6), b = a * escolher(rng, [2, 3]);
    const juntos = (a * b) / (a + b);
    return { enunciado: `Uma torneira enche um tanque em ${a} h e outra em ${b} h. Juntas, em quanto tempo enchem?`, correta: `${fmt(juntos)} h`, distratores: [`${fmt((a + b) / 2)} h`, `${fmt(a + b)} h`, `${fmt(Math.abs(b - a))} h`], explicacao: `Por hora: 1/${a} + 1/${b} do tanque. Tempo = ${a}×${b} ÷ (${a}+${b}) = ${fmt(juntos)} h.` };
  }
  const idade = int(rng, 8, 20), diff = int(rng, 20, 35), anos = int(rng, 2, 10);
  return { enunciado: `Hoje um pai tem ${idade + diff} anos e o filho ${idade}. Daqui a ${anos} anos, quanto somarão as idades?`, correta: fmt(2 * idade + diff + 2 * anos), valor: 2 * idade + diff + 2 * anos, distratores: [fmt(2 * idade + diff + anos), fmt(2 * idade + diff), fmt(2 * idade + diff + 2 * anos + 2)], explicacao: `Cada um ganha ${anos} anos: ${2 * idade + diff} + ${2 * anos} = ${2 * idade + diff + 2 * anos}.` };
};

const GERADORES: Record<TemaGerado, Gerador> = { aritmetica, porcentagem, fracoes, equacoes, sequencias, logica, potencias, problemas };

export function gerarQuestao(tema: TemaGerado, nivel: Nivel, rng: Rng, id: string): Questao {
  const bruta = GERADORES[tema](rng, nivel);
  const { opcoes, correta } = montarOpcoes(rng, bruta, bruta.valor);
  return { id, tema, enunciado: bruta.enunciado, opcoes, correta, explicacao: bruta.explicacao };
}

/** Distribui `quantidade` entre os temas escolhidos, alternando, numa ordem embaralhada pelo dia. */
export function sortearTemas<T>(temas: T[], quantidade: number, rng: Rng): T[] {
  if (!temas.length) return [];
  const ordem = embaralhar(rng, temas);
  return Array.from({ length: quantidade }, (_, i) => ordem[i % ordem.length]);
}
