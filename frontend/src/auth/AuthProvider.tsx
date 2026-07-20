import React, { useCallback, useEffect, useMemo, useState } from 'react';

import { AuthSession, fetchSession, logout as requestLogout } from './api';
import { AuthContext } from './context';

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<AuthSession | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setSession(await fetchSession());
    } catch (caught) {
      setSession(null);
      setError(caught instanceof Error ? caught.message : 'Unable to check session');
    } finally {
      setLoading(false);
    }
  }, []);

  const logout = useCallback(async () => {
    await requestLogout();
    setSession(null);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const value = useMemo(
    () => ({ session, loading, error, refresh, logout }),
    [session, loading, error, refresh, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
