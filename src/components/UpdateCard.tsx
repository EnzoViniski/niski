import { useEffect, useState } from "react";
import { Button } from "./ui";
import { ATUALIZACAO_AUTOMATICA, PAGINA_DE_DOWNLOADS } from "../lib/identidade";
// Fork (atualização automática): mesma lógica da barra; termina em `reiniciar_para_atualizar`
// (engine novo), não no `relaunch()` do plugin-process.
import {
  agentesTrabalhando,
  atualizarAgora,
  procurarAtualizacao,
  reiniciarParaAtualizar,
  textoDaConfirmacao,
  useAtualizacaoRemota,
} from "../lib/atualizacaoRemota";

/**
 * Atualização pelo app (plugin updater do Tauri).
 *
 * O download acontece no Rust, então o CSP da webview não entra na história —
 * e a assinatura minisign é conferida antes de instalar qualquer coisa. Sem a
 * chave privada correspondente à `pubkey` do tauri.conf.json, um pacote
 * adulterado é recusado.
 *
 * Imports do plugin são dinâmicos (em `lib/atualizacaoRemota`): fora do Tauri
 * (`npm run dev:vite`, testes) o módulo nem existe, e um import estático
 * quebraria a tela inteira.
 */
export function UpdateCard() {
  const { fase, versao, notas, progresso, erro } = useAtualizacaoRemota();
  const [confirmacao, setConfirmacao] = useState<string | null>(null);

  // Checagem silenciosa ao abrir a tela: quem entra em Configurações quer
  // saber, e falhar calado é melhor que um erro de rede no rosto.
  useEffect(() => {
    void procurarAtualizacao({ silencioso: true });
  }, []);

  if (!ATUALIZACAO_AUTOMATICA) return (
    <Card>
      <p className="text-xs text-text-muted">Baixe as novas versões na página do Medza Agents.</p>
      <Button onClick={() => { void import("@tauri-apps/plugin-opener").then(({ openUrl }) => openUrl(PAGINA_DE_DOWNLOADS)); }}>
        Ver versões para baixar
      </Button>
    </Card>
  );

  return (
    <Card>
      {fase === "disponivel" && versao && (
        <>
          <p className="text-text-primary text-sm font-medium">
            Versão {versao} disponível
          </p>
          {notas && (
            <p className="text-text-muted text-[11px] leading-relaxed whitespace-pre-line">
              {notas}
            </p>
          )}
          {confirmacao === null ? (
            <Button variant="primary" onClick={async () => setConfirmacao(textoDaConfirmacao(versao, await agentesTrabalhando()))}>
              Atualizar agora
            </Button>
          ) : (
            <>
              <p className="text-text-secondary text-xs">{confirmacao}</p>
              <div className="flex gap-2">
                <Button onClick={() => setConfirmacao(null)}>Cancelar</Button>
                <Button variant="primary" onClick={() => { setConfirmacao(null); void atualizarAgora(); }}>
                  Atualizar e reabrir
                </Button>
              </div>
            </>
          )}
        </>
      )}

      {fase === "baixando" && (
        <>
          <p className="text-text-primary text-sm font-medium">Baixando atualização…</p>
          <div className="glass-inset h-2 rounded-none overflow-hidden">
            <div
              className="shimmer h-full bg-accent transition-all duration-300"
              style={{ width: `${progresso}%` }}
              role="progressbar"
              aria-valuenow={progresso}
            />
          </div>
          <p className="text-text-muted text-[11px]">{progresso}%</p>
        </>
      )}

      {(fase === "instalada" || fase === "reiniciando") && (
        <>
          <p className="text-success text-sm font-medium">Atualização instalada</p>
          <p className="text-text-muted text-[11px]">
            {fase === "reiniciando" ? "Reabrindo o app…" : "Reabra o app para usar a versão nova."}
          </p>
          {erro && (
            <p role="alert" className="text-danger text-xs">
              {erro}
            </p>
          )}
          <Button variant="primary" onClick={() => void reiniciarParaAtualizar()} loading={fase === "reiniciando"}>
            Reabrir agora
          </Button>
        </>
      )}

      {(fase === "ocioso" || fase === "atual" || fase === "checando" || fase === "erro") && (
        <>
          <p className="text-xs">
            {fase === "atual" ? (
              <span className="text-success">✓ Você está na versão mais recente</span>
            ) : (
              <span className="text-text-muted">
                As atualizações são assinadas e verificadas antes de instalar.
              </span>
            )}
          </p>
          {erro && (
            <p role="alert" className="text-danger text-xs">
              {erro}
            </p>
          )}
          <Button onClick={() => void procurarAtualizacao()} loading={fase === "checando"}>
            {fase === "checando" ? "Procurando…" : "Procurar atualizações"}
          </Button>
        </>
      )}
    </Card>
  );
}

/** Mesma casca dos cards de Configurações, sem título próprio. */
function Card({ children }: { children: React.ReactNode }) {
  return <div className="glass rounded-none p-5 space-y-3">{children}</div>;
}
