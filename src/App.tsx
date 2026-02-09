import { useState } from "react";
import "./App.css";

function App() {
  const [isRecording, setIsRecording] = useState(false);

  function handleStartRecording() {
    setIsRecording(true);
  }

  function handleStopRecording() {
    setIsRecording(false);
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
          </div>
        </div>

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
