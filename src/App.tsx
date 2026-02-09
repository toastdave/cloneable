import { useMemo, useState } from "react";

import { invoke } from "@tauri-apps/api/core";

import "./App.css";

type ClickEvent = {
  timestamp_ms: number;
  x: number;
  y: number;
  full_screenshot_path: string | null;
  full_screenshot_error: string | null;
  window_crop_path: string | null;
  window_crop_error: string | null;
  window_crop_fallback: boolean;
  click_crop_path: string | null;
  click_crop_error: string | null;
};

type KeyEvent = {
  timestamp_ms: number;
  key: string | null;
  text: string | null;
  full_screenshot_path: string | null;
  full_screenshot_error: string | null;
  window_crop_path: string | null;
  window_crop_error: string | null;
  window_crop_fallback: boolean;
};

type RecordingSession = {
  session_id: string;
  started_at_ms: number;
  stopped_at_ms: number;
  click_events: ClickEvent[];
  key_events: KeyEvent[];
};

type Step = {
  id: string;
  type: "click" | "key";
  timestamp_ms: number;
  headline: string;
  summary: string;
  fullScreenshot: string | null;
  windowScreenshot: string | null;
  windowFallback: boolean;
  clickScreenshot: string | null;
};

function buildSteps(session: RecordingSession): Step[] {
  const clickSteps = session.click_events.map((event, index) => ({
    id: `click-${event.timestamp_ms}-${index}`,
    type: "click" as const,
    timestamp_ms: event.timestamp_ms,
    headline: `Click at ${event.x.toFixed(0)}, ${event.y.toFixed(0)}`,
    summary: "Mouse click",
    fullScreenshot: event.full_screenshot_path,
    windowScreenshot: event.window_crop_path,
    windowFallback: event.window_crop_fallback,
    clickScreenshot: event.click_crop_path,
  }));
  const keySteps = session.key_events.map((event, index) => ({
    id: `key-${event.timestamp_ms}-${index}`,
    type: "key" as const,
    timestamp_ms: event.timestamp_ms,
    headline: event.key ? `Key ${event.key}` : "Key press",
    summary: "Keyboard input",
    fullScreenshot: event.full_screenshot_path,
    windowScreenshot: event.window_crop_path,
    windowFallback: event.window_crop_fallback,
    clickScreenshot: null,
  }));

  return [...clickSteps, ...keySteps].sort(
    (left, right) => left.timestamp_ms - right.timestamp_ms,
  );
}

function formatTimestamp(timestampMs: number) {
  return new Date(timestampMs).toLocaleTimeString();
}

function App() {
  const [isRecording, setIsRecording] = useState(false);
  const [recordingId, setRecordingId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [loadedSession, setLoadedSession] = useState<RecordingSession | null>(
    null,
  );
  const [selectedStepId, setSelectedStepId] = useState<string | null>(null);

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
      setLoadedSession(null);
      setSelectedStepId(null);
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
      const session = await invoke<RecordingSession>("load_recording", {
        sessionId,
      });
      setLoadedSession(session);
      const steps = buildSteps(session);
      setSelectedStepId(steps[0]?.id ?? null);
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  const steps = useMemo(
    () => (loadedSession ? buildSteps(loadedSession) : []),
    [loadedSession],
  );
  const selectedStep = steps.find((step) => step.id === selectedStepId) ?? null;

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

        {loadedSession ? (
          <section className="review">
            <header className="review-header">
              <div>
                <p className="review-label">Captured steps</p>
                <p className="review-meta">
                  {steps.length} events · Session {loadedSession.session_id}
                </p>
              </div>
              <div className="review-timing">
                <p className="review-label">Recorded</p>
                <p className="review-meta">
                  {formatTimestamp(loadedSession.started_at_ms)} →{" "}
                  {formatTimestamp(loadedSession.stopped_at_ms)}
                </p>
              </div>
            </header>

            <div className="review-body">
              <div className="steps-list">
                {steps.map((step, index) => (
                  <button
                    key={step.id}
                    className={
                      step.id === selectedStepId
                        ? "step-card step-card--active"
                        : "step-card"
                    }
                    onClick={() => setSelectedStepId(step.id)}
                    type="button"
                  >
                    <div>
                      <p className="step-index">Step {index + 1}</p>
                      <p className="step-title">{step.headline}</p>
                      <p className="step-meta">
                        {step.type.toUpperCase()} ·{" "}
                        {formatTimestamp(step.timestamp_ms)}
                      </p>
                    </div>
                    <span className="step-tag">{step.summary}</span>
                  </button>
                ))}
              </div>

              <div className="step-details">
                {selectedStep ? (
                  <div>
                    <p className="detail-title">Step details</p>
                    <p className="detail-subtitle">{selectedStep.headline}</p>
                    <div className="detail-grid">
                      <div>
                        <p className="detail-label">Type</p>
                        <p className="detail-value">
                          {selectedStep.type.toUpperCase()}
                        </p>
                      </div>
                      <div>
                        <p className="detail-label">Timestamp</p>
                        <p className="detail-value">
                          {formatTimestamp(selectedStep.timestamp_ms)}
                        </p>
                      </div>
                      <div>
                        <p className="detail-label">Full screenshot</p>
                        <p className="detail-value">
                          {selectedStep.fullScreenshot ?? "Unavailable"}
                        </p>
                      </div>
                      <div>
                        <p className="detail-label">Window screenshot</p>
                        <p className="detail-value">
                          {selectedStep.windowScreenshot ?? "Unavailable"}
                        </p>
                        {selectedStep.windowFallback ? (
                          <p className="detail-note">
                            Window capture fell back to full screen.
                          </p>
                        ) : null}
                      </div>
                      {selectedStep.type === "click" ? (
                        <div>
                          <p className="detail-label">Click crop</p>
                          <p className="detail-value">
                            {selectedStep.clickScreenshot ?? "Unavailable"}
                          </p>
                        </div>
                      ) : null}
                    </div>
                  </div>
                ) : (
                  <p className="detail-empty">Select a step to inspect it.</p>
                )}
              </div>
            </div>
          </section>
        ) : null}
      </section>
    </main>
  );
}

export default App;
