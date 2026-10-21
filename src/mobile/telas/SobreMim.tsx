// Fork (Sobre mim): a mesma tela do PC, lida e editada pelo celular pareado (rotas `/sobre-mim`).
import { request } from "../api";
import { SobreMim as TelaDoSobreMim } from "../../features/sobreMim/SobreMim";
import type { ChamarSobreMim, EstadoDoSobreMim } from "../../features/sobreMim/apiDoSobreMim";

const chamarPeloCelular: ChamarSobreMim = (metodo, caminho, corpo) => request<EstadoDoSobreMim>(caminho, metodo === "GET" ? undefined : {
  method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(corpo ?? {}),
});

export function SobreMim() {
  return <TelaDoSobreMim chamar={chamarPeloCelular} />;
}
