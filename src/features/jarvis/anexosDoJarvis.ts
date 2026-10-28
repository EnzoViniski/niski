// Anexos enviados em blocos oficiais à CLI do Jarvis, sem gravar o conteúdo em log.
export const MAXIMO_DE_ANEXOS = 6;
export const MAXIMO_POR_ANEXO = 6 * 1024 * 1024;
export const MAXIMO_TOTAL = 12 * 1024 * 1024;
export const TIPOS_DO_JARVIS = "image/png,image/jpeg,image/gif,image/webp,application/pdf,text/plain,text/markdown,text/csv,application/json,.md,.txt,.csv,.json";

const EXTENSOES: Record<string, string> = {
  png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp",
  pdf: "application/pdf", txt: "text/plain", md: "text/markdown", csv: "text/csv", json: "application/json",
};
const TIPOS = new Set(Object.values(EXTENSOES));

export interface AnexoDoJarvis { nome: string; mime: string; base64: string }
export interface AnexoNaJanela extends AnexoDoJarvis { id: number; tamanho: number }
let sequencia = 0;

function ler(arquivo: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const leitor = new FileReader();
    leitor.onerror = () => reject(new Error(`Não consegui ler ${arquivo.name}. Tente anexar de novo.`));
    leitor.onload = () => {
      const resultado = String(leitor.result);
      const separador = resultado.indexOf(",");
      if (separador < 0) reject(new Error(`Não consegui ler ${arquivo.name}.`));
      else resolve(resultado.slice(separador + 1));
    };
    leitor.readAsDataURL(arquivo);
  });
}

export async function prepararAnexos(arquivos: File[], anteriores: AnexoNaJanela[]): Promise<AnexoNaJanela[]> {
  if (arquivos.length + anteriores.length > MAXIMO_DE_ANEXOS) throw new Error("Anexe até 6 arquivos por mensagem.");
  const tipos = arquivos.map((arquivo) => arquivo.type || EXTENSOES[arquivo.name.split(".").pop()?.toLowerCase() ?? ""]);
  for (let i = 0; i < arquivos.length; i++) {
    if (!TIPOS.has(tipos[i])) throw new Error(`${arquivos[i].name}: use uma imagem PNG, JPEG, GIF ou WebP, PDF ou arquivo de texto, Markdown, CSV ou JSON.`);
    if (arquivos[i].size > MAXIMO_POR_ANEXO) throw new Error(`${arquivos[i].name} ultrapassa 6 MB.`);
    if (!arquivos[i].size) throw new Error(`${arquivos[i].name} está vazio.`);
  }
  const total = [...anteriores.map((a) => a.tamanho), ...arquivos.map((a) => a.size)].reduce((soma, tamanho) => soma + tamanho, 0);
  if (total > MAXIMO_TOTAL) throw new Error("Os anexos juntos ultrapassam 12 MB.");
  return Promise.all(arquivos.map(async (arquivo, i) => ({
    id: ++sequencia, nome: arquivo.name || "imagem", mime: tipos[i], tamanho: arquivo.size, base64: await ler(arquivo),
  })));
}

export function paraOJarvis(anexos: AnexoNaJanela[]): AnexoDoJarvis[] {
  return anexos.map(({ nome, mime, base64 }) => ({ nome, mime, base64 }));
}
