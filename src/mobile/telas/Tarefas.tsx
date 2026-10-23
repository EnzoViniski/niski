// Fork (Tarefas no celular): as mesmas tarefas do PC (mesma pasta, pelo engine: `mobile/tarefas_rotas.rs`).
// O "eu" vem do computador; escolher pasta compartilhada e a IA das tarefas continuam no computador.
import { useEffect, useState } from "react";
import { request } from "../api";
import { adotarEu, usarTransporteDasTarefas, type TransporteDasTarefas } from "../../features/tarefas/loja";
import { normalizarPessoa } from "../../features/tarefas/dominio";
import { TarefasView } from "../../features/tarefas/TarefasView";
import "./tarefasNoCelular.css";

const peloEngine: TransporteDasTarefas = {
  listar: async (_pasta, tipo) => (await request<{ itens: string[] }>(`/tarefas/listar/${tipo}`)).itens,
  gravar: async (_pasta, tipo, id, conteudo) => {
    await request("/tarefas/gravar", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ tipo, id, conteudo }) });
  },
  levarPara: () => Promise.reject(new Error("A pasta compartilhada é escolhida no computador.")),
};
// Dentro do app do computador o transporte continua o do Tauri (esta tela é do celular/web).
if (typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) usarTransporteDasTarefas(peloEngine);

export function Tarefas({ voltar }: { voltar: () => void }) {
  const [pronto, setPronto] = useState<"carregando" | "ok" | "sem-eu" | "erro">("carregando");
  const [aviso, setAviso] = useState<string | null>(null);
  useEffect(() => {
    let vivo = true;
    request<{ eu: unknown }>("/tarefas/contexto")
      .then(({ eu }) => {
        if (!vivo) return;
        const pessoa = normalizarPessoa(eu);
        if (!pessoa?.id || !pessoa.nome) { setPronto("sem-eu"); return; }
        adotarEu(pessoa);
        setPronto("ok");
      })
      .catch(() => { if (vivo) setPronto("erro"); });
    return () => { vivo = false; };
  }, []);
  if (pronto === "carregando") return <p className="m-vazio">Abrindo as tarefas…</p>;
  if (pronto === "sem-eu") return <p className="m-vazio">Abra Tarefas no computador uma vez e diga seu nome; depois elas aparecem aqui.</p>;
  if (pronto === "erro") return <p className="m-vazio">Não consegui falar com o computador. Atualize o Niski no PC e tente de novo.</p>;
  return (
    <div className="tarefas-no-celular">
      {aviso && <p className="m-aviso" role="status">{aviso}</p>}
      <TarefasView onClose={voltar} onAbrirAjustes={() => setAviso("Os ajustes das tarefas ficam no computador (Configurações → Tarefas).")} />
    </div>
  );
}
