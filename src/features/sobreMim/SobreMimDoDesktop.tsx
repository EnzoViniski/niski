// Fork (Sobre mim): a tela nas Configurações do desktop, pela API local do engine.
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { SobreMim } from "./SobreMim";
import { chamarPeloEngine } from "./apiDoSobreMim";

export function SobreMimDoDesktop() {
  return <SobreMim chamar={chamarPeloEngine} abrirPasta={(pasta) => void revealItemInDir(pasta).catch(() => undefined)} />;
}
