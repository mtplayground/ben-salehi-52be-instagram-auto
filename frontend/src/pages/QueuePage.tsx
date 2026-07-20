import {
  CalendarDays,
  CheckCircle2,
  Clock3,
  Edit3,
  ImageOff,
  ListChecks,
  MessageSquareText,
  RefreshCw,
  Save,
  XCircle,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { QueuePost, fetchQueuePosts } from '../queue/api';
import { approvePost, editPost, regeneratePost, rejectPost } from '../review/api';

type TimelineGroup = {
  key: string;
  label: string;
  posts: QueuePost[];
};

export function QueuePage() {
  const [posts, setPosts] = useState<QueuePost[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadQueue = useCallback(async (showLoading = true) => {
    if (showLoading) {
      setLoading(true);
    }
    setError(null);

    try {
      setPosts(await fetchQueuePosts());
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Queue could not be loaded');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadQueue();
  }, [loadQueue]);

  const now = useMemo(() => new Date(), []);
  const upcomingCount = posts.filter((post) => !isPastPost(post, now)).length;
  const pastCount = posts.length - upcomingCount;
  const calendarGroups = useMemo(() => groupPostsByMonth(posts), [posts]);

  return (
    <section className="space-y-6">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
        <div>
          <p className="text-sm font-semibold uppercase tracking-wide text-meadow">Queue</p>
          <h2 className="mt-3 text-3xl font-bold">Queue and calendar</h2>
          <p className="mt-3 max-w-2xl text-base leading-7 text-ink/70">
            Review upcoming and past Instagram posts with their image preview, on-image text,
            caption, scheduled time, and publishing state.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-2 sm:min-w-[360px]">
          <SummaryTile label="Total" value={posts.length} icon={ListChecks} />
          <SummaryTile label="Upcoming" value={upcomingCount} icon={Clock3} />
          <SummaryTile label="Past" value={pastCount} icon={CheckCircle2} />
        </div>
      </div>

      {error ? (
        <div className="rounded-md border border-coral/30 bg-coral/10 px-3 py-2 text-sm text-ink">
          {error}
        </div>
      ) : null}

      {loading ? (
        <div className="rounded-lg border border-ink/10 bg-white p-8 text-center shadow-panel">
          <CalendarDays className="mx-auto text-sky" size={34} aria-hidden="true" />
          <h3 className="mt-4 text-lg font-semibold">Loading queue</h3>
          <p className="mx-auto mt-2 max-w-md text-sm leading-6 text-ink/65">
            Gathering scheduled posts, generated previews, captions, and publishing states.
          </p>
        </div>
      ) : posts.length === 0 ? (
        <div className="rounded-lg border border-dashed border-ink/20 bg-white p-8 text-center">
          <CalendarDays className="mx-auto text-sky" size={36} aria-hidden="true" />
          <h3 className="mt-4 text-lg font-semibold">No posts in the queue</h3>
          <p className="mx-auto mt-2 max-w-md text-sm leading-6 text-ink/65">
            Saved content settings and an active posting schedule will begin filling this calendar.
          </p>
        </div>
      ) : (
        <div className="grid gap-5 xl:grid-cols-[280px_1fr]">
          <aside className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
            <div className="flex items-center gap-3">
              <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-sky/12 text-sky">
                <CalendarDays size={20} aria-hidden="true" />
              </span>
              <div>
                <h3 className="font-semibold">Calendar</h3>
                <p className="text-sm text-ink/55">{posts.length} posts</p>
              </div>
            </div>

            <div className="mt-5 space-y-4">
              {calendarGroups.map((group) => (
                <div key={group.key}>
                  <p className="text-xs font-semibold uppercase tracking-wide text-ink/45">
                    {group.label}
                  </p>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {group.posts.map((post) => (
                      <span
                        key={post.post_id}
                        className={[
                          'flex h-9 w-9 items-center justify-center rounded-md text-sm font-semibold',
                          isPastPost(post, now)
                            ? 'bg-ink/5 text-ink/65'
                            : 'bg-meadow/12 text-meadow',
                        ].join(' ')}
                        title={formatDateTime(displayDate(post))}
                      >
                        {new Date(displayDate(post)).getDate()}
                      </span>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </aside>

          <div className="space-y-4">
            {posts.map((post) => (
              <PostCard
                key={post.post_id}
                post={post}
                now={now}
                onChanged={() => loadQueue(false)}
              />
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function PostCard({
  post,
  now,
  onChanged,
}: {
  post: QueuePost;
  now: Date;
  onChanged: () => Promise<void>;
}) {
  const date = displayDate(post);
  const past = isPastPost(post, now);
  const status = statusMeta(post.status);
  const StatusIcon = status.icon;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState({
    header_text: post.header_text,
    paragraph_text: post.paragraph_text,
    caption: post.caption,
  });
  const [working, setWorking] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const canReview = post.status !== 'published' && post.status !== 'failed';

  async function runAction(label: string, action: () => Promise<void>) {
    setWorking(label);
    setActionError(null);
    try {
      await action();
      await onChanged();
      setEditing(false);
    } catch (caught) {
      setActionError(caught instanceof Error ? caught.message : `${label} failed`);
    } finally {
      setWorking(null);
    }
  }

  function startEditing() {
    setDraft({
      header_text: post.header_text,
      paragraph_text: post.paragraph_text,
      caption: post.caption,
    });
    setActionError(null);
    setEditing(true);
  }

  return (
    <article className="overflow-hidden rounded-lg border border-ink/10 bg-white shadow-panel">
      <div className="grid gap-0 md:grid-cols-[220px_1fr]">
        <div className="relative min-h-[220px] bg-ink/5">
          {post.image_url ? (
            <img
              src={post.image_url}
              alt=""
              className="h-full min-h-[220px] w-full object-cover"
              loading="lazy"
            />
          ) : (
            <div className="flex h-full min-h-[220px] items-center justify-center text-ink/35">
              <ImageOff size={34} aria-hidden="true" />
            </div>
          )}
        </div>

        <div className="min-w-0 p-5">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-sm font-semibold text-ink/55">
                {past ? 'Past post' : 'Upcoming post'} - {formatDateTime(date)}
              </p>
              {editing ? (
                <input
                  value={draft.header_text}
                  maxLength={64}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, header_text: event.target.value }))
                  }
                  className="mt-2 w-full rounded-md border border-ink/15 px-3 py-2 text-base font-bold outline-none focus:border-meadow focus:ring-2 focus:ring-meadow/20"
                />
              ) : (
                <h3 className="mt-2 text-xl font-bold leading-snug">{post.header_text}</h3>
              )}
            </div>
            <span
              className={[
                'inline-flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-semibold',
                status.className,
              ].join(' ')}
            >
              <StatusIcon size={16} aria-hidden="true" />
              {status.label}
            </span>
          </div>

          {editing ? (
            <textarea
              value={draft.paragraph_text}
              maxLength={220}
              rows={3}
              onChange={(event) =>
                setDraft((current) => ({ ...current, paragraph_text: event.target.value }))
              }
              className="mt-4 w-full resize-y rounded-md border border-ink/15 px-3 py-2 text-sm leading-6 text-ink outline-none focus:border-meadow focus:ring-2 focus:ring-meadow/20"
            />
          ) : (
            <p className="mt-4 text-sm leading-6 text-ink/70">{post.paragraph_text}</p>
          )}

          <div className="mt-5 rounded-md bg-paper px-4 py-3">
            <div className="flex items-center gap-2 text-sm font-semibold text-ink">
              <MessageSquareText size={16} aria-hidden="true" />
              Caption
            </div>
            {editing ? (
              <textarea
                value={draft.caption}
                maxLength={2200}
                rows={5}
                onChange={(event) =>
                  setDraft((current) => ({ ...current, caption: event.target.value }))
                }
                className="mt-2 w-full resize-y rounded-md border border-ink/15 bg-white px-3 py-2 text-sm leading-6 text-ink outline-none focus:border-meadow focus:ring-2 focus:ring-meadow/20"
              />
            ) : (
              <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-ink/70">
                {post.caption}
              </p>
            )}
          </div>

          {post.failure_message ? (
            <div className="mt-4 rounded-md border border-coral/30 bg-coral/10 px-3 py-2 text-sm text-ink">
              {post.failure_message}
            </div>
          ) : null}

          {actionError ? (
            <div className="mt-4 rounded-md border border-coral/30 bg-coral/10 px-3 py-2 text-sm text-ink">
              {actionError}
            </div>
          ) : null}

          {canReview ? (
            <div className="mt-5 flex flex-wrap gap-2">
              {editing ? (
                <>
                  <button
                    type="button"
                    disabled={Boolean(working)}
                    onClick={() =>
                      void runAction('edit', () => editPost(post.post_id, draft))
                    }
                    className="inline-flex items-center gap-2 rounded-md bg-meadow px-3 py-2 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:bg-ink/25"
                  >
                    <Save size={16} aria-hidden="true" />
                    Save edit
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(working)}
                    onClick={() => setEditing(false)}
                    className="rounded-md border border-ink/15 bg-white px-3 py-2 text-sm font-semibold text-ink/70 disabled:cursor-not-allowed disabled:text-ink/35"
                  >
                    Cancel
                  </button>
                </>
              ) : (
                <>
                  <button
                    type="button"
                    disabled={Boolean(working)}
                    onClick={() => void runAction('approve', () => approvePost(post.post_id))}
                    className="inline-flex items-center gap-2 rounded-md bg-meadow px-3 py-2 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:bg-ink/25"
                  >
                    <CheckCircle2 size={16} aria-hidden="true" />
                    Approve
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(working)}
                    onClick={startEditing}
                    className="inline-flex items-center gap-2 rounded-md border border-ink/15 bg-white px-3 py-2 text-sm font-semibold text-ink/70 disabled:cursor-not-allowed disabled:text-ink/35"
                  >
                    <Edit3 size={16} aria-hidden="true" />
                    Edit
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(working)}
                    onClick={() => void runAction('regenerate', () => regeneratePost(post.post_id))}
                    className="inline-flex items-center gap-2 rounded-md border border-ink/15 bg-white px-3 py-2 text-sm font-semibold text-ink/70 disabled:cursor-not-allowed disabled:text-ink/35"
                  >
                    <RefreshCw size={16} aria-hidden="true" />
                    Regenerate
                  </button>
                  <button
                    type="button"
                    disabled={Boolean(working)}
                    onClick={() => void runAction('reject', () => rejectPost(post.post_id))}
                    className="inline-flex items-center gap-2 rounded-md border border-coral/25 bg-white px-3 py-2 text-sm font-semibold text-coral disabled:cursor-not-allowed disabled:text-coral/35"
                  >
                    <XCircle size={16} aria-hidden="true" />
                    Reject
                  </button>
                </>
              )}
            </div>
          ) : null}

          <dl className="mt-5 grid gap-3 text-sm sm:grid-cols-3">
            <MetaRow label="Queue slot" value={post.queue_position === null ? 'None' : String(post.queue_position)} />
            <MetaRow label="Image source" value={post.image_source ?? 'None'} />
            <MetaRow label="Updated" value={formatDateTime(post.updated_at)} />
          </dl>
        </div>
      </div>
    </article>
  );
}

function SummaryTile({
  label,
  value,
  icon: Icon,
}: {
  label: string;
  value: number;
  icon: LucideIcon;
}) {
  return (
    <div className="rounded-lg border border-ink/10 bg-white px-3 py-3 shadow-panel">
      <div className="flex items-center gap-2 text-ink/55">
        <Icon size={16} aria-hidden="true" />
        <span className="text-xs font-semibold uppercase tracking-wide">{label}</span>
      </div>
      <p className="mt-2 text-2xl font-bold">{value}</p>
    </div>
  );
}

function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs font-semibold uppercase tracking-wide text-ink/45">{label}</dt>
      <dd className="mt-1 break-words text-ink/70">{value}</dd>
    </div>
  );
}

function groupPostsByMonth(posts: QueuePost[]): TimelineGroup[] {
  const groups = new Map<string, TimelineGroup>();

  posts.forEach((post) => {
    const date = new Date(displayDate(post));
    const key = `${date.getFullYear()}-${date.getMonth()}`;
    const label = new Intl.DateTimeFormat(undefined, {
      month: 'long',
      year: 'numeric',
    }).format(date);
    const existing = groups.get(key);

    if (existing) {
      existing.posts.push(post);
    } else {
      groups.set(key, { key, label, posts: [post] });
    }
  });

  return Array.from(groups.values());
}

function displayDate(post: QueuePost): string {
  return post.scheduled_for ?? post.scheduled_at ?? post.published_at ?? post.created_at;
}

function isPastPost(post: QueuePost, now: Date): boolean {
  return new Date(displayDate(post)).getTime() < now.getTime();
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function statusMeta(status: QueuePost['status']) {
  switch (status) {
    case 'published':
      return {
        label: 'Published',
        icon: CheckCircle2,
        className: 'bg-meadow/12 text-meadow',
      };
    case 'failed':
    case 'rejected':
      return {
        label: status === 'failed' ? 'Failed' : 'Rejected',
        icon: XCircle,
        className: 'bg-coral/12 text-coral',
      };
    case 'scheduled':
      return {
        label: 'Scheduled',
        icon: CalendarDays,
        className: 'bg-sky/12 text-sky',
      };
    case 'approved':
      return {
        label: 'Approved',
        icon: CheckCircle2,
        className: 'bg-meadow/12 text-meadow',
      };
    case 'pending-review':
      return {
        label: 'Pending review',
        icon: Clock3,
        className: 'bg-amber-100 text-amber-700',
      };
    default:
      return {
        label: 'Draft',
        icon: ListChecks,
        className: 'bg-ink/5 text-ink/65',
      };
  }
}
