import { ArrowRight, RefreshCw } from 'lucide-react';
import { Link } from 'react-router-dom';

interface LoginPageProps {
  mode: 'login' | 'signup';
  error: string | null;
  onRetry: () => Promise<void>;
}

export function LoginPage({ mode, error, onRetry }: LoginPageProps) {
  const isSignup = mode === 'signup';

  return (
    <main className="min-h-screen bg-paper text-ink">
      <div className="mx-auto grid min-h-screen max-w-6xl items-center gap-10 px-4 py-10 sm:px-6 lg:grid-cols-[1fr_420px] lg:px-8">
        <section>
          <p className="text-sm font-semibold uppercase tracking-wide text-meadow">
            Creator account
          </p>
          <h1 className="mt-4 max-w-3xl text-4xl font-bold leading-tight sm:text-5xl">
            {isSignup ? 'Create your creator workspace.' : 'Sign in to your creator workspace.'}
          </h1>
          <p className="mt-5 max-w-2xl text-base leading-7 text-ink/70">
            Manage Instagram post planning, review, and publishing from an account that is scoped
            to your own creator profile.
          </p>

          <div className="mt-8 flex flex-wrap gap-3">
            <a
              href="/api/auth/login"
              className="inline-flex items-center gap-2 rounded-md bg-meadow px-4 py-2.5 text-sm font-semibold text-white shadow-panel transition hover:bg-meadow/90"
            >
              {isSignup ? 'Create account' : 'Sign in'}
              <ArrowRight size={18} aria-hidden="true" />
            </a>
            <Link
              to={isSignup ? '/' : '/signup'}
              className="inline-flex items-center rounded-md border border-ink/15 bg-white px-4 py-2.5 text-sm font-semibold text-ink/75 transition hover:text-ink"
            >
              {isSignup ? 'I already have an account' : 'Create an account'}
            </Link>
          </div>

          {error ? (
            <div className="mt-6 rounded-lg border border-coral/30 bg-coral/10 p-4 text-sm text-ink">
              <p className="font-semibold">Session check failed</p>
              <p className="mt-1 text-ink/70">{error}</p>
              <button
                type="button"
                onClick={() => void onRetry()}
                className="mt-3 inline-flex items-center gap-2 rounded-md bg-white px-3 py-2 text-sm font-semibold text-ink/75"
              >
                <RefreshCw size={16} aria-hidden="true" />
                Retry
              </button>
            </div>
          ) : null}
        </section>

        <aside className="rounded-lg border border-ink/10 bg-white p-6 shadow-panel">
          <h2 className="text-lg font-semibold">Private by default</h2>
          <p className="mt-3 text-sm leading-6 text-ink/65">
            Your workspace opens only for your creator account, keeping profile details and planned
            posts scoped to you.
          </p>
          <dl className="mt-6 space-y-4">
            <InfoRow label="Account" value="Verified" />
            <InfoRow label="Workspace" value="Creator scoped" />
            <InfoRow label="Posts" value="Private" />
          </dl>
        </aside>
      </div>
    </main>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-md bg-paper px-3 py-2">
      <dt className="text-sm text-ink/60">{label}</dt>
      <dd className="text-sm font-semibold text-ink">{value}</dd>
    </div>
  );
}
