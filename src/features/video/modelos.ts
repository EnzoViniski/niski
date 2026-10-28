// Fork (Editor de vídeo): o que vai na pasta de um projeto novo — composição inicial, instruções do
// agente (CLAUDE.md/AGENTS.md) e o BRIEF.md montado do briefing e das referências.
import type { Briefing, Formato, LinkDeReferencia, Midia, PastaDeReferencia } from "./tipos";

/** Versão fixada (a mesma do Rust, do player e das skills). */
export const VERSAO_DO_HYPERFRAMES = "0.8.183";
/** GSAP da composição mínima da documentação oficial do HyperFrames. */
export const GSAP_DA_COMPOSICAO = "https://cdn.jsdelivr.net/npm/gsap@3.14.2/dist/gsap.min.js";
export const COMANDO_DO_HYPERFRAMES = `npx --yes hyperframes@${VERSAO_DO_HYPERFRAMES}`;

export const FORMATOS: Record<Formato, { largura: number; altura: number; rotulo: string; dica: string }> = {
  "16:9": { largura: 1920, altura: 1080, rotulo: "16:9", dica: "YouTube, apresentação" },
  "9:16": { largura: 1080, altura: 1920, rotulo: "9:16", dica: "Reels, TikTok, Shorts" },
  "1:1": { largura: 1080, altura: 1080, rotulo: "1:1", dica: "Feed quadrado" },
  "4:5": { largura: 1080, altura: 1350, rotulo: "4:5", dica: "Feed do Instagram" },
};

export const BRIEFING_VAZIO: Briefing = {
  descricao: "", estilo: "", duracao: 15, formato: "16:9", publico: "", plataforma: "", observacoes: "",
};

