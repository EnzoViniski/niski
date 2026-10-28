// Jarvis. Botão na barra do topo, ao lado de Layouts: abre e fecha a janela flutuante (o mesmo
// que ⌃⌥J). Fica aceso enquanto o Jarvis está aberto.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
// A janela flutuante pede para abrir a equipe no layout principal.
import { pedirAbrirFluxo } from "../orquestracao/abrirFluxo";

export function BotaoDoJarvis() {
  const [aberto, setAberto] = useState(false);

  useEffect(() => {
    let solto = false;
    let soltar: (() => void) | undefined;
    let soltarFluxo: (() => void) | undefined;
    void invoke<boolean>("jarvis_janela_aberta").then((v) => { if (!solto) setAberto(Boolean(v)); }).catch(() => undefined);
    void listen<{ aberta: boolean }>("jarvis-janela", ({ payload }) => setAberto(payload.aberta))
      .then((parar) => { if (solto) parar(); else soltar = parar; });
    void listen("jarvis-abrir-fluxo", () => pedirAbrirFluxo("jarvis-medza", "Equipe do Jarvis"))
      .then((parar) => { if (solto) parar(); else soltarFluxo = parar; }).catch(() => undefined);
    return () => { solto = true; soltar?.(); soltarFluxo?.(); };
  }, []);

  return (
    <button type="button" className={`titlebar-acao${aberto ? " is-aberto" : ""}`} aria-pressed={aberto}
      aria-label={aberto ? "Fechar o Jarvis" : "Abrir o Jarvis"} title="Jarvis, o copiloto por voz (⌃⌥J)"
      onClick={() => { void invoke("jarvis_alternar").catch(() => undefined); }}>
      <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden>
        <circle cx="8" cy="8" r="2.2" fill="currentColor" stroke="none" />
        <path d="M8 1.5a6.5 6.5 0 0 1 6.5 6.5M8 14.5A6.5 6.5 0 0 1 1.5 8" strokeLinecap="round" />
        <circle cx="8" cy="8" r="4.4" opacity=".55" />
      </svg>
      <span>Jarvis</span>
    </button>
  );
}
