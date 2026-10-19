import { useEffect, useState, type ReactNode } from "react";
import { MarcaDoProduto } from "./ui/MarcaDoProduto";
import { NotificationBell } from "./NotificationBell";
import { useJanelaEmFoco } from "../hooks/useJanelaEmFoco";
import type { SettingsSection } from "../hooks/useNotifications";
import { GlifoFechar, GlifoMaximizar, GlifoMinimizar, GlifoRestaurar } from "./ui/PixelGlyph";
import { NOME_DO_PRODUTO } from "../lib/identidade";
import { NO_MAC } from "../lib/plataforma";
import { GavetaDeUso } from "../features/terminal/GavetaDeUso";
import { AvisoDeAtualizacao } from "./AvisoDeAtualizacao";

/*
 * Barra superior própria (a janela nasce sem a moldura do Windows: `decorations: false`).
 *
 * Arrastar: o Tauri só arrasta pelo elemento que tem `data-tauri-drag-region` — não pelos filhos.
 * Por isso o atributo vai na barra e em cada pedaço de texto; botões ficam sem ele e continuam
 * clicáveis. Duplo clique numa região de arrasto maximiza/restaura, como na barra nativa.
 */

type Janela = {
  minimize: () => Promise<void>;
  toggleMaximize: () => Promise<void>;
  close: () => Promise<void>;
  isMaximized: () => Promise<boolean>;
  onResized: (cb: () => void) => Promise<() => void>;
};

/** A janela do Tauri, ou `null` fora dele (testes, navegador). */
async function janelaAtual(): Promise<Janela | null> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow() as unknown as Janela;
  } catch {
    return null;
  }
}

function useMaximizada(): boolean {
  const [maximizada, setMaximizada] = useState(false);
  useEffect(() => {
    let desligar: (() => void) | undefined;
    let desmontado = false;
    void (async () => {
      const janela = await janelaAtual();
      if (!janela) return;
      const ler = () => void janela.isMaximized().then((valor) => { if (!desmontado) setMaximizada(valor); }).catch(() => undefined);
      ler();
      try {
        const parar = await janela.onResized(ler);
        if (desmontado) parar(); else desligar = parar;
      } catch {
        // Sem evento de redimensionar: o ícone só não troca sozinho.
      }
    })();
    return () => { desmontado = true; desligar?.(); };
  }, []);
  return maximizada;
}

function BotaoDaJanela({ rotulo, perigo, onClick, children }: { rotulo: string; perigo?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={rotulo}
      title={rotulo}
      onClick={onClick}
      className={`titlebar-botao${perigo ? " titlebar-botao-fechar" : ""}`}
    >
      {children}
    </button>
  );
}

export function TitleBar({
  trilha,
  dica,
  acoes,
  onHome,
  onOpenSettings,
}: {
  /** Onde você está — workspace e projeto. Some quando vazio. */
  trilha: string[];
  /** Dica do último item (o caminho do projeto). */
  dica?: string;
  /** Controles da tela atual (os layouts do projeto), antes do sino. */
  acoes?: ReactNode;
  onHome: () => void;
  onOpenSettings: (secao: SettingsSection) => void;
}) {
  const maximizada = useMaximizada();
  const emFoco = useJanelaEmFoco();
  const agir = (acao: (janela: Janela) => Promise<void>) => () => {
    void janelaAtual().then((janela) => (janela ? acao(janela) : undefined)).catch(() => undefined);
  };

  return (
    <header data-tauri-drag-region className={`titlebar${emFoco ? "" : " titlebar-sem-foco"}${NO_MAC ? " titlebar-mac" : ""}`}>
      <button type="button" onClick={onHome} className="titlebar-marca" aria-label={`${NOME_DO_PRODUTO} — início`}>
        <MarcaDoProduto className="h-[18px] w-[18px] shrink-0" />
        <span className="pixel-text">{NOME_DO_PRODUTO}</span>
      </button>

      {trilha.length > 0 && (
        <span data-tauri-drag-region className="titlebar-trilha">
          {trilha.map((parte, indice) => (
            <span data-tauri-drag-region key={indice} className={indice === trilha.length - 1 ? "titlebar-trilha-atual" : undefined}
              title={indice === trilha.length - 1 ? dica : undefined}>
              {indice > 0 && <span data-tauri-drag-region className="titlebar-sep" aria-hidden>›</span>}
              {parte}
            </span>
          ))}
        </span>
      )}

      <span data-tauri-drag-region className="flex-1 self-stretch" />

      {acoes}

      {/* Fork: build novo no disco → botão que reabre o app. */}
      <AvisoDeAtualizacao />

      <GavetaDeUso />

      <span className="titlebar-sino">
        <NotificationBell onOpenSettings={onOpenSettings} />
      </span>

      {/* No Mac os semáforos são os nativos (tauri.macos.conf.json, canto esquerdo). */}
      {!NO_MAC && <span className="titlebar-controles">
        <BotaoDaJanela rotulo="Minimizar" onClick={agir((janela) => janela.minimize())}><GlifoMinimizar /></BotaoDaJanela>
        <BotaoDaJanela rotulo={maximizada ? "Restaurar" : "Maximizar"} onClick={agir((janela) => janela.toggleMaximize())}>
          {maximizada ? <GlifoRestaurar /> : <GlifoMaximizar />}
        </BotaoDaJanela>
        <BotaoDaJanela rotulo="Fechar" perigo onClick={agir((janela) => janela.close())}><GlifoFechar /></BotaoDaJanela>
      </span>}
    </header>
  );
}
