import { Save, SlidersHorizontal } from 'lucide-react';
import { FormEvent, useEffect, useMemo, useState } from 'react';

import {
  ContentSettings,
  fetchContentSettings,
  saveContentSettings,
} from '../settings/api';

const emptyForm = {
  theme_topic: '',
  style_notes: '',
};

export function SettingsPage() {
  const [form, setForm] = useState(emptyForm);
  const [saved, setSaved] = useState<ContentSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let active = true;

    async function loadSettings() {
      setLoading(true);
      setError(null);
      try {
        const settings = await fetchContentSettings();
        if (!active) {
          return;
        }
        setSaved(settings);
        setForm(
          settings
            ? {
                theme_topic: settings.theme_topic,
                style_notes: settings.style_notes,
              }
            : emptyForm,
        );
      } catch (caught) {
        if (active) {
          setError(caught instanceof Error ? caught.message : 'Settings could not be loaded');
        }
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    }

    void loadSettings();

    return () => {
      active = false;
    };
  }, []);

  const isDirty = useMemo(() => {
    const savedTheme = saved?.theme_topic ?? '';
    const savedStyle = saved?.style_notes ?? '';
    return form.theme_topic !== savedTheme || form.style_notes !== savedStyle;
  }, [form, saved]);

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
      setNotice('Content settings saved.');
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Settings could not be saved');
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="space-y-6">
      <div>
        <p className="text-sm font-semibold uppercase tracking-wide text-meadow">Settings</p>
        <h2 className="mt-3 text-3xl font-bold">Creator content</h2>
        <p className="mt-3 max-w-2xl text-base leading-7 text-ink/70">
          Define the topic and visual voice that generated Instagram posts should follow for this
          creator account.
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
                <label htmlFor="theme_topic" className="text-sm font-semibold text-ink">
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
                  disabled={loading || saving}
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
                <label htmlFor="style_notes" className="text-sm font-semibold text-ink">
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
                  disabled={loading || saving}
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
                  {saved ? `Last saved ${formatDate(saved.updated_at)}` : 'No settings saved yet'}
                </p>
                <button
                  type="submit"
                  disabled={loading || saving || !isDirty}
                  className="inline-flex items-center gap-2 rounded-md bg-meadow px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-meadow/90 disabled:cursor-not-allowed disabled:bg-ink/25"
                >
                  <Save size={17} aria-hidden="true" />
                  {saving ? 'Saving' : 'Save'}
                </button>
              </div>
            </div>
          </div>
        </form>

        <aside className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
          <h3 className="text-lg font-semibold">Current direction</h3>
          <dl className="mt-5 space-y-4">
            <PreviewRow label="Theme" value={form.theme_topic || 'Unset'} />
            <PreviewRow label="Style" value={form.style_notes || 'Unset'} />
          </dl>
        </aside>
      </div>
    </section>
  );
}

function PreviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs font-semibold uppercase tracking-wide text-ink/45">{label}</dt>
      <dd className="mt-1 whitespace-pre-wrap text-sm leading-6 text-ink/75">{value}</dd>
    </div>
  );
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}
