import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

function App() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<string>("audio-error", (e) => setError(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

    async function playTestFile() {
    setError(null);
    setBusy(true);
    try {
      await invoke("play_test_file");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }


  return (
    <main className="container">
      <h1>Reproductor</h1>

      <div className="row">
        <button onClick={playTestFile} disabled={busy}>
          Reproducir archivo de prueba
        </button>
      </div>
      {error &&<p className="error">{error}</p>}
    </main>
  );
}

export default App;
