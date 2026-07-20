import { CalendarDays } from 'lucide-react';

export function QueuePage() {
  return (
    <section className="space-y-5">
      <div>
        <p className="text-sm font-semibold uppercase tracking-wide text-meadow">Queue</p>
        <h2 className="mt-3 text-3xl font-bold">Upcoming posts</h2>
        <p className="mt-3 max-w-2xl text-base leading-7 text-ink/70">
          Scheduled posts appear here with image previews, on-image text, captions, and publishing
          state.
        </p>
      </div>

      <div className="rounded-lg border border-dashed border-ink/20 bg-white p-8 text-center">
        <CalendarDays className="mx-auto text-sky" size={36} aria-hidden="true" />
        <h3 className="mt-4 text-lg font-semibold">No posts scheduled yet</h3>
        <p className="mx-auto mt-2 max-w-md text-sm leading-6 text-ink/65">
          Add creator settings and a posting cadence to begin filling the queue.
        </p>
      </div>
    </section>
  );
}
