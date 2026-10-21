import { createRoot } from "react-dom/client";
import { WebApp } from "./WebApp";
import "../index.css";
import "../mobile/pagina.css";
import "../mobile/mobile.css";
import "./web.css";
// Fork: estilo Minimalista também na web, quando o PC usa ele.
import "../styles/minimalista.css";
// Fork: estilo Vidro (só vale com data-variante="vidro"; depois do Minimalista, que ele reaproveita).
import "../styles/vidro.css";
// Fork: símbolo do Niski conforme o tema.
import "../styles/marca.css";

createRoot(document.getElementById("root")!).render(<WebApp />);
