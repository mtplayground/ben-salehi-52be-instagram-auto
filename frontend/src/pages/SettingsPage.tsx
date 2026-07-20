import { CalendarClock, Save, SlidersHorizontal } from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";

import {
  ContentSettings,
  PostingSchedule,
  fetchContentSettings,
  fetchPostingSchedule,
  saveContentSettings,
  savePostingSchedule,
} from "../settings/api";

const emptyForm = {
  theme_topic: "",
  style_notes: "",
};

const weekdayOptions = [
  { value: 1, label: "Mon" },
  { value: 2, label: "Tue" },
  { value: 3, label: "Wed" },
  { value: 4, label: "Thu" },
  { value: 5, label: "Fri" },
  { value: 6, label: "Sat" },
  { value: 7, label: "Sun" },
];

type ScheduleForm = {
  timezone: string;
  cadence: "daily" | "weekly";
  time_of_day: string;
  weekdays: number[];
  is_active: boolean;
};

function emptyScheduleForm(): ScheduleForm {
  return {
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
    cadence: "daily",
    time_of_day: "09:00",
    weekdays: [1, 3, 5],
    is_active: true,
  };
}

export function SettingsPage() {
  const [form, setForm] = useState(emptyForm);
  const [saved, setSaved] = useState<ContentSettings | null>(null);
  const [contentLoading, setContentLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [scheduleForm, setScheduleForm] = useState<ScheduleForm>(() =>
    emptyScheduleForm(),
  );
  const [savedSchedule, setSavedSchedule] = useState<PostingSchedule | null>(
    null,
  );
  const [scheduleLoading, setScheduleLoading] = useState(true);
  const [scheduleSaving, setScheduleSaving] = useState(false);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const [scheduleNotice, setScheduleNotice] = useState<string | null>(null);

  useEffect(() => {
    let active = true;

    async function loadSettings() {
      setContentLoading(true);
      setScheduleLoading(true);
      setError(null);
      setScheduleError(null);

      const [contentResult, scheduleResult] = await Promise.allSettled([
        fetchContentSettings(),
        fetchPostingSchedule(),
      ]);

      if (!active) {
        return;
      }

      if (contentResult.status === "fulfilled") {
        setSaved(contentResult.value);
        setForm(
          contentResult.value
            ? contentSettingsToForm(contentResult.value)
            : emptyForm,
        );
      } else {
        setError(
          contentResult.reason instanceof Error
            ? contentResult.reason.message
            : "Settings could not be loaded",
        );
      }

      if (scheduleResult.status === "fulfilled") {
        setSavedSchedule(scheduleResult.value);
        setScheduleForm(
          scheduleResult.value
            ? postingScheduleToForm(scheduleResult.value)
            : emptyScheduleForm(),
        );
      } else {
        setScheduleError(
          scheduleResult.reason instanceof Error
            ? scheduleResult.reason.message
            : "Schedule could not be loaded",
        );
      }

      setContentLoading(false);
      setScheduleLoading(false);
    }

    void loadSettings();

    return () => {
      active = false;
    };
  }, []);

  const isDirty = useMemo(() => {
    const savedTheme = saved?.theme_topic ?? "";
    const savedStyle = saved?.style_notes ?? "";
    return form.theme_topic !== savedTheme || form.style_notes !== savedStyle;
  }, [form, saved]);

  const isScheduleDirty = useMemo(() => {
    if (!savedSchedule) {
      return true;
    }

    const savedForm = postingScheduleToForm(savedSchedule);
    return (
      scheduleForm.timezone !== savedForm.timezone ||
      scheduleForm.cadence !== savedForm.cadence ||
      scheduleForm.time_of_day !== savedForm.time_of_day ||
      scheduleForm.is_active !== savedForm.is_active ||
      !sameWeekdays(scheduleForm.weekdays, savedForm.weekdays)
    );
  }, [scheduleForm, savedSchedule]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    setNotice(null);

    try {
      const settings = await saveContentSettings(form);
      setSaved(settings);
      setForm({
        theme_topic: settings.theme_topic,
        style_notes: settings.style_notes,
      });
      setNotice("Content settings saved.");
    } catch (caught) {
      setError(
        caught instanceof Error
          ? caught.message
          : "Settings could not be saved",
      );
    } finally {
      setSaving(false);
    }
  }

  async function handleScheduleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setScheduleSaving(true);
    setScheduleError(null);
    setScheduleNotice(null);

    try {
      const schedule = await savePostingSchedule(scheduleForm);
      setSavedSchedule(schedule);
      setScheduleForm(postingScheduleToForm(schedule));
      setScheduleNotice("Posting schedule saved.");
    } catch (caught) {
      setScheduleError(
        caught instanceof Error
          ? caught.message
          : "Schedule could not be saved",
      );
    } finally {
      setScheduleSaving(false);
    }
  }

  function setCadence(cadence: "daily" | "weekly") {
    setScheduleForm((current) => ({
      ...current,
      cadence,
      weekdays: cadence === "daily" ? [1, 2, 3, 4, 5, 6, 7] : current.weekdays,
    }));
  }

  function toggleWeekday(day: number) {
    setScheduleForm((current) => {
      const hasDay = current.weekdays.includes(day);
      const weekdays = hasDay
        ? current.weekdays.filter((value) => value !== day)
        : [...current.weekdays, day].sort((left, right) => left - right);

      return {
        ...current,
        weekdays: weekdays.length > 0 ? weekdays : current.weekdays,
      };
    });
  }

  const contentDisabled = contentLoading || saving;
  const scheduleDisabled = scheduleLoading || scheduleSaving;

  return (
    <section className="space-y-6">
      <div>
        <p className="text-sm font-semibold uppercase tracking-wide text-meadow">
          Settings
        </p>
        <h2 className="mt-3 text-3xl font-bold">Creator content</h2>
        <p className="mt-3 max-w-2xl text-base leading-7 text-ink/70">
          Define the topic and visual voice that generated Instagram posts
          should follow for this creator account.
        </p>
      </div>

      <div className="grid gap-5 lg:grid-cols-[1fr_320px]">
        <form
          onSubmit={handleSubmit}
          className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel"
        >
          <div className="flex items-start gap-4">
            <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-coral/12 text-coral">
              <SlidersHorizontal size={22} aria-hidden="true" />
            </span>
            <div className="min-w-0 flex-1 space-y-5">
              <div>
                <label
                  htmlFor="theme_topic"
                  className="text-sm font-semibold text-ink"
                >
                  Theme or topic
                </label>
                <input
                  id="theme_topic"
                  name="theme_topic"
                  type="text"
                  minLength={3}
                  maxLength={180}
                  required
                  value={form.theme_topic}
                  disabled={contentDisabled}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      theme_topic: event.target.value,
                    }))
                  }
                  className="mt-2 w-full rounded-md border border-ink/15 bg-white px-3 py-2 text-sm text-ink outline-none transition focus:border-meadow focus:ring-2 focus:ring-meadow/20 disabled:bg-ink/5"
                  placeholder="Mindful fitness for busy parents"
                />
              </div>

              <div>
                <label
                  htmlFor="style_notes"
                  className="text-sm font-semibold text-ink"
                >
                  Style notes
                </label>
                <textarea
                  id="style_notes"
                  name="style_notes"
                  minLength={3}
                  maxLength={1200}
                  required
                  rows={8}
                  value={form.style_notes}
                  disabled={contentDisabled}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      style_notes: event.target.value,
                    }))
                  }
                  className="mt-2 w-full resize-y rounded-md border border-ink/15 bg-white px-3 py-2 text-sm leading-6 text-ink outline-none transition focus:border-meadow focus:ring-2 focus:ring-meadow/20 disabled:bg-ink/5"
                  placeholder="Warm, practical, encouraging, simple illustrations, plain language."
                />
              </div>

              {error ? (
                <div className="rounded-md border border-coral/30 bg-coral/10 px-3 py-2 text-sm text-ink">
                  {error}
                </div>
              ) : null}

              {notice ? (
                <div className="rounded-md border border-meadow/25 bg-meadow/10 px-3 py-2 text-sm text-meadow">
                  {notice}
                </div>
              ) : null}

              <div className="flex items-center justify-between gap-3">
                <p className="text-sm text-ink/55">
                  {saved
                    ? `Last saved ${formatDate(saved.updated_at)}`
                    : "No settings saved yet"}
                </p>
                <button
                  type="submit"
                  disabled={contentDisabled || !isDirty}
                  className="inline-flex items-center gap-2 rounded-md bg-meadow px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-meadow/90 disabled:cursor-not-allowed disabled:bg-ink/25"
                >
                  <Save size={17} aria-hidden="true" />
                  {saving ? "Saving" : "Save"}
                </button>
              </div>
            </div>
          </div>
        </form>

        <aside className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
          <h3 className="text-lg font-semibold">Current direction</h3>
          <dl className="mt-5 space-y-4">
            <PreviewRow label="Theme" value={form.theme_topic || "Unset"} />
            <PreviewRow label="Style" value={form.style_notes || "Unset"} />
          </dl>
        </aside>
      </div>

      <div className="grid gap-5 lg:grid-cols-[1fr_320px]">
        <form
          onSubmit={handleScheduleSubmit}
          className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel"
        >
          <div className="flex items-start gap-4">
            <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-sky/12 text-sky">
              <CalendarClock size={22} aria-hidden="true" />
            </span>
            <div className="min-w-0 flex-1 space-y-5">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <h3 className="text-lg font-semibold">Posting schedule</h3>
                  <p className="mt-1 text-sm text-ink/60">
                    {savedSchedule
                      ? `Last saved ${formatDate(savedSchedule.updated_at)}`
                      : "No schedule saved yet"}
                  </p>
                </div>
                <label className="inline-flex items-center gap-2 text-sm font-semibold text-ink">
                  <input
                    type="checkbox"
                    checked={scheduleForm.is_active}
                    disabled={scheduleDisabled}
                    onChange={(event) =>
                      setScheduleForm((current) => ({
                        ...current,
                        is_active: event.target.checked,
                      }))
                    }
                    className="h-4 w-4 rounded border-ink/20 text-meadow focus:ring-meadow"
                  />
                  Active
                </label>
              </div>

              <div>
                <span className="text-sm font-semibold text-ink">
                  Frequency
                </span>
                <div className="mt-2 grid grid-cols-2 gap-2 rounded-lg bg-ink/5 p-1">
                  {(["daily", "weekly"] as const).map((cadence) => (
                    <button
                      key={cadence}
                      type="button"
                      disabled={scheduleDisabled}
                      onClick={() => setCadence(cadence)}
                      className={[
                        "rounded-md px-3 py-2 text-sm font-semibold capitalize transition",
                        scheduleForm.cadence === cadence
                          ? "bg-white text-meadow shadow-sm"
                          : "text-ink/65 hover:text-ink",
                      ].join(" ")}
                    >
                      {cadence}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <div>
                  <label
                    htmlFor="time_of_day"
                    className="text-sm font-semibold text-ink"
                  >
                    Posting time
                  </label>
                  <input
                    id="time_of_day"
                    name="time_of_day"
                    type="time"
                    required
                    value={scheduleForm.time_of_day}
                    disabled={scheduleDisabled}
                    onChange={(event) =>
                      setScheduleForm((current) => ({
                        ...current,
                        time_of_day: event.target.value,
                      }))
                    }
                    className="mt-2 w-full rounded-md border border-ink/15 bg-white px-3 py-2 text-sm text-ink outline-none transition focus:border-meadow focus:ring-2 focus:ring-meadow/20 disabled:bg-ink/5"
                  />
                </div>

                <div>
                  <label
                    htmlFor="timezone"
                    className="text-sm font-semibold text-ink"
                  >
                    Timezone
                  </label>
                  <input
                    id="timezone"
                    name="timezone"
                    type="text"
                    required
                    maxLength={64}
                    value={scheduleForm.timezone}
                    disabled={scheduleDisabled}
                    onChange={(event) =>
                      setScheduleForm((current) => ({
                        ...current,
                        timezone: event.target.value,
                      }))
                    }
                    className="mt-2 w-full rounded-md border border-ink/15 bg-white px-3 py-2 text-sm text-ink outline-none transition focus:border-meadow focus:ring-2 focus:ring-meadow/20 disabled:bg-ink/5"
                    placeholder="America/Los_Angeles"
                  />
                </div>
              </div>

              {scheduleForm.cadence === "weekly" ? (
                <div>
                  <span className="text-sm font-semibold text-ink">
                    Posting days
                  </span>
                  <div className="mt-2 grid grid-cols-4 gap-2 sm:grid-cols-7">
                    {weekdayOptions.map((day) => {
                      const selected = scheduleForm.weekdays.includes(
                        day.value,
                      );
                      return (
                        <button
                          key={day.value}
                          type="button"
                          disabled={scheduleDisabled}
                          onClick={() => toggleWeekday(day.value)}
                          className={[
                            "rounded-md border px-2 py-2 text-sm font-semibold transition",
                            selected
                              ? "border-meadow bg-meadow text-white"
                              : "border-ink/15 bg-white text-ink/65 hover:text-ink",
                          ].join(" ")}
                        >
                          {day.label}
                        </button>
                      );
                    })}
                  </div>
                </div>
              ) : null}

              {scheduleError ? (
                <div className="rounded-md border border-coral/30 bg-coral/10 px-3 py-2 text-sm text-ink">
                  {scheduleError}
                </div>
              ) : null}

              {scheduleNotice ? (
                <div className="rounded-md border border-meadow/25 bg-meadow/10 px-3 py-2 text-sm text-meadow">
                  {scheduleNotice}
                </div>
              ) : null}

              <div className="flex justify-end">
                <button
                  type="submit"
                  disabled={scheduleDisabled || !isScheduleDirty}
                  className="inline-flex items-center gap-2 rounded-md bg-meadow px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-meadow/90 disabled:cursor-not-allowed disabled:bg-ink/25"
                >
                  <Save size={17} aria-hidden="true" />
                  {scheduleSaving ? "Saving" : "Save schedule"}
                </button>
              </div>
            </div>
          </div>
        </form>

        <aside className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
          <h3 className="text-lg font-semibold">Schedule summary</h3>
          <dl className="mt-5 space-y-4">
            <PreviewRow
              label="Status"
              value={scheduleForm.is_active ? "Active" : "Paused"}
            />
            <PreviewRow
              label="Frequency"
              value={scheduleSummary(scheduleForm)}
            />
            <PreviewRow
              label="Next run"
              value={formatOptionalDate(savedSchedule?.next_run_at)}
            />
          </dl>
        </aside>
      </div>
    </section>
  );
}

function PreviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs font-semibold uppercase tracking-wide text-ink/45">
        {label}
      </dt>
      <dd className="mt-1 whitespace-pre-wrap text-sm leading-6 text-ink/75">
        {value}
      </dd>
    </div>
  );
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function formatOptionalDate(value: string | null | undefined) {
  return value ? formatDate(value) : "Not scheduled";
}

function contentSettingsToForm(settings: ContentSettings) {
  return {
    theme_topic: settings.theme_topic,
    style_notes: settings.style_notes,
  };
}

function postingScheduleToForm(schedule: PostingSchedule): ScheduleForm {
  return {
    timezone: schedule.timezone,
    cadence: schedule.cadence,
    time_of_day: schedule.time_of_day,
    weekdays: schedule.weekdays,
    is_active: schedule.is_active,
  };
}

function sameWeekdays(left: number[], right: number[]) {
  if (left.length !== right.length) {
    return false;
  }

  const sortedLeft = [...left].sort((a, b) => a - b);
  const sortedRight = [...right].sort((a, b) => a - b);
  return sortedLeft.every((value, index) => value === sortedRight[index]);
}

function scheduleSummary(schedule: ScheduleForm) {
  if (schedule.cadence === "daily") {
    return `Daily at ${schedule.time_of_day} (${schedule.timezone})`;
  }

  const labels = weekdayOptions
    .filter((day) => schedule.weekdays.includes(day.value))
    .map((day) => day.label)
    .join(", ");
  return `${labels} at ${schedule.time_of_day} (${schedule.timezone})`;
}
