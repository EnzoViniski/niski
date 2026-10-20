import { useEffect, useState } from "react";
import { request } from "../../mobile/api";
import { AgentIcon } from "../../components/ui/AgentIcon";

interface Agente { id: string; nome: string; ligado: boolean; ativo: boolean }
interface Ajustes { versao_conferida: string; suportado: boolean; agentes: Agente[] }
const ROTA = "/codex/app-server/ajustes";

/** Fork 3A: escolha por conversa/agente, nunca liga globalmente nem muda os agentes abertos. */
export function CodexAppServer() {
  const [ajustes, setAjustes] = useState<Ajustes | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [salvando, setSalvando] = useState(false);
  useEffect(() => {
    let vivo = true;
    request<Ajustes>(ROTA).then((a) => { if (vivo) setAjustes(a); })
      .catch(() => { if (vivo) setErro("Engine antigo: reinicie o app para configurar o Codex."); });
    return () => { vivo = false; };
  }, []);
  async function mudar(agente: string, ligado: boolean) {
    setSalvando(true); setErro(null);
    try {
      setAjustes(await request<Ajustes>(ROTA, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ agente, ligado }) }));
    } catch { setErro("Não consegui salvar. A opção anterior continua valendo."); }
    finally { setSalvando(false); }
  }
  return (
    <section className="glass mt-4 border border-border-subtle p-4" aria-labelledby="codex-app-server">
      <h3 id="codex-app-server" className="flex items-center gap-2 text-xs font-semibold text-text-primary">
        <AgentIcon provider="codex" size={14} />Codex: conexão por agente
      </h3>
      <p className="mt-1 text-[11px] text-text-secondary">
        Terminal é o padrão. App-server (experimental) recebe eventos e permissões direto do Codex e mantém o terminal disponível.
        Se a conexão falhar, o agente volta ao terminal. Compatibilidade conferida com codex-cli {ajustes?.versao_conferida ?? "0.163.0"}.
      </p>
      <p className="mt-2 text-[11px] text-text-muted">A escolha vale ao reabrir ou reiniciar este agente. Cada conversa tem sua própria opção.</p>
      {ajustes && !ajustes.suportado && <p className="mt-2 text-[11px] text-text-muted">App-server disponível em macOS e Linux. Neste sistema, use Terminal.</p>}
      {ajustes?.agentes.length === 0 && <p className="mt-3 text-xs text-text-muted">Abra um agente Codex para escolher a conexão dele aqui.</p>}
      <div className="mt-3 flex flex-col gap-3">
        {ajustes?.agentes.map((a) => (
          <label key={a.id} className="flex items-center justify-between gap-3 text-xs text-text-primary">
            <span>{a.nome}<span className="ml-2 text-[11px] text-text-muted">{a.ativo ? "App-server ativo" : "Terminal"}</span></span>
            <select aria-label={`Conexão de ${a.nome}`} value={a.ligado ? "app-server" : "terminal"} disabled={salvando || !ajustes.suportado}
              onChange={(e) => void mudar(a.id, e.target.value === "app-server")}
              className="field px-2 py-1 text-xs">
              <option value="terminal">Terminal (padrão)</option>
              <option value="app-server">App-server (experimental)</option>
            </select>
          </label>
        ))}
      </div>
      {erro && <p role="alert" className="mt-2 text-[11px] text-danger">{erro}</p>}
    </section>
  );
}
