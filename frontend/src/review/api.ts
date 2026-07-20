export interface EditPostInput {
  header_text: string;
  paragraph_text: string;
  caption: string;
}

interface ReviewResponse {
  post: {
    id: string;
    status: string;
    updated_at: string;
  };
}

export async function approvePost(postId: string): Promise<void> {
  await postReviewAction(`/api/review/${postId}/approve`);
}

export async function rejectPost(postId: string): Promise<void> {
  await postReviewAction(`/api/review/${postId}/reject`);
}

export async function regeneratePost(postId: string): Promise<void> {
  await postReviewAction(`/api/review/${postId}/regenerate`);
}

export async function editPost(postId: string, input: EditPostInput): Promise<void> {
  const response = await fetch(`/api/review/${postId}`, {
    method: 'PUT',
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(input),
  });

  await readReviewResponse(response, 'post edit failed');
}

async function postReviewAction(url: string): Promise<void> {
  const response = await fetch(url, {
    method: 'POST',
    credentials: 'include',
  });

  await readReviewResponse(response, 'post review action failed');
}

async function readReviewResponse(response: Response, fallback: string): Promise<void> {
  const body = (await response.json()) as Partial<ReviewResponse> & { error?: string };

  if (!response.ok) {
    throw new Error(body.error ?? `${fallback}: ${response.status}`);
  }

  if (!body.post) {
    throw new Error(`${fallback}: missing post`);
  }
}