function escaparHtml(texto: string) {
  return texto.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Composição raiz de um projeto novo: um cartão com o nome, pronta para o agente substituir. */
export function indexInicial({ nome, formato, duracao, fps = 30, cor = "#0b0f14" }: { nome: string; formato: Formato; duracao: number; fps?: number; cor?: string }) {
  const { largura, altura } = FORMATOS[formato];
  const segundos = Math.max(1, Math.min(600, Math.round(duracao || 15)));
  const titulo = escaparHtml(nome.trim() || "Meu vídeo");
  const fonte = Math.round(Math.min(largura, altura) * 0.085);
  return `<!doctype html>
<html lang="pt-BR">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=${largura}, height=${altura}" />
    <title>${titulo}</title>
    <script src="${GSAP_DA_COMPOSICAO}"></script>
    <style>
      body { margin: 0; background: ${cor}; color: white; font-family: system-ui, sans-serif; }
      #root { position: relative; width: 100%; height: 100%; overflow: hidden; }
      .clip { position: absolute; inset: 0; display: grid; place-items: center; }
      #titulo { margin: 0; box-sizing: border-box; font-size: ${fonte}px; font-weight: 700; letter-spacing: -0.02em; text-align: center; padding: 0 8%; }
    </style>
  </head>
  <body>
    <div id="root" data-composition-id="main" data-start="0" data-width="${largura}" data-height="${altura}" data-duration="${segundos}" data-fps="${fps}">
      <h1 id="titulo" class="clip" data-start="0" data-duration="${segundos}" data-track-index="0">${titulo}</h1>
    </div>
    <script>
      const tl = gsap.timeline({ paused: true });
      tl.from("#titulo", { y: 48, opacity: 0, duration: 0.6, ease: "power3.out" }, 0.2);
      window.__timelines["main"] = tl;
    </script>
  </body>
</html>
`;
}

/** CLAUDE.md e AGENTS.md do projeto (Claude lê um, Codex o outro). */
export function instrucoesDoAgente() {
  return `# Projeto de vídeo do Niski (HyperFrames ${VERSAO_DO_HYPERFRAMES})

Você edita um vídeo feito em HTML com HyperFrames. O app mostra o preview e a timeline ao lado do
chat; a pessoa pede mudanças e você edita os arquivos desta pasta. O preview recarrega sozinho.

## Onde está cada coisa

- \`BRIEF.md\`: o que a pessoa quer (estilo, duração, formato, público, referências). Leia antes de começar.
- \`index.html\`: composição raiz. \`compositions/*.html\`: sub-composições.
- \`midias.json\`: mídias importadas. \`"papel": "usar"\` entra no vídeo; \`"papel": "referencia"\` é só
  inspiração (estilo, ritmo, cores): olhe, mas não coloque no vídeo sem a pessoa pedir.
- \`assets/\`: os arquivos das mídias. Use sempre o caminho relativo (\`assets/nome.mp4\`).
- Skills oficiais do HyperFrames em \`.claude/skills/\`. Comece por \`.claude/skills/hyperframes-core/SKILL.md\`
  (no Codex, leia esse arquivo antes da primeira edição).

## Regras

- Edite só arquivos desta pasta. Não mexa em \`projeto.json\`, \`midias.json\`, \`conversa.jsonl\` nem \`historico/\`:
  o app cuida deles (e guarda uma versão antes de cada pedido, para a pessoa desfazer).
- Preserve \`id\`, \`data-start\`, \`data-duration\`, \`data-media-start\` e \`data-track-index\` de tudo que não
  foi pedido para mudar. A timeline do app encontra os clipes pelo \`id\`; todo clipe novo precisa de um \`id\` único.
- Toda \`<video>\` e \`<audio>\` com \`id\`. O root com \`data-composition-id\`, \`data-width\`, \`data-height\` e \`data-duration\`.
- Animação só com timeline GSAP pausada em \`window.__timelines["<id do root>"]\`, de preferência em \`opacity\`
  e \`transform\`. Nada de \`Date.now\`, \`requestAnimationFrame\` ou \`Math.random\` sem semente.
- Para conferir: \`${COMANDO_DO_HYPERFRAMES} lint\`. Não rode \`render\`, \`preview\`, \`init\`, \`publish\`,
  \`cloud\`, \`lambda\`, \`auth\`, \`feedback\`, \`upgrade\` nem \`skills update\`: o app faz o preview e a exportação.
- Nada de render em nuvem, upload de mídia ou relatório de feedback. A telemetria fica desligada
  (\`HYPERFRAMES_NO_TELEMETRY=1\`).
- Não instale nada global e não baixe mídia da internet sem a pessoa pedir.
- Responda curto, em português, dizendo o que mudou (ex.: "Cortei os 3 primeiros segundos do clipe da abertura").
`;
}

const INICIO_DAS_REFERENCIAS = "<!-- medza:referencias -->";
const FIM_DAS_REFERENCIAS = "<!-- /medza:referencias -->";

function linha(rotulo: string, valor: string) { return valor.trim() ? `- **${rotulo}:** ${valor.trim()}\n` : ""; }

const NOME_DO_TIPO: Record<string, string> = {
  video: "vídeo", imagem: "imagem", audio: "áudio", fonte: "fonte", animacao: "animação", texto: "texto", outro: "arquivo",
};

/** Seção de referências do BRIEF.md, regenerada quando a biblioteca muda (entre as marcas). */
export function secaoDeReferencias(midias: Midia[], links: LinkDeReferencia[], pastas: Pick<PastaDeReferencia, "nome" | "descricao">[] = [], computador?: string) {
  const usar = midias.filter((m) => m.papel === "usar");
  const refs = midias.filter((m) => m.papel === "referencia");
  const item = (m: Midia) => `- \`${m.arquivo}\` (${NOME_DO_TIPO[m.tipo] ?? "arquivo"})${m.nota ? `: ${m.nota}` : ""}\n`;
  let texto = `${INICIO_DAS_REFERENCIAS}\n## Mídias e referências\n\n`;
  // A pasta do computador (identidade visual, informações) vem antes de tudo.
  if (computador) texto += `Pasta do computador deste vídeo: \`${computador}\` (identidade visual e informações; leia antes de criar).\n\n`;
  if (pastas.length) {
    texto += "Pastas de referências escolhidas (os caminhos estão no CLAUDE.md/AGENTS.md):\n\n";
    for (const p of pastas) texto += `- **${p.nome}**${p.descricao.trim() ? `: ${p.descricao.trim()}` : ""}\n`;
    texto += "\n";
  }
  texto += usar.length ? `Para usar no vídeo:\n\n${usar.map(item).join("")}\n` : "Nenhuma mídia para usar no vídeo ainda.\n\n";
  if (refs.length) texto += `Só referência de estilo (não colocar no vídeo sem pedir):\n\n${refs.map(item).join("")}\n`;
  if (links.length) texto += `Links de referência:\n\n${links.map((l) => `- ${l.url}${l.nota ? ` (${l.nota})` : ""}\n`).join("")}\n`;
  return `${texto}${FIM_DAS_REFERENCIAS}`;
}

/** BRIEF.md completo de um projeto novo. */
export function gerarBrief(nome: string, b: Briefing, midias: Midia[], links: LinkDeReferencia[], pastas: Pick<PastaDeReferencia, "nome" | "descricao">[] = [], computador?: string) {
  const { largura, altura } = FORMATOS[b.formato];
  return `# ${nome.trim() || "Meu vídeo"}

## O que eu quero

${b.descricao.trim() || "(sem descrição ainda)"}

## Formato

${linha("Formato", `${b.formato} (${largura}×${altura})`)}${linha("Duração", `${b.duracao} segundos`)}${linha("Estilo", b.estilo)}${linha("Público", b.publico)}${linha("Onde vai passar", b.plataforma)}${linha("Observações", b.observacoes)}
${secaoDeReferencias(midias, links, pastas, computador)}
`;
}

/** Troca só a seção de referências (o resto do BRIEF.md pode ter sido editado pela pessoa ou pelo agente). */
export function atualizarReferenciasNoBrief(brief: string, secao: string) {
  const inicio = brief.indexOf(INICIO_DAS_REFERENCIAS);
  const fim = brief.indexOf(FIM_DAS_REFERENCIAS);
  if (inicio >= 0 && fim > inicio) return brief.slice(0, inicio) + secao + brief.slice(fim + FIM_DAS_REFERENCIAS.length);
  return `${brief.trimEnd()}\n\n${secao}\n`;
}
