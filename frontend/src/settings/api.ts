export interface ContentSettings {
  id: string;
  creator_id: string;
  theme_topic: string;
  style_notes: string;
  created_at: string;
  updated_at: string;
}

interface ContentSettingsResponse {
  settings: ContentSettings | null;
}

export async function fetchContentSettings(): Promise<ContentSettings | null> {
  const response = await fetch('/api/settings/content', {
    credentials: 'include',
  });

  if (!response.ok) {
    throw new Error(`settings load failed: ${response.status}`);
  }

  const body = (await response.json()) as ContentSettingsResponse;
  return body.settings;
}

export async function saveContentSettings(input: {
  theme_topic: string;
  style_notes: string;
}): Promise<ContentSettings> {
  const response = await fetch('/api/settings/content', {
    method: 'PUT',
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(input),
  });

  const body = (await response.json()) as Partial<ContentSettingsResponse> & { error?: string };

  if (!response.ok) {
    throw new Error(body.error ?? `settings save failed: ${response.status}`);
  }

  if (!body.settings) {
    throw new Error('settings save returned no settings');
  }

  return body.settings;
}
