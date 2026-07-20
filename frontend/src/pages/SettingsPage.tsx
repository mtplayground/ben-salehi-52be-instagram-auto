import { SlidersHorizontal } from 'lucide-react';

export function SettingsPage() {
  return (
    <section className="space-y-5">
      <div>
        <p className="text-sm font-semibold uppercase tracking-wide text-meadow">Settings</p>
        <h2 className="mt-3 text-3xl font-bold">Creator preferences</h2>
        <p className="mt-3 max-w-2xl text-base leading-7 text-ink/70">
          Manage the creator inputs that guide content, review behavior, and posting cadence.
        </p>
      </div>

      <div className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
        <div className="flex items-start gap-4">
          <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-coral/12 text-coral">
            <SlidersHorizontal size={22} aria-hidden="true" />
          </span>
          <div>
            <h3 className="font-semibold">No preferences saved yet</h3>
            <p className="mt-2 text-sm leading-6 text-ink/65">
              Preferences will appear here after the creator completes setup.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
