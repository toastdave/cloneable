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

type RecordingStep = {
  id: string;
  event_type: "click" | "key";
  timestamp_ms: number;
  full_screenshot_path: string | null;
  window_crop_path: string | null;
  window_crop_fallback: boolean;
  click_crop_path: string | null;
};

type RecordingSession = {
  session_id: string;
  started_at_ms: number;
  stopped_at_ms: number;
  click_events: ClickEvent[];
  key_events: KeyEvent[];
  steps: RecordingStep[];
};

type StopRecordingResult = {
  session_id: string;
  click_count: number;
  key_count: number;
  listener_error: string | null;
};

type DisplayStep = {
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

function buildDisplaySteps(session: RecordingSession): DisplayStep[] {
  return [...session.steps]
    .sort((left, right) => left.timestamp_ms - right.timestamp_ms)
    .map((step) => ({
      id: step.id,
      type: step.event_type,
      timestamp_ms: step.timestamp_ms,
      headline: step.event_type === "click" ? "Click" : "Key press",
      summary: step.event_type === "click" ? "Mouse click" : "Keyboard input",
      fullScreenshot: step.full_screenshot_path,
      windowScreenshot: step.window_crop_path,
      windowFallback: step.window_crop_fallback,
      clickScreenshot: step.click_crop_path,
    }));
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
      const result = await invoke<StopRecordingResult>("stop_recording");
      setRecordingId(result.session_id);
      setIsRecording(false);
      const session = await invoke<RecordingSession>("load_recording", {
        sessionId: result.session_id,
      });
      setLoadedSession(session);
      const steps = buildDisplaySteps(session);
      setSelectedStepId(steps[0]?.id ?? null);
      if (result.listener_error) {
        setErrorMessage(result.listener_error);
      } else if (result.click_count + result.key_count === 0) {
        setErrorMessage(
          "No input events were captured. On Wayland compositors like Hyprland, global input capture may be blocked. Try an X11 session or ensure your user has permission to read input devices.",
        );
      }
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  const steps = useMemo(
    () => (loadedSession ? buildDisplaySteps(loadedSession) : []),
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
                    {steps.length} steps · Session {loadedSession.session_id}
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
