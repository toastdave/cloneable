import { useEffect, useMemo, useState } from "react";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
  input_text?: string | null;
  title?: string | null;
  description?: string | null;
  action_type?: "click" | "type" | "wait" | "assert" | null;
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

type RecordingShortcutPayload = {
  is_recording: boolean;
  session_id: string | null;
  listener_error: string | null;
  error: string | null;
};

type DisplayStep = {
  id: string;
  type: "click" | "key";
  actionType: "click" | "type" | "wait" | "assert";
  timestamp_ms: number;
  headline: string;
  summary: string;
  title: string;
  description: string;
  fullScreenshot: string | null;
  windowScreenshot: string | null;
  windowFallback: boolean;
  clickScreenshot: string | null;
};

function buildDisplaySteps(session: RecordingSession): DisplayStep[] {
  return [...session.steps]
    .sort((left, right) => left.timestamp_ms - right.timestamp_ms)
    .map((step) => {
      const hasInputText = Boolean(step.input_text?.trim().length);
      const fallbackHeadline = step.event_type === "click" ? "Click" : "Typing";
      const fallbackSummary =
        step.event_type === "click"
          ? "Mouse click"
          : hasInputText
            ? (step.input_text ?? "")
            : "Keyboard input";
      const fallbackActionType = step.event_type === "click" ? "click" : "type";
      const title = step.title ?? "";
      const description = step.description ?? "";
      return {
        id: step.id,
        type: step.event_type,
        actionType: step.action_type ?? fallbackActionType,
        timestamp_ms: step.timestamp_ms,
        headline: title.trim().length ? title : fallbackHeadline,
        summary: description.trim().length ? description : fallbackSummary,
        title,
        description,
        fullScreenshot: step.full_screenshot_path,
        windowScreenshot: step.window_crop_path,
        windowFallback: step.window_crop_fallback,
        clickScreenshot: step.click_crop_path,
      };
    });
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
  const [draftTitle, setDraftTitle] = useState("");
  const [draftDescription, setDraftDescription] = useState("");
  const [draftActionType, setDraftActionType] = useState<
    "click" | "type" | "wait" | "assert"
  >("click");
  const [isSaving, setIsSaving] = useState(false);

  function getErrorMessage(error: unknown) {
    if (error instanceof Error) {
      return error.message;
    }

    if (typeof error === "string") {
      return error;
    }

    return "Unable to update recording state.";
  }

  async function loadRecordingSession(
    sessionId: string,
    listenerError?: string | null,
  ) {
    const session = await invoke<RecordingSession>("load_recording", {
      sessionId,
    });
    setLoadedSession(session);
    const nextSteps = buildDisplaySteps(session);
    setSelectedStepId(nextSteps[0]?.id ?? null);
    if (listenerError) {
      setErrorMessage(listenerError);
    } else if (nextSteps.length === 0) {
      setErrorMessage(
        "No input events were captured. On Wayland compositors like Hyprland, global input capture may be blocked. Try an X11 session or ensure your user has permission to read input devices.",
      );
    }
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
      await loadRecordingSession(result.session_id, result.listener_error);
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  const steps = useMemo(
    () => (loadedSession ? buildDisplaySteps(loadedSession) : []),
    [loadedSession],
  );
  const selectedStep = steps.find((step) => step.id === selectedStepId) ?? null;

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<RecordingShortcutPayload>("recording-shortcut", (event) => {
      const payload = event.payload;
      if (payload.error) {
        setErrorMessage(payload.error);
        return;
      }
      if (payload.is_recording) {
        setIsRecording(true);
        setRecordingId(payload.session_id ?? null);
        setLoadedSession(null);
        setSelectedStepId(null);
        setErrorMessage(null);
        return;
      }
      setIsRecording(false);
      if (payload.session_id) {
        setRecordingId(payload.session_id);
        loadRecordingSession(payload.session_id, payload.listener_error).catch(
          (error) => setErrorMessage(getErrorMessage(error)),
        );
      }
    })
      .then((unlistenFn) => {
        unlisten = unlistenFn;
      })
      .catch(() => {
        unlisten = null;
      });
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    if (!selectedStep) {
      setDraftTitle("");
      setDraftDescription("");
      setDraftActionType("click");
      return;
    }
    setDraftTitle(selectedStep.title ?? "");
    setDraftDescription(selectedStep.description ?? "");
    setDraftActionType(selectedStep.actionType);
  }, [selectedStep]);

  const hasAnnotationChanges =
    selectedStep !== null &&
    (draftTitle !== selectedStep.title ||
      draftDescription !== selectedStep.description ||
      draftActionType !== selectedStep.actionType);

  async function handleSaveAnnotations() {
    if (!loadedSession || !selectedStep) {
      return;
    }
    setErrorMessage(null);
    setIsSaving(true);
    try {
      const updatedStep = await invoke<RecordingStep>("update_step_annotations", {
        sessionId: loadedSession.session_id,
        stepId: selectedStep.id,
        title: draftTitle,
        description: draftDescription,
        actionType: draftActionType,
      });
      setLoadedSession((previous) => {
        if (!previous) {
          return previous;
        }
        const updatedSteps = previous.steps.map((step) =>
          step.id === updatedStep.id
            ? {
                ...step,
                title: updatedStep.title ?? null,
                description: updatedStep.description ?? null,
                action_type: updatedStep.action_type ?? draftActionType,
              }
            : step,
        );
        return {
          ...previous,
          steps: updatedSteps,
        };
      });
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSaving(false);
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
          to review the captured steps. Shortcut: Cmd/Ctrl + Shift + R.
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
                        <p className="detail-label">Action type</p>
                        <p className="detail-value">
                          {selectedStep.actionType.toUpperCase()}
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
                    <div className="detail-form">
                      <label className="detail-field">
                        <span className="detail-label">Title</span>
                        <input
                          className="detail-input"
                          onChange={(event) => setDraftTitle(event.target.value)}
                          placeholder="Add a short title"
                          type="text"
                          value={draftTitle}
                        />
                      </label>
                      <label className="detail-field">
                        <span className="detail-label">Action type</span>
                        <select
                          className="detail-select"
                          onChange={(event) =>
                            setDraftActionType(
                              event.currentTarget.value as
                                | "click"
                                | "type"
                                | "wait"
                                | "assert",
                            )
                          }
                          value={draftActionType}
                        >
                          <option value="click">Click</option>
                          <option value="type">Type</option>
                          <option value="wait">Wait</option>
                          <option value="assert">Assert</option>
                        </select>
                      </label>
                      <label className="detail-field">
                        <span className="detail-label">Description</span>
                        <textarea
                          className="detail-textarea"
                          onChange={(event) =>
                            setDraftDescription(event.target.value)
                          }
                          placeholder="Add supporting details"
                          rows={4}
                          value={draftDescription}
                        />
                      </label>
                      <div className="detail-actions">
                        <button
                          className="btn btn-secondary"
                          disabled={!hasAnnotationChanges || isSaving}
                          onClick={handleSaveAnnotations}
                          type="button"
                        >
                          {isSaving ? "Saving" : "Save annotations"}
                        </button>
                      </div>
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
