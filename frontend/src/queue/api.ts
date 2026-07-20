export type PostStatus =
  | 'draft'
  | 'pending-review'
  | 'approved'
  | 'scheduled'
  | 'published'
  | 'failed'
  | 'rejected';

export interface QueuePost {
  post_id: string;
  queue_id: string | null;
  media_asset_id: string | null;
  image_url: string | null;
  image_source: string | null;
  image_license: string | null;
  image_width: number | null;
  image_height: number | null;
  image_mime_type: string | null;
  image_reference: string | null;
  header_text: string;
  paragraph_text: string;
  caption: string;
  status: PostStatus;
  scheduled_at: string | null;
  scheduled_for: string | null;
  published_at: string | null;
  failed_at: string | null;
  failure_message: string | null;
  queue_position: number | null;
  created_at: string;
  updated_at: string;
}

interface QueueResponse {
  posts: QueuePost[];
}

export async function fetchQueuePosts(): Promise<QueuePost[]> {
  const response = await fetch('/api/queue/', {
    credentials: 'include',
  });

  if (!response.ok) {
    throw new Error(`queue load failed: ${response.status}`);
  }

  const body = (await response.json()) as QueueResponse;
  return body.posts;
}
