import { Navigate, Outlet, useLocation } from 'react-router-dom';

import { useAuth } from './useAuth';
import { LoginPage } from '../pages/LoginPage';

export function AuthGate() {
  const { session, loading, error, refresh } = useAuth();
  const location = useLocation();

  if (loading) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-paper px-4 text-ink">
        <div className="rounded-lg border border-ink/10 bg-white px-5 py-4 shadow-panel">
          <p className="text-sm font-medium text-ink/70">Checking session...</p>
        </div>
      </main>
    );
  }

  if (!session) {
    if (location.pathname === '/signup') {
      return <LoginPage mode="signup" error={error} onRetry={refresh} />;
    }

    if (location.pathname !== '/') {
      return <Navigate to="/" replace />;
    }

    return <LoginPage mode="login" error={error} onRetry={refresh} />;
  }

  if (location.pathname === '/signup') {
    return <Navigate to="/" replace />;
  }

  return <Outlet />;
}
