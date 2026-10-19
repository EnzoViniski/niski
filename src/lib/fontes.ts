/**
 * Fonte monoespaçada dos terminais. As do Windows primeiro; depois as do macOS e do Linux.
 *
 * No WKWebView do macOS o genérico `monospace` resolve para **Courier**, e Cascadia/Consolas não
 * existem lá: o terminal saía em Courier, e os caracteres de desenho de caixa da interface do Claude
 * Code caíam numa fonte de fallback com outra largura. Menlo e SF Mono vêm com o sistema.
 */
export const FONTE_MONO =
  'Cascadia Code, Consolas, ui-monospace, "SF Mono", Menlo, Monaco, "DejaVu Sans Mono", "Liberation Mono", monospace';
