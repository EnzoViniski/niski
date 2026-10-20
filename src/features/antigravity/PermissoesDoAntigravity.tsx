import { useEffect, useState } from "react";
import { request } from "../../mobile/api";
import { AgentIcon } from "../../components/ui/AgentIcon";

/**
 * Fork: o que o Antigravity aprova sozinho nas conversas abertas pelo app (`--mode` do `agy`, só nessas
 * sessões; a config global do agy não muda). O que ele ainda perguntar aparece na conversa, com
 * Permitir/Negar, como no Claude e no Codex.
 */
type Modo = "auto" | "accept-edits" | "padrao";

const OPCOES: { valor: Modo; nome: string; desc: string }[] = [
  { valor: "accept-edits", nome: "Arquivos sem perguntar (recomendado)", desc: "Cria e edita arquivos sem perguntar. Comandos pedem aprovação na conversa." },
  { valor: "auto", nome: "Automático", desc: "Também roda comandos seguros sem perguntar; pergunta antes do que não dá para desfazer. Depende da sua conta do Antigravity: sem ele, o app usa \"arquivos sem perguntar\"." },
  { valor: "padrao", nome: "Perguntar tudo", desc: "O padrão do Antigravity: pede aprovação para criar, editar e rodar." },
];

export function PermissoesDoAntigravity() {
  const [modo, setModo] = useState<Modo | null>(null);
  const [autoDisponivel, setAutoDisponivel] = useState(true);
  const [efetivo, setEfetivo] = useState<Modo | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    request<{ modo: Modo; efetivo: Modo; auto_disponivel: boolean }>("/antigravity/permissoes")
      .then((dados) => { if (vivo) { setModo(dados.modo); setEfetivo(dados.efetivo); setAutoDisponivel(dados.auto_disponivel); } })
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
          Sua conta do Antigravity ainda não liberou o Automático: as conversas abrem com "arquivos sem perguntar". Escolha Automático de novo para tentar outra vez.
        </p>
      )}
      {erro && <p role="alert" className="mt-2 text-[11px] text-danger">{erro}</p>}
    </section>
  );
}
