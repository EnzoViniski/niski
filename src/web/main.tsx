import { createRoot } from "react-dom/client";
import { WebApp } from "./WebApp";
import "../index.css";
import "../mobile/pagina.css";
import "../mobile/mobile.css";
import "./web.css";
// Estilo Minimalista também na web, quando o PC usa ele.
import "../styles/minimalista.css";
// Estilo Vidro (só vale com data-variante="vidro"; depois do Minimalista, que ele reaproveita).
import "../styles/vidro.css";
// Símbolo do Niski conforme o tema.
import "../styles/marca.css";

createRoot(document.getElementById("root")!).render(<WebApp />);
