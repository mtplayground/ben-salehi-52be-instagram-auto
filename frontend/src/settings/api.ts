export interface ContentSettings {
  id: string;
  creator_id: string;
  theme_topic: string;
  style_notes: string;
  review_mode_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface PostingSchedule {
  id: string;
  creator_id: string;
  timezone: string;
  cadence: "daily" | "weekly";
  time_of_day: string;
  weekdays: number[];
  is_active: boolean;
  next_run_at: string | null;
  created_at: string;
  updated_at: string;
}

interface ContentSettingsResponse {
  settings: ContentSettings | null;
}

interface ScheduleResponse {
  schedule: PostingSchedule | null;
}

export async function fetchContentSettings(): Promise<ContentSettings | null> {
  const response = await fetch("/api/settings/content", {
    credentials: "include",
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
  review_mode_enabled: boolean;
}): Promise<ContentSettings> {
  const response = await fetch("/api/settings/content", {
    method: "PUT",
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(input),
  });

  const body = (await response.json()) as Partial<ContentSettingsResponse> & {
    error?: string;
  };

  if (!response.ok) {
    throw new Error(body.error ?? `settings save failed: ${response.status}`);
  }

  if (!body.settings) {
    throw new Error("settings save returned no settings");
  }

  return body.settings;
}

export async function fetchPostingSchedule(): Promise<PostingSchedule | null> {
  const response = await fetch("/api/schedule/", {
    credentials: "include",
  });

  if (!response.ok) {
    throw new Error(`schedule load failed: ${response.status}`);
  }

  const body = (await response.json()) as ScheduleResponse;
  return body.schedule;
}

export async function savePostingSchedule(input: {
  timezone: string;
  cadence: "daily" | "weekly";
  time_of_day: string;
  weekdays: number[];
  is_active: boolean;
}): Promise<PostingSchedule> {
  const response = await fetch("/api/schedule/", {
    method: "PUT",
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(input),
  });

  const body = (await response.json()) as Partial<ScheduleResponse> & {
    error?: string;
  };

  if (!response.ok) {
    throw new Error(body.error ?? `schedule save failed: ${response.status}`);
  }

  if (!body.schedule) {
    throw new Error("schedule save returned no schedule");
  }

  return body.schedule;
}
