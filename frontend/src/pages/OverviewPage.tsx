import { CalendarClock, Image, MessageSquareText } from 'lucide-react';

const cards = [
  {
    title: 'Content plan',
    value: 'No theme set',
    detail: 'Creator themes and style preferences keep every post aligned with the account.',
    icon: Image,
  },
  {
    title: 'Post queue',
    value: 'No posts queued',
    detail: 'Scheduled posts collect image previews, captions, review state, and publish status.',
    icon: CalendarClock,
  },
  {
    title: 'Captions',
    value: 'Not generated',
    detail: 'Captions support the image text while sounding casual and creator-led.',
    icon: MessageSquareText,
  },
];

export function OverviewPage() {
  return (
    <div className="space-y-8">
      <section className="grid gap-6 lg:grid-cols-[1fr_320px] lg:items-start">
        <div>
          <p className="text-sm font-semibold uppercase tracking-wide text-meadow">Dashboard</p>
          <h2 className="mt-3 max-w-3xl text-3xl font-bold leading-tight sm:text-4xl">
            Plan, review, and publish Instagram posts from one creator workspace.
          </h2>
          <p className="mt-4 max-w-2xl text-base leading-7 text-ink/70">
            Keep the posting workflow organized around creator settings, upcoming post previews,
            review decisions, and publishing status.
          </p>
        </div>

        <div className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
          <p className="text-sm font-semibold text-ink/60">Workspace summary</p>
          <div className="mt-4 space-y-3">
            <StatusRow label="Connected account" state="Not connected" />
            <StatusRow label="Posting cadence" state="Unset" />
            <StatusRow label="Review mode" state="Unset" />
          </div>
        </div>
      </section>

      <section className="grid gap-4 md:grid-cols-3">
        {cards.map((card) => (
          <article key={card.title} className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
            <div className="flex items-center gap-3">
              <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-sky/12 text-sky">
                <card.icon size={20} aria-hidden="true" />
              </span>
              <div>
                <h3 className="font-semibold">{card.title}</h3>
                <p className="text-sm text-coral">{card.value}</p>
              </div>
            </div>
            <p className="mt-4 text-sm leading-6 text-ink/65">{card.detail}</p>
          </article>
        ))}
      </section>
    </div>
  );
}

function StatusRow({ label, state }: { label: string; state: string }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-md bg-paper px-3 py-2">
      <span className="text-sm text-ink/70">{label}</span>
      <span className="text-sm font-semibold text-meadow">{state}</span>
    </div>
  );
}
