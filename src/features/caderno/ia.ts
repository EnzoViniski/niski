/**
 * Fork (Caderno): a IA ao lado do caderno. Mesmo caminho dos Roadmaps (`roadmaps_ia`: CLI oficial
 * sem tela, na conta escolhida, fora do histórico de conversas). Ela recebe o caderno inteiro (com
 * limite), a página aberta e a conversa recente; responde em Markdown, e você cola na página o que quiser.
 */
import { comTarefa } from "../roadmaps/loja";
import { perguntar } from "../roadmaps/ia";
import { acrescentarNaConversa, caderno, conversaDoCaderno, lerPagina } from "./loja";
import { SOBRE_O_TIPO, type MensagemDoCaderno, type MetaDoCaderno, type TipoDeCaderno } from "./tipos";

export const chaveDaIa = (id: string) => `caderno:${id}`;

/** Quanto do caderno vai junto (a página aberta vai inteira até o limite dela). */
const LIMITE_DA_PAGINA = 40_000;
const LIMITE_DAS_OUTRAS = 30_000;
const MENSAGENS_RECENTES = 16;

export const SISTEMA_DO_CADERNO = [
  "Você é a IA do caderno de anotações de uma pessoa, aberta ao lado das páginas que ela escreve à mão no app.",
  "Responda sempre em português do Brasil, em Markdown, curto e direto, como um bom parceiro de estudo e de reflexão.",
  "Baseie-se no que está escrito no caderno. Quando usar algo que não está nas anotações, diga que é de fora.",
  "Nunca invente citações, números de página ou trechos de livros; se não tiver certeza, diga que não tem.",
  "Quando pedirem um texto para a página, escreva pronto para colar, com as palavras da própria pessoa quando possível, sem comentários antes ou depois.",
].join("\n");

/** Botões rápidos por tipo de caderno (o texto vira a pergunta). */
export const SUGESTOES: Record<TipoDeCaderno, string[]> = {
  livro: ["Resuma o que anotei deste capítulo", "Faça 5 perguntas para eu fixar o que li", "Que ideias do livro aparecem nas minhas anotações?", "Organize esta página em tópicos"],
  estudo: ["Explique de outro jeito o que anotei", "Faça 5 perguntas para eu me testar", "O que está faltando nestas anotações?", "Organize esta página em tópicos"],
  vida: ["Me ajude a refletir sobre o que escrevi", "Que padrões você vê nas minhas páginas?", "Transforme isto em 3 próximos passos", "Faça uma pergunta que me faça pensar"],
  livre: ["Resuma esta página", "Organize esta página em tópicos", "Que perguntas este texto deixa em aberto?", "Continue a ideia comigo"],
};

function cortar(texto: string, max: number) {
  return texto.length <= max ? texto : `${texto.slice(0, max)}\n[…cortado: a página continua]`;
}

/** O pedido que vai para a CLI: o caderno, a página aberta, a conversa recente e a pergunta. */
export function montarPedido(meta: MetaDoCaderno, paginaId: string | undefined, textos: Record<string, string>, historico: MensagemDoCaderno[], pergunta: string): string {
  const tipo = SOBRE_O_TIPO[meta.tipo];
  const aberta = meta.paginas.find((p) => p.id === paginaId);
  const partes = [
    `# Caderno: ${meta.nome}`,
    `Tipo: ${tipo.nome}${meta.tipo === "livro" && meta.autor ? ` · livro de ${meta.autor}` : ""} · ${meta.paginas.length} página(s).`,
  ];
  if (aberta) partes.push(`\n## Página aberta agora: "${aberta.titulo || "Sem título"}"\n${cortar(textos[aberta.id] ?? "", LIMITE_DA_PAGINA) || "(vazia)"}`);
  let resto = LIMITE_DAS_OUTRAS;
  const outras = meta.paginas.filter((p) => p.id !== paginaId).map((p) => {
    const t = (textos[p.id] ?? "").trim();
    const trecho = resto > 0 ? cortar(t, Math.min(resto, 6_000)) : "";
    resto -= trecho.length;
    return `### ${p.titulo || "Sem título"}\n${trecho || "(sem texto ou fora do limite)"}`;
  });
  if (outras.length) partes.push(`\n## Outras páginas do caderno\n${outras.join("\n\n")}`);
  const recentes = historico.slice(-MENSAGENS_RECENTES);
  if (recentes.length) partes.push(`\n## Conversa até aqui\n${recentes.map((m) => `${m.de === "eu" ? "Pessoa" : "Você"}: ${m.texto}`).join("\n\n")}`);
  partes.push(`\n## Pergunta agora\n${pergunta}`);
  return partes.join("\n");
}

export async function perguntarAoCaderno(id: string, paginaId: string | undefined, pergunta: string, web: boolean, agora = () => new Date()): Promise<void> {
  const meta = caderno(id);
  if (!meta) throw new Error("Caderno não encontrado");
  const historico = conversaDoCaderno(id) ?? [];
  const textos: Record<string, string> = {};
  await Promise.all(meta.paginas.map(async (p) => { textos[p.id] = await lerPagina(id, p.id).catch(() => ""); }));
  const minha: MensagemDoCaderno = { de: "eu", texto: pergunta.trim().slice(0, 8_000), em: agora().toISOString(), ...(paginaId ? { paginaId } : {}) };
  await acrescentarNaConversa(id, minha);
  const resposta = await comTarefa(chaveDaIa(id), web ? "Pesquisando e pensando…" : "Pensando…", () =>
    perguntar({ chave: chaveDaIa(id), ia: meta.ia, sistema: SISTEMA_DO_CADERNO, web, texto: montarPedido(meta, paginaId, textos, historico, minha.texto) }));
  await acrescentarNaConversa(id, { de: "ia", texto: resposta.trim(), em: agora().toISOString(), ...(paginaId ? { paginaId } : {}) });
}
