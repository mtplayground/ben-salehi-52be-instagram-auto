export interface InstagramAccount {
  id: string;
  instagram_user_id: string;
  username: string | null;
  connection_status: 'connected' | 'reconnect-needed' | 'disconnected';
  reconnect_reason: string | null;
  connected_at: string;
  disconnected_at: string | null;
}

interface InstagramStatusResponse {
  account: InstagramAccount | null;
}

export async function fetchInstagramStatus(): Promise<InstagramAccount | null> {
  const response = await fetch('/api/instagram/status', {
    credentials: 'include',
  });

  if (!response.ok) {
    throw new Error(`Instagram status load failed: ${response.status}`);
  }

  const body = (await response.json()) as InstagramStatusResponse;
  return body.account;
}

export async function disconnectInstagram(): Promise<InstagramAccount | null> {
  const response = await fetch('/api/instagram/disconnect', {
    method: 'POST',
    credentials: 'include',
  });

  const body = (await response.json()) as InstagramStatusResponse & { error?: string };

  if (!response.ok) {
    throw new Error(body.error ?? `Instagram disconnect failed: ${response.status}`);
  }

  return body.account;
}
