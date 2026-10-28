// "Atualizar o PC" pelo celular. O PC confere o pedido em até ~15 s e faz o mesmo que o botão
// "Atualizar agora" da barra; quando o engine novo sobe, esta página volta sozinha com a versão nova.
import { useState } from "react";
import { request } from "../api";

export function AtualizarPc() {
  const [estado, setEstado] = useState<"ocioso" | "enviando" | "enviado" | "erro">("ocioso");

  async function pedir() {
    if (!window.confirm("Atualizar o Niski do PC agora? Os agentes que estiverem trabalhando reiniciam e voltam sozinhos.")) return;
    setEstado("enviando");
    try {
      const resposta = await request<{ ok: boolean }>("/pc/atualizar", { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" });
      setEstado(resposta?.ok ? "enviado" : "erro");
    } catch {
      setEstado("erro");
    }
  }

  return (
    <div className="m-atualizar-pc">
      <button type="button" className="btn btn-ghost" disabled={estado === "enviando"} onClick={() => void pedir()}>
        {estado === "enviando" ? "Pedindo…" : "Atualizar o PC"}
      </button>
      {estado === "enviado" && <p className="m-dica" role="status">Pedido enviado. O PC baixa a versão nova e reabre em instantes; esta página reconecta sozinha.</p>}
      {estado === "erro" && <p className="m-alerta" role="alert">Não consegui pedir a atualização. O PC está ligado e com o app aberto?</p>}
    </div>
  );
}
