import icone from "../../assets/marca/medza-icone.png";

/** Marca do produto (ícone da Medza). Substitui o `OmniLogo` do upstream onde a marca aparece:
 *  barra de título, web do celular, pareamento. Proporção do arquivo: 412 × 287. */
export function MarcaDoProduto({ size, className, "aria-hidden": ariaHidden }: { size?: number; className?: string; "aria-hidden"?: boolean }) {
  return (
    <img src={icone} alt={ariaHidden ? "" : "Medza"} aria-hidden={ariaHidden} draggable={false}
      width={size} height={size ? Math.round((size * 287) / 412) : undefined}
      className={`object-contain ${className ?? ""}`} />
  );
}
