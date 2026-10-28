import { createRoot } from "react-dom/client";
import { MobileApp } from "./MobileApp";
import "../index.css";
import "./pagina.css";
import "./mobile.css";
// Estilo Minimalista, quando o PC usa ele (`tema.ts` põe o data-estilo). Por último, como no PC.
import "../styles/minimalista.css";
// Estilo Vidro (só vale com data-variante="vidro"; depois do Minimalista, que ele reaproveita).
import "../styles/vidro.css";
// Símbolo do Niski conforme o tema.
import "../styles/marca.css";

createRoot(document.getElementById("root")!).render(<MobileApp />);
