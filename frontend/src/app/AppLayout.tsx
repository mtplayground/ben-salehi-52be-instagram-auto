import { CalendarDays, Home, LogOut, Settings, Sparkles } from 'lucide-react';
import { NavLink, Outlet } from 'react-router-dom';

import { useAuth } from '../auth/useAuth';

const navItems = [
  { to: '/', label: 'Overview', icon: Home },
  { to: '/queue', label: 'Queue', icon: CalendarDays },
  { to: '/settings', label: 'Settings', icon: Settings },
];

export function AppLayout() {
  const { session, logout } = useAuth();
  const displayName = session?.user.name ?? session?.user.email ?? 'Creator';
  const picture = session?.user.picture;

  return (
    <div className="min-h-screen bg-paper text-ink">
      <aside className="fixed inset-y-0 left-0 hidden w-64 border-r border-ink/10 bg-white/85 px-5 py-6 shadow-panel backdrop-blur lg:flex lg:flex-col">
        <div className="flex items-center gap-3">
          <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-meadow text-white">
            <Sparkles size={20} aria-hidden="true" />
          </span>
          <div>
            <p className="text-sm font-semibold uppercase tracking-wide text-ink/55">Creator</p>
            <h1 className="text-lg font-bold">Workspace</h1>
          </div>
        </div>

        <nav className="mt-10 flex flex-col gap-1" aria-label="Primary">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.to === '/'}
              className={({ isActive }) =>
                [
                  'flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition',
                  isActive ? 'bg-meadow text-white' : 'text-ink/70 hover:bg-ink/5 hover:text-ink',
                ].join(' ')
              }
            >
              <item.icon size={18} aria-hidden="true" />
              {item.label}
            </NavLink>
          ))}
        </nav>

        <div className="mt-auto border-t border-ink/10 pt-5">
          <UserBadge displayName={displayName} picture={picture} />
          <button
            type="button"
            onClick={() => void logout()}
            className="mt-4 flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm font-semibold text-ink/65 transition hover:bg-ink/5 hover:text-ink"
          >
            <LogOut size={17} aria-hidden="true" />
            Sign out
          </button>
        </div>
      </aside>

      <div className="lg:pl-64">
        <header className="sticky top-0 z-10 border-b border-ink/10 bg-paper/90 px-4 py-3 backdrop-blur lg:hidden">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-meadow text-white">
                <Sparkles size={18} aria-hidden="true" />
              </span>
              <span className="font-bold">Creator Workspace</span>
            </div>
            <button
              type="button"
              onClick={() => void logout()}
              className="flex h-9 w-9 items-center justify-center rounded-md bg-white text-ink/70"
              aria-label="Sign out"
            >
              <LogOut size={17} aria-hidden="true" />
            </button>
          </div>
          <nav className="mt-3 grid grid-cols-3 gap-2" aria-label="Primary">
            {navItems.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.to === '/'}
                className={({ isActive }) =>
                  [
                    'flex items-center justify-center gap-2 rounded-md px-2 py-2 text-sm font-medium',
                    isActive ? 'bg-meadow text-white' : 'bg-white text-ink/70',
                  ].join(' ')
                }
              >
                <item.icon size={16} aria-hidden="true" />
                <span>{item.label}</span>
              </NavLink>
            ))}
          </nav>
        </header>

        <main className="mx-auto min-h-screen max-w-6xl px-4 py-6 sm:px-6 lg:px-8 lg:py-10">
          <Outlet />
        </main>
      </div>
    </div>
  );
}

function UserBadge({ displayName, picture }: { displayName: string; picture: string | null | undefined }) {
  return (
    <div className="flex items-center gap-3">
      {picture ? (
        <img
          src={picture}
          alt=""
          className="h-10 w-10 rounded-lg object-cover"
          referrerPolicy="no-referrer"
        />
      ) : (
        <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-sky/12 text-sm font-bold text-sky">
          {displayName.slice(0, 1).toUpperCase()}
        </span>
      )}
      <div className="min-w-0">
        <p className="truncate text-sm font-semibold">{displayName}</p>
        <p className="text-xs text-ink/50">Signed in</p>
      </div>
    </div>
  );
}
