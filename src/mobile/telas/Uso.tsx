// Fork: uso das contas no celular (janela de 5 h e semanal), o mesmo painel da web (`UsoDasContas`)
// com medidas de toque. O PC é quem lê o uso; o celular só pede para reler.
import { UsoDasContas } from "../../web/UsoDasContas";
import "./uso.css";

export function Uso() {
  return (
    <section className="m-uso" aria-label="Uso das contas">
      <UsoDasContas contaAtiva={null}
        vazio={<p className="m-vazio">Nenhuma conta de Claude ou Codex no PC ainda.</p>} />
      <p className="m-dica">O PC relê o uso de tempos em tempos. "Atualizar" pede uma leitura nova agora (no Claude leva alguns segundos).</p>
    </section>
  );
}
