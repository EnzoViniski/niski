import { useEffect, useState } from "react";
import { request } from "../../mobile/api";
import { AgentIcon } from "../../components/ui/AgentIcon";

/**
 * Fork: o que o Antigravity aprova sozinho nas conversas abertas pelo app (`--mode` do `agy`, só nessas
 * sessões; a config global do agy não muda). O que ele ainda perguntar aparece na conversa, com
 * Permitir/Negar, como no Claude e no Codex.
 */
type Modo = "sem-perguntar" | "auto" | "accept-edits" | "padrao";

const OPCOES: { valor: Modo; nome: string; desc: string }[] = [
  // Fork: padrão do app desde 23/10/2026 (pedido do Enzo), `--dangerously-skip-permissions` do agy 1.4.6+.
  { valor: "sem-perguntar", nome: "Sem perguntar (recomendado)", desc: "Cria, edita e roda comandos sem pedir aprovação, inclusive os que apagam ou publicam. Use em pastas em que você confia." },
  { valor: "accept-edits", nome: "Arquivos sem perguntar", desc: "Cria e edita arquivos sem perguntar. Comandos pedem aprovação na conversa." },
  { valor: "auto", nome: "Automático", desc: "Também roda comandos seguros sem perguntar; pergunta antes do que não dá para desfazer. Depende da sua conta do Antigravity: sem ele, o app usa \"arquivos sem perguntar\"." },
  { valor: "padrao", nome: "Perguntar tudo", desc: "O padrão do Antigravity: pede aprovação para criar, editar e rodar." },
];

export function PermissoesDoAntigravity() {
  const [modo, setModo] = useState<Modo | null>(null);
  const [autoDisponivel, setAutoDisponivel] = useState(true);
  const [semPerguntarDisponivel, setSemPerguntarDisponivel] = useState(true);
  const [efetivo, setEfetivo] = useState<Modo | null>(null);
  const [liberados, setLiberados] = useState(0);
  const [comuns, setComuns] = useState<string[]>([]);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    request<{ modo: Modo; efetivo: Modo; auto_disponivel: boolean; sem_perguntar_disponivel?: boolean; comandos_liberados?: number; comandos_comuns?: string[] }>("/antigravity/permissoes")
      .then((dados) => {
        if (!vivo) return;
        setModo(dados.modo); setEfetivo(dados.efetivo); setAutoDisponivel(dados.auto_disponivel);
        setSemPerguntarDisponivel(dados.sem_perguntar_disponivel ?? true);
        setLiberados(dados.comandos_liberados ?? 0); setComuns(dados.comandos_comuns ?? []);
      })
      .catch(() => { if (vivo) setErro("Engine antigo: reinicie o app para escolher."); });
    return () => { vivo = false; };
  }, []);

  async function escolher(valor: Modo) {
    setErro(null);
    try {
      const dados = await request<{ modo: Modo; efetivo: Modo; auto_disponivel: boolean }>("/antigravity/permissoes", {
        method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ modo: valor }),
      });
      setModo(dados.modo);
      setEfetivo(dados.efetivo);
      setAutoDisponivel(dados.auto_disponivel);
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
    }
  }

  async function liberar(sim: boolean) {
    setErro(null);
    try {
      const dados = await request<{ comandos_liberados: number }>("/antigravity/permissoes", {
        method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ liberar_comuns: sim }),
      });
      setLiberados(dados.comandos_liberados);
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
    }
  }

  return (
    <section className="glass mt-4 rounded-none border border-border-subtle p-4" aria-labelledby="agy-permissoes">
      <h3 id="agy-permissoes" className="flex items-center gap-2 text-xs font-semibold text-text-primary">
        <AgentIcon provider="antigravity" size={14} />Antigravity: o que aprovar sozinho
      </h3>
      <p className="mt-1 text-[11px] text-text-secondary">
        Vale para as conversas que o app abre (as já abertas mudam ao reiniciar). O que ele ainda perguntar aparece na conversa para você aceitar.
      </p>
      <div role="radiogroup" aria-label="Modo do Antigravity" className="mt-3 flex flex-col gap-2">
        {OPCOES.map((opcao) => (
          <label key={opcao.valor} className="flex cursor-pointer items-start gap-2 text-xs">
            <input type="radio" name="agy-modo" className="mt-0.5" checked={modo === opcao.valor} disabled={modo === null}
              onChange={() => void escolher(opcao.valor)} />
            <span><span className="font-semibold text-text-primary">{opcao.nome}</span>
              <span className="block text-[11px] text-text-muted">{opcao.desc}</span></span>
          </label>
        ))}
      </div>
      {modo === "auto" && efetivo !== "auto" && !autoDisponivel && (
        <p className="mt-2 text-[11px] text-warning">
          Sua conta do Antigravity ainda não liberou o Automático: as conversas abrem com "{efetivo === "sem-perguntar" ? "sem perguntar" : "arquivos sem perguntar"}". Escolha Automático de novo para tentar outra vez.
        </p>
      )}
      {modo === "sem-perguntar" && !semPerguntarDisponivel && (
        <p className="mt-2 text-[11px] text-warning">
          Este Antigravity é anterior à opção de não perguntar: atualize o agy (agy update). Até lá, as conversas abrem com "arquivos sem perguntar".
        </p>
      )}
      <div className="mt-4 border-t border-border-subtle pt-3">
        <p className="text-xs font-semibold text-text-primary">Comandos de pouco risco sem perguntar</p>
        <p className="mt-1 text-[11px] text-text-secondary" title={comuns.join(", ")}>
          Leitura, versão, testes e build ({comuns.slice(0, 6).join(", ")}…). Fica no settings.json do Antigravity, então vale também fora do app.
          Apagar, mover, instalar, push e checkout continuam pedindo.
        </p>
        <div className="mt-2 flex items-center gap-2 text-[11px]">
          {liberados > 0 ? (
            <>
              <span className="text-success">{liberados} comando(s) liberado(s) pelo app</span>
              <button type="button" className="btn btn-ghost min-h-0 px-2 py-0.5 text-[11px]" onClick={() => void liberar(false)}>Desfazer</button>
            </>
          ) : (
            <button type="button" className="btn btn-ghost min-h-0 px-2 py-0.5 text-[11px]" onClick={() => void liberar(true)}>Liberar comandos comuns</button>
          )}
        </div>
      </div>
      {erro && <p role="alert" className="mt-2 text-[11px] text-danger">{erro}</p>}
    </section>
  );
}
