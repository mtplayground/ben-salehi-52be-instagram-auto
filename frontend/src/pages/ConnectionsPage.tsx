import { Instagram, Link2, RefreshCw, Unplug } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';

import {
  InstagramAccount,
  disconnectInstagram,
  fetchInstagramStatus,
} from '../instagram/api';

export function ConnectionsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [account, setAccount] = useState<InstagramAccount | null>(null);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const initialNotice = useMemo(() => {
    const result = searchParams.get('instagram');
    if (result === 'connected') {
      return 'Instagram account connected.';
    }
    if (result === 'denied') {
      return 'Instagram connection was not completed.';
    }
    return null;
  }, [searchParams]);
  const [notice, setNotice] = useState<string | null>(initialNotice);

  useEffect(() => {
    let active = true;

    async function loadStatus() {
      setLoading(true);
      setError(null);
      try {
        const nextAccount = await fetchInstagramStatus();
        if (active) {
          setAccount(nextAccount);
        }
      } catch (caught) {
        if (active) {
          setError(caught instanceof Error ? caught.message : 'Instagram status could not be loaded');
        }
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    }

    void loadStatus();

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (initialNotice) {
      setNotice(initialNotice);
      setSearchParams({}, { replace: true });
    }
  }, [initialNotice, setSearchParams]);

  async function handleDisconnect() {
    setWorking(true);
    setError(null);
    try {
      setAccount(await disconnectInstagram());
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Instagram could not be disconnected');
    } finally {
      setWorking(false);
    }
  }

  const connected = account?.connection_status === 'connected';
  const displayName = account?.username ? `@${account.username}` : account?.instagram_user_id;

  return (
    <section className="space-y-6">
      <div>
        <p className="text-sm font-semibold uppercase tracking-wide text-meadow">Connections</p>
        <h2 className="mt-3 text-3xl font-bold">Instagram account</h2>
        <p className="mt-3 max-w-2xl text-base leading-7 text-ink/70">
          Connect the creator-owned Instagram account that generated posts will use.
        </p>
      </div>

      <div className="grid gap-5 lg:grid-cols-[1fr_320px]">
        <div className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
          <div className="flex flex-col gap-5 sm:flex-row sm:items-start sm:justify-between">
            <div className="flex items-start gap-4">
              <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-coral/12 text-coral">
                <Instagram size={22} aria-hidden="true" />
              </span>
              <div>
                <h3 className="text-lg font-semibold">
                  {connected ? displayName : 'No account connected'}
                </h3>
                <p className="mt-2 text-sm leading-6 text-ink/65">
                  {statusCopy(account, loading)}
                </p>
              </div>
            </div>

            <div className="flex shrink-0 flex-wrap gap-2">
              <a
                href="/api/instagram/connect"
                className="inline-flex items-center gap-2 rounded-md bg-meadow px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-meadow/90"
              >
                {connected ? <RefreshCw size={17} aria-hidden="true" /> : <Link2 size={17} aria-hidden="true" />}
                {connected ? 'Reconnect' : 'Connect'}
              </a>
              {account ? (
                <button
                  type="button"
                  onClick={() => void handleDisconnect()}
                  disabled={working || account.connection_status === 'disconnected'}
                  className="inline-flex items-center gap-2 rounded-md border border-ink/15 bg-white px-4 py-2.5 text-sm font-semibold text-ink/70 transition hover:text-ink disabled:cursor-not-allowed disabled:text-ink/35"
                >
                  <Unplug size={17} aria-hidden="true" />
                  Disconnect
                </button>
              ) : null}
            </div>
          </div>

          {notice ? (
            <div className="mt-5 rounded-md border border-meadow/25 bg-meadow/10 px-3 py-2 text-sm text-meadow">
              {notice}
            </div>
          ) : null}

          {error ? (
            <div className="mt-5 rounded-md border border-coral/30 bg-coral/10 px-3 py-2 text-sm text-ink">
              {error}
            </div>
          ) : null}
        </div>

        <aside className="rounded-lg border border-ink/10 bg-white p-5 shadow-panel">
          <h3 className="text-lg font-semibold">Connection status</h3>
          <dl className="mt-5 space-y-4">
            <StatusRow label="State" value={account?.connection_status ?? 'not connected'} />
            <StatusRow label="Account ID" value={account?.instagram_user_id ?? 'Unset'} />
            <StatusRow
              label="Connected"
              value={account ? formatDate(account.connected_at) : 'Unset'}
            />
          </dl>
        </aside>
      </div>
    </section>
  );
}

function statusCopy(account: InstagramAccount | null, loading: boolean) {
  if (loading) {
    return 'Loading connection status.';
  }
  if (!account) {
    return 'Authorize an Instagram account owned by this creator.';
  }
  if (account.connection_status === 'connected') {
    return 'This creator can use the connected Instagram account for upcoming publishing steps.';
  }
  if (account.connection_status === 'reconnect-needed') {
    return account.reconnect_reason ?? 'This account needs to be reconnected.';
  }
  return 'This account is disconnected.';
}

function StatusRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs font-semibold uppercase tracking-wide text-ink/45">{label}</dt>
      <dd className="mt-1 break-words text-sm leading-6 text-ink/75">{value}</dd>
    </div>
  );
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}
