// Fork: depois que o PC atualiza, o engine novo serve a página nova, mas o celular continua com a
// antiga aberta. A cada minuto a página confere se o `index.html` servido ainda aponta para o arquivo
// dela (o nome leva o hash do build); se não aponta, recarrega e pega a versão nova.
import { useEffect } from "react";

export const INTERVALO_DA_VERSAO_MS = 60_000;

/** Nome do arquivo deste módulo no build (`index-AbC123.js`), ou null em dev. */
export function arquivoDestaVersao(url = import.meta.url): string | null {
  const nome = new URL(url).pathname.split("/").pop() ?? "";
  return /-[A-Za-z0-9_-]{6,}\.js$/.test(nome) ? nome : null;
}

export function versaoMudou(html: string, arquivo: string | null): boolean {
  return Boolean(arquivo) && html.includes("<script") && !html.includes(arquivo!);
}

export function useRecarregarComVersaoNova() {
  useEffect(() => {
    const arquivo = arquivoDestaVersao();
    if (!arquivo) return;
    const conferir = async () => {
      try {
        const resposta = await fetch(location.pathname + location.search, { cache: "no-store" });
        if (!resposta.ok || !versaoMudou(await resposta.text(), arquivo)) return;
        // Uma recarga por versão: se o nome não bater por outro motivo, não entra em laço.
        if (sessionStorage.getItem("medza-recarregou-de") === arquivo) return;
        sessionStorage.setItem("medza-recarregou-de", arquivo);
        location.reload();
      } catch { /* PC reiniciando: tenta de novo no próximo minuto */ }
    };
    const relogio = window.setInterval(() => void conferir(), INTERVALO_DA_VERSAO_MS);
    return () => window.clearInterval(relogio);
  }, []);
}
