// Conta, modelo e voz do Jarvis em um painel compacto da própria janela.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";

export interface ConfiguracaoDoJarvis {
  perfil_id: string;
  modelo: string;
  voz_id: string | null;
  contas: { id: string; nome: string; autenticada: boolean }[];
  modelos: { id: string; nome: string }[];
  ativo: boolean;
}
export interface VozDoJarvis { id: string; nome: string; idioma: string; qualidade: string; genero: string }
interface Vozes { vozes: VozDoJarvis[]; voz: string | null }

export function AjustesDoJarvis({ ocupado, onFechar, onSalvo }: {
  ocupado: boolean; onFechar: () => void; onSalvo: (ajustes: ConfiguracaoDoJarvis) => void;
}) {
  const [config, setConfig] = useState<ConfiguracaoDoJarvis | null>(null);
  const [vozes, setVozes] = useState<VozDoJarvis[]>([]);
  const [salvando, setSalvando] = useState(false);
  const [erro, setErro] = useState("");
  const [avisoDaVoz, setAvisoDaVoz] = useState("");
  useEffect(() => {
    let vivo = true;
    void invoke<ConfiguracaoDoJarvis>("jarvis_configuracao").then((valor) => {
      if (vivo && valor) setConfig(valor);
    }).catch((e: unknown) => { if (vivo) setErro(String(e)); });
    void invoke<Vozes>("jarvis_vozes").then((valor) => {
      if (vivo) setVozes(valor?.vozes ?? []);
    }).catch(() => { if (vivo) setAvisoDaVoz("Não consegui listar as vozes. A voz automática continua disponível."); });
    return () => { vivo = false; };
  }, []);
  const salvar = async (e: FormEvent) => {
    e.preventDefault();
    if (!config || ocupado || salvando) return;
    setSalvando(true); setErro("");
    try {
      const ajustes = await invoke<ConfiguracaoDoJarvis>("jarvis_configurar", { ajustes: {
        perfil_id: config.perfil_id, modelo: config.modelo, voz_id: config.voz_id || null,
      } });
      onSalvo(ajustes); onFechar();
    } catch (e) { setErro(String(e)); }
    finally { setSalvando(false); }
  };
  return (
    <section className="jarvis-ajustes" aria-label="Ajustes do Jarvis" onKeyDown={(e) => { if (e.key === "Escape") { e.stopPropagation(); onFechar(); } }}>
      <div className="jarvis-ajustes-topo"><strong>Ajustes do Jarvis</strong><button type="button" className="jarvis-icone" aria-label="Fechar ajustes" onClick={onFechar}>×</button></div>
      {erro && <p role="alert" className="jarvis-ajustes-erro">{erro}</p>}
      {!config ? <p>Carregando ajustes…</p> : (
        <form onSubmit={(e) => void salvar(e)}>
          <fieldset disabled={salvando}>
            <label>Conta do Claude<select value={config.perfil_id} onChange={(e) => setConfig({ ...config, perfil_id: e.target.value })}>
              {!config.contas.some((c) => c.id === config.perfil_id) && <option value={config.perfil_id}>Escolha uma conta</option>}
              {config.contas.map((conta) => <option key={conta.id} value={conta.id} disabled={!conta.autenticada}>{conta.nome}{conta.autenticada ? "" : " (entrar na conta)"}</option>)}
            </select></label>
            <label>Modelo<select value={config.modelo} onChange={(e) => setConfig({ ...config, modelo: e.target.value })}>
              {!config.modelos.some((m) => m.id === config.modelo) && <option value={config.modelo}>{config.modelo}</option>}
              {config.modelos.map((modelo) => <option key={modelo.id} value={modelo.id}>{modelo.nome}</option>)}
            </select></label>
            <label>Voz<select value={config.voz_id ?? ""} onChange={(e) => setConfig({ ...config, voz_id: e.target.value || null })}>
              <option value="">Automática (preferir masculina)</option>
              {config.voz_id && !vozes.some((v) => v.id === config.voz_id) && <option value={config.voz_id}>Voz salva (indisponível)</option>}
              {vozes.map((voz) => <option key={voz.id} value={voz.id}>{voz.nome} · {voz.idioma}{voz.qualidade === "premium" ? " · Premium" : voz.qualidade === "aprimorada" ? " · Aprimorada" : ""}</option>)}
            </select></label>
          </fieldset>
          <p className="jarvis-ajustes-dica">A voz usa as opções instaladas no Mac. A troca de conta ou modelo preserva o contexto recente desta conversa.</p>
          {vozes.length > 0 && !vozes.some((voz) => voz.genero === "masculino" && voz.idioma.startsWith("pt")) && <p className="jarvis-ajustes-dica">Para adicionar uma voz masculina de melhor qualidade: Ajustes do Sistema → Acessibilidade → Leitura e Fala → informações da Voz do sistema.</p>}
          {avisoDaVoz && <p className="jarvis-ajustes-dica">{avisoDaVoz}</p>}
          {ocupado && <p className="jarvis-ajustes-dica" role="status">Espere o pedido terminar para trocar os ajustes.</p>}
          <button type="submit" className="jarvis-acao" disabled={ocupado || salvando || !config.perfil_id}>{salvando ? "Salvando…" : "Salvar ajustes"}</button>
        </form>
      )}
    </section>
  );
}
