export interface AuthUser {
  sub: string;
  email: string;
  email_verified: boolean;
  name: string | null;
  picture: string | null;
}

export interface CreatorIdentity {
  id: string;
  email: string;
  display_name: string | null;
  avatar_url: string | null;
}

export interface AuthSession {
  authenticated: boolean;
  user: AuthUser;
  creator: CreatorIdentity;
  is_new_registration: boolean;
  message: string;
}

export async function fetchSession(): Promise<AuthSession | null> {
  const response = await fetch('/api/auth/me', {
    credentials: 'include',
  });

  if (response.status === 401) {
    return null;
  }

  if (!response.ok) {
    throw new Error(`session check failed: ${response.status}`);
  }

  return (await response.json()) as AuthSession;
}

export async function logout(): Promise<void> {
  const response = await fetch('/api/auth/logout', {
    method: 'POST',
    credentials: 'include',
  });

  if (!response.ok) {
    throw new Error(`logout failed: ${response.status}`);
  }
}
