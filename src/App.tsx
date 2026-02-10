import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [isRecording, setIsRecording] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [sessionPath, setSessionPath] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  async function toggleRecording() {
    setIsLoading(true);
    setErrorMsg(null);
    try {
      if (isRecording) {
        const path = await invoke<string>("stop_recording");
        setSessionPath(path);
        setIsRecording(false);
        console.log("[app] recording saved to:", path);
      } else {
        const id = await invoke<string>("start_recording");
        setSessionId(id);
        setSessionPath(null);
        setIsRecording(true);
        console.log("[app] recording started, session:", id);
      }
    } catch (error) {
      console.error("Recording toggle failed:", error);
      setErrorMsg(String(error));
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <main className="flex flex-col items-center justify-center gap-6 bg-gray-900 text-white min-h-dvh">
      <h1 className="text-3xl font-bold tracking-tight">Capture Any Workflow</h1>

      <div className="flex items-center gap-2 text-sm">
        <span
          className={`inline-block h-2.5 w-2.5 rounded-full ${
            isRecording ? "bg-red-500 animate-pulse" : "bg-gray-500"
          }`}
        />
        <span className="text-gray-300">
          {isRecording ? "Recording..." : "Idle"}
        </span>
      </div>

      <button
        onClick={toggleRecording}
        disabled={isLoading}
        className={`px-6 py-3 rounded-lg font-semibold text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
          isRecording
            ? "bg-red-600 hover:bg-red-700"
            : "bg-blue-600 hover:bg-blue-700"
        }`}
      >
        {isLoading
          ? "..."
          : isRecording
            ? "Stop Recording"
            : "Start Recording"}
      </button>

      {errorMsg && (
        <p className="text-red-400 text-xs max-w-md text-center">{errorMsg}</p>
      )}

      {sessionId && !isRecording && sessionPath && (
        <div className="text-xs text-gray-400 text-center max-w-md">
          <p>Session <span className="text-gray-200 font-mono">{sessionId}</span> saved.</p>
          <p className="truncate mt-1" title={sessionPath}>
            {sessionPath}
          </p>
        </div>
      )}
    </main>
  );
}

export default App;
