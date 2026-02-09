import { useState } from "react";

import { invoke } from "@tauri-apps/api/core";

import "./App.css";

function App() {
  const [isRecording, setIsRecording] = useState(false);
  const [recordingId, setRecordingId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  function getErrorMessage(error: unknown) {
    if (error instanceof Error) {
      return error.message;
    }

    if (typeof error === "string") {
      return error;
    }

    return "Unable to update recording state.";
  }

  async function handleStartRecording() {
    setErrorMessage(null);
    try {
      const sessionId = await invoke<string>("start_recording");
      setRecordingId(sessionId);
      setIsRecording(true);
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  async function handleStopRecording() {
    setErrorMessage(null);
    try {
      const sessionId = await invoke<string>("stop_recording");
      setRecordingId(sessionId);
      setIsRecording(false);
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  return (
    <main className="app">
      <section className="panel">
        <header className="panel-header">
          <p className="eyebrow">Cloneable</p>
          <h1>Workflow Recorder</h1>
          <p className="subtitle">
            Capture your next workflow with a single click. When recording is
            active, global mouse and keyboard events will be saved locally.
          </p>
        </header>

        <div className="status">
          <span
            className={
              isRecording ? "status-dot status-dot--live" : "status-dot"
            }
          />
          <div>
            <p className="status-label">Recording status</p>
            <p className="status-value">
              {isRecording ? "Recording" : "Idle"}
            </p>
            {recordingId ? (
              <p className="status-meta">Session: {recordingId}</p>
            ) : null}
          </div>
        </div>

        {errorMessage ? (
          <p className="status-error" role="alert">
            {errorMessage}
          </p>
        ) : null}

        <div className="controls">
          <button
            className="btn btn-primary"
            onClick={handleStartRecording}
            disabled={isRecording}
            type="button"
          >
            Start recording
          </button>
          <button
            className="btn btn-ghost"
            onClick={handleStopRecording}
            disabled={!isRecording}
            type="button"
          >
            Stop recording
          </button>
        </div>

        <p className="footnote">
          Recordings are stored locally on this device. You can stop at any time
          to review the captured steps.
        </p>
      </section>
    </main>
  );
}

export default App;
