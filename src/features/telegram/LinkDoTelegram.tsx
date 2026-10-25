// Fork (Telegram no celular): link para o Telegram que funciona nos dois lugares. No app do Mac abre
// pelo plugin opener (precisa de `https://t.me/*` na capability); no celular/web é um `<a>` de verdade:
// o toque abre o app do Telegram direto, sem bloqueio de pop-up do Safari.
import type { ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

const noApp = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function LinkDoTelegram({ url, children, className, onFalha }: {
  url: string; children: ReactNode; className?: string; onFalha: (mensagem: string) => void;
}) {
  if (!noApp()) return <a href={url} target="_blank" rel="noreferrer" className={className}>{children}</a>;
  return <a href={url} className={className} onClick={(e) => {
    e.preventDefault();
    void openUrl(url).catch(() => onFalha(`Não consegui abrir o Telegram. Abra ${url} no navegador.`));
  }}>{children}</a>;
}

/** Visual do `Button` pequeno, para o link principal. */
export const CLASSE_DE_BOTAO = "btn btn-glass px-3 py-2 text-xs inline-flex";
