import simbolo from "../../assets/marca/niski-simbolo.png";
import simboloPreto from "../../assets/marca/niski-simbolo-preto.png";
import { NOME_DO_PRODUTO } from "../../lib/identidade";

/** Marca do produto (símbolo do Niski). Substitui o `OmniLogo` do upstream onde a marca aparece:
 *  barra de título, web do celular, pareamento. Quadrado.
 *
 *  Fork (Niski, 21/10/2026): o símbolo com brilho só fica bonito no escuro; no tema claro entra a
 *  versão preta chapada. As duas vão no DOM e o CSS (`fork.css`, `data-theme`) mostra uma — assim
 *  "Sistema" trocando de claro para escuro não precisa de re-render. */
export function MarcaDoProduto({ size, className, "aria-hidden": ariaHidden }: { size?: number; className?: string; "aria-hidden"?: boolean }) {
  const comum = { width: size, height: size, draggable: false, className: `object-contain ${className ?? ""}` };
  return (
    <>
      <img {...comum} src={simbolo} alt={ariaHidden ? "" : NOME_DO_PRODUTO} aria-hidden={ariaHidden}
        className={`${comum.className} marca-no-escuro`} />
      <img {...comum} src={simboloPreto} alt="" aria-hidden className={`${comum.className} marca-no-claro`} />
    </>
  );
}
